//! Numeric building blocks shared by transforms and scales: fixed-order sums, quantiles, "nice"
//! 1/2/5×10ᵏ steps (d3's algorithm, so authors get the ticks they expect), exact decimal steps.

use datars_math::m;

/// Sum in a fixed pairwise tree (blocks of 8 summed left to right, halves combined recursively).
/// The tree depends only on the length, so the result is identical on every target and far more
/// accurate than a running sum for long columns. NaNs propagate; filter nulls first.
pub fn pairwise_sum(xs: &[f64]) -> f64 {
    if xs.len() <= 8 {
        let mut s = 0.0;
        for &x in xs {
            s += x;
        }
        s
    } else {
        let mid = xs.len() / 2;
        pairwise_sum(&xs[..mid]) + pairwise_sum(&xs[mid..])
    }
}

/// Pairwise sum of the non-null (non-NaN) values.
pub fn sum_non_null(xs: &[f64]) -> f64 {
    let v: Vec<f64> = xs.iter().copied().filter(|x| !x.is_nan()).collect();
    pairwise_sum(&v)
}

/// Quantile `p` ∈ [0, 1] of *sorted* values by linear interpolation between closest ranks (R-7,
/// d3's `quantileSorted`). NaN for no values.
pub fn quantile_sorted(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 0 || p.is_nan() {
        return f64::NAN;
    }
    if p <= 0.0 || n < 2 {
        return sorted[0];
    }
    if p >= 1.0 {
        return sorted[n - 1];
    }
    let i = (n - 1) as f64 * p;
    let i0 = i.floor() as usize;
    let (a, b) = (sorted[i0], sorted[(i0 + 1).min(n - 1)]);
    a + (b - a) * (i - i0 as f64)
}

/// The non-null values of a slice, sorted ascending (total order).
pub fn sorted_values(xs: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = xs.iter().copied().filter(|x| !x.is_nan()).collect();
    v.sort_by(|a, b| datars_math::total_cmp(*a, *b));
    v
}

/// 10ᵏ.
pub fn pow10(k: i32) -> f64 {
    m::pow(10.0, k as f64)
}

/// Shortest round-trip text for a number (`3`, `2.5`, `-0` → `0`). Not a formatted label.
pub fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        "0".into()
    } else {
        format!("{v}")
    }
}

/// A step of the form 1, 2 or 5 × 10ᵏ, kept exact: steps below 1 are stored as their (integral)
/// inverse, so tick `i` is `i / inverse` — `3 / 10` is exactly the double `0.3`, while `3 * 0.1`
/// is not. This is d3's `tickIncrement` convention with a named type instead of a negative sign.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Multiply: value(i) = i × step (step ≥ 1).
    Mul(f64),
    /// Divide: value(i) = i ÷ inverse (step = 1/inverse < 1).
    Div(f64),
}

impl Step {
    /// The step as a plain number.
    pub fn size(self) -> f64 {
        match self {
            Step::Mul(s) => s,
            Step::Div(inv) => 1.0 / inv,
        }
    }
    /// The `i`-th multiple, computed exactly as above.
    pub fn at(self, i: f64) -> f64 {
        match self {
            Step::Mul(s) => i * s,
            Step::Div(inv) => i / inv,
        }
    }
    /// The index of the multiple at or below `v` (floor), robust to representation error: the
    /// result satisfies `at(k) <= v < at(k + 1)`.
    pub fn floor_index(self, v: f64) -> f64 {
        let mut k = match self {
            Step::Mul(s) => (v / s).floor(),
            Step::Div(inv) => (v * inv).floor(),
        };
        if self.at(k) > v {
            k -= 1.0;
        } else if self.at(k + 1.0) <= v {
            k += 1.0;
        }
        k
    }
    /// Digits after the decimal point that tick labels at this step need (0 for steps ≥ 1).
    pub fn decimals(self) -> u8 {
        match self {
            Step::Mul(_) => 0,
            // The fewest digits d with 10^d / inverse integral (0.25 → 2, 0.2 → 1).
            Step::Div(inv) => (0..=17u8)
                .find(|&d| {
                    let x = pow10(d as i32) / inv;
                    (x - x.round()).abs() < 1e-9 * x.max(1.0)
                })
                .unwrap_or(17),
        }
    }
    /// From an arbitrary positive step: exact inverse form when `1/step` is (nearly) an integer.
    pub fn from_size(s: f64) -> Step {
        if s >= 1.0 || s <= 0.0 || !s.is_finite() {
            return Step::Mul(s);
        }
        let inv = 1.0 / s;
        if (inv - inv.round()).abs() < 1e-9 * inv {
            Step::Div(inv.round())
        } else {
            Step::Mul(s)
        }
    }
}

