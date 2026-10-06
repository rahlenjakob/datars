//! Circle packing: the front-chain sibling packer and minimal enclosing circle of d3-hierarchy
//! (Wang et al., "Visualization of large hierarchical data by circle packing"), flat and nested.

use crate::hierarchy::Hierarchy;
use crate::util::mag;
use datars_math::{Rect, Rng, Vec2};
use serde::{Deserialize, Serialize};

/// A circle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Circle {
    pub center: Vec2,
    pub r: f64,
}

impl Circle {
    pub const fn new(center: Vec2, r: f64) -> Circle {
        Circle { center, r }
    }
}

/// The result of [`pack`]: the packed circles (indexed like the input) and their enclosing circle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackLayout {
    pub circles: Vec<Circle>,
    pub enclosing: Circle,
}

/// Packs circles of the given radii tightly around each other, in input order (pack them sorted by
/// descending radius for the tightest, roundest result). The layout is translated so the enclosing
/// circle is centred on the origin. Non-finite and negative radii count as 0.
///
/// Front-chain algorithm: each circle is placed tangent to two neighbours on the current front and
/// the front is repaired when it intersects; then the minimal enclosing circle is computed with
/// Welzl's algorithm over the front. Deterministic: the only randomness (Welzl's shuffle) uses a
/// fixed seed.
pub fn pack(radii: &[f64]) -> PackLayout {
    let mut cs: Vec<Circle> = radii.iter().map(|&r| Circle::new(Vec2::ZERO, mag(r))).collect();
    let enclosing = pack_siblings(&mut cs);
    PackLayout { circles: cs, enclosing }
}

/// Packs one circle per value (area ∝ value) and scales the result to fit inside `bounds`, centred.
/// `padding` is the minimum gap between circles, in output units. Returns circles indexed like
/// `values` (zero values get radius 0 at the centre of their slot in the packing).
pub fn pack_values(values: &[f64], bounds: Rect, padding: f64) -> Vec<Circle> {
    let mut parents = vec![Some(0usize); values.len() + 1];
    parents[0] = None;
    let mut vals = vec![0.0];
    vals.extend_from_slice(values);
    let mut out = pack_hierarchy(&parents, &vals, bounds, padding);
    out.remove(0);
    out
}

/// Nested circle packing over a parent-index hierarchy (see [`crate::hierarchy`]): leaves get area ∝
/// value, each internal node encloses its packed children, and the whole thing is scaled to fit
/// `bounds`, centred. `padding` is the gap between siblings and between children and their parent's
/// circle, in output units. As in d3.pack, a first pass without padding fixes the scale and later
/// passes pack with the padding converted into value units; unlike d3 (which stops after one such
/// pass, leaving gaps a little under `padding`), the conversion is iterated to its fixed point.
///
/// Returns one circle per node, indexed like `parents`; several roots are packed as siblings inside
/// an invisible root. Deterministic; non-finite and negative values count as 0.
pub fn pack_hierarchy(parents: &[Option<usize>], values: &[f64], bounds: Rect, padding: f64) -> Vec<Circle> {
    let h = Hierarchy::new(parents);
    let n = h.len();
    if n == 0 {
        return Vec::new();
    }
    let sums = h.sums(values);
    // Node n is the invisible root holding the real roots.
    let mut kids: Vec<Vec<usize>> = h.children.clone();
    kids.push(h.roots.clone());
    let mut post = h.postorder();
    post.push(n);
    let mut r = vec![0.0; n + 1];
    let mut rel = vec![Vec2::ZERO; n + 1]; // centre relative to the parent's centre
    for v in 0..n {
        if kids[v].is_empty() {
            r[v] = sums[v].sqrt();
        }
    }
    // One packing pass with every child inflated by `pad` (value units): siblings end up 2·pad
    // apart and children 2·pad inside their parent. The invisible root adds no outer gap, so the
    // real roots touch the bounds.
    let pack_pass = |r: &mut [f64], rel: &mut [Vec2], pad: f64| {
        for &v in &post {
            if kids[v].is_empty() {
                continue;
            }
            let mut cs: Vec<Circle> = kids[v].iter().map(|&c| Circle::new(Vec2::ZERO, r[c] + pad)).collect();
            let e = pack_siblings(&mut cs);
            for (k, &c) in kids[v].iter().enumerate() {
                rel[c] = cs[k].center;
            }
            r[v] = if v == n { e.r - pad } else { e.r + pad };
        }
    };
    pack_pass(&mut r, &mut rel, 0.0);
    let (bw, bh) = (mag(bounds.w), mag(bounds.h));
    let side = bw.min(bh);
    let pad = mag(padding);
    if pad > 0.0 && r[n] > 0.0 && side > 0.0 {
        // The padding is given in output units but applied in value units, and the scale depends on
        // the padded result: iterate the fixed point `pad_v = pad · R(pad_v) / side` (d3 stops after
        // the first step, which leaves gaps a little under `pad`).
        let mut pv = pad * r[n] / side;
        for _ in 0..8 {
            pack_pass(&mut r, &mut rel, pv);
            let next = pad * r[n] / side;
            if (next - pv).abs() <= 1e-12 * next {
                break;
            }
            pv = next;
        }
    }
    let k = if r[n] > 0.0 { side / (2.0 * r[n]) } else { 0.0 };
    let bx = if bounds.x.is_finite() { bounds.x } else { 0.0 };
    let by = if bounds.y.is_finite() { bounds.y } else { 0.0 };
    let mut pos = vec![Vec2::ZERO; n + 1];
    pos[n] = Vec2::new(bx + bw / 2.0, by + bh / 2.0);
    let mut out = vec![Circle::default(); n];
    // Pre-order from the invisible root: a node's position is its parent's plus its scaled offset.
    let mut stack = vec![n];
    while let Some(v) = stack.pop() {
        for &c in kids[v].iter().rev() {
            pos[c] = pos[v] + rel[c] * k;
            out[c] = Circle::new(pos[c], r[c] * k);
            stack.push(c);
        }
    }
    out
}

