//! `Table` and `Column`: typed columnar storage with declared keys (docs/06 §Tables).
//!
//! Nulls: `Num` uses NaN, `Str` and `Date` use `None`; `Bool` has no null (missing becomes
//! `false`). Dates are days since 1970-01-01 (see [`crate::date`]).

use crate::value::Value;
use crate::{date, DataError};
use datars_math::{Hash64, StableHash};
use datars_scene::{Key, KeyPart};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::sync::Arc;

/// A column's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Num,
    Str,
    Bool,
    Date,
}

impl ColumnType {
    pub fn name(self) -> &'static str {
        match self {
            ColumnType::Num => "num",
            ColumnType::Str => "str",
            ColumnType::Bool => "bool",
            ColumnType::Date => "date",
        }
    }
}

/// One column of values. See the module docs for nulls.
#[derive(Clone, Debug)]
pub enum Column {
    Num(Vec<f64>),
    Str(Vec<Option<Arc<str>>>),
    Bool(Vec<bool>),
    Date(Vec<Option<i32>>),
}

impl PartialEq for Column {
    /// Value equality where null equals null (NaN == NaN), so tables with gaps compare equal.
    fn eq(&self, other: &Column) -> bool {
        match (self, other) {
            (Column::Num(a), Column::Num(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan())),
            (Column::Str(a), Column::Str(b)) => a == b,
            (Column::Bool(a), Column::Bool(b)) => a == b,
            (Column::Date(a), Column::Date(b)) => a == b,
            _ => false,
        }
    }
}

