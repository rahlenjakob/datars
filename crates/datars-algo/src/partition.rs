//! Adjacency partitions (icicles, and sunbursts in polar space).

use crate::hierarchy::Hierarchy;
use crate::util::mag;
use datars_math::Rect;

/// Partition layout over a parent-index hierarchy (see [`crate::hierarchy`]): one band per depth
/// (roots on top, `rect.h / (height + 1)` each), and within its band each node spans a share of its
/// parent's extent ∝ its value (a leaf's value; an internal node's is the sum of its leaves).
/// Several roots share the top band side by side. Children keep input order.
///
/// Returns one rect per node, indexed like `parents`. `padding` is removed from each rect's right
/// and bottom edges (a rect too small for it collapses to its centre line).
///
/// For a **sunburst**, lay out in angle × radius space and map it yourself: pass
/// `Rect::new(0.0, 0.0, TAU, radius)`, then each rect's `x..x1` is the arc's angular span and
/// `y..y1` its inner and outer radius (padding is then in radians and radius units).
///
/// Deterministic; non-finite and negative values count as 0.
pub fn partition(parents: &[Option<usize>], values: &[f64], rect: Rect, padding: f64) -> Vec<Rect> {
    let h = Hierarchy::new(parents);
    let n = h.len();
    if n == 0 {
        return Vec::new();
    }
    let sums = h.sums(values);
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (x0, y0, w, hh) = (fin(rect.x), fin(rect.y), mag(rect.w), mag(rect.h));
    let levels = (h.height() + 1) as f64;
    let dy = hh / levels;
    let pad = mag(padding);
    let mut span = vec![(x0, x0); n];
    let dice = |kids: &[usize], a: f64, b: f64, span: &mut Vec<(f64, f64)>| {
        let total: f64 = kids.iter().map(|&c| sums[c]).sum();
        let k = if total > 0.0 { (b - a) / total } else { 0.0 };
        let last = kids.iter().rposition(|&c| sums[c] > 0.0);
        let mut x = a;
        for (pos, &c) in kids.iter().enumerate() {
            let x1 = if Some(pos) == last { b } else { x + sums[c] * k };
            span[c] = (x, x1);
            x = x1;
        }
    };
    dice(&h.roots, x0, x0 + w, &mut span);
    for &v in &h.preorder {
        if !h.children[v].is_empty() {
            let (a, b) = span[v];
            dice(&h.children[v], a, b, &mut span);
        }
    }
    (0..n)
        .map(|v| {
            let (a, b) = span[v];
            let (top, bottom) = (y0 + dy * h.depth[v] as f64, y0 + dy * (h.depth[v] + 1) as f64);
            let (mut xa, mut xb) = (a, b - pad);
            if xb < xa {
                xa = (xa + xb) / 2.0;
                xb = xa;
            }
            let (mut ya, mut yb) = (top, bottom - pad);
            if yb < ya {
                ya = (ya + yb) / 2.0;
                yb = ya;
            }
            Rect::new(xa, ya, xb - xa, yb - ya)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_by_depth_and_spans_by_value() {
        // 0 → {1, 2}; 1 → {3, 4}
        let parents = [None, Some(0), Some(0), Some(1), Some(1)];
        let values = [0.0, 0.0, 2.0, 1.0, 1.0];
        let rs = partition(&parents, &values, Rect::new(0.0, 0.0, 400.0, 300.0), 0.0);
        assert_eq!(rs[0], Rect::new(0.0, 0.0, 400.0, 100.0));
        assert_eq!(rs[1], Rect::new(0.0, 100.0, 200.0, 100.0));
        assert_eq!(rs[2], Rect::new(200.0, 100.0, 200.0, 100.0));
        assert_eq!(rs[3], Rect::new(0.0, 200.0, 100.0, 100.0));
        assert_eq!(rs[4], Rect::new(100.0, 200.0, 100.0, 100.0));
    }

    #[test]
    fn children_within_parent_span_with_padding() {
        let parents = [None, Some(0), Some(0), Some(0), Some(2)];
        let values = [0.0, 1.0, 0.0, 3.0, 5.0];
        let rs = partition(&parents, &values, Rect::new(0.0, 0.0, 90.0, 30.0), 1.0);
        for (c, p) in [(1, 0), (2, 0), (3, 0), (4, 2)] {
            assert!(rs[c].x >= rs[p].x - 1e-9 && rs[c].x1() <= rs[p].x1() + 1.0 + 1e-9, "{c} in {p}");
            assert!(rs[c].y >= rs[p].y1());
        }
        assert!((rs[0].w - 89.0).abs() < 1e-9 && (rs[0].h - 9.0).abs() < 1e-9);
    }

    #[test]
    fn areas_proportional_on_a_random_hierarchy_and_deterministic() {
        // 400 nodes from a seeded rule, random leaf values: every leaf's width (its area within
        // its band) is its share of the total, every parent spans exactly its children, and the
        // same input gives the same bits.
        let n = 400;
        let mut rng = datars_math::Rng::new(11);
        let parents: Vec<Option<usize>> = (0..n).map(|i| if i == 0 { None } else { Some(rng.below(i as u64) as usize) }).collect();
        let values: Vec<f64> = (0..n).map(|_| rng.range(0.5, 10.0)).collect();
        let r = Rect::new(0.0, 0.0, 1000.0, 500.0);
        let rs = partition(&parents, &values, r, 0.0);
        assert_eq!(partition(&parents, &values, r, 0.0), rs, "deterministic");
        let h = Hierarchy::new(&parents);
        let sums = h.sums(&values);
        for v in 0..n {
            assert!((rs[v].w - 1000.0 * sums[v] / sums[0]).abs() < 1e-6, "node {v} ∝ its value");
            if !h.children[v].is_empty() {
                let kids = &h.children[v];
                assert!((rs[kids[0]].x - rs[v].x).abs() < 1e-9 && (rs[*kids.last().unwrap()].x1() - rs[v].x1()).abs() < 1e-6, "children tile node {v}");
            }
        }
    }

    #[test]
    fn forest_and_empty() {
        let rs = partition(&[None, None], &[1.0, 3.0], Rect::new(0.0, 0.0, 8.0, 2.0), 0.0);
        assert_eq!(rs[0], Rect::new(0.0, 0.0, 2.0, 2.0));
        assert_eq!(rs[1], Rect::new(2.0, 0.0, 6.0, 2.0));
        assert!(partition(&[], &[], Rect::new(0.0, 0.0, 1.0, 1.0), 0.0).is_empty());
        let z = partition(&[None, Some(0)], &[f64::NAN, -1.0], Rect::new(0.0, 0.0, 1.0, 1.0), 5.0);
        assert!(z.iter().all(|r| r.w >= 0.0 && r.h >= 0.0 && r.x.is_finite()));
    }
}
