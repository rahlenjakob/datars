//! `datars-graph` — the reactive graph (docs/06-data-and-reactivity.md).
//!
//! Inputs (sources, signals, the viewport, the theme) carry versions. Derived nodes declare their
//! dependencies and a pure compute function; `get` recomputes a node only when a dependency's
//! version changed since it was last computed, and memoizes otherwise. Evaluation is demand-driven
//! and depth-first in declared dependency order, so it is deterministic. Recompute counts are kept
//! for the dev tools ("what did this signal change invalidate?").

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub type Value = Arc<dyn Any + Send + Sync>;
type Compute = Arc<dyn Fn(&Deps) -> Result<Value, String> + Send + Sync>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    Unknown(String),
    Cycle(Vec<String>),
    Compute { node: String, message: String },
    Type(String),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::Unknown(n) => write!(f, "unknown node `{n}`"),
            GraphError::Cycle(p) => write!(f, "dependency cycle: {}", p.join(" → ")),
            GraphError::Compute { node, message } => write!(f, "`{node}`: {message}"),
            GraphError::Type(n) => write!(f, "`{n}` has a different type"),
        }
    }
}

enum Entry {
    Input { value: Value, version: u64 },
    Derived { deps: Vec<String>, compute: Compute, cache: Option<Cached> },
}

struct Cached {
    dep_versions: Vec<u64>,
    value: Value,
    version: u64,
}

/// The values of a derived node's dependencies, in declaration order.
pub struct Deps {
    names: Vec<String>,
    values: Vec<Value>,
}

impl Deps {
    pub fn get<T: Any + Send + Sync>(&self, name: &str) -> Result<&T, String> {
        let i = self.names.iter().position(|n| n == name).ok_or_else(|| format!("`{name}` is not a declared dependency"))?;
        self.values[i].downcast_ref::<T>().ok_or_else(|| format!("`{name}` has an unexpected type"))
    }
    pub fn at<T: Any + Send + Sync>(&self, i: usize) -> Result<&T, String> {
        self.values.get(i).and_then(|v| v.downcast_ref::<T>()).ok_or_else(|| format!("dependency {i} missing or of an unexpected type"))
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Default)]
pub struct Graph {
    nodes: BTreeMap<String, Entry>,
    clock: u64,
    recomputes: BTreeMap<String, u64>,
}

