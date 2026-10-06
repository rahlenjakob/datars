//! Colour scales. They return [`Ink`]s, not colours: a palette stays a theme reference
//! (`$sequential~0.42`, `$categorical[3]`) resolved at frame time, so theme and mode switches need
//! no re-resolve (docs/18). Explicit colour stops interpolate in OKLab and return literal colours.

use super::continuous::Transform;
use crate::value::Value;
use datars_color::Color;
use datars_theme::Ink;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

fn yes() -> bool {
    true
}

/// Where a continuous colour scale gets its colours: a theme palette used as a ramp (JSON: its
/// name, `"sequential"`), or explicit stops (JSON: an array of colours) spaced evenly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Colors {
    Palette(String),
    Stops(Vec<Color>),
}

impl Colors {
    pub fn palette(name: &str) -> Colors {
        Colors::Palette(name.trim_start_matches('$').to_string())
    }
    /// Explicit stops from colour strings (`"#f7fbff"`); unparseable entries are skipped.
    pub fn stops(colors: &[&str]) -> Colors {
        Colors::Stops(colors.iter().filter_map(|c| Color::parse(c)).collect())
    }
    /// The ink at ramp position `t` ∈ [0, 1].
    pub fn at(&self, t: f64) -> Ink {
        match self {
            Colors::Palette(name) => Ink::ramp(name.trim_start_matches('$'), t),
            Colors::Stops(stops) => Ink::Color(datars_color::palette::ramp(stops, t)),
        }
    }
}

/// A continuous domain onto a colour ramp (`t` = normalized position, after the transform).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sequential {
    pub domain: [f64; 2],
    pub colors: Colors,
    #[serde(default, skip_serializing_if = "Transform::is_linear")]
    pub transform: Transform,
    #[serde(default = "yes")]
    pub clamp: bool,
}

impl Sequential {
    /// Ramp position of `x` (NaN for null).
    pub fn position(&self, x: f64) -> f64 {
        let t = self.transform.normalize(x, self.domain);
        if self.clamp {
            t.clamp(0.0, 1.0)
        } else {
            t
        }
    }
}

/// Two ramps meeting at a midpoint: `domain = [low, mid, high]` maps to positions 0, 0.5, 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diverging {
    pub domain: [f64; 3],
    pub colors: Colors,
    #[serde(default, skip_serializing_if = "Transform::is_linear")]
    pub transform: Transform,
    #[serde(default = "yes")]
    pub clamp: bool,
}

impl Diverging {
    pub fn position(&self, x: f64) -> f64 {
        let [lo, mid, hi] = self.domain;
        let t = if x < mid {
            0.5 * self.transform.normalize(x, [lo, mid])
        } else {
            0.5 + 0.5 * self.transform.normalize(x, [mid, hi])
        };
        // A collapsed half (lo == mid) normalizes to 0.5; keep the midpoint exact there.
        let t = if x == mid { 0.5 } else { t };
        if self.clamp {
            t.clamp(0.0, 1.0)
        } else {
            t
        }
    }
    /// The domain value at ramp position `t`.
    pub fn value_at(&self, t: f64) -> f64 {
        let [lo, mid, hi] = self.domain;
        if t < 0.5 {
            self.transform.denormalize(t * 2.0, [lo, mid])
        } else {
            self.transform.denormalize((t - 0.5) * 2.0, [mid, hi])
        }
    }
}

/// Keys → palette colours in domain (first-appearance) order: key i gets `$palette[i]`
/// (palettes cycle when resolved). `overrides` pin inks to keys (a party's colour is data); keys
/// keep their palette slot either way, so pinning one key never shifts the others. Keys outside
/// the domain get `unknown` (default `$muted`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Categorical {
    pub domain: Vec<Value>,
    pub palette: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overrides: Vec<(Value, Ink)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown: Option<Ink>,
}

impl Categorical {
    pub fn ink(&self, v: &Value) -> Option<Ink> {
        if let Some((_, ink)) = self.overrides.iter().find(|(k, _)| k == v) {
            return Some(ink.clone());
        }
        self.index_of(v).map(|i| Ink::Palette { name: Arc::from(self.palette.trim_start_matches('$')), index: i as u32, alpha: 1.0 })
    }
    pub fn index_of(&self, v: &Value) -> Option<usize> {
        self.domain.iter().position(|d| d == v)
    }
}

/// Colours pinned to values (`#22c55e 2 · #f5a524 4 · #f97362 6.5`): below the first stop the
/// first colour, above the last the last, between stops an OKLab blend — or, with `stepped`, the
/// lower stop's colour (thresholds). Stops that are theme references can't be blended in data
/// space; between two such stops the nearer one wins (two ramp positions on the same palette do
/// blend, by position).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Piecewise {
    pub stops: Vec<(f64, Ink)>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stepped: bool,
}

impl Piecewise {
    /// Stops sorted by value (stable).
    pub fn new(mut stops: Vec<(f64, Ink)>) -> Piecewise {
        stops.retain(|s| s.0.is_finite());
        stops.sort_by(|a, b| datars_math::total_cmp(a.0, b.0));
        Piecewise { stops, stepped: false }
    }

    /// Parse `"#22c55e 2 · #f5a524 4 · #f97362 6.5"` (ink, value, ink, value …; separators `·`,
    /// `,`, `;` or spaces). Inks may be theme references (`$accent`).
    pub fn parse(s: &str) -> Option<Piecewise> {
        let toks: Vec<&str> = s.split(|c: char| c.is_whitespace() || matches!(c, '·' | ',' | ';')).filter(|t| !t.is_empty()).collect();
        if toks.is_empty() || toks.len() % 2 == 1 {
            return None;
        }
        let stops = toks.chunks(2).map(|p| Some((p[1].parse::<f64>().ok()?, Ink::parse(p[0])?))).collect::<Option<Vec<_>>>()?;
        Some(Piecewise::new(stops))
    }

    /// Normalized position of `x` between the first and last stop.
    pub fn position(&self, x: f64) -> f64 {
        match (self.stops.first(), self.stops.last()) {
            (Some(a), Some(b)) if b.0 > a.0 => ((x - a.0) / (b.0 - a.0)).clamp(0.0, 1.0),
            (Some(_), Some(_)) if !x.is_nan() => 0.5,
            _ => f64::NAN,
        }
    }

    pub fn ink(&self, x: f64) -> Option<Ink> {
        if x.is_nan() {
            return None;
        }
        let (first, last) = (self.stops.first()?, self.stops.last()?);
        if x <= first.0 {
            return Some(first.1.clone());
        }
        if x >= last.0 {
            return Some(last.1.clone());
        }
        let i = self.stops.partition_point(|s| s.0 <= x) - 1;
        let (a, b) = (&self.stops[i], &self.stops[i + 1]);
        if self.stepped || x == a.0 {
            return Some(a.1.clone());
        }
        let f = (x - a.0) / (b.0 - a.0);
        Some(match (&a.1, &b.1) {
            (Ink::Color(ca), Ink::Color(cb)) => Ink::Color(ca.lerp_oklab(*cb, f)),
            (Ink::Ramp { name: na, at: ta, alpha: aa }, Ink::Ramp { name: nb, at: tb, alpha: ab }) if na == nb => {
                Ink::Ramp { name: na.clone(), at: ta + (tb - ta) * f as f32, alpha: aa + (ab - aa) * f as f32 }
            }
            _ if f < 0.5 => a.1.clone(),
            _ => b.1.clone(),
        })
    }
}
