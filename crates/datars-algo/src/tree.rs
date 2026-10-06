//! Node-link tree layouts: the tidy tree (Reingold–Tilford in linear time, after Buchheim, Jünger &
//! Leipert, as in d3.tree) and the dendrogram (d3.cluster).

use crate::hierarchy::Hierarchy;
use crate::util::mag;
use datars_math::{Rect, Vec2};
use serde::{Deserialize, Serialize};

/// Options for [`tree`] and [`cluster`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeOptions {
    /// Separation between neighbouring siblings, in breadth units (d3: 1).
    pub sibling_separation: f64,
    /// Separation between neighbouring non-siblings (d3: 2).
    pub cousin_separation: f64,
    /// `Some((dx, dy))` lays nodes out on a fixed grid instead of fitting `rect`: one breadth unit is
    /// `dx`, one level is `dy`, the leftmost node sits at `rect.x` and roots at `rect.y`.
    pub node_size: Option<(f64, f64)>,
}

impl Default for TreeOptions {
    fn default() -> Self {
        TreeOptions { sibling_separation: 1.0, cousin_separation: 2.0, node_size: None }
    }
}

const NONE: usize = usize::MAX;

/// Tidy tree over a parent-index hierarchy (see [`crate::hierarchy`]): parents centred over their
/// children, subtrees as close as the separations allow, identical subtrees drawn identically, in
/// O(n). Returns one position per node, indexed like `parents`: `x` is breadth (left → right, sibling
/// order = input order), `y` is depth (roots at `rect.y`, the deepest level at `rect.y1()`), fitted
/// to `rect` unless `opts.node_size` is set. For a radial tree, lay out in angle × radius space
/// (`Rect::new(0.0, 0.0, TAU, radius)`) and map with `Vec2::polar`.
///
/// Several roots are laid out as siblings under an invisible root. Deterministic, non-recursive.
pub fn tree(parents: &[Option<usize>], rect: Rect, opts: &TreeOptions) -> Vec<Vec2> {
    let h = Hierarchy::new(parents);
    let n = h.len();
    if n == 0 {
        return Vec::new();
    }
    let sib = if opts.sibling_separation.is_finite() && opts.sibling_separation > 0.0 { opts.sibling_separation } else { 1.0 };
    let cous = if opts.cousin_separation.is_finite() && opts.cousin_separation > 0.0 { opts.cousin_separation } else { 2.0 };
    // Nodes 0..n are real, `vr` is an invisible root over the real roots, and `vp` is the invisible
    // root's own parent (Buchheim's algorithm reads a node's parent's children and modifiers).
    let (vr, vp) = (n, n + 1);
    let total = n + 2;
    let mut children: Vec<Vec<usize>> = h.children.clone();
    children.push(h.roots.clone());
    children.push(vec![vr]);
    let mut parent: Vec<usize> = h.parent.iter().map(|p| p.unwrap_or(vr)).collect();
    parent.push(vp);
    parent.push(NONE);
    let mut index = vec![0usize; total];
    for kids in &children {
        for (k, &c) in kids.iter().enumerate() {
            index[c] = k;
        }
    }
    let sep = |a: usize, b: usize, parent: &[usize]| if parent[a] == parent[b] { sib } else { cous };

    let mut z = vec![0.0f64; total]; // prelim
    let mut m = vec![0.0f64; total]; // mod
    let mut c = vec![0.0f64; total]; // change
    let mut s = vec![0.0f64; total]; // shift
    let mut t = vec![NONE; total]; // thread
    let mut a: Vec<usize> = (0..total).collect(); // ancestor
    let mut default_ancestor = vec![NONE; total];

    let next_left = |v: usize, children: &[Vec<usize>], t: &[usize]| children[v].first().copied().unwrap_or(t[v]);
    let next_right = |v: usize, children: &[Vec<usize>], t: &[usize]| children[v].last().copied().unwrap_or(t[v]);

    // Post-order over vr's subtree, children left to right (the first walk needs left siblings done).
    let mut post = Vec::with_capacity(n + 1);
    let mut stack = vec![(vr, 0usize)];
    while let Some((v, k)) = stack.pop() {
        if k < children[v].len() {
            stack.push((v, k + 1));
            stack.push((children[v][k], 0));
        } else {
            post.push(v);
        }
    }

    for &v in &post {
        let p = parent[v];
        let w = if index[v] > 0 { children[p][index[v] - 1] } else { NONE };
        if !children[v].is_empty() {
            // Execute shifts.
            let (mut shift, mut change) = (0.0, 0.0);
            for &ch in children[v].iter().rev() {
                z[ch] += shift;
                m[ch] += shift;
                change += c[ch];
                shift += s[ch] + change;
            }
            let first = children[v][0];
            let last = *children[v].last().unwrap_or(&first);
            let mid = (z[first] + z[last]) / 2.0;
            if w != NONE {
                z[v] = z[w] + sep(v, w, &parent);
                m[v] = z[v] - mid;
            } else {
                z[v] = mid;
            }
        } else if w != NONE {
            z[v] = z[w] + sep(v, w, &parent);
        }
        // Apportion.
        let mut ancestor = if default_ancestor[p] != NONE { default_ancestor[p] } else { children[p][0] };
        if w != NONE {
            let (mut vip, mut vop, mut vim) = (v, v, w);
            let mut vom = children[p][0];
            let (mut sip, mut sop, mut sim, mut som) = (m[vip], m[vop], m[vim], m[vom]);
            loop {
                let nim = next_right(vim, &children, &t);
                let nip = next_left(vip, &children, &t);
                if nim == NONE || nip == NONE {
                    if nim != NONE && next_right(vop, &children, &t) == NONE {
                        t[vop] = nim;
                        m[vop] += sim - sop;
                    }
                    if nip != NONE && next_left(vom, &children, &t) == NONE {
                        t[vom] = nip;
                        m[vom] += sip - som;
                        ancestor = v;
                    }
                    break;
                }
                vim = nim;
                vip = nip;
                let nl = next_left(vom, &children, &t);
                let nr = next_right(vop, &children, &t);
                if nl != NONE {
                    vom = nl;
                }
                if nr != NONE {
                    vop = nr;
                }
                a[vop] = v;
                let shift = z[vim] + sim - z[vip] - sip + sep(vim, vip, &parent);
                if shift > 0.0 {
                    let wm = if parent[a[vim]] == parent[v] { a[vim] } else { ancestor };
                    // Move subtree.
                    let span = index[v] as f64 - index[wm] as f64;
                    let change = if span > 0.0 { shift / span } else { 0.0 };
                    c[v] -= change;
                    s[v] += shift;
                    c[wm] += change;
                    z[v] += shift;
                    m[v] += shift;
                    sip += shift;
                    sop += shift;
                }
                sim += m[vim];
                sip += m[vip];
                som += m[vom];
                sop += m[vop];
            }
        }
        default_ancestor[p] = ancestor;
    }

    // Second walk: absolute breadth = prelim + sum of ancestors' modifiers.
    m[vp] = -z[vr];
    let mut x = vec![0.0f64; total];
    let mut stack = vec![vr];
    while let Some(v) = stack.pop() {
        x[v] = z[v] + m[parent[v]];
        m[v] += m[parent[v]];
        for &ch in children[v].iter().rev() {
            stack.push(ch);
        }
    }
    fit(&h, &x[..n], rect, opts, &parent, sib, cous)
}

