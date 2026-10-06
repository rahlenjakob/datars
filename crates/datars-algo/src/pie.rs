//! Angular partition of a span by value (the geometry behind pies, donuts and radial bars).

use crate::util::mag;
use datars_math::total_cmp;
use serde::{Deserialize, Serialize};

/// The order in which [`pie`] lays slices around the span. The output is always indexed like the input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PieSort {
    /// Input order.
    #[default]
    None,
    /// Smallest first.
    Asc,
    /// Largest first.
    Desc,
}

/// Divides the angular span `start_angle..end_angle` among `values`.
///
/// Returns one `(a0, a1)` per value, **indexed like the input** whatever `sort` says; `sort` only
/// decides the order in which slices follow each other around the span. Angles are radians,
/// clockwise from 12 o'clock (the convention of `Vec2::polar`); a negative span runs
/// counter-clockwise, and then `a1 < a0`.
///
/// `pad_angle` is the gap between neighbouring visible slices. Each positive slice owns a slot of
/// `value · k + pad` and is drawn inset by `pad / 2` at both ends, so the visible arcs plus one pad
/// per positive slice add up to the span exactly (the last slot is pinned to `end_angle`). The pad
/// is clamped to `span / positive_count`, so it can never eat more than the span.
///
/// Totality: non-finite and negative values count as 0. Zero-valued slices take no pad and come back
/// as an empty arc `(a, a)` at their place in the order (the middle of the neighbouring gap), so they
/// can grow in place when animated. With nothing positive, every arc is `(start, start)`.
///
/// Deterministic: sorting is stable (ties keep input order) with a total order on f64.
pub fn pie(values: &[f64], start_angle: f64, end_angle: f64, pad_angle: f64, sort: PieSort) -> Vec<(f64, f64)> {
    let n = values.len();
    let start = if start_angle.is_finite() { start_angle } else { 0.0 };
    let mut out = vec![(start, start); n];
    let span = end_angle - start;
    if n == 0 || !span.is_finite() || span == 0.0 {
        return out;
    }
    let vals: Vec<f64> = values.iter().map(|&v| mag(v)).collect();
    let mut order: Vec<usize> = (0..n).collect();
    match sort {
        PieSort::None => {}
        PieSort::Asc => order.sort_by(|&a, &b| total_cmp(vals[a], vals[b])),
        PieSort::Desc => order.sort_by(|&a, &b| total_cmp(vals[b], vals[a])),
    }
    let positive = vals.iter().filter(|&&v| v > 0.0).count();
    let total: f64 = vals.iter().sum();
    if positive == 0 || total <= 0.0 || !total.is_finite() {
        return out;
    }
    let dir = span.signum();
    let abs = span.abs();
    let pad = mag(pad_angle).min(abs / positive as f64);
    let k = (abs - pad * positive as f64) / total;
    let last = order.iter().rposition(|&i| vals[i] > 0.0).unwrap_or(0);
    let mut a = start;
    for (pos, &i) in order.iter().enumerate() {
        let v = vals[i];
        if v > 0.0 {
            let a1 = if pos == last { end_angle } else { a + dir * (v * k + pad) };
            out[i] = (a + dir * pad / 2.0, a1 - dir * pad / 2.0);
            a = a1;
        } else {
            out[i] = (a, a);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::m::{PI, TAU};

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn arcs_sum_to_span_with_pad() {
        let v = [1.0, 2.0, 3.0, 0.0, 4.0];
        let pad = 0.05;
        let arcs = pie(&v, 0.0, TAU, pad, PieSort::None);
        let visible: f64 = arcs.iter().map(|a| a.1 - a.0).sum();
        assert!(close(visible + 4.0 * pad, TAU), "{visible}");
        // Proportional: slice widths ∝ values.
        let k = (arcs[1].1 - arcs[1].0) / 2.0;
        for (i, &x) in v.iter().enumerate() {
            assert!(close(arcs[i].1 - arcs[i].0, x * k), "slice {i}");
        }
        // Gaps between consecutive visible slices equal the pad.
        assert!(close(arcs[1].0 - arcs[0].1, pad));
        assert!(close(arcs[4].0 - arcs[2].1, pad));
        // The zero slice sits inside the gap, empty.
        assert_eq!(arcs[3].0, arcs[3].1);
        assert!(arcs[3].0 > arcs[2].1 && arcs[3].0 < arcs[4].0);
        assert!(close(arcs[0].0, pad / 2.0) && close(arcs[4].1, TAU - pad / 2.0));
    }

    #[test]
    fn sort_changes_layout_order_not_output_order() {
        let v = [1.0, 3.0, 2.0];
        let d = pie(&v, 0.0, TAU, 0.0, PieSort::Desc);
        assert!(close(d[1].0, 0.0), "largest first");
        assert!(close(d[2].0, d[1].1) && close(d[0].0, d[2].1));
        assert_eq!(d[0].1, TAU, "last slot pinned to the end");
        let a = pie(&v, 0.0, TAU, 0.0, PieSort::Asc);
        assert!(close(a[0].0, 0.0));
    }

    #[test]
    fn negative_span_runs_counter_clockwise() {
        let arcs = pie(&[1.0, 1.0], 0.0, -PI, 0.0, PieSort::None);
        assert!(close(arcs[0].0, 0.0) && close(arcs[0].1, -PI / 2.0));
        assert!(close(arcs[1].1, -PI));
    }

    #[test]
    fn degenerate_inputs_are_total() {
        assert!(pie(&[], 0.0, TAU, 0.0, PieSort::None).is_empty());
        let z = pie(&[0.0, f64::NAN, -3.0], 1.0, 2.0, 0.1, PieSort::Desc);
        assert!(z.iter().all(|&a| a == (1.0, 1.0)));
        // Pad larger than the span is clamped: slices become empty but nothing goes out of range.
        let p = pie(&[1.0, 1.0], 0.0, 0.1, 10.0, PieSort::None);
        assert!(p.iter().all(|a| a.0 >= 0.0 && a.1 <= 0.1 + 1e-12 && a.1 >= a.0 - 1e-12));
        let inf = pie(&[1.0, f64::INFINITY], 0.0, f64::NAN, 0.0, PieSort::None);
        assert!(inf.iter().all(|&a| a == (0.0, 0.0)));
    }
}
