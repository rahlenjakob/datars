//! Number and date formatting for labels and animated numbers.
//!
//! [`number`] implements d3-format's specifier language
//! (`[[fill]align][sign][symbol][0][width][,][.precision][~][type]`) with d3's semantics, including
//! JavaScript's rounding (`toFixed`/`toExponential` round exact halves away from zero, so
//! `number(2.5, ".0f", "en")` is `"3"`, as in d3). Separators, currency, percent sign and minus sign
//! come from the [`locale`](crate::locale). [`date`] implements strftime-style directives over a
//! day count, with locale month and weekday names.
//!
//! Both are total: unknown specifiers fall back to defaults, NaN renders as `"–"` and infinities
//! as `"∞"` / `"−∞"`. Everything is integer or exactly rounded decimal arithmetic, so output is
//! identical on every target (P1).

use crate::locale::{self, Locale};

/// Format `v` with a d3-format specifier (`",.2f"`, `".0%"`, `"$,.2s"`, `"+.1f"`, `"~s"`, …) in
/// `locale` (a BCP 47 tag; unknown tags use English).
pub fn number(v: f64, spec: &str, locale: &str) -> String {
    NumberFormat::new(spec, locale).format(v)
}

/// Format a date given as days since 1970-01-01 with strftime-style directives in `locale`.
///
/// Supported: `%Y %y %m %d %e %j %b %h %B %a %A %q %u %w %U %W %V %G %H %M %S %%`, with the
/// padding modifiers `-` (none), `_` (space) and `0` (zero), e.g. `%-d`. `%q` is the quarter
/// (1–4); `%V`/`%G` are the ISO week and week-based year. Time directives read midnight.
pub fn date(days_since_epoch: i64, spec: &str, locale: &str) -> String {
    format_date(days_since_epoch, spec, locale::get(locale))
}

/// A parsed d3-format specifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub fill: char,
    /// One of `<`, `>`, `^`, `=`.
    pub align: char,
    /// One of `-`, `+`, `(`, ` `.
    pub sign: char,
    /// `$` (currency) or `#` (radix prefix), if present.
    pub symbol: Option<char>,
    pub zero: bool,
    pub width: usize,
    pub comma: bool,
    pub precision: Option<usize>,
    pub trim: bool,
    /// The format type (`f`, `e`, `g`, `r`, `s`, `%`, `p`, `d`, `b`, `o`, `x`, `X`, `c`, `n`), or
    /// `None` for the default (d3's `""`: shortest up to 12 significant digits).
    pub kind: Option<char>,
}

impl Default for Spec {
    fn default() -> Self {
        Spec { fill: ' ', align: '>', sign: '-', symbol: None, zero: false, width: 0, comma: false, precision: None, trim: false, kind: None }
    }
}

impl Spec {
    /// Parse a specifier. Malformed specifiers (d3 would throw) yield the default spec.
    pub fn parse(s: &str) -> Spec {
        Spec::try_parse(s).unwrap_or_default()
    }

    fn try_parse(s: &str) -> Option<Spec> {
        let c: Vec<char> = s.chars().collect();
        let n = c.len();
        let mut i = 0;
        let mut spec = Spec::default();
        let is_align = |ch: char| matches!(ch, '<' | '>' | '=' | '^');
        if n >= 2 && is_align(c[1]) {
            spec.fill = c[0];
            spec.align = c[1];
            i = 2;
        } else if n >= 1 && is_align(c[0]) {
            spec.align = c[0];
            i = 1;
        }
        if i < n && matches!(c[i], '+' | '-' | '(' | ' ') {
            spec.sign = c[i];
            i += 1;
        }
        if i < n && matches!(c[i], '$' | '#') {
            spec.symbol = Some(c[i]);
            i += 1;
        }
        if i < n && c[i] == '0' {
            spec.zero = true;
            i += 1;
        }
        let w0 = i;
        while i < n && c[i].is_ascii_digit() {
            i += 1;
        }
        if i > w0 {
            spec.width = c[w0..i].iter().collect::<String>().parse().ok()?;
        }
        if i < n && c[i] == ',' {
            spec.comma = true;
            i += 1;
        }
        if i < n && c[i] == '.' {
            i += 1;
            let p0 = i;
            while i < n && c[i].is_ascii_digit() {
                i += 1;
            }
            if i == p0 {
                return None;
            }
            spec.precision = Some(c[p0..i].iter().collect::<String>().parse().ok()?);
        }
        if i < n && c[i] == '~' {
            spec.trim = true;
            i += 1;
        }
        if i < n && (c[i].is_ascii_alphabetic() || c[i] == '%') {
            spec.kind = Some(c[i]);
            i += 1;
        }
        (i == n).then_some(spec)
    }
}

