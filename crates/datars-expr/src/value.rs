//! Runtime values and the scalar semantics every evaluator shares.
//!
//! The rules, chosen to be JavaScript-like where that's harmless and stricter where JS coercions
//! would hide data bugs:
//!
//! - **Truthiness** as in JS: `null`, `false`, `0`, `NaN` and `""` are falsy.
//! - **NaN is the numeric null**: numeric columns store null as NaN, so `== null`, `??` and
//!   `isNaN` treat NaN and `null` alike (`NaN == NaN` is therefore true).
//! - **`+`** concatenates if either side is a string, else adds numerically.
//! - **Arithmetic** coerces: booleans are 0/1, `null` and strings are NaN (no implicit parsing —
//!   use `Number(s)`).
//! - **Ordering** (`<` …) compares two strings lexicographically (by code point), otherwise
//!   numerically; a string against a non-string is always false.
//! - **Equality** (`==` and `===` alike) never coerces across types: `1 == "1"` is false,
//!   `null == null` is true.

use crate::types::Type;
use datars_math::m;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// A runtime value. (Array literals exist only at compile time: they are lowered into the
/// operations that use them.) Serializes as plain JSON — null, number, string, boolean — so NaN
/// (the numeric null) round-trips as `Null`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Num(f64),
    Str(Arc<str>),
    Bool(bool),
}

pub(crate) static NULL: Value = Value::Null;

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Arc::from(s))
    }

    /// JS truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Num(x) => !(*x == 0.0 || x.is_nan()),
            Value::Str(s) => !s.is_empty(),
            Value::Bool(b) => *b,
        }
    }

    /// `null`, or NaN (the numeric null).
    pub fn is_null(&self) -> bool {
        match self {
            Value::Null => true,
            Value::Num(x) => x.is_nan(),
            _ => false,
        }
    }

    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(x) => Some(*x),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The arithmetic coercion: numbers as-is, booleans 0/1, everything else NaN.
    #[inline]
    pub fn to_num(&self) -> f64 {
        match self {
            Value::Num(x) => *x,
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Null | Value::Str(_) => f64::NAN,
        }
    }

    /// The explicit `Number(x)` conversion: like [`Value::to_num`], but strings are parsed as JS
    /// numeric literals (`" 1.5e3 "`, `"0x1f"`, `"-Infinity"`). Empty strings are NaN (JS: 0).
    pub fn to_number(&self) -> f64 {
        match self {
            Value::Str(s) => parse_number(s),
            v => v.to_num(),
        }
    }

    /// The `String(x)` conversion (JS number formatting).
    pub fn to_str(&self) -> Arc<str> {
        match self {
            Value::Str(s) => s.clone(),
            v => Arc::from(v.to_string()),
        }
    }

    /// The static type of this value (`Any` for null).
    pub fn type_of(&self) -> Type {
        match self {
            Value::Null => Type::Any,
            Value::Num(_) => Type::Num,
            Value::Str(_) => Type::Str,
            Value::Bool(_) => Type::Bool,
        }
    }

    /// Identity: like `==` on the enum, but numbers compare by bits (NaN is identical to NaN,
    /// `-0` is not identical to `0`). What determinism tests want.
    pub fn identical(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Num(a), Value::Num(b)) => a.to_bits() == b.to_bits(),
            (a, b) => a == b,
        }
    }
}

impl fmt::Display for Value {
    /// `String(x)`: `null`, `true`/`false`, JS number formatting, strings as-is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(x) => f.write_str(&js_number_string(*x)),
            Value::Str(s) => f.write_str(s),
        }
    }
}

