//! Paths: move/line/quad/cubic/close, builders for the parametric shapes, flattening to polylines,
//! bounds, transforms, arc length and resampling (for morphs).

use crate::{m, Affine, Rect, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum PathEl {
    Move { p: Vec2 },
    Line { p: Vec2 },
    Quad { c: Vec2, p: Vec2 },
    Cubic { c1: Vec2, c2: Vec2, p: Vec2 },
    Close,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// A path: a sequence of subpaths.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathData {
    pub els: Vec<PathEl>,
}

/// Circle-approximation constant for cubic arcs (4/3·tan(θ/4) for θ = 90°).
const KAPPA: f64 = 0.552_284_749_830_793_4;

impl PathData {
    pub fn new() -> PathData {
        PathData { els: Vec::new() }
    }
    pub fn move_to(&mut self, p: Vec2) -> &mut Self {
        self.els.push(PathEl::Move { p });
        self
    }
    pub fn line_to(&mut self, p: Vec2) -> &mut Self {
        self.els.push(PathEl::Line { p });
        self
    }
    pub fn quad_to(&mut self, c: Vec2, p: Vec2) -> &mut Self {
        self.els.push(PathEl::Quad { c, p });
        self
    }
    pub fn cubic_to(&mut self, c1: Vec2, c2: Vec2, p: Vec2) -> &mut Self {
        self.els.push(PathEl::Cubic { c1, c2, p });
        self
    }
    pub fn close(&mut self) -> &mut Self {
        self.els.push(PathEl::Close);
        self
    }
    pub fn is_empty(&self) -> bool {
        self.els.is_empty()
    }

    /// A closed polygon through `pts`.
    pub fn polygon(pts: &[Vec2]) -> PathData {
        PathData::polygon_iter(pts.iter().copied())
    }

    /// A closed polygon through the points an iterator yields, in one allocation (a morph builds
    /// one per shape per frame).
    pub fn polygon_iter(pts: impl ExactSizeIterator<Item = Vec2>) -> PathData {
        let mut p = PathData { els: Vec::with_capacity(pts.len() + 1) };
        p.extend_polygon(pts);
        p
    }

    /// Append a closed polygon (a subpath) through `pts`.
    pub fn extend_polygon(&mut self, mut pts: impl Iterator<Item = Vec2>) {
        if let Some(first) = pts.next() {
            self.move_to(first);
            for q in pts {
                self.line_to(q);
            }
            self.close();
        }
    }

    /// An open polyline through `pts`.
    pub fn polyline(pts: &[Vec2]) -> PathData {
        let mut p = PathData::new();
        if let Some((first, rest)) = pts.split_first() {
            p.move_to(*first);
            for q in rest {
                p.line_to(*q);
            }
        }
        p
    }

    /// A rectangle with per-corner radii [top-left, top-right, bottom-right, bottom-left], drawn
    /// clockwise (y down) from the top-centre, so rects, circles and wedges share a start point.
    pub fn rounded_rect(r: Rect, radii: [f64; 4]) -> PathData {
        let (w, h) = (r.w.max(0.0), r.h.max(0.0));
        let lim = (w / 2.0).min(h / 2.0);
        let rr = radii.map(|v| v.max(0.0).min(lim));
        let (x0, y0, x1, y1) = (r.x, r.y, r.x + w, r.y + h);
        let mut p = PathData::new();
        p.move_to(Vec2::new((x0 + x1) / 2.0, y0));
        // top-right
        p.line_to(Vec2::new(x1 - rr[1], y0));
        if rr[1] > 0.0 {
            p.cubic_to(Vec2::new(x1 - rr[1] + rr[1] * KAPPA, y0), Vec2::new(x1, y0 + rr[1] - rr[1] * KAPPA), Vec2::new(x1, y0 + rr[1]));
        }
        p.line_to(Vec2::new(x1, y1 - rr[2]));
        if rr[2] > 0.0 {
            p.cubic_to(Vec2::new(x1, y1 - rr[2] + rr[2] * KAPPA), Vec2::new(x1 - rr[2] + rr[2] * KAPPA, y1), Vec2::new(x1 - rr[2], y1));
        }
        p.line_to(Vec2::new(x0 + rr[3], y1));
        if rr[3] > 0.0 {
            p.cubic_to(Vec2::new(x0 + rr[3] - rr[3] * KAPPA, y1), Vec2::new(x0, y1 - rr[3] + rr[3] * KAPPA), Vec2::new(x0, y1 - rr[3]));
        }
        p.line_to(Vec2::new(x0, y0 + rr[0]));
        if rr[0] > 0.0 {
            p.cubic_to(Vec2::new(x0, y0 + rr[0] - rr[0] * KAPPA), Vec2::new(x0 + rr[0] - rr[0] * KAPPA, y0), Vec2::new(x0 + rr[0], y0));
        }
        p.close();
        p
    }

    pub fn rect(r: Rect) -> PathData {
        PathData::rounded_rect(r, [0.0; 4])
    }

    /// An ellipse, clockwise from 12 o'clock.
    pub fn ellipse(c: Vec2, rx: f64, ry: f64) -> PathData {
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        let mut p = PathData::new();
        p.move_to(Vec2::new(c.x, c.y - ry));
        p.cubic_to(Vec2::new(c.x + kx, c.y - ry), Vec2::new(c.x + rx, c.y - ky), Vec2::new(c.x + rx, c.y));
        p.cubic_to(Vec2::new(c.x + rx, c.y + ky), Vec2::new(c.x + kx, c.y + ry), Vec2::new(c.x, c.y + ry));
        p.cubic_to(Vec2::new(c.x - kx, c.y + ry), Vec2::new(c.x - rx, c.y + ky), Vec2::new(c.x - rx, c.y));
        p.cubic_to(Vec2::new(c.x - rx, c.y - ky), Vec2::new(c.x - kx, c.y - ry), Vec2::new(c.x, c.y - ry));
        p.close();
        p
    }

    pub fn circle(c: Vec2, r: f64) -> PathData {
        PathData::ellipse(c, r, r)
    }

    /// Append a circular arc around `c` of radius `r` from angle `a0` to `a1` (radians, clockwise from
    /// 12 o'clock), as cubic segments of at most 90°. Assumes the current point is the arc's start
    /// (or `start_with_move` to begin a subpath).
    pub fn arc(&mut self, c: Vec2, r: f64, a0: f64, a1: f64, start_with_move: bool) -> &mut Self {
        let start = Vec2::polar(c, r, a0);
        if start_with_move {
            self.move_to(start);
        } else {
            self.line_to(start);
        }
        let sweep = a1 - a0;
        if r <= 0.0 || sweep == 0.0 {
            return self;
        }
        let n = ((sweep.abs() / (m::PI / 2.0)).ceil() as usize).max(1);
        let step = sweep / n as f64;
        let k = 4.0 / 3.0 * m::tan(step / 4.0);
        for i in 0..n {
            let t0 = a0 + step * i as f64;
            let t1 = t0 + step;
            let p0 = Vec2::polar(c, r, t0);
            let p1 = Vec2::polar(c, r, t1);
            // tangent of clockwise-from-top parameterisation: d/dt (sin t, -cos t) = (cos t, sin t)
            let d0 = Vec2::new(m::cos(t0), m::sin(t0)) * (r * k);
            let d1 = Vec2::new(m::cos(t1), m::sin(t1)) * (r * k);
            self.cubic_to(p0 + d0, p1 - d1, p1);
        }
        self
    }

    /// An annular sector (a pie slice when `r0` = 0): angles clockwise from 12 o'clock.
    pub fn annular_sector(c: Vec2, r0: f64, r1: f64, a0: f64, a1: f64) -> PathData {
        let mut p = PathData::new();
        if (a1 - a0).abs() >= m::TAU - 1e-9 {
            // Full ring: two subpaths with opposite winding.
            p.arc(c, r1, a0, a0 + m::TAU, true).close();
            if r0 > 0.0 {
                p.arc(c, r0, a0 + m::TAU, a0, true).close();
            }
            return p;
        }
        p.arc(c, r1, a0, a1, true);
        if r0 > 0.0 {
            p.arc(c, r0, a1, a0, false);
        } else {
            p.line_to(c);
        }
        p.close();
        p
    }

    pub fn transform(&self, xf: &Affine) -> PathData {
        PathData {
            els: self
                .els
                .iter()
                .map(|e| match *e {
                    PathEl::Move { p } => PathEl::Move { p: xf.apply(p) },
                    PathEl::Line { p } => PathEl::Line { p: xf.apply(p) },
                    PathEl::Quad { c, p } => PathEl::Quad { c: xf.apply(c), p: xf.apply(p) },
                    PathEl::Cubic { c1, c2, p } => PathEl::Cubic { c1: xf.apply(c1), c2: xf.apply(c2), p: xf.apply(p) },
                    PathEl::Close => PathEl::Close,
                })
                .collect(),
        }
    }

    /// Control-point bounds (a conservative superset of the exact bounds).
    pub fn bounds(&self) -> Rect {
        let mut r = Rect::empty();
        for e in &self.els {
            match *e {
                PathEl::Move { p } | PathEl::Line { p } => r = r.include(p),
                PathEl::Quad { c, p } => r = r.include(c).include(p),
                PathEl::Cubic { c1, c2, p } => r = r.include(c1).include(c2).include(p),
                PathEl::Close => {}
            }
        }
        r
    }

    /// Flatten to polylines within `tolerance` (max distance from the curve). Each returned
    /// polyline carries whether it was closed.
    pub fn flatten(&self, tolerance: f64) -> Vec<(Vec<Vec2>, bool)> {
        let tol = tolerance.max(1e-6);
        let mut out: Vec<(Vec<Vec2>, bool)> = Vec::new();
        let mut cur: Vec<Vec2> = Vec::new();
        let mut start = Vec2::ZERO;
        let mut last = Vec2::ZERO;
        let flush = |cur: &mut Vec<Vec2>, out: &mut Vec<(Vec<Vec2>, bool)>, closed: bool| {
            if cur.len() >= 2 || (closed && !cur.is_empty()) {
                out.push((std::mem::take(cur), closed));
            } else {
                cur.clear();
            }
        };
        for e in &self.els {
            match *e {
                PathEl::Move { p } => {
                    flush(&mut cur, &mut out, false);
                    cur.push(p);
                    start = p;
                    last = p;
                }
                PathEl::Line { p } => {
                    if cur.is_empty() {
                        cur.push(last);
                    }
                    cur.push(p);
                    last = p;
                }
                PathEl::Quad { c, p } => {
                    if cur.is_empty() {
                        cur.push(last);
                    }
                    let dd = (last - c * 2.0 + p).len();
                    let n = ((dd / (8.0 * tol)).sqrt().ceil() as usize).clamp(1, 1024);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let mt = 1.0 - t;
                        cur.push(last * (mt * mt) + c * (2.0 * mt * t) + p * (t * t));
                    }
                    last = p;
                }
                PathEl::Cubic { c1, c2, p } => {
                    if cur.is_empty() {
                        cur.push(last);
                    }
                    let d1 = (last - c1 * 2.0 + c2).len();
                    let d2 = (c1 - c2 * 2.0 + p).len();
                    let dd = d1.max(d2);
                    let n = ((dd * 3.0 / (4.0 * tol)).sqrt().ceil() as usize).clamp(1, 1024);
                    for i in 1..=n {
                        let t = i as f64 / n as f64;
                        let mt = 1.0 - t;
                        cur.push(
                            last * (mt * mt * mt) + c1 * (3.0 * mt * mt * t) + c2 * (3.0 * mt * t * t) + p * (t * t * t),
                        );
                    }
                    last = p;
                }
                PathEl::Close => {
                    if !cur.is_empty() {
                        flush(&mut cur, &mut out, true);
                    }
                    last = start;
                }
            }
        }
        flush(&mut cur, &mut out, false);
        out
    }

    /// Total arc length of the flattened path.
    pub fn length(&self, tolerance: f64) -> f64 {
        self.flatten(tolerance)
            .iter()
            .map(|(pts, closed)| {
                let mut l: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
                if *closed && pts.len() > 1 {
                    l += pts[pts.len() - 1].dist(pts[0]);
                }
                l
            })
            .sum()
    }

    /// Reverse the direction of every subpath (for winding control).
    pub fn reversed(&self) -> PathData {
        let mut out = PathData::new();
        for (pts, closed) in self.flatten(0.05) {
            let rev: Vec<Vec2> = pts.into_iter().rev().collect();
            if closed {
                out.els.extend(PathData::polygon(&rev).els);
            } else {
                out.els.extend(PathData::polyline(&rev).els);
            }
        }
        out
    }

    /// Append another path's subpaths.
    pub fn extend(&mut self, o: &PathData) {
        self.els.extend_from_slice(&o.els);
    }
}

