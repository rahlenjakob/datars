//! A small XML reader for SVG files: elements, attributes, text and CDATA, the five named
//! entities and numeric references. Enough for every icon and logo exporter we know of, and no
//! more: no DTDs (an internal subset with `<!ENTITY …>` is refused, so a file can't expand itself
//! into gigabytes), no namespaces beyond keeping the prefix in the name, and limits on depth and
//! size so a hostile file can't make the reader work without end.

/// Elements nested deeper than this are refused (real icons nest a handful).
const MAX_DEPTH: usize = 64;
/// Elements in one file, at most.
const MAX_ELEMENTS: usize = 50_000;
/// Bytes in one file, at most (a detailed map logo is a few hundred KB).
pub const MAX_BYTES: usize = 8 << 20;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Element {
    /// The tag name as written, prefix included (`svg`, `sodipodi:namedview`).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    El(Element),
    Text(String),
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    /// The name without an `svg:` prefix (some exporters write `<svg:path>`).
    pub fn local(&self) -> &str {
        self.name.strip_prefix("svg:").unwrap_or(&self.name)
    }
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|c| match c {
            Node::El(e) => Some(e),
            Node::Text(_) => None,
        })
    }
    /// All the text directly inside (a `<style>` sheet, a `<title>`).
    pub fn text(&self) -> String {
        self.children.iter().filter_map(|c| if let Node::Text(t) = c { Some(t.as_str()) } else { None }).collect()
    }
}

/// Read a document's root element.
pub fn parse(src: &str) -> Result<Element, String> {
    if src.len() > MAX_BYTES {
        return Err(format!("the file is {} KB; SVGs over {} KB aren't read", src.len() / 1024, MAX_BYTES / 1024));
    }
    let mut r = Reader { s: src.as_bytes(), i: 0, count: 0 };
    r.prolog()?;
    let root = r.element(0)?;
    Ok(root)
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
    count: usize,
}