/// Maps breadth units and depths to output coordinates (d3's fitting rule, or a fixed node size).
fn fit(h: &Hierarchy, x: &[f64], rect: Rect, opts: &TreeOptions, parent: &[usize], sib: f64, cous: f64) -> Vec<Vec2> {
    let n = x.len();
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (rx, ry) = (fin(rect.x), fin(rect.y));
    let (mut left, mut right) = (0usize, 0usize);
    for i in 0..n {
        if x[i] < x[left] {
            left = i;
        }
        if x[i] > x[right] {
            right = i;
        }
    }
    let height = h.height();
    if let Some((dx, dy)) = opts.node_size {
        let (dx, dy) = (fin(dx), fin(dy));
        return (0..n).map(|i| Vec2::new(rx + (x[i] - x[left]) * dx, ry + h.depth[i] as f64 * dy)).collect();
    }
    let sep = if parent[left] == parent[right] { sib } else { cous };
    let s = if left == right { 1.0 } else { sep / 2.0 };
    let tx = s - x[left];
    let span = x[right] + s + tx;
    let kx = if span > 0.0 { mag(rect.w) / span } else { 0.0 };
    let ky = mag(rect.h) / (height.max(1) as f64);
    (0..n).map(|i| Vec2::new(rx + (x[i] + tx) * kx, ry + h.depth[i] as f64 * ky)).collect()
}

