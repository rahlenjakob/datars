//! Lines that keep their points: how two open polylines (or two areas' edges) interpolate when
//! their vertices don't simply pair up one by one.
//!
//! A line's vertices are data points, and other marks may sit on them (a line chart's dots).
//! Resampling both lines to a common count by arc length moves every vertex off its data point:
//! the line cuts its corners from the first frame and leaves its dots behind. Here the two lines
//! are *merged* instead: every vertex of both is kept, and each gets a partner on the other line
//! at the same relative place — by x when both run monotonically in x (a series over time: points
//! travel straight up and down), else by arc length — between **anchors**, vertex pairs known to
//! be the same point (the ends; or the vertices under the same key of a marker riding the line,
//! see `build.rs`). A vertex with no partner starts (or ends) on the other line, where it would
//! lie, so a new point grows out of the line instead of appearing beside it.
//!
//! Partners are placed on the other line's *drawn* curve, not its chords. Linear and step lines
//! keep their vertices and their curve (a step through an inserted on-curve point draws the same
//! steps). Smooth curves (monotone-x, Catmull-Rom) are recomputed from their vertices each frame,
//! so an inserted point would bend them: both lines are sampled along their curve pieces instead
//! (at the flattening tolerance) and interpolate as straight polylines, starting and ending on the
//! curves as drawn. Every data vertex is one of the samples, so a marker interpolating between the
//! same two vertices stays on the line in every frame.

use datars_math::{PathEl, Vec2};
use datars_scene::geom::curve_path;
use datars_scene::Curve;

/// A place on a line: piece `seg` (from vertex `seg` to `seg + 1`) at parameter `t` ∈ [0, 1] of
/// its drawn curve (the x fraction for lines and steps).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Loc {
    seg: usize,
    t: f64,
}

fn smooth(c: Curve) -> bool {
    matches!(c, Curve::MonotoneX | Curve::CatmullRom)
}

/// One line as drawn: its vertices, curve, and the cubic of each piece for smooth curves.
pub(crate) struct Line {
    pts: Vec<Vec2>,
    curve: Curve,
    cubics: Vec<[Vec2; 4]>,
}

impl Line {
    pub fn new(pts: &[Vec2], curve: Curve) -> Line {
        let mut cubics = Vec::new();
        if smooth(curve) && pts.len() >= 2 {
            let p = curve_path(pts, curve);
            let mut last = pts[0];
            for el in p.els.iter().skip(1) {
                if let PathEl::Cubic { c1, c2, p } = *el {
                    cubics.push([last, c1, c2, p]);
                    last = p;
                }
            }
            if cubics.len() != pts.len() - 1 {
                cubics.clear();
            }
        }
        let curve = if smooth(curve) && cubics.is_empty() { Curve::Linear } else { curve };
        Line { pts: pts.to_vec(), curve, cubics }
    }

    fn pieces(&self) -> usize {
        self.pts.len().saturating_sub(1)
    }

    /// The loc of vertex `k`.
    fn vertex(&self, k: usize) -> Loc {
        let n = self.pieces();
        if n == 0 {
            Loc { seg: 0, t: 0.0 }
        } else if k >= n {
            Loc { seg: n - 1, t: 1.0 }
        } else {
            Loc { seg: k, t: 0.0 }
        }
    }

    /// The point at a loc, on the drawn curve (vertices exactly).
    fn at(&self, l: Loc) -> Vec2 {
        if self.pts.len() < 2 {
            return self.pts.first().copied().unwrap_or_default();
        }
        let (a, b) = (self.pts[l.seg], self.pts[l.seg + 1]);
        if l.t <= 0.0 {
            return a;
        }
        if l.t >= 1.0 {
            return b;
        }
        let t = l.t;
        match self.curve {
            Curve::Linear => a.lerp(b, t),
            Curve::StepAfter => Vec2::new(a.x + (b.x - a.x) * t, a.y),
            Curve::StepBefore => Vec2::new(a.x + (b.x - a.x) * t, b.y),
            Curve::Step => Vec2::new(a.x + (b.x - a.x) * t, if t <= 0.5 { a.y } else { b.y }),
            Curve::MonotoneX | Curve::CatmullRom => {
                // De Casteljau: a flat piece stays exactly flat.
                let [p0, c1, c2, p1] = self.cubics[l.seg];
                let (q0, q1, q2) = (p0.lerp(c1, t), c1.lerp(c2, t), c2.lerp(p1, t));
                let (r0, r1) = (q0.lerp(q1, t), q1.lerp(q2, t));
                r0.lerp(r1, t)
            }
        }
    }

