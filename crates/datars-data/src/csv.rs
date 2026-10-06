//! CSV → `Table` (RFC 4180 and the real-world variants around it).
//!
//! - Quoted fields with escaped quotes (`""`) and embedded delimiters / newlines; CRLF, LF or CR
//!   line ends; a UTF-8 byte-order mark is skipped; blank lines are skipped.
//! - Delimiter detection among `,` `;` and tab (the one that splits the first lines consistently).
//! - Encoding: UTF-8, falling back to Windows-1252 when the bytes aren't valid UTF-8 (spreadsheet
//!   exports), unless [`Encoding::Utf8`] is requested.
//! - Type inference per column: bool, number, ISO date, else string. Empty cells are null;
//!   common missing-value markers (`NA`, `..`, `-`, …) are null in non-text columns. Numbers accept
//!   signs, decimals and exponents, and a decimal comma (`3,14`, `1 234,5`) when the delimiter is `;`
//!   (or when asked). Zero-padded integers (`0114`) stay text: they are codes, and dropping the
//!   zero would break joins. Plain four-digit years stay numbers unless `years_as_dates` is set.
//! - Per-column type overrides are parsed leniently and report bad cells by line and column.

use crate::table::{Column, ColumnType, Table};
use crate::{date, DataError};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Text encoding of the input bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 if valid, else Windows-1252.
    #[default]
    Auto,
    /// UTF-8 only; invalid bytes are an error.
    Utf8,
    /// Windows-1252 (a superset of Latin-1).
    Windows1252,
}

/// How to read a CSV file. `CsvOptions::default()` infers everything.
#[derive(Clone, Debug)]
pub struct CsvOptions {
    /// The table's name.
    pub name: String,
    /// Field delimiter; `None` detects `,` `;` or tab.
    pub delimiter: Option<u8>,
    /// Whether the first record holds column names (otherwise `column_1`, `column_2`, …).
    pub header: bool,
    /// Accept `3,14` as a number. `None` = only when the delimiter is `;`.
    pub decimal_comma: Option<bool>,
    /// Column types that skip inference.
    pub types: BTreeMap<String, ColumnType>,
    /// Key columns (validated: present, non-null, unique).
    pub key: Vec<String>,
    /// Cell texts read as null in number, date and bool columns (empty cells are always null).
    pub null_values: Vec<String>,
    /// Infer columns of four-digit integers as dates (January 1st) instead of numbers.
    pub years_as_dates: bool,
    /// Trim spaces around unquoted fields.
    pub trim: bool,
    pub encoding: Encoding,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            name: String::new(),
            delimiter: None,
            header: true,
            decimal_comma: None,
            types: BTreeMap::new(),
            key: Vec::new(),
            null_values: ["NA", "N/A", "n/a", "NaN", "nan", "null", "NULL", "..", "-", "–"].iter().map(|s| s.to_string()).collect(),
            years_as_dates: false,
            trim: true,
            encoding: Encoding::Auto,
        }
    }
}

impl CsvOptions {
    pub fn named(name: &str) -> CsvOptions {
        CsvOptions { name: name.to_string(), ..CsvOptions::default() }
    }
    pub fn delimiter(mut self, d: u8) -> Self {
        self.delimiter = Some(d);
        self
    }
    pub fn no_header(mut self) -> Self {
        self.header = false;
        self
    }
    pub fn decimal_comma(mut self, on: bool) -> Self {
        self.decimal_comma = Some(on);
        self
    }
    pub fn column_type(mut self, column: &str, ty: ColumnType) -> Self {
        self.types.insert(column.to_string(), ty);
        self
    }
    pub fn key(mut self, columns: &[&str]) -> Self {
        self.key = columns.iter().map(|c| c.to_string()).collect();
        self
    }
    pub fn years_as_dates(mut self, on: bool) -> Self {
        self.years_as_dates = on;
        self
    }
    pub fn encoding(mut self, e: Encoding) -> Self {
        self.encoding = e;
        self
    }
}

