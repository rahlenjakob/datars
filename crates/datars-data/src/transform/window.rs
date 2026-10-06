//! Window functions: per-row values computed over the row's partition in a given order.

use super::{cmp_rows, group_rows, sorted_indices};
use crate::num::{pairwise_sum, sum_non_null};
use crate::table::{Column, Table};
use crate::value::Value;
use crate::DataError;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowOp {
    /// Running sum of the column (nulls count as 0).
    Cumsum,
    /// Competition rank (1, 2, 2, 4) by the order keys — or, without order keys, by the column
    /// descending (largest = 1).
    Rank,
    /// Dense rank (1, 2, 2, 3), same ordering as `Rank`.
    DenseRank,
    /// The column's value `n` rows earlier in the partition (null before the start).
    Lag(usize),
    /// The column's value `n` rows later (null past the end).
    Lead(usize),
    /// Mean of the last `k` non-null values up to and including this row (partial windows at the
    /// start use what is there).
    RollingMean(usize),
    /// The value divided by its partition's (pairwise) total; null when the total is 0.
    ShareOfTotal,
    /// (value − previous) / previous; null for the first row or a zero previous value.
    PctChange,
    /// Sum of the last `k` non-null values up to and including this row (partial windows like
    /// `RollingMean`).
    RollingSum(usize),
    /// Population standard deviation (dividing by n) of the last `k` non-null values: the σ of
    /// volatility bands. A single value has σ = 0.
    RollingStd(usize),
    /// Smallest of the last `k` non-null values (channels, stochastics).
    RollingMin(usize),
    /// Largest of the last `k` non-null values.
    RollingMax(usize),
    /// Exponential moving average with span `k`: α = 2 / (k + 1), seeded with the first non-null
    /// value (pandas' `ewm(span=k, adjust=False)`); a null row repeats the running average.
    Ema(usize),
    /// Running maximum of the non-null values so far (null before the first one): a series'
    /// drawdown is value / running max − 1.
    Cummax,
    /// Running minimum of the non-null values so far.
    Cummin,
    /// The partition's first non-null value, in order, on every row: a series rebased to its
    /// start is value / first.
    First,
    /// The partition's last non-null value, in order, on every row.
    Last,
}

/// Options for [`window_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowOpts {
    /// Rolling windows and the EMA: fewer non-null values than this (in the window, or seen so
    /// far) give null instead of a partial result. 1 (the default) keeps the partial windows at
    /// the start of a series; a 50-row average that must not start early wants 50.
    pub min_periods: usize,
}

impl Default for WindowOpts {
    fn default() -> Self {
        WindowOpts { min_periods: 1 }
    }
}

/// The non-null values among the last `k` up to and including position `p`.
fn last_k(x: &[f64], p: usize, k: usize) -> Vec<f64> {
    x[(p + 1).saturating_sub(k)..=p].iter().copied().filter(|v| !v.is_nan()).collect()
}

/// Adds `as_name` with `op` over `column`, computed per partition (`partition_by` values) with rows
/// in `order_by` order (stable; input order when empty). Output rows stay in input order.
pub fn window(t: &Table, op: WindowOp, column: &str, partition_by: &[&str], order_by: &[(&str, bool)], as_name: &str) -> Result<Table, DataError> {
    window_with(t, op, column, partition_by, order_by, as_name, WindowOpts::default())
}