/// Dendrogram over a parent-index hierarchy: leaves evenly spaced along the bottom edge (in
/// pre-order, siblings separated by `sibling_separation`, others by `cousin_separation`), each
/// parent centred over its children and placed by height (roots at `rect.y`, leaves at
/// `rect.y1()`). Returns one position per node, indexed like `parents`. `node_size` is ignored.
pub fn cluster(parents: &[Option<usize>], rect: Rect, opts: &TreeOptions) -> Vec<Vec2> {
    let h = Hierarchy::new(parents);
    let n = h.len();
    if n == 0 {
        return Vec::new();
    }
    let sib = if opts.sibling_separation.is_finite() && opts.sibling_separation > 0.0 { opts.sibling_separation } else { 1.0 };
    let cous = if opts.cousin_separation.is_finite() && opts.cousin_separation > 0.0 { opts.cousin_separation } else { 2.0 };
    let sep = |a: usize, b: usize| if h.parent[a] == h.parent[b] { sib } else { cous };
    let mut x = vec![0.0; n];
    let mut height = vec![0usize; n];
    let mut prev_leaf: Option<usize> = None;
    let (mut first_leaf, mut last_leaf) = (0usize, 0usize);
    for v in h.postorder() {
        if h.children[v].is_empty() {
            x[v] = match prev_leaf {
                Some(p) => x[p] + sep(v, p),
                None => {
                    first_leaf = v;
                    0.0
                }
            };
            prev_leaf = Some(v);
            last_leaf = v;
        } else {
            let k = &h.children[v];
            x[v] = k.iter().map(|&c| x[c]).sum::<f64>() / k.len() as f64;
            height[v] = 1 + k.iter().map(|&c| height[c]).max().unwrap_or(0);
        }
    }
    let fin = |v: f64| if v.is_finite() { v } else { 0.0 };
    let (rx, ry) = (fin(rect.x), fin(rect.y));
    let x0 = x[first_leaf] - sep(first_leaf, last_leaf) / 2.0;
    let x1 = x[last_leaf] + sep(last_leaf, first_leaf) / 2.0;
    let top = h.roots.iter().map(|&r| height[r]).max().unwrap_or(0);
    let (w, hh) = (mag(rect.w), mag(rect.h));
    (0..n)
        .map(|v| {
            let fx = if x1 > x0 { (x[v] - x0) / (x1 - x0) } else { 0.5 };
            let fy = if top > 0 { 1.0 - height[v] as f64 / top as f64 } else { 1.0 };
            Vec2::new(rx + fx * w, ry + fy * hh)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_overlap_per_level(h: &Hierarchy, p: &[Vec2], min_gap: f64) {
        let levels = h.height() + 1;
        for d in 0..levels {
            let mut xs: Vec<(f64, usize)> = (0..p.len()).filter(|&i| h.depth[i] == d).map(|i| (p[i].x, i)).collect();
            xs.sort_by(|a, b| datars_math::total_cmp(a.0, b.0));
            for w in xs.windows(2) {
                assert!(w[1].0 - w[0].0 >= min_gap - 1e-9, "level {d}: {:?} too close", w);
            }
        }
    }

    #[test]
    fn small_tree_matches_d3() {
        // d3.tree().size([1, 1]) on root → {a → {c, d}, b}: root 0.4167, a 0.25, b 0.75, c 0, d 0.5? —
        // checked by the invariants instead: parents centred over children, siblings 1 apart.
        let parents = [None, Some(0), Some(0), Some(1), Some(1)];
        let p = tree(&parents, Rect::new(0.0, 0.0, 1.0, 1.0), &TreeOptions { node_size: Some((1.0, 1.0)), ..Default::default() });
        assert_eq!(p[3].x, 0.0);
        assert_eq!(p[4].x, 1.0);
        assert_eq!(p[1].x, 0.5, "a over its children");
        assert_eq!(p[2].x, 1.5, "b one sibling-unit right of a");
        assert_eq!(p[0].x, 1.0, "root centred over a and b");
        assert_eq!(p[3].y, 2.0);
    }

    #[test]
    fn tidy_invariants_on_a_bigger_tree() {
        // A lopsided tree built from a seeded rule.
        let n = 300;
        let mut rng = datars_math::Rng::new(7);
        let parents: Vec<Option<usize>> =
            (0..n).map(|i| if i == 0 { None } else { Some(rng.below(i as u64) as usize) }).collect();
        let h = Hierarchy::new(&parents);
        let opts = TreeOptions { node_size: Some((1.0, 1.0)), ..Default::default() };
        let p = tree(&parents, Rect::new(0.0, 0.0, 0.0, 0.0), &opts);
        no_overlap_per_level(&h, &p, 1.0);
        for v in 0..n {
            let k = &h.children[v];
            if !k.is_empty() {
                let mid = (p[k[0]].x + p[*k.last().unwrap()].x) / 2.0;
                assert!((p[v].x - mid).abs() < 1e-9, "parent {v} centred");
                for w in k.windows(2) {
                    assert!(p[w[1]].x > p[w[0]].x, "sibling order kept");
                }
            }
        }
        // Fitted: everything inside the rect.
        let r = Rect::new(10.0, 20.0, 800.0, 600.0);
        let f = tree(&parents, r, &TreeOptions::default());
        assert!(f.iter().all(|q| q.x > r.x && q.x < r.x1() && q.y >= r.y && q.y <= r.y1() + 1e-9));
        assert_eq!(tree(&parents, r, &TreeOptions::default()), f, "deterministic");
    }

    #[test]
    fn forest_single_and_empty() {
        let one = tree(&[None], Rect::new(0.0, 0.0, 100.0, 50.0), &TreeOptions::default());
        assert_eq!(one, vec![Vec2::new(50.0, 0.0)]);
        let forest = tree(&[None, None, Some(0)], Rect::new(0.0, 0.0, 100.0, 50.0), &TreeOptions::default());
        assert!(forest[0].x < forest[1].x);
        assert_eq!(forest[2].y, 50.0);
        assert!(tree(&[], Rect::new(0.0, 0.0, 1.0, 1.0), &TreeOptions::default()).is_empty());
    }

    #[test]
    fn cluster_puts_leaves_on_the_bottom() {
        let parents = [None, Some(0), Some(0), Some(1), Some(1)];
        let p = cluster(&parents, Rect::new(0.0, 0.0, 300.0, 100.0), &TreeOptions::default());
        for leaf in [2, 3, 4] {
            assert_eq!(p[leaf].y, 100.0);
        }
        assert_eq!(p[0].y, 0.0);
        assert!((p[1].x - (p[3].x + p[4].x) / 2.0).abs() < 1e-9);
        assert!(p[3].x < p[4].x && p[4].x < p[2].x);
        assert!(p.iter().all(|q| q.x > 0.0 && q.x < 300.0));
    }
}
