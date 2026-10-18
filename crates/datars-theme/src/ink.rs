//! `Ink`: a colour as the scene stores it — a literal, or a late-bound reference to a theme token.
//! JSON forms: `"#e8112d"`, `"$accent"`, `"$categorical[3]"`, `"$ink@0.4"` (token with alpha),
//! `"on($mark)"` (the theme's ink or paper, whichever reads better on that background),
//! `"legible($categorical[2])"` (that colour, as text on the theme's paper: moved towards the ink
//! as far as it takes to read — WCAG's 4.5:1).

use crate::ResolvedTheme;
use datars_color::Color;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum Ink {
    Color(Color),
    Token { name: Arc<str>, alpha: f32 },
    Palette { name: Arc<str>, index: u32, alpha: f32 },
    /// A position `at` ∈ [0, 1] along a palette used as a continuous ramp (sequential/diverging
    /// colour scales). JSON: `"$sequential~0.42"`.
    Ramp { name: Arc<str>, at: f32, alpha: f32 },
    /// Text on a background ink: the theme's `ink` or `paper`, whichever contrasts more with it —
    /// late-bound like the background, so a label on a brand-coloured bar stays readable in every
    /// theme and mode. JSON: `"on($mark)"`, `"on(#1d4e89)"`.
    On { bg: Arc<Ink>, alpha: f32 },
    /// Text in a colour (a series' own, a "positive" green) on the theme's paper: the colour, moved
    /// towards the theme's ink just as far as it takes to have [`TEXT_CONTRAST`] against the paper
    /// — the series' hue kept, readable in every theme and mode. JSON: `"legible($categorical[2])"`.
    Legible { fg: Arc<Ink>, alpha: f32 },
}

/// The contrast text needs against what it's drawn on (WCAG 2.2, 1.4.3, at body sizes).
pub const TEXT_CONTRAST: f64 = 4.5;

