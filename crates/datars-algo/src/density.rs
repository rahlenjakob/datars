//! 2D kernel density estimation on a grid (Gaussian kernel, separable blur).

use crate::util::{mag, rect_area_ok};
use datars_math::{m, Rect, Vec2};

/// Gaussian kernel density of `points` (optionally `weights`) sampled on a `w × h` grid over
/// `bounds`, row-major (`out[y * w + x]`), sample `(x, y)` at the centre of its cell — the same
/// convention as [`crate::contour`], so `contour(&density(…), w, h, …)` gives density contours in
/// cell units.
///
/// Values are weight per unit area: integrated over the plane the estimate equals the total weight
/// (so with unit weights, "points per square unit"). `bandwidth` is the kernel's standard deviation
/// in world units (per axis, scaled by the cell size); ≤ 0 or non-finite gives a plain
/// (bilinearly splatted) histogram.
///
/// Method: each point's weight is split bilinearly among the four nearest cell centres, then the
/// grid is blurred by a Gaussian truncated at 3σ, first along x then along y. The grid is padded
/// by the kernel radius while blurring, so points just outside `bounds` still contribute and mass
/// near the edges isn't lost or reflected. Cost O(n + w·h·σ).
///
/// Totality: non-finite points and non-finite or negative weights are skipped; missing weights
/// (a shorter slice) count as 1; an empty grid or degenerate bounds give zeros. Deterministic.
pub fn density(points: &[Vec2], weights: Option<&[f64]>, bandwidth: f64, w: usize, h: usize, bounds: Rect) -> Vec<f64> {
    let Some(cells) = w.checked_mul(h) else { return Vec::new() };
    let mut out = vec![0.0; cells];
    if w == 0 || h == 0 || !rect_area_ok(&bounds) {
        return out;
    }
    let (cw, ch) = (bounds.w / w as f64, bounds.h / h as f64);
    let bw = mag(bandwidth);
    let kx = kernel(bw / cw);
    let ky = kernel(bw / ch);
    let (rx, ry) = (kx.len() / 2, ky.len() / 2);
    let (pw, ph) = (w + 2 * rx, h + 2 * ry);
    let mut grid = vec![0.0; pw * ph];
    for (i, p) in points.iter().enumerate() {
        let wt = match weights {
            Some(ws) => ws.get(i).copied().unwrap_or(1.0),
            None => 1.0,
        };
        if !p.is_finite() || !(wt.is_finite() && wt >= 0.0) || wt == 0.0 {
            continue;
        }
        // Continuous padded-grid coordinates where cell centres sit on integers.
        let gx = (p.x - bounds.x) / cw - 0.5 + rx as f64;
        let gy = (p.y - bounds.y) / ch - 0.5 + ry as f64;
        let (x0, y0) = (gx.floor(), gy.floor());
        let (fx, fy) = (gx - x0, gy - y0);
        for (dx, wx) in [(0.0, 1.0 - fx), (1.0, fx)] {
            for (dy, wy) in [(0.0, 1.0 - fy), (1.0, fy)] {
                let (cx, cy) = (x0 + dx, y0 + dy);
                if cx >= 0.0 && cy >= 0.0 && cx < pw as f64 && cy < ph as f64 && wx * wy > 0.0 {
                    grid[cy as usize * pw + cx as usize] += wt * wx * wy;
                }
            }
        }
    }
    // Blur along x (rows), then along y (columns).
    let mut tmp = vec![0.0; pw * ph];
    for y in 0..ph {
        let row = &grid[y * pw..(y + 1) * pw];
        for x in 0..pw {
            let mut s = 0.0;
            for (k, &kv) in kx.iter().enumerate() {
                let sx = x as isize + k as isize - rx as isize;
                if sx >= 0 && (sx as usize) < pw {
                    s += row[sx as usize] * kv;
                }
            }
            tmp[y * pw + x] = s;
        }
    }
    let area = cw * ch;
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x + rx, y + ry);
            let mut s = 0.0;
            for (k, &kv) in ky.iter().enumerate() {
                let sy = py as isize + k as isize - ry as isize;
                if sy >= 0 && (sy as usize) < ph {
                    s += tmp[sy as usize * pw + px] * kv;
                }
            }
            out[y * w + x] = s / area;
        }
    }
    out
}

/// A normalised Gaussian kernel with standard deviation `sigma` (in cells), truncated at 3σ.
fn kernel(sigma: f64) -> Vec<f64> {
    if !(sigma.is_finite() && sigma > 1e-3) {
        return vec![1.0];
    }
    let r = (3.0 * sigma).ceil().min(10_000.0) as usize;
    let k: Vec<f64> = (0..=2 * r)
        .map(|i| {
            let d = i as f64 - r as f64;
            m::exp(-d * d / (2.0 * sigma * sigma))
        })
        .collect();
    let s: f64 = k.iter().sum();
    k.into_iter().map(|v| v / s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Rng;

    #[test]
    fn integrates_to_total_weight() {
        let mut rng = Rng::new(11);
        let pts: Vec<Vec2> = (0..500).map(|_| Vec2::new(rng.range(30.0, 70.0), rng.range(30.0, 70.0))).collect();
        let b = Rect::new(0.0, 0.0, 100.0, 100.0);
        let g = density(&pts, None, 5.0, 50, 50, b);
        let cell = 2.0 * 2.0;
        let total: f64 = g.iter().map(|v| v * cell).sum();
        assert!((total - 500.0).abs() < 0.5, "{total}");
        let wts = vec![2.0; 500];
        let g2 = density(&pts, Some(&wts), 5.0, 50, 50, b);
        let total2: f64 = g2.iter().map(|v| v * cell).sum();
        assert!((total2 - 1000.0).abs() < 1.0);
    }

    #[test]
    fn single_point_is_a_centred_gaussian() {
        let b = Rect::new(0.0, 0.0, 21.0, 21.0);
        let g = density(&[Vec2::new(10.5, 10.5)], None, 2.0, 21, 21, b);
        let peak = g[10 * 21 + 10];
        assert!(g.iter().all(|&v| v <= peak));
        assert!((g[10 * 21 + 8] - g[10 * 21 + 12]).abs() < 1e-15, "symmetric");
        assert!((g[8 * 21 + 10] - g[10 * 21 + 8]).abs() < 1e-15, "isotropic");
        // Peak of a unit 2D Gaussian with σ = 2: 1 / (2π σ²).
        let expect = 1.0 / (2.0 * std::f64::consts::PI * 4.0);
        assert!((peak - expect).abs() / expect < 0.05, "{peak} vs {expect}");
    }

    #[test]
    fn points_outside_still_contribute_and_degenerate_inputs() {
        let b = Rect::new(0.0, 0.0, 10.0, 10.0);
        let g = density(&[Vec2::new(-1.0, 5.0)], None, 2.0, 10, 10, b);
        assert!(g[5 * 10] > 0.0);
        assert!(density(&[Vec2::ZERO], None, 1.0, 0, 5, b).is_empty());
        assert!(density(&[Vec2::new(f64::NAN, 1.0)], Some(&[f64::NAN]), 1.0, 4, 4, b).iter().all(|&v| v == 0.0));
        let hist = density(&[Vec2::new(0.5, 0.5)], None, 0.0, 10, 10, b);
        assert_eq!(hist[0], 1.0, "no bandwidth: a histogram per unit area");
        assert!(density(&[Vec2::ZERO], None, 1.0, 3, 3, Rect::new(0.0, 0.0, 0.0, 1.0)).iter().all(|&v| v == 0.0));
    }
}