/// A specifier compiled against a locale, for formatting many values (scale labels, animation frames).
#[derive(Clone, Debug)]
pub struct NumberFormat {
    spec: Spec,
    kind: char,
    precision: usize,
    prefix: String,
    suffix: String,
    locale: &'static Locale,
}

const SI_PREFIXES: [&str; 17] = ["y", "z", "a", "f", "p", "n", "\u{b5}", "m", "", "k", "M", "G", "T", "P", "E", "Z", "Y"];

impl NumberFormat {
    pub fn new(spec: &str, locale: &str) -> NumberFormat {
        NumberFormat::with_locale(Spec::parse(spec), locale::get(locale))
    }

    pub fn with_locale(mut spec: Spec, locale: &'static Locale) -> NumberFormat {
        let mut kind = spec.kind.unwrap_or('\0');
        if kind == 'n' {
            spec.comma = true;
            kind = 'g';
        } else if kind == '\0' || !"efgrs%pdboxXc".contains(kind) {
            // d3: the "" type (and any unknown type) is ".12~g".
            if spec.precision.is_none() {
                spec.precision = Some(12);
            }
            spec.trim = true;
            kind = 'g';
        }
        if spec.zero || (spec.fill == '0' && spec.align == '=') {
            spec.zero = true;
            spec.fill = '0';
            spec.align = '=';
        }
        let prefix = match spec.symbol {
            Some('$') => locale.currency_prefix.to_string(),
            Some('#') if "boxX".contains(kind) => format!("0{}", kind.to_ascii_lowercase()),
            _ => String::new(),
        };
        let suffix = match spec.symbol {
            Some('$') => locale.currency_suffix.to_string(),
            _ if kind == '%' || kind == 'p' => locale.percent.to_string(),
            _ => String::new(),
        };
        let precision = match spec.precision {
            None => 6,
            Some(p) if "gprs".contains(kind) => p.clamp(1, 21),
            Some(p) => p.min(20),
        };
        NumberFormat { spec, kind, precision, prefix, suffix, locale }
    }