    /// Uniform-parameter subdivisions of a piece that keep its chords within `tol` of the curve
    /// (the flattener's bound); 1 for straight pieces.
    fn subdivisions(&self, seg: usize, tol: f64) -> usize {
        match self.cubics.get(seg) {
            Some([p0, c1, c2, p1]) => {
                let dd = (*p0 - *c1 * 2.0 + *c2).len().max((*c1 - *c2 * 2.0 + *p1).len());
                let n = (dd * 3.0 / (4.0 * tol.max(1e-9))).sqrt().ceil();
                if n.is_finite() {
                    (n as usize).clamp(1, 256)
                } else {
                    1
                }
            }
            None => 1,
        }
    }

    /// Sample locs along the line (every vertex among them) and each vertex's sample index.
    fn samples(&self, flat: bool, tol: f64) -> (Vec<Loc>, Vec<usize>) {
        let n = self.pts.len();
        let mut locs = Vec::with_capacity(n);
        let mut verts = Vec::with_capacity(n);
        for k in 0..n {
            verts.push(locs.len());
            locs.push(self.vertex(k));
            if k + 1 < n && flat {
                let m = self.subdivisions(k, tol);
                for s in 1..m {
                    locs.push(Loc { seg: k, t: s as f64 / m as f64 });
                }
            }
        }
        (locs, verts)
    }

    /// Solve for the loc between `lo` and `hi` (consecutive samples) whose point has x = `x`.
    fn loc_at_x(&self, lo: Loc, hi: Loc, x: f64) -> Loc {
        let seg = lo.seg;
        let (t0, t1) = (lo.t, if hi.seg == seg { hi.t } else { 1.0 });
        let (x0, x1) = (self.at(Loc { seg, t: t0 }).x, self.at(Loc { seg, t: t1 }).x);
        if (x1 - x0).abs() <= 1e-12 {
            return lo;
        }
        let f = ((x - x0) / (x1 - x0)).clamp(0.0, 1.0);
        if self.cubics.is_empty() || self.curve == Curve::MonotoneX {
            // x is linear in t on straight and step pieces, and on monotone-x cubics (their
            // control points split each piece's x range in thirds).
            return Loc { seg, t: t0 + (t1 - t0) * f };
        }
        let (mut a, mut b) = (t0, t1);
        let rising = x1 > x0;
        for _ in 0..48 {
            let m = 0.5 * (a + b);
            let xm = self.at(Loc { seg, t: m }).x;
            if (xm < x) == rising {
                a = m;
            } else {
                b = m;
            }
        }
        Loc { seg, t: 0.5 * (a + b) }
    }

    /// Cumulative drawn length at each vertex as a fraction of the whole (what a trim measures).
    pub fn vertex_fractions(&self, tol: f64) -> Vec<f64> {
        let n = self.pts.len();
        let mut acc = vec![0.0; n];
        for k in 0..self.pieces() {
            let (a, b) = (self.pts[k], self.pts[k + 1]);
            let len = match self.curve {
                Curve::Linear => a.dist(b),
                Curve::Step | Curve::StepBefore | Curve::StepAfter => (b.x - a.x).abs() + (b.y - a.y).abs(),
                Curve::MonotoneX | Curve::CatmullRom => {
                    let m = self.subdivisions(k, tol);
                    let mut prev = a;
                    let mut l = 0.0;
                    for s in 1..=m {
                        let p = self.at(Loc { seg: k, t: s as f64 / m as f64 });
                        l += prev.dist(p);
                        prev = p;
                    }
                    l
                }
            };
            acc[k + 1] = acc[k] + len;
        }
        let total = acc.last().copied().unwrap_or(0.0);
        if total <= 1e-12 {
            return (0..n).map(|k| if n > 1 { k as f64 / (n - 1) as f64 } else { 0.0 }).collect();
        }
        acc.iter().map(|v| (v / total).clamp(0.0, 1.0)).collect()
    }
}