impl Graph {
    pub fn new() -> Graph {
        Graph::default()
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Set an input. The version bumps on every set.
    pub fn set<T: Any + Send + Sync>(&mut self, id: &str, value: T) {
        let v = self.tick();
        self.nodes.insert(id.to_string(), Entry::Input { value: Arc::new(value), version: v });
    }

    /// Set an input only if it differs from the current value (no version bump when equal).
    pub fn set_if_changed<T: Any + Send + Sync + PartialEq>(&mut self, id: &str, value: T) -> bool {
        if let Some(Entry::Input { value: old, .. }) = self.nodes.get(id) {
            if old.downcast_ref::<T>() == Some(&value) {
                return false;
            }
        }
        self.set(id, value);
        true
    }

    /// Define (or redefine) a derived node.
    pub fn define(&mut self, id: &str, deps: &[&str], compute: impl Fn(&Deps) -> Result<Value, String> + Send + Sync + 'static) {
        self.nodes.insert(id.to_string(), Entry::Derived { deps: deps.iter().map(|s| s.to_string()).collect(), compute: Arc::new(compute), cache: None });
    }

    pub fn contains(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn remove(&mut self, id: &str) {
        self.nodes.remove(id);
    }

    /// The node's current value, recomputing what's stale.
    pub fn get_value(&mut self, id: &str) -> Result<(Value, u64), GraphError> {
        let mut stack = Vec::new();
        self.eval(id, &mut stack)
    }

    pub fn get<T: Any + Send + Sync>(&mut self, id: &str) -> Result<Arc<T>, GraphError> {
        let (v, _) = self.get_value(id)?;
        v.downcast::<T>().map_err(|_| GraphError::Type(id.to_string()))
    }

    fn eval(&mut self, id: &str, stack: &mut Vec<String>) -> Result<(Value, u64), GraphError> {
        if stack.iter().any(|s| s == id) {
            let mut cyc = stack.clone();
            cyc.push(id.to_string());
            return Err(GraphError::Cycle(cyc));
        }
        let deps = match self.nodes.get(id) {
            None => return Err(GraphError::Unknown(id.to_string())),
            Some(Entry::Input { value, version }) => return Ok((value.clone(), *version)),
            Some(Entry::Derived { deps, .. }) => deps.clone(),
        };
        stack.push(id.to_string());
        let mut values = Vec::with_capacity(deps.len());
        let mut versions = Vec::with_capacity(deps.len());
        for d in &deps {
            let (v, ver) = self.eval(d, stack)?;
            values.push(v);
            versions.push(ver);
        }
        stack.pop();
        if let Some(Entry::Derived { cache: Some(c), .. }) = self.nodes.get(id) {
            if c.dep_versions == versions {
                return Ok((c.value.clone(), c.version));
            }
        }
        let compute = match self.nodes.get(id) {
            Some(Entry::Derived { compute, .. }) => compute.clone(),
            _ => unreachable!(),
        };
        let value = compute(&Deps { names: deps, values }).map_err(|message| GraphError::Compute { node: id.to_string(), message })?;
        let version = self.tick();
        *self.recomputes.entry(id.to_string()).or_default() += 1;
        if let Some(Entry::Derived { cache, .. }) = self.nodes.get_mut(id) {
            *cache = Some(Cached { dep_versions: versions, value: value.clone(), version });
        }
        Ok((value, version))
    }

    /// How many times each derived node has been computed (dev tools).
    pub fn recompute_counts(&self) -> &BTreeMap<String, u64> {
        &self.recomputes
    }

    /// Everything that (transitively) depends on `id` — what a change to it invalidates.
    pub fn dependents(&self, id: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut frontier = vec![id.to_string()];
        while let Some(cur) = frontier.pop() {
            for (name, e) in &self.nodes {
                if let Entry::Derived { deps, .. } = e {
                    if deps.contains(&cur) && out.insert(name.clone()) {
                        frontier.push(name.clone());
                    }
                }
            }
        }
        out
    }

    /// Node names with their dependencies (inputs have none) — for the graph view.
    pub fn describe(&self) -> Vec<(String, Vec<String>)> {
        self.nodes
            .iter()
            .map(|(k, e)| (k.clone(), if let Entry::Derived { deps, .. } = e { deps.clone() } else { Vec::new() }))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recomputes_only_what_changed() {
        let mut g = Graph::new();
        g.set("a", 2.0f64);
        g.set("b", 10.0f64);
        g.define("sum", &["a", "b"], |d| Ok(Arc::new(d.get::<f64>("a")? + d.get::<f64>("b")?)));
        g.define("double_a", &["a"], |d| Ok(Arc::new(d.get::<f64>("a")? * 2.0)));
        g.define("total", &["sum", "double_a"], |d| Ok(Arc::new(d.at::<f64>(0)? + d.at::<f64>(1)?)));
        assert_eq!(*g.get::<f64>("total").unwrap(), 16.0);
        assert_eq!(*g.get::<f64>("total").unwrap(), 16.0);
        assert_eq!(g.recompute_counts()["total"], 1, "memoized");
        g.set("b", 20.0);
        assert_eq!(*g.get::<f64>("total").unwrap(), 26.0);
        assert_eq!(g.recompute_counts()["double_a"], 1, "b doesn't invalidate double_a");
        assert_eq!(g.recompute_counts()["sum"], 2);
        assert!(!g.set_if_changed("a", 2.0f64), "same value: no bump");
        assert_eq!(*g.get::<f64>("total").unwrap(), 26.0);
        assert_eq!(g.recompute_counts()["total"], 2);
        assert_eq!(g.dependents("b").into_iter().collect::<Vec<_>>(), vec!["sum".to_string(), "total".to_string()]);
    }

    #[test]
    fn cycles_and_errors_are_reported() {
        let mut g = Graph::new();
        g.define("x", &["y"], |_| Ok(Arc::new(0u8)));
        g.define("y", &["x"], |_| Ok(Arc::new(0u8)));
        assert!(matches!(g.get::<u8>("x"), Err(GraphError::Cycle(_))));
        g.define("bad", &[], |_| Err("boom".into()));
        assert!(matches!(g.get::<u8>("bad"), Err(GraphError::Compute { .. })));
        assert!(matches!(g.get::<u8>("nope"), Err(GraphError::Unknown(_))));
    }
}
