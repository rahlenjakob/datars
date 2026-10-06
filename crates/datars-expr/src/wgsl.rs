//! WGSL for the numeric subset, so per-instance channels can be computed on the GPU
//! (docs/06-data-and-reactivity.md).
//!
//! Columns become `inst.<name>` (the per-instance attribute struct) and signals `u.<name>` (the
//! uniform struct); both must be WGSL identifiers of type `f32`. Supported: number/boolean
//! literals, arithmetic (`**` → `pow`), comparisons, `!`, `&&`/`||` on booleans, `?:` (→ `select`),
//! and the math built-ins. The result is always `f32` (booleans become 0.0/1.0). Anything else —
//! strings, null, `??`, host calls, the row index, non-finite literals — returns `None`, and the
//! caller evaluates on the CPU instead.
//!
//! GPU float semantics differ from the CPU evaluator in the corners (f32 precision, NaN handling,
//! `pow` of negative bases, `x != 0.0` as the truthiness of a number): the GPU path is for visual
//! channels, where that's acceptable.

use crate::ast::{BinOp, Expr, UnOp};
use crate::builtins::{self, Builtin, F1, F2, F3, FN};
use crate::compile::compile;
use crate::print::is_identifier;
use crate::types::{Type, TypeEnv};
use crate::value::Value;

/// WGSL for `e`, or `None` if it's outside the numeric subset. Bare identifiers ([`Expr::Ident`])
/// are ambiguous without an environment and make this return `None`; use [`to_wgsl_with`].
pub fn to_wgsl(e: &Expr) -> Option<String> {
    top(&Gen { env: None }, e)
}

/// Like [`to_wgsl`], resolving bare identifiers (column first, then signal) and requiring every
/// column and signal read to be numeric.
pub fn to_wgsl_with(e: &Expr, env: &dyn TypeEnv) -> Option<String> {
    top(&Gen { env: Some(env) }, e)
}

