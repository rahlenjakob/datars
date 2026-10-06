//! `Value`: one cell, typed. Used where code works across column types — group keys, join keys,
//! discrete scale domains, pivots — with a total order so it can key a `BTreeMap`.

use crate::date;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Num(f64),
    /// Days since 1970-01-01.
    Date(i32),
    Str(Arc<str>),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Arc::from(s))
    }
    /// Null, or a NaN number (numeric columns store null as NaN).
    pub fn is_null(&self) -> bool {
        match self {
            Value::Null => true,
            Value::Num(v) => v.is_nan(),
            _ => false,
        }
    }
    /// The value on a number line: numbers as themselves, dates as days, booleans as 0/1.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(v) if !v.is_nan() => Some(*v),
            Value::Date(d) => Some(*d as f64),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Num(_) => "num",
            Value::Date(_) => "date",
            Value::Str(_) => "str",
        }
    }
    fn rank(&self) -> u8 {
        match self {
            Value::Bool(_) => 0,
            Value::Num(v) if v.is_nan() => 4,
            Value::Num(_) => 1,
            Value::Date(_) => 2,
            Value::Str(_) => 3,
            Value::Null => 4,
        }
    }
    /// A key part for scene identity: integral numbers and dates become `Int`, everything else a
    /// string (non-integral numbers in their shortest round-trip form).
    pub fn to_key_part(&self) -> datars_scene::KeyPart {
        use datars_scene::KeyPart;
        match self {
            Value::Num(v) if v.fract() == 0.0 && v.abs() < 9.2e18 => KeyPart::Int(*v as i64),
            Value::Date(d) => KeyPart::Int(*d as i64),
            Value::Str(s) => KeyPart::Str(s.clone()),
            other => KeyPart::Str(Arc::from(other.to_string().as_str())),
        }
    }
}

/// Total order: booleans < numbers < dates < strings < null (nulls — and NaN — last). Numbers use
/// `datars_math::total_cmp` (−0 == 0); strings compare by code point (locale-independent).
impl Ord for Value {
    fn cmp(&self, other: &Value) -> Ordering {
        match (self, other) {
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Num(a), Value::Num(b)) => datars_math::total_cmp(*a, *b),
            (Value::Date(a), Value::Date(b)) => a.cmp(b),
            (Value::Str(a), Value::Str(b)) => a.cmp(b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Value) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Value) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}

impl From<f64> for Value {
    fn from(v: f64) -> Value {
        Value::Num(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Value {
        Value::Num(v as f64)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Value {
        Value::Num(v as f64)
    }
}
impl From<usize> for Value {
    fn from(v: usize) -> Value {
        Value::Num(v as f64)
    }
}
impl From<bool> for Value {
    fn from(v: bool) -> Value {
        Value::Bool(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Value {
        Value::Str(Arc::from(v))
    }
}
impl From<String> for Value {
    fn from(v: String) -> Value {
        Value::Str(Arc::from(v.as_str()))
    }
}
impl From<Arc<str>> for Value {
    fn from(v: Arc<str>) -> Value {
        Value::Str(v)
    }
}
impl From<&Value> for Value {
    fn from(v: &Value) -> Value {
        v.clone()
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

/// Plain text, not a formatted label: numbers in shortest round-trip form, dates as `YYYY-MM-DD`,
/// null as the empty string. (Locale-aware formatting belongs to `datars-text`.)
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => Ok(()),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(v) if v.is_nan() => Ok(()),
            Value::Num(v) => write!(f, "{}", crate::num::fmt_num(*v)),
            Value::Date(d) => write!(f, "{}", date::format_date(*d)),
            Value::Str(s) => write!(f, "{s}"),
        }
    }
}

/// JSON: `null`, booleans, numbers, strings; dates as `{"date": "YYYY-MM-DD"}`.
impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        match self {
            Value::Null => s.serialize_none(),
            Value::Bool(b) => s.serialize_bool(*b),
            Value::Num(v) if !v.is_finite() => s.serialize_none(),
            Value::Num(v) => s.serialize_f64(*v),
            Value::Str(v) => s.serialize_str(v),
            Value::Date(d) => {
                let mut m = s.serialize_map(Some(1))?;
                m.serialize_entry("date", &date::format_date(*d))?;
                m.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Null(()),
            Bool(bool),
            Num(f64),
            Str(String),
            Date { date: DateRepr },
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum DateRepr {
            Days(i32),
            Text(String),
        }
        Ok(match Option::<Raw>::deserialize(d)? {
            None | Some(Raw::Null(())) => Value::Null,
            Some(Raw::Bool(b)) => Value::Bool(b),
            Some(Raw::Num(v)) => Value::Num(v),
            Some(Raw::Str(s)) => Value::Str(Arc::from(s.as_str())),
            Some(Raw::Date { date: DateRepr::Days(v) }) => Value::Date(v),
            Some(Raw::Date { date: DateRepr::Text(s) }) => {
                Value::Date(date::parse_date_lenient(&s).ok_or_else(|| serde::de::Error::custom(format!("not a date: {s}")))?)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_order_and_equality() {
        let mut v = [Value::Null, Value::str("b"), Value::Num(f64::NAN), Value::Num(2.0), Value::Date(3), Value::Bool(true), Value::Num(-1.0), Value::str("a")];
        v.sort();
        assert_eq!(v[..6], [Value::Bool(true), Value::Num(-1.0), Value::Num(2.0), Value::Date(3), Value::str("a"), Value::str("b")]);
        assert!(v[6].is_null() && v[7].is_null());
        assert_eq!(Value::Num(0.0), Value::Num(-0.0));
        assert_ne!(Value::Num(1.0), Value::Date(1));
    }

    #[test]
    fn serde_round_trip() {
        let v = vec![Value::Null, Value::Bool(false), Value::Num(1.5), Value::str("x"), Value::Date(19_000)];
        let j = serde_json::to_string(&v).unwrap();
        assert_eq!(j, r#"[null,false,1.5,"x",{"date":"2022-01-08"}]"#);
        let back: Vec<Value> = serde_json::from_str(&j).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn key_parts() {
        use datars_scene::KeyPart;
        assert_eq!(Value::Num(3.0).to_key_part(), KeyPart::Int(3));
        assert_eq!(Value::Num(-0.0).to_key_part(), KeyPart::Int(0));
        assert_eq!(Value::Num(2.5).to_key_part(), KeyPart::str("2.5"));
        assert_eq!(Value::Date(7).to_key_part(), KeyPart::Int(7));
        assert_eq!(Value::str("S").to_key_part(), KeyPart::str("S"));
    }
}