/// How two lines interpolate: point lists of equal length, and where every vertex goes.
#[derive(Clone, Debug)]
pub(crate) struct LinePlan {
    pub a: Vec<Vec2>,
    pub b: Vec<Vec2>,
    /// The curve of every frame (`None`: the source's curve until halfway, then the target's).
    pub curve: Option<Curve>,
    /// Vertex k of `b`: where it starts, on `a`.
    pub enter_from: Vec<Vec2>,
    /// Vertex k of `a`: where it ends, on `b`.
    pub exit_to: Vec<Vec2>,
    /// The vertices pair one to one, in order (the plain point-wise lerp).
    pub identity: bool,
    /// Each vertex of `a` / of `b`: its index in the merged lists.
    pub va: Vec<usize>,
    pub vb: Vec<usize>,
}

/// One section's parameters: normalized positions of its samples by x (both sides monotone in
/// x) or by chord length.
fn params(pts: &[Vec2], by_x: bool) -> Vec<f64> {
    let n = pts.len();
    if n < 2 {
        return vec![0.0; n];
    }
    if by_x {
        let (x0, x1) = (pts[0].x, pts[n - 1].x);
        return pts.iter().map(|p| ((p.x - x0) / (x1 - x0)).clamp(0.0, 1.0)).collect();
    }
    let mut acc = Vec::with_capacity(n);
    let mut s = 0.0;
    acc.push(0.0);
    for w in pts.windows(2) {
        s += w[0].dist(w[1]);
        acc.push(s);
    }
    if s <= 1e-12 {
        return (0..n).map(|k| k as f64 / (n - 1) as f64).collect();
    }
    acc.iter().map(|v| v / s).collect()
}

/// Monotone in x (either direction, flat steps allowed) with some x extent.
fn monotone_x(pts: &[Vec2]) -> bool {
    let n = pts.len();
    if n < 2 {
        return true;
    }
    let dir = pts[n - 1].x - pts[0].x;
    if dir.abs() <= 1e-9 {
        return false;
    }
    pts.windows(2).all(|w| (w[1].x - w[0].x) * dir >= 0.0)
}

/// The longest run of anchors increasing in both indices (the rest would cross).
fn increasing(mut anchors: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    anchors.sort_unstable();
    anchors.dedup_by_key(|p| p.0);
    // Patience: `tails[l]` = index of the smallest tail of an increasing run of length l + 1.
    let mut tails: Vec<usize> = Vec::new();
    let mut prev: Vec<Option<usize>> = vec![None; anchors.len()];
    for i in 0..anchors.len() {
        let y = anchors[i].1;
        let l = tails.partition_point(|&t| anchors[t].1 < y);
        prev[i] = if l > 0 { Some(tails[l - 1]) } else { None };
        if l == tails.len() {
            tails.push(i);
        } else {
            tails[l] = i;
        }
    }
    let mut out = Vec::with_capacity(tails.len());
    let mut cur = tails.last().copied();
    while let Some(i) = cur {
        out.push(anchors[i]);
        cur = prev[i];
    }
    out.reverse();
    out
}

/// Two lines merged: corresponding locs, and the entry of each line's vertices.
struct Merged {
    a: Vec<Loc>,
    b: Vec<Loc>,
    va: Vec<usize>,
    vb: Vec<usize>,
}

