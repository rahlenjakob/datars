//! Unit grids (waffles) and integer apportionment.

use crate::util::mag;
use datars_math::{total_cmp, Rect};
use serde::{Deserialize, Serialize};

/// The order a [`waffle`] fills its cells in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaffleOrder {
    /// Left → right along a row, rows top → bottom.
    #[default]
    RowMajor,
    /// Top → bottom along a column, columns left → right.
    ColumnMajor,
}

/// A grid of `cols × rows` cells tiling `rect`, filled with `counts[k]` cells for each category
/// `k` in order. Returns `(category, cell rect)` for every filled cell in fill order.
///
/// Cells tile `rect` exactly (inset them yourself for gaps). Units beyond the grid's capacity are
/// dropped — the output never has more than `cols · rows` cells. To show shares on a 10 × 10
/// grid, turn values into exactly 100 units first with [`apportion`].
pub fn waffle(counts: &[usize], cols: usize, rows: usize, rect: Rect, order: WaffleOrder) -> Vec<(usize, Rect)> {
    let cap = cols.saturating_mul(rows);
    if cap == 0 {
        return Vec::new();
    }
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (x0, y0) = (fin(rect.x), fin(rect.y));
    let (cw, ch) = (mag(rect.w) / cols as f64, mag(rect.h) / rows as f64);
    let mut out = Vec::with_capacity(counts.iter().fold(0usize, |a, &c| a.saturating_add(c)).min(cap));
    let mut k = 0usize;
    'fill: for (cat, &c) in counts.iter().enumerate() {
        for _ in 0..c {
            if k >= cap {
                break 'fill;
            }
            let (col, row) = match order {
                WaffleOrder::RowMajor => (k % cols, k / cols),
                WaffleOrder::ColumnMajor => (k / rows, k % rows),
            };
            out.push((cat, Rect::new(x0 + col as f64 * cw, y0 + row as f64 * ch, cw, ch)));
            k += 1;
        }
    }
    out
}

/// Integer shares of `total` proportional to `weights` (the largest-remainder / Hamilton method):
/// floors first, then the leftover units go to the largest fractional parts (ties: lower index).
/// The result sums to exactly `total` unless every weight is 0 (then all zeros). Non-finite and
/// negative weights count as 0.
///
/// ```
/// # use datars_algo::apportion;
/// assert_eq!(apportion(&[1.0, 1.0, 1.0], 100), vec![34, 33, 33]);
/// ```
pub fn apportion(weights: &[f64], total: usize) -> Vec<usize> {
    let w: Vec<f64> = weights.iter().map(|&v| mag(v)).collect();
    let sum: f64 = w.iter().sum();
    if !(sum > 0.0 && sum.is_finite()) {
        return vec![0; w.len()];
    }
    let quota: Vec<f64> = w.iter().map(|&v| v / sum * total as f64).collect();
    let mut out: Vec<usize> = quota.iter().map(|&q| q.floor() as usize).collect();
    let given: usize = out.iter().sum();
    let mut left = total.saturating_sub(given);
    let mut by_rest: Vec<usize> = (0..w.len()).filter(|&i| w[i] > 0.0).collect();
    by_rest.sort_by(|&a, &b| total_cmp(quota[b] - quota[b].floor(), quota[a] - quota[a].floor()));
    let mut i = 0;
    while left > 0 && !by_rest.is_empty() {
        out[by_rest[i % by_rest.len()]] += 1;
        left -= 1;
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_order_and_counts_cells() {
        let cells = waffle(&[3, 2, 0, 4], 3, 3, Rect::new(0.0, 0.0, 30.0, 30.0), WaffleOrder::RowMajor);
        assert_eq!(cells.len(), 9);
        let cats: Vec<usize> = cells.iter().map(|c| c.0).collect();
        assert_eq!(cats, vec![0, 0, 0, 1, 1, 3, 3, 3, 3]);
        assert_eq!(cells[3].1, Rect::new(0.0, 10.0, 10.0, 10.0));
        let col = waffle(&[2], 3, 3, Rect::new(0.0, 0.0, 30.0, 30.0), WaffleOrder::ColumnMajor);
        assert_eq!(col[1].1, Rect::new(0.0, 10.0, 10.0, 10.0));
    }

    #[test]
    fn caps_at_grid_capacity() {
        let cells = waffle(&[7, 7], 2, 5, Rect::new(0.0, 0.0, 1.0, 1.0), WaffleOrder::RowMajor);
        assert_eq!(cells.len(), 10);
        assert_eq!(cells.iter().filter(|c| c.0 == 1).count(), 3);
        assert!(waffle(&[5], 0, 10, Rect::new(0.0, 0.0, 1.0, 1.0), WaffleOrder::RowMajor).is_empty());
    }

    #[test]
    fn apportion_sums_exactly() {
        let a = apportion(&[33.4, 33.3, 33.3], 100);
        assert_eq!(a.iter().sum::<usize>(), 100);
        let b = apportion(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0], 100);
        assert_eq!(b.iter().sum::<usize>(), 100);
        assert_eq!(apportion(&[0.0, f64::NAN], 10), vec![0, 0]);
        assert_eq!(apportion(&[1.0, 0.0, -1.0], 3), vec![3, 0, 0]);
        assert_eq!(apportion(&[], 3), Vec::<usize>::new());
    }

    #[test]
    fn percentage_waffle() {
        let units = apportion(&[48.2, 31.9, 19.9], 100);
        let cells = waffle(&units, 10, 10, Rect::new(0.0, 0.0, 100.0, 100.0), WaffleOrder::RowMajor);
        assert_eq!(cells.len(), 100);
        let area: f64 = cells.iter().map(|c| c.1.w * c.1.h).sum();
        assert!((area - 10_000.0).abs() < 1e-9);
    }
}
