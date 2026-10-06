//! `datars-theme` — themes as typed, layered, late-bound data (docs/18-themes.md).
//!
//! A theme is a set of **tokens** (colours, palettes, numbers, fonts, text) with light / dark /
//! high-contrast **modes**, **inheritance** (`extends`), **locks** (guardrails lower layers can't
//! override) and **checks** (contrast, palette distinguishability). Colours can be expressions over
//! other tokens (`mix($ink, $paper, 0.88)`), so a brand sets a few source tokens and the rest derives.
//! Scenes reference colours as [`Ink`]s (`"$accent"`), resolved at frame time — so switching theme or
//! mode, or a host app overriding tokens, needs no re-resolve, and theme changes can animate.
//!
//! The engine knows the mechanism; the vocabulary (which tokens exist) is defined by themes and the
//! standard library. `themes/neutral.json` is the built-in fallback.

mod expr;
mod ink;
mod resolve;
mod validate;

pub use expr::{parse_color_expr, CExpr};
pub use ink::Ink;
pub use resolve::{resolve, Diag, ResolvedTheme};
pub use validate::{validate, Finding, Severity};

use datars_color::Color;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

pub type Sym = Arc<str>;

/// Which variant of a theme is active. Hosts set it from platform preferences (signals
/// `color_scheme`, `contrast`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    Light,
    Dark,
    HighContrast,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Light => "light",
            Mode::Dark => "dark",
            Mode::HighContrast => "high-contrast",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaletteKind {
    #[default]
    Categorical,
    Sequential,
    Diverging,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PaletteDef {
    Colors { kind: PaletteKind, colors: Vec<CExpr> },
    /// Generated from a source colour (OKLCH): `{"generate": "categorical", "from": "$brand", "n": 8}`;
    /// diverging generators also take `to` (the high side).
    Generate { kind: PaletteKind, from: CExpr, to: Option<CExpr>, n: usize },
}

/// A font token: the family to draw with (then fallbacks), its weight and style — and, optionally,
/// where that face comes from, so the build step can acquire it and ship it with the chart
/// (docs/18-themes.md §Fonts). Text must measure and render the same everywhere (P1), so faces
/// always travel with the chart; system fonts are never used.
///
/// ```json
/// { "family": "Cambon", "weight": 700, "src": "fonts/Cambon-Bold.otf" }
/// { "google": "Source Serif 4", "weight": 600, "italic": true }
/// { "family": ["Brand Sans", "Inter"], "src": "https://cdn.example.com/BrandSans-Regular.ttf" }
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FontSpec {
    /// The family stack, first choice first (a string is a one-family stack).
    #[serde(default, deserialize_with = "one_or_many", skip_serializing_if = "Vec::is_empty")]
    pub family: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: u16,
    #[serde(default)]
    pub italic: bool,
    /// The face's file: a path relative to the document (like data URLs), an `https://` URL, or
    /// `datars:fonts/…` (the default fonts that ship with the datars toolchain and runtimes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
    /// A Google Fonts family: static instances downloaded by the build step (never at runtime),
    /// cached on disk and shipped in the bundle. Names the stack's first family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<String>,
}

fn default_weight() -> u16 {
    400
}

fn one_or_many<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

impl FontSpec {
    /// The family stack text is laid out with: the Google family first (when named), then
    /// `family`, without duplicates.
    pub fn stack(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for f in self.google.iter().chain(&self.family) {
            let f = f.trim();
            if !f.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(f)) {
                out.push(f.to_string());
            }
        }
        out
    }

    /// The family this token's source provides (the stack's first family).
    pub fn primary(&self) -> Option<String> {
        self.stack().into_iter().next()
    }

    /// Where the face comes from, as a URL the host (or the build step) resolves: `src` as
    /// written, or `google:<Family>:<weight>[italic]` (the css2 API's naming). `None` when the
    /// token names only families.
    pub fn source_url(&self) -> Option<String> {
        if let Some(s) = self.src.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            return Some(s.to_string());
        }
        let g = self.google.as_ref().map(|g| g.trim()).filter(|g| !g.is_empty())?;
        Some(format!("google:{g}:{}{}", self.weight, if self.italic { "italic" } else { "" }))
    }
}

/// A `google:<Family>:<weight>[italic]` font URL, parsed (see [`FontSpec::source_url`]).
pub fn parse_google_url(url: &str) -> Option<(String, u16, bool)> {
    let rest = url.strip_prefix("google:")?;
    let (family, style) = match rest.rsplit_once(':') {
        Some((f, s)) if s.chars().next().is_some_and(|c| c.is_ascii_digit()) || s == "italic" || s.is_empty() => (f, s),
        _ => (rest, ""),
    };
    let italic = style.ends_with("italic");
    let weight = style.trim_end_matches("italic").parse::<u16>().unwrap_or(400);
    let family = family.trim();
    (!family.is_empty()).then(|| (family.to_string(), weight.clamp(1, 1000), italic))
}