/// Merge two lines into corresponding locs. `anchors`: (vertex of a, vertex of b) pairs that are
/// the same point; the two ends always pair unless an anchor says otherwise.
fn merge(la: &Line, lb: &Line, anchors: &[(usize, usize)], tol: f64) -> Option<Merged> {
    let (na, nb) = (la.pts.len(), lb.pts.len());
    if na == 0 || nb == 0 {
        return None;
    }
    let flat = smooth(la.curve) && smooth(lb.curve);
    let (sa, va) = la.samples(flat, tol);
    let (sb, vb) = lb.samples(flat, tol);
    let pa: Vec<Vec2> = sa.iter().map(|l| la.at(*l)).collect();
    let pb: Vec<Vec2> = sb.iter().map(|l| lb.at(*l)).collect();
    // Section boundaries, in sample indices: the starts, the anchors (increasing in both), the
    // ends. An anchor at one line's start makes a section where that line is a single point.
    let mut bounds: Vec<(usize, usize)> = vec![(0, 0)];
    for &(i, j) in &increasing(anchors.iter().copied().filter(|&(i, j)| i < na && j < nb).collect()) {
        if bounds.last() != Some(&(va[i], vb[j])) {
            bounds.push((va[i], vb[j]));
        }
    }
    let end = (sa.len() - 1, sb.len() - 1);
    if bounds.last() != Some(&end) {
        bounds.push(end);
    }
    let (mut oa, mut ob) = (Vec::with_capacity(sa.len() + sb.len()), Vec::with_capacity(sa.len() + sb.len()));
    let mut entry_a = vec![usize::MAX; sa.len()];
    let mut entry_b = vec![usize::MAX; sb.len()];
    let mut push = |ia: Option<usize>, la_: Loc, ib: Option<usize>, lb_: Loc, oa: &mut Vec<Loc>, ob: &mut Vec<Loc>| {
        if let Some(i) = ia {
            if entry_a[i] == usize::MAX {
                entry_a[i] = oa.len();
            }
        }
        if let Some(j) = ib {
            if entry_b[j] == usize::MAX {
                entry_b[j] = oa.len();
            }
        }
        oa.push(la_);
        ob.push(lb_);
    };
    let (a0, b0) = bounds[0];
    push(Some(a0), sa[a0], Some(b0), sb[b0], &mut oa, &mut ob);
    for w in bounds.windows(2) {
        let ((a0, b0), (a1, b1)) = (w[0], w[1]);
        if a0 == a1 || b0 == b1 {
            // One side is a single point here: the other's samples gather to (or spread from) it.
            for (i, l) in sa.iter().enumerate().take(a1).skip(a0 + 1) {
                push(Some(i), *l, None, sb[b0], &mut oa, &mut ob);
            }
            for (j, l) in sb.iter().enumerate().take(b1).skip(b0 + 1) {
                push(None, sa[a0], Some(j), *l, &mut oa, &mut ob);
            }
        } else {
            let (ca, cb) = (&pa[a0..=a1], &pb[b0..=b1]);
            let by_x = monotone_x(ca) && monotone_x(cb);
            let (qa, qb) = (params(ca, by_x), params(cb, by_x));
            // The loc on one side at parameter q (samples `s`, points `p`, params `par`).
            let locate = |line: &Line, s: &[Loc], p: &[Vec2], par: &[f64], q: f64| -> Loc {
                let k = par.partition_point(|&v| v <= q).clamp(1, par.len() - 1) - 1;
                let (lo, hi) = (s[k], s[k + 1]);
                let span = par[k + 1] - par[k];
                if span <= 1e-15 {
                    return lo;
                }
                if by_x {
                    let (x0, x1) = (p[0].x, p[p.len() - 1].x);
                    line.loc_at_x(lo, hi, x0 + (x1 - x0) * q)
                } else {
                    let t1 = if hi.seg == lo.seg { hi.t } else { 1.0 };
                    Loc { seg: lo.seg, t: lo.t + (t1 - lo.t) * ((q - par[k]) / span) }
                }
            };
            let (mut i, mut j) = (1usize, 1usize);
            let (ia_end, ib_end) = (ca.len() - 1, cb.len() - 1);
            while i < ia_end || j < ib_end {
                let qa_i = if i < ia_end { qa[i] } else { f64::INFINITY };
                let qb_j = if j < ib_end { qb[j] } else { f64::INFINITY };
                if (qa_i - qb_j).abs() <= 1e-9 {
                    push(Some(a0 + i), sa[a0 + i], Some(b0 + j), sb[b0 + j], &mut oa, &mut ob);
                    i += 1;
                    j += 1;
                } else if qa_i < qb_j {
                    let l = locate(lb, &sb[b0..=b1], cb, &qb, qa_i);
                    push(Some(a0 + i), sa[a0 + i], None, l, &mut oa, &mut ob);
                    i += 1;
                } else {
                    let l = locate(la, &sa[a0..=a1], ca, &qa, qb_j);
                    push(None, l, Some(b0 + j), sb[b0 + j], &mut oa, &mut ob);
                    j += 1;
                }
            }
        }
        push(Some(a1), sa[a1], Some(b1), sb[b1], &mut oa, &mut ob);
    }
    Some(Merged { va: va.iter().map(|&s| entry_a[s]).collect(), vb: vb.iter().map(|&s| entry_b[s]).collect(), a: oa, b: ob })
}

