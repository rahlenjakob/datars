//! The expression AST: what the parser produces, what the TypeScript toolchain emits as JSON, and
//! what the compiler lowers to bytecode.
//!
//! The JSON form is internally tagged by `"kind"` (`{"kind":"binary","op":"+","lhs":…,"rhs":…}`).
//! `Display` prints canonical source that re-parses to an equal tree (see `print.rs`).

use serde::{Deserialize, Serialize};

/// The deepest expression tree the parser produces and the compiler, typechecker and WGSL
/// emitter accept. Real expressions are a few levels deep; the limit keeps the recursive passes
/// well inside any stack (expressions must never crash the engine). Measured worst case at this
/// depth: ~1.2 MiB of stack in an unoptimized build, ~125 KiB optimized.
pub const MAX_DEPTH: usize = 128;

/// A byte range in the source an expression was parsed from.
///
/// Spans are debug information: they never take part in [`Expr`] equality and are not serialized
/// (so the JSON AST of an expression doesn't depend on how it was written), but they are accepted
/// on input so a toolchain can point errors at its own source. `Span::UNKNOWN` (0..0) marks trees
/// built by hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const UNKNOWN: Span = Span { start: 0, end: 0 };

    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn is_unknown(&self) -> bool {
        self.start == 0 && self.end == 0
    }

    /// The smallest span covering both (unknown spans are ignored).
    pub fn to(self, other: Span) -> Span {
        if self.is_unknown() {
            other
        } else if other.is_unknown() {
            self
        } else {
            Span { start: self.start.min(other.start), end: self.end.max(other.end) }
        }
    }
}

/// Prefix operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UnOp {
    #[serde(rename = "-")]
    Neg,
    #[serde(rename = "+")]
    Pos,
    #[serde(rename = "!")]
    Not,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Pos => "+",
            UnOp::Not => "!",
        }
    }
}

/// Infix operators. `==`/`===` (and `!=`/`!==`) have the same (strict-ish) semantics; both are kept
/// so printed source matches what was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BinOp {
    #[serde(rename = "+")]
    Add,
    #[serde(rename = "-")]
    Sub,
    #[serde(rename = "*")]
    Mul,
    #[serde(rename = "/")]
    Div,
    #[serde(rename = "%")]
    Rem,
    #[serde(rename = "**")]
    Pow,
    #[serde(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    Le,
    #[serde(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    Ge,
    #[serde(rename = "==")]
    Eq,
    #[serde(rename = "!=")]
    Ne,
    #[serde(rename = "===")]
    StrictEq,
    #[serde(rename = "!==")]
    StrictNe,
    #[serde(rename = "&&")]
    And,
    #[serde(rename = "||")]
    Or,
    #[serde(rename = "??")]
    Coalesce,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        use BinOp::*;
        match self {
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            Rem => "%",
            Pow => "**",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            Eq => "==",
            Ne => "!=",
            StrictEq => "===",
            StrictNe => "!==",
            And => "&&",
            Or => "||",
            Coalesce => "??",
        }
    }

    /// JavaScript precedence level (higher binds tighter). Shared by the parser and the printer.
    /// Ternary is 2, prefix operators 10, postfix (member, call, index) 11.
    pub(crate) fn prec(self) -> u8 {
        use BinOp::*;
        match self {
            Or | Coalesce => 3,
            And => 4,
            Eq | Ne | StrictEq | StrictNe => 5,
            Lt | Le | Gt | Ge => 6,
            Add | Sub => 7,
            Mul | Div | Rem => 8,
            Pow => 9,
        }
    }

    pub(crate) fn is_logical(self) -> bool {
        matches!(self, BinOp::And | BinOp::Or | BinOp::Coalesce)
    }
}