fn top(g: &Gen<'_>, e: &Expr) -> Option<String> {
    if e.depth() > crate::ast::MAX_DEPTH {
        return None;
    }
    let (s, t) = g.gen(e, 0)?;
    Some(match t {
        Ty::F => s,
        Ty::B => format!("select(0.0, 1.0, {s})"),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ty {
    F,
    B,
}

struct Gen<'a> {
    env: Option<&'a dyn TypeEnv>,
}

/// WGSL keywords (can't be struct member names).
const KEYWORDS: &[&str] = &[
    "alias", "break", "case", "const", "const_assert", "continue", "continuing", "default", "diagnostic", "discard",
    "else", "enable", "false", "fn", "for", "if", "let", "loop", "override", "requires", "return", "struct",
    "switch", "true", "var", "while",
];

fn wgsl_ident(name: &str) -> bool {
    is_identifier(name)
        && name.is_ascii()
        && !name.contains('$')
        && !name.starts_with("__")
        && name != "_"
        && !KEYWORDS.contains(&name)
}

fn float(v: f64) -> Option<String> {
    if !v.is_finite() || v.abs() > f32::MAX as f64 {
        return None;
    }
    let s = format!("{:?}", v.abs());
    let s = if s.contains(['.', 'e']) { s } else { format!("{s}.0") };
    Some(if v.is_sign_negative() { format!("(-{s})") } else { s })
}

const MAX_DEPTH: u32 = crate::ast::MAX_DEPTH as u32;

/// The value of an input-free, host-call-free compound subtree, folded with the CPU semantics
/// (so `round(PI, 2)` is emitted as `3.14`, and `1 / 0` — a WGSL const-expression error — as
/// nothing at all).
fn constant(e: &Expr) -> Option<Value> {
    if matches!(e, Expr::Num { .. } | Expr::Bool { .. }) {
        return None;
    }
    let mut pure = true;
    e.visit(&mut |x| {
        let input = matches!(x, Expr::Field { .. } | Expr::Ident { .. } | Expr::Signal { .. } | Expr::Row { .. });
        let host = matches!(x, Expr::Call { name, .. } if builtins::arity(name).is_none());
        pure &= !input && !host;
    });
    if !pure {
        return None;
    }
    compile(e).ok()?.constant().cloned()
}

impl Gen<'_> {
    fn num_input(&self, prefix: &str, name: &str, ty: Option<Option<Type>>) -> Option<(String, Ty)> {
        if !wgsl_ident(name) {
            return None;
        }
        // With an environment, the input must exist and be numeric.
        if let Some(t) = ty {
            if !matches!(t, Some(Type::Num)) {
                return None;
            }
        }
        Some((format!("{prefix}.{name}"), Ty::F))
    }

    fn f(&self, e: &Expr, depth: u32) -> Option<String> {
        match self.gen(e, depth)? {
            (s, Ty::F) => Some(s),
            _ => None,
        }
    }

    fn gen(&self, e: &Expr, depth: u32) -> Option<(String, Ty)> {
        if depth > MAX_DEPTH {
            return None;
        }
        let d = depth + 1;
        if let Some(v) = constant(e) {
            return match v {
                Value::Num(x) => Some((float(x)?, Ty::F)),
                Value::Bool(b) => Some((b.to_string(), Ty::B)),
                _ => None,
            };
        }
        Some(match e {
            Expr::Num { value, .. } => (float(*value)?, Ty::F),
            Expr::Bool { value, .. } => ((if *value { "true" } else { "false" }).to_string(), Ty::B),
            Expr::Field { name, .. } => self.num_input("inst", name, self.env.map(|env| env.column_type(name)))?,
            Expr::Signal { name, .. } => self.num_input("u", name, self.env.map(|env| env.signal_type(name)))?,
            Expr::Ident { name, .. } => {
                let env = self.env?;
                if let Some(t) = env.column_type(name) {
                    self.num_input("inst", name, Some(Some(t)))?
                } else {
                    self.num_input("u", name, Some(env.signal_type(name)))?
                }
            }
            Expr::Unary { op, arg, .. } => {
                let (a, t) = self.gen(arg, d)?;
                match (op, t) {
                    (UnOp::Neg, Ty::F) => (format!("(-{a})"), Ty::F),
                    (UnOp::Pos, Ty::F) => (a, Ty::F),
                    (UnOp::Not, Ty::B) => (format!("(!{a})"), Ty::B),
                    (UnOp::Not, Ty::F) => (format!("({a} == 0.0)"), Ty::B),
                    _ => return None,
                }
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let (a, ta) = self.gen(lhs, d)?;
                let (b, tb) = self.gen(rhs, d)?;
                use BinOp::*;
                match (op, ta, tb) {
                    (Add | Sub | Mul | Div | Rem, Ty::F, Ty::F) => (format!("({a} {} {b})", op.symbol()), Ty::F),
                    (Pow, Ty::F, Ty::F) => (format!("pow({a}, {b})"), Ty::F),
                    (Lt | Le | Gt | Ge, Ty::F, Ty::F) => (format!("({a} {} {b})", op.symbol()), Ty::B),
                    (Eq | StrictEq, x, y) if x == y => (format!("({a} == {b})"), Ty::B),
                    (Ne | StrictNe, x, y) if x == y => (format!("({a} != {b})"), Ty::B),
                    (And | Or, Ty::B, Ty::B) => (format!("({a} {} {b})", op.symbol()), Ty::B),
                    _ => return None,
                }
            }
            Expr::Cond { test, then, otherwise, .. } => {
                let (t, tt) = self.gen(test, d)?;
                let t = if tt == Ty::F { format!("({t} != 0.0)") } else { t };
                let (a, ta) = self.gen(then, d)?;
                let (b, tb) = self.gen(otherwise, d)?;
                if ta != tb {
                    return None;
                }
                (format!("select({b}, {a}, {t})"), ta)
            }
            Expr::Call { name, args, .. } => (self.call(name, args, d)?, Ty::F),
            _ => return None,
        })
    }

    fn call(&self, name: &str, args: &[Expr], d: u32) -> Option<String> {
        let (lo, hi) = builtins::arity(name)?;
        if args.len() < lo || args.len() > hi {
            return None;
        }
        let a: Vec<String> = args.iter().map(|x| self.f(x, d)).collect::<Option<_>>()?;
        Some(match builtins::resolve(name, args.len())? {
            Builtin::F1(f) => {
                let x = &a[0];
                match f {
                    F1::Log10 => format!("(log({x}) * 0.4342944819032518)"),
                    F1::Cbrt => format!("(sign({x}) * pow(abs({x}), 0.3333333333333333))"),
                    F1::Round => format!("floor({x} + 0.5)"),
                    F1::Abs => format!("abs({x})"),
                    F1::Floor => format!("floor({x})"),
                    F1::Ceil => format!("ceil({x})"),
                    F1::Trunc => format!("trunc({x})"),
                    F1::Sqrt => format!("sqrt({x})"),
                    F1::Exp => format!("exp({x})"),
                    F1::Log => format!("log({x})"),
                    F1::Log2 => format!("log2({x})"),
                    F1::Sin => format!("sin({x})"),
                    F1::Cos => format!("cos({x})"),
                    F1::Tan => format!("tan({x})"),
                    F1::Asin => format!("asin({x})"),
                    F1::Acos => format!("acos({x})"),
                    F1::Atan => format!("atan({x})"),
                    F1::Sign => format!("sign({x})"),
                    // Hash-based: no WGSL counterpart that gives the same bits.
                    F1::Rand | F1::Randn => return None,
                }
            }
            Builtin::F2(F2::Rand | F2::Randn) => return None,
            Builtin::F2(F2::Pow) => format!("pow({}, {})", a[0], a[1]),
            Builtin::F2(F2::Atan2) => format!("atan2({}, {})", a[0], a[1]),
            Builtin::F2(F2::RoundTo) => {
                // Only constant digits: the scale factor must be a literal.
                let Expr::Num { value, .. } = &args[1] else { return None };
                let digits = value.trunc();
                if !digits.is_finite() || digits.abs() > 22.0 {
                    return None;
                }
                let p = float(builtins::P10[digits.abs() as usize])?;
                if digits >= 0.0 {
                    format!("(floor({} * {p} + 0.5) / {p})", a[0])
                } else {
                    format!("(floor({} / {p} + 0.5) * {p})", a[0])
                }
            }
            Builtin::F3(F3::Clamp) => format!("clamp({}, {}, {})", a[0], a[1], a[2]),
            Builtin::F3(F3::Lerp) => format!("mix({}, {}, {})", a[0], a[1], a[2]),
            Builtin::FN(f) => {
                let wf = match f {
                    FN::Min => "min",
                    FN::Max => "max",
                    FN::Hypot => {
                        return match a.len() {
                            1 => Some(format!("abs({})", a[0])),
                            2 => Some(format!("length(vec2<f32>({}, {}))", a[0], a[1])),
                            3 => Some(format!("length(vec3<f32>({}, {}, {}))", a[0], a[1], a[2])),
                            _ => None,
                        }
                    }
                };
                let mut it = a.into_iter();
                let first = it.next()?;
                it.fold(first, |acc, x| format!("{wf}({acc}, {x})"))
            }
            Builtin::Conv(_) => return None,
        })
    }
}