/// One token's authored value. Typed by shape in JSON: colour expressions are strings starting with
/// `#`, `$` or a colour function; numbers are numbers; palettes are arrays or `{colors}` /
/// `{generate}` objects; fonts are `{family}` or `{google}` objects; any other string is text.
#[derive(Clone, Debug, PartialEq)]
pub enum TokenValue {
    Color(CExpr),
    Palette(PaletteDef),
    Number(f64),
    Font(FontSpec),
    Text(String),
}

/// A declared check, evaluated by [`validate`] on the resolved theme.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Check {
    /// Text token `pair[0]` on background `pair[1]` must reach `min` (WCAG ratio).
    Contrast { contrast: [String; 2], min: f64 },
    /// Every pair of colours in a palette must differ by at least `min` ΔE (also under colour-vision
    /// deficiency simulations when `cvd` is set).
    /// Pairwise ΔE of a palette's colours (also under simulated colour-vision deficiencies with
    /// `cvd`), over its `first` colours if given: beyond ~8 categories no palette stays distinct,
    /// and later colours are overflow.
    Distinct { distinct: String, min: f64, #[serde(default)] cvd: bool, #[serde(default, skip_serializing_if = "Option::is_none")] first: Option<usize> },
    /// A palette's lightness must change monotonically.
    Monotone { monotone: String },
}

/// Which mode a theme's base tokens are written for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    #[default]
    Light,
    /// Dark-first (a brand that is dark everywhere): in light mode, every theme beneath it resolves
    /// as in dark mode, so the tokens it doesn't set — map roads, the sequential ramp — suit its
    /// dark paper instead of the parent's light one.
    Dark,
}

/// A theme as authored (a JSON file, or built by the SDK).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Theme {
    pub name: String,
    pub extends: Option<String>,
    pub scheme: Scheme,
    pub tokens: BTreeMap<String, TokenValue>,
    pub modes: BTreeMap<Mode, BTreeMap<String, TokenValue>>,
    pub locked: Vec<String>,
    pub checks: Vec<Check>,
}

impl Theme {
    pub fn from_json(s: &str) -> Result<Theme, String> {
        let raw: RawTheme = serde_json::from_str(s).map_err(|e| e.to_string())?;
        raw.into_theme()
    }

    /// The built-in fallback theme.
    pub fn neutral() -> Theme {
        Theme::from_json(include_str!("../themes/neutral.json")).expect("built-in neutral theme parses")
    }

    pub fn to_json(&self) -> String {
        let mut o = serde_json::Map::new();
        o.insert("name".into(), self.name.clone().into());
        if let Some(e) = &self.extends {
            o.insert("extends".into(), e.clone().into());
        }
        if self.scheme == Scheme::Dark {
            o.insert("scheme".into(), "dark".into());
        }
        o.insert("tokens".into(), tokens_json(&self.tokens));
        if !self.modes.is_empty() {
            let mut mm = serde_json::Map::new();
            for (m, t) in &self.modes {
                mm.insert(m.name().into(), tokens_json(t));
            }
            o.insert("modes".into(), mm.into());
        }
        if !self.locked.is_empty() {
            o.insert("locked".into(), self.locked.clone().into());
        }
        if !self.checks.is_empty() {
            o.insert("checks".into(), serde_json::to_value(&self.checks).unwrap_or_default());
        }
        serde_json::to_string_pretty(&serde_json::Value::Object(o)).unwrap_or_default()
    }
}

fn tokens_json(t: &BTreeMap<String, TokenValue>) -> serde_json::Value {
    let mut o = serde_json::Map::new();
    for (k, v) in t {
        o.insert(k.clone(), v.to_json());
    }
    o.into()
}

