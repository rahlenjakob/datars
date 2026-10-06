//! Beeswarm: dots at their value along an axis, pushed perpendicular to it until none overlap.

use datars_math::total_cmp;
use serde::{Deserialize, Serialize};

/// Which side(s) of the axis a [`beeswarm`] grows into.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwarmSide {
    /// Both sides: each dot takes the free spot nearest the axis.
    #[default]
    Both,
    /// Only offsets ≥ 0 (dots pile up on one side, e.g. above a baseline once you flip y).
    Positive,
    /// Only offsets ≤ 0.
    Negative,
}

/// Perpendicular positions for equal-radius dots at `positions` along an axis so no two overlap.
///
/// Returns, per dot (indexed like the input), its coordinate perpendicular to the axis:
/// `axis_offset + d`, where `d` is as close to 0 as possible (on the allowed `side`). Dots are
/// placed in order of position (ties: input order); each takes the free spot nearest the axis given
/// the dots already placed. Two dots are compatible when their centres are at least `2·radius`
/// apart, so the perpendicular band a placed dot blocks is exact (`±√(4r² − Δx²)`): no stepping, no
/// search tolerance, the nearest free spot is found by merging the blocked intervals.
///
/// Speed: dots are processed in position order, so the only possible conflicts are the most recent
/// dots within `2·radius` along the axis — a window walked backwards — giving O(n log n + n·k log k)
/// for window size k. 20 000 densely stacked dots take about 0.1 s in a release build.
///
/// Totality: non-finite positions are skipped and get `axis_offset`; a non-positive or non-finite
/// radius gives every dot `axis_offset`. Deterministic (stable sort with a total order; ties
/// between equally near spots go to the positive side).
pub fn beeswarm(positions: &[f64], radius: f64, axis_offset: f64, side: SwarmSide) -> Vec<f64> {
    let n = positions.len();
    let base = if axis_offset.is_finite() { axis_offset } else { 0.0 };
    let mut out = vec![base; n];
    if !(radius.is_finite() && radius > 0.0) {
        return out;
    }
    let diameter = 2.0 * radius;
    let d2 = diameter * diameter;
    // A hair of slack so dots placed at a band's edge never overlap by rounding.
    let slack = diameter * 1e-9;
    let mut order: Vec<usize> = (0..n).filter(|&i| positions[i].is_finite()).collect();
    order.sort_by(|&a, &b| total_cmp(positions[a], positions[b]));
    // Placed dots in position order: (x, offset).
    let mut placed: Vec<(f64, f64)> = Vec::with_capacity(order.len());
    let mut blocked: Vec<(f64, f64)> = Vec::new();
    for &i in &order {
        let x = positions[i];
        blocked.clear();
        for &(px, py) in placed.iter().rev() {
            let dx = x - px;
            if dx >= diameter {
                break;
            }
            let half = (d2 - dx * dx).max(0.0).sqrt() + slack;
            blocked.push((py - half, py + half));
        }
        let d = if blocked.is_empty() { 0.0 } else { nearest_free(&mut blocked, side) };
        placed.push((x, d));
        out[i] = base + d;
    }
    out
}