    pub fn format(&self, v: f64) -> String {
        let loc = self.locale;
        if v.is_nan() {
            return "\u{2013}".to_string();
        }
        if v.is_infinite() {
            return if v < 0.0 { format!("{}\u{221e}", loc.minus) } else { "\u{221e}".to_string() };
        }
        let spec = &self.spec;
        let kind = self.kind;
        let mut value_prefix = self.prefix.clone();
        let mut value_suffix = self.suffix.clone();
        let mut value;
        if kind == 'c' {
            value_suffix = js_to_string(v) + &value_suffix;
            value = String::new();
        } else {
            let mut negative = v < 0.0 || (v == 0.0 && v.is_sign_negative());
            let (formatted, si) = format_type(kind, v.abs(), self.precision);
            value = formatted;
            if spec.trim {
                value = trim_zeros(&value);
            }
            // A negative value that rounds to zero loses its sign unless "+" was asked for.
            if negative && is_zero_str(&value) && spec.sign != '+' {
                negative = false;
            }
            let sign = if negative {
                if spec.sign == '(' {
                    "(".to_string()
                } else {
                    loc.minus.to_string()
                }
            } else if spec.sign == '-' || spec.sign == '(' {
                String::new()
            } else {
                spec.sign.to_string()
            };
            value_prefix = sign + &value_prefix;
            let si = if kind == 's' { SI_PREFIXES[(8 + si) as usize] } else { "" };
            value_suffix = format!("{si}{value_suffix}{}", if negative && spec.sign == '(' { ")" } else { "" });
            // Split the groupable integer digits from the fraction/exponent.
            if "defgprs%".contains(kind) {
                if let Some(i) = value.find(|ch: char| !ch.is_ascii_digit()) {
                    let rest = if value[i..].starts_with('.') { format!("{}{}", loc.decimal, &value[i + 1..]) } else { value[i..].to_string() };
                    value_suffix = rest + &value_suffix;
                    value.truncate(i);
                }
            }
        }
        if spec.comma && !spec.zero {
            value = group(&value, usize::MAX, loc);
        }
        let length = value_prefix.chars().count() + value.chars().count() + value_suffix.chars().count();
        let mut padding: String = if length < spec.width { std::iter::repeat_n(spec.fill, spec.width - length).collect() } else { String::new() };
        if spec.comma && spec.zero {
            let limit = if padding.is_empty() { usize::MAX } else { spec.width.saturating_sub(value_suffix.chars().count()) };
            value = group(&(padding.clone() + &value), limit, loc);
            padding.clear();
        }
        match spec.align {
            '<' => format!("{value_prefix}{value}{value_suffix}{padding}"),
            '=' => format!("{value_prefix}{padding}{value}{value_suffix}"),
            '^' => {
                let pc: Vec<char> = padding.chars().collect();
                let half = pc.len() / 2;
                let (l, r): (String, String) = (pc[..half].iter().collect(), pc[half..].iter().collect());
                format!("{l}{value_prefix}{value}{value_suffix}{r}")
            }
            _ => format!("{padding}{value_prefix}{value}{value_suffix}"),
        }
    }
}

/// The digits of `|v|` for a format type; the second value is the SI exponent (÷3) for `s`.
fn format_type(kind: char, x: f64, p: usize) -> (String, i32) {
    match kind {
        'f' => (to_fixed(x, p), 0),
        '%' => (to_fixed(x * 100.0, p), 0),
        'e' => (to_exponential(x, p), 0),
        'g' => (to_precision(x, p), 0),
        'r' => (format_rounded(x, p), 0),
        'p' => (format_rounded(x * 100.0, p), 0),
        's' => format_prefix_auto(x, p),
        'd' => (format!("{:.0}", x.round()), 0),
        'b' => (radix(x, 2, false), 0),
        'o' => (radix(x, 8, false), 0),
        'x' => (radix(x, 16, false), 0),
        'X' => (radix(x, 16, true), 0),
        _ => (to_precision(x, p), 0),
    }
}

fn radix(x: f64, base: u32, upper: bool) -> String {
    let r = x.round();
    if r >= 3.4e38 {
        return js_to_string(r);
    }
    let mut n = r as u128;
    if n == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while n > 0 {
        let d = (n % base as u128) as u32;
        let ch = std::char::from_digit(d, base).unwrap_or('0');
        digits.push(if upper { ch.to_ascii_uppercase() } else { ch });
        n /= base as u128;
    }
    digits.iter().rev().collect()
}

/// `x = m · 2^e` with `m` odd (`m = 0` for zero). Exact.
fn dyadic(x: f64) -> (u64, i32) {
    let bits = x.to_bits();
    let exp_bits = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (m, e) = if exp_bits == 0 { (frac, -1074) } else { (frac | (1u64 << 52), exp_bits - 1075) };
    if m == 0 {
        return (0, 0);
    }
    let tz = m.trailing_zeros();
    (m >> tz, e + tz as i32)
}

/// Whether `x · 10^q` is exactly `k + ½`, i.e. rounding `x` at 10^-q is an exact tie. With
/// `x = m·2^e` (m odd): for q ≥ 0, `m·5^q·2^(e+q)` is half an odd number iff `e + q = −1`; for
/// q < 0 (s = −q), `m·2^(e−s)/5^s` is iff `5^s | m` and `e − s = −1`.
fn is_tie(x: f64, q: i32) -> bool {
    let (m, e) = dyadic(x);
    if m == 0 {
        return false;
    }
    if q >= 0 {
        e + q == -1
    } else {
        let s = -q;
        // 5^23 > 2^53 > m, so larger s can't divide.
        s < 23 && e - s == -1 && m % 5u64.pow(s as u32) == 0
    }
}

