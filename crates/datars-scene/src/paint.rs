use datars_theme::{Ink, ResolvedTheme};
use datars_color::Color;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    pub at: f64,
    pub ink: Ink,
}

/// Where a fill or stroke gets its colour. Colours are inks: literals or late-bound theme tokens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Paint {
    Solid(Ink),
    Linear { linear: [f64; 4], stops: Vec<Stop> },
    Radial { radial: [f64; 3], stops: Vec<Stop> },
}

impl Default for Paint {
    fn default() -> Self {
        Paint::Solid(Ink::default())
    }
}

impl From<Ink> for Paint {
    fn from(i: Ink) -> Paint {
        Paint::Solid(i)
    }
}
impl From<Color> for Paint {
    fn from(c: Color) -> Paint {
        Paint::Solid(Ink::Color(c))
    }
}

impl Paint {
    pub fn token(name: &str) -> Paint {
        Paint::Solid(Ink::token(name))
    }
    pub fn parse(s: &str) -> Option<Paint> {
        Ink::parse(s).map(Paint::Solid)
    }
    /// A representative colour (solid colour, or the first stop) under `theme`.
    pub fn representative(&self, theme: &ResolvedTheme) -> Color {
        match self {
            Paint::Solid(i) => i.resolve(theme),
            Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => stops.first().map(|s| s.ink.resolve(theme)).unwrap_or(Color::BLACK),
        }
    }
    pub fn fade(&self, k: f32) -> Paint {
        match self {
            Paint::Solid(i) => Paint::Solid(i.fade(k)),
            Paint::Linear { linear, stops } => Paint::Linear { linear: *linear, stops: stops.iter().map(|s| Stop { at: s.at, ink: s.ink.fade(k) }).collect() },
            Paint::Radial { radial, stops } => Paint::Radial { radial: *radial, stops: stops.iter().map(|s| Stop { at: s.at, ink: s.ink.fade(k) }).collect() },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub paint: Paint,
    pub width: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<f64>>,
    #[serde(default)]
    pub cap: Cap,
    #[serde(default)]
    pub join: Join,
    /// Width stays in screen px under a zooming camera (maps, zoomed charts).
    #[serde(default)]
    pub non_scaling: bool,
}

impl Stroke {
    pub fn new(paint: impl Into<Paint>, width: f64) -> Stroke {
        Stroke { paint: paint.into(), width, dash: None, cap: Cap::Butt, join: Join::Miter, non_scaling: false }
    }
}
