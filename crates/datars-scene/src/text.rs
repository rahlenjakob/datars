//! Text as the scene stores it: already shaped and laid out by `datars-text`, so every backend draws
//! the same glyphs at the same positions (P1). The plain string and style are kept for accessibility,
//! SVG export, and re-shaping animated numbers.

use crate::key::Sym;
use datars_math::{Rect, Vec2};
use datars_theme::Ink;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GlyphPos {
    pub id: u16,
    pub x: f32,
    pub y: f32,
}

/// Glyphs of one font face at one size and colour. Positions are px relative to the text origin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextRun {
    pub font: Sym,
    pub size: f64,
    pub ink: Ink,
    pub glyphs: Arc<[GlyphPos]>,
}

/// Horizontal alignment at the text's origin — physical, for any script: `start` puts the left
/// edge there, `end` the right edge (a right-to-left label aligned `end` still ends at its tick).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Start,
    Middle,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Baseline {
    #[default]
    Alphabetic,
    Middle,
    Top,
    Bottom,
}

/// The authored style (kept for re-shaping and vector export).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub family: Sym,
    #[serde(default = "w400")]
    pub weight: u16,
    pub size: f64,
    pub ink: Ink,
    #[serde(default)]
    pub align: Align,
    #[serde(default)]
    pub baseline: Baseline,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_width: Option<f64>,
    #[serde(default = "lh")]
    pub line_height: f64,
}

fn w400() -> u16 {
    400
}
fn lh() -> f64 {
    1.2
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: Arc::from("Inter"),
            weight: 400,
            size: 12.0,
            ink: Ink::token("ink"),
            align: Align::Start,
            baseline: Baseline::Alphabetic,
            max_width: None,
            line_height: 1.2,
        }
    }
}

/// A number shown as text that counts when it animates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumberText {
    pub value: f64,
    /// A format spec (`datars-text` number formats: ",.1f", ".0%", "$,.0s", …).
    pub format: Sym,
    #[serde(default = "en")]
    pub locale: Sym,
}

fn en() -> Sym {
    Arc::from("en")
}

fn is_zero(v: &Vec2) -> bool {
    v.x == 0.0 && v.y == 0.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextNode {
    pub text: String,
    pub origin: Vec2,
    pub style: TextStyle,
    #[serde(default)]
    pub runs: Vec<TextRun>,
    /// Bounds of the laid-out text relative to `origin`, in px.
    #[serde(default)]
    pub bounds: Rect,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub halo: Option<(Ink, f64)>,
    /// Glyph offsets stay screen-sized under a zooming camera (labels on maps and zoomed charts).
    #[serde(default)]
    pub screen_size: bool,
    /// Shifts the text from its origin in screen px when `screen_size` (in local units scaled like
    /// the glyphs otherwise): a label beside a point stays beside it at any camera zoom.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<NumberText>,
    #[serde(default)]
    pub rotate: f64,
    /// Nudged back inside the canvas after layout when it would cross an edge.
    #[serde(default, skip_serializing_if = "is_false")]
    pub contain: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl TextNode {
    /// An unshaped text node; the engine's text stage fills `runs` and `bounds`.
    pub fn new(text: impl Into<String>, origin: Vec2, style: TextStyle) -> TextNode {
        TextNode { text: text.into(), origin, style, runs: Vec::new(), bounds: Rect::default(), halo: None, screen_size: true, offset: Vec2::ZERO, number: None, rotate: 0.0, contain: false }
    }
    pub fn is_shaped(&self) -> bool {
        !self.runs.is_empty() || self.text.is_empty()
    }
}