/// Read CSV bytes into a table (see the module docs for what is accepted and inferred).
pub fn read_csv(bytes: &[u8], opts: &CsvOptions) -> Result<Table, DataError> {
    let text = decode(bytes, opts.encoding)?;
    let src: &str = &text;
    let delim = opts.delimiter.unwrap_or_else(|| detect_delimiter(src));
    if matches!(delim, b'"' | b'\n' | b'\r') || !delim.is_ascii() {
        return Err(DataError::Invalid(format!("unusable delimiter {:?}", delim as char)));
    }
    let decimal_comma = opts.decimal_comma.unwrap_or(delim == b';');
    let records = Parser { src, b: src.as_bytes(), pos: 0, line: 1, line_start: 0, delim, trim: opts.trim }.records()?;

    let (names, body): (Vec<String>, &[Record]) = if opts.header {
        match records.split_first() {
            Some((h, rest)) => (header_names(h), rest),
            None => (Vec::new(), &[]),
        }
    } else {
        let w = records.iter().map(|r| r.fields.len()).max().unwrap_or(0);
        ((1..=w).map(|i| format!("column_{i}")).collect(), &records[..])
    };
    let width = names.len();
    for r in body {
        if r.fields.len() > width && r.fields[width..].iter().any(|f| !f.text.is_empty()) {
            return Err(DataError::Parse {
                line: r.line,
                column: 0,
                message: format!("{} fields, but the header has {width} (is the delimiter {:?} right?)", r.fields.len(), delim as char),
            });
        }
    }
    for name in opts.types.keys() {
        if !names.contains(name) {
            return Err(DataError::UnknownColumn { name: name.clone(), available: names.clone() });
        }
    }

    let cx = Cx { src, decimal_comma, opts };
    let mut columns = Vec::with_capacity(width);
    for (c, name) in names.iter().enumerate() {
        let cells: Vec<Option<&Field>> = body.iter().map(|r| r.fields.get(c)).collect();
        let col = match opts.types.get(name) {
            Some(&ty) => cx.convert(name, ty, &cells)?,
            None => cx.infer(&cells),
        };
        columns.push((name.clone(), col));
    }
    let table = Table { name: opts.name.clone(), columns, key: opts.key.clone(), version: 0 };
    table.validate()?;
    table.validate_keys()?;
    Ok(table)
}

/// Parse one number the way the reader does (`decimal_comma` also accepts `1 234,5` / `1.234,5`).
pub fn parse_number(s: &str, decimal_comma: bool) -> Option<f64> {
    let s = s.trim();
    let owned: String;
    let s = if decimal_comma && s.contains(',') {
        if s.matches(',').count() > 1 {
            return None;
        }
        owned = s.chars().filter(|c| !matches!(c, '.' | ' ' | '\u{a0}' | '\u{202f}')).map(|c| if c == ',' { '.' } else { c }).collect();
        owned.as_str()
    } else {
        s
    };
    let b = s.as_bytes();
    let mut i = 0;
    let digits = |i: &mut usize| {
        let s = *i;
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
        *i - s
    };
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut n = digits(&mut i);
    if i < b.len() && b[i] == b'.' {
        i += 1;
        n += digits(&mut i);
    }
    if n == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        if digits(&mut i) == 0 {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    s.parse::<f64>().ok()
}

fn decode(bytes: &[u8], enc: Encoding) -> Result<Cow<'_, str>, DataError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match (enc, std::str::from_utf8(bytes)) {
        (Encoding::Windows1252, _) => Ok(Cow::Owned(windows_1252(bytes))),
        (_, Ok(s)) => Ok(Cow::Borrowed(s)),
        (Encoding::Auto, Err(_)) => Ok(Cow::Owned(windows_1252(bytes))),
        (Encoding::Utf8, Err(e)) => {
            let good = &bytes[..e.valid_up_to()];
            let line = good.iter().filter(|&&b| b == b'\n').count() + 1;
            let start = good.iter().rposition(|&b| b == b'\n').map_or(0, |p| p + 1);
            let column = std::str::from_utf8(&good[start..]).map_or(1, |s| s.chars().count() + 1);
            Err(DataError::Parse { line, column, message: "invalid UTF-8 (is the file Windows-1252 / Latin-1?)".into() })
        }
    }
}