/// d3's `tickSpec`: first and last tick index and the step for about `count` ticks over
/// [start, stop] (start ≤ stop). `None` when there is no valid step.
pub fn tick_spec(start: f64, stop: f64, count: f64) -> Option<(f64, f64, Step)> {
    if count.is_nan() || count <= 0.0 || !start.is_finite() || !stop.is_finite() {
        return None;
    }
    let raw = (stop - start) / count;
    if !raw.is_finite() || raw <= 0.0 {
        return None;
    }
    let power = m::log10(raw).floor();
    let error = raw / m::pow(10.0, power);
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    let (mut i1, mut i2, step);
    if power < 0.0 {
        let inv = m::pow(10.0, -power) / factor;
        i1 = (start * inv).round();
        i2 = (stop * inv).round();
        if i1 / inv < start {
            i1 += 1.0;
        }
        if i2 / inv > stop {
            i2 -= 1.0;
        }
        step = Step::Div(inv);
    } else {
        let s = m::pow(10.0, power) * factor;
        i1 = (start / s).round();
        i2 = (stop / s).round();
        if i1 * s < start {
            i1 += 1.0;
        }
        if i2 * s > stop {
            i2 -= 1.0;
        }
        step = Step::Mul(s);
    }
    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }
    Some((i1, i2, step))
}

/// The nice step for about `count` ticks over [start, stop] (either order).
pub fn tick_step(start: f64, stop: f64, count: f64) -> Option<Step> {
    let (a, b) = if stop < start { (stop, start) } else { (start, stop) };
    tick_spec(a, b, count).map(|s| s.2)
}

/// About `count` nice ticks inside [start, stop] (in the order of the arguments), d3's `ticks`.
pub fn ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if count == 0 || !start.is_finite() || !stop.is_finite() {
        return Vec::new();
    }
    if start == stop {
        return vec![start];
    }
    let reverse = stop < start;
    let (a, b) = if reverse { (stop, start) } else { (start, stop) };
    let Some((i1, i2, step)) = tick_spec(a, b, count as f64) else { return Vec::new() };
    if i1.is_nan() || i2.is_nan() || i2 < i1 {
        return Vec::new();
    }
    let n = (i2 - i1 + 1.0) as usize;
    let mut out: Vec<f64> = (0..n).map(|i| step.at(i1 + i as f64)).collect();
    if reverse {
        out.reverse();
    }
    out
}

/// Extend [start, stop] to multiples of the nice step (d3's `linear.nice`), iterating until the
/// step settles. Keeps the direction of the domain.
pub fn nice_domain(start: f64, stop: f64, count: usize) -> (f64, f64) {
    let reverse = stop < start;
    let (mut a, mut b) = if reverse { (stop, start) } else { (start, stop) };
    let mut prev: Option<Step> = None;
    for _ in 0..10 {
        let Some((_, _, step)) = tick_spec(a, b, count as f64) else { break };
        if prev == Some(step) {
            break;
        }
        match step {
            Step::Mul(s) => {
                a = (a / s).floor() * s;
                b = (b / s).ceil() * s;
            }
            Step::Div(inv) => {
                a = (a * inv).floor() / inv;
                b = (b * inv).ceil() / inv;
            }
        }
        prev = Some(step);
    }
    if reverse {
        (b, a)
    } else {
        (a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairwise_is_order_fixed_and_accurate() {
        let xs: Vec<f64> = (0..10_000).map(|i| 0.1 + (i % 7) as f64 * 1e-3).collect();
        let a = pairwise_sum(&xs);
        assert_eq!(a.to_bits(), pairwise_sum(&xs.clone()).to_bits());
        let exact: f64 = (0..10_000).map(|i| (i % 7) as f64).sum::<f64>() * 1e-3 + 1000.0;
        assert!((a - exact).abs() < 1e-9);
        assert_eq!(pairwise_sum(&[]), 0.0);
    }

    #[test]
    fn d3_ticks() {
        assert_eq!(ticks(0.0, 97.0, 5), vec![0.0, 20.0, 40.0, 60.0, 80.0]);
        assert_eq!(ticks(0.0, 1.0, 10), vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]);
        assert_eq!(ticks(1.0, 0.0, 2), vec![1.0, 0.5, 0.0]);
        assert_eq!(ticks(-10.0, 10.0, 4), vec![-10.0, -5.0, 0.0, 5.0, 10.0]);
        assert_eq!(ticks(3.0, 3.0, 5), vec![3.0]);
        assert_eq!(nice_domain(0.0, 97.0, 5), (0.0, 100.0));
        assert_eq!(nice_domain(0.201479, 0.996679, 10), (0.2, 1.0));
        assert_eq!(nice_domain(100.0, 3.5, 10), (100.0, 0.0));
    }

    #[test]
    fn exact_steps() {
        let s = Step::from_size(0.1);
        assert_eq!(s, Step::Div(10.0));
        assert_eq!(s.at(3.0), 0.3);
        assert_eq!(s.floor_index(0.3), 3.0);
        assert_eq!(s.floor_index(0.29999), 2.0);
        assert_eq!(s.decimals(), 1);
        assert_eq!(Step::Div(4.0).decimals(), 2);
        assert_eq!(Step::Div(20.0).decimals(), 2);
        assert_eq!(Step::Div(2.0).decimals(), 1);
        assert_eq!(Step::Mul(5.0).floor_index(-0.1), -1.0);
    }

    #[test]
    fn quantiles() {
        let v = sorted_values(&[3.0, f64::NAN, 1.0, 2.0, 4.0]);
        assert_eq!(v, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(quantile_sorted(&v, 0.5), 2.5);
        assert_eq!(quantile_sorted(&v, 0.0), 1.0);
        assert_eq!(quantile_sorted(&v, 1.0), 4.0);
        assert!(quantile_sorted(&[], 0.5).is_nan());
    }
}
