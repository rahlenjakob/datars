//! Stacking series on top of each other (stacked bars and areas, 100 % stacks, streamgraphs).

use crate::util::finite;
use datars_math::total_cmp;
use serde::{Deserialize, Serialize};

/// Where each column's baseline goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackOffset {
    /// Baseline at zero. Diverging: positive values stack upward from 0, negative values stack
    /// downward from 0, each in stack order.
    #[default]
    Zero,
    /// Like `Zero`, after dividing each column by the sum of its absolute values: an all-positive
    /// column spans exactly `[0, 1]` (a 100 % stack); a mixed one spans `[-q, p]` with `p + q = 1`.
    Expand,
    /// Centred on zero (the "ThemeRiver" baseline). Negative values count as 0.
    Silhouette,
    /// Byron & Wattenberg's streamgraph baseline, which minimises the weighted wiggle of the layers.
    /// Starts at 0 in the first column. Negative values count as 0.
    Wiggle,
}

/// The order series are stacked in, from the baseline outward. The output stays indexed by input series.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackOrder {
    /// Input order.
    #[default]
    Input,
    /// Reverse input order.
    Reverse,
    /// Smallest total first (at the baseline).
    Ascending,
    /// Largest total first.
    Descending,
    /// Series ordered by when they peak, placed alternately above and below the middle so the
    /// largest ones sit inside — the classic streamgraph order (pair with `Wiggle`).
    InsideOut,
}

/// The series stacking order for `order` (indices into `series`, baseline first).
///
/// Deterministic: stable sorts with a total order; ties keep input order.
pub fn stack_order(series: &[Vec<f64>], order: StackOrder) -> Vec<usize> {
    let n = series.len();
    let sums: Vec<f64> = series.iter().map(|s| s.iter().map(|&v| finite(v)).sum()).collect();
    let mut idx: Vec<usize> = (0..n).collect();
    match order {
        StackOrder::Input => {}
        StackOrder::Reverse => idx.reverse(),
        StackOrder::Ascending => idx.sort_by(|&a, &b| total_cmp(sums[a], sums[b])),
        StackOrder::Descending => idx.sort_by(|&a, &b| total_cmp(sums[b], sums[a])),
        StackOrder::InsideOut => {
            // Peak position of each series (first maximum), then alternate sides by running total.
            let peak: Vec<usize> = series
                .iter()
                .map(|s| {
                    let mut best = (0usize, f64::NEG_INFINITY);
                    for (j, &v) in s.iter().enumerate() {
                        let v = finite(v);
                        if v > best.1 {
                            best = (j, v);
                        }
                    }
                    best.0
                })
                .collect();
            idx.sort_by_key(|&i| peak[i]);
            let (mut top, mut bottom) = (0.0, 0.0);
            let (mut tops, mut bottoms) = (Vec::new(), Vec::new());
            for &i in &idx {
                if top < bottom {
                    top += sums[i];
                    tops.push(i);
                } else {
                    bottom += sums[i];
                    bottoms.push(i);
                }
            }
            bottoms.reverse();
            bottoms.extend(tops);
            idx = bottoms;
        }
    }
    idx
}