/// The next double above a positive finite `x` (rounds an exact tie upward).
fn next_up(x: f64) -> f64 {
    let y = f64::from_bits(x.to_bits() + 1);
    if y.is_finite() {
        y
    } else {
        x
    }
}

/// JavaScript `Number.prototype.toFixed` for finite `x ≥ 0`: exact decimal rounding with halves
/// away from zero (Rust's formatter is exact but rounds halves to even).
fn to_fixed(x: f64, p: usize) -> String {
    if x >= 1e21 {
        return js_to_string(x);
    }
    let x = if is_tie(x, p as i32) { next_up(x) } else { x };
    format!("{x:.p$}")
}

/// Decimal exponent of finite `x > 0`, i.e. ⌊log10 x⌋. 26 significant digits keep this exact for
/// every double that could be a rounding tie at ≤ 21 digits.
fn decimal_exponent(x: f64) -> i32 {
    let s = format!("{x:.25e}");
    s.rsplit('e').next().and_then(|e| e.parse().ok()).unwrap_or(0)
}

/// Rust's `1.5e-7` exponent style → JavaScript's `1.5e-7` / `1.5e+7`.
fn js_exponent(s: String) -> String {
    match s.find('e') {
        Some(i) if !s[i + 1..].starts_with('-') => format!("{}e+{}", &s[..i], &s[i + 1..]),
        _ => s,
    }
}

/// JavaScript `toExponential(p)` for finite `x ≥ 0`.
fn to_exponential(x: f64, p: usize) -> String {
    if x == 0.0 {
        return if p == 0 { "0e+0".to_string() } else { format!("0.{}e+0", "0".repeat(p)) };
    }
    let e = decimal_exponent(x);
    let x = if is_tie(x, p as i32 - e) { next_up(x) } else { x };
    js_exponent(format!("{x:.p$e}"))
}

/// JavaScript `toExponential()` (shortest round-trip digits) for finite `x ≥ 0`.
fn to_exponential_shortest(x: f64) -> String {
    js_exponent(format!("{x:e}"))
}

/// Coefficient digits (no point) and decimal exponent of `x` rounded to `p` significant digits
/// (`p = 0`: shortest round-trip), as d3's `formatDecimalParts`.
fn decimal_parts(x: f64, p: usize) -> (String, i32) {
    let s = if p == 0 { to_exponential_shortest(x) } else { to_exponential(x, p - 1) };
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    (mant.replace('.', ""), exp.trim_start_matches('+').parse().unwrap_or(0))
}

/// JavaScript `toPrecision(p)` for finite `x ≥ 0`, `1 ≤ p ≤ 21`.
fn to_precision(x: f64, p: usize) -> String {
    if x == 0.0 {
        return if p <= 1 { "0".to_string() } else { format!("0.{}", "0".repeat(p - 1)) };
    }
    let (digits, e) = decimal_parts(x, p);
    if e < -6 || e >= p as i32 {
        return to_exponential(x, p - 1);
    }
    if e >= 0 {
        let e = e as usize;
        if digits.len() > e + 1 {
            format!("{}.{}", &digits[..e + 1], &digits[e + 1..])
        } else {
            digits
        }
    } else {
        format!("0.{}{}", "0".repeat((-e - 1) as usize), digits)
    }
}

/// d3's `formatRounded` (type `r`): `p` significant digits in fixed notation.
fn format_rounded(x: f64, p: usize) -> String {
    let (c, e) = decimal_parts(x, p);
    if e < 0 {
        format!("0.{}{}", "0".repeat((-e - 1) as usize), c)
    } else if c.len() > e as usize + 1 {
        format!("{}.{}", &c[..e as usize + 1], &c[e as usize + 1..])
    } else {
        format!("{}{}", c, "0".repeat(e as usize + 1 - c.len()))
    }
}

