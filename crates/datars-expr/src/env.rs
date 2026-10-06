//! What an expression can see: columns of the current table, signals, and host functions.

use crate::types::{Type, TypeEnv};
use crate::value::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// The evaluation environment. Implemented by the engine over its tables and signal store.
///
/// Host functions (`call`) must be pure: the evaluator may call them once for arguments that are
/// the same on every row, or once per row, and may evaluate both branches of a conditional.
pub trait Env {
    fn column(&self, name: &str) -> Option<ColumnView<'_>>;
    fn signal(&self, name: &str) -> Option<Value>;
    /// Host functions: scales (`"scale.y"`), `"format"`, signal methods (`"selected.has"`), …
    /// `None` evaluates to null.
    fn call(&self, name: &str, args: &[Value]) -> Option<Value>;
    /// Whether `name` is a signal. A bare name (`time`) reads the signal when there is one and a
    /// same-named column otherwise; `d.time` always reads the row. Without this rule a data
    /// column would silently hide a signal.
    fn has_signal(&self, _name: &str) -> bool {
        false
    }
}

/// A borrowed column. Numeric nulls are NaN; string nulls are `None`. Rows past the end of a
/// column read as null.
#[derive(Clone, Copy, Debug)]
pub enum ColumnView<'a> {
    Num(&'a [f64]),
    Str(&'a [Option<Arc<str>>]),
    Bool(&'a [bool]),
}

impl ColumnView<'_> {
    pub fn len(&self) -> usize {
        match self {
            ColumnView::Num(s) => s.len(),
            ColumnView::Str(s) => s.len(),
            ColumnView::Bool(s) => s.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The value at `row` (null past the end; NaN stays `Num(NaN)`).
    pub fn get(&self, row: usize) -> Value {
        match self {
            ColumnView::Num(s) => Value::Num(s.get(row).copied().unwrap_or(f64::NAN)),
            ColumnView::Str(s) => s.get(row).cloned().flatten().map_or(Value::Null, Value::Str),
            ColumnView::Bool(s) => s.get(row).map_or(Value::Null, |b| Value::Bool(*b)),
        }
    }

    pub fn type_of(&self) -> Type {
        match self {
            ColumnView::Num(_) => Type::Num,
            ColumnView::Str(_) => Type::Str,
            ColumnView::Bool(_) => Type::Bool,
        }
    }
}

/// No columns, no signals, no host functions: for constant expressions.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyEnv;

impl Env for EmptyEnv {
    fn column(&self, _: &str) -> Option<ColumnView<'_>> {
        None
    }
    fn signal(&self, _: &str) -> Option<Value> {
        None
    }
    fn call(&self, _: &str, _: &[Value]) -> Option<Value> {
        None
    }
}

impl TypeEnv for EmptyEnv {
    fn column_type(&self, _: &str) -> Option<Type> {
        None
    }
    fn signal_type(&self, _: &str) -> Option<Type> {
        None
    }
    fn call_type(&self, _: &str, _: &[Type]) -> Option<Type> {
        None
    }
}

/// An owned column, for [`MapEnv`].
#[derive(Clone, Debug, PartialEq)]
pub enum OwnedColumn {
    Num(Vec<f64>),
    Str(Vec<Option<Arc<str>>>),
    Bool(Vec<bool>),
}

impl OwnedColumn {
    pub fn view(&self) -> ColumnView<'_> {
        match self {
            OwnedColumn::Num(v) => ColumnView::Num(v),
            OwnedColumn::Str(v) => ColumnView::Str(v),
            OwnedColumn::Bool(v) => ColumnView::Bool(v),
        }
    }
}

/// A host function for [`MapEnv`].
pub type HostFn = Box<dyn Fn(&[Value]) -> Option<Value> + Send + Sync>;

/// A simple owned environment (tests, tools, small hosts). Implements both [`Env`] and [`TypeEnv`]
/// (types come from the column variants and signal values; every registered function returns `Any`).
#[derive(Default)]
pub struct MapEnv {
    pub columns: BTreeMap<String, OwnedColumn>,
    pub signals: BTreeMap<String, Value>,
    pub functions: BTreeMap<String, HostFn>,
}

impl MapEnv {
    pub fn new() -> MapEnv {
        MapEnv::default()
    }

    pub fn num(mut self, name: &str, values: Vec<f64>) -> MapEnv {
        self.columns.insert(name.into(), OwnedColumn::Num(values));
        self
    }

    pub fn strs(mut self, name: &str, values: &[Option<&str>]) -> MapEnv {
        self.columns.insert(name.into(), OwnedColumn::Str(values.iter().map(|s| s.map(Arc::from)).collect()));
        self
    }

    pub fn bools(mut self, name: &str, values: Vec<bool>) -> MapEnv {
        self.columns.insert(name.into(), OwnedColumn::Bool(values));
        self
    }

    pub fn signal(mut self, name: &str, value: impl Into<Value>) -> MapEnv {
        self.signals.insert(name.into(), value.into());
        self
    }

    pub fn function(mut self, name: &str, f: impl Fn(&[Value]) -> Option<Value> + Send + Sync + 'static) -> MapEnv {
        self.functions.insert(name.into(), Box::new(f));
        self
    }
}

impl fmt::Debug for MapEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapEnv")
            .field("columns", &self.columns)
            .field("signals", &self.signals)
            .field("functions", &self.functions.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Env for MapEnv {
    fn column(&self, name: &str) -> Option<ColumnView<'_>> {
        self.columns.get(name).map(OwnedColumn::view)
    }
    fn signal(&self, name: &str) -> Option<Value> {
        self.signals.get(name).cloned()
    }
    fn call(&self, name: &str, args: &[Value]) -> Option<Value> {
        self.functions.get(name).and_then(|f| f(args))
    }
}

impl TypeEnv for MapEnv {
    fn column_type(&self, name: &str) -> Option<Type> {
        self.columns.get(name).map(|c| c.view().type_of())
    }
    fn signal_type(&self, name: &str) -> Option<Type> {
        self.signals.get(name).map(Value::type_of)
    }
    fn call_type(&self, name: &str, _: &[Type]) -> Option<Type> {
        self.functions.contains_key(name).then_some(Type::Any)
    }
    fn column_names(&self) -> Vec<String> {
        self.columns.keys().cloned().collect()
    }
    fn signal_names(&self) -> Vec<String> {
        self.signals.keys().cloned().collect()
    }
}