/// Stacks `series[s][j]` (series `s`, position `j`) into `(y0, y1)` extents with `y0 <= y1`.
///
/// The output has the same shape as the input: `out[s].len() == series[s].len()`. Series may have
/// different lengths; a missing value counts as 0 for the others. Non-finite values count as 0.
///
/// * `Zero` / `Expand` are diverging: a negative value's segment hangs below the column's negative
///   running total, so positives and negatives never overlap. A zero value gets an empty segment
///   at the current positive top, so it grows from the right place when animated.
/// * `Silhouette` / `Wiggle` are for non-negative data (streamgraphs); negatives count as 0.
///
/// Every column's segments are contiguous: on each side of the baseline, each segment starts where
/// the previous one in stack order ended.
///
/// Deterministic: fixed summation order (stack order), stable sorts.
pub fn stack(series: &[Vec<f64>], offset: StackOffset, order: StackOrder) -> Vec<Vec<(f64, f64)>> {
    let n = series.len();
    let m = series.iter().map(|s| s.len()).max().unwrap_or(0);
    let ord = stack_order(series, order);
    let value = |s: usize, j: usize| series[s].get(j).copied().map(finite).unwrap_or(0.0);
    let mut out: Vec<Vec<(f64, f64)>> = series.iter().map(|s| vec![(0.0, 0.0); s.len()]).collect();
    let put = |out: &mut Vec<Vec<(f64, f64)>>, s: usize, j: usize, seg: (f64, f64)| {
        if let Some(slot) = out[s].get_mut(j) {
            *slot = seg;
        }
    };
    match offset {
        StackOffset::Zero | StackOffset::Expand => {
            for j in 0..m {
                let scale = if offset == StackOffset::Expand {
                    let abs: f64 = (0..n).map(|s| value(s, j).abs()).sum();
                    if abs > 0.0 {
                        1.0 / abs
                    } else {
                        0.0
                    }
                } else {
                    1.0
                };
                let (mut pos, mut neg) = (0.0f64, 0.0f64);
                for &s in &ord {
                    let v = value(s, j) * scale;
                    if v > 0.0 {
                        put(&mut out, s, j, (pos, pos + v));
                        pos += v;
                    } else if v < 0.0 {
                        put(&mut out, s, j, (neg + v, neg));
                        neg += v;
                    } else {
                        put(&mut out, s, j, (pos, pos));
                    }
                }
            }
        }
        StackOffset::Silhouette | StackOffset::Wiggle => {
            let v = |s: usize, j: usize| value(s, j).max(0.0);
            let mut base = vec![0.0; m];
            if offset == StackOffset::Silhouette {
                for (j, b) in base.iter_mut().enumerate() {
                    let total: f64 = ord.iter().map(|&s| v(s, j)).sum();
                    *b = -total / 2.0;
                }
            } else {
                // d3's stackOffsetWiggle: y_j = y_{j-1} − Σ_i v_ij·(Δv_ij/2 + Σ_{k<i} Δv_kj) / Σ_i v_ij.
                // The step is homogeneous of degree 1 in the values, so compute it on values scaled
                // to ≤ 1 and scale back: the squares can't overflow even for huge inputs.
                let big = (0..n).flat_map(|s| (0..m).map(move |j| (s, j))).map(|(s, j)| v(s, j)).fold(0.0, f64::max);
                let k = if big > 0.0 { 1.0 / big } else { 0.0 };
                let mut y = 0.0;
                for j in 1..m {
                    let (mut s1, mut s2) = (0.0, 0.0);
                    let mut below = 0.0; // Σ_{k<i} Δv_kj in stack order
                    for &s in &ord {
                        let (a, b) = (v(s, j) * k, v(s, j - 1) * k);
                        let d = a - b;
                        s1 += a;
                        s2 += (d / 2.0 + below) * a;
                        below += d;
                    }
                    if s1 != 0.0 {
                        y -= s2 / s1;
                    }
                    base[j] = y * big;
                }
            }
            for (j, &b) in base.iter().enumerate() {
                let mut y = b;
                for &s in &ord {
                    let x = v(s, j);
                    put(&mut out, s, j, (y, y + x));
                    y += x;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn zero_offset_is_contiguous_and_diverging() {
        let s = vec![vec![1.0, -2.0, 3.0], vec![2.0, -1.0, 0.0], vec![-1.0, 4.0, 1.0]];
        let st = stack(&s, StackOffset::Zero, StackOrder::Input);
        assert_eq!(st[0][0], (0.0, 1.0));
        assert_eq!(st[1][0], (1.0, 3.0));
        assert_eq!(st[2][0], (-1.0, 0.0));
        // Column 1: negatives hang below zero in order; the positive starts at 0.
        assert_eq!(st[0][1], (-2.0, 0.0));
        assert_eq!(st[1][1], (-3.0, -2.0));
        assert_eq!(st[2][1], (0.0, 4.0));
        // Zero value: an empty segment at the current positive top.
        assert_eq!(st[1][2], (3.0, 3.0));
        for col in 0..3 {
            for s in &st {
                assert!(s[col].0 <= s[col].1);
            }
        }
    }

    #[test]
    fn expand_spans_unit_interval() {
        let s = vec![vec![1.0, 0.0, 5.0], vec![3.0, 0.0, -5.0]];
        let st = stack(&s, StackOffset::Expand, StackOrder::Input);
        assert!(close(st[1][0].1, 1.0) && close(st[0][0].1, 0.25));
        assert_eq!(st[0][1], (0.0, 0.0), "all-zero column stays empty");
        assert!(close(st[0][2].1 - st[1][2].0, 1.0), "mixed column spans 1 in total");
    }

    #[test]
    fn silhouette_is_centred_and_wiggle_is_contiguous() {
        let s = vec![vec![1.0, 2.0, 3.0], vec![3.0, 2.0, 1.0], vec![2.0, 2.0, 2.0]];
        let st = stack(&s, StackOffset::Silhouette, StackOrder::Input);
        for j in 0..3 {
            let lo = st.iter().map(|x| x[j].0).fold(f64::INFINITY, f64::min);
            let hi = st.iter().map(|x| x[j].1).fold(f64::NEG_INFINITY, f64::max);
            assert!(close(lo, -hi), "centred column {j}");
        }
        let w = stack(&s, StackOffset::Wiggle, StackOrder::InsideOut);
        let ord = stack_order(&s, StackOrder::InsideOut);
        for j in 0..3 {
            for pair in ord.windows(2) {
                assert!(close(w[pair[0]][j].1, w[pair[1]][j].0), "contiguous");
            }
        }
        assert_eq!(w[ord[0]][0].0, 0.0, "wiggle baseline starts at 0");
    }

    #[test]
    fn orders() {
        let s = vec![vec![5.0, 0.0], vec![1.0, 1.0], vec![0.0, 9.0]];
        assert_eq!(stack_order(&s, StackOrder::Ascending), vec![1, 0, 2]);
        assert_eq!(stack_order(&s, StackOrder::Descending), vec![2, 0, 1]);
        assert_eq!(stack_order(&s, StackOrder::Reverse), vec![2, 1, 0]);
        let io = stack_order(&s, StackOrder::InsideOut);
        let mut sorted = io.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    #[test]
    fn ragged_and_nan_inputs() {
        let s = vec![vec![1.0, f64::NAN], vec![2.0], vec![]];
        let st = stack(&s, StackOffset::Zero, StackOrder::Input);
        assert_eq!(st[0].len(), 2);
        assert_eq!(st[1].len(), 1);
        assert!(st[2].is_empty());
        assert_eq!(st[1][0], (1.0, 3.0));
        assert_eq!(st[0][1], (0.0, 0.0));
        assert!(stack(&[], StackOffset::Wiggle, StackOrder::InsideOut).is_empty());
    }
}
