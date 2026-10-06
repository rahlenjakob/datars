//! Static types, checked at load time against the table schema and signal types.
//!
//! Types are deliberately coarse (`Num`, `Str`, `Bool`, `Any`): enough to catch the bugs that
//! matter (arithmetic on strings, comparing numbers with strings, unknown columns, wrong arity)
//! without a type language. `null` has type `Any`. Evaluation is total regardless: a program that
//! fails to typecheck still evaluates to something defined, it's just probably not what was meant.

use crate::ast::{BinOp, Expr, Span, UnOp, MAX_DEPTH};
use crate::builtins::{self, Builtin, Conv, Method};
use crate::error::TypeError;
use crate::parser::arity_message;
use crate::print::source;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Type {
    Num,
    Str,
    Bool,
    Any,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Type::Num => "number",
            Type::Str => "string",
            Type::Bool => "boolean",
            Type::Any => "any",
        })
    }
}

/// What the typechecker knows about the world: column and signal types, and host function
/// signatures. The `*_names` methods only improve error messages ("did you mean …").
pub trait TypeEnv {
    fn column_type(&self, name: &str) -> Option<Type>;
    fn signal_type(&self, name: &str) -> Option<Type>;
    /// Result type of host function `name` given argument types; `None` = unknown function.
    /// The default accepts any host call and returns `Any`.
    fn call_type(&self, name: &str, args: &[Type]) -> Option<Type> {
        let _ = (name, args);
        Some(Type::Any)
    }
    fn column_names(&self) -> Vec<String> {
        Vec::new()
    }
    fn signal_names(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Checks `e` and returns its result type, or the first error found (in evaluation order).
pub fn typecheck(e: &Expr, env: &dyn TypeEnv) -> Result<Type, TypeError> {
    if e.depth() > MAX_DEPTH {
        return err(e.span(), format!("expression is nested too deeply (more than {MAX_DEPTH} levels)"));
    }
    Checker { env }.check(e)
}

struct Checker<'a> {
    env: &'a dyn TypeEnv,
}

type TResult = Result<Type, TypeError>;

fn err<T>(span: Span, msg: impl Into<String>) -> Result<T, TypeError> {
    Err(TypeError::new(span, msg))
}

/// The least upper bound of two branch types.
fn unify(a: Type, b: Type) -> Type {
    if a == b {
        a
    } else {
        Type::Any
    }
}

fn concrete(t: Type) -> bool {
    t != Type::Any
}

impl Checker<'_> {
    /// Requires a number (or `Any`); `what` names the context for the message.
    fn want_num(&mut self, e: &Expr, what: &str) -> TResult {
        let t = self.check(e)?;
        if matches!(t, Type::Num | Type::Any) {
            Ok(t)
        } else {
            let hint = if t == Type::Str { " (convert with Number(…))" } else { "" };
            err(e.span(), format!("{what} needs a number, but `{}` is a {t}{hint}", source(e)))
        }
    }

    fn want_str(&mut self, e: &Expr, what: &str) -> TResult {
        let t = self.check(e)?;
        if matches!(t, Type::Str | Type::Any) {
            Ok(t)
        } else {
            err(e.span(), format!("{what} needs a string, but `{}` is a {t} (convert with String(…))", source(e)))
        }
    }

