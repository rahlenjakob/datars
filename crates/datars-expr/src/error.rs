//! Error types. All three carry a [`Span`] (into the source the expression was parsed from, or
//! `Span::UNKNOWN` for hand-built trees) and a message written for the person who typed the code.

use crate::ast::Span;
use std::fmt;

/// A syntax error, or a construct outside the supported subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

/// A type error found by [`crate::typecheck`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeError {
    pub span: Span,
    pub message: String,
}

/// An AST the compiler can't lower (only reachable for trees that didn't come from [`crate::parse`],
/// e.g. hand-built or deserialized JSON: wrong arity, unknown method, array in value position).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompileError {
    pub span: Span,
    pub message: String,
}

macro_rules! impl_error {
    ($t:ident, $what:literal) => {
        impl $t {
            pub(crate) fn new(span: Span, message: impl Into<String>) -> $t {
                $t { span, message: message.into() }
            }

            /// The message with the offending source line and a caret underline, for terminals and logs.
            pub fn render(&self, src: &str) -> String {
                render(src, self.span, $what, &self.message)
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.span.is_unknown() {
                    write!(f, "{}: {}", $what, self.message)
                } else {
                    write!(f, "{}: {} (at {}..{})", $what, self.message, self.span.start, self.span.end)
                }
            }
        }

        impl std::error::Error for $t {}
    };
}

impl_error!(ParseError, "syntax error");
impl_error!(TypeError, "type error");
impl_error!(CompileError, "compile error");

fn render(src: &str, span: Span, what: &str, message: &str) -> String {
    let boundary = |mut i: usize| {
        i = i.min(src.len());
        while !src.is_char_boundary(i) {
            i -= 1;
        }
        i
    };
    let start = boundary(span.start);
    let end = boundary(span.end.max(start));
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[start..].find('\n').map_or(src.len(), |i| start + i);
    let line = &src[line_start..line_end];
    let col = src[line_start..start].chars().count();
    let width = src[start..end.min(line_end)].chars().count().max(1);
    format!("{what}: {message}\n  {line}\n  {}{}", " ".repeat(col), "^".repeat(width))
}
