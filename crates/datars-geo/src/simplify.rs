//! Line and ring simplification: Douglas–Peucker and Visvalingam–Whyatt.
//!
//! Both are expressed as **vertex weights** (topojson's "presimplify"): each vertex gets an
//! importance, and simplifying at tolerance `t` keeps exactly the vertices with weight `> t`. So a
//! geometry is analysed once and filtered cheaply for every zoom band, and TopoJSON arcs can be
//! weighted once and shared by every polygon that uses them (see `topojson::Topology::simplify`).
//!
//! Tolerances are lengths in the coordinate units (degrees for lon/lat data): the perpendicular
//! distance for Douglas–Peucker, and the square root of the effective triangle area for
//! Visvalingam. Rings stay valid: closed, with at least four points (a triangle) whenever the input
//! had them — a ring never collapses to a line or disappears.

use crate::geometry::{Geometry, Polygon};
use datars_math::{m, Vec2};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimplifyMethod {
    /// Keeps the vertices farthest from the simplified line: faithful extremes (capes, peaks).
    #[default]
    DouglasPeucker,
    /// Drops the vertices whose triangle with their neighbours is smallest: smoother, more even
    /// shapes; the usual choice for small-scale cartography.
    Visvalingam,
}

/// Per-vertex importance. A vertex survives simplification at tolerance `t` iff `weight > t`.
/// The endpoints are `+∞` (for a closed ring, the start/end point is kept).
pub fn vertex_weights(pts: &[Vec2], method: SimplifyMethod) -> Vec<f64> {
    match method {
        SimplifyMethod::DouglasPeucker => dp_weights(pts),
        SimplifyMethod::Visvalingam => visvalingam_weights(pts),
    }
}

/// Keep the points whose weight exceeds `tolerance`; if fewer than `min_keep` survive, add the
/// most important remaining ones (ties by position) so shapes keep a minimum vertex count.
pub fn filter_by_weight(pts: &[Vec2], weights: &[f64], tolerance: f64, min_keep: usize) -> Vec<Vec2> {
    let n = pts.len().min(weights.len());
    let mut keep: Vec<bool> = weights[..n].iter().map(|&w| w > tolerance).collect();
    let mut count = keep.iter().filter(|&&k| k).count();
    if count < min_keep.min(n) {
        let mut order: Vec<usize> = (0..n).filter(|&i| !keep[i]).collect();
        order.sort_by(|&a, &b| datars_math::total_cmp(weights[b], weights[a]).then(a.cmp(&b)));
        for i in order {
            if count >= min_keep {
                break;
            }
            keep[i] = true;
            count += 1;
        }
    }
    pts[..n].iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
}

/// Douglas–Peucker on an open polyline (endpoints kept).
pub fn douglas_peucker(pts: &[Vec2], tolerance: f64) -> Vec<Vec2> {
    simplify_line(pts, tolerance, SimplifyMethod::DouglasPeucker)
}

/// Visvalingam–Whyatt on an open polyline (endpoints kept).
pub fn visvalingam(pts: &[Vec2], tolerance: f64) -> Vec<Vec2> {
    simplify_line(pts, tolerance, SimplifyMethod::Visvalingam)
}

/// Simplify an open polyline; never fewer than two points.
pub fn simplify_line(pts: &[Vec2], tolerance: f64, method: SimplifyMethod) -> Vec<Vec2> {
    if pts.len() <= 2 || tolerance.is_nan() || tolerance <= 0.0 {
        return pts.to_vec();
    }
    filter_by_weight(pts, &vertex_weights(pts, method), tolerance, 2)
}

/// Simplify a closed ring (first == last); keeps at least four points (a triangle plus closure)
/// when the input has them, so the ring stays a valid polygon ring.
pub fn simplify_ring(ring: &[Vec2], tolerance: f64, method: SimplifyMethod) -> Vec<Vec2> {
    if ring.len() <= 4 || tolerance.is_nan() || tolerance <= 0.0 {
        return ring.to_vec();
    }
    filter_by_weight(ring, &vertex_weights(ring, method), tolerance, 4)
}