/// The smallest circle enclosing all `circles` (Welzl's algorithm, deterministic shuffle).
/// Empty input gives a zero circle at the origin.
pub fn enclose(circles: &[Circle]) -> Circle {
    let mut cs: Vec<Circle> =
        circles.iter().filter(|c| c.center.is_finite()).map(|c| Circle::new(c.center, mag(c.r))).collect();
    if cs.is_empty() {
        return Circle::default();
    }
    // Fisher–Yates with a fixed seed: Welzl's expected O(n) needs a random order, determinism
    // needs it to be the same one every time.
    let mut rng = Rng::new(0x5EED_C1C1E);
    for i in (1..cs.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        cs.swap(i, j);
    }
    let mut basis: Vec<Circle> = Vec::new();
    let mut e: Option<Circle> = None;
    let mut i = 0;
    let mut guard = 0usize;
    let limit = 64 * cs.len() * cs.len() + 64;
    while i < cs.len() {
        let p = cs[i];
        match e {
            Some(c) if encloses_weak(&c, &p) => i += 1,
            _ => {
                basis = extend_basis(&basis, &p);
                e = Some(enclose_basis(&basis));
                i = 0;
            }
        }
        guard += 1;
        if guard > limit {
            break;
        }
    }
    let e = e.unwrap_or_default();
    if e.r.is_finite() && e.center.is_finite() && cs.iter().all(|c| encloses_weak(&e, c)) {
        e
    } else {
        bounding_circle(&cs)
    }
}