/// d3's `formatPrefixAuto` (type `s`): `p` significant digits scaled to an SI prefix.
fn format_prefix_auto(x: f64, p: usize) -> (String, i32) {
    let (c, e) = decimal_parts(x, p);
    let pe = e.div_euclid(3).clamp(-8, 8);
    let i = e - pe * 3 + 1;
    let n = c.len() as i32;
    let s = if i == n {
        c
    } else if i > n {
        format!("{}{}", c, "0".repeat((i - n) as usize))
    } else if i > 0 {
        format!("{}.{}", &c[..i as usize], &c[i as usize..])
    } else {
        // Smaller than the smallest prefix (1e-24).
        let q = (p as i32 + i - 1).max(0) as usize;
        format!("0.{}{}", "0".repeat((-i) as usize), decimal_parts(x, q).0)
    };
    (s, pe)
}

/// JavaScript `Number.prototype.toString()` for finite values (shortest round-trip digits; fixed
/// notation for 1e-7 ≤ |x| < 1e21, exponential otherwise).
fn js_to_string(x: f64) -> String {
    if x == 0.0 {
        return "0".to_string();
    }
    let neg = x < 0.0;
    let (digits, e) = decimal_parts(x.abs(), 0);
    let k = digits.len() as i32;
    let body = if (-6..21).contains(&e) {
        if e >= k - 1 {
            format!("{}{}", digits, "0".repeat((e - k + 1) as usize))
        } else if e >= 0 {
            format!("{}.{}", &digits[..(e + 1) as usize], &digits[(e + 1) as usize..])
        } else {
            format!("0.{}{}", "0".repeat((-e - 1) as usize), digits)
        }
    } else {
        let mant = if k > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.clone() };
        format!("{}e{}{}", mant, if e >= 0 { "+" } else { "-" }, e.abs())
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

/// d3's `formatTrim`: drop insignificant trailing zeros of the first decimal fraction
/// (`1.2000k` → `1.2k`, `1.0` → `1`).
fn trim_zeros(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let (mut i0, mut i1): (isize, isize) = (-1, -1);
    let mut i = 1;
    while i < n {
        match c[i] {
            '.' => {
                i0 = i as isize;
                i1 = i as isize;
            }
            '0' => {
                if i0 == 0 {
                    i0 = i as isize;
                }
                i1 = i as isize;
            }
            ch => {
                if !ch.is_ascii_digit() || ch == '0' {
                    break;
                }
                if i0 > 0 {
                    i0 = 0;
                }
            }
        }
        i += 1;
    }
    if i0 > 0 {
        let head: String = c[..i0 as usize].iter().collect();
        let tail: String = c[(i1 + 1) as usize..].iter().collect();
        head + &tail
    } else {
        s.to_string()
    }
}

/// Whether a formatted magnitude reads as zero (JavaScript `+value === 0`).
fn is_zero_str(s: &str) -> bool {
    let mant = s.split(['e', 'E']).next().unwrap_or("");
    mant.chars().any(|c| c.is_ascii_digit()) && mant.chars().all(|c| c == '0' || c == '.')
}

/// d3's locale `group`: insert the locale separator per `grouping`, keeping at most `width` chars.
fn group(value: &str, width: usize, loc: &Locale) -> String {
    let c: Vec<char> = value.chars().collect();
    let grouping = if loc.grouping.is_empty() { &[3][..] } else { loc.grouping };
    let mut i = c.len();
    let mut parts: Vec<String> = Vec::new();
    let mut j = 0;
    let mut g = grouping[0];
    let mut length = 0usize;
    while i > 0 && g > 0 {
        if length.saturating_add(g + 1) > width {
            g = width.saturating_sub(length).max(1);
        }
        let start = i.saturating_sub(g);
        parts.push(c[start..i].iter().collect());
        i = start;
        length = length.saturating_add(g + 1);
        if length > width {
            break;
        }
        j = (j + 1) % grouping.len();
        g = grouping[j];
    }
    parts.reverse();
    parts.join(loc.group)
}

// ---- dates ------------------------------------------------------------------------------------

/// Civil date (proleptic Gregorian) from days since 1970-01-01 (H. Hinnant's algorithm).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days.clamp(-(1 << 40), 1 << 40) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m as u32, d as u32)
}