/// Simplify every line and ring of a geometry (points unchanged). Rings are simplified
/// independently — for shared borders that must stay identical between neighbours, simplify a
/// TopoJSON topology instead.
pub fn simplify_geometry(g: &Geometry, tolerance: f64, method: SimplifyMethod) -> Geometry {
    let poly = |p: &Polygon| p.iter().map(|r| simplify_ring(r, tolerance, method)).collect::<Polygon>();
    match g {
        Geometry::Point(_) | Geometry::MultiPoint(_) => g.clone(),
        Geometry::LineString(l) => Geometry::LineString(simplify_line(l, tolerance, method)),
        Geometry::MultiLineString(ls) => Geometry::MultiLineString(ls.iter().map(|l| simplify_line(l, tolerance, method)).collect()),
        Geometry::Polygon(p) => Geometry::Polygon(poly(p)),
        Geometry::MultiPolygon(pp) => Geometry::MultiPolygon(pp.iter().map(poly).collect()),
        Geometry::Collection(gs) => Geometry::Collection(gs.iter().map(|g| simplify_geometry(g, tolerance, method)).collect()),
    }
}

/// The Web-Mercator degrees-per-pixel at `zoom` (512 px tiles) times `pixels`: a tolerance in
/// degrees for lon/lat data rendered at that zoom. Use the finest zoom of a band.
pub fn zoom_tolerance(zoom: f64, pixels: f64) -> f64 {
    pixels * 360.0 / (512.0 * m::exp2(zoom))
}

// ---- Douglas–Peucker -------------------------------------------------------------------------

/// DP at tolerance t keeps a vertex iff its distance *and every ancestor's* exceed t, so the
/// weight is the running minimum down the split tree. Iterative (no recursion depth limits).
fn dp_weights(pts: &[Vec2]) -> Vec<f64> {
    let n = pts.len();
    let mut w = vec![0.0; n];
    if n == 0 {
        return w;
    }
    w[0] = f64::INFINITY;
    w[n - 1] = f64::INFINITY;
    let mut stack = vec![(0usize, n - 1, f64::INFINITY)];
    while let Some((a, b, cap)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (mut best, mut best_d) = (a + 1, -1.0);
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = seg_dist2(*p, pts[a], pts[b]);
            if d > best_d {
                best = i;
                best_d = d;
            }
        }
        let wi = best_d.max(0.0).sqrt().min(cap);
        w[best] = wi;
        stack.push((a, best, wi));
        stack.push((best, b, wi));
    }
    w
}

/// Squared distance from `p` to the segment `a`–`b` (to the point when `a == b`, e.g. the base of
/// a closed ring).
pub(crate) fn seg_dist2(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l2 = d.len2();
    if l2 == 0.0 {
        return (p - a).len2();
    }
    let t = ((p - a).dot(d) / l2).clamp(0.0, 1.0);
    (p - (a + d * t)).len2()
}

// ---- Visvalingam -----------------------------------------------------------------------------

#[derive(PartialEq)]
struct Cand {
    area: f64,
    i: usize,
    gen: u32,
}
impl Eq for Cand {}
impl Ord for Cand {
    // Min-heap on area, ties by index: deterministic elimination order.
    fn cmp(&self, o: &Self) -> Ordering {
        datars_math::total_cmp(o.area, self.area).then(o.i.cmp(&self.i))
    }
}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

fn tri_area(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    ((b - a).cross(c - a) / 2.0).abs()
}

