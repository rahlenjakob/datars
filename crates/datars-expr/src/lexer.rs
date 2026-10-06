//! Tokenizer. Lazy (one token at a time) so the parser can switch into raw template-literal text
//! after a backtick or a `}` that closes `${…}`.

use crate::ast::Span;
use crate::error::ParseError;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Num(f64),
    Str(String),
    /// Identifiers and keywords alike; the parser gives `true`, `null`, `Math`, … their meaning.
    Ident(String),
    Backtick,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    Dot,
    Question,
    Arrow,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    StarStar,
    Bang,
    EqEq,
    NotEq,
    EqEqEq,
    NotEqEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    QQ,
    Eof,
}

impl Tok {
    pub(crate) fn describe(&self) -> String {
        use Tok::*;
        let s = match self {
            Num(v) => return format!("number {v}"),
            Str(_) => "string",
            Ident(n) => return format!("`{n}`"),
            Backtick => "`",
            LParen => "(",
            RParen => ")",
            LBracket => "[",
            RBracket => "]",
            LBrace => "{",
            RBrace => "}",
            Comma => ",",
            Semi => ";",
            Colon => ":",
            Dot => ".",
            Question => "?",
            Arrow => "=>",
            Plus => "+",
            Minus => "-",
            Star => "*",
            Slash => "/",
            Percent => "%",
            StarStar => "**",
            Bang => "!",
            EqEq => "==",
            NotEq => "!=",
            EqEqEq => "===",
            NotEqEq => "!==",
            Lt => "<",
            Le => "<=",
            Gt => ">",
            Ge => ">=",
            AndAnd => "&&",
            OrOr => "||",
            QQ => "??",
            Eof => return "end of input".into(),
        };
        format!("`{s}`")
    }
}

pub(crate) fn is_ident_start(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphabetic()
}

pub(crate) fn is_ident_continue(c: char) -> bool {
    c == '_' || c == '$' || c.is_alphanumeric()
}

#[derive(Clone)]
pub(crate) struct Lexer<'s> {
    src: &'s str,
    pub(crate) pos: usize,
}