fn windows_1252(bytes: &[u8]) -> String {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{81}', '\u{201A}', '\u{192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}', '\u{2C6}', '\u{2030}', '\u{160}',
        '\u{2039}', '\u{152}', '\u{8D}', '\u{17D}', '\u{8F}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}',
        '\u{2013}', '\u{2014}', '\u{2DC}', '\u{2122}', '\u{161}', '\u{203A}', '\u{153}', '\u{9D}', '\u{17E}', '\u{178}',
    ];
    bytes.iter().map(|&b| if (0x80..0xA0).contains(&b) { HIGH[(b - 0x80) as usize] } else { b as char }).collect()
}

/// The candidate that splits the first lines into the same number (> 1) of fields; failing that,
/// the one that splits the first line most. Quote-aware.
fn detect_delimiter(src: &str) -> u8 {
    let mut best = (0usize, 0usize, b',');
    for &d in b",;\t" {
        let counts = field_counts(src.as_bytes(), d, 10);
        let Some(&first) = counts.first() else { continue };
        let consistent = counts.iter().all(|&c| c == first);
        let score = if first > 1 && consistent { (2, first) } else if first > 1 { (1, first) } else { (0, 0) };
        if (score.0, score.1) > (best.0, best.1) {
            best = (score.0, score.1, d);
        }
    }
    best.2
}

/// Fields per line (outside quotes) for the first `max_lines` non-blank lines.
fn field_counts(b: &[u8], d: u8, max_lines: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let (mut n, mut in_q, mut blank) = (1usize, false, true);
    let mut i = 0;
    while i < b.len() && out.len() < max_lines {
        let c = b[i];
        if in_q {
            if c == b'"' {
                in_q = false;
            }
        } else if c == b'"' {
            in_q = true;
            blank = false;
        } else if c == d {
            n += 1;
            blank = false;
        } else if c == b'\n' || c == b'\r' {
            if !blank {
                out.push(n);
            }
            n = 1;
            blank = true;
        } else if !c.is_ascii_whitespace() {
            blank = false;
        }
        i += 1;
    }
    if !blank && out.len() < max_lines {
        out.push(n);
    }
    out
}

struct Field<'a> {
    text: Cow<'a, str>,
    quoted: bool,
    line: usize,
    /// Byte offsets of the field start and its line start (for column numbers in errors).
    pos: usize,
    line_start: usize,
}

struct Record<'a> {
    line: usize,
    fields: Vec<Field<'a>>,
}

struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    line: usize,
    line_start: usize,
    delim: u8,
    trim: bool,
}

fn column_of(src: &str, line_start: usize, pos: usize) -> usize {
    src.get(line_start..pos).map_or(1, |s| s.chars().count() + 1)
}

impl<'a> Parser<'a> {
    fn err(&self, line: usize, line_start: usize, pos: usize, message: &str) -> DataError {
        DataError::Parse { line, column: column_of(self.src, line_start, pos), message: message.into() }
    }

    fn newline(&mut self) {
        self.line += 1;
        self.line_start = self.pos;
    }

    /// Consume a line end at `pos` (CRLF, LF or CR); true if there was one.
    fn eat_newline(&mut self) -> bool {
        match self.b.get(self.pos) {
            Some(b'\r') => {
                self.pos += 1;
                if self.b.get(self.pos) == Some(&b'\n') {
                    self.pos += 1;
                }
                self.newline();
                true
            }
            Some(b'\n') => {
                self.pos += 1;
                self.newline();
                true
            }
            _ => false,
        }
    }