/// Days since 1970-01-01 of a civil date (inverse of [`civil_from_days`]).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let (m, d) = (m as i64, d as i64);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn pad(v: i64, width: usize, pad: Option<char>) -> String {
    let digits = v.unsigned_abs().to_string();
    let sign = if v < 0 { "-" } else { "" };
    match pad {
        Some(ch) if digits.len() < width => format!("{sign}{}{digits}", std::iter::repeat_n(ch, width - digits.len()).collect::<String>()),
        _ => format!("{sign}{digits}"),
    }
}

fn format_date(days: i64, spec: &str, loc: &Locale) -> String {
    let days = days.clamp(-(1 << 40), 1 << 40);
    let (y, m, d) = civil_from_days(days);
    let weekday = (days + 4).rem_euclid(7); // 0 = Sunday (1970-01-01 was a Thursday)
    let doy0 = days - days_from_civil(y, 1, 1); // 0-based day of year
    let mut out = String::with_capacity(spec.len() + 8);
    let mut it = spec.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        // Padding modifier: '-' none, '_' space, '0' zero; `None` = the directive's default.
        let modifier: Option<Option<char>> = match it.peek() {
            Some('-') => Some(None),
            Some('_') => Some(Some(' ')),
            Some('0') => Some(Some('0')),
            _ => None,
        };
        if modifier.is_some() {
            it.next();
        }
        let p = |default: char| modifier.unwrap_or(Some(default));
        let Some(dir) = it.next() else {
            out.push('%');
            break;
        };
        let s = match dir {
            'Y' => pad(y, 4, p('0')),
            'y' => pad(y.rem_euclid(100), 2, p('0')),
            'm' => pad(m as i64, 2, p('0')),
            'd' => pad(d as i64, 2, p('0')),
            'e' => pad(d as i64, 2, p(' ')),
            'j' => pad(doy0 + 1, 3, p('0')),
            'b' | 'h' => loc.months_short[m as usize - 1].to_string(),
            'B' => loc.months[m as usize - 1].to_string(),
            'a' => loc.days_short[weekday as usize].to_string(),
            'A' => loc.days[weekday as usize].to_string(),
            'q' => ((m - 1) / 3 + 1).to_string(),
            'u' => (if weekday == 0 { 7 } else { weekday }).to_string(),
            'w' => weekday.to_string(),
            'U' => pad((doy0 + 7 - weekday) / 7, 2, p('0')),
            'W' => pad((doy0 + 7 - (weekday + 6) % 7) / 7, 2, p('0')),
            'V' => pad(iso_week(days).1, 2, p('0')),
            'G' => pad(iso_week(days).0, 4, p('0')),
            'H' | 'M' | 'S' => pad(0, 2, p('0')),
            '%' => "%".to_string(),
            other => {
                out.push('%');
                if let Some(Some(c)) = modifier {
                    out.push(if c == ' ' { '_' } else { c });
                } else if modifier == Some(None) {
                    out.push('-');
                }
                out.push(other);
                continue;
            }
        };
        out.push_str(&s);
    }
    out
}

