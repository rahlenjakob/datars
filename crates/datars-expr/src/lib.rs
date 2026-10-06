//! `datars-expr` — the per-element, per-frame expression language (docs/06-data-and-reactivity.md).
//!
//! Expressions are **pure, total, typed and small**. They run per row (vectorized over columns) and
//! per frame, never in user JS: TypeScript lambdas compile to them (docs/07-extensibility.md), so the
//! syntax is a JavaScript subset.
//!
//! ```text
//! d => d.share > 30 ? palette.accent : "#ccc"
//! (d, i) => `${d.name}: ${round(d.value, 1)}`
//! selected.isEmpty() || selected.has(d.party) ? 1 : 0.3
//! ```
//!
//! Pipeline: [`parse`] source → [`Expr`] (a serde JSON AST whose `Display` is canonical source) →
//! [`typecheck`] against a [`TypeEnv`] (load time) → [`compile`] to register bytecode →
//! [`Compiled::eval_rows`] over `n` rows of an [`Env`], one dispatch per instruction per chunk of rows.
//! [`to_wgsl`] emits the numeric subset for per-instance GPU channels.
//!
//! **Semantics in one paragraph.** `d.x` reads column `x` of the current row; without a lambda
//! wrapper a bare `x` is a column if the env has one, else a signal; inside a lambda, free names are
//! signals. `a.b(…)` calls host function `"a.b"` (scales, `format`, signal methods). Built-in math is
//! deterministic (`datars_math::m`). Values are null, numbers, strings and booleans; NaN is the
//! numeric null (`== null` and `??` treat it as missing). Everything is total: division by zero is
//! IEEE (±Infinity / NaN), a missing column, signal or host result is null, nothing panics.

pub mod ast;
mod builtins;
mod compile;
pub mod env;
mod error;
mod lexer;
mod parser;
mod print;
pub mod types;
pub mod value;
mod vm;
pub mod wgsl;

pub use ast::{BinOp, Expr, Span, UnOp, MAX_DEPTH};
pub use compile::{compile, Compiled};
pub use env::{ColumnView, EmptyEnv, Env, HostFn, MapEnv, OwnedColumn};
pub use error::{CompileError, ParseError, TypeError};
pub use parser::parse;
pub use types::{typecheck, Type, TypeEnv};
pub use value::Value;
pub use wgsl::{to_wgsl, to_wgsl_with};

/// Names of the built-in functions (callable bare, and the math ones also as `Math.<name>`).
/// Any other called name is a host call.
pub fn builtin_names() -> &'static [&'static str] {
    builtins::NAMES
}
