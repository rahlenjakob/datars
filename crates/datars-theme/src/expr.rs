//! Colour expressions: `#hex`, `$token`, `$palette[3]`, and functions — `mix(a, b, t)`,
//! `alpha(a, x)`, `lighten(a, d)`, `darken(a, d)`, `saturate(a, d)`, `desaturate(a, d)`,
//! `oklch(l, c, hdeg)`, `rgb(r, g, b)`, `on(bg)` (black or white, whichever reads better on `bg`),
//! `contrast(bg, a, b)` (whichever of a/b contrasts more with bg).

use datars_color::Color;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum CExpr {
    Lit(Color),
    Ref { name: String, index: Option<u32> },
    Num(f64),
    Call { f: String, args: Vec<CExpr> },
}

const FUNCS: &[&str] = &["mix", "alpha", "lighten", "darken", "saturate", "desaturate", "oklch", "rgb", "on", "contrast"];

impl fmt::Display for CExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CExpr::Lit(c) => write!(f, "{}", c.to_hex()),
            CExpr::Ref { name, index: None } => write!(f, "${name}"),
            CExpr::Ref { name, index: Some(i) } => write!(f, "${name}[{i}]"),
            CExpr::Num(n) => write!(f, "{n}"),
            CExpr::Call { f: name, args } => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ")")
            }
        }
    }
}

/// Parse a colour expression; `None` if the string isn't one (then it's a text token).
pub fn parse_color_expr(s: &str) -> Option<CExpr> {
    let mut p = P { s: s.trim().as_bytes(), i: 0 };
    let e = p.expr()?;
    p.ws();
    (p.i == p.s.len() && !matches!(e, CExpr::Num(_))).then_some(e)
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn ident(&mut self) -> String {
        let st = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || matches!(self.s[self.i], b'-' | b'_' | b'.')) {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).into_owned()
    }
    fn expr(&mut self) -> Option<CExpr> {
        self.ws();
        let c = *self.s.get(self.i)?;
        if c == b'#' {
            let st = self.i;
            self.i += 1;
            while self.i < self.s.len() && self.s[self.i].is_ascii_hexdigit() {
                self.i += 1;
            }
            return Color::parse(std::str::from_utf8(&self.s[st..self.i]).ok()?).map(CExpr::Lit);
        }
        if c == b'$' {
            self.i += 1;
            let name = self.ident();
            if name.is_empty() {
                return None;
            }
            let mut index = None;
            if self.s.get(self.i) == Some(&b'[') {
                self.i += 1;
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i].is_ascii_digit() {
                    self.i += 1;
                }
                index = std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok();
                if self.s.get(self.i) != Some(&b']') {
                    return None;
                }
                self.i += 1;
            }
            return Some(CExpr::Ref { name, index });
        }
        if c.is_ascii_digit() || c == b'-' || c == b'.' {
            let st = self.i;
            self.i += 1;
            while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || self.s[self.i] == b'.') {
                self.i += 1;
            }
            let mut v: f64 = std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok()?;
            if self.s.get(self.i) == Some(&b'%') {
                self.i += 1;
                v /= 100.0;
            }
            return Some(CExpr::Num(v));
        }
        if c.is_ascii_alphabetic() {
            let name = self.ident();
            if !FUNCS.contains(&name.as_str()) {
                return None;
            }
            self.ws();
            if self.s.get(self.i) != Some(&b'(') {
                return None;
            }
            self.i += 1;
            let mut args = Vec::new();
            loop {
                self.ws();
                if self.s.get(self.i) == Some(&b')') {
                    self.i += 1;
                    break;
                }
                args.push(self.expr()?);
                self.ws();
                match self.s.get(self.i) {
                    Some(b',') => self.i += 1,
                    Some(b')') => {
                        self.i += 1;
                        break;
                    }
                    _ => return None,
                }
            }
            return Some(CExpr::Call { f: name, args });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_prints() {
        for s in ["#e8112d", "$accent", "$categorical[3]", "mix($ink, $paper, 0.88)", "alpha($accent, 30%)", "on($accent)"] {
            let e = parse_color_expr(s).unwrap_or_else(|| panic!("{s}"));
            let again = parse_color_expr(&e.to_string()).unwrap();
            assert_eq!(e, again, "{s}");
        }
        assert!(parse_color_expr("cubic-in-out").is_none(), "plain text is not a colour");
        assert!(parse_color_expr("12").is_none());
    }
}
