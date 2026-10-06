//! Which characters a chart can show — what its font subsets must cover.
//!
//! The text of every state (the baked scenes), the text in the document that can become a label
//! (string values, and the quoted literals inside expressions — not keys, tokens, identifiers or
//! expression code), the text derived from its data (every character of every data file the
//! build fetched), and what number and date formatting can produce in its locale: digits,
//! separators, signs, SI prefixes, month and weekday names. Case variants are added when
//! expressions change case, mirrored brackets when text runs right to left.

use datars_scene::{NodeKind, Scene};
use std::collections::{BTreeMap, BTreeSet};

/// Characters number formatting can produce in any locale, besides the locale's own symbols:
/// digits, signs (hyphen, U+2212 minus, en dash for NaN), infinity, and the spaces formats pad
/// with. Animated numbers tween through them.
pub const NUMBERS: &str = "0123456789+-\u{2212}\u{2013}.,%\u{221e} \u{a0}\u{202f}";

/// SI prefixes, for `s` formats (`"~s"` → `"1.2k"`).
const SI: &str = "yzafpn\u{b5}mkMGTPEZY";

/// Does a string look like a d3-format specifier (`",.1~f"`, `"$,.2s"`, `".0%"`)? Returns its type.
fn format_type(s: &str) -> Option<char> {
    let t = s.chars().last()?;
    let ok = s.len() <= 12 && s.chars().all(|c| c.is_ascii_digit() || ",.~$#=<>^+-( %".contains(c) || c == t) && "efgrsdpcxXob%".contains(t);
    ok.then_some(t)
}

/// Characters layout itself may draw: the ellipsis of truncated labels.
const LAYOUT: &str = "\u{2026}";

/// Pairs right-to-left text draws mirrored (Unicode Bidi_Mirrored, the common ones).
const MIRRORS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>'), ('\u{ab}', '\u{bb}'), ('\u{2039}', '\u{203a}'), ('\u{2264}', '\u{2265}')];

/// Fields whose string values are identifiers, references or styles, never label text.
const NOT_TEXT: &[&str] = &["color", "kind", "recipe", "proto", "prov", "coord", "projection", "blend", "cap", "join", "easing", "ease", "fill", "stroke", "ink", "font", "use", "locale", "format", "url", "src", "tiles", "atlas", "google", "slot", "from", "type", "align", "baseline", "anchor", "id", "datars"];

#[derive(Default)]
pub struct Coverage {
    pub chars: BTreeSet<char>,
    /// Types of the number formats the document uses (`s` brings SI prefixes, `e` exponents).
    formats: BTreeSet<char>,
}

impl Coverage {
    pub fn new() -> Coverage {
        let mut c = Coverage::default();
        c.add_str(NUMBERS);
        c.add_str(LAYOUT);
        c
    }

    pub fn add_str(&mut self, s: &str) {
        self.chars.extend(s.chars().filter(|c| !c.is_control()));
    }

    /// The strings of a document (JSON) that can end up drawn: values, not keys; for expressions
    /// (`"=…"`) only their quoted literals; no `$tokens`; nothing under [`NOT_TEXT`] fields.
    pub fn add_doc_text(&mut self, v: &serde_json::Value) {
        match v {
            serde_json::Value::String(s) => self.add_text_value(s),
            serde_json::Value::Array(a) => a.iter().for_each(|x| self.add_doc_text(x)),
            serde_json::Value::Object(o) => {
                for (k, x) in o {
                    match (k.as_str(), x) {
                        // Pre-expanded documents keep expressions as `{ "expr": "…" }`.
                        ("expr", serde_json::Value::String(e)) => self.add_text_value(&format!("={e}")),
                        ("format", serde_json::Value::String(f)) => {
                            self.formats.extend(format_type(f));
                        }
                        (k, _) if NOT_TEXT.contains(&k) => {}
                        _ => self.add_doc_text(x),
                    }
                }
            }
            _ => {}
        }
    }

    fn add_text_value(&mut self, s: &str) {
        if let Some(expr) = s.strip_prefix('=') {
            for lit in quoted_literals(expr) {
                // `format(x, ",.1~f")`: a format specifier, not text.
                match format_type(&lit) {
                    Some(t) if lit.len() > 1 => {
                        self.formats.insert(t);
                    }
                    _ => self.add_str(&lit),
                }
            }
        } else if !(s.starts_with('$') && s[1..].chars().all(|c| c.is_ascii_alphanumeric() || "._-[]@".contains(c))) {
            self.add_str(s);
        }
    }

    /// A data file's text (CSV, JSON, GeoJSON): every character in it, a superset of its values.
    pub fn add_bytes(&mut self, b: &[u8]) {
        self.add_str(&String::from_utf8_lossy(b));
    }

    /// Every label drawn in a scene.
    pub fn add_scene(&mut self, s: &Scene) {
        self.chars.extend(scene_text(s).into_values().flatten());
    }