/// A (non-minimal) enclosing circle — the fallback if numerical trouble ever defeats Welzl.
fn bounding_circle(cs: &[Circle]) -> Circle {
    let (mut lo, mut hi) = (Vec2::new(f64::INFINITY, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
    for c in cs {
        lo.x = lo.x.min(c.center.x - c.r);
        lo.y = lo.y.min(c.center.y - c.r);
        hi.x = hi.x.max(c.center.x + c.r);
        hi.y = hi.y.max(c.center.y + c.r);
    }
    let center = (lo + hi) / 2.0;
    let r = cs.iter().map(|c| c.center.dist(center) + c.r).fold(0.0, f64::max);
    Circle::new(center, r)
}

fn extend_basis(b: &[Circle], p: &Circle) -> Vec<Circle> {
    if b.iter().all(|q| encloses_weak(p, q)) {
        return vec![*p];
    }
    for i in 0..b.len() {
        if encloses_not(p, &b[i]) && encloses_weak_all(&enclose2(&b[i], p), b) {
            return vec![b[i], *p];
        }
    }
    for i in 0..b.len().saturating_sub(1) {
        for j in i + 1..b.len() {
            if encloses_not(&enclose2(&b[i], &b[j]), p)
                && encloses_not(&enclose2(&b[i], p), &b[j])
                && encloses_not(&enclose2(&b[j], p), &b[i])
                && encloses_weak_all(&enclose3(&b[i], &b[j], p), b)
            {
                return vec![b[i], b[j], *p];
            }
        }
    }
    // Numerically degenerate: fall back to the point alone plus whatever it doesn't cover.
    let mut all = b.to_vec();
    all.push(*p);
    vec![bounding_circle(&all)]
}

fn encloses_not(a: &Circle, b: &Circle) -> bool {
    let dr = a.r - b.r;
    let d = b.center - a.center;
    dr < 0.0 || dr * dr < d.len2()
}

fn encloses_weak(a: &Circle, b: &Circle) -> bool {
    let dr = a.r - b.r + a.r.max(b.r).max(1.0) * 1e-9;
    let d = b.center - a.center;
    dr > 0.0 && dr * dr > d.len2()
}

fn encloses_weak_all(a: &Circle, b: &[Circle]) -> bool {
    b.iter().all(|q| encloses_weak(a, q))
}

fn enclose_basis(b: &[Circle]) -> Circle {
    match b.len() {
        1 => b[0],
        2 => enclose2(&b[0], &b[1]),
        3 => enclose3(&b[0], &b[1], &b[2]),
        _ => Circle::default(),
    }
}

fn enclose2(a: &Circle, b: &Circle) -> Circle {
    let d = b.center - a.center;
    let l = d.len();
    if l == 0.0 {
        return if a.r >= b.r { *a } else { *b };
    }
    let r21 = b.r - a.r;
    Circle::new(
        Vec2::new((a.center.x + b.center.x + d.x / l * r21) / 2.0, (a.center.y + b.center.y + d.y / l * r21) / 2.0),
        (l + a.r + b.r) / 2.0,
    )
}

fn enclose3(a: &Circle, b: &Circle, c: &Circle) -> Circle {
    let (x1, y1, r1) = (a.center.x, a.center.y, a.r);
    let (x2, y2, r2) = (b.center.x, b.center.y, b.r);
    let (x3, y3, r3) = (c.center.x, c.center.y, c.r);
    let a2 = x1 - x2;
    let a3 = x1 - x3;
    let b2 = y1 - y2;
    let b3 = y1 - y3;
    let c2 = r2 - r1;
    let c3 = r3 - r1;
    let d1 = x1 * x1 + y1 * y1 - r1 * r1;
    let d2 = d1 - x2 * x2 - y2 * y2 + r2 * r2;
    let d3 = d1 - x3 * x3 - y3 * y3 + r3 * r3;
    let ab = a3 * b2 - a2 * b3;
    let xa = (b2 * d3 - b3 * d2) / (ab * 2.0) - x1;
    let xb = (b3 * c2 - b2 * c3) / ab;
    let ya = (a3 * d2 - a2 * d3) / (ab * 2.0) - y1;
    let yb = (a2 * c3 - a3 * c2) / ab;
    let qa = xb * xb + yb * yb - 1.0;
    let qb = 2.0 * (r1 + xa * xb + ya * yb);
    let qc = xa * xa + ya * ya - r1 * r1;
    let r = -(if qa.abs() > 1e-6 { (qb + (qb * qb - 4.0 * qa * qc).max(0.0).sqrt()) / (2.0 * qa) } else { qc / qb });
    Circle::new(Vec2::new(x1 + xa + xb * r, y1 + ya + yb * r), r)
}

/// Places `c` tangent to `a` and `b` (on the side that keeps the front chain turning one way).
fn place(b: &Circle, a: &Circle, c: &mut Circle) {
    let d = b.center - a.center;
    let d2 = d.len2();
    if d2 > 0.0 {
        let a2 = (a.r + c.r) * (a.r + c.r);
        let b2 = (b.r + c.r) * (b.r + c.r);
        if a2 > b2 {
            let x = (d2 + b2 - a2) / (2.0 * d2);
            let y = (b2 / d2 - x * x).max(0.0).sqrt();
            c.center = Vec2::new(b.center.x - x * d.x - y * d.y, b.center.y - x * d.y + y * d.x);
        } else {
            let x = (d2 + a2 - b2) / (2.0 * d2);
            let y = (a2 / d2 - x * x).max(0.0).sqrt();
            c.center = Vec2::new(a.center.x + x * d.x - y * d.y, a.center.y + x * d.y + y * d.x);
        }
    } else {
        c.center = Vec2::new(a.center.x + c.r, a.center.y);
    }
}

fn intersects(a: &Circle, b: &Circle) -> bool {
    let dr = a.r + b.r - 1e-6;
    let d = b.center - a.center;
    dr > 0.0 && dr * dr > d.len2()
}

/// Packs `cs` in place (positions only) and returns the enclosing circle, after translating
/// everything so it's centred on the origin.
fn pack_siblings(cs: &mut [Circle]) -> Circle {
    let n = cs.len();
    if n == 0 {
        return Circle::default();
    }
    cs[0].center = Vec2::ZERO;
    if n == 1 {
        return Circle::new(Vec2::ZERO, cs[0].r);
    }
    cs[0].center = Vec2::new(-cs[1].r, 0.0);
    cs[1].center = Vec2::new(cs[0].r, 0.0);
    if n == 2 {
        let e = enclose(&cs[..2]);
        for c in cs.iter_mut() {
            c.center -= e.center;
        }
        return Circle::new(Vec2::ZERO, e.r);
    }
    let (c0, c1) = (cs[0], cs[1]);
    place(&c1, &c0, &mut cs[2]);
    // The front chain as a circular doubly linked list over circle indices.
    let mut next = vec![0usize; n];
    let mut prev = vec![0usize; n];
    let (mut a, mut b) = (0usize, 1usize);
    next[0] = 1;
    prev[1] = 0;
    next[1] = 2;
    prev[2] = 1;
    next[2] = 0;
    prev[0] = 2;
    let score = |cs: &[Circle], node: usize, next: &[usize]| {
        let (p, q) = (cs[node], cs[next[node]]);
        let ab = p.r + q.r;
        if ab <= 0.0 {
            return p.center.len2();
        }
        let dx = (p.center.x * q.r + q.center.x * p.r) / ab;
        let dy = (p.center.y * q.r + q.center.y * p.r) / ab;
        dx * dx + dy * dy
    };
    let mut i = 3;
    let mut restarts = 0usize;
    'pack: while i < n {
        let (ca, cb) = (cs[a], cs[b]);
        place(&ca, &cb, &mut cs[i]);
        // Find the closest circle on the front chain that intersects the new one, looking ahead
        // (from b) and behind (from a) by accumulated radius.
        let (mut j, mut k) = (next[b], prev[a]);
        let (mut sj, mut sk) = (cs[b].r, cs[a].r);
        loop {
            if sj <= sk {
                if intersects(&cs[j], &cs[i]) && restarts < 4 * n * n + 16 {
                    b = j;
                    next[a] = b;
                    prev[b] = a;
                    restarts += 1;
                    continue 'pack;
                }
                sj += cs[j].r;
                j = next[j];
            } else {
                if intersects(&cs[k], &cs[i]) && restarts < 4 * n * n + 16 {
                    a = k;
                    next[a] = b;
                    prev[b] = a;
                    restarts += 1;
                    continue 'pack;
                }
                sk += cs[k].r;
                k = prev[k];
            }
            if j == next[k] {
                break;
            }
        }
        // Insert i between a and b.
        prev[i] = a;
        next[i] = b;
        next[a] = i;
        prev[b] = i;
        b = i;
        // The new closest pair to the centroid.
        let mut best = score(cs, a, &next);
        let mut c = next[b];
        while c != b {
            let s = score(cs, c, &next);
            if s < best {
                a = c;
                best = s;
            }
            c = next[c];
        }
        b = next[a];
        i += 1;
    }
    // Enclose the front chain, then centre everything on it.
    let mut front = vec![cs[b]];
    let mut c = next[b];
    while c != b {
        front.push(cs[c]);
        c = next[c];
    }
    let e = enclose(&front);
    for c in cs.iter_mut() {
        c.center -= e.center;
    }
    Circle::new(Vec2::ZERO, e.r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_no_overlap(cs: &[Circle], tol: f64) {
        for i in 0..cs.len() {
            for j in i + 1..cs.len() {
                let d = cs[i].center.dist(cs[j].center);
                assert!(d + tol >= cs[i].r + cs[j].r, "{i} and {j} overlap: d={d} r={} {}", cs[i].r, cs[j].r);
            }
        }
    }

    fn assert_encloses(e: &Circle, cs: &[Circle], tol: f64) {
        for c in cs {
            assert!(c.center.dist(e.center) + c.r <= e.r + tol, "{c:?} outside {e:?}");
        }
    }

    #[test]
    fn packs_without_overlap_and_encloses() {
        let radii: Vec<f64> = (0..60).map(|i| 1.0 + ((i * 7919) % 13) as f64).collect();
        let p = pack(&radii);
        assert_no_overlap(&p.circles, 1e-6);
        assert_encloses(&p.enclosing, &p.circles, 1e-6);
        assert!(p.enclosing.center.len() < 1e-9, "centred on the origin");
        // Reasonably tight: total circle area is a decent share of the enclosing disc.
        let area: f64 = radii.iter().map(|r| r * r).sum();
        assert!(area / (p.enclosing.r * p.enclosing.r) > 0.5);
        assert_eq!(pack(&radii), p, "deterministic");
    }

    #[test]
    fn small_and_degenerate() {
        assert_eq!(pack(&[]).circles.len(), 0);
        let one = pack(&[3.0]);
        assert_eq!(one.enclosing.r, 3.0);
        let two = pack(&[1.0, 2.0]);
        assert!((two.enclosing.r - 3.0).abs() < 1e-9);
        assert_no_overlap(&two.circles, 1e-9);
        let zeros = pack(&[0.0, f64::NAN, -1.0, 0.0]);
        assert!(zeros.circles.iter().all(|c| c.center.is_finite() && c.r == 0.0));
    }

    #[test]
    fn enclose_known_cases() {
        let e = enclose(&[Circle::new(Vec2::new(0.0, 0.0), 1.0), Circle::new(Vec2::new(4.0, 0.0), 1.0)]);
        assert!((e.r - 3.0).abs() < 1e-9 && (e.center.x - 2.0).abs() < 1e-9);
        let tri = [
            Circle::new(Vec2::new(0.0, 0.0), 0.0),
            Circle::new(Vec2::new(2.0, 0.0), 0.0),
            Circle::new(Vec2::new(1.0, 3.0_f64.sqrt()), 0.0),
        ];
        let e = enclose(&tri);
        assert!((e.r - 2.0 / 3.0_f64.sqrt()).abs() < 1e-9);
        assert_eq!(enclose(&[]), Circle::default());
    }

    #[test]
    fn values_fit_bounds_with_padding() {
        let values = [10.0, 5.0, 5.0, 3.0, 1.0, 1.0, 0.5, 20.0];
        let b = Rect::new(0.0, 0.0, 400.0, 300.0);
        let cs = pack_values(&values, b, 4.0);
        assert_eq!(cs.len(), values.len());
        let e = Circle::new(Vec2::new(200.0, 150.0), 150.0);
        assert_encloses(&e, &cs, 1e-6);
        assert_no_overlap(&cs, -4.0 + 1e-6); // at least the padding apart
        // Area ∝ value: r² / value is constant.
        let k = cs[0].r * cs[0].r / 10.0;
        for (c, v) in cs.iter().zip(values) {
            assert!((c.r * c.r / v - k).abs() < 1e-6 * k);
        }
    }

    #[test]
    fn hierarchy_children_inside_parents() {
        let parents = [None, Some(0), Some(0), Some(1), Some(1), Some(1), Some(2), Some(2)];
        let values = [0.0, 0.0, 0.0, 4.0, 2.0, 1.0, 3.0, 3.0];
        let cs = pack_hierarchy(&parents, &values, Rect::new(0.0, 0.0, 500.0, 500.0), 3.0);
        assert!((cs[0].r - 250.0).abs() < 1e-6, "root fills the bounds");
        for (c, p) in [(1, 0), (2, 0), (3, 1), (4, 1), (5, 1), (6, 2), (7, 2)] {
            assert!(cs[c].center.dist(cs[p].center) + cs[c].r <= cs[p].r - 3.0 + 1e-6, "{c} inside {p}");
        }
        assert_no_overlap(&[cs[1], cs[2]], -3.0 + 1e-6);
        assert_no_overlap(&[cs[3], cs[4], cs[5]], -3.0 + 1e-6);
        assert!(pack_hierarchy(&[], &[], Rect::new(0.0, 0.0, 1.0, 1.0), 0.0).is_empty());
    }
}
