//! Squarified treemaps (Bruls, Huizing & van Wijk), flat and nested.

use crate::hierarchy::Hierarchy;
use crate::util::mag;
use datars_math::{total_cmp, Rect};
use serde::{Deserialize, Serialize};

/// The golden ratio, the default target aspect ratio for squarified tiles (as in d3).
pub const GOLDEN: f64 = 1.618033988749895;

/// Squarified treemap of `values` in `rect`; returns one rect per value, **indexed like the input**.
///
/// Values are laid out largest first (a stable descending sort — the order squarify needs for good
/// aspect ratios) in rows whose tiles approach `ratio` (width / height or its inverse; `GOLDEN` is
/// the usual choice, values below 1 or non-finite mean 1). The rects tile `rect` exactly: they
/// don't overlap, the last row is pinned to the far edge, and each area is ∝ its value.
///
/// Totality: non-finite and negative values count as 0 and get an empty rect (at the corner where
/// the leftover space ended). A degenerate `rect` or an all-zero input gives empty rects at `rect`'s
/// origin. Deterministic.
pub fn treemap(values: &[f64], rect: Rect, ratio: f64) -> Vec<Rect> {
    let vals: Vec<f64> = values.iter().map(|&v| mag(v)).collect();
    let items: Vec<usize> = (0..vals.len()).collect();
    let mut out = vec![Rect::new(finite0(rect.x), finite0(rect.y), 0.0, 0.0); vals.len()];
    squarify(&items, &vals, rect, ratio, &mut out);
    out
}

fn finite0(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Lays `items` (indices into `vals`) into `rect`, writing `out[item]`.
fn squarify(items: &[usize], vals: &[f64], rect: Rect, ratio: f64, out: &mut [Rect]) {
    let ratio = if ratio.is_finite() && ratio > 1.0 { ratio } else { 1.0 };
    let mut order: Vec<usize> = items.to_vec();
    order.sort_by(|&a, &b| total_cmp(vals[b], vals[a]));
    let (x0, y0) = (finite0(rect.x), finite0(rect.y));
    let (w, h) = (finite0(rect.w).max(0.0), finite0(rect.h).max(0.0));
    for &i in &order {
        out[i] = Rect::new(x0, y0, 0.0, 0.0);
    }
    let mut value: f64 = order.iter().map(|&i| vals[i]).sum();
    if value <= 0.0 || !value.is_finite() || w <= 0.0 || h <= 0.0 {
        return;
    }
    let n = order.len();
    let last_positive = order.iter().rposition(|&i| vals[i] > 0.0).unwrap_or(0);
    let (mut x0, mut y0, x1, y1) = (x0, y0, x0 + w, y0 + h);
    let (mut i0, mut i1) = (0usize, 0usize);
    while i0 < n {
        let (dx, dy) = (x1 - x0, y1 - y0);
        // The next non-empty node starts a row.
        let mut sum;
        loop {
            sum = vals[order[i1]];
            i1 += 1;
            if sum != 0.0 || i1 >= n {
                break;
            }
        }
        let (mut min_v, mut max_v) = (sum, sum);
        let alpha = (dy / dx).max(dx / dy) / (value * ratio);
        let mut beta = sum * sum * alpha;
        let mut min_ratio = (max_v / beta).max(beta / min_v);
        // Keep adding nodes while the worst aspect ratio holds or improves.
        while i1 < n {
            let v = vals[order[i1]];
            let s = sum + v;
            let (mn, mx) = (min_v.min(v), max_v.max(v));
            beta = s * s * alpha;
            let r = (mx / beta).max(beta / mn);
            if r > min_ratio || r.is_nan() {
                break;
            }
            sum = s;
            min_v = mn;
            max_v = mx;
            min_ratio = r;
            i1 += 1;
        }
        let row = &order[i0..i1];
        let closes = i1 > last_positive;
        if dx < dy {
            // A row across the top, split left → right.
            let ny = if closes { y1 } else { y0 + dy * sum / value };
            split(row, vals, sum, x0, x1, |i, a, b| out[i] = Rect::new(a, y0, b - a, ny - y0));
            y0 = ny;
        } else {
            // A column down the left, split top → bottom.
            let nx = if closes { x1 } else { x0 + dx * sum / value };
            split(row, vals, sum, y0, y1, |i, a, b| out[i] = Rect::new(x0, a, nx - x0, b - a));
            x0 = nx;
        }
        value -= sum;
        i0 = i1;
        if closes {
            // Everything left is zero: park it at the far corner.
            for &i in &order[i0..] {
                out[i] = Rect::new(x1, y1, 0.0, 0.0);
            }
            break;
        }
    }
}

/// Splits `lo..hi` among `row` ∝ value, the last positive item pinned to `hi`.
fn split(row: &[usize], vals: &[f64], sum: f64, lo: f64, hi: f64, mut put: impl FnMut(usize, f64, f64)) {
    let k = if sum > 0.0 { (hi - lo) / sum } else { 0.0 };
    let last = row.iter().rposition(|&i| vals[i] > 0.0);
    let mut a = lo;
    for (pos, &i) in row.iter().enumerate() {
        let b = if Some(pos) == last { hi } else { a + vals[i] * k };
        put(i, a, b);
        a = b;
    }
}

/// Options for [`treemap_nested`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreemapOptions {
    /// Target aspect ratio of tiles (see [`treemap`]).
    pub ratio: f64,
    /// Gap between sibling tiles.
    pub padding_inner: f64,
    /// Inset of an internal node's children from its edges.
    pub padding_outer: f64,
    /// Inset from an internal node's top edge (overrides `padding_outer` there) — room for a label.
    pub padding_top: f64,
}

