//! Iso-contours of a grid (marching squares with ring assembly).

use datars_math::Vec2;

/// Contour rings of a `w × h` grid (row-major: `grid[y * w + x]`) at each threshold.
///
/// Returns `(threshold, rings)` per threshold, in input order. The rings bound the region where
/// the value is `>= threshold`; they're closed (implicitly), and wound so that outer rings have
/// positive `signed_area` (clockwise on a y-down screen) and holes negative — fill them with either
/// rule and holes come out right. Group them into polygons with [`crate::group_rings`] if needed.
///
/// Coordinates are in pixel space `[0, w] × [0, h]`, sample `(x, y)` sitting at the centre of its
/// pixel `(x + 0.5, y + 0.5)` (d3-contour's convention); map to world with
/// `world = bounds.x + X · bounds.w / w`. Crossings are linearly interpolated between samples.
/// Outside the grid counts as below every threshold, so every ring closes; a region reaching the
/// edge is closed along the grid's outer border. Saddle cells are resolved by the cell's mean value
/// (above: the diagonal regions connect).
///
/// Totality: NaN samples count as below every threshold; a grid shorter than `w · h` is padded
/// with NaN; NaN thresholds, an empty grid, or one too large to index give no rings.
/// Deterministic scan order. O(w · h) per threshold.
pub fn contour(grid: &[f64], w: usize, h: usize, thresholds: &[f64]) -> Vec<(f64, Vec<Vec<Vec2>>)> {
    // The edge-key table is (2w + 3) × (2h + 3) u32 keys: refuse sizes whose keys would overflow.
    let side = |n: usize| n.checked_mul(2).and_then(|k| k.checked_add(3));
    let keys = side(w).zip(side(h)).and_then(|(a, b)| a.checked_mul(b));
    let usable = w > 0 && h > 0 && keys.is_some_and(|k| k < u32::MAX as usize);
    thresholds
        .iter()
        .map(|&t| (t, if usable && !t.is_nan() { rings_at(grid, w, h, t) } else { Vec::new() }))
        .collect()
}

const NONE: u32 = u32::MAX;

fn rings_at(grid: &[f64], w: usize, h: usize, thr: f64) -> Vec<Vec<Vec2>> {
    let value = |x: isize, y: isize| -> f64 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            f64::NEG_INFINITY
        } else {
            grid.get(y as usize * w + x as usize).copied().unwrap_or(f64::NAN)
        }
    };
    let above = |v: f64| v >= thr;
    // Edge-midpoint keys in doubled, padded coordinates: kx ∈ [0, 2(w+1)], ky ∈ [0, 2(h+1)].
    let kw = 2 * (w + 1) + 1;
    let kh = 2 * (h + 1) + 1;
    let key = |kx: usize, ky: usize| (ky * kw + kx) as u32;
    let mut next = vec![NONE; kw * kh];
    let mut starts: Vec<u32> = Vec::new();
    // Segments per case, as (from, to) edges: 0 = top, 1 = right, 2 = bottom, 3 = left. The
    // above-region is on the visual right of each segment (y down), so outer rings run clockwise.
    const T: u8 = 0;
    const R: u8 = 1;
    const B: u8 = 2;
    const L: u8 = 3;
    for j in -1..h as isize {
        for i in -1..w as isize {
            let (tl, tr, br, bl) = (value(i, j), value(i + 1, j), value(i + 1, j + 1), value(i, j + 1));
            let code = (above(tl) as u8) << 3 | (above(tr) as u8) << 2 | (above(br) as u8) << 1 | above(bl) as u8;
            let segs: &[(u8, u8)] = match code {
                0 | 15 => &[],
                1 => &[(L, B)],
                2 => &[(B, R)],
                3 => &[(L, R)],
                4 => &[(R, T)],
                6 => &[(B, T)],
                7 => &[(L, T)],
                8 => &[(T, L)],
                9 => &[(T, B)],
                11 => &[(T, R)],
                12 => &[(R, L)],
                13 => &[(R, B)],
                14 => &[(B, L)],
                5 | 10 => {
                    let centre = above((tl + tr + br + bl) / 4.0);
                    match (code, centre) {
                        (5, true) => &[(L, T), (R, B)],
                        (5, false) => &[(L, B), (R, T)],
                        (10, true) => &[(T, R), (B, L)],
                        _ => &[(T, L), (B, R)],
                    }
                }
                _ => &[],
            };
            let (ci, cj) = ((i + 1) as usize, (j + 1) as usize);
            let edge_key = |e: u8| match e {
                T => key(2 * ci + 1, 2 * cj),
                R => key(2 * ci + 2, 2 * cj + 1),
                B => key(2 * ci + 1, 2 * cj + 2),
                _ => key(2 * ci, 2 * cj + 1),
            };
            for &(a, b) in segs {
                let (ka, kb) = (edge_key(a), edge_key(b));
                next[ka as usize] = kb;
                starts.push(ka);
            }
        }
    }
    // Position of an edge key: interpolate between the two samples the edge joins.
    let point = |k: u32| -> Vec2 {
        let (kx, ky) = (k as usize % kw, k as usize / kw);
        // Padded sample coordinates → grid coordinates (−1 is the padding column/row).
        if kx % 2 == 1 {
            let (x0, y) = (((kx - 1) / 2) as isize - 1, (ky / 2) as isize - 1);
            let t = cross_t(value(x0, y), value(x0 + 1, y), thr);
            Vec2::new(x0 as f64 + 0.5 + t, y as f64 + 0.5)
        } else {
            let (x, y0) = ((kx / 2) as isize - 1, ((ky - 1) / 2) as isize - 1);
            let t = cross_t(value(x, y0), value(x, y0 + 1), thr);
            Vec2::new(x as f64 + 0.5, y0 as f64 + 0.5 + t)
        }
    };
    let mut rings = Vec::new();
    for &s in &starts {
        if next[s as usize] == NONE {
            continue; // already consumed
        }
        let mut ring = Vec::new();
        let mut k = s;
        loop {
            ring.push(point(k));
            let nk = next[k as usize];
            next[k as usize] = NONE;
            if nk == NONE || nk == s {
                break;
            }
            k = nk;
        }
        let ring = drop_axis_runs(ring);
        if ring.len() >= 3 {
            rings.push(ring);
        }
    }
    rings
}