impl TokenValue {
    pub fn parse(v: &serde_json::Value) -> Result<TokenValue, String> {
        use serde_json::Value as J;
        Ok(match v {
            J::Number(n) => TokenValue::Number(n.as_f64().unwrap_or(0.0)),
            J::Bool(b) => TokenValue::Number(if *b { 1.0 } else { 0.0 }),
            J::String(s) => match parse_color_expr(s) {
                Some(e) => TokenValue::Color(e),
                None => TokenValue::Text(s.clone()),
            },
            J::Array(a) => TokenValue::Palette(PaletteDef::Colors {
                kind: PaletteKind::Categorical,
                colors: a.iter().map(color_of).collect::<Result<_, _>>()?,
            }),
            J::Object(o) => {
                if o.contains_key("family") || o.contains_key("google") {
                    let f: FontSpec = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
                    if f.stack().is_empty() {
                        return Err("a font token needs `family` (a name or a list) or `google` (a Google Fonts family)".into());
                    }
                    TokenValue::Font(f)
                } else if let Some(g) = o.get("generate") {
                    let kind: PaletteKind = serde_json::from_value(g.clone()).map_err(|e| e.to_string())?;
                    let from = color_of(o.get("from").ok_or("palette generator needs `from`")?)?;
                    let to = o.get("to").map(color_of).transpose()?;
                    let n = o.get("n").and_then(|n| n.as_u64()).unwrap_or(8) as usize;
                    TokenValue::Palette(PaletteDef::Generate { kind, from, to, n })
                } else if let Some(cs) = o.get("colors") {
                    let kind: PaletteKind = match o.get("kind") {
                        Some(k) => serde_json::from_value(k.clone()).map_err(|e| e.to_string())?,
                        None => PaletteKind::Categorical,
                    };
                    let colors = cs.as_array().ok_or("`colors` must be an array")?.iter().map(color_of).collect::<Result<_, _>>()?;
                    TokenValue::Palette(PaletteDef::Colors { kind, colors })
                } else {
                    return Err(format!("unrecognised token value: {v}"));
                }
            }
            J::Null => return Err("null token value".into()),
        })
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            TokenValue::Color(e) => e.to_string().into(),
            TokenValue::Number(n) => (*n).into(),
            TokenValue::Text(s) => s.clone().into(),
            TokenValue::Font(f) => serde_json::to_value(f).unwrap_or_default(),
            TokenValue::Palette(PaletteDef::Colors { kind, colors }) => serde_json::json!({
                "kind": kind, "colors": colors.iter().map(|c| c.to_string()).collect::<Vec<_>>()
            }),
            TokenValue::Palette(PaletteDef::Generate { kind, from, to, n }) => {
                let mut v = serde_json::json!({ "generate": kind, "from": from.to_string(), "n": n });
                if let Some(t) = to {
                    v["to"] = t.to_string().into();
                }
                v
            }
        }
    }
}

fn color_of(v: &serde_json::Value) -> Result<CExpr, String> {
    v.as_str().and_then(parse_color_expr).ok_or_else(|| format!("not a colour expression: {v}"))
}

#[derive(Deserialize)]
struct RawTheme {
    name: String,
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    scheme: Scheme,
    #[serde(default)]
    tokens: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    modes: BTreeMap<Mode, BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    locked: Vec<String>,
    #[serde(default)]
    checks: Vec<Check>,
}

impl RawTheme {
    fn into_theme(self) -> Result<Theme, String> {
        let conv = |m: BTreeMap<String, serde_json::Value>| -> Result<BTreeMap<String, TokenValue>, String> {
            m.into_iter().map(|(k, v)| TokenValue::parse(&v).map(|t| (k.clone(), t)).map_err(|e| format!("{k}: {e}"))).collect()
        };
        let mut modes = BTreeMap::new();
        for (m, t) in self.modes {
            modes.insert(m, conv(t)?);
        }
        Ok(Theme { name: self.name, extends: self.extends, scheme: self.scheme, tokens: conv(self.tokens)?, modes, locked: self.locked, checks: self.checks })
    }
}

/// A registry of themes by name, for resolving `extends` chains.
#[derive(Clone, Debug, Default)]
pub struct ThemeSet {
    pub themes: BTreeMap<String, Theme>,
}

impl ThemeSet {
    pub fn with_builtins() -> ThemeSet {
        let mut s = ThemeSet::default();
        let n = Theme::neutral();
        s.themes.insert(n.name.clone(), n);
        s
    }
    pub fn add(&mut self, t: Theme) {
        self.themes.insert(t.name.clone(), t);
    }
    /// The inheritance chain for `name`, base first. Errors on unknown names or cycles.
    pub fn chain(&self, name: &str) -> Result<Vec<&Theme>, String> {
        let mut out = Vec::new();
        let mut cur = Some(name.to_string());
        while let Some(n) = cur {
            if out.iter().any(|t: &&Theme| t.name == n) {
                return Err(format!("theme inheritance cycle at `{n}`"));
            }
            let t = self.themes.get(&n).ok_or_else(|| format!("unknown theme `{n}`"))?;
            out.push(t);
            cur = t.extends.clone();
        }
        out.reverse();
        Ok(out)
    }
}

/// A palette after resolution.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub kind: PaletteKind,
    pub colors: Vec<Color>,
}

impl Palette {
    /// Categorical: cycle; sequential/diverging: sample the ramp at i/(n-1).
    pub fn get(&self, i: u32) -> Color {
        if self.colors.is_empty() {
            return Color::BLACK;
        }
        self.colors[i as usize % self.colors.len()]
    }
    /// Evaluate as a continuous ramp at t ∈ [0, 1].
    pub fn at(&self, t: f64) -> Color {
        datars_color::palette::ramp(&self.colors, t)
    }
}