    fn check(&mut self, e: &Expr) -> TResult {
        match e {
            Expr::Null { .. } => Ok(Type::Any),
            Expr::Bool { .. } => Ok(Type::Bool),
            Expr::Num { .. } => Ok(Type::Num),
            Expr::Str { .. } => Ok(Type::Str),
            Expr::Row { .. } => Ok(Type::Num),
            Expr::Array { span, .. } => {
                err(*span, "an array literal can only be used with .includes(), .indexOf(), .length or [index]")
            }
            Expr::Field { name, span } => match self.env.column_type(name) {
                Some(t) => Ok(t),
                None => err(*span, format!("unknown column `{name}`{}", suggest(name, &self.env.column_names()))),
            },
            Expr::Signal { name, span } => match self.env.signal_type(name) {
                Some(t) => Ok(t),
                None => err(*span, format!("unknown signal `{name}`{}", suggest(name, &self.env.signal_names()))),
            },
            Expr::Ident { name, span } => {
                if let Some(t) = self.env.signal_type(name).or_else(|| self.env.column_type(name)) {
                    return Ok(t);
                }
                let mut known = self.env.column_names();
                known.extend(self.env.signal_names());
                err(*span, format!("unknown name `{name}` (not a column or a signal){}", suggest(name, &known)))
            }
            Expr::Unary { op, arg, .. } => match op {
                UnOp::Not => {
                    self.check(arg)?;
                    Ok(Type::Bool)
                }
                UnOp::Neg | UnOp::Pos => {
                    self.want_num(arg, &format!("unary `{}`", op.symbol()))?;
                    Ok(Type::Num)
                }
            },
            Expr::Binary { op, lhs, rhs, span } => self.binary(*op, lhs, rhs, *span),
            Expr::Cond { test, then, otherwise, .. } => {
                self.check(test)?;
                let a = self.check(then)?;
                let b = self.check(otherwise)?;
                Ok(unify(a, b))
            }
            Expr::Call { name, args, span } => self.call(name, args, *span),
            Expr::Method { recv, name, args, span } => self.method(recv, name, args, *span),
            Expr::Index { obj, index, span } => {
                if let Expr::Array { items, .. } = &**obj {
                    self.want_num(index, "an array index")?;
                    return self.items(items);
                }
                let t = self.check(obj)?;
                self.want_num(index, "an index")?;
                match t {
                    Type::Str | Type::Any => Ok(t),
                    _ => err(*span, format!("can't index a {t}: `{}`", source(obj))),
                }
            }
            Expr::Concat { parts, .. } => {
                for p in parts {
                    self.check(p)?;
                }
                Ok(Type::Str)
            }
        }
    }

    fn items(&mut self, items: &[Expr]) -> TResult {
        let mut t: Option<Type> = None;
        for it in items {
            let ti = self.check(it)?;
            t = Some(t.map_or(ti, |t| unify(t, ti)));
        }
        Ok(t.unwrap_or(Type::Any))
    }

