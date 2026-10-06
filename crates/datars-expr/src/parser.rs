//! Pratt parser for the JavaScript-like subset.
//!
//! Grammar summary (JS precedence, lowest first):
//!
//! ```text
//! top      := lambda | expr
//! lambda   := IDENT "=>" body | "(" [IDENT ["," IDENT]] ")" "=>" body
//! body     := expr | "{" "return" expr [";"] "}"
//! expr     := expr "?" expr ":" expr                      (right assoc)
//!           | expr ("||" | "??") expr | expr "&&" expr     (?? can't mix with || / && unparenthesized)
//!           | expr ("==" | "!=" | "===" | "!==") expr | expr ("<" | "<=" | ">" | ">=") expr
//!           | expr ("+" | "-") expr | expr ("*" | "/" | "%") expr
//!           | expr "**" expr                               (right assoc; no bare unary on the left)
//!           | ("-" | "+" | "!") expr | postfix
//! postfix  := primary { "." method "(" args ")" | ".length" | "[" expr "]" }
//! primary  := NUMBER | STRING | TEMPLATE | "[" args "]" | "(" expr ")" | "true" | "false" | "null"
//!           | "undefined" | "NaN" | "Infinity" | "PI" | "E" | "Math." NAME ["(" args ")"]
//!           | datum "." NAME | datum "[" STRING "]" | index
//!           | NAME { "." NAME } ["(" args ")"]            (a call is built-in or host "a.b.c")
//! ```

use crate::ast::{BinOp, Expr, Span, UnOp, MAX_DEPTH as MAX_TREE_DEPTH};
use crate::builtins::{self, Method};
use crate::error::ParseError;
use crate::lexer::{Lexer, Tok};

/// Nesting limit, so hostile input can't overflow the stack (the parser must never panic).
const MAX_DEPTH: u32 = 200;

/// Binding power of the ternary operator (and the minimum for its `else` branch).
const TERNARY_BP: u8 = 4;
/// Operands of prefix operators bind tighter than any infix operator.
const UNARY_BP: u8 = 20;

/// How an operand was written, for the JS rules that depend on syntax rather than the tree:
/// `-x ** 2` is an error, and so is `a ?? b || c`; parentheses make both fine.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Paren,
    Unary,
    Other,
}

/// Words that are JavaScript but not part of the expression subset.
const RESERVED: &[&str] = &[
    "this", "new", "function", "typeof", "void", "delete", "in", "instanceof", "let", "const", "var", "return", "if",
    "else", "for", "while", "do", "class", "await", "yield", "async", "throw", "try", "catch", "switch", "case",
    "super", "import", "export", "with", "debugger",
];

/// Words that can't be lambda parameter names.
const NOT_PARAMS: &[&str] = &["true", "false", "null", "undefined", "NaN", "Infinity", "Math", "PI", "E"];

/// Parses an expression, optionally wrapped in a lambda (`d => …`, `(d) => …`, `(d, i) => …`).
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    let mut lex = Lexer::new(src);
    let (tok, span) = lex.next_token()?;
    let mut p = Parser { lex, tok, span, datum: Some("d".into()), index: None, lambda: false, depth: 0 };
    let e = p.top()?;
    // Left-associative chains (`a + b + …`, `s.trim().trim()…`) are parsed by loops, so the tree
    // can be deeper than the parser's recursion; bound it for the recursive passes downstream.
    if e.depth() > MAX_TREE_DEPTH {
        return Err(ParseError::new(
            Span::new(0, src.len()),
            format!("expression is nested too deeply (more than {MAX_TREE_DEPTH} levels)"),
        ));
    }
    Ok(e)
}

struct Parser<'s> {
    lex: Lexer<'s>,
    tok: Tok,
    span: Span,
    /// The datum parameter (`d` when there is no lambda wrapper).
    datum: Option<String>,
    /// The row-index parameter (the lambda's second parameter).
    index: Option<String>,
    /// Inside a lambda, free names are signals; outside, bare names are column-or-signal.
    lambda: bool,
    depth: u32,
}

type PResult<T> = Result<T, ParseError>;