/// Effective areas with the monotonic fix (an eliminated vertex never outranks one eliminated
/// before it), reported as `sqrt(area)` so tolerances are lengths.
fn visvalingam_weights(pts: &[Vec2]) -> Vec<f64> {
    let n = pts.len();
    let mut w = vec![f64::INFINITY; n];
    if n < 3 {
        return w;
    }
    let mut prev: Vec<usize> = (0..n).map(|i| i.wrapping_sub(1)).collect();
    let mut next: Vec<usize> = (1..=n).collect();
    let mut gen = vec![0u32; n];
    let mut heap = BinaryHeap::with_capacity(n);
    for i in 1..n - 1 {
        heap.push(Cand { area: tri_area(pts[i - 1], pts[i], pts[i + 1]), i, gen: 0 });
    }
    let mut last = 0.0f64;
    while let Some(Cand { area, i, gen: g }) = heap.pop() {
        if g != gen[i] || w[i].is_finite() {
            continue; // stale entry
        }
        let a = area.max(last);
        last = a;
        w[i] = a.sqrt();
        let (p, q) = (prev[i], next[i]);
        next[p] = q;
        prev[q] = p;
        for j in [p, q] {
            if j != 0 && j != n - 1 {
                gen[j] += 1;
                heap.push(Cand { area: tri_area(pts[prev[j]], pts[j], pts[next[j]]), i: j, gen: gen[j] });
            }
        }
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn dp_drops_collinear_and_keeps_corners() {
        let line: Vec<Vec2> = (0..10).map(|i| v(i as f64, 0.0)).collect();
        assert_eq!(douglas_peucker(&line, 0.01), vec![v(0.0, 0.0), v(9.0, 0.0)]);
        let peak = vec![v(0.0, 0.0), v(1.0, 5.0), v(2.0, 0.0)];
        assert_eq!(douglas_peucker(&peak, 0.1).len(), 3);
    }

    #[test]
    fn weights_match_classic_recursive_dp() {
        // A zig-zag with decreasing amplitude: filtering by weight must equal running DP directly.
        let pts: Vec<Vec2> = (0..40).map(|i| v(i as f64, if i % 2 == 0 { 0.0 } else { 10.0 / (1.0 + i as f64) })).collect();
        fn classic(p: &[Vec2], tol: f64) -> Vec<Vec2> {
            fn rec(p: &[Vec2], a: usize, b: usize, tol: f64, keep: &mut [bool]) {
                if b <= a + 1 {
                    return;
                }
                let (mut bi, mut bd) = (a + 1, -1.0);
                for i in a + 1..b {
                    let d = seg_dist2(p[i], p[a], p[b]);
                    if d > bd {
                        bi = i;
                        bd = d;
                    }
                }
                if bd.sqrt() > tol {
                    keep[bi] = true;
                    rec(p, a, bi, tol, keep);
                    rec(p, bi, b, tol, keep);
                }
            }
            let mut keep = vec![false; p.len()];
            keep[0] = true;
            keep[p.len() - 1] = true;
            rec(p, 0, p.len() - 1, tol, &mut keep);
            p.iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
        }
        for tol in [0.05, 0.3, 1.0, 2.5, 6.0] {
            assert_eq!(douglas_peucker(&pts, tol), classic(&pts, tol), "tol {tol}");
        }
    }

    #[test]
    fn visvalingam_removes_small_triangles_first() {
        let line = vec![v(0.0, 0.0), v(1.0, 0.1), v(2.0, 0.0), v(3.0, 3.0), v(4.0, 0.0)];
        let s = visvalingam(&line, 0.5);
        assert!(!s.contains(&v(1.0, 0.1)), "the flat bump goes: {s:?}");
        assert!(s.contains(&v(3.0, 3.0)), "the peak stays");
        // Weights are monotonic in elimination order.
        let w = vertex_weights(&line, SimplifyMethod::Visvalingam);
        assert!(w[1] < w[3]);
    }

    #[test]
    fn rings_never_collapse() {
        // A tiny, finely sampled circle simplified with a huge tolerance stays a triangle.
        let mut ring: Vec<Vec2> = (0..64).map(|i| {
            let a = i as f64 / 64.0 * m::TAU;
            v(m::cos(a), m::sin(a))
        }).collect();
        ring.push(ring[0]);
        for method in [SimplifyMethod::DouglasPeucker, SimplifyMethod::Visvalingam] {
            let s = simplify_ring(&ring, 100.0, method);
            assert_eq!(s.len(), 4, "{method:?}");
            assert_eq!(s.first(), s.last());
            assert!(datars_math::path::signed_area(&s).abs() > 0.1, "non-degenerate triangle");
            let fine = simplify_ring(&ring, 0.01, method);
            assert!(fine.len() > 16 && fine.len() <= ring.len());
        }
    }

    #[test]
    fn geometry_and_zoom_tolerances() {
        let g = Geometry::MultiLineString(vec![vec![v(0.0, 0.0), v(1.0, 0.001), v(2.0, 0.0)]]);
        let s = simplify_geometry(&g, 0.1, SimplifyMethod::DouglasPeucker);
        assert_eq!(s.lines()[0].len(), 2);
        assert!((zoom_tolerance(0.0, 1.0) - 360.0 / 512.0).abs() < 1e-15);
        assert!((zoom_tolerance(3.0, 2.0) - zoom_tolerance(4.0, 4.0)).abs() < 1e-15);
    }
}
