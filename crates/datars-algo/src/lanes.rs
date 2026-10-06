//! Lanes: intervals packed into as few rows as possible without overlapping — labels along a time
//! axis (a timeline's events), spans on a swimlane, anything 1-D that has a width.

use datars_math::total_cmp;

/// The lane of each interval `[starts[i], ends[i]]`, packed so that intervals in one lane are at
/// least `gap` apart.
///
/// Greedy first fit in order of start (ties in input order): each interval takes the lowest lane
/// whose last interval ends `gap` or more before it starts. For intervals that is optimal — the
/// number of lanes is the largest number of intervals over any one point (an interval graph is
/// coloured optimally in left-endpoint order) — and it's stable: an interval that moves a little
/// keeps its lane unless it now touches a neighbour.
///
/// `max` caps the lanes: an interval that fits none of the first `max` gets `None` and takes no
/// room (a label left out, its point still drawn). Indexed like the input; an interval with a
/// non-finite end also gets `None`; `end < start` is read the other way round. A non-finite or
/// negative `gap` counts as 0.
///
/// ```
/// # use datars_algo::lanes;
/// // Two labels that overlap share no lane; the third fits after the first.
/// let l = lanes(&[0.0, 5.0, 12.0], &[10.0, 15.0, 20.0], 1.0, None);
/// assert_eq!(l, vec![Some(0), Some(1), Some(0)]);
/// ```
pub fn lanes(starts: &[f64], ends: &[f64], gap: f64, max: Option<usize>) -> Vec<Option<usize>> {
    let n = starts.len().min(ends.len());
    let gap = if gap.is_finite() && gap > 0.0 { gap } else { 0.0 };
    let mut out = vec![None; starts.len()];
    let span = |i: usize| {
        let (a, b) = (starts[i], ends[i]);
        if a <= b {
            (a, b)
        } else {
            (b, a)
        }
    };
    let mut order: Vec<usize> = (0..n).filter(|&i| starts[i].is_finite() && ends[i].is_finite()).collect();
    order.sort_by(|&a, &b| total_cmp(span(a).0, span(b).0).then(a.cmp(&b)));
    // The end of the last interval in each lane opened so far.
    let mut last: Vec<f64> = Vec::new();
    for i in order {
        let (a, b) = span(i);
        match last.iter().position(|&end| end + gap <= a) {
            Some(l) => {
                last[l] = b;
                out[i] = Some(l);
            }
            None if max.is_none_or(|m| last.len() < m) => {
                last.push(b);
                out[i] = Some(last.len() - 1);
            }
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apart_intervals_share_the_first_lane() {
        assert_eq!(lanes(&[0.0, 20.0, 40.0], &[10.0, 30.0, 50.0], 2.0, None), vec![Some(0); 3]);
    }

    #[test]
    fn as_many_lanes_as_the_deepest_overlap() {
        // Four intervals, at most three over any point: three lanes, the last reusing the first.
        let l = lanes(&[0.0, 1.0, 2.0, 11.0], &[10.0, 10.0, 10.0, 20.0], 0.0, None);
        assert_eq!(l, vec![Some(0), Some(1), Some(2), Some(0)]);
    }

    #[test]
    fn the_gap_counts_and_input_order_breaks_ties() {
        // 10 + gap 3 > 12: the second can't follow the first in lane 0.
        assert_eq!(lanes(&[0.0, 12.0], &[10.0, 20.0], 3.0, None), vec![Some(0), Some(1)]);
        assert_eq!(lanes(&[0.0, 13.0], &[10.0, 20.0], 3.0, None), vec![Some(0), Some(0)]);
        // Same start: the earlier row gets the lower lane, whatever order they're sorted in.
        assert_eq!(lanes(&[5.0, 5.0], &[8.0, 9.0], 0.0, None), vec![Some(0), Some(1)]);
    }

    #[test]
    fn indexed_like_the_input_in_any_order() {
        let l = lanes(&[40.0, 0.0, 5.0], &[50.0, 10.0, 15.0], 0.0, None);
        assert_eq!(l, vec![Some(0), Some(0), Some(1)]);
    }

    #[test]
    fn a_cap_leaves_out_what_doesnt_fit() {
        let l = lanes(&[0.0, 1.0, 2.0, 30.0], &[10.0, 10.0, 10.0, 40.0], 0.0, Some(2));
        assert_eq!(l, vec![Some(0), Some(1), None, Some(0)]);
        assert_eq!(lanes(&[0.0], &[1.0], 0.0, Some(0)), vec![None]);
    }

    #[test]
    fn total_on_bad_input() {
        let l = lanes(&[f64::NAN, 10.0, 0.0], &[5.0, 0.0, f64::INFINITY], f64::NAN, None);
        assert_eq!(l, vec![None, Some(0), None], "non-finite ends are left out; a reversed interval reads forwards");
        assert!(lanes(&[], &[], 1.0, None).is_empty());
        // Mismatched lengths: the extra starts get no lane.
        assert_eq!(lanes(&[0.0, 1.0], &[2.0], 0.0, None), vec![Some(0), None]);
    }
}