impl<'s> Lexer<'s> {
    pub(crate) fn new(src: &'s str) -> Lexer<'s> {
        Lexer { src, pos: 0 }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    fn err<T>(&self, start: usize, msg: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError::new(Span::new(start, self.pos.max(start + 1).min(self.src.len().max(start))), msg))
    }

    fn skip_trivia(&mut self) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_at(1) == Some('/') => {
                    while let Some(c) = self.bump() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some('/') if self.peek_at(1) == Some('*') => {
                    let start = self.pos;
                    self.pos += 2;
                    loop {
                        match self.bump() {
                            Some('*') if self.eat('/') => break,
                            Some(_) => {}
                            None => return self.err(start, "unterminated /* comment"),
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    pub(crate) fn next_token(&mut self) -> Result<(Tok, Span), ParseError> {
        self.skip_trivia()?;
        let start = self.pos;
        let Some(c) = self.bump() else {
            return Ok((Tok::Eof, Span::new(start, start)));
        };
        use Tok::*;
        let tok = match c {
            '0'..='9' => self.number(start)?,
            '.' if matches!(self.peek(), Some('0'..='9')) => self.number(start)?,
            '.' if self.peek() == Some('.') => return self.err(start, "spread `...` isn't supported"),
            '.' => Dot,
            '"' | '\'' => Str(self.string(start, c)?),
            '`' => Backtick,
            '(' => LParen,
            ')' => RParen,
            '[' => LBracket,
            ']' => RBracket,
            '{' => LBrace,
            '}' => RBrace,
            ',' => Comma,
            ';' => Semi,
            ':' => Colon,
            '?' => {
                if self.eat('?') {
                    QQ
                } else if self.peek() == Some('.') && !matches!(self.peek_at(1), Some('0'..='9')) {
                    return self.err(start, "optional chaining `?.` isn't supported; missing values are already null");
                } else {
                    Question
                }
            }
            '+' if self.peek() == Some('+') => return self.err(start, "`++` isn't supported: expressions are pure"),
            '-' if self.peek() == Some('-') => return self.err(start, "`--` isn't supported: expressions are pure"),
            '+' => Plus,
            '-' => Minus,
            '*' => {
                if self.eat('*') {
                    StarStar
                } else {
                    Star
                }
            }
            '/' => Slash,
            '%' => Percent,
            '!' => {
                if self.eat('=') {
                    if self.eat('=') {
                        NotEqEq
                    } else {
                        NotEq
                    }
                } else {
                    Bang
                }
            }
            '=' => {
                if self.eat('=') {
                    if self.eat('=') {
                        EqEqEq
                    } else {
                        EqEq
                    }
                } else if self.eat('>') {
                    Arrow
                } else {
                    return self.err(start, "assignment isn't allowed in expressions; did you mean `==`?");
                }
            }
            '<' => {
                if self.eat('=') {
                    Le
                } else if self.peek() == Some('<') {
                    return self.err(start, "bitwise shifts aren't supported");
                } else {
                    Lt
                }
            }
            '>' => {
                if self.eat('=') {
                    Ge
                } else if self.peek() == Some('>') {
                    return self.err(start, "bitwise shifts aren't supported");
                } else {
                    Gt
                }
            }
            '&' => {
                if self.eat('&') {
                    AndAnd
                } else {
                    return self.err(start, "bitwise `&` isn't supported; did you mean `&&`?");
                }
            }
            '|' => {
                if self.eat('|') {
                    OrOr
                } else {
                    return self.err(start, "bitwise `|` isn't supported; did you mean `||`?");
                }
            }
            '^' | '~' => return self.err(start, format!("bitwise `{c}` isn't supported")),
            c if is_ident_start(c) => {
                while self.peek().is_some_and(is_ident_continue) {
                    self.bump();
                }
                Ident(self.src[start..self.pos].to_string())
            }
            c => return self.err(start, format!("unexpected character `{c}`")),
        };
        Ok((tok, Span::new(start, self.pos)))
    }

    fn digits(&mut self, radix: u32, out: &mut String) {
        while let Some(c) = self.peek() {
            if c.is_digit(radix) {
                out.push(c);
            } else if c != '_' || !self.peek_at(1).is_some_and(|d| d.is_digit(radix)) || out.is_empty() {
                break;
            }
            self.bump();
        }
    }

    /// `c` (the first character) has been consumed.
    fn number(&mut self, start: usize) -> Result<Tok, ParseError> {
        self.pos = start;
        let prefixed = self.src[start..].get(..2).map(|p| p.to_ascii_lowercase());
        let radix = match prefixed.as_deref() {
            Some("0x") => 16,
            Some("0b") => 2,
            Some("0o") => 8,
            _ => 10,
        };
        let value = if radix != 10 {
            self.pos += 2;
            let mut ds = String::new();
            self.digits(radix, &mut ds);
            if ds.is_empty() {
                return self.err(start, "missing digits after the number prefix");
            }
            ds.chars().fold(0.0, |acc, d| acc * radix as f64 + d.to_digit(radix).unwrap_or(0) as f64)
        } else {
            let mut int = String::new();
            let mut frac = String::new();
            self.digits(10, &mut int);
            if self.eat('.') {
                self.digits(10, &mut frac);
            }
            let mut exp = String::new();
            if matches!(self.peek(), Some('e' | 'E')) {
                self.bump();
                if let Some(s @ ('+' | '-')) = self.peek() {
                    exp.push(s);
                    self.bump();
                }
                let before = exp.len();
                self.digits(10, &mut exp);
                if exp.len() == before {
                    return self.err(start, "missing exponent digits");
                }
            }
            if int.is_empty() && frac.is_empty() {
                return self.err(start, "malformed number");
            }
            let norm = format!(
                "{}.{}e{}",
                if int.is_empty() { "0" } else { &int },
                if frac.is_empty() { "0" } else { &frac },
                if exp.is_empty() { "0" } else { &exp }
            );
            norm.parse::<f64>().unwrap_or(f64::NAN)
        };
        if self.peek().is_some_and(is_ident_continue) {
            return self.err(start, "unexpected character after a number");
        }
        Ok(Tok::Num(value))
    }

    fn hex(&mut self, start: usize, n: usize) -> Result<u32, ParseError> {
        let mut v = 0u32;
        for _ in 0..n {
            match self.bump().and_then(|c| c.to_digit(16)) {
                Some(d) => v = v * 16 + d,
                None => return self.err(start, "malformed escape sequence"),
            }
        }
        Ok(v)
    }

    /// Decodes one escape after a backslash into `out`.
    fn escape(&mut self, start: usize, out: &mut String) -> Result<(), ParseError> {
        let Some(c) = self.bump() else {
            return self.err(start, "unterminated string");
        };
        match c {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !matches!(self.peek(), Some('0'..='9')) => out.push('\0'),
            'x' => {
                let v = self.hex(start, 2)?;
                out.push(char::from_u32(v).unwrap_or('\u{FFFD}'));
            }
            'u' => {
                if self.eat('{') {
                    let mut v = 0u32;
                    let mut n = 0;
                    while let Some(d) = self.peek().and_then(|c| c.to_digit(16)) {
                        self.bump();
                        v = v.saturating_mul(16).saturating_add(d);
                        n += 1;
                    }
                    if n == 0 || !self.eat('}') {
                        return self.err(start, "malformed \\u{…} escape");
                    }
                    match char::from_u32(v) {
                        Some(ch) => out.push(ch),
                        None => return self.err(start, "\\u{…} escape is not a Unicode scalar value"),
                    }
                } else {
                    let hi = self.hex(start, 4)?;
                    if (0xD800..0xDC00).contains(&hi) && self.src[self.pos..].starts_with("\\u") {
                        let save = self.pos;
                        self.pos += 2;
                        match self.hex(start, 4) {
                            Ok(lo) if (0xDC00..0xE000).contains(&lo) => {
                                let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                                out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                                return Ok(());
                            }
                            _ => self.pos = save,
                        }
                    }
                    out.push(char::from_u32(hi).unwrap_or('\u{FFFD}'));
                }
            }
            '\r' => {
                self.eat('\n');
            }
            '\n' | '\u{2028}' | '\u{2029}' => {}
            other => out.push(other),
        }
        Ok(())
    }

    fn string(&mut self, start: usize, quote: char) -> Result<String, ParseError> {
        let mut out = String::new();
        loop {
            match self.bump() {
                None | Some('\n') | Some('\r') => return self.err(start, "unterminated string"),
                Some(c) if c == quote => return Ok(out),
                Some('\\') => self.escape(start, &mut out)?,
                Some(c) => out.push(c),
            }
        }
    }

    /// Raw template text from `pos` up to the closing backtick (returns `false`) or the next `${`
    /// (returns `true`); both delimiters are consumed.
    pub(crate) fn template_chunk(&mut self) -> Result<(String, bool), ParseError> {
        let start = self.pos;
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return self.err(start.saturating_sub(1), "unterminated template literal"),
                Some('`') => return Ok((out, false)),
                Some('$') if self.eat('{') => return Ok((out, true)),
                Some('\\') => self.escape(start, &mut out)?,
                Some(c) => out.push(c),
            }
        }
    }
}
