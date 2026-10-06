//! The program runtime: a small statechart (docs/08-programs.md). States set signals; events move
//! between states; chapters are parameterized sub-programs entered with a key and left with `back`.

use crate::resolve::json_to_value;
use datars_expr::Value;
use datars_ir::{Program, State};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
struct Frame {
    program: Program,
    current: usize,
    /// For chapters: (param name, key).
    param: Option<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Runtime {
    stack: Vec<Frame>,
}

impl Runtime {
    pub fn new(p: &Program) -> Runtime {
        let current = if p.initial.is_empty() { 0 } else { p.states.iter().position(|s| s.name == p.initial).unwrap_or(0) };
        Runtime { stack: vec![Frame { program: p.clone(), current, param: None }] }
    }

    fn top(&self) -> &Frame {
        self.stack.last().expect("program stack never empty")
    }

    pub fn state(&self) -> Option<&State> {
        let f = self.top();
        f.program.states.get(f.current)
    }

    pub fn state_name(&self) -> String {
        self.state().map(|s| s.name.clone()).unwrap_or_default()
    }

    /// Qualified name including chapters: `"europe/country:SWE/zoom"`.
    pub fn path(&self) -> String {
        self.stack
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let s = f.program.states.get(f.current).map(|s| s.name.clone()).unwrap_or_default();
                match (&f.param, i) {
                    (Some((_, k)), _) => format!("{k}:{s}"),
                    _ => s,
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    pub fn state_names(&self) -> Vec<String> {
        self.top().program.states.iter().map(|s| s.name.clone()).collect()
    }

    pub fn index(&self) -> usize {
        self.top().current
    }

    pub fn len(&self) -> usize {
        self.top().program.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn depth(&self) -> usize {
        self.stack.len() - 1
    }

    /// Signals the current state (and enclosing chapter parameters) set. Later frames win.
    pub fn signals(&self) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        for f in &self.stack {
            if let Some((p, k)) = &f.param {
                out.insert(p.clone(), Value::Str(Arc::from(k.as_str())));
            }
            if let Some(s) = f.program.states.get(f.current) {
                for (k, v) in &s.set {
                    let v = match (v, &f.param) {
                        (serde_json::Value::String(s), Some((p, key))) => serde_json::Value::String(s.replace(&format!("{{{p}}}"), key)),
                        _ => v.clone(),
                    };
                    out.insert(k.clone(), json_to_value(&v));
                }
            }
            out.insert("step".into(), Value::Num(f.current as f64));
            out.insert("state".into(), Value::Str(Arc::from(f.program.states.get(f.current).map(|s| s.name.as_str()).unwrap_or(""))));
            // The state's narration as signals, so a card on the canvas can tell the story
            // (`card({ text: e("narration.text") })`); "" where a state has none.
            let nar = f.program.states.get(f.current).and_then(|s| s.narration.as_ref());
            let sub = |t: &str| match &f.param {
                Some((p, key)) => t.replace(&format!("{{{p}}}"), key),
                None => t.to_string(),
            };
            out.insert("narration.title".into(), Value::Str(Arc::from(sub(nar.map_or("", |n| n.title.as_str())).as_str())));
            out.insert("narration.text".into(), Value::Str(Arc::from(sub(nar.map_or("", |n| n.text.as_str())).as_str())));
        }
        out
    }

    /// Handle an event. Returns true if the state changed.
    pub fn event(&mut self, ev: &str) -> bool {
        if ev == "back" {
            if self.stack.len() > 1 {
                self.stack.pop();
                return true;
            }
            return false;
        }
        let f = self.stack.last_mut().expect("stack");
        let n = f.program.states.len();
        if n == 0 {
            return false;
        }
        let cur_name = f.program.states[f.current].name.clone();
        // Explicit edges first.
        if let Some(e) = f.program.edges.iter().find(|e| (e.from == cur_name || e.from == "*") && e.on == ev) {
            if let Some(i) = f.program.states.iter().position(|s| s.name == e.to) {
                let changed = i != f.current;
                f.current = i;
                return changed;
            }
        }
        let next = match ev {
            "next" => (f.current + 1 < n).then_some(f.current + 1),
            "prev" => f.current.checked_sub(1),
            "first" => Some(0),
            "last" => Some(n - 1),
            _ => ev.strip_prefix("goto:").and_then(|name| f.program.states.iter().position(|s| s.name == name)),
        };
        match next {
            Some(i) if i != f.current => {
                f.current = i;
                true
            }
            _ => false,
        }
    }

    /// Enter chapter `name` with a datum key.
    pub fn enter_chapter(&mut self, name: &str, key: &str) -> bool {
        let Some(ch) = self.top().program.chapters.get(name).cloned() else { return false };
        self.stack.push(Frame { program: (*ch.program).clone(), current: 0, param: Some((ch.param.clone(), key.to_string())) });
        true
    }

    pub fn goto_index(&mut self, i: usize) -> bool {
        let f = self.stack.last_mut().expect("stack");
        if i < f.program.states.len() && i != f.current {
            f.current = i;
            true
        } else {
            false
        }
    }

    /// Seconds to hold a state in autoplay and film.
    pub fn hold(&self, i: usize) -> f64 {
        self.top().program.states.get(i).and_then(|s| s.hold).unwrap_or(2.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prog() -> Program {
        serde_json::from_value(serde_json::json!({
            "preset": "story",
            "states": [ { "name": "all", "set": { "focus": "" } }, { "name": "north", "set": { "focus": "SE" } }, { "name": "end" } ],
            "edges": [ { "from": "end", "on": "next", "to": "all" } ],
            "chapters": { "country": { "param": "c", "program": { "states": [ { "name": "zoom", "set": { "focus": "{c}" } } ] } } }
        }))
        .unwrap()
    }

    #[test]
    fn steps_edges_and_chapters() {
        let mut r = Runtime::new(&prog());
        assert_eq!(r.state_name(), "all");
        assert!(r.event("next"));
        assert_eq!(r.signals()["focus"], Value::Str(Arc::from("SE")));
        assert!(r.event("next"));
        assert!(r.event("next"), "explicit edge loops back");
        assert_eq!(r.state_name(), "all");
        assert!(!r.event("prev"), "no state before the first");
        assert!(r.enter_chapter("country", "NOR"));
        assert_eq!(r.signals()["focus"], Value::Str(Arc::from("NOR")));
        assert_eq!(r.signals()["c"], Value::Str(Arc::from("NOR")));
        assert_eq!(r.path(), "all/NOR:zoom");
        assert!(r.event("back"));
        assert_eq!(r.path(), "all");
        assert!(r.event("goto:end"));
        assert_eq!(r.state_name(), "end");
    }
}
