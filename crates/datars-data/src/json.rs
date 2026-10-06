//! JSON → `Table`. Accepts an array of records (`[{"a": 1, "b": "x"}, …]`, columns in order of
//! first appearance, missing fields null), an object of columns (`{"a": [1, 2], "b": ["x", "y"]}`),
//! or the table's own columnar serde form (`{"name", "key", "columns": [{"name", "type",
//! "values"}]}`). Types come from the JSON values: numbers → Num, booleans → Bool, strings →
//! Date when every one is an ISO date, else Str; mixed columns become text.

use crate::table::{Column, ColumnType, Table};
use crate::value::Value;
use crate::{date, DataError};
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use std::fmt;
use std::sync::Arc;

/// A JSON value that keeps object keys in document order (column order matters; serde_json's own
/// `Value` sorts keys unless a workspace-wide feature is enabled).
enum J {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<J, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = J;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON")
            }
            fn visit_unit<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_none<E>(self) -> Result<J, E> {
                Ok(J::Null)
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<J, D::Error> {
                J::deserialize(d)
            }
            fn visit_bool<E>(self, v: bool) -> Result<J, E> {
                Ok(J::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<J, E> {
                Ok(J::Num(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<J, E> {
                Ok(J::Num(v as f64))
            }
            fn visit_f64<E>(self, v: f64) -> Result<J, E> {
                Ok(J::Num(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<J, E> {
                Ok(J::Str(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<J, E> {
                Ok(J::Str(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut v = Vec::new();
                while let Some(x) = a.next_element()? {
                    v.push(x);
                }
                Ok(J::Arr(v))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<J, A::Error> {
                let mut v: Vec<(String, J)> = Vec::new();
                while let Some((k, x)) = a.next_entry::<String, J>()? {
                    // A repeated key replaces the earlier value (JSON's usual last-wins rule).
                    match v.iter_mut().find(|e| e.0 == k) {
                        Some(e) => e.1 = x,
                        None => v.push((k, x)),
                    }
                }
                Ok(J::Obj(v))
            }
        }
        d.deserialize_any(V)
    }
}

/// Read JSON bytes into a table (see the module docs for the accepted shapes).
pub fn read_json(bytes: &[u8]) -> Result<Table, DataError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let parse_err = DataError::from;
    let doc: J = serde_json::from_slice(bytes).map_err(parse_err)?;
    match doc {
        J::Arr(records) => from_records(records),
        J::Obj(fields) => {
            // `columns` holding objects can only be the serde form: plain columns hold scalars.
            let serde_form = fields.iter().any(|(k, v)| k == "columns" && matches!(v, J::Arr(cs) if !cs.is_empty() && cs.iter().all(|c| matches!(c, J::Obj(_)))));
            if serde_form {
                return serde_json::from_slice::<Table>(bytes).map_err(parse_err);
            }
            from_columns(fields)
        }
        _ => Err(DataError::Parse { line: 1, column: 1, message: "expected an array of records or an object of columns".into() }),
    }
}

fn from_records(records: Vec<J>) -> Result<Table, DataError> {
    let n = records.len();
    let mut names: Vec<String> = Vec::new();
    let mut cells: Vec<Vec<Value>> = Vec::new();
    for (i, r) in records.into_iter().enumerate() {
        let J::Obj(fields) = r else {
            return Err(DataError::Invalid(format!("record {i} is not an object (expected [{{\"column\": value, …}}, …])")));
        };
        for (k, v) in fields {
            let c = match names.iter().position(|x| *x == k) {
                Some(c) => c,
                None => {
                    names.push(k.clone());
                    cells.push(vec![Value::Null; n]);
                    names.len() - 1
                }
            };
            cells[c][i] = scalar(v).map_err(|what| DataError::Invalid(format!("record {i}, field \"{k}\": {what}")))?;
        }
    }
    let columns = names.into_iter().zip(cells).map(|(name, vals)| (name, build(&vals))).collect();
    Table::from_columns("", columns)
}

fn from_columns(fields: Vec<(String, J)>) -> Result<Table, DataError> {
    let mut columns: Vec<(String, Column)> = Vec::new();
    for (name, v) in fields {
        let J::Arr(items) = v else {
            return Err(DataError::Invalid(format!("column \"{name}\" is not an array (expected {{\"column\": [values…], …}})")));
        };
        let vals = items
            .into_iter()
            .enumerate()
            .map(|(i, x)| scalar(x).map_err(|what| DataError::Invalid(format!("column \"{name}\", value {i}: {what}"))))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some((first, c)) = columns.first() {
            if c.len() != vals.len() {
                return Err(DataError::Invalid(format!("column \"{name}\" has {} values, column \"{first}\" has {}", vals.len(), c.len())));
            }
        }
        columns.push((name, build(&vals)));
    }
    Table::from_columns("", columns)
}

fn scalar(v: J) -> Result<Value, &'static str> {
    Ok(match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Num(x) => Value::Num(x),
        J::Str(s) => Value::Str(Arc::from(s.as_str())),
        J::Arr(_) | J::Obj(_) => return Err("nested arrays and objects aren't supported (flatten them first)"),
    })
}

/// Column type from the JSON types present (see module docs). Booleans with nulls become text,
/// since `Column::Bool` has no null.
fn build(vals: &[Value]) -> Column {
    let ty = Column::infer_type(vals, ColumnType::Str);
    match ty {
        ColumnType::Bool if vals.iter().any(Value::is_null) => Column::from_values(ColumnType::Str, vals),
        ColumnType::Str => {
            let all_dates = vals.iter().any(|v| !v.is_null()) && vals.iter().all(|v| match v {
                Value::Str(s) => date::parse_date(s).is_some(),
                v => v.is_null(),
            });
            Column::from_values(if all_dates { ColumnType::Date } else { ColumnType::Str }, vals)
        }
        ty => Column::from_values(ty, vals),
    }
}
