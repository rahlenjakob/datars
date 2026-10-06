//! The stroker: device-space polylines + a stroke style → closed contours, filled non-zero.
//!
//! A stroke is, by definition, the union of pieces: one rectangle per segment, one join shape per
//! outer corner and one cap per open end. If every piece has the same (positive) orientation, the
//! non-zero fill of all of them together is exactly that union — no inner-join loops can cancel
//! into holes, whatever the segment lengths or turn angles. Rather than emitting the pieces one by
//! one, the stroker writes the boundary chain of their sum: along one side of the polyline (join
//! shapes on outer corners, a detour through the vertex on inner ones), round the end cap, back
//! along the other side, round the start cap. Edges shared by neighbouring pieces cancel in that
//! sum, so the contour has the same winding number everywhere as the pieces — the same fill — with
//! far fewer edges.
//!
//! Round joins and caps use `datars_math::m` trigonometry (deterministic libm), flattened to the
//! same tolerance as curves.

use datars_math::{m, Vec2};
use datars_scene::{Cap, Join};
use std::mem;

/// Points closer than this (device px) are merged before stroking, so near-duplicate points can't
/// produce unstable directions (and miter spikes).
const MERGE_EPS: f64 = 1e-3;
/// Dash patterns that would produce more dashes than this are drawn solid (like browsers do).
const MAX_DASHES: f64 = 1e6;

/// How to stroke, in device pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct StrokeParams {
    pub width: f64,
    pub cap: Cap,
    pub join: Join,
    pub miter_limit: f64,
    pub dash: Option<Vec<f64>>,
    pub dash_offset: f64,
    /// Flattening tolerance for round joins and caps.
    pub tolerance: f64,
}

impl StrokeParams {
    pub fn new(width: f64) -> StrokeParams {
        StrokeParams { width, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None, dash_offset: 0.0, tolerance: 0.1 }
    }
}

/// A set of closed contours stored flat: contour `i` is `pts[ends[i - 1]..ends[i]]`.
#[derive(Clone, Debug, Default)]
pub struct Outline {
    pub pts: Vec<Vec2>,
    pub ends: Vec<usize>,
}

impl Outline {
    pub fn clear(&mut self) {
        self.pts.clear();
        self.ends.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }
    pub fn contours(&self) -> impl Iterator<Item = &[Vec2]> {
        let starts = std::iter::once(0).chain(self.ends.iter().copied());
        starts.zip(self.ends.iter().copied()).map(move |(a, b)| &self.pts[a..b])
    }
    /// Append a convex polygon with positive orientation, reversing it if needed. Degenerate
    /// (zero-area) polygons contribute nothing and are skipped.
    fn convex(&mut self, pts: &[Vec2]) {
        let Some(&last) = pts.last() else { return };
        let mut area = last.cross(pts[0]);
        for w in pts.windows(2) {
            area += w[0].cross(w[1]);
        }
        if area > 0.0 {
            self.pts.extend_from_slice(pts);
        } else if area < 0.0 {
            self.pts.extend(pts.iter().rev());
        } else {
            return;
        }
        self.ends.push(self.pts.len());
    }
}