/// Plan two open lines (`tol`: flattening tolerance in their units). With no anchors, lines of
/// equal length pair vertex by vertex.
pub(crate) fn plan_lines(a: &[Vec2], ca: Curve, b: &[Vec2], cb: Curve, anchors: &[(usize, usize)], tol: f64) -> Option<LinePlan> {
    let identity = a.len() == b.len() && anchors.iter().all(|&(i, j)| i == j);
    if identity {
        return Some(LinePlan { a: a.to_vec(), b: b.to_vec(), curve: None, enter_from: a.to_vec(), exit_to: b.to_vec(), identity: true, va: (0..a.len()).collect(), vb: (0..b.len()).collect() });
    }
    if a.is_empty() || b.is_empty() {
        // An empty line: the other grows from (or shrinks into) its first point.
        let p = a.first().or(b.first()).copied()?;
        let (pa, pb) = if a.is_empty() { (vec![p; b.len()], b.to_vec()) } else { (a.to_vec(), vec![p; a.len()]) };
        return Some(LinePlan { enter_from: vec![p; b.len()], exit_to: vec![p; a.len()], a: pa, b: pb, curve: None, identity: false, va: (0..a.len()).collect(), vb: (0..b.len()).collect() });
    }
    let (la, lb) = (Line::new(a, ca), Line::new(b, cb));
    let m = merge(&la, &lb, anchors, tol)?;
    let pa: Vec<Vec2> = m.a.iter().map(|l| la.at(*l)).collect();
    let pb: Vec<Vec2> = m.b.iter().map(|l| lb.at(*l)).collect();
    let flat = smooth(la.curve) && smooth(lb.curve);
    let enter_from = m.vb.iter().map(|&e| pa[e]).collect();
    let exit_to = m.va.iter().map(|&e| pb[e]).collect();
    Some(LinePlan { a: pa, b: pb, curve: flat.then_some(Curve::Linear), enter_from, exit_to, identity: false, va: m.va, vb: m.vb })
}

/// Two areas merged ([`plan_areas`]): tops and bases, the curve of every frame, and each top
/// vertex's index in the merged lists.
pub(crate) type AreaPlan = ([Vec<Vec2>; 2], [Vec<Vec2>; 2], Option<Curve>, Vec<usize>, Vec<usize>);