impl Column {
    /// A string column from text (empty strings stay empty strings, not null).
    pub fn strs<S: AsRef<str>>(v: &[S]) -> Column {
        Column::Str(v.iter().map(|s| Some(Arc::from(s.as_ref()))).collect())
    }
    /// A string column with nulls.
    pub fn strs_opt<S: AsRef<str>>(v: &[Option<S>]) -> Column {
        Column::Str(v.iter().map(|s| s.as_ref().map(|s| Arc::from(s.as_ref()))).collect())
    }
    /// A date column from `YYYY-MM-DD` / `YYYY-MM` / `YYYY` text; unparseable text becomes null.
    pub fn dates<S: AsRef<str>>(v: &[S]) -> Column {
        Column::Date(v.iter().map(|s| date::parse_date_lenient(s.as_ref())).collect())
    }
    /// An all-null column.
    pub fn nulls(ty: ColumnType, n: usize) -> Column {
        match ty {
            ColumnType::Num => Column::Num(vec![f64::NAN; n]),
            ColumnType::Str => Column::Str(vec![None; n]),
            ColumnType::Bool => Column::Bool(vec![false; n]),
            ColumnType::Date => Column::Date(vec![None; n]),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Column::Num(v) => v.len(),
            Column::Str(v) => v.len(),
            Column::Bool(v) => v.len(),
            Column::Date(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn ty(&self) -> ColumnType {
        match self {
            Column::Num(_) => ColumnType::Num,
            Column::Str(_) => ColumnType::Str,
            Column::Bool(_) => ColumnType::Bool,
            Column::Date(_) => ColumnType::Date,
        }
    }

    /// The value at row `i` (`Value::Null` for nulls and out-of-range rows).
    pub fn get(&self, i: usize) -> Value {
        match self {
            Column::Num(v) => v.get(i).map_or(Value::Null, |x| if x.is_nan() { Value::Null } else { Value::Num(*x) }),
            Column::Str(v) => v.get(i).and_then(|s| s.clone()).map_or(Value::Null, Value::Str),
            Column::Bool(v) => v.get(i).map_or(Value::Null, |b| Value::Bool(*b)),
            Column::Date(v) => v.get(i).copied().flatten().map_or(Value::Null, Value::Date),
        }
    }
    pub fn is_null(&self, i: usize) -> bool {
        match self {
            Column::Num(v) => v.get(i).is_none_or(|x| x.is_nan()),
            Column::Str(v) => v.get(i).is_none_or(Option::is_none),
            Column::Bool(v) => i >= v.len(),
            Column::Date(v) => v.get(i).is_none_or(Option::is_none),
        }
    }
    pub fn null_count(&self) -> usize {
        match self {
            Column::Num(v) => v.iter().filter(|x| x.is_nan()).count(),
            Column::Str(v) => v.iter().filter(|x| x.is_none()).count(),
            Column::Bool(_) => 0,
            Column::Date(v) => v.iter().filter(|x| x.is_none()).count(),
        }
    }
    /// Row `i` on a number line: numbers, dates as days, booleans as 0/1; strings and nulls NaN.
    pub fn f64_at(&self, i: usize) -> f64 {
        match self {
            Column::Num(v) => v.get(i).copied().unwrap_or(f64::NAN),
            Column::Date(v) => v.get(i).copied().flatten().map_or(f64::NAN, |d| d as f64),
            Column::Bool(v) => v.get(i).map_or(f64::NAN, |b| if *b { 1.0 } else { 0.0 }),
            Column::Str(_) => f64::NAN,
        }
    }
    /// The whole column on a number line (see [`Column::f64_at`]).
    pub fn to_f64(&self) -> Vec<f64> {
        match self {
            Column::Num(v) => v.clone(),
            _ => (0..self.len()).map(|i| self.f64_at(i)).collect(),
        }
    }
    pub fn as_num(&self) -> Option<&[f64]> {
        match self {
            Column::Num(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&[Option<Arc<str>>]> {
        match self {
            Column::Str(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<&[bool]> {
        match self {
            Column::Bool(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_date(&self) -> Option<&[Option<i32>]> {
        match self {
            Column::Date(v) => Some(v),
            _ => None,
        }
    }
    /// All values, in order.
    pub fn values(&self) -> Vec<Value> {
        (0..self.len()).map(|i| self.get(i)).collect()
    }

    /// Rows `idx`, in that order (gather).
    pub fn take(&self, idx: &[usize]) -> Column {
        match self {
            Column::Num(v) => Column::Num(idx.iter().map(|&i| v[i]).collect()),
            Column::Str(v) => Column::Str(idx.iter().map(|&i| v[i].clone()).collect()),
            Column::Bool(v) => Column::Bool(idx.iter().map(|&i| v[i]).collect()),
            Column::Date(v) => Column::Date(idx.iter().map(|&i| v[i]).collect()),
        }
    }
    /// Rows `idx` where `None` produces a null (for outer joins and reshaping).
    pub fn take_opt(&self, idx: &[Option<usize>]) -> Column {
        match self {
            Column::Num(v) => Column::Num(idx.iter().map(|i| i.map_or(f64::NAN, |i| v[i])).collect()),
            Column::Str(v) => Column::Str(idx.iter().map(|i| i.and_then(|i| v[i].clone())).collect()),
            Column::Bool(v) => Column::Bool(idx.iter().map(|i| i.is_some_and(|i| v[i])).collect()),
            Column::Date(v) => Column::Date(idx.iter().map(|i| i.and_then(|i| v[i])).collect()),
        }
    }
    /// This column followed by `other` (same type).
    pub fn concat(&self, other: &Column) -> Option<Column> {
        Some(match (self, other) {
            (Column::Num(a), Column::Num(b)) => Column::Num(a.iter().chain(b).copied().collect()),
            (Column::Str(a), Column::Str(b)) => Column::Str(a.iter().chain(b).cloned().collect()),
            (Column::Bool(a), Column::Bool(b)) => Column::Bool(a.iter().chain(b).copied().collect()),
            (Column::Date(a), Column::Date(b)) => Column::Date(a.iter().chain(b).copied().collect()),
            _ => return None,
        })
    }

    /// Build a column of type `ty` from values, converting where it makes sense: numbers ↔ dates
    /// (days), booleans → 0/1, anything → text; everything else becomes null.
    pub fn from_values(ty: ColumnType, values: &[Value]) -> Column {
        match ty {
            ColumnType::Num => Column::Num(values.iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect()),
            ColumnType::Str => Column::Str(
                values
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => Some(s.clone()),
                        v if v.is_null() => None,
                        v => Some(Arc::from(v.to_string().as_str())),
                    })
                    .collect(),
            ),
            ColumnType::Bool => Column::Bool(values.iter().map(|v| matches!(v, Value::Bool(true))).collect()),
            ColumnType::Date => Column::Date(
                values
                    .iter()
                    .map(|v| match v {
                        Value::Date(d) => Some(*d),
                        Value::Num(x) if x.fract() == 0.0 && x.abs() < 1e9 => Some(*x as i32),
                        Value::Str(s) => date::parse_date_lenient(s),
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }

    /// The narrowest type that holds every non-null value without loss: all booleans → Bool, all
    /// numbers → Num, all dates → Date, otherwise Str. All-null → `fallback`.
    pub fn infer_type(values: &[Value], fallback: ColumnType) -> ColumnType {
        let mut seen: Option<ColumnType> = None;
        for v in values {
            let t = match v {
                v if v.is_null() => continue,
                Value::Bool(_) => ColumnType::Bool,
                Value::Num(_) => ColumnType::Num,
                Value::Date(_) => ColumnType::Date,
                _ => ColumnType::Str,
            };
            match seen {
                None => seen = Some(t),
                Some(s) if s == t => {}
                Some(_) => return ColumnType::Str,
            }
        }
        seen.unwrap_or(fallback)
    }
}

/// A column's name, type and null count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColumnType,
    pub nulls: usize,
}

/// What a table looks like: fields in order, key columns, row count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub fields: Vec<Field>,
    pub key: Vec<String>,
    pub rows: usize,
}

impl Schema {
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }
}

/// A typed, keyed, columnar table. All columns have the same length. `key` names the columns
/// whose values identify a row (empty = rows are identified by index, which is unstable).
/// `version` is bumped by live sources on every change; transforms carry it through.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<(String, Column)>,
    pub key: Vec<String>,
    pub version: u64,
}

impl Table {
    /// An empty table (no columns, no rows).
    pub fn new(name: &str) -> Table {
        Table { name: name.to_string(), ..Table::default() }
    }

    /// A table from named columns; checks equal lengths and unique names.
    pub fn from_columns<S: Into<String>>(name: &str, columns: Vec<(S, Column)>) -> Result<Table, DataError> {
        let t = Table { name: name.to_string(), columns: columns.into_iter().map(|(n, c)| (n.into(), c)).collect(), key: Vec::new(), version: 0 };
        t.validate()?;
        Ok(t)
    }

    /// Declare the key columns and validate them (present, non-null, unique).
    pub fn with_key(mut self, key: &[&str]) -> Result<Table, DataError> {
        self.key = key.iter().map(|k| k.to_string()).collect();
        self.validate_keys()?;
        Ok(self)
    }

    /// Structural checks: every column the same length, names unique, key columns present.
    pub fn validate(&self) -> Result<(), DataError> {
        let n = self.len();
        for (i, (name, c)) in self.columns.iter().enumerate() {
            if c.len() != n {
                return Err(DataError::LengthMismatch { column: name.clone(), expected: n, found: c.len() });
            }
            if self.columns[..i].iter().any(|(m, _)| m == name) {
                return Err(DataError::Invalid(format!("column \"{name}\" appears twice")));
            }
        }
        for k in &self.key {
            self.require(k)?;
        }
        Ok(())
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.columns.first().map_or(0, |c| c.1.len())
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Number of columns.
    pub fn width(&self) -> usize {
        self.columns.len()
    }
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.0.as_str()).collect()
    }
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.0 == name)
    }
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.0 == name).map(|c| &c.1)
    }
    pub fn column_mut(&mut self, name: &str) -> Option<&mut Column> {
        self.columns.iter_mut().find(|c| c.0 == name).map(|c| &mut c.1)
    }
    /// The column, or an error naming the columns that do exist.
    pub fn require(&self, name: &str) -> Result<&Column, DataError> {
        self.column(name).ok_or_else(|| self.unknown(name))
    }
    pub(crate) fn unknown(&self, name: &str) -> DataError {
        DataError::UnknownColumn { name: name.to_string(), available: self.columns.iter().map(|c| c.0.clone()).collect() }
    }
    pub fn num(&self, name: &str) -> Option<&[f64]> {
        self.column(name).and_then(Column::as_num)
    }
    pub fn str(&self, name: &str) -> Option<&[Option<Arc<str>>]> {
        self.column(name).and_then(Column::as_str)
    }
    pub fn bool(&self, name: &str) -> Option<&[bool]> {
        self.column(name).and_then(Column::as_bool)
    }
    pub fn date(&self, name: &str) -> Option<&[Option<i32>]> {
        self.column(name).and_then(Column::as_date)
    }
    /// The value at (`column`, `row`); `Value::Null` when either is missing.
    pub fn get(&self, column: &str, row: usize) -> Value {
        self.column(column).map_or(Value::Null, |c| c.get(row))
    }
    /// Every column's value at `row`, in column order.
    pub fn row(&self, row: usize) -> Vec<Value> {
        self.columns.iter().map(|c| c.1.get(row)).collect()
    }

    /// This table with `column` added (or replaced, keeping its position).
    pub fn with_column(mut self, name: &str, column: Column) -> Result<Table, DataError> {
        if !self.columns.is_empty() && column.len() != self.len() {
            return Err(DataError::LengthMismatch { column: name.to_string(), expected: self.len(), found: column.len() });
        }
        match self.column_index(name) {
            Some(i) => self.columns[i].1 = column,
            None => self.columns.push((name.to_string(), column)),
        }
        Ok(self)
    }

    /// The named columns, in the order given. The key survives if all its columns are selected;
    /// otherwise it is cleared (a partial key may not be unique).
    pub fn select(&self, names: &[&str]) -> Result<Table, DataError> {
        let mut columns = Vec::with_capacity(names.len());
        for n in names {
            columns.push((n.to_string(), self.require(n)?.clone()));
        }
        let key = if self.key.iter().all(|k| names.contains(&k.as_str())) { self.key.clone() } else { Vec::new() };
        Ok(Table { name: self.name.clone(), columns, key, version: self.version })
    }

    /// This table without the named columns (unknown names are an error).
    pub fn drop_columns(&self, names: &[&str]) -> Result<Table, DataError> {
        for n in names {
            self.require(n)?;
        }
        let keep: Vec<&str> = self.columns.iter().map(|c| c.0.as_str()).filter(|c| !names.contains(c)).collect();
        self.select(&keep)
    }

    /// Rename a column (the key follows).
    pub fn rename(&self, from: &str, to: &str) -> Result<Table, DataError> {
        let i = self.column_index(from).ok_or_else(|| self.unknown(from))?;
        if from != to && self.column_index(to).is_some() {
            return Err(DataError::Invalid(format!("can't rename \"{from}\" to \"{to}\": a column \"{to}\" already exists")));
        }
        let mut t = self.clone();
        t.columns[i].0 = to.to_string();
        for k in &mut t.key {
            if k == from {
                *k = to.to_string();
            }
        }
        Ok(t)
    }

    /// Rows `idx`, in that order (every column gathered). Keeps name, key and version.
    pub fn take_rows(&self, idx: &[usize]) -> Table {
        Table {
            name: self.name.clone(),
            columns: self.columns.iter().map(|(n, c)| (n.clone(), c.take(idx))).collect(),
            key: self.key.clone(),
            version: self.version,
        }
    }

    /// Whether rows have a declared key (otherwise identity is the row index, which breaks object
    /// constancy on filters and sorts — the linter flags it, P3).
    pub fn has_stable_key(&self) -> bool {
        !self.key.is_empty()
    }

    /// The identity of row `i`: one key part per key column (integral numbers and dates → `Int`,
    /// strings → `Str`, other numbers → their shortest text). Without a key: `(Int(i),)`.
    pub fn row_key(&self, i: usize) -> Key {
        if self.key.is_empty() {
            return Key::one(KeyPart::Int(i as i64));
        }
        Key::new(self.key.iter().map(|k| self.get(k, i).to_key_part()).collect())
    }
    /// Alias of [`Table::row_key`] (the name used in docs/dev/contracts.md).
    pub fn key_of(&self, i: usize) -> Key {
        self.row_key(i)
    }
    /// Every row's key.
    pub fn keys(&self) -> Vec<Key> {
        (0..self.len()).map(|i| self.row_key(i)).collect()
    }

    /// Check the declared key: its columns exist, have no nulls, and no two rows share a key.
    /// Duplicates are reported with up to three examples (key and rows). No key → Ok.
    pub fn validate_keys(&self) -> Result<(), DataError> {
        let cols: Vec<&Column> = self.key.iter().map(|k| self.require(k)).collect::<Result<_, _>>()?;
        if cols.is_empty() {
            return Ok(());
        }
        for (k, c) in self.key.iter().zip(&cols) {
            if let Some(row) = (0..c.len()).find(|&i| c.is_null(i)) {
                return Err(DataError::NullKey { column: k.clone(), row });
            }
        }
        // Whether any key repeats: the rows sorted by key, then equal neighbours — no allocation per
        // row. (The map below, built for every table, took 150 ms of a contour chart's opening on a
        // phone-class CPU.) Only a table with a repeat goes on to it, for the error's details. The
        // comparison is `Value`'s order, so the two agree on what counts as the same key.
        let by: Vec<(&Column, bool)> = cols.iter().map(|c| (*c, false)).collect();
        let same = |a: usize, b: usize| crate::transform::cmp_rows(&by, a, b) == std::cmp::Ordering::Equal;
        // Keys already in order (generated ids, rows sorted upstream) need no sort: one pass.
        if (1..self.len()).all(|i| crate::transform::cmp_rows(&by, i - 1, i) == std::cmp::Ordering::Less) {
            return Ok(());
        }
        let mut idx: Vec<usize> = (0..self.len()).collect();
        idx.sort_unstable_by(|&a, &b| crate::transform::cmp_rows(&by, a, b));
        if !idx.windows(2).any(|w| same(w[0], w[1])) {
            return Ok(());
        }
        let mut seen: BTreeMap<Vec<Value>, Vec<usize>> = BTreeMap::new();
        let mut order: Vec<Vec<Value>> = Vec::new(); // duplicated keys, in order of first repeat
        let mut count = 0;
        for i in 0..self.len() {
            let k: Vec<Value> = cols.iter().map(|c| c.get(i)).collect();
            let rows = seen.entry(k.clone()).or_default();
            rows.push(i);
            if rows.len() > 1 {
                count += 1;
                if rows.len() == 2 {
                    order.push(k);
                }
            }
        }
        if count == 0 {
            return Ok(());
        }
        let examples = order.iter().take(3).map(|k| (self.row_key(seen[k][0]).to_string(), seen[k].clone())).collect();
        Err(DataError::DuplicateKeys { key: self.key.clone(), count, examples })
    }

    pub fn schema(&self) -> Schema {
        Schema {
            fields: self.columns.iter().map(|(n, c)| Field { name: n.clone(), ty: c.ty(), nulls: c.null_count() }).collect(),
            key: self.key.clone(),
            rows: self.len(),
        }
    }

    /// A stable content hash (name, key, column names, types and values; not `version`), identical
    /// on every target. Nulls hash alike; −0 and 0 hash alike.
    pub fn hash(&self) -> u64 {
        self.hash64()
    }

    /// The JSON columnar form (see the `Serialize` impl).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    pub fn from_json(s: &str) -> Result<Table, DataError> {
        serde_json::from_str(s).map_err(DataError::from)
    }
}

impl StableHash for Column {
    fn stable_hash(&self, h: &mut Hash64) {
        h.u8(self.ty() as u8);
        h.u64(self.len() as u64);
        match self {
            Column::Num(v) => v.iter().for_each(|x| h.f64(*x)),
            Column::Str(v) => v.iter().for_each(|s| match s {
                None => h.u8(0),
                Some(s) => {
                    h.u8(1);
                    h.str(s)
                }
            }),
            Column::Bool(v) => v.iter().for_each(|b| h.u8(*b as u8)),
            Column::Date(v) => v.iter().for_each(|d| match d {
                None => h.u8(0),
                Some(d) => {
                    h.u8(1);
                    h.u32(*d as u32)
                }
            }),
        }
    }
}

impl StableHash for Table {
    fn stable_hash(&self, h: &mut Hash64) {
        h.str(&self.name);
        h.u64(self.key.len() as u64);
        self.key.iter().for_each(|k| h.str(k));
        h.u64(self.columns.len() as u64);
        for (n, c) in &self.columns {
            h.str(n);
            c.stable_hash(h);
        }
    }
}

// ---- serde: the JSON columnar form -------------------------------------------------------------
//
// {"name": "t", "key": ["id"], "version": 3,
//  "columns": [{"name": "id", "type": "str", "values": ["a", null]},
//              {"name": "v", "type": "num", "values": [1.5, null]},
//              {"name": "d", "type": "date", "values": ["2024-01-31", null]}]}
//
// Nulls are `null`; non-finite numbers also become `null` (JSON has no NaN/Infinity).

#[derive(Serialize)]
struct ColumnOut<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    ty: ColumnType,
    values: ValuesOut<'a>,
}

struct ValuesOut<'a>(&'a Column);

impl Serialize for ValuesOut<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = s.serialize_seq(Some(self.0.len()))?;
        match self.0 {
            Column::Num(v) => {
                for x in v {
                    seq.serialize_element(&x.is_finite().then_some(*x))?;
                }
            }
            Column::Str(v) => {
                for x in v {
                    seq.serialize_element(&x.as_deref())?;
                }
            }
            Column::Bool(v) => {
                for x in v {
                    seq.serialize_element(x)?;
                }
            }
            Column::Date(v) => {
                for x in v {
                    seq.serialize_element(&x.map(date::format_date))?;
                }
            }
        }
        seq.end()
    }
}