/// Stroke `polys` (device-space polylines with their closed flags) into `out`.
pub fn stroke(polys: &[(Vec<Vec2>, bool)], params: &StrokeParams, out: &mut Outline) {
    let w = params.width;
    if !(w > 0.0 && w.is_finite()) {
        return;
    }
    let h = w * 0.5;
    let tol = if params.tolerance > 0.0 { params.tolerance } else { 0.1 };
    // Largest angle per arc segment keeping the chord within `tol` of the circle.
    let arc_step = if tol >= h { m::PI / 2.0 } else { (2.0 * m::acos(1.0 - tol / h)).min(m::PI / 2.0) };
    let miter_limit = if params.miter_limit.is_finite() { params.miter_limit } else { 4.0 };
    let mut s = Stroker {
        h,
        cap: params.cap,
        join: params.join,
        miter_limit,
        arc_step,
        out,
        segs: Vec::new(),
        scratch: Vec::new(),
        reversals: Vec::new(),
        start: 0,
    };
    let pattern = params.dash.as_deref().and_then(dash_pattern);
    let mut pts = Vec::new();
    let mut dashes = Vec::new();
    for (poly, closed) in polys {
        if !dedupe(poly, *closed, &mut pts) {
            continue;
        }
        match &pattern {
            Some(pat) if pts.len() >= 2 => {
                dashes.clear();
                if dash(&pts, *closed, pat, params.dash_offset, &mut dashes) {
                    let mut dp = Vec::new();
                    for (d, dclosed, dir) in &dashes {
                        if dedupe(d, *dclosed, &mut dp) {
                            s.polyline(&dp, *dclosed, *dir);
                        }
                    }
                } else {
                    s.polyline(&pts, *closed, Vec2::new(1.0, 0.0));
                }
            }
            _ => s.polyline(&pts, *closed, Vec2::new(1.0, 0.0)),
        }
    }
}

/// Copy `src` into `dst` without non-finite input and with near-duplicate points merged. Returns
/// false if nothing is left to stroke.
fn dedupe(src: &[Vec2], closed: bool, dst: &mut Vec<Vec2>) -> bool {
    dst.clear();
    for &p in src {
        if !p.is_finite() {
            dst.clear();
            return false;
        }
        match dst.last() {
            Some(q) if (p - *q).len2() <= MERGE_EPS * MERGE_EPS => {}
            _ => dst.push(p),
        }
    }
    if closed {
        while dst.len() > 1 && (dst[0] - dst[dst.len() - 1]).len2() <= MERGE_EPS * MERGE_EPS {
            dst.pop();
        }
    }
    !dst.is_empty()
}

/// A usable dash pattern (even length, non-negative, positive total), or `None` for solid — SVG
/// renders invalid patterns as solid lines.
fn dash_pattern(d: &[f64]) -> Option<Vec<f64>> {
    if d.is_empty() || d.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
        return None;
    }
    let mut p = d.to_vec();
    if p.len() % 2 == 1 {
        p.extend_from_slice(d);
    }
    let total: f64 = p.iter().sum();
    (total > 0.0).then_some(p)
}

/// Split a polyline into dashes (open polylines, or the whole closed polyline if the first dash
/// covers it). Each dash carries the direction it started in (for caps on zero-length dashes).
/// Returns false if the pattern would produce too many dashes (draw solid instead).
fn dash(pts: &[Vec2], closed: bool, pat: &[f64], offset: f64, out: &mut Vec<(Vec<Vec2>, bool, Vec2)>) -> bool {
    let n = pts.len();
    let seg = |i: usize| (pts[i], if i + 1 == n { pts[0] } else { pts[i + 1] });
    let nseg = if closed { n } else { n - 1 };
    let total: f64 = pat.iter().sum();
    let length: f64 = (0..nseg).map(|i| (seg(i).1 - seg(i).0).len()).sum();
    if length / total * pat.len() as f64 > MAX_DASHES {
        return false;
    }
    // Where in the pattern the path starts.
    let mut phase = if offset.is_finite() { offset - (offset / total).floor() * total } else { 0.0 };
    if !(phase >= 0.0 && phase < total) {
        phase = 0.0;
    }
    // Skip whole elements before the phase; a zero-length dash exactly at the start still draws.
    let mut idx = 0;
    for _ in 0..2 * pat.len() {
        if phase < pat[idx] || (phase == 0.0 && pat[idx] == 0.0) {
            break;
        }
        phase -= pat[idx];
        idx = (idx + 1) % pat.len();
    }
    let mut rem = (pat[idx] - phase).max(0.0);
    let mut on = idx % 2 == 0;
    let first = seg(0);
    let mut cur_dir = (first.1 - first.0).normalize();
    let mut cur: Vec<Vec2> = if on { vec![pts[0]] } else { Vec::new() };
    let mut broke = false;
    for i in 0..nseg {
        let (a, b) = seg(i);
        let l = (b - a).len();
        if l <= 0.0 {
            continue;
        }
        let dir = (b - a) / l;
        let mut pos = 0.0;
        while l - pos > rem {
            pos += rem;
            let q = a + (b - a) * (pos / l);
            if on {
                cur.push(q);
                out.push((mem::take(&mut cur), false, cur_dir));
            } else {
                cur.clear();
                cur.push(q);
                cur_dir = dir;
            }
            broke = true;
            idx = (idx + 1) % pat.len();
            rem = pat[idx];
            on = !on;
        }
        rem -= l - pos;
        if on {
            cur.push(b);
        }
    }
    if on && !cur.is_empty() {
        if closed && !broke {
            cur.pop(); // the closing point repeats the first
            out.push((cur, true, cur_dir));
        } else {
            out.push((cur, false, cur_dir));
        }
    }
    true
}