impl Default for TreemapOptions {
    fn default() -> Self {
        TreemapOptions { ratio: GOLDEN, padding_inner: 0.0, padding_outer: 0.0, padding_top: 0.0 }
    }
}

/// Nested squarified treemap over a parent-index hierarchy (see [`crate::hierarchy`]).
///
/// Returns one rect per node, indexed like `parents`. A node's size is the sum of its leaves'
/// `values` (internal nodes' own values are ignored). Each internal node's children are squarified
/// inside the node's rect inset by `padding_outer` (`padding_top` at the top), with
/// `padding_inner` between siblings; several roots are tiled side by side in `rect`, a single root
/// fills it. Padding that doesn't fit collapses a rect to its centre line rather than inverting it.
///
/// Deterministic; total (see [`treemap`] and the hierarchy repair rules).
pub fn treemap_nested(parents: &[Option<usize>], values: &[f64], rect: Rect, opts: &TreemapOptions) -> Vec<Rect> {
    let h = Hierarchy::new(parents);
    let sums = h.sums(values);
    let n = h.len();
    let pi = mag(opts.padding_inner);
    let po = mag(opts.padding_outer);
    let pt = if opts.padding_top.is_finite() && opts.padding_top > 0.0 { opts.padding_top } else { po };
    let mut out = vec![Rect::new(finite0(rect.x), finite0(rect.y), 0.0, 0.0); n];
    // An invisible root holds the real roots: its tiling area is `rect` grown by half the inner
    // padding, because every tile shrinks itself by that much on each side.
    let half = pi / 2.0;
    let grown = Rect::new(rect.x - half, rect.y - half, rect.w + pi, rect.h + pi);
    squarify(&h.roots, &sums, grown, opts.ratio, &mut out);
    for &r in &h.roots {
        out[r] = shrink(out[r], half, half, half, half);
    }
    for &v in &h.preorder {
        if h.children[v].is_empty() {
            continue;
        }
        let area = shrink(out[v], pt - half, po - half, po - half, po - half);
        squarify(&h.children[v], &sums, area, opts.ratio, &mut out);
        for &c in &h.children[v] {
            out[c] = shrink(out[c], half, half, half, half);
        }
    }
    out
}

