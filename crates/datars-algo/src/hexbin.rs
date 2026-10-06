//! Hexagonal binning (the d3-hexbin lattice: pointy-top hexagons, odd rows shifted half a column).

use datars_math::{m, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One non-empty hexagonal bin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hexbin {
    /// Centre of the hexagon.
    pub center: Vec2,
    /// Indices of the points in it, ascending.
    pub indices: Vec<usize>,
}

/// Bins `points` into pointy-top hexagons of circumradius `radius` on a lattice anchored at
/// `bounds`' top-left corner (row 0's centres lie on `bounds.y`, column 0's on `bounds.x`).
///
/// Only points inside `bounds` (edges included) are binned; non-finite points are skipped.
/// Returns the non-empty bins ordered by row then column (top to bottom, left to right) — a
/// deterministic order independent of the input order. Each point lands in the hexagon whose
/// centre is nearest. Empty for a non-positive radius.
pub fn hexbin(points: &[Vec2], radius: f64, bounds: Rect) -> Vec<Hexbin> {
    if !(radius.is_finite() && radius > 0.0) {
        return Vec::new();
    }
    let dx = radius * 3f64.sqrt();
    let dy = radius * 1.5;
    let mut bins: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, p) in points.iter().enumerate() {
        if !p.is_finite() || !bounds.contains(*p) {
            continue;
        }
        // The nearest centre lies in one of the two rows bracketing the point, and within a row
        // at the rounded column. (d3-hexbin compares distances in lattice units, which are
        // anisotropic — √3·r across, 1.5·r down — and misassigns a sliver near bin corners; this
        // compares true distances.)
        let (x, y) = (p.x - bounds.x, p.y - bounds.y);
        let row0 = (y / dy).floor();
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for j in [row0, row0 + 1.0] {
            let shift = m::rem_euclid(j, 2.0) / 2.0;
            let col = (x / dx - shift + 0.5).floor();
            let (cx, cy) = ((col + shift) * dx, j * dy);
            let d = (x - cx) * (x - cx) + (y - cy) * (y - cy);
            if d < best.0 {
                best = (d, col, j);
            }
        }
        bins.entry((best.2 as i64, best.1 as i64)).or_default().push(i);
    }
    bins.into_iter()
        .map(|((j, i), indices)| {
            let odd = j.rem_euclid(2) as f64;
            Hexbin { center: Vec2::new(bounds.x + (i as f64 + odd / 2.0) * dx, bounds.y + j as f64 * dy), indices }
        })
        .collect()
}

/// The six corners of a pointy-top hexagon of circumradius `radius` centred on the origin
/// (clockwise on a y-down screen, starting at the top). Add a bin's centre to draw it.
pub fn hexagon(radius: f64) -> [Vec2; 6] {
    let mut out = [Vec2::ZERO; 6];
    for (k, c) in out.iter_mut().enumerate() {
        *c = Vec2::polar(Vec2::ZERO, radius, k as f64 * m::PI / 3.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Rng;

    #[test]
    fn every_point_in_its_nearest_hexagon() {
        let mut rng = Rng::new(3);
        let b = Rect::new(10.0, 20.0, 300.0, 200.0);
        let pts: Vec<Vec2> = (0..2000).map(|_| Vec2::new(rng.range(10.0, 310.0), rng.range(20.0, 220.0))).collect();
        let r = 12.0;
        let bins = hexbin(&pts, r, b);
        let total: usize = bins.iter().map(|h| h.indices.len()).sum();
        assert_eq!(total, pts.len());
        let centers: Vec<Vec2> = bins.iter().map(|h| h.center).collect();
        for h in &bins {
            for &i in &h.indices {
                let d = pts[i].dist(h.center);
                assert!(d <= r + 1e-9, "inside the hexagon's circumcircle");
                for c in &centers {
                    assert!(d <= pts[i].dist(*c) + 1e-9, "nearest centre");
                }
            }
        }
        // Row-major order.
        for w in bins.windows(2) {
            let (a, c) = (w[0].center, w[1].center);
            assert!(a.y < c.y - 1e-9 || ((a.y - c.y).abs() < 1e-9 && a.x < c.x));
        }
    }

    #[test]
    fn lattice_anchor_and_filters() {
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let bins = hexbin(&[Vec2::new(0.0, 0.0), Vec2::new(0.1, 0.1), Vec2::new(-5.0, 0.0), Vec2::new(f64::NAN, 1.0)], 10.0, b);
        assert_eq!(bins.len(), 1);
        assert_eq!(bins[0].center, Vec2::new(0.0, 0.0));
        assert_eq!(bins[0].indices, vec![0, 1]);
        assert!(hexbin(&[Vec2::ZERO], 0.0, b).is_empty());
        let hx = hexagon(2.0);
        assert!((hx[0].y + 2.0).abs() < 1e-12 && hx[0].x.abs() < 1e-12, "top corner first");
    }
}