/// ISO 8601 week-based year and week number (weeks start Monday; week 1 holds the first Thursday).
fn iso_week(days: i64) -> (i64, i64) {
    let monday0 = (days + 3).rem_euclid(7); // 0 = Monday
    let thursday = days - monday0 + 3;
    let (ty, _, _) = civil_from_days(thursday);
    let week = (thursday - days_from_civil(ty, 1, 1)) / 7 + 1;
    (ty, week)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f64, spec: &str) -> String {
        number(v, spec, "en")
    }

    #[test]
    fn spec_parsing() {
        let s = Spec::parse("*^+$012,.3~f");
        assert_eq!(s.fill, '*');
        assert_eq!(s.align, '^');
        assert_eq!(s.sign, '+');
        assert_eq!(s.symbol, Some('$'));
        assert!(s.zero && s.comma && s.trim);
        assert_eq!(s.width, 12);
        assert_eq!(s.precision, Some(3));
        assert_eq!(s.kind, Some('f'));
        assert_eq!(Spec::parse("garbage!"), Spec::default());
        assert_eq!(Spec::parse(".f"), Spec::default());
    }

    #[test]
    fn js_rounding_primitives() {
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.5, 0), "1");
        assert_eq!(to_fixed(1.25, 1), "1.3");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(1.005, 2), "1.00", "1.005 is really 1.00499…");
        assert_eq!(to_fixed(1e21, 2), "1e+21");
        assert_eq!(to_exponential(12345.0, 2), "1.23e+4");
        assert_eq!(to_exponential(0.00015, 0), "1e-4", "0.00015 is really 0.000149999…");
        assert_eq!(to_exponential(25.0, 0), "3e+1");
        assert_eq!(to_exponential(0.0, 2), "0.00e+0");
        assert_eq!(to_precision(123.456, 4), "123.5");
        assert_eq!(to_precision(0.000123, 2), "0.00012");
        assert_eq!(to_precision(1e-7, 2), "1.0e-7");
        assert_eq!(to_precision(123456.0, 2), "1.2e+5");
        assert_eq!(js_to_string(1e21), "1e+21");
        assert_eq!(js_to_string(123.25), "123.25");
        assert_eq!(js_to_string(1e-7), "1e-7");
        assert_eq!(js_to_string(0.000001), "0.000001");
        assert!(is_tie(2.5, 0) && !is_tie(2.4, 0) && is_tie(1.25, 1));
        assert!(is_tie(25.0, -1) && !is_tie(250.0, -1) && !is_tie(35.0, -2));
    }

    #[test]
    fn trim() {
        assert_eq!(trim_zeros("1.2000"), "1.2");
        assert_eq!(trim_zeros("1.000"), "1");
        assert_eq!(trim_zeros("100"), "100");
        assert_eq!(trim_zeros("1.50e+3"), "1.5e+3");
    }

    #[test]
    fn required_examples() {
        assert_eq!(f(1234567.891, ",.2f"), "1,234,567.89");
        assert_eq!(number(1234567.891, ",.2f", "sv"), "1\u{a0}234\u{a0}567,89");
        assert_eq!(f(0.123, ".0%"), "12%");
        assert_eq!(f(1_200_000.0, "~s"), "1.2M");
        assert_eq!(f(1200.0, "~s"), "1.2k");
        assert_eq!(f(1234.5, "$,.0f"), "$1,235");
        assert_eq!(f(1234.56, "$,.0f"), "$1,235");
    }

    #[test]
    fn d3_parity_fixed_and_grouping() {
        assert_eq!(f(0.0, ",.2f"), "0.00");
        assert_eq!(f(-1234.5, ",.1f"), "\u{2212}1,234.5");
        assert_eq!(f(-0.0001, ".2f"), "0.00", "negative rounding to zero drops the sign");
        assert_eq!(f(-0.0001, "+.2f"), "\u{2212}0.00");
        assert_eq!(f(12.0, "+.1f"), "+12.0");
        assert_eq!(f(12.0, " .1f"), " 12.0");
        assert_eq!(f(-12.0, "(.1f"), "(12.0)");
        assert_eq!(f(1234.0, ","), "1,234");
        assert_eq!(f(1234.5678, ""), "1234.5678");
        assert_eq!(f(0.1 + 0.2, ""), "0.3");
        assert_eq!(f(42.0, "d"), "42");
        assert_eq!(f(42.5, "d"), "43");
        assert_eq!(f(1234567.0, ",d"), "1,234,567");
        assert_eq!(f(1e6, ".2e"), "1.00e+6");
        assert_eq!(f(0.000123, ".2~e"), "1.23e-4");
        assert_eq!(f(123.456, ".2r"), "120");
        assert_eq!(f(0.012344, ".3r"), "0.0123");
        assert_eq!(f(1234.0, ".3g"), "1.23e+3");
        assert_eq!(f(0.25, ".1p"), "30%");
        assert_eq!(f(255.0, "x"), "ff");
        assert_eq!(f(255.0, "#X"), "0xFF");
        assert_eq!(f(5.0, "b"), "101");
        assert_eq!(f(1234.0, "n"), "1,234.00", "d3: n is ,g with precision 6");
    }

    #[test]
    fn d3_parity_si() {
        assert_eq!(f(1234567.0, "s"), "1.23457M");
        assert_eq!(f(1234567.0, ".2s"), "1.2M");
        assert_eq!(f(0.00123, ".2s"), "1.2m");
        assert_eq!(f(0.000001, "~s"), "1\u{b5}");
        assert_eq!(f(1500.0, "$.2s"), "$1.5k");
        assert_eq!(f(999.5, ".3s"), "1.00k");
        assert_eq!(f(0.0, "~s"), "0");
        assert_eq!(f(42.0, ".2~s"), "42");
    }

    #[test]
    fn d3_parity_padding() {
        assert_eq!(f(42.0, "08.2f"), "00042.00");
        assert_eq!(f(-42.0, "08.2f"), "\u{2212}0042.00");
        assert_eq!(f(1234.0, "010,d"), "00,001,234");
        assert_eq!(f(42.0, ">8d"), "      42");
        assert_eq!(f(42.0, "<8d"), "42      ");
        assert_eq!(f(42.0, "^8d"), "   42   ");
        assert_eq!(f(42.0, "*^7d"), "**42***");
        assert_eq!(f(-42.0, "=8d"), "\u{2212}     42");
    }

    #[test]
    fn locales() {
        assert_eq!(number(1234.5, "$,.2f", "sv"), "1\u{a0}234,50\u{a0}kr");
        assert_eq!(number(1234.5, "$,.2f", "de"), "1.234,50\u{a0}€");
        assert_eq!(number(0.5, ".0%", "fr"), "50\u{202f}%");
        assert_eq!(number(1234.5, ",.1f", "es"), "1.234,5");
        assert_eq!(number(1234.5, "$,.0f", "en-GB"), "£1,235");
        assert_eq!(number(1234.5, ",.1f", "xx"), "1,234.5");
    }

    #[test]
    fn non_finite() {
        assert_eq!(f(f64::NAN, ",.2f"), "\u{2013}");
        assert_eq!(f(f64::INFINITY, ",.2f"), "\u{221e}");
        assert_eq!(f(f64::NEG_INFINITY, "$,.2f"), "\u{2212}\u{221e}");
    }

    #[test]
    fn civil_round_trip() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(days_from_civil(2024, 3, 1), 19_783);
        for d in (-800_000..800_000).step_by(997) {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }

    #[test]
    fn dates() {
        let d = days_from_civil(2024, 3, 5); // a Tuesday
        assert_eq!(date(d, "%Y-%m-%d", "en"), "2024-03-05");
        assert_eq!(date(d, "%-d %b %Y", "en"), "5 Mar 2024");
        assert_eq!(date(d, "%A %e %B", "en"), "Tuesday  5 March");
        assert_eq!(date(d, "%a %-m/%-d/%y", "en"), "Tue 3/5/24");
        assert_eq!(date(d, "%j %q", "en"), "065 1");
        assert_eq!(date(d, "%-j", "en"), "65");
        assert_eq!(date(d, "%A %-d %B %Y", "sv"), "tisdag 5 mars 2024");
        assert_eq!(date(d, "%b", "de"), "Mrz");
        assert_eq!(date(d, "%A", "fr"), "mardi");
        assert_eq!(date(d, "%B", "fi"), "maaliskuu");
        assert_eq!(date(days_from_civil(2023, 12, 31), "%q", "en"), "4");
        assert_eq!(date(0, "%A %Y", "en"), "Thursday 1970");
        assert_eq!(date(d, "100%% %Q", "en"), "100% %Q");
        assert_eq!(date(d, "%u %w", "en"), "2 2");
        assert_eq!(date(days_from_civil(2024, 3, 3), "%u %w", "en"), "7 0");
    }

    #[test]
    fn iso_weeks() {
        // 2021-01-03 (Sunday) belongs to ISO week 53 of 2020; 2024-12-30 is week 1 of 2025.
        assert_eq!(date(days_from_civil(2021, 1, 3), "%G-W%V", "en"), "2020-W53");
        assert_eq!(date(days_from_civil(2021, 1, 4), "%G-W%V", "en"), "2021-W01");
        assert_eq!(date(days_from_civil(2024, 12, 30), "%G-W%V", "en"), "2025-W01");
        assert_eq!(date(days_from_civil(2024, 1, 7), "%U %W", "en"), "01 01");
    }
}