    fn records(mut self) -> Result<Vec<Record<'a>>, DataError> {
        let mut out = Vec::new();
        while self.pos < self.b.len() {
            let line = self.line;
            let mut fields = Vec::new();
            loop {
                fields.push(self.field()?);
                if self.b.get(self.pos) == Some(&self.delim) {
                    self.pos += 1;
                    continue;
                }
                self.eat_newline();
                break;
            }
            let blank = fields.len() == 1 && !fields[0].quoted && fields[0].text.trim().is_empty();
            if !blank {
                out.push(Record { line, fields });
            }
        }
        Ok(out)
    }

    fn field(&mut self) -> Result<Field<'a>, DataError> {
        let (b, d) = (self.b, self.delim);
        let is_space = |c: u8| c == b' ' || (c == b'\t' && d != b'\t');
        let mut p = self.pos;
        while p < b.len() && is_space(b[p]) {
            p += 1;
        }
        if p < b.len() && b[p] == b'"' {
            let (line, line_start, open) = (self.line, self.line_start, p);
            self.pos = p + 1;
            let start = self.pos;
            let mut escaped = false;
            let end = loop {
                match b.get(self.pos) {
                    None => return Err(self.err(line, line_start, open, "unterminated quoted field (the quote opened here is never closed)")),
                    Some(b'"') if b.get(self.pos + 1) == Some(&b'"') => {
                        escaped = true;
                        self.pos += 2;
                    }
                    Some(b'"') => {
                        let end = self.pos;
                        self.pos += 1;
                        break end;
                    }
                    Some(b'\r') | Some(b'\n') => {
                        self.eat_newline();
                    }
                    Some(_) => self.pos += 1,
                }
            };
            while self.pos < b.len() && is_space(b[self.pos]) {
                self.pos += 1;
            }
            match b.get(self.pos) {
                None | Some(b'\r') | Some(b'\n') => {}
                Some(&c) if c == d => {}
                Some(_) => {
                    let (l, ls, p) = (self.line, self.line_start, self.pos);
                    return Err(self.err(l, ls, p, "unexpected text after a closing quote (quotes inside a quoted field are written \"\")"));
                }
            }
            let raw = &self.src[start..end];
            let text = if escaped { Cow::Owned(raw.replace("\"\"", "\"")) } else { Cow::Borrowed(raw) };
            return Ok(Field { text, quoted: true, line, pos: open, line_start });
        }
        let start = self.pos;
        while self.pos < b.len() && !matches!(b[self.pos], b'\r' | b'\n') && b[self.pos] != d {
            self.pos += 1;
        }
        let raw = &self.src[start..self.pos];
        let text = if self.trim { raw.trim() } else { raw };
        Ok(Field { text: Cow::Borrowed(text), quoted: false, line: self.line, pos: start, line_start: self.line_start })
    }
}

fn header_names(h: &Record) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(h.fields.len());
    for (i, f) in h.fields.iter().enumerate() {
        let base = match f.text.trim() {
            "" => format!("column_{}", i + 1),
            t => t.to_string(),
        };
        let mut name = base.clone();
        let mut k = 2;
        while names.contains(&name) {
            name = format!("{base}_{k}");
            k += 1;
        }
        names.push(name);
    }
    names
}

struct Cx<'a> {
    src: &'a str,
    decimal_comma: bool,
    opts: &'a CsvOptions,
}

impl Cx<'_> {
    /// Empty, or a missing-value marker (only for typed columns).
    fn is_missing(&self, t: &str) -> bool {
        let t = t.trim();
        t.is_empty() || self.opts.null_values.iter().any(|n| n == t)
    }

    fn infer(&self, cells: &[Option<&Field>]) -> Column {
        let present: Vec<&str> = cells.iter().flatten().map(|f| f.text.as_ref()).filter(|t| !self.is_missing(t)).collect();
        let any_missing = present.len() < cells.len();
        let texts = || cells.iter().map(|c| c.map(|f| f.text.as_ref()));
        if present.is_empty() {
            return self.strings(cells);
        }
        if !any_missing && present.iter().all(|t| parse_bool(t).is_some()) {
            return Column::Bool(texts().map(|t| t.and_then(parse_bool).unwrap_or(false)).collect());
        }
        let is_code = |t: &str| {
            let t = t.trim().trim_start_matches(['+', '-']);
            t.len() > 1 && t.starts_with('0') && t.as_bytes()[1].is_ascii_digit()
        };
        if present.iter().all(|t| !is_code(t) && parse_number(t, self.decimal_comma).is_some()) {
            if self.opts.years_as_dates && present.iter().all(|t| t.trim().len() == 4 && t.trim().bytes().all(|c| c.is_ascii_digit())) {
                return Column::Date(texts().map(|t| t.and_then(|t| date::parse_date_with(t, true))).collect());
            }
            return Column::Num(texts().map(|t| t.and_then(|t| parse_number(t, self.decimal_comma)).unwrap_or(f64::NAN)).collect());
        }
        if present.iter().all(|t| date::parse_date(t).is_some()) {
            return Column::Date(texts().map(|t| t.and_then(date::parse_date)).collect());
        }
        self.strings(cells)
    }

    /// Text as written (empty → null), with repeated strings sharing one allocation.
    fn strings(&self, cells: &[Option<&Field>]) -> Column {
        let mut pool: BTreeMap<&str, Arc<str>> = BTreeMap::new();
        Column::Str(
            cells
                .iter()
                .map(|c| match c {
                    Some(f) if !f.text.is_empty() => Some(pool.entry(f.text.as_ref()).or_insert_with(|| Arc::from(f.text.as_ref())).clone()),
                    _ => None,
                })
                .collect(),
        )
    }

    fn convert(&self, name: &str, ty: ColumnType, cells: &[Option<&Field>]) -> Result<Column, DataError> {
        if ty == ColumnType::Str {
            return Ok(self.strings(cells));
        }
        let bad = |f: &Field, what: &str| DataError::Parse {
            line: f.line,
            column: column_of(self.src, f.line_start, f.pos),
            message: format!("column \"{name}\": \"{}\" is not {what}", f.text),
        };
        let mut nums = Vec::new();
        let mut dates = Vec::new();
        let mut bools = Vec::new();
        for c in cells {
            let f = match c {
                Some(f) if !self.is_missing(&f.text) => f,
                _ => {
                    nums.push(f64::NAN);
                    dates.push(None);
                    bools.push(false);
                    continue;
                }
            };
            match ty {
                ColumnType::Num => nums.push(parse_number(&f.text, self.decimal_comma).ok_or_else(|| bad(f, "a number"))?),
                ColumnType::Date => dates.push(Some(date::parse_date_lenient(&f.text).ok_or_else(|| bad(f, "a date (YYYY-MM-DD, YYYY-MM or YYYY)"))?)),
                ColumnType::Bool => bools.push(parse_bool_lenient(&f.text).ok_or_else(|| bad(f, "true or false"))?),
                ColumnType::Str => unreachable!(),
            }
        }
        Ok(match ty {
            ColumnType::Num => Column::Num(nums),
            ColumnType::Date => Column::Date(dates),
            _ => Column::Bool(bools),
        })
    }
}

