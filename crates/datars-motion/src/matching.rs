//! Matching: who becomes whom. Produces pairs (1:1), splits (1:N), merges (N:1); every other
//! element enters or exits.
//!
//! Elements are grouped by the matcher their rules resolve to (on each side); each group is
//! matched independently. All algorithms are deterministic: ties break by element order.

use crate::elements::Flat;
use crate::rules::{Matcher, Partition};
use datars_math::{total_cmp, Vec2};
use datars_scene::{Key, KeyPath, Role};
use std::collections::BTreeMap;

/// Above this many elements on a side, nearest matching is greedy instead of optimal.
pub const AUCTION_MAX: usize = 2000;

#[derive(Clone, Debug, Default)]
pub(crate) struct Corr {
    pub pairs: Vec<(usize, usize)>,
    pub splits: Vec<(usize, Vec<usize>, Partition)>,
    pub merges: Vec<(Vec<usize>, usize, Partition)>,
}

/// Pair elements with equal paths (the last occurrence of a duplicated path is the matchable
/// one).
fn by_path(fa: &Flat, fb: &Flat, a: &[usize], b: &[usize], corr: &mut Corr, used_a: &mut [bool], used_b: &mut [bool]) {
    // Two scenes from the same template list their elements in the same order (a hover, a
    // recolour, a nudged value): pair that common run in one pass, and only what follows it
    // through maps — building maps of a thousand key paths each costs more than the rest of
    // matching.
    let mut k = 0;
    while k < a.len() && k < b.len() && fa.elems[a[k]].path == fb.elems[b[k]].path {
        let (i, j) = (a[k], b[k]);
        if !used_a[i] && !used_b[j] {
            corr.pairs.push((i, j));
            used_a[i] = true;
            used_b[j] = true;
        }
        k += 1;
    }
    let (a, b) = (&a[k..], &b[k..]);
    if a.is_empty() || b.is_empty() {
        return;
    }
    let mb = fb.elem_paths(b.iter().copied());
    let ma = fa.elem_paths(a.iter().copied());
    for (p, &i) in &ma {
        if let Some(&j) = mb.get(p) {
            if !used_a[i] && !used_b[j] {
                corr.pairs.push((i, j));
                used_a[i] = true;
                used_b[j] = true;
            }
        }
    }
}