/// The pure black or white that reads better on `b`: one of them always has at least 4.58:1.
fn black_or_white(b: Color) -> Color {
    if datars_color::contrast_ratio(Color::BLACK, b) >= datars_color::contrast_ratio(Color::WHITE, b) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// `fg` moved towards `ink` (in OKLab) just far enough to have `min` contrast with `bg`; `ink`
/// itself if even that doesn't (then the stronger of black and white).
pub fn legible_on(fg: Color, bg: Color, ink: Color, min: f64) -> Color {
    let fg = fg.with_alpha(1.0);
    if datars_color::contrast_ratio(fg, bg) >= min {
        return fg;
    }
    if datars_color::contrast_ratio(ink, bg) < min {
        return black_or_white(bg);
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..24 {
        let t = (lo + hi) / 2.0;
        if datars_color::contrast_ratio(fg.lerp_oklab(ink, t), bg) >= min {
            hi = t;
        } else {
            lo = t;
        }
    }
    fg.lerp_oklab(ink, hi)
}

impl Default for Ink {
    fn default() -> Self {
        Ink::Color(Color::BLACK)
    }
}

impl From<Color> for Ink {
    fn from(c: Color) -> Ink {
        Ink::Color(c)
    }
}

impl Ink {
    pub fn token(name: &str) -> Ink {
        Ink::Token { name: Arc::from(name), alpha: 1.0 }
    }
    pub fn palette(name: &str, index: u32) -> Ink {
        Ink::Palette { name: Arc::from(name), index, alpha: 1.0 }
    }
    pub fn ramp(name: &str, at: f64) -> Ink {
        Ink::Ramp { name: Arc::from(name), at: at.clamp(0.0, 1.0) as f32, alpha: 1.0 }
    }
    /// Multiply the ink's alpha.
    pub fn fade(&self, k: f32) -> Ink {
        match self {
            Ink::Color(c) => Ink::Color(c.fade(k)),
            Ink::Token { name, alpha } => Ink::Token { name: name.clone(), alpha: alpha * k },
            Ink::Palette { name, index, alpha } => Ink::Palette { name: name.clone(), index: *index, alpha: alpha * k },
            Ink::Ramp { name, at, alpha } => Ink::Ramp { name: name.clone(), at: *at, alpha: alpha * k },
            Ink::On { bg, alpha } => Ink::On { bg: bg.clone(), alpha: alpha * k },
            Ink::Legible { fg, alpha } => Ink::Legible { fg: fg.clone(), alpha: alpha * k },
        }
    }
    /// The concrete colour under `theme`. Unknown tokens resolve to magenta so they're visible (and
    /// the linter reports them).
    pub fn resolve(&self, theme: &ResolvedTheme) -> Color {
        const MISSING: Color = Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
        match self {
            Ink::Color(c) => *c,
            Ink::Token { name, alpha } => theme.color(name).or_else(|| theme.palette(name).map(|p| p.get(0))).unwrap_or(MISSING).fade(*alpha),
            Ink::Palette { name, index, alpha } => theme.palette(name).map(|p| p.get(*index)).unwrap_or(MISSING).fade(*alpha),
            Ink::Ramp { name, at, alpha } => theme.palette(name).map(|p| p.at(*at as f64)).unwrap_or(MISSING).fade(*alpha),
            Ink::On { bg, alpha } => {
                let ink = theme.color("ink").unwrap_or(Color::BLACK);
                let paper = theme.color("paper").unwrap_or(Color::WHITE);
                // A see-through fill (`on($categorical[2]@0.8)`) is read as drawn, over the paper.
                let b = bg.resolve(theme).over(paper);
                let pick = if datars_color::contrast_ratio(ink, b) >= datars_color::contrast_ratio(paper, b) { ink } else { paper };
                // A mid-tone fill neither reaches 4.5:1 with: pure black or white does.
                let min = theme.number("text.contrast").unwrap_or(TEXT_CONTRAST);
                let pick = if datars_color::contrast_ratio(pick, b) < min { black_or_white(b) } else { pick };
                pick.fade(*alpha)
            }
            Ink::Legible { fg, alpha } => {
                let ink = theme.color("ink").unwrap_or(Color::BLACK);
                let paper = theme.color("paper").unwrap_or(Color::WHITE);
                let c = fg.resolve(theme);
                // Against the paper, and the surfaces text sits on as often (a panel, a card): the
                // one of them it reads worst on.
                let worst = ["paper", "surface", "card"].iter().filter_map(|t| theme.color(t)).map(|b| b.over(paper)).min_by(|a, b| datars_color::contrast_ratio(c.over(paper), *a).total_cmp(&datars_color::contrast_ratio(c.over(paper), *b))).unwrap_or(paper);
                // (`text.contrast`: what the theme asks of text — 7:1 in the high-contrast mode.)
                let min = theme.number("text.contrast").unwrap_or(TEXT_CONTRAST);
                legible_on(c.over(paper), worst, ink, min).with_alpha(c.a).fade(*alpha)
            }
        }
    }
    /// This ink as text is drawn with it: a data colour — a palette's, a ramp's, or one of the
    /// tokens marks are drawn in (`$accent`, `$mark`, `$positive`, `$negative`, `$up`, `$down`,
    /// `$highlight`, `$map.marker`) — is [`Ink::Legible`]: a series' label in its own colour, a
    /// green "+8.9 %", reads at 4.5:1 on the paper in every theme and mode. Text tokens, `on(…)`
    /// and literal colours are as written.
    pub fn as_text(&self) -> Ink {
        const MARK_TOKENS: &[&str] = &["accent", "mark", "positive", "negative", "up", "down", "highlight", "map.marker"];
        match self {
            Ink::Palette { alpha, .. } | Ink::Ramp { alpha, .. } => Ink::Legible { fg: Arc::new(self.fade(1.0 / alpha.max(1e-6))), alpha: *alpha },
            Ink::Token { name, alpha } if MARK_TOKENS.contains(&name.as_ref()) => Ink::Legible { fg: Arc::new(Ink::Token { name: name.clone(), alpha: 1.0 }), alpha: *alpha },
            _ => self.clone(),
        }
    }
    pub fn is_token(&self) -> bool {
        !matches!(self, Ink::Color(_))
    }
    pub fn parse(s: &str) -> Option<Ink> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("on(") {
            let (inner, alpha) = match rest.rsplit_once(")@") {
                Some((i, a)) => (i, a.parse::<f32>().ok()?),
                None => (rest.strip_suffix(')')?, 1.0),
            };
            return Some(Ink::On { bg: Arc::new(Ink::parse(inner)?), alpha });
        }
        if let Some(rest) = s.strip_prefix("legible(") {
            let (inner, alpha) = match rest.rsplit_once(")@") {
                Some((i, a)) => (i, a.parse::<f32>().ok()?),
                None => (rest.strip_suffix(')')?, 1.0),
            };
            return Some(Ink::Legible { fg: Arc::new(Ink::parse(inner)?), alpha });
        }
        if let Some(rest) = s.strip_prefix('$') {
            let (body, alpha) = match rest.split_once('@') {
                Some((b, a)) => (b, a.trim_end_matches('%').parse::<f32>().ok().map(|v| if a.ends_with('%') { v / 100.0 } else { v })?),
                None => (rest, 1.0),
            };
            if let Some((name, at)) = body.split_once('~') {
                let at: f32 = at.parse().ok()?;
                return Some(Ink::Ramp { name: Arc::from(name), at, alpha });
            }
            if let Some((name, idx)) = body.split_once('[') {
                let index = idx.strip_suffix(']')?.parse().ok()?;
                return Some(Ink::Palette { name: Arc::from(name), index, alpha });
            }
            return (!body.is_empty()).then(|| Ink::Token { name: Arc::from(body), alpha });
        }
        Color::parse(s).map(Ink::Color)
    }
}

impl std::fmt::Display for Ink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = |alpha: f32| if alpha == 1.0 { String::new() } else { format!("@{}", (alpha * 1000.0).round() / 1000.0) };
        match self {
            Ink::Color(c) => write!(f, "{}", c.to_hex()),
            Ink::Token { name, alpha } => write!(f, "${name}{}", a(*alpha)),
            Ink::Palette { name, index, alpha } => write!(f, "${name}[{index}]{}", a(*alpha)),
            Ink::Ramp { name, at, alpha } => write!(f, "${name}~{}{}", (*at * 10000.0).round() / 10000.0, a(*alpha)),
            Ink::On { bg, alpha } => write!(f, "on({bg}){}", a(*alpha)),
            Ink::Legible { fg, alpha } => write!(f, "legible({fg}){}", a(*alpha)),
        }
    }
}

impl Serialize for Ink {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Ink {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Ink, D::Error> {
        let s = String::deserialize(d)?;
        Ink::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not an ink: {s}")))
    }
}

impl datars_math::StableHash for Ink {
    fn stable_hash(&self, h: &mut datars_math::Hash64) {
        match self {
            Ink::Color(c) => {
                h.u8(0);
                c.stable_hash(h)
            }
            Ink::Token { name, alpha } => {
                h.u8(1);
                h.str(name);
                h.f32(*alpha)
            }
            Ink::Palette { name, index, alpha } => {
                h.u8(2);
                h.str(name);
                h.u32(*index);
                h.f32(*alpha)
            }
            Ink::Ramp { name, at, alpha } => {
                h.u8(3);
                h.str(name);
                h.f32(*at);
                h.f32(*alpha)
            }
            Ink::On { bg, alpha } => {
                h.u8(4);
                bg.stable_hash(h);
                h.f32(*alpha)
            }
            Ink::Legible { fg, alpha } => {
                h.u8(5);
                fg.stable_hash(h);
                h.f32(*alpha)
            }
        }
    }
}