fn parse_bool(t: &str) -> Option<bool> {
    let t = t.trim();
    if t.eq_ignore_ascii_case("true") {
        Some(true)
    } else if t.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_bool_lenient(t: &str) -> Option<bool> {
    parse_bool(t).or(match t.trim().to_ascii_lowercase().as_str() {
        "1" | "yes" | "y" => Some(true),
        "0" | "no" | "n" => Some(false),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(parse_number("-12.5e-1", false), Some(-1.25));
        assert_eq!(parse_number(".5", false), Some(0.5));
        assert_eq!(parse_number("+3", false), Some(3.0));
        assert_eq!(parse_number("2,5", false), None);
        assert_eq!(parse_number("2,75", true), Some(2.75));
        assert_eq!(parse_number("1 234,5", true), Some(1234.5));
        assert_eq!(parse_number("1.234,5", true), Some(1234.5));
        assert_eq!(parse_number("1,2,3", true), None);
        for bad in ["", "-", ".", "e5", "1e", "inf", "NaN", "1_000", "12a", "0x10"] {
            assert_eq!(parse_number(bad, true), None, "{bad}");
        }
    }

    #[test]
    fn delimiter_detection() {
        assert_eq!(detect_delimiter("a,b,c\n1,2,3\n"), b',');
        assert_eq!(detect_delimiter("a;b\n1,5;2,5\n"), b';');
        assert_eq!(detect_delimiter("a\tb\nx, y\t1\n"), b'\t');
        assert_eq!(detect_delimiter("name,value\n\"x;y\",1\n"), b',');
        assert_eq!(detect_delimiter("single\n1\n"), b',');
    }

    #[test]
    fn windows_1252_fallback() {
        let t = read_csv(b"stad,v\nG\xe4vle,1\n\x80,2\n", &CsvOptions::default()).unwrap();
        assert_eq!(t.str("stad").unwrap()[0].as_deref(), Some("Gävle"));
        assert_eq!(t.str("stad").unwrap()[1].as_deref(), Some("€"));
        let e = read_csv(b"a\nG\xe4vle\n", &CsvOptions::default().encoding(Encoding::Utf8)).unwrap_err();
        assert_eq!(e, DataError::Parse { line: 2, column: 2, message: "invalid UTF-8 (is the file Windows-1252 / Latin-1?)".into() });
    }
}
