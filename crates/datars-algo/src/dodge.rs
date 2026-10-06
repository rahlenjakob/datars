//! Side-by-side sub-bands inside one band (grouped bars).

use crate::util::finite;

/// Places items side by side inside one band that starts at `band_start` and is `bandwidth` wide.
///
/// `groups[i]` is the **slot** (usually the series index) of item `i`. Slots are fixed — slot `s`
/// is always at the same offset — so a series lines up across bands even where other series are
/// missing. There are `max(n_series, largest slot + 1)` slots. `padding` (0..1) is the fraction of
/// each slot's step left empty between neighbours (like a band scale's inner padding); the outer
/// slots touch the band edges, so one slot with any padding fills the band.
///
/// Returns `(x, width)` per item. Several items in the same slot overlap (by design: that's what
/// you asked for). Non-finite inputs count as 0; padding is clamped into `[0, 1]`.
///
/// ```
/// # use datars_algo::dodge;
/// // Three series in a band [100, 130): each bar is 10 wide with no padding.
/// assert_eq!(dodge(&[0, 1, 2], 3, 100.0, 30.0, 0.0), vec![(100.0, 10.0), (110.0, 10.0), (120.0, 10.0)]);
/// ```
pub fn dodge(groups: &[usize], n_series: usize, band_start: f64, bandwidth: f64, padding: f64) -> Vec<(f64, f64)> {
    let slots = groups.iter().map(|&g| g.saturating_add(1)).max().unwrap_or(0).max(n_series).max(1);
    let start = finite(band_start);
    let w = finite(bandwidth);
    let p = if padding.is_finite() { padding.clamp(0.0, 1.0) } else { 0.0 };
    let denom = slots as f64 - p;
    let step = if denom > 0.0 { w / denom } else { 0.0 };
    let width = step * (1.0 - p);
    groups.iter().map(|&g| (start + g as f64 * step, width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_tile_the_band_with_padding() {
        let d = dodge(&[0, 1, 2, 3], 4, 0.0, 100.0, 0.2);
        let last = d[3];
        assert!((last.0 + last.1 - 100.0).abs() < 1e-9, "last bar ends at the band edge");
        assert_eq!(d[0].0, 0.0);
        let gap = d[1].0 - (d[0].0 + d[0].1);
        assert!((gap - 0.2 * (d[1].0 - d[0].0)).abs() < 1e-9);
    }

    #[test]
    fn missing_series_keep_their_slot() {
        let full = dodge(&[0, 1, 2], 3, 10.0, 30.0, 0.1);
        let sparse = dodge(&[2, 0], 3, 10.0, 30.0, 0.1);
        assert_eq!(sparse[0], full[2]);
        assert_eq!(sparse[1], full[0]);
    }

    #[test]
    fn degenerate() {
        assert!(dodge(&[], 3, 0.0, 10.0, 0.0).is_empty());
        let one = dodge(&[0], 1, 5.0, 10.0, 1.0);
        assert_eq!(one[0], (5.0, 0.0));
        let grown = dodge(&[4], 2, 0.0, 50.0, 0.0);
        assert_eq!(grown[0], (40.0, 10.0), "slot count grows to fit the largest slot");
        let nan = dodge(&[0, 1], 2, f64::NAN, f64::INFINITY, f64::NAN);
        assert!(nan.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
    }
}