/// An expression.
///
/// Name resolution is explicit in the tree: [`Expr::Field`] is a column of the current row (`d.x`),
/// [`Expr::Signal`] a signal (a free name inside a lambda), [`Expr::Ident`] a bare name outside a
/// lambda (a column if the env has one, else a signal), [`Expr::Row`] the row index (the second
/// lambda parameter). [`Expr::Call`] is a built-in when its name is one (`abs`, `max`, `String`, …),
/// else a host call (`"scale.y"`, `"format"`). [`Expr::Method`] is a string method on a value
/// (`d.name.toUpperCase()`, `.length`), or `includes`/`indexOf`/`length` on an array literal.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Expr {
    Null {
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Bool {
        value: bool,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    /// Non-finite values serialize as the strings `"NaN"`, `"Infinity"`, `"-Infinity"`.
    Num {
        #[serde(with = "num_serde")]
        value: f64,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Str {
        value: String,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    /// Only valid as the receiver of `includes`/`indexOf`/`length` or the object of `[index]`.
    Array {
        items: Vec<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Field {
        name: String,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Ident {
        name: String,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Signal {
        name: String,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Row {
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Unary {
        op: UnOp,
        arg: Box<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Cond {
        test: Box<Expr>,
        then: Box<Expr>,
        #[serde(rename = "else")]
        otherwise: Box<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Call {
        name: String,
        args: Vec<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Method {
        recv: Box<Expr>,
        name: String,
        args: Vec<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
    /// String concatenation of every part (converted with `String(…)`): a template literal.
    Concat {
        parts: Vec<Expr>,
        #[serde(default, skip_serializing)]
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        use Expr::*;
        match self {
            Null { span }
            | Bool { span, .. }
            | Num { span, .. }
            | Str { span, .. }
            | Array { span, .. }
            | Field { span, .. }
            | Ident { span, .. }
            | Signal { span, .. }
            | Row { span }
            | Unary { span, .. }
            | Binary { span, .. }
            | Cond { span, .. }
            | Call { span, .. }
            | Method { span, .. }
            | Index { span, .. }
            | Concat { span, .. } => *span,
        }
    }

    /// Direct children, in source order.
    pub fn children(&self) -> Vec<&Expr> {
        use Expr::*;
        match self {
            Null { .. } | Bool { .. } | Num { .. } | Str { .. } | Field { .. } | Ident { .. } | Signal { .. } | Row { .. } => {
                Vec::new()
            }
            Array { items: v, .. } | Call { args: v, .. } | Concat { parts: v, .. } => v.iter().collect(),
            Unary { arg, .. } => vec![arg],
            Binary { lhs, rhs, .. } => vec![lhs, rhs],
            Cond { test, then, otherwise, .. } => vec![test, then, otherwise],
            Method { recv, args, .. } => std::iter::once(&**recv).chain(args.iter()).collect(),
            Index { obj, index, .. } => vec![obj, index],
        }
    }

    /// Nesting depth (a leaf is 1). Computed without recursion, so it's safe on any tree; the
    /// recursive passes (typecheck, compile, WGSL) refuse trees deeper than [`MAX_DEPTH`].
    pub fn depth(&self) -> usize {
        let mut max = 0;
        let mut stack = vec![(self, 1usize)];
        while let Some((e, d)) = stack.pop() {
            max = max.max(d);
            stack.extend(e.children().into_iter().map(|c| (c, d + 1)));
        }
        max
    }

    /// Pre-order traversal.
    pub fn visit(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        for c in self.children() {
            c.visit(f);
        }
    }

    /// The JSON AST (spans omitted).
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(json: &str) -> Result<Expr, String> {
        serde_json::from_str(json).map_err(|e| e.to_string())
    }

    // Span-less constructors for building trees by hand (tests, toolchains).
    pub fn null() -> Expr {
        Expr::Null { span: Span::UNKNOWN }
    }
    pub fn bool(value: bool) -> Expr {
        Expr::Bool { value, span: Span::UNKNOWN }
    }
    pub fn num(value: f64) -> Expr {
        Expr::Num { value, span: Span::UNKNOWN }
    }
    pub fn str(value: impl Into<String>) -> Expr {
        Expr::Str { value: value.into(), span: Span::UNKNOWN }
    }
    pub fn field(name: impl Into<String>) -> Expr {
        Expr::Field { name: name.into(), span: Span::UNKNOWN }
    }
    pub fn ident(name: impl Into<String>) -> Expr {
        Expr::Ident { name: name.into(), span: Span::UNKNOWN }
    }
    pub fn signal(name: impl Into<String>) -> Expr {
        Expr::Signal { name: name.into(), span: Span::UNKNOWN }
    }
    pub fn row() -> Expr {
        Expr::Row { span: Span::UNKNOWN }
    }
    pub fn unary(op: UnOp, arg: Expr) -> Expr {
        Expr::Unary { op, arg: Box::new(arg), span: Span::UNKNOWN }
    }
    pub fn binary(op: BinOp, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span: Span::UNKNOWN }
    }
    pub fn cond(test: Expr, then: Expr, otherwise: Expr) -> Expr {
        Expr::Cond { test: Box::new(test), then: Box::new(then), otherwise: Box::new(otherwise), span: Span::UNKNOWN }
    }
    pub fn call(name: impl Into<String>, args: Vec<Expr>) -> Expr {
        Expr::Call { name: name.into(), args, span: Span::UNKNOWN }
    }
    pub fn method(recv: Expr, name: impl Into<String>, args: Vec<Expr>) -> Expr {
        Expr::Method { recv: Box::new(recv), name: name.into(), args, span: Span::UNKNOWN }
    }
}

/// Structural equality, ignoring spans. Number literals compare by bits (so `-0` ≠ `0`, as they
/// print differently), except that every NaN equals every NaN.
impl PartialEq for Expr {
    fn eq(&self, other: &Expr) -> bool {
        use Expr::*;
        match (self, other) {
            (Null { .. }, Null { .. }) | (Row { .. }, Row { .. }) => true,
            (Bool { value: a, .. }, Bool { value: b, .. }) => a == b,
            (Num { value: a, .. }, Num { value: b, .. }) => a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()),
            (Str { value: a, .. }, Str { value: b, .. }) => a == b,
            (Array { items: a, .. }, Array { items: b, .. }) => a == b,
            (Field { name: a, .. }, Field { name: b, .. })
            | (Ident { name: a, .. }, Ident { name: b, .. })
            | (Signal { name: a, .. }, Signal { name: b, .. }) => a == b,
            (Unary { op: o1, arg: a1, .. }, Unary { op: o2, arg: a2, .. }) => o1 == o2 && a1 == a2,
            (Binary { op: o1, lhs: l1, rhs: r1, .. }, Binary { op: o2, lhs: l2, rhs: r2, .. }) => {
                o1 == o2 && l1 == l2 && r1 == r2
            }
            (Cond { test: t1, then: a1, otherwise: b1, .. }, Cond { test: t2, then: a2, otherwise: b2, .. }) => {
                t1 == t2 && a1 == a2 && b1 == b2
            }
            (Call { name: n1, args: a1, .. }, Call { name: n2, args: a2, .. }) => n1 == n2 && a1 == a2,
            (Method { recv: r1, name: n1, args: a1, .. }, Method { recv: r2, name: n2, args: a2, .. }) => {
                n1 == n2 && r1 == r2 && a1 == a2
            }
            (Index { obj: o1, index: i1, .. }, Index { obj: o2, index: i2, .. }) => o1 == o2 && i1 == i2,
            (Concat { parts: a, .. }, Concat { parts: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// JSON has no NaN/Infinity: finite numbers are JSON numbers, the rest are strings.
mod num_serde {
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            s.serialize_f64(*v)
        } else if v.is_nan() {
            s.serialize_str("NaN")
        } else if *v > 0.0 {
            s.serialize_str("Infinity")
        } else {
            s.serialize_str("-Infinity")
        }
    }

    struct NumVisitor;

    impl Visitor<'_> for NumVisitor {
        type Value = f64;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a number or \"NaN\" / \"Infinity\" / \"-Infinity\"")
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<f64, E> {
            match v {
                "NaN" => Ok(f64::NAN),
                "Infinity" => Ok(f64::INFINITY),
                "-Infinity" => Ok(f64::NEG_INFINITY),
                _ => Err(E::custom(format!("not a number: {v:?}"))),
            }
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        d.deserialize_any(NumVisitor)
    }
}
