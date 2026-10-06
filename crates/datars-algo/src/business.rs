//! Running totals and ratios: waterfall (bridge) extents, funnel stages, Pareto cumulative shares.

use crate::util::{finite, mag};
use serde::{Deserialize, Serialize};

/// Waterfall (bridge) extents: each change floats from the running total.
///
/// Returns `(start, end)` per value: a change `v` spans `(run, run + v)` and moves the running total
/// to `run + v`; a total (`is_total[i]`) spans `(0, run)` and ignores its own value (it *shows* the
/// running total, it doesn't change it). `start > end` for a fall. `is_total` may be shorter than
/// `values` (missing entries are changes). Non-finite changes count as 0.
///
/// ```
/// # use datars_algo::waterfall;
/// let w = waterfall(&[100.0, -30.0, 10.0, 0.0], &[false, false, false, true]);
/// assert_eq!(w, vec![(0.0, 100.0), (100.0, 70.0), (70.0, 80.0), (0.0, 80.0)]);
/// ```
pub fn waterfall(values: &[f64], is_total: &[bool]) -> Vec<(f64, f64)> {
    let mut run = 0.0;
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            if is_total.get(i).copied().unwrap_or(false) {
                (0.0, run)
            } else {
                let a = run;
                run += finite(v);
                (a, run)
            }
        })
        .collect()
}

/// One stage of a [`funnel`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunnelStage {
    /// Left edge of the stage's centred bar.
    pub x0: f64,
    /// Right edge.
    pub x1: f64,
    /// Value divided by the previous stage's value (1 for the first stage; 0 after an empty stage).
    pub of_previous: f64,
    /// Value divided by the first stage's value (0 when the first stage is empty).
    pub of_first: f64,
}

/// Funnel stages: bars centred in `x0..x1`, width ∝ value relative to the largest stage, plus the
/// conversion ratios a funnel labels. Non-finite and negative values count as 0.
pub fn funnel(values: &[f64], x0: f64, x1: f64) -> Vec<FunnelStage> {
    let vals: Vec<f64> = values.iter().map(|&v| mag(v)).collect();
    let max = vals.iter().copied().fold(0.0, f64::max);
    let (x0, x1) = (finite(x0), finite(x1));
    let (c, w) = ((x0 + x1) / 2.0, x1 - x0);
    let ratio = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };
    let first = vals.first().copied().unwrap_or(0.0);
    vals.iter()
        .enumerate()
        .map(|(i, &v)| {
            let half = if max > 0.0 { w * v / max / 2.0 } else { 0.0 };
            FunnelStage {
                x0: c - half,
                x1: c + half,
                of_previous: if i == 0 { 1.0 } else { ratio(v, vals[i - 1]) },
                of_first: ratio(v, first),
            }
        })
        .collect()
}

/// Pareto cumulative share: for each value (in input order), the share of the total covered by it
/// and everything before it, in `[0, 1]`; the last entry is 1 unless the total is 0 (then all are 0).
/// Sort descending first for a classic Pareto chart. Non-finite and negative values count as 0.
pub fn pareto(values: &[f64]) -> Vec<f64> {
    let total: f64 = values.iter().map(|&v| mag(v)).sum();
    let mut acc = 0.0;
    let n = values.len();
    values
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            acc += mag(v);
            if total > 0.0 {
                if i + 1 == n {
                    1.0
                } else {
                    (acc / total).min(1.0)
                }
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waterfall_totals_show_running_sum() {
        let w = waterfall(&[10.0, 5.0, f64::NAN, -20.0, 99.0], &[false, false, false, false, true]);
        assert_eq!(w[1], (10.0, 15.0));
        assert_eq!(w[2], (15.0, 15.0));
        assert_eq!(w[3], (15.0, -5.0));
        assert_eq!(w[4], (0.0, -5.0));
        assert!(waterfall(&[], &[]).is_empty());
        assert_eq!(waterfall(&[1.0], &[true, true]), vec![(0.0, 0.0)]);
    }

    #[test]
    fn funnel_is_centred_and_proportional() {
        let f = funnel(&[100.0, 50.0, 0.0, 10.0], 0.0, 200.0);
        assert_eq!((f[0].x0, f[0].x1), (0.0, 200.0));
        assert_eq!((f[1].x0, f[1].x1), (50.0, 150.0));
        assert_eq!(f[1].of_previous, 0.5);
        assert_eq!(f[2].x1 - f[2].x0, 0.0);
        assert_eq!(f[3].of_previous, 0.0, "after an empty stage");
        assert_eq!(f[3].of_first, 0.1);
        assert!(funnel(&[0.0], 0.0, 1.0).iter().all(|s| s.x0 == 0.5 && s.x1 == 0.5));
    }

    #[test]
    fn pareto_accumulates_to_one() {
        let p = pareto(&[5.0, 3.0, 2.0]);
        assert_eq!(p, vec![0.5, 0.8, 1.0]);
        assert_eq!(pareto(&[0.0, f64::NAN]), vec![0.0, 0.0]);
        assert!(pareto(&[]).is_empty());
    }
}
