//! Canonical source printing (`Display for Expr`).
//!
//! The printed form re-parses to an equal tree for every tree the parser produces (and for hand-built
//! trees whose names are identifier paths): minimal parentheses by JS precedence, negative literals
//! written `-3` and negations of literals `-(3)`, `Math.*` normalized to the built-in names, template
//! literals for [`Expr::Concat`]. Trees with signals or a row index print as a lambda
//! (`d => …` / `(d, i) => …`, parameter names chosen not to collide with free names); trees with
//! bare identifiers print without a wrapper.

use crate::ast::{BinOp, Expr, UnOp};
use crate::lexer::{is_ident_continue, is_ident_start};
use std::fmt::{self, Write};

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut lambda = false;
        let mut row = false;
        let mut roots: Vec<String> = Vec::new();
        self.visit(&mut |e| match e {
            Expr::Signal { name, .. } => {
                lambda = true;
                roots.push(root(name).to_string());
            }
            Expr::Row { .. } => {
                lambda = true;
                row = true;
            }
            Expr::Ident { name, .. } | Expr::Call { name, .. } => roots.push(root(name).to_string()),
            _ => {}
        });
        if !lambda {
            let mut out = String::new();
            Printer { datum: "d", index: "i" }.expr(self, &mut out);
            return f.write_str(&out);
        }
        let datum = fresh("d", &roots, None);
        let index = fresh("i", &roots, Some(&datum));
        let mut out = String::new();
        if row {
            let _ = write!(out, "({datum}, {index}) => ");
        } else {
            let _ = write!(out, "{datum} => ");
        }
        Printer { datum: &datum, index: &index }.expr(self, &mut out);
        f.write_str(&out)
    }
}

/// The body of `e` in source form, never wrapped in a lambda (for error messages).
pub(crate) fn source(e: &Expr) -> String {
    let mut out = String::new();
    Printer { datum: "d", index: "i" }.expr(e, &mut out);
    out
}

fn root(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

/// `base`, or `base2`, `base3`, … — the first name no free name (or `taken`) uses.
fn fresh(base: &str, roots: &[String], taken: Option<&str>) -> String {
    let used = |n: &str| roots.iter().any(|r| r == n) || taken == Some(n);
    if !used(base) {
        return base.to_string();
    }
    (2..).map(|i| format!("{base}{i}")).find(|n| !used(n)).unwrap_or_default()
}

pub(crate) fn is_identifier(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(is_ident_start) && cs.all(is_ident_continue)
}

const TERNARY: u8 = 2;
const UNARY: u8 = 10;
const POSTFIX: u8 = 11;

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Cond { .. } => TERNARY,
        Expr::Binary { op, .. } => op.prec(),
        Expr::Unary { .. } => UNARY,
        Expr::Num { value, .. } if value.is_sign_negative() && !value.is_nan() => UNARY,
        _ => POSTFIX,
    }
}

struct Printer<'a> {
    datum: &'a str,
    index: &'a str,
}