fn by_key(fa: &Flat, fb: &Flat, a: &[usize], b: &[usize], corr: &mut Corr, used_a: &mut [bool], used_b: &mut [bool]) {
    let mut ka: BTreeMap<&Key, Vec<usize>> = BTreeMap::new();
    let mut kb: BTreeMap<&Key, Vec<usize>> = BTreeMap::new();
    for &i in a {
        if !used_a[i] {
            ka.entry(&fa.elems[i].key).or_default().push(i);
        }
    }
    for &j in b {
        if !used_b[j] {
            kb.entry(&fb.elems[j].key).or_default().push(j);
        }
    }
    for (k, la) in &ka {
        let Some(lb) = kb.get(k) else { continue };
        // Same full path first, then the rest in tree order.
        for &i in la {
            if let Some(&j) = lb.iter().find(|&&j| !used_b[j] && fb.elems[j].path == fa.elems[i].path) {
                corr.pairs.push((i, j));
                used_a[i] = true;
                used_b[j] = true;
            }
        }
        let ra: Vec<usize> = la.iter().copied().filter(|&i| !used_a[i]).collect();
        let rb: Vec<usize> = lb.iter().copied().filter(|&j| !used_b[j]).collect();
        for (&i, &j) in ra.iter().zip(&rb) {
            corr.pairs.push((i, j));
            used_a[i] = true;
            used_b[j] = true;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn hierarchy(fa: &Flat, fb: &Flat, a: &[usize], b: &[usize], corr: &mut Corr, used_a: &mut [bool], used_b: &mut [bool], part_a: &[Partition], part_b: &[Partition]) {
    by_path(fa, fb, a, b, corr, used_a, used_b);
    // Children grouped by their parent key, per side.
    let children = |f: &Flat, idx: &[usize], used: &[bool]| -> BTreeMap<Key, Vec<usize>> {
        let mut m: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
        for &i in idx {
            if !used[i] {
                if let Some(pk) = f.elems[i].key.parent() {
                    m.entry(pk).or_default().push(i);
                }
            }
        }
        for v in m.values_mut() {
            v.sort_by(|&x, &y| f.elems[x].key.cmp(&f.elems[y].key).then(x.cmp(&y)));
        }
        m
    };
    let mut kids_b = children(fb, b, used_b);
    for (ai, &i) in a.iter().enumerate() {
        if used_a[i] {
            continue;
        }
        if let Some(ks) = kids_b.get_mut(&fa.elems[i].key) {
            let ks: Vec<usize> = std::mem::take(ks).into_iter().filter(|&j| !used_b[j]).collect();
            if ks.is_empty() {
                continue;
            }
            for &j in &ks {
                used_b[j] = true;
            }
            used_a[i] = true;
            let part = settle(part_a[ai], fb, &ks);
            corr.splits.push((i, ks, part));
        }
    }
    let mut kids_a = children(fa, a, used_a);
    for (bj, &j) in b.iter().enumerate() {
        if used_b[j] {
            continue;
        }
        if let Some(ks) = kids_a.get_mut(&fb.elems[j].key) {
            let ks: Vec<usize> = std::mem::take(ks).into_iter().filter(|&i| !used_a[i]).collect();
            if ks.is_empty() {
                continue;
            }
            for &i in &ks {
                used_a[i] = true;
            }
            used_b[j] = true;
            let part = settle(part_b[bj], fa, &ks);
            corr.merges.push((ks, j, part));
        }
    }
}

/// `Auto` decided by what the pieces become: compact marks (the median piece within 1:2 of square)
/// take grid cells, anything longer takes slices.
fn settle(p: Partition, f: &Flat, pieces: &[usize]) -> Partition {
    if p != Partition::Auto {
        return p;
    }
    if pieces.len() > 1 && pieces.iter().all(|&i| f.elems[i].value.is_some_and(|v| v.is_finite() && v > 0.0)) {
        return Partition::Proportional;
    }
    let mut aspects: Vec<f64> = pieces.iter().map(|&i| f.elems[i].bounds).filter(|b| b.w > 0.0 && b.h > 0.0).map(|b| b.w / b.h).collect();
    if aspects.is_empty() {
        return Partition::Slices;
    }
    aspects.sort_by(|a, b| total_cmp(*a, *b));
    let mid = aspects[aspects.len() / 2];
    if (0.5..=2.0).contains(&mid) {
        Partition::Grid
    } else {
        Partition::Slices
    }
}

/// Optimal assignment minimizing total Euclidean distance (so paths never cross), by an
/// integer forward auction with ε-scaling. Costs are quantized to 1/8 unit and scaled by
/// (N + 1) so the final ε = 1 guarantees optimality for the quantized costs. Returns pairs
/// (index into `pa`, index into `pb`) for min(|pa|, |pb|) elements.
pub fn auction(pa: &[Vec2], pb: &[Vec2]) -> Vec<(usize, usize)> {
    let (n, mb) = (pa.len(), pb.len());
    if n == 0 || mb == 0 {
        return Vec::new();
    }
    let size = n.max(mb);
    let scale = (size + 1) as i64;
    // Benefit matrix (negated cost), dummies at 0.
    let mut ben = vec![0i64; size * size];
    let mut cmax = 0i64;
    for i in 0..n {
        for j in 0..mb {
            let c = (pa[i].dist(pb[j]) * 8.0).round();
            let c = if c.is_finite() { (c as i64).min(1 << 40) } else { 1 << 40 };
            cmax = cmax.max(c);
            ben[i * size + j] = -c * scale;
        }
    }
    let mut price = vec![0i64; size];
    let mut owner: Vec<Option<usize>> = vec![None; size];
    let mut assigned: Vec<Option<usize>> = vec![None; size];
    let mut eps = ((cmax * scale) / 4).max(1);
    loop {
        owner.iter_mut().for_each(|o| *o = None);
        assigned.iter_mut().for_each(|a| *a = None);
        let mut queue: std::collections::VecDeque<usize> = (0..size).collect();
        while let Some(i) = queue.pop_front() {
            let row = &ben[i * size..(i + 1) * size];
            let (mut best, mut best_j, mut second) = (i64::MIN, 0usize, i64::MIN);
            for (j, (&bv, &p)) in row.iter().zip(&price).enumerate() {
                let v = bv - p;
                if v > best {
                    second = best;
                    best = v;
                    best_j = j;
                } else if v > second {
                    second = v;
                }
            }
            let inc = if second == i64::MIN { eps } else { best - second + eps };
            price[best_j] += inc;
            if let Some(k) = owner[best_j] {
                assigned[k] = None;
                queue.push_back(k);
            }
            owner[best_j] = Some(i);
            assigned[i] = Some(best_j);
        }
        if eps == 1 {
            break;
        }
        eps = (eps / 5).max(1);
    }
    (0..n).filter_map(|i| assigned[i].filter(|&j| j < mb).map(|j| (i, j))).collect()
}

/// Greedy nearest assignment over a uniform grid (for sets beyond `AUCTION_MAX`): each `pa`
/// point in (x, y) order takes the nearest free `pb` point.
pub fn greedy_nearest(pa: &[Vec2], pb: &[Vec2]) -> Vec<(usize, usize)> {
    if pa.is_empty() || pb.is_empty() {
        return Vec::new();
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in pb.iter().chain(pa) {
        if p.is_finite() {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
    }
    if x0 > x1 {
        return pa.iter().enumerate().zip(0..pb.len()).map(|((i, _), j)| (i, j)).collect();
    }
    let (w, h) = ((x1 - x0).max(1e-9), (y1 - y0).max(1e-9));
    let cell = ((w * h) / pb.len() as f64).sqrt().max(1e-9);
    let (gw, gh) = (((w / cell).ceil() as usize).clamp(1, 4096), ((h / cell).ceil() as usize).clamp(1, 4096));
    let cell_of = |p: Vec2| -> (usize, usize) {
        let cx = if p.x.is_finite() { (((p.x - x0) / w) * gw as f64) as usize } else { 0 };
        let cy = if p.y.is_finite() { (((p.y - y0) / h) * gh as f64) as usize } else { 0 };
        (cx.min(gw - 1), cy.min(gh - 1))
    };
    let mut grid: Vec<Vec<usize>> = vec![Vec::new(); gw * gh];
    for (j, p) in pb.iter().enumerate() {
        let (cx, cy) = cell_of(*p);
        grid[cy * gw + cx].push(j);
    }
    let mut free = vec![true; pb.len()];
    let mut remaining = pb.len();
    let mut order: Vec<usize> = (0..pa.len()).collect();
    order.sort_by(|&i, &j| total_cmp(pa[i].x, pa[j].x).then(total_cmp(pa[i].y, pa[j].y)).then(i.cmp(&j)));
    let mut out = Vec::with_capacity(pa.len().min(pb.len()));
    for i in order {
        if remaining == 0 {
            break;
        }
        let p = pa[i];
        let (cx, cy) = cell_of(p);
        let mut best: Option<(f64, usize)> = None;
        let maxr = gw.max(gh);
        for r in 0..=maxr {
            let (lx, hx) = (cx.saturating_sub(r), (cx + r).min(gw - 1));
            let (ly, hy) = (cy.saturating_sub(r), (cy + r).min(gh - 1));
            for gy in ly..=hy {
                for gx in lx..=hx {
                    if gx.abs_diff(cx) != r && gy.abs_diff(cy) != r {
                        continue;
                    }
                    let bucket = &mut grid[gy * gw + gx];
                    bucket.retain(|&j| free[j]);
                    for &j in bucket.iter() {
                        let d = p.dist(pb[j]);
                        if best.is_none_or(|(bd, bj)| d < bd || (d == bd && j < bj)) {
                            best = Some((d, j));
                        }
                    }
                }
            }
            if let Some((bd, _)) = best {
                if bd <= r as f64 * cell {
                    break;
                }
            }
        }
        if let Some((_, j)) = best {
            free[j] = false;
            remaining -= 1;
            out.push((i, j));
        }
    }
    out.sort();
    out
}

fn nearest(fa: &Flat, fb: &Flat, a: &[usize], b: &[usize], corr: &mut Corr, used_a: &mut [bool], used_b: &mut [bool]) {
    // Match within categories: shapes with shapes, text with text, instances with instances…
    let cat = |k: &str, shape: bool| -> u8 {
        if shape {
            0
        } else {
            match k {
                "text" => 1,
                "image" => 2,
                _ => 3,
            }
        }
    };
    for c in 0..4u8 {
        let ia: Vec<usize> = a.iter().copied().filter(|&i| !used_a[i] && cat(fa.elems[i].kind, fa.elems[i].is_shape) == c).collect();
        let ib: Vec<usize> = b.iter().copied().filter(|&j| !used_b[j] && cat(fb.elems[j].kind, fb.elems[j].is_shape) == c).collect();
        if ia.is_empty() || ib.is_empty() {
            continue;
        }
        let pa: Vec<Vec2> = ia.iter().map(|&i| fa.elems[i].center).collect();
        let pb: Vec<Vec2> = ib.iter().map(|&j| fb.elems[j].center).collect();
        let pairs = if ia.len().max(ib.len()) <= AUCTION_MAX { auction(&pa, &pb) } else { greedy_nearest(&pa, &pb) };
        for (x, y) in pairs {
            let (i, j) = (ia[x], ib[y]);
            corr.pairs.push((i, j));
            used_a[i] = true;
            used_b[j] = true;
        }
    }
}

/// Match elements. `ma[i]` / `mb[j]` are the resolved matchers of each side's elements (only
/// indices listed in `a` / `b` take part).
pub(crate) fn match_elements(fa: &Flat, fb: &Flat, a: &[usize], b: &[usize], ma: &[Matcher], mb: &[Matcher]) -> Corr {
    let mut corr = Corr::default();
    let mut used_a = vec![false; fa.elems.len()];
    let mut used_b = vec![false; fb.elems.len()];
    for g in 0..5u8 {
        let ga: Vec<usize> = a.iter().copied().filter(|&i| ma[i].group() == g).collect();
        let gb: Vec<usize> = b.iter().copied().filter(|&j| mb[j].group() == g).collect();
        if ga.is_empty() || gb.is_empty() {
            continue;
        }
        match g {
            0 => by_path(fa, fb, &ga, &gb, &mut corr, &mut used_a, &mut used_b),
            1 => by_key(fa, fb, &ga, &gb, &mut corr, &mut used_a, &mut used_b),
            2 => {
                let part = |m: &Matcher| match m {
                    Matcher::Hierarchy { partition } => *partition,
                    _ => Partition::Auto,
                };
                let pa: Vec<Partition> = ga.iter().map(|&i| part(&ma[i])).collect();
                let pb: Vec<Partition> = gb.iter().map(|&j| part(&mb[j])).collect();
                hierarchy(fa, fb, &ga, &gb, &mut corr, &mut used_a, &mut used_b, &pa, &pb)
            }
            3 => nearest(fa, fb, &ga, &gb, &mut corr, &mut used_a, &mut used_b),
            _ => {}
        }
    }
    // Keys are identity across recipes (P3): data elements the path matcher left unpaired — a bar
    // and the slice it becomes live at different paths — pair by their own key. Roles keep ticks
    // and labels out of it, and only keys that identify one data element on each side count (a
    // structural name like `"bar"` repeated under every datum is not an identity).
    let data = |r: Option<Role>| matches!(r, Some(Role::Datum) | Some(Role::Region));
    let ra: Vec<usize> = a.iter().copied().filter(|&i| !used_a[i] && ma[i].group() == 0 && data(fa.elems[i].role)).collect();
    let rb: Vec<usize> = b.iter().copied().filter(|&j| !used_b[j] && mb[j].group() == 0 && data(fb.elems[j].role)).collect();
    let unique = |f: &Flat, all: &[usize], idx: &[usize]| -> Vec<usize> {
        let mut n: BTreeMap<&Key, usize> = BTreeMap::new();
        for &i in all.iter().filter(|&&i| data(f.elems[i].role)) {
            *n.entry(&f.elems[i].key).or_default() += 1;
        }
        idx.iter().copied().filter(|&i| n.get(&f.elems[i].key) == Some(&1)).collect()
    };
    let (ra, rb) = (unique(fa, a, &ra), unique(fb, b, &rb));
    if !ra.is_empty() && !rb.is_empty() {
        by_key(fa, fb, &ra, &rb, &mut corr, &mut used_a, &mut used_b);
    }
    // And a data element still unpaired whose key is the parent of keys on the other side — a
    // party's bar and its seats `("S", 1…107)` — splits into them (or they merge into it), with
    // no rule needed: identity says the seats are the bar's parts.
    let parts = |f: &Flat, i: usize| data(f.elems[i].role) || f.elems[i].kind == "instance";
    let la: Vec<usize> = a.iter().copied().filter(|&i| !used_a[i] && matches!(ma[i].group(), 0 | 1) && parts(fa, i)).collect();
    let lb: Vec<usize> = b.iter().copied().filter(|&j| !used_b[j] && matches!(mb[j].group(), 0 | 1) && parts(fb, j)).collect();
    if !la.is_empty() && !lb.is_empty() {
        let (pa, pb) = (vec![Partition::Auto; la.len()], vec![Partition::Auto; lb.len()]);
        hierarchy(fa, fb, &la, &lb, &mut corr, &mut used_a, &mut used_b, &pa, &pb);
    }
    corr
}

/// For inspection: a correspondence as key paths.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Correspondence {
    pub pairs: Vec<(KeyPath, KeyPath)>,
    pub splits: Vec<(KeyPath, Vec<KeyPath>)>,
    pub merges: Vec<(Vec<KeyPath>, KeyPath)>,
    pub enters: Vec<KeyPath>,
    pub exits: Vec<KeyPath>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auction_is_optimal_on_a_small_case() {
        // Two points swapping places: crossing is worse than going straight.
        let a = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(20.0, 0.0)];
        let b = [Vec2::new(21.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(11.0, 1.0)];
        let p = auction(&a, &b);
        assert_eq!(p, vec![(0, 1), (1, 2), (2, 0)]);
    }

    #[test]
    fn auction_rectangular_picks_the_closest_subset() {
        let a = [Vec2::new(0.0, 0.0)];
        let b = [Vec2::new(50.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(-30.0, 0.0)];
        assert_eq!(auction(&a, &b), vec![(0, 1)]);
        assert_eq!(auction(&b, &a), vec![(1, 0)]);
    }

    #[test]
    fn greedy_matches_everything_it_can() {
        let a: Vec<Vec2> = (0..50).map(|i| Vec2::new(i as f64, 0.0)).collect();
        let b: Vec<Vec2> = (0..40).map(|i| Vec2::new(i as f64 + 0.25, 1.0)).collect();
        let p = greedy_nearest(&a, &b);
        assert_eq!(p.len(), 40);
        assert!(p.iter().all(|&(i, j)| i == j));
    }
}