impl Reader<'_> {
    fn peek(&self, lit: &str) -> bool {
        self.s[self.i..].starts_with(lit.as_bytes())
    }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn skip_past(&mut self, end: &str) -> Result<(), String> {
        match find(&self.s[self.i..], end.as_bytes()) {
            Some(k) => {
                self.i += k + end.len();
                Ok(())
            }
            None => Err(format!("unterminated markup (no `{end}`)")),
        }
    }

    /// The BOM, the XML declaration, processing instructions, comments and a doctype, up to the
    /// root element.
    fn prolog(&mut self) -> Result<(), String> {
        if self.peek("\u{feff}") {
            self.i += 3;
        }
        loop {
            self.skip_ws();
            if self.peek("<?") {
                self.skip_past("?>")?;
            } else if self.peek("<!--") {
                self.skip_past("-->")?;
            } else if self.peek("<!DOCTYPE") || self.peek("<!doctype") {
                self.doctype()?;
            } else if self.peek("<") {
                return Ok(());
            } else {
                return Err("not an SVG file (no root element)".into());
            }
        }
    }

    fn doctype(&mut self) -> Result<(), String> {
        // `<!DOCTYPE svg PUBLIC "…" "…">`, maybe with an internal subset in brackets.
        let start = self.i;
        let mut depth = 0i32;
        while self.i < self.s.len() {
            match self.s[self.i] {
                b'[' => depth += 1,
                b']' => depth -= 1,
                b'>' if depth <= 0 => {
                    self.i += 1;
                    let text = String::from_utf8_lossy(&self.s[start..self.i]);
                    if text.contains("<!ENTITY") {
                        return Err("the file declares XML entities, which aren't read (they can make a small file expand without end)".into());
                    }
                    return Ok(());
                }
                _ => {}
            }
            self.i += 1;
        }
        Err("unterminated doctype".into())
    }

    fn name(&mut self) -> String {
        let st = self.i;
        while self.i < self.s.len() && !self.s[self.i].is_ascii_whitespace() && !matches!(self.s[self.i], b'>' | b'/' | b'=' | b'<') {
            self.i += 1;
        }
        String::from_utf8_lossy(&self.s[st..self.i]).into_owned()
    }

    fn element(&mut self, depth: usize) -> Result<Element, String> {
        if depth > MAX_DEPTH {
            return Err(format!("elements nested over {MAX_DEPTH} deep"));
        }
        self.count += 1;
        if self.count > MAX_ELEMENTS {
            return Err(format!("over {MAX_ELEMENTS} elements"));
        }
        if !self.peek("<") {
            return Err("expected an element".into());
        }
        self.i += 1;
        let name = self.name();
        if name.is_empty() {
            return Err("an element without a name".into());
        }
        let mut el = Element { name, ..Default::default() };
        loop {
            self.skip_ws();
            if self.i >= self.s.len() {
                return Err(format!("<{}> isn't closed", el.name));
            }
            if self.peek("/>") {
                self.i += 2;
                return Ok(el);
            }
            if self.peek(">") {
                self.i += 1;
                break;
            }
            let k = self.name();
            if k.is_empty() {
                return Err(format!("a malformed attribute in <{}>", el.name));
            }
            self.skip_ws();
            let v = if self.peek("=") {
                self.i += 1;
                self.skip_ws();
                let q = *self.s.get(self.i).ok_or("an attribute without a value")?;
                if q != b'"' && q != b'\'' {
                    return Err(format!("attribute `{k}` isn't quoted"));
                }
                self.i += 1;
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i] != q {
                    self.i += 1;
                }
                if self.i >= self.s.len() {
                    return Err(format!("attribute `{k}` isn't closed"));
                }
                let raw = String::from_utf8_lossy(&self.s[st..self.i]).into_owned();
                self.i += 1;
                unescape(&raw)
            } else {
                String::new()
            };
            el.attrs.push((k, v));
        }
        // Content up to the matching end tag.
        loop {
            if self.i >= self.s.len() {
                return Err(format!("<{}> isn't closed", el.name));
            }
            if self.peek("</") {
                self.i += 2;
                let end = self.name();
                self.skip_ws();
                if !self.peek(">") {
                    return Err(format!("a malformed end tag </{end}"));
                }
                self.i += 1;
                if end != el.name {
                    return Err(format!("<{}> is closed by </{end}>", el.name));
                }
                return Ok(el);
            }
            if self.peek("<!--") {
                self.skip_past("-->")?;
            } else if self.peek("<![CDATA[") {
                self.i += 9;
                let st = self.i;
                self.skip_past("]]>")?;
                el.children.push(Node::Text(String::from_utf8_lossy(&self.s[st..self.i - 3]).into_owned()));
            } else if self.peek("<?") {
                self.skip_past("?>")?;
            } else if self.peek("<") {
                let child = self.element(depth + 1)?;
                el.children.push(Node::El(child));
            } else {
                let st = self.i;
                while self.i < self.s.len() && self.s[self.i] != b'<' {
                    self.i += 1;
                }
                let t = unescape(&String::from_utf8_lossy(&self.s[st..self.i]));
                if !t.trim().is_empty() {
                    el.children.push(Node::Text(t));
                }
            }
        }
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The five named entities and numeric character references; anything else stays as written.
fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(k) = rest.find('&') {
        out.push_str(&rest[..k]);
        rest = &rest[k..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")).and_then(|h| u32::from_str_radix(h, 16).ok()).or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok())).and_then(char::from_u32),
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elements_attributes_and_text() {
        let e = parse("<?xml version=\"1.0\"?>\n<!-- made by hand -->\n<svg viewBox='0 0 4 4'><g id=\"a\"><path d=\"M0 0\"/></g><title>A &amp; B &#x41;</title></svg>").unwrap();
        assert_eq!(e.name, "svg");
        assert_eq!(e.attr("viewBox"), Some("0 0 4 4"));
        let g = e.elements().next().unwrap();
        assert_eq!(g.attr("id"), Some("a"));
        assert_eq!(g.elements().next().unwrap().attr("d"), Some("M0 0"));
        assert_eq!(e.elements().nth(1).unwrap().text(), "A & B A");
    }

    #[test]
    fn cdata_style_sheets_are_text() {
        let e = parse("<svg><style><![CDATA[.a{fill:red}]]></style></svg>").unwrap();
        assert_eq!(e.elements().next().unwrap().text(), ".a{fill:red}");
    }

    #[test]
    fn entity_declarations_and_bad_markup_are_refused() {
        let bomb = "<!DOCTYPE svg [<!ENTITY a \"aaaa\"><!ENTITY b \"&a;&a;\">]><svg>&b;</svg>";
        assert!(parse(bomb).unwrap_err().contains("entities"));
        assert!(parse("<svg><g></svg>").is_err());
        assert!(parse("<svg width=3/>").is_err());
        assert!(parse("hello").is_err());
        let deep = format!("<svg>{}{}</svg>", "<g>".repeat(100), "</g>".repeat(100));
        assert!(parse(&deep).unwrap_err().contains("deep"));
    }

    #[test]
    fn a_plain_doctype_is_fine() {
        let e = parse("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\"><svg/>").unwrap();
        assert_eq!(e.name, "svg");
    }
}