/// Signed area of a closed polygon (positive = clockwise on a y-down screen).
pub fn signed_area(pts: &[Vec2]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        s += pts[i].cross(pts[(i + 1) % n]);
    }
    s / 2.0
}

/// `n` points evenly spaced by arc length around a closed polyline.
pub fn resample_closed(pts: &[Vec2], n: usize) -> Vec<Vec2> {
    if pts.is_empty() || n == 0 {
        return vec![pts.first().copied().unwrap_or_default(); n];
    }
    let mut ring: Vec<Vec2> = pts.to_vec();
    if ring.first() != ring.last() {
        ring.push(ring[0]);
    }
    let seg: Vec<f64> = ring.windows(2).map(|w| w[0].dist(w[1])).collect();
    let total: f64 = seg.iter().sum();
    if total <= 1e-12 {
        return vec![ring[0]; n];
    }
    let mut out = Vec::with_capacity(n);
    let (mut i, mut acc) = (0usize, 0.0);
    for k in 0..n {
        let target = total * k as f64 / n as f64;
        while i < seg.len() - 1 && acc + seg[i] < target {
            acc += seg[i];
            i += 1;
        }
        let f = if seg[i] > 0.0 { (target - acc) / seg[i] } else { 0.0 };
        out.push(ring[i].lerp(ring[i + 1], f));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rect_flattens_to_four_corners_plus_start() {
        let p = PathData::rect(Rect::new(0.0, 0.0, 10.0, 20.0));
        let f = p.flatten(0.1);
        assert_eq!(f.len(), 1);
        assert!(f[0].1, "closed");
        let a = signed_area(&f[0].0);
        assert!((a - 200.0).abs() < 1e-9, "clockwise on screen, area {a}");
    }
    #[test]
    fn circle_length_close_to_two_pi_r() {
        let c = PathData::circle(Vec2::new(0.0, 0.0), 10.0);
        let l = c.length(0.001);
        assert!((l - m::TAU * 10.0).abs() < 0.05, "{l}");
    }
    #[test]
    fn sector_is_closed_and_positive() {
        let s = PathData::annular_sector(Vec2::ZERO, 5.0, 10.0, 0.0, m::PI / 2.0);
        let f = s.flatten(0.01);
        assert_eq!(f.len(), 1);
        assert!(signed_area(&f[0].0) > 0.0);
    }
    #[test]
    fn resample_even_spacing() {
        let sq = [Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        let r = resample_closed(&sq, 8);
        assert_eq!(r.len(), 8);
        assert_eq!(r[1], Vec2::new(2.0, 0.0));
    }
}
