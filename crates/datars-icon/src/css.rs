//! The CSS that icon and logo exporters write: `style="fill:#e30613;stroke:none"` on elements, and
//! a `<style>` sheet of simple rules (`.cls-1{fill:#e30613}`, `path, circle {…}`, `#logo …`).
//! Selectors are a tag, a class, an id, `*`, or a tag with classes; anything else (descendants,
//! pseudo-classes, attribute selectors) is skipped and noted. `@import` and `url(…)` to anything
//! but a fragment of the same file are refused: an icon never reaches outside itself.

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selector: Simple,
    /// (id count, class count, tag count): CSS specificity, compared in that order.
    pub specificity: (u32, u32, u32),
    pub order: usize,
    pub decls: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Simple {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

impl Simple {
    pub fn matches(&self, tag: &str, id: Option<&str>, classes: &[&str]) -> bool {
        self.tag.as_deref().is_none_or(|t| t == tag) && self.id.as_deref().is_none_or(|i| Some(i) == id) && self.classes.iter().all(|c| classes.contains(&c.as_str()))
    }
}

/// `prop: value; prop: value` → pairs (names lower-cased, `!important` dropped).
pub fn declarations(s: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for d in s.split(';') {
        let Some((k, v)) = d.split_once(':') else { continue };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim().trim_end_matches("!important").trim().to_string();
        if k.is_empty() {
            continue;
        }
        check_value(&v)?;
        out.push((k, v));
    }
    Ok(out)
}

/// Values may refer to this file's own fragments only (`url(#gradient)`).
pub fn check_value(v: &str) -> Result<(), String> {
    let lower = v.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(k) = rest.find("url(") {
        let inner = rest[k + 4..].trim_start().trim_start_matches(['"', '\'']);
        if !inner.starts_with('#') {
            return Err(format!("`{v}` refers outside the file; icons are drawn from their own contents only"));
        }
        rest = &rest[k + 4..];
    }
    if lower.contains("expression(") || lower.contains("javascript:") {
        return Err(format!("`{v}` is script"));
    }
    Ok(())
}

/// A `<style>` sheet's rules, and notes on what was skipped.
pub fn sheet(src: &str, order0: usize) -> Result<(Vec<Rule>, Vec<String>), String> {
    let src = strip_comments(src);
    if src.to_ascii_lowercase().contains("@import") {
        return Err("the style sheet imports another (`@import`); icons are drawn from their own contents only".into());
    }
    let mut rules = Vec::new();
    let mut notes = Vec::new();
    let mut rest = src.as_str();
    let mut order = order0;
    while let Some(open) = rest.find('{') {
        let sel = rest[..open].trim();
        let Some(close) = rest[open..].find('}') else { break };
        let body = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        if sel.starts_with('@') {
            // `@media`, `@font-face`: their blocks nest; skip to the end of the outer one.
            notes.push(format!("skipped the style sheet's `{}` block", sel.split_whitespace().next().unwrap_or(sel)));
            if body.contains('{') {
                if let Some(end) = rest.find('}') {
                    rest = &rest[end + 1..];
                }
            }
            continue;
        }
        let decls = declarations(body)?;
        for one in sel.split(',') {
            match simple(one.trim()) {
                Some(s) => {
                    let spec = (s.id.is_some() as u32, s.classes.len() as u32, s.tag.is_some() as u32);
                    rules.push(Rule { selector: s, specificity: spec, order, decls: decls.clone() });
                    order += 1;
                }
                None => notes.push(format!("skipped the style rule `{}` (only tags, classes and ids are read)", one.trim())),
            }
        }
    }
    Ok((rules, notes))
}

fn strip_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find("/*") {
        out.push_str(&rest[..k]);
        match rest[k + 2..].find("*/") {
            Some(e) => rest = &rest[k + 2 + e + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// `path`, `.a`, `#b`, `*`, `path.a.b` — a compound selector without combinators.
fn simple(s: &str) -> Option<Simple> {
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || matches!(c, '>' | '+' | '~' | ':' | '[')) {
        return None;
    }
    let mut out = Simple { tag: None, id: None, classes: Vec::new() };
    let mut cur = String::new();
    let mut kind = 't';
    let flush = |kind: char, cur: &mut String, out: &mut Simple| {
        if !cur.is_empty() {
            match kind {
                't' if cur != "*" => out.tag = Some(std::mem::take(cur)),
                '.' => out.classes.push(std::mem::take(cur)),
                '#' => out.id = Some(std::mem::take(cur)),
                _ => cur.clear(),
            }
        }
    };
    for c in s.chars() {
        if c == '.' || c == '#' {
            flush(kind, &mut cur, &mut out);
            kind = c;
        } else {
            cur.push(c);
        }
    }
    flush(kind, &mut cur, &mut out);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_and_rules() {
        assert_eq!(declarations("fill:#E30613; stroke : none !important;").unwrap(), vec![("fill".into(), "#E30613".into()), ("stroke".into(), "none".into())]);
        let (rules, notes) = sheet("/* Illustrator */ .cls-1,.cls-2{fill:#fff}#a{fill:red} g path{fill:blue} @media print{.x{fill:#000}} rect.b.c{stroke:#000}", 0).unwrap();
        assert_eq!(rules.len(), 4, "{rules:?}");
        assert!(rules[0].selector.matches("path", None, &["cls-1"]));
        assert!(!rules[0].selector.matches("path", None, &["cls-3"]));
        assert_eq!(rules[2].specificity, (1, 0, 0));
        assert!(rules[3].selector.matches("rect", None, &["c", "b"]));
        assert!(notes.iter().any(|n| n.contains("g path")) && notes.iter().any(|n| n.contains("@media")), "{notes:?}");
    }

    #[test]
    fn nothing_outside_the_file() {
        assert!(declarations("fill:url(#g)").is_ok());
        assert!(declarations("fill:url(https://example.com/x.svg#g)").is_err());
        assert!(declarations("fill: url( 'x.svg#g')").is_err());
        assert!(sheet("@import url(theme.css); .a{fill:red}", 0).is_err());
    }
}