/// Areas: tops merge like lines (with `anchors` on their vertices); bases follow at the same
/// places. `None` for mismatched edges.
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_areas(ta: &[Vec2], ba: &[Vec2], ca: Curve, tb: &[Vec2], bb: &[Vec2], cb: Curve, anchors: &[(usize, usize)], tol: f64) -> Option<AreaPlan> {
    if ta.len() != ba.len() || tb.len() != bb.len() {
        return None;
    }
    let (lta, ltb) = (Line::new(ta, ca), Line::new(tb, cb));
    let (lba, lbb) = (Line::new(ba, ca), Line::new(bb, cb));
    let m = merge(&lta, &ltb, anchors, tol)?;
    let flat = smooth(lta.curve) && smooth(ltb.curve);
    let a = [m.a.iter().map(|l| lta.at(*l)).collect(), m.a.iter().map(|l| lba.at(*l)).collect()];
    let b = [m.b.iter().map(|l| ltb.at(*l)).collect(), m.b.iter().map(|l| lbb.at(*l)).collect()];
    Some((a, b, flat.then_some(Curve::Linear), m.va, m.vb))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(p: &[(f64, f64)]) -> Vec<Vec2> {
        p.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
    }

    fn dist_to_segment(q: Vec2, a: Vec2, b: Vec2) -> f64 {
        let d = b - a;
        let l2 = d.x * d.x + d.y * d.y;
        let t = if l2 > 0.0 { (((q - a).x * d.x + (q - a).y * d.y) / l2).clamp(0.0, 1.0) } else { 0.0 };
        q.dist(a.lerp(b, t))
    }

    #[test]
    fn every_vertex_of_both_lines_is_kept() {
        let a = v(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]);
        let b = v(&[(0.0, 5.0), (5.0, 0.0), (10.0, 5.0), (15.0, 0.0), (20.0, 5.0)]);
        let p = plan_lines(&a, Curve::Linear, &b, Curve::Linear, &[], 0.1).unwrap();
        assert_eq!(p.a.len(), p.b.len());
        for q in &a {
            assert!(p.a.contains(q), "{q:?} kept");
        }
        for q in &b {
            assert!(p.b.contains(q), "{q:?} kept");
        }
        // By x: every partner sits at the same x (points move straight up and down).
        for (x, y) in p.a.iter().zip(&p.b) {
            assert!((x.x - y.x).abs() < 1e-9, "{x:?} → {y:?}");
        }
        assert_eq!(p.enter_from[1], Vec2::new(5.0, 5.0), "b's second vertex starts on a's first piece");
    }

    #[test]
    fn anchors_slide_a_window() {
        // x 1..4 → x 2..5 (the same data points, keyed): anchors pair the shared points.
        let a = v(&[(0.0, 1.0), (10.0, 2.0), (20.0, 3.0), (30.0, 4.0)]);
        let b = v(&[(0.0, 2.0), (10.0, 3.0), (20.0, 4.0), (30.0, 5.0)]);
        let p = plan_lines(&a, Curve::Linear, &b, Curve::Linear, &[(1, 0), (2, 1), (3, 2)], 0.1).unwrap();
        assert!(!p.identity);
        assert_eq!(p.a.len(), 5);
        assert_eq!(p.exit_to[0], b[0], "the first point leaves into the new first vertex");
        assert_eq!(p.enter_from[3], a[3], "the new last point grows from the old end");
        for &(i, j) in &[(1usize, 0usize), (2, 1), (3, 2)] {
            let k = p.a.iter().position(|q| *q == a[i]).unwrap();
            assert_eq!(p.b[k], b[j]);
        }
    }

    #[test]
    fn smooth_lines_are_sampled_on_their_curves() {
        let a = v(&[(0.0, 0.0), (10.0, 10.0), (20.0, 0.0)]);
        let b = v(&[(0.0, 0.0), (5.0, 8.0), (10.0, 0.0), (15.0, 8.0), (20.0, 0.0)]);
        let p = plan_lines(&a, Curve::MonotoneX, &b, Curve::MonotoneX, &[], 0.05).unwrap();
        assert_eq!(p.curve, Some(Curve::Linear));
        // Every sample of `a` lies on a's drawn curve.
        let flat = curve_path(&a, Curve::MonotoneX).flatten(0.01);
        let d = |q: Vec2| flat[0].0.windows(2).map(|w| dist_to_segment(q, w[0], w[1])).fold(f64::INFINITY, f64::min);
        for q in &p.a {
            assert!(d(*q) < 0.02, "{q:?} off the curve by {}", d(*q));
        }
    }

    #[test]
    fn single_and_empty_lines() {
        let b = v(&[(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)]);
        let p = plan_lines(&[], Curve::Linear, &b, Curve::MonotoneX, &[], 0.1).unwrap();
        assert!(p.a.iter().all(|q| *q == b[0]) && p.b == b, "grows from its first point");
        let one = v(&[(5.0, 5.0)]);
        let p = plan_lines(&one, Curve::Linear, &b, Curve::Linear, &[], 0.1).unwrap();
        assert_eq!(p.a.len(), p.b.len());
        assert!(p.a.iter().all(|q| *q == one[0]) && p.b == b);
    }

    #[test]
    fn crossing_anchors_are_dropped() {
        assert_eq!(increasing(vec![(0, 0), (1, 3), (2, 1), (3, 2), (4, 4)]), vec![(0, 0), (2, 1), (3, 2), (4, 4)]);
    }
}