    /// The locale's number symbols, and its month and weekday names when the chart formats
    /// dates.
    pub fn add_locale(&mut self, tag: &str, dates: bool) {
        let l = datars_text::locale::get(tag);
        for s in [l.decimal, l.group, l.currency_prefix, l.currency_suffix, l.percent, l.minus] {
            self.add_str(s);
        }
        if dates {
            for s in l.months.iter().chain(&l.months_short).chain(&l.days).chain(&l.days_short) {
                self.add_str(s);
            }
        }
    }

    /// Close over case changes (when expressions can change case) and bidi mirroring (when
    /// there is right-to-left text).
    pub fn finish(mut self, case_changes: bool) -> BTreeSet<char> {
        if self.formats.contains(&'s') {
            self.add_str(SI);
        }
        if self.formats.iter().any(|t| matches!(t, 'e' | 'g' | 'r')) {
            self.add_str("e");
        }
        close(self.chars, case_changes)
    }
}

/// [`Coverage::finish`] for any set.
pub fn close(mut chars: BTreeSet<char>, case_changes: bool) -> BTreeSet<char> {
    let rtl = chars.iter().any(|&c| matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF));
    let mut extra: Vec<char> = Vec::new();
    for &c in &chars {
        if case_changes && c.is_alphabetic() {
            // Same script only: the micro sign's uppercase is Greek, which no label asked for.
            let same = |v: &char| (c as u32) < 0x250 && (*v as u32) < 0x250 || (c as u32) >> 8 == (*v as u32) >> 8;
            extra.extend(c.to_uppercase().chain(c.to_lowercase()).filter(same));
        }
        if rtl {
            for &(a, b) in MIRRORS {
                if c == a {
                    extra.push(b);
                } else if c == b {
                    extra.push(a);
                }
            }
        }
    }
    chars.extend(extra.into_iter().filter(|c| !c.is_control()));
    chars
}

/// The characters each face draws in a scene, by face id. A label's characters count for every
/// face its runs use (fallback splits a label across faces; subsetting drops what a face lacks).
pub fn scene_text(s: &Scene) -> BTreeMap<String, BTreeSet<char>> {
    let mut out: BTreeMap<String, BTreeSet<char>> = BTreeMap::new();
    s.root.walk(&Default::default(), &mut |_, n| {
        if let NodeKind::Text(t) = &n.kind {
            for r in &t.runs {
                out.entry(r.font.to_string()).or_default().extend(t.text.chars().filter(|c| !c.is_control()));
            }
        }
    });
    out
}

/// The string literals in an expression: `"…"` and `'…'` (with `\` escapes), and the text of
/// template literals (`` `${a}: ${b}` `` → `": "`).
fn quoted_literals(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            let mut lit = String::new();
            let mut depth = 0usize;
            while let Some(d) = chars.next() {
                match d {
                    '$' if depth == 0 && chars.peek() == Some(&'{') => {
                        chars.next();
                        depth = 1;
                    }
                    '{' if depth > 0 => depth += 1,
                    '}' if depth > 0 => depth -= 1,
                    '`' if depth == 0 => break,
                    _ if depth == 0 => lit.push(d),
                    _ => {}
                }
            }
            out.push(lit);
        } else if c == '"' || c == '\'' {
            let mut lit = String::new();
            let mut escaped = false;
            for d in chars.by_ref() {
                if escaped {
                    lit.push(d);
                    escaped = false;
                } else if d == '\\' {
                    escaped = true;
                } else if d == c {
                    break;
                } else {
                    lit.push(d);
                }
            }
            out.push(lit);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_is_text_numbers_and_locale() {
        let mut c = Coverage::new();
        c.add_doc_text(&serde_json::json!({
            "title": "Växjö (kommun)", "fill": "$accent", "kind": "group",
            "label": "=d.share > 0.5 ? \"Majorité\" : 'Övriga'", "values": [1, 2],
            "text": { "expr": "`${key.name(d.party)}: ${format(d.share, \",.1~f\")}`" }
        }));
        c.add_bytes("name,pop\nÅre,3000\n".as_bytes());
        c.add_locale("sv", true);
        let set = c.finish(false);
        for ch in ['V', 'ä', 'x', 'j', 'ö', '(', ')', 'Å', 'é', 'Ö', '0', '9', '\u{2212}', '\u{a0}', 'k', '\u{2026}'] {
            assert!(set.contains(&ch), "{ch:?}");
        }
        // Expression code, keys and tokens are not text: no `?`, `>`, `$`, `{`, `~`; no case
        // variants. A template literal's text is (`:`).
        assert!(set.contains(&':'));
        for ch in ['?', '>', '$', '{', '~', '\n', 'Ä', 'É'] {
            assert!(!set.contains(&ch), "{ch:?}");
        }
        assert!(!set.iter().any(|&ch| ('\u{400}'..='\u{4ff}').contains(&ch) || ('\u{4e00}'..='\u{9fff}').contains(&ch)));
    }

    #[test]
    fn case_changes_and_right_to_left_close_the_set() {
        let set = close("aö\u{b5}(".chars().collect(), true);
        assert!(set.contains(&'A') && set.contains(&'Ö') && !set.contains(&'\u{39c}'), "{set:?}");
        assert!(!set.contains(&')'), "no mirroring without right-to-left text");
        let rtl = close("א(".chars().collect(), false);
        assert!(rtl.contains(&')'));
    }
}
