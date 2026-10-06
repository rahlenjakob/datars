//! Enter and exit ghost states: partial property overrides relative to an element's final
//! (enter) or initial (exit) state. An entering element interpolates *from* its ghost; an exiting
//! one *to* it (docs/05 §Enter and exit).
//!
//! - `opacity` multiplies the element's opacity (`0` = fade).
//! - `scale` scales about `origin` (uniformly about the centre; along y only about a baseline,
//!   top or bottom; along x only about the left or right edge) — bars grow from their axis.
//! - `dx`/`dy` offset the element in its placement space (≈ px).
//! - `from` places the element's centre at its parent datum's centre (drill-down), at a point, or
//!   at the plan's event point (burst out of the click); it implies `scale: 0` unless given.
//! - `trim` draws only this share of a shape's length, from its start (`0` = nothing): an
//!   entering line draws on, an exiting one draws off. Shapes only (text and images ignore it).

use datars_math::{Affine, Rect, Vec2};
use serde::{Deserialize, Serialize};

/// The point a scaling ghost scales about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    /// Uniform scale about the element's centre.
    #[default]
    Center,
    /// Scale along y about this y (the element's local coordinates): bars grow from the axis.
    Baseline(f64),
    /// Scale along y about the bottom edge of the element's bounds.
    Bottom,
    /// Scale along y about the top edge.
    Top,
    /// Scale along x about the left edge.
    Left,
    /// Scale along x about the right edge.
    Right,
    /// Uniform scale about a point (local coordinates).
    Point(Vec2),
}

/// Where a ghost appears from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GhostFrom {
    /// The parent datum's centre in the other scene: the element whose key is this key minus its
    /// last part (`("S",)` for `("S", Unit(3))`), else the node at the parent key path.
    Parent,
    /// A point in root-content coordinates.
    Point(Vec2),
    /// The plan's event point (`PlanCx::event_point`), e.g. the clicked position.
    Event,
}

/// A ghost state. The default is the identity (no change); see [`Ghost::fade`] and friends.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ghost {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "is_center")]
    pub origin: Origin,
    #[serde(skip_serializing_if = "is_zero")]
    pub dx: f64,
    #[serde(skip_serializing_if = "is_zero")]
    pub dy: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<GhostFrom>,
    /// The share of a shape's length drawn, from its start (of its own trim, if it has one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim: Option<f64>,
}

fn is_center(o: &Origin) -> bool {
    *o == Origin::Center
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Ghost {
    /// `{ opacity: 0 }` — the default enter and exit.
    pub fn fade() -> Ghost {
        Ghost { opacity: Some(0.0), ..Default::default() }
    }
    /// `{ scale: 0, origin }` — grow from (shrink to) nothing.
    pub fn grow(origin: Origin) -> Ghost {
        Ghost { scale: Some(0.0), origin, ..Default::default() }
    }
    /// `{ opacity: 0, dy }` — rise from below (`dy > 0`) or drop from above (`dy < 0`).
    pub fn rise(dy: f64) -> Ghost {
        Ghost { opacity: Some(0.0), dy, ..Default::default() }
    }
    /// Appear from the parent datum's position (drill-down, splits).
    pub fn from_parent() -> Ghost {
        Ghost { from: Some(GhostFrom::Parent), ..Default::default() }
    }
    /// Burst out of a point (root-content coordinates).
    pub fn from_point(p: Vec2) -> Ghost {
        Ghost { from: Some(GhostFrom::Point(p)), ..Default::default() }
    }
    /// `{ trim: 0 }` — a line draws on (enter) or off (exit) along its length.
    pub fn draw_on() -> Ghost {
        Ghost { trim: Some(0.0), ..Default::default() }
    }
    pub fn opacity(mut self, o: f64) -> Ghost {
        self.opacity = Some(o);
        self
    }
    pub fn scale(mut self, s: f64, origin: Origin) -> Ghost {
        self.scale = Some(s);
        self.origin = origin;
        self
    }

    /// The opacity multiplier (clamped to [0, 1] so a ghost can never brighten an element).
    pub fn opacity_factor(&self) -> f64 {
        self.opacity.map_or(1.0, |o| if o.is_finite() { o.clamp(0.0, 1.0) } else { 1.0 })
    }

    /// The trim of a ghost whose element is trimmed to `own` (`None` = whole): the first
    /// `trim` share of it, so the ghost's drawn part starts where the element's does.
    pub(crate) fn trimmed(&self, own: Option<[f64; 2]>) -> Option<[f64; 2]> {
        let Some(f) = self.trim.filter(|f| f.is_finite()) else { return own };
        let [s, e] = own.unwrap_or([0.0, 1.0]);
        Some([s, s + (e - s) * f.clamp(0.0, 1.0)])
    }

    pub(crate) fn scale_factor(&self) -> Option<f64> {
        self.scale.or(if self.from.is_some() { Some(0.0) } else { None }).map(|s| if s.is_finite() { s } else { 1.0 })
    }

    /// Scale origin and factors for an element with local `bounds` and `center`.
    pub(crate) fn scaling(&self, bounds: Rect, center: Vec2) -> Option<(Vec2, f64, f64)> {
        let s = self.scale_factor()?;
        let b = if bounds.is_empty() { Rect::new(center.x, center.y, 0.0, 0.0) } else { bounds };
        let c = center;
        Some(match self.origin {
            Origin::Center => (c, s, s),
            Origin::Baseline(y) => (Vec2::new(c.x, y), 1.0, s),
            Origin::Bottom => (Vec2::new(c.x, b.y1()), 1.0, s),
            Origin::Top => (Vec2::new(c.x, b.y), 1.0, s),
            Origin::Left => (Vec2::new(b.x, c.y), s, 1.0),
            Origin::Right => (Vec2::new(b.x1(), c.y), s, 1.0),
            Origin::Point(p) => (p, s, s),
        })
    }

    /// The ghost transform of an element whose own transform (local → placement space) is `xf`.
    /// `target` is where the element's centre should sit (placement space) when the ghost comes
    /// `from` somewhere.
    pub(crate) fn transform(&self, xf: Affine, bounds: Rect, center: Vec2, target: Option<Vec2>) -> Affine {
        let mut out = xf;
        if let Some((o, sx, sy)) = self.scaling(bounds, center) {
            let s = Affine::translate(-o.x, -o.y).then(Affine::scale(sx, sy)).then(Affine::translate(o.x, o.y));
            out = xf.mul(s);
        }
        let mut d = Vec2::new(self.dx, self.dy);
        if let Some(p) = target {
            d += p - out.apply(center);
        }
        if d != Vec2::ZERO && d.is_finite() {
            out = out.then(Affine::translate(d.x, d.y));
        }
        out
    }

    pub(crate) fn needs_target(&self) -> bool {
        self.from.is_some()
    }
}