/// Insets a rect (top, right, bottom, left); an inset that doesn't fit collapses to the centre line.
fn shrink(r: Rect, top: f64, right: f64, bottom: f64, left: f64) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (r.x + left, r.y + top, r.x1() - right, r.y1() - bottom);
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &Rect, b: &Rect) -> f64 {
        let w = (a.x1().min(b.x1()) - a.x.max(b.x)).max(0.0);
        let h = (a.y1().min(b.y1()) - a.y.max(b.y)).max(0.0);
        w * h
    }

    fn check_tiling(values: &[f64], rect: Rect) {
        let rs = treemap(values, rect, GOLDEN);
        assert_eq!(rs.len(), values.len());
        let total: f64 = values.iter().map(|&v| mag(v)).sum();
        let area = rect.w * rect.h;
        let covered: f64 = rs.iter().map(|r| r.w * r.h).sum();
        assert!((covered - area).abs() < 1e-6 * area, "covered {covered} of {area}");
        for (i, r) in rs.iter().enumerate() {
            assert!(r.w >= 0.0 && r.h >= 0.0);
            assert!(r.x >= rect.x - 1e-9 && r.x1() <= rect.x1() + 1e-9, "inside x {r:?}");
            assert!(r.y >= rect.y - 1e-9 && r.y1() <= rect.y1() + 1e-9, "inside y {r:?}");
            let expect = mag(values[i]) / total * area;
            assert!((r.w * r.h - expect).abs() < 1e-6 * area, "area ∝ value for {i}");
        }
        for i in 0..rs.len() {
            for j in i + 1..rs.len() {
                assert!(overlap(&rs[i], &rs[j]) < 1e-9 * area, "{i} and {j} overlap");
            }
        }
    }

    #[test]
    fn tiles_and_areas_are_proportional() {
        check_tiling(&[6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0], Rect::new(0.0, 0.0, 600.0, 400.0));
        check_tiling(&[1.0, 50.0, 3.0, 0.0, 7.0, f64::NAN, 2.0, -4.0], Rect::new(10.0, 20.0, 100.0, 700.0));
        let many: Vec<f64> = (0..200).map(|i| ((i * 37) % 101) as f64 + 1.0).collect();
        check_tiling(&many, Rect::new(0.0, 0.0, 1000.0, 1000.0));
    }

    #[test]
    fn classic_example_rows() {
        // Bruls et al.'s example: 6 6 4 3 2 2 1 in 6×4 gives a first column of two 3×2 tiles.
        let rs = treemap(&[6.0, 6.0, 4.0, 3.0, 2.0, 2.0, 1.0], Rect::new(0.0, 0.0, 6.0, 4.0), 1.0);
        assert!((rs[0].w - 3.0).abs() < 1e-9 && (rs[0].h - 2.0).abs() < 1e-9);
        assert!((rs[1].y - 2.0).abs() < 1e-9);
    }

    #[test]
    fn preserves_input_indexing() {
        let rs = treemap(&[1.0, 9.0], Rect::new(0.0, 0.0, 10.0, 10.0), GOLDEN);
        assert!(rs[1].w * rs[1].h > rs[0].w * rs[0].h);
    }

    #[test]
    fn degenerate_inputs() {
        assert!(treemap(&[], Rect::new(0.0, 0.0, 1.0, 1.0), GOLDEN).is_empty());
        let z = treemap(&[0.0, 0.0], Rect::new(5.0, 5.0, 10.0, 10.0), GOLDEN);
        assert!(z.iter().all(|r| r.w == 0.0 && r.h == 0.0));
        let flat = treemap(&[1.0, 2.0], Rect::new(0.0, 0.0, 0.0, 10.0), GOLDEN);
        assert!(flat.iter().all(|r| r.w == 0.0));
        let nan = treemap(&[1.0], Rect::new(f64::NAN, 0.0, 10.0, 10.0), f64::NAN);
        assert!(nan[0].x.is_finite());
    }

    #[test]
    fn nested_children_inside_parents_with_padding() {
        // root 0 → {1, 2}; 1 → {3, 4, 5}; 2 → {6}
        let parents = [None, Some(0), Some(0), Some(1), Some(1), Some(1), Some(2)];
        let values = [0.0, 0.0, 0.0, 3.0, 2.0, 1.0, 4.0];
        let opts = TreemapOptions { padding_inner: 2.0, padding_outer: 3.0, padding_top: 12.0, ..Default::default() };
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let rs = treemap_nested(&parents, &values, rect, &opts);
        assert_eq!(rs[0], rect, "a single root fills the rect");
        for (c, p) in [(1, 0), (2, 0), (3, 1), (4, 1), (5, 1), (6, 2)] {
            let (r, q) = (rs[c], rs[p]);
            assert!(r.x >= q.x + 3.0 - 1e-9 && r.x1() <= q.x1() - 3.0 + 1e-9, "{c} inside {p} (x)");
            assert!(r.y >= q.y + 12.0 - 1e-9 && r.y1() <= q.y1() - 3.0 + 1e-9, "{c} inside {p} (y)");
        }
        for (a, b) in [(1, 2), (3, 4), (3, 5), (4, 5)] {
            assert!(overlap(&rs[a], &rs[b]) == 0.0, "siblings {a} {b} don't overlap");
        }
        // Siblings 3, 4, 5 are separated by at least the inner padding along one axis.
        let gap = |a: &Rect, b: &Rect| (b.x - a.x1()).max(a.x - b.x1()).max(b.y - a.y1()).max(a.y - b.y1());
        assert!(gap(&rs[3], &rs[4]) >= 2.0 - 1e-9);
        // Leaf areas stay proportional within a parent.
        let a3 = rs[3].w * rs[3].h;
        let a5 = rs[5].w * rs[5].h;
        assert!(a3 > 2.0 * a5);
    }

    #[test]
    fn nested_forest_and_zero_padding_tiles_exactly() {
        let parents = [None, None, Some(0), Some(0)];
        let rs = treemap_nested(&parents, &[0.0, 5.0, 2.0, 3.0], Rect::new(0.0, 0.0, 100.0, 100.0), &TreemapOptions::default());
        let total: f64 = [rs[1], rs[2], rs[3]].iter().map(|r| r.w * r.h).sum();
        assert!((total - 10_000.0).abs() < 1e-6);
        assert!((rs[0].w * rs[0].h - 5000.0).abs() < 1e-6);
    }
}