/// The free point nearest 0 (on the allowed side) outside the union of open intervals.
fn nearest_free(blocked: &mut [(f64, f64)], side: SwarmSide) -> f64 {
    // Only the union matters, so the order of equal intervals can't change the result.
    blocked.sort_unstable_by(|a, b| total_cmp(a.0, b.0));
    // Merge into disjoint open intervals.
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(blocked.len());
    for &(lo, hi) in blocked.iter() {
        match merged.last_mut() {
            Some(last) if lo < last.1 => last.1 = last.1.max(hi),
            _ => merged.push((lo, hi)),
        }
    }
    let inside = |y: f64| merged.iter().find(|&&(lo, hi)| lo < y && y < hi).copied();
    match side {
        SwarmSide::Both => match inside(0.0) {
            None => 0.0,
            Some((lo, hi)) => {
                if hi <= -lo {
                    hi
                } else {
                    lo
                }
            }
        },
        SwarmSide::Positive => {
            let mut y = 0.0;
            for &(lo, hi) in &merged {
                if lo < y && y < hi {
                    y = hi;
                } else if lo >= y {
                    break;
                }
            }
            y
        }
        SwarmSide::Negative => {
            let mut y = 0.0;
            for &(lo, hi) in merged.iter().rev() {
                if lo < y && y < hi {
                    y = lo;
                } else if hi <= y {
                    break;
                }
            }
            y
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Rng;

    fn assert_no_overlap(xs: &[f64], ys: &[f64], r: f64) {
        // Sweep by x so the check itself stays fast.
        let mut idx: Vec<usize> = (0..xs.len()).collect();
        idx.sort_by(|&a, &b| total_cmp(xs[a], xs[b]));
        for (k, &i) in idx.iter().enumerate() {
            for &j in &idx[k + 1..] {
                if xs[j] - xs[i] >= 2.0 * r {
                    break;
                }
                let d = ((xs[i] - xs[j]).powi(2) + (ys[i] - ys[j]).powi(2)).sqrt();
                assert!(d >= 2.0 * r * (1.0 - 1e-9), "dots {i} and {j} overlap: {d}");
            }
        }
    }

    #[test]
    fn identical_values_alternate_sides() {
        let ys = beeswarm(&[5.0, 5.0, 5.0, 5.0, 5.0], 1.0, 100.0, SwarmSide::Both);
        let mut d: Vec<f64> = ys.iter().map(|y| ((y - 100.0) / 2.0).round()).collect();
        d.sort_by(|a, b| total_cmp(*a, *b));
        assert_eq!(d, vec![-2.0, -1.0, 0.0, 1.0, 2.0]);
        assert_no_overlap(&[5.0; 5], &ys, 1.0);
    }

    #[test]
    fn no_overlaps_and_deterministic_on_random_data() {
        let mut rng = Rng::new(42);
        let xs: Vec<f64> = (0..3000).map(|_| {
            // A rough bell curve: sum of uniforms.
            (0..4).map(|_| rng.next_f64()).sum::<f64>() * 250.0
        }).collect();
        for side in [SwarmSide::Both, SwarmSide::Positive, SwarmSide::Negative] {
            let ys = beeswarm(&xs, 3.0, 0.0, side);
            assert_no_overlap(&xs, &ys, 3.0);
            assert_eq!(beeswarm(&xs, 3.0, 0.0, side), ys, "deterministic");
            match side {
                SwarmSide::Positive => assert!(ys.iter().all(|&y| y >= 0.0)),
                SwarmSide::Negative => assert!(ys.iter().all(|&y| y <= 0.0)),
                SwarmSide::Both => {
                    let above = ys.iter().filter(|&&y| y > 0.0).count() as f64;
                    let below = ys.iter().filter(|&&y| y < 0.0).count() as f64;
                    assert!((above - below).abs() / (above + below) < 0.2, "roughly symmetric");
                }
            }
        }
    }

    #[test]
    fn sparse_dots_stay_on_the_axis() {
        let ys = beeswarm(&[0.0, 10.0, 20.0], 1.0, 7.0, SwarmSide::Both);
        assert_eq!(ys, vec![7.0, 7.0, 7.0]);
        // A dot touching a neighbour exactly (Δx = 2r) stays on the axis too.
        let ys = beeswarm(&[0.0, 2.0], 1.0, 0.0, SwarmSide::Both);
        assert_eq!(ys, vec![0.0, 0.0]);
    }

    #[test]
    fn nearest_spot_is_exact() {
        // Second dot 1 unit along from the first (r = 1): must rise by √(4 − 1) = √3.
        let ys = beeswarm(&[0.0, 1.0], 1.0, 0.0, SwarmSide::Positive);
        assert!((ys[1] - 3.0_f64.sqrt()).abs() < 1e-6);
    }

    #[test]
    fn degenerate() {
        assert!(beeswarm(&[], 1.0, 0.0, SwarmSide::Both).is_empty());
        let ys = beeswarm(&[f64::NAN, 1.0, f64::INFINITY, 1.0], 1.0, 3.0, SwarmSide::Both);
        assert_eq!(ys[0], 3.0);
        assert_eq!(ys[2], 3.0);
        assert!((ys[1] - ys[3]).abs() >= 2.0 - 1e-9);
        assert_eq!(beeswarm(&[1.0, 1.0], 0.0, 2.0, SwarmSide::Both), vec![2.0, 2.0]);
        assert_eq!(beeswarm(&[1.0, 1.0], f64::NAN, f64::NAN, SwarmSide::Both), vec![0.0, 0.0]);
    }
}
