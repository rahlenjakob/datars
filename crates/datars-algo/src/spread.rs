//! 1-D spreading: push positions apart to a minimum gap, moving them as little as possible
//! (labels at the ends of lines, tick labels, callouts along an edge).

use datars_math::total_cmp;

/// Moves `positions` so that neighbours (in position order) are at least `gap` apart, with the
/// least total squared displacement, keeping their order and staying inside `[lo, hi]` when they
/// fit (when they don't, they start at `lo`).
///
/// Exact: with `z_i = q_i − i·gap` over the sorted positions the gap constraints become
/// `z` non-decreasing, so the optimum is the isotonic regression of `p_i − i·gap`
/// (pool-adjacent-violators), clamped to the bounds. Indexed like the input; non-finite positions
/// come back unchanged and take no part. A non-finite or non-positive `gap` returns the input.
///
/// ```
/// # use datars_algo::spread;
/// // Two labels on top of each other move apart symmetrically; a far one stays.
/// assert_eq!(spread(&[10.0, 10.0, 50.0], 4.0, f64::NEG_INFINITY, f64::INFINITY), vec![8.0, 12.0, 50.0]);
/// ```
pub fn spread(positions: &[f64], gap: f64, lo: f64, hi: f64) -> Vec<f64> {
    let mut out = positions.to_vec();
    if !(gap.is_finite() && gap > 0.0) {
        return out;
    }
    let mut idx: Vec<usize> = (0..positions.len()).filter(|&i| positions[i].is_finite()).collect();
    idx.sort_by(|&a, &b| total_cmp(positions[a], positions[b]).then(a.cmp(&b)));
    let n = idx.len();
    if n == 0 {
        return out;
    }
    // Pool adjacent violators over w_i = p_i − i·gap: blocks of (sum, count), means non-decreasing.
    let mut blocks: Vec<(f64, usize)> = Vec::with_capacity(n);
    for (i, &k) in idx.iter().enumerate() {
        blocks.push((positions[k] - i as f64 * gap, 1));
        while blocks.len() > 1 {
            let (s1, c1) = blocks[blocks.len() - 1];
            let (s0, c0) = blocks[blocks.len() - 2];
            if s0 / c0 as f64 <= s1 / c1 as f64 {
                break;
            }
            blocks.pop();
            let last = blocks.len() - 1;
            blocks[last] = (s0 + s1, c0 + c1);
        }
    }
    // The span the labels need; clamp the shifted positions so the whole run stays in bounds.
    let (zlo, zhi) = {
        let a = if lo.is_finite() { lo } else { f64::NEG_INFINITY };
        let b = if hi.is_finite() { hi - (n - 1) as f64 * gap } else { f64::INFINITY };
        (a, if b < a { a } else { b })
    };
    let mut i = 0;
    for (s, c) in blocks {
        let z = (s / c as f64).clamp(zlo, zhi);
        for _ in 0..c {
            out[idx[i]] = z + i as f64 * gap;
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    const INF: f64 = f64::INFINITY;

    #[test]
    fn well_separated_positions_stay() {
        assert_eq!(spread(&[0.0, 20.0, 40.0], 10.0, -INF, INF), vec![0.0, 20.0, 40.0]);
    }

    #[test]
    fn crowded_positions_part_symmetrically_and_keep_their_order() {
        // Three labels within 2 px, gap 10: centred on their mean, 10 apart, indexed like the input.
        let out = spread(&[31.0, 30.0, 29.0], 10.0, -INF, INF);
        assert_eq!(out, vec![40.0, 30.0, 20.0]);
    }

    #[test]
    fn the_least_movement_wins() {
        // Only the close pair moves; the rest is untouched.
        let out = spread(&[0.0, 100.0, 102.0, 200.0], 6.0, -INF, INF);
        assert_eq!(out, vec![0.0, 98.0, 104.0, 200.0]);
    }

    #[test]
    fn bounds_hold_when_the_run_fits() {
        let out = spread(&[0.0, 1.0, 2.0], 10.0, 0.0, 100.0);
        assert_eq!(out, vec![0.0, 10.0, 20.0]);
        let out = spread(&[99.0, 100.0], 10.0, 0.0, 100.0);
        assert_eq!(out, vec![90.0, 100.0]);
    }

    #[test]
    fn non_finite_positions_pass_through() {
        let out = spread(&[f64::NAN, 5.0, 5.0], 2.0, -INF, INF);
        assert!(out[0].is_nan());
        assert_eq!(&out[1..], &[4.0, 6.0]);
        assert_eq!(spread(&[1.0, 1.0], 0.0, -INF, INF), vec![1.0, 1.0]);
        assert!(spread(&[], 3.0, 0.0, 1.0).is_empty());
    }
}
