//! 1-D kernel density estimation (Gaussian kernel): the smooth outline of a distribution for
//! violins, ridgelines and density curves.

use datars_math::{m, total_cmp};

/// Gaussian kernel density of `values` (optionally `weights`) evaluated at each of `at`.
///
/// The estimate is normalised to integrate to 1 over the real line (a probability density: with
/// unit weights, the share of values per unit), whatever the weights' total. `bandwidth` is the
/// kernel's standard deviation in value units; ≤ 0 or non-finite uses
/// [`silverman_bandwidth`] of the (unweighted) values.
///
/// Method: the finite values are sorted once, and each evaluation point sums only the values
/// within 8 bandwidths of it (found by binary search) — the kernel beyond is below 1.3·10⁻¹⁴ of
/// its peak, far under what a chart can show. Summing in sorted order makes the result
/// independent of the input's row order, bit for bit. Cost O(n log n + m·(log n + k)), `k` the
/// values in a window.
///
/// Totality: non-finite values, and non-finite or negative weights, are skipped (missing weights —
/// a shorter slice — count as 1); no usable value, or a zero total weight, gives zeros;
/// non-finite evaluation points give 0. Deterministic (`datars_math::m::exp`).
///
/// ```
/// # use datars_algo::kde;
/// // One value: a normal curve around it, peaking at 1 / (h·√(2π)).
/// let d = kde(&[0.0], None, 1.0, &[0.0, 1.0]);
/// assert!((d[0] - 0.398_942_28).abs() < 1e-8 && d[1] < d[0]);
/// ```
pub fn kde(values: &[f64], weights: Option<&[f64]>, bandwidth: f64, at: &[f64]) -> Vec<f64> {
    let mut pts: Vec<(f64, f64)> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, weights.map_or(1.0, |w| w.get(i).copied().unwrap_or(1.0))))
        .filter(|&(v, w)| v.is_finite() && w.is_finite() && w >= 0.0)
        .collect();
    pts.sort_by(|a, b| total_cmp(a.0, b.0).then(total_cmp(a.1, b.1)));
    let total: f64 = pts.iter().map(|p| p.1).sum();
    let mut out = vec![0.0; at.len()];
    if pts.is_empty() || !(total > 0.0 && total.is_finite()) {
        return out;
    }
    let h = if bandwidth.is_finite() && bandwidth > 0.0 { bandwidth } else { silverman_bandwidth(values) };
    if !(h.is_finite() && h > 0.0) {
        return out;
    }
    let norm = 1.0 / (total * h * m::sqrt(m::TAU));
    let reach = 8.0 * h;
    for (o, &x) in out.iter_mut().zip(at) {
        if !x.is_finite() {
            continue;
        }
        let lo = pts.partition_point(|p| p.0 < x - reach);
        let hi = pts.partition_point(|p| p.0 <= x + reach);
        let mut s = 0.0;
        for &(v, w) in &pts[lo..hi] {
            let z = (x - v) / h;
            s += w * m::exp(-0.5 * z * z);
        }
        *o = s * norm;
    }
    out
}

/// Silverman's rule-of-thumb bandwidth for a Gaussian kernel: `0.9 · min(σ, IQR / 1.34) · n^(−1/5)`
/// over the finite values (σ the sample standard deviation, IQR between the R-7 quartiles) — R's
/// `bw.nrd0`, the default of R's `density()` and ggplot's violins.
///
/// Where the spread is zero it falls back as `bw.nrd0` does: to σ when only the IQR is zero, then to
/// the magnitude of the first value, then to 1 — so a single value or a column of equal values
/// still gets a curve. No finite value gives NaN. Deterministic (`m::pow`).
///
/// ```
/// # use datars_algo::silverman_bandwidth;
/// // σ ≈ 1.58, IQR / 1.34 ≈ 1.49: the IQR term wins; × 0.9 × 5^(−1/5).
/// let h = silverman_bandwidth(&[1.0, 2.0, 3.0, 4.0, 5.0]);
/// assert!((h - 0.9 * (2.0 / 1.34) * datars_math::m::pow(5.0, -0.2)).abs() < 1e-12);
/// ```
pub fn silverman_bandwidth(values: &[f64]) -> f64 {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    let n = v.len();
    if n == 0 {
        return f64::NAN;
    }
    v.sort_by(|a, b| total_cmp(*a, *b));
    let mean = v.iter().sum::<f64>() / n as f64;
    let sd = if n > 1 { m::sqrt(v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1) as f64) } else { 0.0 };
    let iqr = quantile(&v, 0.75) - quantile(&v, 0.25);
    let mut lo = sd.min(iqr / 1.34);
    if lo <= 0.0 {
        lo = sd;
    }
    if lo <= 0.0 {
        lo = values.iter().copied().find(|x| x.is_finite()).map_or(0.0, f64::abs);
    }
    if lo <= 0.0 {
        lo = 1.0;
    }
    0.9 * lo * m::pow(n as f64, -0.2)
}

/// R-7 quantile of sorted values (linear between closest ranks).
fn quantile(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let (i, f) = (h.floor() as usize, h - h.floor());
    match sorted.get(i + 1) {
        Some(&b) => sorted[i] + f * (b - sorted[i]),
        None => sorted[i],
    }
}