/// A segment's unit direction and its normal scaled to half the width (`perp(d) · h`).
#[derive(Clone, Copy, Debug)]
struct Seg {
    d: Vec2,
    n: Vec2,
}

struct Stroker<'a> {
    h: f64,
    cap: Cap,
    join: Join,
    miter_limit: f64,
    arc_step: f64,
    out: &'a mut Outline,
    segs: Vec<Seg>,
    scratch: Vec<Vec2>,
    /// Round joins at full reversals, emitted as separate pieces after the current polyline.
    reversals: Vec<(Vec2, Seg)>,
    /// Where the contour being written starts in `out.pts`.
    start: usize,
}

impl Stroker<'_> {
    /// Stroke one deduplicated polyline. `dir` orients square caps of a zero-length one.
    ///
    /// Open: one contour — forward along the −n side, round the end cap, back along the +n side,
    /// round the start cap. Closed: one contour per side. (For a single segment with butt caps
    /// this is `[a − n, b − n, b, b + n, a + n, a]`: the segment's rectangle, positive.)
    fn polyline(&mut self, p: &[Vec2], closed: bool, dir: Vec2) {
        let k = p.len();
        if k == 1 {
            self.dot(p[0], dir);
            return;
        }
        let nseg = if closed { k } else { k - 1 };
        self.segs.clear();
        for i in 0..nseg {
            let b = if i + 1 == k { p[0] } else { p[i + 1] };
            let d = (b - p[i]).normalize();
            self.segs.push(Seg { d, n: d.perp() * self.h });
        }
        self.reversals.clear();
        if closed {
            for forward in [true, false] {
                self.begin();
                for j in 0..k {
                    let i = if forward { j } else { k - 1 - j };
                    let (u, v) = (self.segs[if i == 0 { nseg - 1 } else { i - 1 }], self.segs[i]);
                    self.vertex(p[i], u, v, forward);
                }
                self.end();
            }
        } else {
            let (first, last) = (self.segs[0], self.segs[nseg - 1]);
            let (a, b) = (p[0], p[k - 1]);
            self.begin();
            self.push(a - first.n);
            for (i, &q) in p.iter().enumerate().take(k - 1).skip(1) {
                self.vertex(q, self.segs[i - 1], self.segs[i], true);
            }
            self.push(b - last.n);
            self.cap(b, last, false);
            self.push(b + last.n);
            for (i, &q) in p.iter().enumerate().take(k - 1).skip(1).rev() {
                self.vertex(q, self.segs[i - 1], self.segs[i], false);
            }
            self.push(a + first.n);
            self.cap(a, first, true);
            self.end();
        }
        for i in 0..self.reversals.len() {
            let (c, s) = self.reversals[i];
            self.half_circle(c, s.n, s.d);
        }
    }

    fn begin(&mut self) {
        self.start = self.out.pts.len();
    }

    #[inline]
    fn push(&mut self, q: Vec2) {
        self.out.pts.push(q);
    }

    fn end(&mut self) {
        if self.out.pts.len() - self.start >= 3 {
            self.out.ends.push(self.out.pts.len());
        } else {
            self.out.pts.truncate(self.start);
        }
    }

    /// The contour around vertex `p` (incoming segment `u`, outgoing `v`): forward along the −n
    /// side (`p − u.n … p − v.n`) or backward along the +n side (`p + v.n … p + u.n`). The outer
    /// side gets the join shape; the inner side detours through `p`, which is where the two
    /// segment rectangles' end edges meet.
    fn vertex(&mut self, p: Vec2, u: Seg, v: Seg, forward: bool) {
        let cross = u.d.cross(v.d);
        // The outer side is opposite the turn: −n for cross > 0, +n for cross < 0.
        let outer = if forward { cross > 0.0 } else { cross < 0.0 };
        let (from, to) = if forward { (p - u.n, p - v.n) } else { (p + v.n, p + u.n) };
        self.push(from);
        if outer {
            self.scratch.clear();
            self.join_shape(p, u, v, cross);
            if forward {
                self.out.pts.extend_from_slice(&self.scratch);
            } else {
                self.out.pts.extend(self.scratch.iter().rev());
            }
        } else {
            self.push(p);
            if forward && cross == 0.0 && u.d.dot(v.d) < 0.0 && self.join == Join::Round {
                self.reversals.push((p, u)); // a full reversal: both sides are "outer"
            }
        }
        self.push(to);
    }

    /// Into `scratch`: the outer join boundary strictly between `p ± u.n` and `p ± v.n`, in that
    /// order.
    fn join_shape(&mut self, p: Vec2, u: Seg, v: Seg, cross: f64) {
        match self.join {
            Join::Bevel => {}
            Join::Miter => {
                // Miter length / width = 1 / cos(turn / 2) = 1 / sqrt((1 + dot) / 2).
                let dot = u.d.dot(v.d);
                if self.miter_limit > 1.0 && 1.0 + dot >= 2.0 / (self.miter_limit * self.miter_limit) {
                    let k = (u.n + v.n) / (1.0 + dot);
                    self.scratch.push(if cross > 0.0 { p - k } else { p + k });
                }
            }
            Join::Round => {
                let (va, vb) = if cross > 0.0 { (-u.n, -v.n) } else { (u.n, v.n) };
                self.arc(p, va, m::atan2(va.cross(vb), va.dot(vb)));
            }
        }
    }

    /// The cap boundary at end point `e` of segment `s`, strictly between the two offset points:
    /// from `e − n` to `e + n` at the end, from `e + n` to `e − n` at the start.
    fn cap(&mut self, e: Vec2, s: Seg, start: bool) {
        let (out_dir, v0) = if start { (-s.d, s.n) } else { (s.d, -s.n) };
        match self.cap {
            Cap::Butt => self.push(e),
            Cap::Square => {
                let ext = out_dir * self.h;
                self.push(e + v0 + ext);
                self.push(e - v0 + ext);
            }
            Cap::Round => {
                self.scratch.clear();
                self.arc(e, v0, if v0.cross(out_dir) > 0.0 { m::PI } else { -m::PI });
                self.out.pts.extend_from_slice(&self.scratch);
            }
        }
    }

    /// A separate half disc at `c` from `c + n` through `c + dir·h` to `c − n`.
    fn half_circle(&mut self, c: Vec2, n: Vec2, dir: Vec2) {
        self.scratch.clear();
        self.scratch.push(c + n);
        self.arc(c, n, if n.cross(dir) > 0.0 { m::PI } else { -m::PI });
        self.scratch.push(c - n);
        self.scratch.push(c);
        let pts = mem::take(&mut self.scratch);
        self.out.convex(&pts);
        self.scratch = pts;
    }

    /// A zero-length subpath: a disc for round caps, a square along `dir` for square caps.
    fn dot(&mut self, c: Vec2, dir: Vec2) {
        let dir = if dir.len2() > 0.0 { dir.normalize() } else { Vec2::new(1.0, 0.0) };
        match self.cap {
            Cap::Butt => {}
            Cap::Square => {
                let (d, n) = (dir * self.h, dir.perp() * self.h);
                self.out.convex(&[c - d - n, c + d - n, c + d + n, c - d + n]);
            }
            Cap::Round => {
                let v = dir.perp() * self.h;
                self.scratch.clear();
                self.scratch.push(c + v);
                self.arc(c, v, m::TAU);
                let pts = mem::take(&mut self.scratch);
                self.out.convex(&pts);
                self.scratch = pts;
            }
        }
    }

    /// Push onto `scratch` the interior points of the arc around `c` starting at offset `v0`
    /// (length h) and sweeping `sweep` radians. End points are excluded: callers place exact ones.
    fn arc(&mut self, c: Vec2, v0: Vec2, sweep: f64) {
        let n = ((sweep.abs() / self.arc_step).ceil() as usize).clamp(1, 1024);
        let a0 = m::atan2(v0.y, v0.x);
        for i in 1..n {
            let (s, co) = m::sin_cos(a0 + sweep * (i as f64 / n as f64));
            self.scratch.push(c + Vec2::new(co, s) * self.h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::path::signed_area;

    fn run(pts: &[Vec2], closed: bool, p: &StrokeParams) -> Outline {
        let mut o = Outline::default();
        stroke(&[(pts.to_vec(), closed)], p, &mut o);
        o
    }

    fn total_area(o: &Outline) -> f64 {
        o.contours().map(signed_area).sum()
    }

    #[test]
    fn every_contour_is_positively_oriented() {
        // The signed area of each contour is the sum of its pieces' areas, so it must be positive
        // whatever the shape — including very short segments and sharp turns.
        let shapes: [&[Vec2]; 3] = [
            &[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(3.0, 2.0)],
            &[Vec2::new(0.0, 0.0), Vec2::new(0.5, 0.1), Vec2::new(0.2, 0.9), Vec2::new(9.0, 1.0), Vec2::new(0.0, 1.5)],
            &[Vec2::new(5.0, 5.0), Vec2::new(-5.0, 5.0), Vec2::new(5.0, 5.2)],
        ];
        for pts in shapes {
            for closed in [false, true] {
                for join in [Join::Miter, Join::Round, Join::Bevel] {
                    for cap in [Cap::Butt, Cap::Round, Cap::Square] {
                        let mut p = StrokeParams::new(3.0);
                        p.join = join;
                        p.cap = cap;
                        let o = run(pts, closed, &p);
                        assert!(!o.is_empty());
                        for c in o.contours() {
                            assert!(signed_area(c) > 0.0, "{join:?} {cap:?} closed={closed}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn outline_area_is_the_sum_of_its_pieces() {
        // Right angle, width 2 (h = 1), segment lengths 10 and 6.
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 6.0)];
        let rects = 2.0 * 10.0 + 2.0 * 6.0;
        let area = |join, cap| {
            let mut p = StrokeParams::new(2.0);
            p.join = join;
            p.cap = cap;
            total_area(&run(&pts, false, &p))
        };
        assert!((area(Join::Bevel, Cap::Butt) - (rects + 0.5)).abs() < 1e-9);
        assert!((area(Join::Miter, Cap::Butt) - (rects + 1.0)).abs() < 1e-9);
        assert!((area(Join::Miter, Cap::Square) - (rects + 1.0 + 4.0)).abs() < 1e-9);
        let round = area(Join::Round, Cap::Round) - rects;
        // Quarter disc + two half discs of r = 1; at r = 1 the 0.1 px tolerance allows 45° steps.
        let want = m::PI / 4.0 + m::PI;
        assert!(round < want && round > want - 0.5, "{round}");
    }

    #[test]
    fn dashes_split_a_line() {
        let mut p = StrokeParams::new(2.0);
        p.dash = Some(vec![4.0, 2.0]);
        let mut out = Vec::new();
        let pat = dash_pattern(p.dash.as_ref().unwrap()).unwrap();
        // A 32 px line: every dash end is exactly representable.
        let line = [Vec2::new(0.0, 0.0), Vec2::new(32.0, 0.0)];
        assert!(dash(&line, false, &pat, 0.0, &mut out));
        let spans: Vec<(f64, f64)> = out.iter().map(|(d, _, _)| (d[0].x, d[d.len() - 1].x)).collect();
        assert_eq!(spans, vec![(0.0, 4.0), (6.0, 10.0), (12.0, 16.0), (18.0, 22.0), (24.0, 28.0), (30.0, 32.0)]);
        // An offset shifts the pattern backwards along the path.
        out.clear();
        assert!(dash(&line, false, &pat, 3.0, &mut out));
        let spans: Vec<(f64, f64)> = out.iter().map(|(d, _, _)| (d[0].x, d[d.len() - 1].x)).collect();
        assert_eq!(spans, vec![(0.0, 1.0), (3.0, 7.0), (9.0, 13.0), (15.0, 19.0), (21.0, 25.0), (27.0, 31.0)]);
        // Through the public API: dashed area = on-length × width.
        let mut p = StrokeParams::new(2.0);
        p.dash = Some(vec![4.0, 2.0]);
        p.dash_offset = 3.0;
        assert!((total_area(&run(&line, false, &p)) - 2.0 * 21.0).abs() < 1e-9);
    }

    #[test]
    fn odd_and_invalid_patterns() {
        assert_eq!(dash_pattern(&[3.0]), Some(vec![3.0, 3.0]));
        assert_eq!(dash_pattern(&[1.0, 2.0, 3.0]), Some(vec![1.0, 2.0, 3.0, 1.0, 2.0, 3.0]));
        assert_eq!(dash_pattern(&[0.0, 0.0]), None);
        assert_eq!(dash_pattern(&[1.0, -1.0]), None);
        assert_eq!(dash_pattern(&[]), None);
    }

    #[test]
    fn closed_path_fully_covered_by_one_dash_stays_closed() {
        let sq = [Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(0.0, 4.0)];
        let mut out = Vec::new();
        assert!(dash(&sq, true, &[100.0, 1.0], 0.0, &mut out));
        assert_eq!(out.len(), 1);
        assert!(out[0].1);
        assert_eq!(out[0].0.len(), 4);
    }

    #[test]
    fn zero_length_subpaths_draw_dots_only_with_caps() {
        let pt = [Vec2::new(5.0, 5.0), Vec2::new(5.0, 5.0)];
        let mut p = StrokeParams::new(4.0);
        assert!(run(&pt, false, &p).is_empty(), "butt caps: nothing");
        p.cap = Cap::Round;
        let area = total_area(&run(&pt, false, &p));
        // An inscribed polygon within the 0.1 px tolerance: slightly under π·r².
        assert!(area < m::PI * 4.0 && area > m::PI * 4.0 - 2.0 * m::PI * 2.0 * 0.1, "{area}");
        p.cap = Cap::Square;
        assert!((total_area(&run(&pt, false, &p)) - 16.0).abs() < 1e-9);
    }

    #[test]
    fn reversal_with_round_join_adds_a_half_disc() {
        let pts = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(4.0, 0.0)];
        let mut p = StrokeParams::new(2.0);
        let bevel = total_area(&run(&pts, false, &p));
        assert!((bevel - 32.0).abs() < 1e-9, "{bevel}");
        p.join = Join::Round;
        let round = total_area(&run(&pts, false, &p));
        assert!(round - bevel > 1.4 && round - bevel < m::PI / 2.0, "{}", round - bevel);
    }

    #[test]
    fn too_many_dashes_fall_back_to_solid() {
        let mut out = Vec::new();
        assert!(!dash(&[Vec2::new(0.0, 0.0), Vec2::new(1e7, 0.0)], false, &[0.5, 0.5], 0.0, &mut out));
    }
}