    fn binary(&mut self, op: BinOp, lhs: &Expr, rhs: &Expr, span: Span) -> TResult {
        use BinOp::*;
        match op {
            And | Or | Coalesce => {
                let a = self.check(lhs)?;
                let b = self.check(rhs)?;
                Ok(unify(a, b))
            }
            Add => {
                let a = self.check(lhs)?;
                let b = self.check(rhs)?;
                match (a, b) {
                    (Type::Str, _) | (_, Type::Str) => Ok(Type::Str),
                    (Type::Num, Type::Num) => Ok(Type::Num),
                    (Type::Bool, t) | (t, Type::Bool) if concrete(t) => err(
                        span,
                        format!(
                            "can't add a {a} and a {b} in `{}`; convert with Number(…) or String(…)",
                            source_of_binary(op, lhs, rhs)
                        ),
                    ),
                    _ => Ok(Type::Any),
                }
            }
            Sub | Mul | Div | Rem | Pow => {
                let what = format!("`{}`", op.symbol());
                self.want_num(lhs, &what)?;
                self.want_num(rhs, &what)?;
                Ok(Type::Num)
            }
            Lt | Le | Gt | Ge => {
                let a = self.check(lhs)?;
                let b = self.check(rhs)?;
                let ok = !concrete(a) || !concrete(b) || (a == b && a != Type::Bool);
                if !ok {
                    return err(
                        span,
                        format!(
                            "`{}` compares a {a} with a {b}; ordering needs two numbers or two strings",
                            source_of_binary(op, lhs, rhs)
                        ),
                    );
                }
                Ok(Type::Bool)
            }
            Eq | Ne | StrictEq | StrictNe => {
                let a = self.check(lhs)?;
                let b = self.check(rhs)?;
                if concrete(a) && concrete(b) && a != b {
                    let always = if matches!(op, Eq | StrictEq) { "false" } else { "true" };
                    return err(
                        span,
                        format!(
                            "`{}` compares a {a} with a {b}, which is always {always} (no implicit conversions)",
                            source_of_binary(op, lhs, rhs)
                        ),
                    );
                }
                Ok(Type::Bool)
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Expr], span: Span) -> TResult {
        if let Some((lo, hi)) = builtins::arity(name) {
            if args.len() < lo || args.len() > hi {
                return err(span, arity_message(&format!("{name}()"), lo, hi, args.len()));
            }
            return match builtins::resolve(name, args.len()) {
                Some(Builtin::Conv(c)) => {
                    self.check(&args[0])?;
                    Ok(match c {
                        Conv::String => Type::Str,
                        Conv::Number => Type::Num,
                        Conv::Boolean | Conv::IsNaN | Conv::IsFinite => Type::Bool,
                    })
                }
                _ => {
                    for (i, a) in args.iter().enumerate() {
                        self.want_num(a, &format!("argument {} of {name}()", i + 1))?;
                    }
                    Ok(Type::Num)
                }
            };
        }
        let mut types = Vec::with_capacity(args.len());
        for a in args {
            types.push(self.check(a)?);
        }
        match self.env.call_type(name, &types) {
            Some(t) => Ok(t),
            None => err(span, format!("unknown function `{name}`")),
        }
    }

    fn method(&mut self, recv: &Expr, name: &str, args: &[Expr], span: Span) -> TResult {
        let Some(m) = Method::from_name(name) else {
            return err(span, format!("unknown method `.{name}`; values support {}", Method::list()));
        };
        let (lo, hi) = m.arity();
        if args.len() < lo || args.len() > hi {
            return err(span, arity_message(&format!(".{name}()"), lo, hi, args.len()));
        }
        if let Expr::Array { items, .. } = recv {
            self.items(items)?;
            return match m {
                Method::Length => Ok(Type::Num),
                Method::Includes => {
                    self.check(&args[0])?;
                    Ok(Type::Bool)
                }
                Method::IndexOf => {
                    self.check(&args[0])?;
                    Ok(Type::Num)
                }
                _ => err(span, format!("arrays support only .includes(), .indexOf() and .length, not .{name}()")),
            };
        }
        let t = self.check(recv)?;
        if m != Method::ToString && matches!(t, Type::Num | Type::Bool) {
            return err(
                recv.span(),
                format!("`.{name}` needs a string, but `{}` is a {t} (convert with String(…))", source(recv)),
            );
        }
        match m {
            Method::Slice => {
                for a in args {
                    self.want_num(a, ".slice()")?;
                }
            }
            Method::StartsWith | Method::EndsWith | Method::Includes | Method::IndexOf => {
                self.want_str(&args[0], &format!(".{name}()"))?;
            }
            _ => {}
        }
        Ok(match m {
            Method::Length | Method::IndexOf => Type::Num,
            Method::StartsWith | Method::EndsWith | Method::Includes => Type::Bool,
            Method::ToUpperCase | Method::ToLowerCase | Method::Trim | Method::Slice | Method::ToString => Type::Str,
        })
    }
}

fn source_of_binary(op: BinOp, lhs: &Expr, rhs: &Expr) -> String {
    source(&Expr::binary(op, lhs.clone(), rhs.clone()))
}

/// ", did you mean `x`?" for the closest known name within a small edit distance.
fn suggest(name: &str, known: &[String]) -> String {
    let best = known
        .iter()
        .map(|k| (edit_distance(name, k), k))
        .filter(|(d, k)| *d <= (k.chars().count().max(name.chars().count()) / 3).max(1))
        .min();
    match best {
        Some((_, k)) => format!("; did you mean `{k}`?"),
        None => String::new(),
    }
}

/// Optimal-string-alignment distance: Levenshtein plus adjacent transpositions (the typo `valeu`
/// is one edit from `value`).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let w = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * w];
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            d[i * w + j] = if i == 0 || j == 0 {
                i + j
            } else {
                let cost = usize::from(a[i - 1] != b[j - 1]);
                let mut v = (d[(i - 1) * w + j] + 1).min(d[i * w + j - 1] + 1).min(d[(i - 1) * w + j - 1] + cost);
                if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                    v = v.min(d[(i - 2) * w + j - 2] + 1);
                }
                v
            };
        }
    }
    d[a.len() * w + b.len()]
}
