//! An icon as scenes carry it where one drawing repeats: the prototype of icon instances (a
//! pictogram's thousand people, map markers) and stroke markers. Layers in a box centred on the
//! origin whose longer side spans −1…1, so an instance's `size` scales it like a symbol's radius.
//! (A single `icon` node resolves to plain shapes instead; see the engine.)
//!
//! Colours stay inks (late-bound, P1/themes): a layer paints in the instance's own colour
//! (`Current`, what an SVG's `currentColor` becomes) or in one of the glyph's `inks`.

use crate::paint::{Cap, Join};
use datars_math::{FillRule, PathData};
use datars_theme::Ink;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IconInk {
    /// The instance's fill (the stroke's paint, for a marker).
    Current,
    /// `inks[i]` of the glyph.
    Slot(u8),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IconStroke {
    pub ink: IconInk,
    /// In the glyph's units (the box spans 2).
    pub width: f64,
    #[serde(default)]
    pub cap: Cap,
    #[serde(default)]
    pub join: Join,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IconLayer {
    pub path: Arc<PathData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<IconInk>,
    #[serde(default, skip_serializing_if = "is_nonzero")]
    pub rule: FillRule,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<IconStroke>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub opacity: f64,
}

fn is_nonzero(r: &FillRule) -> bool {
    *r == FillRule::NonZero
}
fn one() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}

/// An icon's drawing, normalized: what an instance or a marker repeats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IconGlyph {
    /// Where it came from (`lucide:heart`, `logo.svg`): its identity, for caches and exports.
    pub name: Arc<str>,
    pub layers: Vec<IconLayer>,
    /// The inks of its other colours (a logo's brand red), by slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inks: Vec<Ink>,
    /// Width ÷ height of its box (the longer side spans 2 units): hit tests and bounds.
    #[serde(default = "one")]
    pub aspect: f64,
}

impl IconGlyph {
    /// Half the box's width and height, in glyph units (the longer is 1).
    pub fn half_extent(&self) -> (f64, f64) {
        let a = if self.aspect.is_finite() && self.aspect > 0.0 { self.aspect } else { 1.0 };
        if a >= 1.0 {
            (1.0, 1.0 / a)
        } else {
            (a, 1.0)
        }
    }
}

impl IconGlyph {
    /// The whole drawing as one path at `c`, `s` px per glyph unit, and how to paint it as one
    /// shape: whether any layer fills, and the first stroke (its width in px) — so an icon can
    /// travel as a single shape (a pictogram's unit flying to a bar and back) and still look like
    /// itself at either end.
    pub fn as_shape(&self, c: datars_math::Vec2, s: f64) -> (PathData, bool, Option<IconStroke>) {
        let xf = datars_math::Affine::translate(c.x, c.y).mul(datars_math::Affine::scale(s, s));
        let mut p = PathData::new();
        for l in &self.layers {
            p.extend(&l.path.transform(&xf));
        }
        let fills = self.layers.iter().any(|l| l.fill.is_some());
        let stroke = self.layers.iter().find_map(|l| l.stroke.clone()).map(|st| IconStroke { width: st.width * s, ..st });
        (p, fills, stroke)
    }
}
