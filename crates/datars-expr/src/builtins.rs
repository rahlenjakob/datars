//! Built-in functions and value methods. Deterministic: transcendental math goes through
//! `datars_math::m`, and every function is total (defined for every input, NaN in, NaN out).

use crate::value::{self, Value};
use datars_math::m;
use std::sync::Arc;

/// Every built-in function name. Anything else that is called is a host function.
pub(crate) const NAMES: &[&str] = &[
    "abs", "floor", "ceil", "trunc", "sqrt", "cbrt", "exp", "log", "log10", "log2", "sin", "cos", "tan", "asin",
    "acos", "atan", "sign", "round", "pow", "atan2", "clamp", "lerp", "min", "max", "hypot", "String", "Number",
    "Boolean", "isNaN", "isFinite", "rand", "randn",
];

/// The subset reachable as `Math.<name>(…)`.
pub(crate) const MATH: &[&str] = &[
    "abs", "floor", "ceil", "trunc", "sqrt", "cbrt", "exp", "log", "log10", "log2", "sin", "cos", "tan", "asin",
    "acos", "atan", "sign", "round", "pow", "atan2", "min", "max", "hypot", "clamp", "lerp",
];

/// `(min, max)` argument counts of a built-in (`usize::MAX` = variadic), or `None` if `name` isn't one.
pub(crate) fn arity(name: &str) -> Option<(usize, usize)> {
    Some(match name {
        "round" | "rand" | "randn" => (1, 2),
        "pow" | "atan2" => (2, 2),
        "clamp" | "lerp" => (3, 3),
        "min" | "max" | "hypot" => (0, usize::MAX),
        n if NAMES.contains(&n) => (1, 1),
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum F1 {
    Abs,
    Floor,
    Ceil,
    Trunc,
    Sqrt,
    Cbrt,
    Exp,
    Log,
    Log10,
    Log2,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sign,
    Round,
    /// `rand(key)`: [`rand`] with stream 0.
    Rand,
    /// `randn(key)`: [`randn`] with stream 0.
    Randn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum F2 {
    Pow,
    Atan2,
    /// `round(x, digits)`.
    RoundTo,
    /// `rand(key, stream)`.
    Rand,
    /// `randn(key, stream)`.
    Randn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum F3 {
    Clamp,
    Lerp,
}

/// Variadic numeric folds, evaluated left to right from the identity (so the scalar and vector
/// evaluators perform the same operations in the same order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FN {
    Min,
    Max,
    Hypot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Conv {
    String,
    Number,
    Boolean,
    IsNaN,
    IsFinite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    F1(F1),
    F2(F2),
    F3(F3),
    FN(FN),
    Conv(Conv),
}

/// Resolves a built-in call with `nargs` arguments (arity already checked).
pub(crate) fn resolve(name: &str, nargs: usize) -> Option<Builtin> {
    use Builtin as B;
    Some(match name {
        "abs" => B::F1(F1::Abs),
        "floor" => B::F1(F1::Floor),
        "ceil" => B::F1(F1::Ceil),
        "trunc" => B::F1(F1::Trunc),
        "sqrt" => B::F1(F1::Sqrt),
        "cbrt" => B::F1(F1::Cbrt),
        "exp" => B::F1(F1::Exp),
        "log" => B::F1(F1::Log),
        "log10" => B::F1(F1::Log10),
        "log2" => B::F1(F1::Log2),
        "sin" => B::F1(F1::Sin),
        "cos" => B::F1(F1::Cos),
        "tan" => B::F1(F1::Tan),
        "asin" => B::F1(F1::Asin),
        "acos" => B::F1(F1::Acos),
        "atan" => B::F1(F1::Atan),
        "sign" => B::F1(F1::Sign),
        "round" if nargs == 2 => B::F2(F2::RoundTo),
        "rand" if nargs == 2 => B::F2(F2::Rand),
        "rand" => B::F1(F1::Rand),
        "randn" if nargs == 2 => B::F2(F2::Randn),
        "randn" => B::F1(F1::Randn),
        "round" => B::F1(F1::Round),
        "pow" => B::F2(F2::Pow),
        "atan2" => B::F2(F2::Atan2),
        "clamp" => B::F3(F3::Clamp),
        "lerp" => B::F3(F3::Lerp),
        "min" => B::FN(FN::Min),
        "max" => B::FN(FN::Max),
        "hypot" => B::FN(FN::Hypot),
        "String" => B::Conv(Conv::String),
        "Number" => B::Conv(Conv::Number),
        "Boolean" => B::Conv(Conv::Boolean),
        "isNaN" => B::Conv(Conv::IsNaN),
        "isFinite" => B::Conv(Conv::IsFinite),
        _ => return None,
    })
}

/// JS `Math.round`: nearest integer, ties toward +∞. (`x - floor(x)` is exact for every finite x.)
#[inline]
pub(crate) fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// JS `Math.sign`: NaN and ±0 map to themselves.
#[inline]
pub(crate) fn js_sign(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 {
        x
    } else if x > 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Exact powers of ten for `round(x, digits)`.
pub(crate) const P10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20,
    1e21, 1e22,
];

/// `round(x, digits)`: round to `digits` decimals (negative digits round to tens, hundreds, …).
#[inline]
pub(crate) fn round_to(x: f64, digits: f64) -> f64 {
    if digits.is_nan() {
        return f64::NAN;
    }
    let d = digits.trunc();
    let p = P10[d.abs().min(22.0) as usize];
    if d >= 0.0 {
        js_round(x * p) / p
    } else {
        js_round(x / p) * p
    }
}

/// The bits a number hashes by: -0 is 0 (the same number), so `rand(-0)` is `rand(0)`.
fn key_bits(x: f64) -> u64 {
    if x == 0.0 {
        0
    } else {
        x.to_bits()
    }
}

/// `rand(key, stream)`: a pseudo-random number in [0, 1) that is a pure function of its arguments
/// — the same key and stream give the same number on every platform, every time. Per-row
/// randomness without state: `rand(i)` for row `i`, `rand(i, 2)` for an independent second draw
/// (jitter, synthetic data, sampling). A hash of both numbers' bits (two rounds of SplitMix64), so
/// neighbouring keys give unrelated values. NaN in, NaN out.
#[inline]
pub(crate) fn rand(key: f64, stream: f64) -> f64 {
    if key.is_nan() || stream.is_nan() {
        return f64::NAN;
    }
    let h = datars_math::mix64(datars_math::mix64(key_bits(key)) ^ key_bits(stream).rotate_left(29));
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// `randn(key, stream)`: a standard normal draw (mean 0, deviation 1) that is a pure function of
/// its arguments, like [`rand`] (Box–Muller over two independent hashes). NaN in, NaN out.
#[inline]
pub(crate) fn randn(key: f64, stream: f64) -> f64 {
    if key.is_nan() || stream.is_nan() {
        return f64::NAN;
    }
    let base = datars_math::mix64(key_bits(key)) ^ key_bits(stream).rotate_left(29);
    let u1 = (datars_math::mix64(base) >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
    let u2 = (datars_math::mix64(base ^ 0xA5A5_5A5A_C3C3_3C3C) >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
    // 1 - u1 ∈ (0, 1]: the logarithm is finite.
    (-2.0 * m::ln(1.0 - u1)).sqrt() * m::cos(std::f64::consts::TAU * u2)
}

/// JS `Math.min` for two values: NaN wins, and `-0` is smaller than `0`.
#[inline]
pub(crate) fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            a
        } else {
            b
        }
    } else if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub(crate) fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a == b {
        if a.is_sign_negative() {
            b
        } else {
            a
        }
    } else if a > b {
        a
    } else {
        b
    }
}

/// `clamp(v, lo, hi)`. NaN stays NaN (a missing value is not clamped into range). Written with
/// comparisons rather than `f64::max/min`, whose choice between ±0 is unspecified.
#[inline]
pub(crate) fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v.is_nan() {
        v
    } else if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

impl F1 {
    #[inline(always)]
    pub(crate) fn apply(self, x: f64) -> f64 {
        match self {
            F1::Abs => x.abs(),
            F1::Floor => x.floor(),
            F1::Ceil => x.ceil(),
            F1::Trunc => x.trunc(),
            F1::Sqrt => x.sqrt(),
            F1::Cbrt => m::cbrt(x),
            F1::Exp => m::exp(x),
            F1::Log => m::ln(x),
            F1::Log10 => m::log10(x),
            F1::Log2 => m::log2(x),
            F1::Sin => m::sin(x),
            F1::Cos => m::cos(x),
            F1::Tan => m::tan(x),
            F1::Asin => m::asin(x),
            F1::Acos => m::acos(x),
            F1::Atan => m::atan(x),
            F1::Sign => js_sign(x),
            F1::Round => js_round(x),
            F1::Rand => rand(x, 0.0),
            F1::Randn => randn(x, 0.0),
        }
    }
}

impl F2 {
    #[inline(always)]
    pub(crate) fn apply(self, a: f64, b: f64) -> f64 {
        match self {
            F2::Pow => value::js_pow(a, b),
            F2::Atan2 => m::atan2(a, b),
            F2::RoundTo => round_to(a, b),
            F2::Rand => rand(a, b),
            F2::Randn => randn(a, b),
        }
    }
}

impl F3 {
    #[inline(always)]
    pub(crate) fn apply(self, a: f64, b: f64, c: f64) -> f64 {
        match self {
            F3::Clamp => clamp(a, b, c),
            F3::Lerp => datars_math::lerp(a, b, c),
        }
    }
}

impl FN {
    pub(crate) fn identity(self) -> f64 {
        match self {
            FN::Min => f64::INFINITY,
            FN::Max => f64::NEG_INFINITY,
            FN::Hypot => 0.0,
        }
    }

    #[inline(always)]
    pub(crate) fn apply(self, acc: f64, x: f64) -> f64 {
        match self {
            FN::Min => js_min(acc, x),
            FN::Max => js_max(acc, x),
            FN::Hypot => m::hypot(acc, x),
        }
    }
}

impl Conv {
    pub(crate) fn apply(self, v: &Value) -> Value {
        match self {
            Conv::String => Value::Str(v.to_str()),
            Conv::Number => Value::Num(v.to_number()),
            Conv::Boolean => Value::Bool(v.truthy()),
            Conv::IsNaN => Value::Bool(v.to_number().is_nan()),
            Conv::IsFinite => Value::Bool(v.to_number().is_finite()),
        }
    }
}

/// Methods on values (strings; `includes`/`indexOf`/`length` also on array literals). Indices
/// count Unicode scalar values (JS counts UTF-16 units; they agree outside the astral planes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    Length,
    ToUpperCase,
    ToLowerCase,
    Trim,
    Slice,
    StartsWith,
    EndsWith,
    Includes,
    IndexOf,
    ToString,
}

impl Method {
    pub(crate) fn from_name(name: &str) -> Option<Method> {
        Some(match name {
            "length" => Method::Length,
            "toUpperCase" => Method::ToUpperCase,
            "toLowerCase" => Method::ToLowerCase,
            "trim" => Method::Trim,
            "slice" => Method::Slice,
            "startsWith" => Method::StartsWith,
            "endsWith" => Method::EndsWith,
            "includes" => Method::Includes,
            "indexOf" => Method::IndexOf,
            "toString" => Method::ToString,
            _ => return None,
        })
    }

    pub(crate) fn list() -> &'static str {
        "`.length`, `.toUpperCase()`, `.toLowerCase()`, `.trim()`, `.slice(a, b)`, `.startsWith(s)`, \
         `.endsWith(s)`, `.includes(x)`, `.indexOf(x)`, `.toString()`"
    }

    pub(crate) fn arity(self) -> (usize, usize) {
        match self {
            Method::Length | Method::ToUpperCase | Method::ToLowerCase | Method::Trim | Method::ToString => (0, 0),
            Method::Slice => (1, 2),
            Method::StartsWith | Method::EndsWith | Method::Includes | Method::IndexOf => (1, 1),
        }
    }

    /// Methods on string values. A null receiver gives null; numbers and booleans are converted
    /// with `String(x)` first.
    pub(crate) fn apply(self, recv: &Value, args: &[Value]) -> Value {
        if matches!(recv, Value::Null) {
            return Value::Null;
        }
        let s = recv.to_str();
        let arg_str = || args.first().map_or_else(|| Arc::from("undefined"), Value::to_str);
        match self {
            Method::Length => Value::Num(s.chars().count() as f64),
            Method::ToUpperCase => Value::from(s.to_uppercase()),
            Method::ToLowerCase => Value::from(s.to_lowercase()),
            Method::Trim => Value::str(s.trim()),
            Method::ToString => Value::Str(s),
            Method::Slice => {
                let len = s.chars().count();
                let start = rel_index(args.first().map_or(0.0, Value::to_num), len);
                let end = args.get(1).map_or(len, |v| rel_index(v.to_num(), len));
                if start >= end {
                    Value::str("")
                } else {
                    Value::from(s.chars().skip(start).take(end - start).collect::<String>())
                }
            }
            Method::StartsWith => Value::Bool(s.starts_with(&*arg_str())),
            Method::EndsWith => Value::Bool(s.ends_with(&*arg_str())),
            Method::Includes => Value::Bool(s.contains(&*arg_str())),
            Method::IndexOf => {
                let pos = s.find(&*arg_str()).map_or(-1.0, |b| s[..b].chars().count() as f64);
                Value::Num(pos)
            }
        }
    }
}

/// JS relative index: NaN → 0, negative counts from the end, clamped to `[0, len]`.
fn rel_index(x: f64, len: usize) -> usize {
    let x = if x.is_nan() { 0.0 } else { x.trunc() };
    let len_f = len as f64;
    let r = if x < 0.0 { (len_f + x).max(0.0) } else { x.min(len_f) };
    r as usize
}

/// `obj[index]` on a value: the character of a string at an integer index, else null.
pub(crate) fn index_value(obj: &Value, index: &Value) -> Value {
    let Value::Str(s) = obj else { return Value::Null };
    match pick_index(index, usize::MAX) {
        Some(i) => s.chars().nth(i).map_or(Value::Null, |c| Value::from(c.to_string())),
        None => Value::Null,
    }
}

/// An array/string index: a non-negative integer number below `len`.
pub(crate) fn pick_index(index: &Value, len: usize) -> Option<usize> {
    let Value::Num(x) = index else { return None };
    if *x >= 0.0 && x.trunc() == *x && *x < len as f64 {
        Some(*x as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_math_edge_cases() {
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(0.49999999999999994), 0.0);
        assert_eq!(round_to(1.2345, 2.0), 1.23);
        assert_eq!(round_to(1234.0, -2.0), 1200.0);
        assert!(js_min(1.0, f64::NAN).is_nan());
        assert!(js_min(0.0, -0.0).is_sign_negative());
        assert!(js_max(-0.0, 0.0).is_sign_positive());
        assert!(clamp(f64::NAN, 0.0, 1.0).is_nan());
        assert_eq!(clamp(5.0, 0.0, 1.0), 1.0);
        assert!(value::js_pow(1.0, f64::INFINITY).is_nan());
        assert_eq!(value::js_pow(f64::NAN, 0.0), 1.0);
    }

    #[test]
    fn string_methods() {
        let s = Value::str("Hello, wörld");
        assert_eq!(Method::Slice.apply(&s, &[Value::Num(-5.0)]), Value::str("wörld"));
        assert_eq!(Method::Slice.apply(&s, &[Value::Num(0.0), Value::Num(5.0)]), Value::str("Hello"));
        assert_eq!(Method::IndexOf.apply(&s, &[Value::str("r")]), Value::Num(9.0));
        assert_eq!(Method::Length.apply(&s, &[]), Value::Num(12.0));
        assert_eq!(Method::ToUpperCase.apply(&Value::Null, &[]), Value::Null);
        assert_eq!(Method::Length.apply(&Value::Num(123.0), &[]), Value::Num(3.0));
        assert_eq!(index_value(&s, &Value::Num(1.0)), Value::str("e"));
        assert_eq!(index_value(&s, &Value::Num(1.5)), Value::Null);
    }
}
