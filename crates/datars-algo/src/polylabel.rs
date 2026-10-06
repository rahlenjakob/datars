//! Pole of inaccessibility: the interior point farthest from the boundary (Mapbox's polylabel).

use crate::polygon::signed_distance;
use crate::util::rings_bbox;
use datars_math::path::signed_area;
use datars_math::{total_cmp, Vec2};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

struct Cell {
    c: Vec2,
    h: f64,
    d: f64,
    max: f64,
    seq: u64,
}

impl PartialEq for Cell {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Cell {}
impl PartialOrd for Cell {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Cell {
    /// Most promising first; ties broken by creation order so the search is fully deterministic.
    fn cmp(&self, o: &Self) -> Ordering {
        total_cmp(self.max, o.max).then(o.seq.cmp(&self.seq))
    }
}

/// The pole of inaccessibility of the polygon `rings` (even-odd, holes allowed): the interior point
/// farthest from any edge, and that distance — the best spot for a label or a proportional symbol
/// (better than the centroid, which can fall outside concave shapes or inside holes).
///
/// Quadtree search (Agafonkin): cover the bounding box with square cells, then keep splitting the
/// cell with the best potential (`distance at centre + half-diagonal`) until no cell can beat the
/// best found by more than `precision`. The result is within `precision` of optimal.
///
/// Totality: `precision ≤ 0` or non-finite means 1/1000 of the larger bbox side; no usable ring
/// → `(0, 0)` at distance 0; a degenerate polygon returns a vertex at distance 0. Deterministic.
pub fn polylabel(rings: &[Vec<Vec2>], precision: f64) -> (Vec2, f64) {
    let rings: Vec<Vec<Vec2>> =
        rings.iter().map(|r| r.iter().copied().filter(|p| p.is_finite()).collect::<Vec<_>>()).filter(|r| r.len() >= 3).collect();
    let Some((lo, hi)) = rings_bbox(&rings) else { return (Vec2::ZERO, 0.0) };
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    let precision = if precision.is_finite() && precision > 0.0 { precision } else { w.max(h) / 1000.0 };
    let size = w.min(h);
    if size <= 0.0 || precision <= 0.0 || size <= precision {
        return (rings[0][0], 0.0_f64.max(signed_distance(&rings, rings[0][0])));
    }
    let mut seq = 0u64;
    let mut cell = |c: Vec2, h: f64| {
        let d = signed_distance(&rings, c);
        seq += 1;
        Cell { c, h, d, max: d + h * std::f64::consts::SQRT_2, seq }
    };
    // First guesses: the area centroid of the largest ring, and the bbox centre.
    let biggest = rings.iter().max_by(|a, b| total_cmp(signed_area(a).abs(), signed_area(b).abs())).unwrap_or(&rings[0]);
    let mut best = {
        let a = cell(centroid(biggest), 0.0);
        let b = cell((lo + hi) / 2.0, 0.0);
        if b.d > a.d {
            (b.c, b.d)
        } else {
            (a.c, a.d)
        }
    };
    let mut queue = BinaryHeap::new();
    let consider = |k: Cell, queue: &mut BinaryHeap<Cell>, best: &mut (Vec2, f64)| {
        if k.d > best.1 {
            *best = (k.c, k.d);
        }
        if k.max > best.1 + precision {
            queue.push(k);
        }
    };
    let half = size / 2.0;
    let (nx, ny) = ((w / size).ceil() as usize, (h / size).ceil() as usize);
    for i in 0..nx {
        for j in 0..ny {
            let k = cell(Vec2::new(lo.x + i as f64 * size + half, lo.y + j as f64 * size + half), half);
            consider(k, &mut queue, &mut best);
        }
    }
    let mut probes = 0usize;
    while let Some(top) = queue.pop() {
        if top.max - best.1 <= precision || probes > 1_000_000 {
            break;
        }
        let h = top.h / 2.0;
        for (dx, dy) in [(-h, -h), (h, -h), (-h, h), (h, h)] {
            let k = cell(top.c + Vec2::new(dx, dy), h);
            probes += 1;
            consider(k, &mut queue, &mut best);
        }
    }
    if !(best.0.is_finite() && best.1.is_finite()) {
        // Coordinates so large that distances overflow.
        return (rings[0][0], 0.0);
    }
    (best.0, best.1.max(0.0))
}

/// Area centroid of a ring (its first vertex if degenerate).
fn centroid(r: &[Vec2]) -> Vec2 {
    let (mut cx, mut cy, mut a) = (0.0, 0.0, 0.0);
    let n = r.len();
    for i in 0..n {
        let (p, q) = (r[i], r[(i + 1) % n]);
        let f = p.cross(q);
        cx += (p.x + q.x) * f;
        cy += (p.y + q.y) * f;
        a += f * 3.0;
    }
    let c = Vec2::new(cx / a, cy / a);
    if a == 0.0 || !c.is_finite() {
        r[0]
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::contains;

    #[test]
    fn square_centre() {
        let sq = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)]];
        let (p, d) = polylabel(&sq, 0.01);
        assert!(p.dist(Vec2::new(5.0, 5.0)) < 0.05);
        assert!((d - 5.0).abs() < 0.02);
    }

    #[test]
    fn concave_shape_label_is_inside() {
        // A "C": its centroid falls in the mouth, outside the shape.
        let c = vec![vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 20.0),
            Vec2::new(20.0, 20.0),
            Vec2::new(20.0, 80.0),
            Vec2::new(100.0, 80.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ]];
        let (p, d) = polylabel(&c, 0.1);
        assert!(contains(&c, p), "{p:?}");
        assert!(d >= 9.9, "as far as the 20-wide bars allow: {d}");
        assert!(!contains(&c, centroid(&c[0])), "the centroid would have been outside");
    }

    #[test]
    fn avoids_holes() {
        let outer = vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), Vec2::new(100.0, 100.0), Vec2::new(0.0, 100.0)];
        let hole = vec![Vec2::new(20.0, 20.0), Vec2::new(80.0, 20.0), Vec2::new(80.0, 80.0), Vec2::new(20.0, 80.0)];
        let rings = vec![outer, hole];
        let (p, d) = polylabel(&rings, 0.1);
        assert!(contains(&rings, p));
        // Best spot: the corners of the frame, equidistant from the outer edges and the hole's corner:
        // x = (20 − x)·√2 → x = 20·(2 − √2) ≈ 11.716.
        assert!((d - 20.0 * (2.0 - std::f64::consts::SQRT_2)).abs() < 0.1, "{d}");
        assert_eq!(polylabel(&rings, 0.1), (p, d), "deterministic");
    }

    #[test]
    fn degenerate() {
        assert_eq!(polylabel(&[], 1.0), (Vec2::ZERO, 0.0));
        let flat = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(5.0, 0.0)]];
        let (p, d) = polylabel(&flat, 1.0);
        assert!(p.is_finite() && d == 0.0);
        let tri = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(0.0, 3.0)]];
        let (p, d) = polylabel(&tri, f64::NAN);
        assert!(contains(&tri, p));
        assert!((d - 1.0).abs() < 0.01, "inradius of the 3-4-5 triangle: {d}");
    }
}