/// Fraction along an edge from the sample with value `a` to the one with `b` where the threshold
/// is crossed (midpoint when either side is outside the grid or NaN).
fn cross_t(a: f64, b: f64, thr: f64) -> f64 {
    if a.is_finite() && b.is_finite() && a != b {
        ((thr - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Removes middle points of axis-aligned runs (the grid border produces one point per cell).
fn drop_axis_runs(ring: Vec<Vec2>) -> Vec<Vec2> {
    let n = ring.len();
    if n < 4 {
        return ring;
    }
    let keep: Vec<bool> = (0..n)
        .map(|i| {
            let (p, c, q) = (ring[(i + n - 1) % n], ring[i], ring[(i + 1) % n]);
            !((p.x == c.x && c.x == q.x) || (p.y == c.y && c.y == q.y))
        })
        .collect();
    let out: Vec<Vec2> = ring.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect();
    if out.len() >= 3 {
        out
    } else {
        ring
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::ring_contains;
    use datars_math::path::signed_area;

    fn radial(n: usize) -> Vec<f64> {
        let c = (n as f64 - 1.0) / 2.0;
        (0..n * n)
            .map(|k| {
                let (x, y) = ((k % n) as f64 - c, (k / n) as f64 - c);
                -(x * x + y * y).sqrt()
            })
            .collect()
    }

    #[test]
    fn radial_field_gives_nested_rings() {
        let n = 41;
        let g = radial(n);
        let out = contour(&g, n, n, &[-15.0, -10.0, -5.0]);
        let centre = Vec2::new(n as f64 / 2.0, n as f64 / 2.0);
        let mut prev_area = f64::INFINITY;
        for (t, rings) in &out {
            assert_eq!(rings.len(), 1, "one ring at {t}");
            let r = &rings[0];
            let a = signed_area(r);
            assert!(a > 0.0, "outer ring clockwise on screen");
            // Close to a circle of radius −t around the centre.
            let expected = std::f64::consts::PI * t * t;
            assert!((a - expected).abs() / expected < 0.05, "area {a} vs {expected}");
            for p in r {
                assert!((p.dist(centre) - (-t)).abs() < 0.2, "on the circle");
            }
            assert!(ring_contains(r, centre));
            assert!(a < prev_area, "higher thresholds nest inside");
            prev_area = a;
        }
        // Nesting: every point of an inner ring lies inside the outer one.
        for w in out.windows(2) {
            for p in &w[1].1[0] {
                assert!(ring_contains(&w[0].1[0], *p));
            }
        }
    }

    #[test]
    fn edge_regions_close_along_the_border_and_holes_wind_backwards() {
        // All above except the centre pixel of a 3×3: an outer square plus a hole.
        let g = vec![1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        let out = contour(&g, 3, 3, &[0.5]);
        let rings = &out[0].1;
        assert_eq!(rings.len(), 2);
        let (outer, hole): (Vec<&Vec<Vec2>>, Vec<&Vec<Vec2>>) = rings.iter().partition(|r| signed_area(r) > 0.0);
        assert_eq!(outer.len(), 1);
        assert_eq!(hole.len(), 1);
        // The outer ring runs along the grid border; like d3-contour, each corner is cut by the
        // half-pixel diagonal of its padding cell (4 × 1/8).
        assert!((signed_area(outer[0]) - 8.5).abs() < 1e-9, "outer ring is the grid border");
        assert!((signed_area(hole[0]) + 0.5).abs() < 1e-9, "a diamond hole around the centre");
        // The whole grid above: one border ring, axis runs reduced to the (cut) corners.
        let full = contour(&[2.0; 12], 4, 3, &[1.0]);
        assert_eq!(full[0].1.len(), 1);
        assert_eq!(full[0].1[0].len(), 8);
        assert!((signed_area(&full[0].1[0]) - 11.5).abs() < 1e-9);
        assert!(full[0].1[0].iter().all(|p| p.x >= 0.0 && p.x <= 4.0 && p.y >= 0.0 && p.y <= 3.0));
    }

    #[test]
    fn saddles_and_degenerate() {
        // Checkerboard 2×2: diagonal above cells; the mean decides whether they join.
        let join = contour(&[1.0, 0.0, 0.0, 1.0], 2, 2, &[0.4]);
        assert_eq!(join[0].1.len(), 1, "mean 0.5 ≥ 0.4: one joined region");
        let split = contour(&[1.0, 0.0, 0.0, 1.0], 2, 2, &[0.6]);
        assert_eq!(split[0].1.len(), 2, "mean 0.5 < 0.6: two islands");
        assert!(contour(&[1.0], 0, 1, &[0.5])[0].1.is_empty());
        assert!(contour(&[0.0; 4], 2, 2, &[1.0])[0].1.is_empty());
        assert!(contour(&[1.0; 4], 2, 2, &[f64::NAN])[0].1.is_empty());
        let nan = contour(&[f64::NAN, 1.0], 2, 2, &[0.5]);
        assert!(nan[0].1.iter().flatten().all(|p| p.is_finite()));
        assert!(contour(&[1.0; 4], 2, 2, &[]).is_empty());
    }
}