/// [`window`] with options (a minimum number of values per rolling window).
pub fn window_with(t: &Table, op: WindowOp, column: &str, partition_by: &[&str], order_by: &[(&str, bool)], as_name: &str, opts: WindowOpts) -> Result<Table, DataError> {
    let c = t.require(column)?;
    let min = opts.min_periods.max(1);
    if let WindowOp::RollingMean(0) | WindowOp::RollingSum(0) | WindowOp::RollingStd(0) | WindowOp::RollingMin(0) | WindowOp::RollingMax(0) | WindowOp::Ema(0) = op {
        return Err(DataError::Invalid("window: a rolling window (or EMA span) of 0 rows is empty".into()));
    }
    let parts = if partition_by.is_empty() { vec![(Vec::new(), (0..t.len()).collect())] } else { group_rows(t, partition_by)? };
    let rank_by: Vec<(&str, bool)> = if order_by.is_empty() && matches!(op, WindowOp::Rank | WindowOp::DenseRank) { vec![(column, true)] } else { order_by.to_vec() };
    let key_cols: Vec<(&Column, bool)> = rank_by.iter().map(|(n, d)| Ok((t.require(n)?, *d))).collect::<Result<_, DataError>>()?;
    let mut nums = vec![f64::NAN; t.len()];
    let mut vals = vec![Value::Null; t.len()];
    for (_, rows) in &parts {
        let rows = sorted_indices(t, rows, &rank_by)?;
        let x: Vec<f64> = rows.iter().map(|&i| c.f64_at(i)).collect();
        match op {
            WindowOp::Cumsum => {
                let mut s = 0.0;
                for (p, &i) in rows.iter().enumerate() {
                    if !x[p].is_nan() {
                        s += x[p];
                    }
                    nums[i] = s;
                }
            }
            WindowOp::Rank | WindowOp::DenseRank => {
                let (mut rank, mut dense) = (0usize, 0usize);
                for (p, &i) in rows.iter().enumerate() {
                    if p == 0 || cmp_rows(&key_cols, rows[p - 1], i) != Ordering::Equal {
                        rank = p + 1;
                        dense += 1;
                    }
                    nums[i] = if op == WindowOp::Rank { rank } else { dense } as f64;
                }
            }
            WindowOp::Lag(n) | WindowOp::Lead(n) => {
                for (p, &i) in rows.iter().enumerate() {
                    let q = if let WindowOp::Lag(_) = op { p.checked_sub(n) } else { Some(p + n).filter(|&q| q < rows.len()) };
                    vals[i] = q.map_or(Value::Null, |q| c.get(rows[q]));
                }
            }
            WindowOp::RollingMean(k) | WindowOp::RollingSum(k) | WindowOp::RollingStd(k) | WindowOp::RollingMin(k) | WindowOp::RollingMax(k) => {
                for (p, &i) in rows.iter().enumerate() {
                    let w = last_k(&x, p, k);
                    if w.len() < min {
                        continue; // stays null
                    }
                    let n = w.len() as f64;
                    nums[i] = match op {
                        WindowOp::RollingSum(_) => pairwise_sum(&w),
                        WindowOp::RollingStd(_) => {
                            // Two passes (the mean, then squared deviations): no cancellation.
                            let mean = pairwise_sum(&w) / n;
                            let dev: Vec<f64> = w.iter().map(|v| (v - mean) * (v - mean)).collect();
                            (pairwise_sum(&dev) / n).sqrt()
                        }
                        WindowOp::RollingMin(_) => w.iter().copied().fold(f64::INFINITY, f64::min),
                        WindowOp::RollingMax(_) => w.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                        _ => pairwise_sum(&w) / n,
                    };
                }
            }
            WindowOp::Ema(k) => {
                let alpha = 2.0 / (k as f64 + 1.0);
                let (mut avg, mut seen) = (f64::NAN, 0usize);
                for (p, &i) in rows.iter().enumerate() {
                    if !x[p].is_nan() {
                        avg = if seen == 0 { x[p] } else { alpha * x[p] + (1.0 - alpha) * avg };
                        seen += 1;
                    }
                    nums[i] = if seen >= min { avg } else { f64::NAN };
                }
            }
            WindowOp::Cummax | WindowOp::Cummin => {
                let mut run = f64::NAN;
                for (p, &i) in rows.iter().enumerate() {
                    if !x[p].is_nan() {
                        run = if run.is_nan() {
                            x[p]
                        } else if op == WindowOp::Cummax {
                            run.max(x[p])
                        } else {
                            run.min(x[p])
                        };
                    }
                    nums[i] = run;
                }
            }
            WindowOp::First | WindowOp::Last => {
                let pick = |i: &&usize| !c.get(**i).is_null();
                let at = if op == WindowOp::First { rows.iter().find(pick) } else { rows.iter().rev().find(pick) };
                let v = at.map_or(Value::Null, |&i| c.get(i));
                for &i in &rows {
                    vals[i] = v.clone();
                }
            }
            WindowOp::ShareOfTotal => {
                let total = sum_non_null(&x);
                for (p, &i) in rows.iter().enumerate() {
                    nums[i] = if total == 0.0 { f64::NAN } else { x[p] / total };
                }
            }
            WindowOp::PctChange => {
                for (p, &i) in rows.iter().enumerate() {
                    nums[i] = match p.checked_sub(1).map(|q| x[q]) {
                        Some(prev) if prev != 0.0 && !prev.is_nan() => (x[p] - prev) / prev,
                        _ => f64::NAN,
                    };
                }
            }
        }
    }
    let out = match op {
        WindowOp::Lag(_) | WindowOp::Lead(_) | WindowOp::First | WindowOp::Last => Column::from_values(c.ty(), &vals),
        _ => Column::Num(nums),
    };
    t.clone().with_column(as_name, out)
}