impl Printer<'_> {
    fn wrap(&self, e: &Expr, parens: bool, out: &mut String) {
        if parens {
            out.push('(');
            self.expr(e, out);
            out.push(')');
        } else {
            self.expr(e, out);
        }
    }

    /// Prints `e` with parentheses unless it binds at least as tightly as `min`.
    fn at(&self, e: &Expr, min: u8, out: &mut String) {
        self.wrap(e, prec(e) < min, out);
    }

    fn list(&self, items: &[Expr], out: &mut String) {
        for (i, it) in items.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            self.expr(it, out);
        }
    }

    fn expr(&self, e: &Expr, out: &mut String) {
        match e {
            Expr::Null { .. } => out.push_str("null"),
            Expr::Bool { value, .. } => out.push_str(if *value { "true" } else { "false" }),
            Expr::Num { value, .. } => out.push_str(&num_source(*value)),
            Expr::Str { value, .. } => quote(value, out),
            Expr::Array { items, .. } => {
                out.push('[');
                self.list(items, out);
                out.push(']');
            }
            Expr::Field { name, .. } => {
                out.push_str(self.datum);
                if is_identifier(name) {
                    out.push('.');
                    out.push_str(name);
                } else {
                    out.push('[');
                    quote(name, out);
                    out.push(']');
                }
            }
            Expr::Ident { name, .. } | Expr::Signal { name, .. } => out.push_str(name),
            Expr::Row { .. } => out.push_str(self.index),
            Expr::Unary { op, arg, .. } => {
                out.push_str(op.symbol());
                // `-(3)` stays a negation (`-3` is a literal); `- -x` / `+ +x` would lex as `--`/`++`.
                let parens = prec(arg) < UNARY
                    || match op {
                        UnOp::Neg => {
                            matches!(**arg, Expr::Num { .. } | Expr::Unary { op: UnOp::Neg, .. })
                        }
                        UnOp::Pos => matches!(**arg, Expr::Unary { op: UnOp::Pos, .. }),
                        UnOp::Not => false,
                    };
                self.wrap(arg, parens, out);
            }
            Expr::Binary { op, lhs, rhs, .. } => {
                let p = op.prec();
                let mixes = |child: &Expr| match child {
                    Expr::Binary { op: inner, .. } => {
                        (*op == BinOp::Coalesce && matches!(inner, BinOp::And | BinOp::Or))
                            || (matches!(op, BinOp::And | BinOp::Or) && *inner == BinOp::Coalesce)
                    }
                    _ => false,
                };
                if *op == BinOp::Pow {
                    // Right-associative, and JS rejects a bare unary (or negative literal) on the left.
                    self.wrap(lhs, prec(lhs) <= UNARY, out);
                    let _ = write!(out, " ** ");
                    self.at(rhs, p, out);
                } else {
                    self.wrap(lhs, prec(lhs) < p || mixes(lhs), out);
                    let _ = write!(out, " {} ", op.symbol());
                    self.wrap(rhs, prec(rhs) <= p || mixes(rhs), out);
                }
            }
            Expr::Cond { test, then, otherwise, .. } => {
                self.at(test, TERNARY + 1, out);
                out.push_str(" ? ");
                self.expr(then, out);
                out.push_str(" : ");
                self.at(otherwise, TERNARY, out);
            }
            Expr::Call { name, args, .. } => {
                out.push_str(name);
                out.push('(');
                self.list(args, out);
                out.push(')');
            }
            Expr::Method { recv, name, args, .. } => {
                let parens = prec(recv) < POSTFIX
                    || matches!(**recv, Expr::Num { .. } | Expr::Ident { .. } | Expr::Signal { .. });
                self.wrap(recv, parens, out);
                out.push('.');
                out.push_str(name);
                if name != "length" || !args.is_empty() {
                    out.push('(');
                    self.list(args, out);
                    out.push(')');
                }
            }
            Expr::Index { obj, index, .. } => {
                self.wrap(obj, prec(obj) < POSTFIX || matches!(**obj, Expr::Num { .. }), out);
                out.push('[');
                self.expr(index, out);
                out.push(']');
            }
            Expr::Concat { parts, .. } => {
                out.push('`');
                for p in parts {
                    match p {
                        Expr::Str { value, .. } => escape_into(value, '`', out),
                        other => {
                            out.push_str("${");
                            self.expr(other, out);
                            out.push('}');
                        }
                    }
                }
                out.push('`');
            }
        }
    }
}

/// A number literal in source form: shortest round-trip digits, exponent form for very large or
/// small magnitudes, `NaN` / `Infinity` / `-Infinity`, and `-0`.
pub(crate) fn num_source(v: f64) -> String {
    if v.is_nan() {
        "NaN".into()
    } else if v.is_infinite() {
        if v > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else if v == 0.0 {
        if v.is_sign_negative() { "-0" } else { "0" }.into()
    } else if v.abs() >= 1e21 || v.abs() < 1e-6 {
        format!("{v:e}")
    } else {
        format!("{v}")
    }
}

fn quote(s: &str, out: &mut String) {
    out.push('"');
    escape_into(s, '"', out);
    out.push('"');
}

/// Escapes `s` for a literal delimited by `delim` (`"` or a backtick).
fn escape_into(s: &str, delim: char, out: &mut String) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if delim == '`' => out.push_str("\\$"),
            c if c == delim => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || c as u32 == 0x7f || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
}