impl Parser<'_> {
    fn advance(&mut self) -> PResult<()> {
        let (tok, span) = self.lex.next_token()?;
        self.tok = tok;
        self.span = span;
        Ok(())
    }

    fn err<T>(&self, span: Span, msg: impl Into<String>) -> PResult<T> {
        Err(ParseError::new(span, msg))
    }

    fn unexpected<T>(&self, expected: &str) -> PResult<T> {
        if self.tok == Tok::Eof {
            self.err(self.span, format!("unexpected end of input, expected {expected}"))
        } else {
            self.err(self.span, format!("unexpected {}, expected {expected}", self.tok.describe()))
        }
    }

    fn expect(&mut self, tok: Tok, expected: &str) -> PResult<Span> {
        if self.tok == tok {
            let span = self.span;
            self.advance()?;
            Ok(span)
        } else {
            self.unexpected(expected)
        }
    }

    fn expect_name(&mut self, expected: &str) -> PResult<(String, Span)> {
        match &self.tok {
            Tok::Ident(name) => {
                let r = (name.clone(), self.span);
                self.advance()?;
                Ok(r)
            }
            _ => self.unexpected(expected),
        }
    }

    fn top(&mut self) -> PResult<Expr> {
        if let Some(params) = self.lambda_params() {
            let bad = params.iter().find(|p| NOT_PARAMS.contains(&p.as_str()) || RESERVED.contains(&p.as_str()));
            if let Some(bad) = bad {
                return self.err(self.span, format!("`{bad}` can't be a parameter name"));
            }
            if params.len() == 2 && params[0] == params[1] {
                return self.err(self.span, "the two lambda parameters need different names");
            }
            self.lambda = true;
            self.datum = params.first().cloned();
            self.index = params.get(1).cloned();
            self.advance()?; // the token after `=>`
            if self.tok == Tok::LBrace {
                self.advance()?;
                match &self.tok {
                    Tok::Ident(w) if w == "return" => self.advance()?,
                    _ => return self.unexpected("`return` (block bodies may only contain `return <expr>`)"),
                }
                let (body, _) = self.expr(0)?;
                if self.tok == Tok::Semi {
                    self.advance()?;
                }
                self.expect(Tok::RBrace, "`}`")?;
                return self.finish(body);
            }
        }
        let (e, _) = self.expr(0)?;
        self.finish(e)
    }

    fn finish(&self, e: Expr) -> PResult<Expr> {
        if self.tok != Tok::Eof {
            return self.unexpected("an operator or the end of the expression");
        }
        Ok(e)
    }

    /// Recognizes a lambda head on a scratch copy of the lexer. On success the real lexer is moved
    /// to just after `=>` (the caller advances onto the body's first token).
    fn lambda_params(&mut self) -> Option<Vec<String>> {
        let mut lx = self.lex.clone();
        let mut next = || lx.next_token().ok().map(|(t, _)| t);
        let params = match &self.tok {
            Tok::Ident(a) => {
                if next()? != Tok::Arrow {
                    return None;
                }
                vec![a.clone()]
            }
            Tok::LParen => {
                let mut params = Vec::new();
                let mut t = next()?;
                loop {
                    match t {
                        Tok::RParen => break,
                        Tok::Ident(name) if params.len() < 2 => {
                            params.push(name);
                            t = next()?;
                            match t {
                                Tok::Comma => t = next()?,
                                Tok::RParen => break,
                                _ => return None,
                            }
                        }
                        _ => return None,
                    }
                }
                if next()? != Tok::Arrow {
                    return None;
                }
                params
            }
            _ => return None,
        };
        self.lex = lx;
        Some(params)
    }

    fn expr(&mut self, min_bp: u8) -> PResult<(Expr, Origin)> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.err(self.span, "expression is nested too deeply");
        }
        let r = self.expr_inner(min_bp);
        self.depth -= 1;
        r
    }

    fn expr_inner(&mut self, min_bp: u8) -> PResult<(Expr, Origin)> {
        let (mut lhs, mut origin) = self.prefix()?;
        loop {
            if self.tok == Tok::Question {
                if TERNARY_BP < min_bp {
                    break;
                }
                self.advance()?;
                let (then, _) = self.expr(0)?;
                self.expect(Tok::Colon, "`:` of the conditional")?;
                let (otherwise, _) = self.expr(TERNARY_BP)?;
                let span = lhs.span().to(otherwise.span());
                lhs = Expr::Cond { test: Box::new(lhs), then: Box::new(then), otherwise: Box::new(otherwise), span };
                origin = Origin::Other;
                continue;
            }
            let Some(op) = binop(&self.tok) else { break };
            let level = op.prec() * 2;
            let (lbp, rbp) = if op == BinOp::Pow { (level, level - 1) } else { (level, level + 1) };
            if lbp < min_bp {
                break;
            }
            let op_span = self.span;
            if op == BinOp::Pow && origin == Origin::Unary {
                return self.err(
                    lhs.span().to(op_span),
                    "a unary operator before `**` is ambiguous; add parentheses, e.g. `(-x) ** 2`",
                );
            }
            self.check_mix(op, &lhs, origin, op_span)?;
            self.advance()?;
            let (rhs, rorigin) = self.expr(rbp)?;
            self.check_mix(op, &rhs, rorigin, op_span)?;
            let span = lhs.span().to(rhs.span());
            lhs = Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), span };
            origin = Origin::Other;
        }
        Ok((lhs, origin))
    }

    /// JS forbids mixing `??` with `||`/`&&` without parentheses (the intent is ambiguous).
    fn check_mix(&self, op: BinOp, operand: &Expr, origin: Origin, op_span: Span) -> PResult<()> {
        if origin == Origin::Paren {
            return Ok(());
        }
        if let Expr::Binary { op: inner, .. } = operand {
            let mixes = (op == BinOp::Coalesce && matches!(inner, BinOp::And | BinOp::Or))
                || (matches!(op, BinOp::And | BinOp::Or) && *inner == BinOp::Coalesce);
            if mixes {
                return self.err(
                    operand.span().to(op_span),
                    format!("mixing `??` with `{}` needs parentheses", if op == BinOp::Coalesce { inner.symbol() } else { op.symbol() }),
                );
            }
        }
        Ok(())
    }

    fn prefix(&mut self) -> PResult<(Expr, Origin)> {
        let op = match self.tok {
            Tok::Minus => UnOp::Neg,
            Tok::Plus => UnOp::Pos,
            Tok::Bang => UnOp::Not,
            _ => return self.primary(),
        };
        let start = self.span;
        self.advance()?;
        // `-3` and `-Infinity` are negative literals (so printed negative numbers re-parse to the
        // same tree); `-(3)` stays a negation.
        let literal = matches!(&self.tok, Tok::Num(_)) || matches!(&self.tok, Tok::Ident(n) if n == "Infinity" || n == "NaN");
        let (arg, _) = self.expr(UNARY_BP)?;
        let span = start.to(arg.span());
        if op == UnOp::Neg && literal {
            if let Expr::Num { value, .. } = arg {
                return Ok((Expr::Num { value: -value, span }, Origin::Unary));
            }
        }
        Ok((Expr::Unary { op, arg: Box::new(arg), span }, Origin::Unary))
    }

    fn primary(&mut self) -> PResult<(Expr, Origin)> {
        let span = self.span;
        let e = match self.tok.clone() {
            Tok::Num(value) => {
                self.advance()?;
                Expr::Num { value, span }
            }
            Tok::Str(value) => {
                self.advance()?;
                Expr::Str { value, span }
            }
            Tok::Backtick => self.template()?,
            Tok::LBracket => {
                self.advance()?;
                let (items, close) = self.list(Tok::RBracket, "array element")?;
                Expr::Array { items, span: span.to(close) }
            }
            Tok::LParen => {
                self.advance()?;
                let (e, _) = self.expr(0)?;
                self.expect(Tok::RParen, "`)`")?;
                if matches!(self.tok, Tok::Dot | Tok::LBracket | Tok::LParen) {
                    return Ok((self.postfix(e)?, Origin::Other));
                }
                return Ok((e, Origin::Paren));
            }
            Tok::Ident(name) => return Ok((self.name(name, span)?, Origin::Other)),
            _ => return self.unexpected("an expression"),
        };
        Ok((self.postfix(e)?, Origin::Other))
    }

    /// Comma-separated expressions up to `close` (consumed, its span returned); a trailing comma is
    /// fine.
    fn list(&mut self, close: Tok, what: &str) -> PResult<(Vec<Expr>, Span)> {
        let mut items = Vec::new();
        loop {
            if self.tok == close {
                let span = self.span;
                self.advance()?;
                return Ok((items, span));
            }
            if self.tok == Tok::Comma {
                return self.err(self.span, format!("missing {what}"));
            }
            let (e, _) = self.expr(0)?;
            items.push(e);
            match &self.tok {
                Tok::Comma => self.advance()?,
                t if *t == close => {}
                _ => return self.unexpected(&format!("`,` or {}", close.describe())),
            }
        }
    }

    fn args(&mut self) -> PResult<(Vec<Expr>, Span)> {
        self.expect(Tok::LParen, "`(`")?;
        self.list(Tok::RParen, "argument")
    }

    fn template(&mut self) -> PResult<Expr> {
        let start = self.span.start;
        // The current token is the opening backtick, so the lexer sits right after it.
        let mut parts = Vec::new();
        loop {
            let chunk_start = self.lex.pos;
            let (text, subst) = self.lex.template_chunk()?;
            if !text.is_empty() {
                let end = self.lex.pos - if subst { 2 } else { 1 };
                parts.push(Expr::Str { value: text, span: Span::new(chunk_start, end) });
            }
            if !subst {
                break;
            }
            self.advance()?;
            let (e, _) = self.expr(0)?;
            if self.tok != Tok::RBrace {
                return self.unexpected("`}` closing `${`");
            }
            parts.push(e);
            // Resume raw text right after the `}` (the lexer hasn't looked past it).
            self.lex.pos = self.span.end;
        }
        let span = Span::new(start, self.lex.pos);
        self.advance()?;
        Ok(Expr::Concat { parts, span })
    }

    /// Member access, method calls and indexing on a value.
    fn postfix(&mut self, mut e: Expr) -> PResult<Expr> {
        loop {
            match self.tok {
                Tok::Dot => {
                    self.advance()?;
                    let (name, nspan) = self.expect_name("a method name after `.`")?;
                    let calls = self.tok == Tok::LParen;
                    let method = Method::from_name(&name);
                    e = match method {
                        Some(Method::Length) if !calls => {
                            let span = e.span().to(nspan);
                            Expr::Method { recv: Box::new(e), name, args: Vec::new(), span }
                        }
                        Some(Method::Length) => return self.err(nspan, "`length` is a property: write `.length`"),
                        Some(m) if calls => {
                            let (args, close) = self.args()?;
                            let span = e.span().to(close);
                            let (lo, hi) = m.arity();
                            if args.len() < lo || args.len() > hi {
                                return self.err(span, arity_message(&format!(".{name}()"), lo, hi, args.len()));
                            }
                            Expr::Method { recv: Box::new(e), name, args, span }
                        }
                        Some(_) => return self.err(nspan, format!("`.{name}` is a method: call it, `.{name}(…)`")),
                        None => {
                            return self.err(
                                nspan,
                                format!("unknown property or method `.{name}`; values support {}", Method::list()),
                            )
                        }
                    };
                }
                Tok::LBracket => {
                    self.advance()?;
                    let (index, _) = self.expr(0)?;
                    let close = self.expect(Tok::RBracket, "`]`")?;
                    let span = e.span().to(close);
                    e = Expr::Index { obj: Box::new(e), index: Box::new(index), span };
                }
                Tok::LParen => {
                    return self.err(self.span, "only named functions can be called (`f(x)`, `scale.y(x)`)");
                }
                _ => return Ok(e),
            }
        }
    }

    fn name(&mut self, name: String, span: Span) -> PResult<Expr> {
        if self.datum.as_deref() == Some(name.as_str()) {
            return self.datum_member(name, span);
        }
        if self.index.as_deref() == Some(name.as_str()) {
            self.advance()?;
            return self.postfix(Expr::Row { span });
        }
        let literal = match name.as_str() {
            "true" => Some(Expr::Bool { value: true, span }),
            "false" => Some(Expr::Bool { value: false, span }),
            "null" | "undefined" => Some(Expr::Null { span }),
            "NaN" => Some(Expr::Num { value: f64::NAN, span }),
            "Infinity" => Some(Expr::Num { value: f64::INFINITY, span }),
            "PI" => Some(Expr::Num { value: std::f64::consts::PI, span }),
            "E" => Some(Expr::Num { value: std::f64::consts::E, span }),
            _ => None,
        };
        if let Some(e) = literal {
            self.advance()?;
            return self.postfix(e);
        }
        if name == "Math" {
            return self.math(span);
        }
        if RESERVED.contains(&name.as_str()) {
            return self.err(span, format!("`{name}` isn't supported in expressions"));
        }
        self.advance()?;
        let mut path = name;
        let mut pspan = span;
        while self.tok == Tok::Dot {
            self.advance()?;
            let (seg, sspan) = self.expect_name("a name after `.`")?;
            path.push('.');
            path.push_str(&seg);
            pspan = pspan.to(sspan);
        }
        let e = if self.tok == Tok::LParen {
            let (args, close) = self.args()?;
            let span = pspan.to(close);
            let name = match path.as_str() {
                "Number.isNaN" => "isNaN".to_string(),
                "Number.isFinite" => "isFinite".to_string(),
                _ => path,
            };
            if let Some((lo, hi)) = builtins::arity(&name) {
                if args.len() < lo || args.len() > hi {
                    return self.err(span, arity_message(&format!("{name}()"), lo, hi, args.len()));
                }
            }
            Expr::Call { name, args, span }
        } else if self.lambda {
            Expr::Signal { name: path, span: pspan }
        } else {
            Expr::Ident { name: path, span: pspan }
        };
        self.postfix(e)
    }

    /// `d.x` / `d["x"]` — a column of the current row.
    fn datum_member(&mut self, datum: String, span: Span) -> PResult<Expr> {
        self.advance()?;
        let field = match self.tok {
            Tok::Dot => {
                self.advance()?;
                let (name, nspan) = self.expect_name(&format!("a column name after `{datum}.`"))?;
                if self.tok == Tok::LParen {
                    return self.err(span.to(nspan), format!("`{datum}.{name}` is a column, not a function"));
                }
                Expr::Field { name, span: span.to(nspan) }
            }
            Tok::LBracket => {
                self.advance()?;
                let Tok::Str(name) = self.tok.clone() else {
                    return self.err(self.span, format!("`{datum}[…]` needs a string literal column name"));
                };
                self.advance()?;
                let close = self.expect(Tok::RBracket, "`]`")?;
                Expr::Field { name, span: span.to(close) }
            }
            _ => {
                return self.err(span, format!("`{datum}` is the datum; read a column with `{datum}.name`"));
            }
        };
        self.postfix(field)
    }

    /// `Math.PI`, `Math.abs(x)`, … map to literals and built-ins.
    fn math(&mut self, span: Span) -> PResult<Expr> {
        self.advance()?;
        self.expect(Tok::Dot, "`.` after `Math`")?;
        let (member, mspan) = self.expect_name("a Math member")?;
        let span = span.to(mspan);
        let e = if self.tok == Tok::LParen {
            if member == "random" {
                return self.err(span, "Math.random() isn't allowed: expressions are deterministic");
            }
            if !builtins::MATH.contains(&member.as_str()) {
                return self.err(span, format!("Math.{member}() isn't supported"));
            }
            let (args, close) = self.args()?;
            let span = span.to(close);
            if let Some((lo, hi)) = builtins::arity(&member) {
                if args.len() < lo || args.len() > hi {
                    return self.err(span, arity_message(&format!("Math.{member}()"), lo, hi, args.len()));
                }
            }
            Expr::Call { name: member, args, span }
        } else {
            use std::f64::consts as c;
            let value = match member.as_str() {
                "PI" => c::PI,
                "E" => c::E,
                "LN2" => c::LN_2,
                "LN10" => c::LN_10,
                "LOG2E" => c::LOG2_E,
                "LOG10E" => c::LOG10_E,
                "SQRT2" => c::SQRT_2,
                "SQRT1_2" => c::FRAC_1_SQRT_2,
                _ => return self.err(span, format!("Math.{member} isn't supported")),
            };
            Expr::Num { value, span }
        };
        self.postfix(e)
    }
}

fn binop(t: &Tok) -> Option<BinOp> {
    use BinOp::*;
    Some(match t {
        Tok::Plus => Add,
        Tok::Minus => Sub,
        Tok::Star => Mul,
        Tok::Slash => Div,
        Tok::Percent => Rem,
        Tok::StarStar => Pow,
        Tok::Lt => Lt,
        Tok::Le => Le,
        Tok::Gt => Gt,
        Tok::Ge => Ge,
        Tok::EqEq => Eq,
        Tok::NotEq => Ne,
        Tok::EqEqEq => StrictEq,
        Tok::NotEqEq => StrictNe,
        Tok::AndAnd => And,
        Tok::OrOr => Or,
        Tok::QQ => Coalesce,
        _ => return None,
    })
}

pub(crate) fn arity_message(what: &str, lo: usize, hi: usize, got: usize) -> String {
    let want = if lo == hi {
        format!("{lo} argument{}", if lo == 1 { "" } else { "s" })
    } else if hi == usize::MAX {
        format!("at least {lo} argument{}", if lo == 1 { "" } else { "s" })
    } else {
        format!("{lo} to {hi} arguments")
    };
    format!("{what} takes {want}, got {got}")
}