#[derive(Serialize)]
struct TableOut<'a> {
    name: &'a str,
    key: &'a [String],
    version: u64,
    columns: Vec<ColumnOut<'a>>,
}

impl Serialize for Table {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        TableOut {
            name: &self.name,
            key: &self.key,
            version: self.version,
            columns: self.columns.iter().map(|(n, c)| ColumnOut { name: n, ty: c.ty(), values: ValuesOut(c) }).collect(),
        }
        .serialize(s)
    }
}

/// `{"type": "num", "values": [...]}`
impl Serialize for Column {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Column", 2)?;
        st.serialize_field("type", &self.ty())?;
        st.serialize_field("values", &ValuesOut(self))?;
        st.end()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CellIn {
    Bool(bool),
    Num(f64),
    Str(String),
}

fn column_from_cells(name: &str, ty: ColumnType, cells: Vec<Option<CellIn>>) -> Result<Column, String> {
    let bad = |i: usize, what: &str| format!("column \"{name}\" value {i}: expected {what}");
    Ok(match ty {
        ColumnType::Num => Column::Num(
            cells
                .into_iter()
                .enumerate()
                .map(|(i, c)| match c {
                    None => Ok(f64::NAN),
                    Some(CellIn::Num(v)) => Ok(v),
                    Some(_) => Err(bad(i, "a number or null")),
                })
                .collect::<Result<_, _>>()?,
        ),
        ColumnType::Str => Column::Str(
            cells
                .into_iter()
                .enumerate()
                .map(|(i, c)| match c {
                    None => Ok(None),
                    Some(CellIn::Str(s)) => Ok(Some(Arc::from(s.as_str()))),
                    Some(_) => Err(bad(i, "a string or null")),
                })
                .collect::<Result<_, _>>()?,
        ),
        ColumnType::Bool => Column::Bool(
            cells
                .into_iter()
                .enumerate()
                .map(|(i, c)| match c {
                    None => Ok(false),
                    Some(CellIn::Bool(b)) => Ok(b),
                    Some(_) => Err(bad(i, "a boolean")),
                })
                .collect::<Result<_, _>>()?,
        ),
        ColumnType::Date => Column::Date(
            cells
                .into_iter()
                .enumerate()
                .map(|(i, c)| match c {
                    None => Ok(None),
                    Some(CellIn::Str(s)) => date::parse_date_lenient(&s).map(Some).ok_or_else(|| bad(i, "a date (YYYY-MM-DD)")),
                    Some(CellIn::Num(v)) if v.fract() == 0.0 && v.abs() < 1e9 => Ok(Some(v as i32)),
                    Some(_) => Err(bad(i, "a date (YYYY-MM-DD) or null")),
                })
                .collect::<Result<_, _>>()?,
        ),
    })
}

#[derive(Deserialize)]
struct ColumnIn {
    #[serde(rename = "type")]
    ty: ColumnType,
    values: Vec<Option<CellIn>>,
}

#[derive(Deserialize)]
struct NamedColumnIn {
    name: String,
    #[serde(rename = "type")]
    ty: ColumnType,
    values: Vec<Option<CellIn>>,
}

#[derive(Deserialize)]
struct TableIn {
    #[serde(default)]
    name: String,
    #[serde(default)]
    key: Vec<String>,
    #[serde(default)]
    version: u64,
    columns: Vec<NamedColumnIn>,
}

impl<'de> Deserialize<'de> for Column {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Column, D::Error> {
        let c = ColumnIn::deserialize(d)?;
        column_from_cells("", c.ty, c.values).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Table {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Table, D::Error> {
        let t = TableIn::deserialize(d)?;
        let mut columns = Vec::with_capacity(t.columns.len());
        for c in t.columns {
            let col = column_from_cells(&c.name, c.ty, c.values).map_err(serde::de::Error::custom)?;
            columns.push((c.name, col));
        }
        let table = Table { name: t.name, columns, key: t.key, version: t.version };
        table.validate().map_err(serde::de::Error::custom)?;
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Table {
        Table::from_columns(
            "parties",
            vec![
                ("party", Column::strs(&["S", "M", "SD"])),
                ("share", Column::Num(vec![30.3, 19.1, f64::NAN])),
                ("in_gov", Column::Bool(vec![false, true, false])),
                ("since", Column::Date(vec![Some(0), None, Some(10)])),
            ],
        )
        .unwrap()
    }

    #[test]
    fn accessors() {
        let t = t();
        assert_eq!(t.len(), 3);
        assert_eq!(t.width(), 4);
        assert_eq!(t.column_index("share"), Some(1));
        assert_eq!(t.num("share").unwrap()[1], 19.1);
        assert!(t.num("party").is_none());
        assert_eq!(t.str("party").unwrap()[2].as_deref(), Some("SD"));
        assert_eq!(t.get("share", 2), Value::Null);
        assert_eq!(t.get("since", 0), Value::Date(0));
        assert_eq!(t.schema().field("share").unwrap().nulls, 1);
        assert!(matches!(t.require("nope"), Err(DataError::UnknownColumn { .. })));
    }

    #[test]
    fn select_rename_with_column() {
        let t = t().with_key(&["party"]).unwrap();
        let s = t.select(&["share", "party"]).unwrap();
        assert_eq!(s.column_names(), vec!["share", "party"]);
        assert_eq!(s.key, vec!["party"]);
        assert!(t.select(&["share"]).unwrap().key.is_empty());
        let r = t.rename("party", "p").unwrap();
        assert_eq!(r.key, vec!["p"]);
        assert!(t.rename("party", "share").is_err());
        let w = t.clone().with_column("x", Column::Num(vec![1.0, 2.0, 3.0])).unwrap();
        assert_eq!(w.width(), 5);
        assert!(t.clone().with_column("x", Column::Num(vec![1.0])).is_err());
        let replaced = t.clone().with_column("share", Column::Num(vec![0.0; 3])).unwrap();
        assert_eq!(replaced.column_index("share"), Some(1));
    }

    #[test]
    fn json_round_trip() {
        let t = t().with_key(&["party"]).unwrap();
        let j = t.to_json();
        assert!(j.contains(r#""values":[30.3,19.1,null]"#), "{j}");
        assert!(j.contains(r#""values":["1970-01-01",null,"1970-01-11"]"#), "{j}");
        let back = Table::from_json(&j).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.hash(), t.hash());
    }

    #[test]
    fn hash_is_content_sensitive() {
        let a = t();
        let mut b = t();
        b.version = 9;
        assert_eq!(a.hash(), b.hash(), "version is not content");
        let c = t().with_column("share", Column::Num(vec![30.3, 19.1, 0.0])).unwrap();
        assert_ne!(a.hash(), c.hash());
    }
}