/// `steps` evenly spaced evaluation points from `lo` to `hi` inclusive (a density curve's grid).
/// Fewer than two steps give `[lo]` (or nothing for zero); a reversed or degenerate extent is
/// returned as asked.
pub fn grid(lo: f64, hi: f64, steps: usize) -> Vec<f64> {
    match steps {
        0 => Vec::new(),
        1 => vec![lo],
        n => (0..n).map(|i| if i == n - 1 { hi } else { lo + (hi - lo) * i as f64 / (n - 1) as f64 }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed, skewed sample (the same every run): squares of a hash-spread sequence.
    fn sample() -> Vec<f64> {
        (0..200).map(|i| {
            let u = ((i * 7919) % 1000) as f64 / 1000.0;
            20.0 + 40.0 * u * u
        }).collect()
    }

    fn trapezoid(xs: &[f64], ys: &[f64]) -> f64 {
        xs.windows(2).zip(ys.windows(2)).map(|(x, y)| (x[1] - x[0]) * (y[0] + y[1]) / 2.0).sum()
    }

    #[test]
    fn integrates_to_one() {
        let v = sample();
        let xs = grid(-20.0, 100.0, 2001);
        for h in [0.5, 2.0, 8.0, 0.0] {
            let d = kde(&v, None, h, &xs);
            let area = trapezoid(&xs, &d);
            assert!((area - 1.0).abs() < 1e-6, "bandwidth {h}: area {area}");
        }
        // Weights shift the mass, never the total.
        let w: Vec<f64> = (0..v.len()).map(|i| (i % 5) as f64).collect();
        let area = trapezoid(&xs, &kde(&v, Some(&w), 2.0, &xs));
        assert!((area - 1.0).abs() < 1e-6, "weighted: {area}");
    }

    #[test]
    fn deterministic_and_independent_of_row_order() {
        let v = sample();
        let xs = grid(15.0, 65.0, 97);
        let a = kde(&v, None, 0.0, &xs);
        assert_eq!(a, kde(&v, None, 0.0, &xs), "same input, same bits");
        let mut r = v.clone();
        r.reverse();
        assert_eq!(a, kde(&r, None, 0.0, &xs), "rows in another order: the same bits");
        assert_eq!(silverman_bandwidth(&v).to_bits(), silverman_bandwidth(&r).to_bits());
    }

    #[test]
    fn a_wider_bandwidth_flattens_and_spreads_the_curve() {
        let v = [0.0, 0.3, -0.2, 0.1];
        let xs = [0.0, 3.0];
        let narrow = kde(&v, None, 0.5, &xs);
        let wide = kde(&v, None, 2.0, &xs);
        assert!(wide[0] < narrow[0], "lower peak: {wide:?} vs {narrow:?}");
        assert!(wide[1] > narrow[1], "fatter tails: {wide:?} vs {narrow:?}");
        // A single value is a normal curve with σ = h: at one σ, e^(−½) of the peak.
        let d = kde(&[5.0], None, 2.0, &[5.0, 7.0]);
        assert!((d[1] / d[0] - m::exp(-0.5)).abs() < 1e-12);
    }

    #[test]
    fn silverman_follows_bw_nrd0() {
        // R: bw.nrd0(c(1, 2, 3, 4, 10)) = 0.9 · min(sd, IQR/1.34) · 5^(−0.2); sd ≈ 3.54, IQR = 2.
        let h = silverman_bandwidth(&[1.0, 2.0, 3.0, 4.0, 10.0]);
        assert!((h - 0.9 * (2.0 / 1.34) * m::pow(5.0, -0.2)).abs() < 1e-12, "{h}");
        // Zero IQR but some spread: σ. Equal values: |x|. Zeros: 1. Nothing: NaN.
        let s = silverman_bandwidth(&[3.0, 3.0, 3.0, 3.0, 9.0]);
        let sd = m::sqrt((4.0 * 1.2 * 1.2 + 4.8 * 4.8) / 4.0);
        assert!((s - 0.9 * sd * m::pow(5.0, -0.2)).abs() < 1e-12, "{s}");
        assert!((silverman_bandwidth(&[4.0, 4.0]) - 0.9 * 4.0 * m::pow(2.0, -0.2)).abs() < 1e-12);
        assert!((silverman_bandwidth(&[0.0]) - 0.9).abs() < 1e-12);
        assert!(silverman_bandwidth(&[f64::NAN]).is_nan());
        // More values, narrower kernel (same spread).
        let few: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let many: Vec<f64> = (0..1000).map(|i| i as f64 / 100.0).collect();
        assert!(silverman_bandwidth(&many) < silverman_bandwidth(&few));
    }

    #[test]
    fn total_on_bad_input() {
        assert_eq!(kde(&[], None, 1.0, &[0.0, 1.0]), vec![0.0, 0.0]);
        assert_eq!(kde(&[f64::NAN, f64::INFINITY], None, 1.0, &[0.0]), vec![0.0]);
        assert_eq!(kde(&[1.0, 2.0], Some(&[0.0, 0.0]), 1.0, &[1.0]), vec![0.0], "zero total weight");
        let d = kde(&[1.0, f64::NAN, 2.0], Some(&[1.0, 1.0, -3.0]), 1.0, &[1.0, f64::NAN]);
        assert!(d[0] > 0.0 && d[1] == 0.0, "{d:?}");
        // Only the first value is usable: the curve is that value's alone.
        assert_eq!(d[0], kde(&[1.0], None, 1.0, &[1.0])[0]);
        assert_eq!(grid(0.0, 1.0, 3), vec![0.0, 0.5, 1.0]);
        assert_eq!(grid(2.0, 3.0, 1), vec![2.0]);
        assert!(grid(0.0, 1.0, 0).is_empty());
    }
}
