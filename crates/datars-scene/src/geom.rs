//! Geometry: parametric where possible (cheap, exact, morph by parameter), a path for everything
//! else. Every geometry converts to a `PathData`.

use datars_math::{m, PathData, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Curve {
    #[default]
    Linear,
    /// Monotone cubic in x (no overshoot) — the default for time series.
    MonotoneX,
    /// Catmull-Rom (centripetal-ish, tension 0.5).
    CatmullRom,
    Step,
    StepBefore,
    StepAfter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    #[default]
    Circle,
    Square,
    Diamond,
    Triangle,
    Cross,
    Star,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Geom {
    Rect { x: f64, y: f64, w: f64, h: f64, #[serde(default)] r: [f64; 4] },
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64 },
    /// Annular sector; angles in radians clockwise from 12 o'clock; `r0` = 0 is a pie slice.
    Arc { cx: f64, cy: f64, r0: f64, r1: f64, a0: f64, a1: f64 },
    Segment { x1: f64, y1: f64, x2: f64, y2: f64 },
    Polyline { pts: Arc<[Vec2]>, #[serde(default)] closed: bool, #[serde(default)] curve: Curve },
    /// A filled band between `top` and `base` (same length; `base` runs in the same x order).
    Area { top: Arc<[Vec2]>, base: Arc<[Vec2]>, #[serde(default)] curve: Curve },
    Path { path: Arc<PathData> },
    Symbol { kind: SymbolKind, x: f64, y: f64, size: f64 },
}

impl Geom {
    pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Geom {
        Geom::Rect { x, y, w, h, r: [0.0; 4] }
    }
    pub fn circle(cx: f64, cy: f64, r: f64) -> Geom {
        Geom::Ellipse { cx, cy, rx: r, ry: r }
    }
    pub fn path(p: PathData) -> Geom {
        Geom::Path { path: Arc::new(p) }
    }
    pub fn polyline(pts: Vec<Vec2>) -> Geom {
        Geom::Polyline { pts: pts.into(), closed: false, curve: Curve::Linear }
    }

    /// Whether this geometry encloses an area (fills apply) or is a line (only strokes apply).
    pub fn is_closed(&self) -> bool {
        match self {
            Geom::Segment { .. } => false,
            Geom::Polyline { closed, .. } => *closed,
            _ => true,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Geom::Rect { .. } => "rect",
            Geom::Ellipse { .. } => "ellipse",
            Geom::Arc { .. } => "arc",
            Geom::Segment { .. } => "segment",
            Geom::Polyline { .. } => "polyline",
            Geom::Area { .. } => "area",
            Geom::Path { .. } => "path",
            Geom::Symbol { .. } => "symbol",
        }
    }

    pub fn to_path(&self) -> PathData {
        match self {
            Geom::Rect { x, y, w, h, r } => {
                let (x0, w0) = if *w < 0.0 { (x + w, -w) } else { (*x, *w) };
                let (y0, h0) = if *h < 0.0 { (y + h, -h) } else { (*y, *h) };
                PathData::rounded_rect(Rect::new(x0, y0, w0, h0), *r)
            }
            Geom::Ellipse { cx, cy, rx, ry } => PathData::ellipse(Vec2::new(*cx, *cy), rx.abs(), ry.abs()),
            Geom::Arc { cx, cy, r0, r1, a0, a1 } => {
                let (a, b) = if a1 >= a0 { (*a0, *a1) } else { (*a1, *a0) };
                PathData::annular_sector(Vec2::new(*cx, *cy), r0.max(0.0), r1.max(0.0), a, b)
            }
            Geom::Segment { x1, y1, x2, y2 } => PathData::polyline(&[Vec2::new(*x1, *y1), Vec2::new(*x2, *y2)]),
            Geom::Polyline { pts, closed, curve } => {
                let mut p = curve_path(pts, *curve);
                if *closed {
                    p.close();
                }
                p
            }
            Geom::Area { top, base, curve } => {
                let mut p = curve_path(top, *curve);
                let rev: Vec<Vec2> = base.iter().rev().copied().collect();
                let back = curve_path(&rev, *curve);
                for (i, e) in back.els.iter().enumerate() {
                    match (i, e) {
                        (0, datars_math::PathEl::Move { p: q }) => {
                            p.line_to(*q);
                        }
                        _ => p.els.push(*e),
                    }
                }
                if !p.is_empty() {
                    p.close();
                }
                p
            }
            Geom::Path { path } => (**path).clone(),
            Geom::Symbol { kind, x, y, size } => symbol_path(*kind, Vec2::new(*x, *y), *size),
        }
    }

    /// Local bounds (control-point bounds for curves).
    pub fn bounds(&self) -> Rect {
        match self {
            Geom::Rect { x, y, w, h, .. } => Rect::from_points(Vec2::new(*x, *y), Vec2::new(x + w, y + h)),
            Geom::Ellipse { cx, cy, rx, ry } => Rect::new(cx - rx.abs(), cy - ry.abs(), 2.0 * rx.abs(), 2.0 * ry.abs()),
            Geom::Symbol { x, y, size, .. } => Rect::new(x - size, y - size, 2.0 * size, 2.0 * size),
            Geom::Segment { x1, y1, x2, y2 } => Rect::from_points(Vec2::new(*x1, *y1), Vec2::new(*x2, *y2)),
            // Measured in place: a copy of a county's outline just to measure it was most of what
            // hit testing a county map cost.
            Geom::Path { path } => path.bounds(),
            _ => self.to_path().bounds(),
        }
    }

    /// Flattened to polylines at `tol` (see `PathData::flatten`), without copying a shared path.
    pub fn flatten(&self, tol: f64) -> Vec<(Vec<Vec2>, bool)> {
        match self {
            Geom::Path { path } => path.flatten(tol),
            _ => self.to_path().flatten(tol),
        }
    }

    /// A representative centre point (for labels, anchors, morph travel).
    pub fn center(&self) -> Vec2 {
        match self {
            Geom::Arc { cx, cy, r0, r1, a0, a1 } => Vec2::polar(Vec2::new(*cx, *cy), (r0 + r1) / 2.0, (a0 + a1) / 2.0),
            _ => self.bounds().center(),
        }
    }
}

/// Interpolate points into a path with the given curve.
pub fn curve_path(pts: &[Vec2], curve: Curve) -> PathData {
    let mut p = PathData::new();
    let n = pts.len();
    if n == 0 {
        return p;
    }
    p.move_to(pts[0]);
    if n == 1 {
        return p;
    }
    match curve {
        Curve::Linear => {
            for q in &pts[1..] {
                p.line_to(*q);
            }
        }
        Curve::Step | Curve::StepBefore | Curve::StepAfter => {
            for w in pts.windows(2) {
                let (a, b) = (w[0], w[1]);
                match curve {
                    Curve::Step => {
                        let mx = (a.x + b.x) / 2.0;
                        p.line_to(Vec2::new(mx, a.y)).line_to(Vec2::new(mx, b.y)).line_to(b);
                    }
                    Curve::StepBefore => {
                        p.line_to(Vec2::new(a.x, b.y)).line_to(b);
                    }
                    _ => {
                        p.line_to(Vec2::new(b.x, a.y)).line_to(b);
                    }
                }
            }
        }
        Curve::CatmullRom => {
            for i in 0..n - 1 {
                let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
                let (p1, p2) = (pts[i], pts[i + 1]);
                let p3 = if i + 2 < n { pts[i + 2] } else { pts[n - 1] };
                let c1 = p1 + (p2 - p0) * (1.0 / 6.0);
                let c2 = p2 - (p3 - p1) * (1.0 / 6.0);
                p.cubic_to(c1, c2, p2);
            }
        }
        Curve::MonotoneX => {
            // Fritsch–Carlson monotone cubic interpolation (as d3.curveMonotoneX).
            let slope = |a: Vec2, b: Vec2| if b.x != a.x { (b.y - a.y) / (b.x - a.x) } else { 0.0 };
            let d: Vec<f64> = pts.windows(2).map(|w| slope(w[0], w[1])).collect();
            let mut t = vec![0.0; n];
            t[0] = d[0];
            t[n - 1] = d[n - 2];
            for i in 1..n - 1 {
                t[i] = if d[i - 1] * d[i] <= 0.0 {
                    0.0
                } else {
                    let (h0, h1) = (pts[i].x - pts[i - 1].x, pts[i + 1].x - pts[i].x);
                    let w1 = 2.0 * h1 + h0;
                    let w2 = h1 + 2.0 * h0;
                    (w1 + w2) / (w1 / d[i - 1] + w2 / d[i])
                };
            }
            for i in 0..n - 1 {
                let (a, b) = (pts[i], pts[i + 1]);
                let h = (b.x - a.x) / 3.0;
                p.cubic_to(Vec2::new(a.x + h, a.y + h * t[i]), Vec2::new(b.x - h, b.y - h * t[i + 1]), b);
            }
        }
    }
    p
}

fn symbol_path(kind: SymbolKind, c: Vec2, s: f64) -> PathData {
    match kind {
        SymbolKind::Circle => PathData::circle(c, s),
        SymbolKind::Square => PathData::rect(Rect::new(c.x - s, c.y - s, 2.0 * s, 2.0 * s)),
        SymbolKind::Diamond => PathData::polygon(&[Vec2::new(c.x, c.y - s * 1.3), Vec2::new(c.x + s, c.y), Vec2::new(c.x, c.y + s * 1.3), Vec2::new(c.x - s, c.y)]),
        SymbolKind::Triangle => {
            let pts: Vec<Vec2> = (0..3).map(|i| Vec2::polar(c, s * 1.2, m::TAU * i as f64 / 3.0)).collect();
            PathData::polygon(&pts)
        }
        SymbolKind::Cross => {
            let a = s / 3.0;
            PathData::polygon(&[
                Vec2::new(c.x - a, c.y - s), Vec2::new(c.x + a, c.y - s), Vec2::new(c.x + a, c.y - a), Vec2::new(c.x + s, c.y - a),
                Vec2::new(c.x + s, c.y + a), Vec2::new(c.x + a, c.y + a), Vec2::new(c.x + a, c.y + s), Vec2::new(c.x - a, c.y + s),
                Vec2::new(c.x - a, c.y + a), Vec2::new(c.x - s, c.y + a), Vec2::new(c.x - s, c.y - a), Vec2::new(c.x - a, c.y - a),
            ])
        }
        SymbolKind::Star => {
            let pts: Vec<Vec2> = (0..10).map(|i| Vec2::polar(c, if i % 2 == 0 { s * 1.25 } else { s * 0.5 }, m::TAU * i as f64 / 10.0)).collect();
            PathData::polygon(&pts)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monotone_does_not_overshoot() {
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(1.0, 10.0), Vec2::new(2.0, 10.0), Vec2::new(3.0, 0.0)];
        let p = curve_path(&pts, Curve::MonotoneX);
        let flat = p.flatten(0.01);
        let maxy = flat[0].0.iter().map(|q| q.y).fold(f64::MIN, f64::max);
        assert!(maxy <= 10.0 + 1e-9, "overshoot {maxy}");
    }
    #[test]
    fn negative_rects_normalize() {
        let b = Geom::rect(10.0, 10.0, -5.0, -5.0).to_path().bounds();
        assert_eq!((b.x, b.y, b.w, b.h), (5.0, 5.0, 5.0, 5.0));
    }
    #[test]
    fn area_is_closed_band() {
        let top: Vec<Vec2> = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 2.0)];
        let base: Vec<Vec2> = vec![Vec2::new(0.0, 5.0), Vec2::new(10.0, 5.0)];
        let g = Geom::Area { top: top.into(), base: base.into(), curve: Curve::Linear };
        let f = g.to_path().flatten(0.1);
        assert_eq!(f.len(), 1);
        assert!(f[0].1);
        assert_eq!(f[0].0.len(), 4);
    }
}