impl From<f64> for Value {
    fn from(x: f64) -> Value {
        Value::Num(x)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::str(s)
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(Arc::from(s))
    }
}
impl From<Arc<str>> for Value {
    fn from(s: Arc<str>) -> Value {
        Value::Str(s)
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Value {
        v.map_or(Value::Null, Into::into)
    }
}

// ---- scalar operator semantics (shared by folding, the scalar and the vector evaluators) ----

pub(crate) fn add(a: &Value, b: &Value) -> Value {
    if matches!(a, Value::Str(_)) || matches!(b, Value::Str(_)) {
        let mut s = String::from(&*a.to_str());
        s.push_str(&b.to_str());
        Value::Str(Arc::from(s))
    } else {
        Value::Num(a.to_num() + b.to_num())
    }
}

/// `==` / `===`: no coercion across types; null-ish (null or NaN) equals only null-ish.
pub(crate) fn equals(a: &Value, b: &Value) -> bool {
    let (an, bn) = (a.is_null(), b.is_null());
    if an || bn {
        return an && bn;
    }
    match (a, b) {
        (Value::Num(x), Value::Num(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
}

#[inline(always)]
pub(crate) fn cmp_num(op: Cmp, x: f64, y: f64) -> bool {
    match op {
        Cmp::Lt => x < y,
        Cmp::Le => x <= y,
        Cmp::Gt => x > y,
        Cmp::Ge => x >= y,
    }
}

#[inline]
pub(crate) fn cmp_str(op: Cmp, x: &str, y: &str) -> bool {
    match op {
        Cmp::Lt => x < y,
        Cmp::Le => x <= y,
        Cmp::Gt => x > y,
        Cmp::Ge => x >= y,
    }
}

pub(crate) fn order(op: Cmp, a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Str(x), Value::Str(y)) => cmp_str(op, x, y),
        (Value::Str(_), _) | (_, Value::Str(_)) => false,
        _ => cmp_num(op, a.to_num(), b.to_num()),
    }
}

/// JS `%` (truncated remainder, the sign of the dividend).
#[inline]
pub(crate) fn js_rem(a: f64, b: f64) -> f64 {
    m::fmod(a, b)
}

/// JS `**`: C `pow` except `x ** NaN` and `(±1) ** ±Infinity` are NaN.
#[inline]
pub(crate) fn js_pow(a: f64, b: f64) -> f64 {
    if b.is_nan() || (a.abs() == 1.0 && b.is_infinite()) {
        f64::NAN
    } else {
        m::pow(a, b)
    }
}

/// JS `Number.prototype.toString()` for a finite or non-finite f64: shortest round-trip digits,
/// plain notation for 1e-7 < |x| < 1e21, exponent notation (`1e+21`, `1.5e-7`) otherwise.
pub fn js_number_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x == 0.0 {
        return "0".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    // Rust's `{:e}` is the shortest representation that round-trips, like JS.
    let e = format!("{x:e}");
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let mut s = String::with_capacity(k as usize + 8);
    if x < 0.0 {
        s.push('-');
    }
    if k <= n && n <= 21 {
        s.push_str(&digits);
        s.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        s.push_str(&digits[..n as usize]);
        s.push('.');
        s.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        s.push_str("0.");
        s.extend(std::iter::repeat_n('0', (-n) as usize));
        s.push_str(&digits);
    } else {
        s.push_str(&digits[..1]);
        if k > 1 {
            s.push('.');
            s.push_str(&digits[1..]);
        }
        s.push('e');
        s.push(if n > 0 { '+' } else { '-' });
        s.push_str(&(n - 1).abs().to_string());
    }
    s
}

/// `Number(s)` for strings: a JS numeric literal after trimming whitespace, else NaN.
pub(crate) fn parse_number(s: &str) -> f64 {
    let t = s.trim();
    match t {
        "" => return f64::NAN,
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let lower = t.get(..2).map(|p| p.to_ascii_lowercase());
    let radix = match lower.as_deref() {
        Some("0x") => 16,
        Some("0b") => 2,
        Some("0o") => 8,
        _ => 10,
    };
    if radix != 10 {
        let ds = &t[2..];
        if ds.is_empty() || !ds.chars().all(|c| c.is_digit(radix)) {
            return f64::NAN;
        }
        return ds.chars().fold(0.0, |acc, d| acc * radix as f64 + d.to_digit(radix).unwrap_or(0) as f64);
    }
    // Validate the decimal-literal shape ourselves: Rust's parser also accepts "inf" and "nan".
    let b = t.as_bytes();
    let mut i = 0;
    let mut out = String::with_capacity(t.len() + 4);
    if matches!(b.first(), Some(b'+' | b'-')) {
        if b[0] == b'-' {
            out.push('-');
        }
        i = 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int = &t[int_start..i];
    let mut frac = "";
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac = &t[fs..i];
    }
    if int.is_empty() && frac.is_empty() {
        return f64::NAN;
    }
    let mut exp = String::new();
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            exp.push(b[i] as char);
            i += 1;
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return f64::NAN;
        }
        exp.push_str(&t[es..i]);
    }
    if i != b.len() {
        return f64::NAN;
    }
    out.push_str(if int.is_empty() { "0" } else { int });
    out.push('.');
    out.push_str(if frac.is_empty() { "0" } else { frac });
    out.push('e');
    out.push_str(if exp.is_empty() { "0" } else { &exp });
    out.parse().unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_strings_match_js() {
        let cases = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (100.0, "100"),
            (0.1, "0.1"),
            (0.1 + 0.2, "0.30000000000000004"),
            (123.456, "123.456"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (1.5e-7, "1.5e-7"),
            (1e-6, "0.000001"),
            (0.001, "0.001"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (2.5e300, "2.5e+300"),
            (5e-324, "5e-324"),
        ];
        for (x, want) in cases {
            assert_eq!(js_number_string(x), want, "{x:e}");
        }
    }

    #[test]
    fn number_parsing() {
        assert_eq!(parse_number(" 1.5e3 "), 1500.0);
        assert_eq!(parse_number("0x1f"), 31.0);
        assert_eq!(parse_number(".5"), 0.5);
        assert_eq!(parse_number("5."), 5.0);
        assert_eq!(parse_number("-Infinity"), f64::NEG_INFINITY);
        for bad in ["", "abc", "inf", "nan", "1e", "1.2.3", "--1", "0x", "1 2"] {
            assert!(parse_number(bad).is_nan(), "{bad}");
        }
    }

    #[test]
    fn equality_and_truthiness() {
        assert!(equals(&Value::Null, &Value::Num(f64::NAN)));
        assert!(!equals(&Value::Num(1.0), &Value::str("1")));
        assert!(!equals(&Value::Num(1.0), &Value::Bool(true)));
        assert!(!Value::Num(f64::NAN).truthy());
        assert!(Value::str("0").truthy());
        assert!(!order(Cmp::Lt, &Value::Num(1.0), &Value::str("2")));
        assert!(order(Cmp::Lt, &Value::str("a"), &Value::str("b")));
        assert!(order(Cmp::Gt, &Value::Bool(true), &Value::Num(0.5)));
    }
}
