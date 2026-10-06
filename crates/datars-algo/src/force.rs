//! Deterministic force-directed layout (the d3-force model: velocity Verlet with cooling, link
//! springs, many-body charge with Barnes–Hut, centring, collision).

use crate::util::{finite, mag};
use datars_math::{m, total_cmp, Rect, Rng, Vec2};
use serde::{Deserialize, Serialize};

/// Options for [`force`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ForceOptions {
    /// Simulation steps. Cooling is scaled so the temperature reaches d3's minimum on the last step.
    pub iterations: usize,
    /// Seed for the initial rotation and for separating coincident nodes.
    pub seed: u64,
    /// Many-body strength per node: negative repels (d3: −30), positive attracts, 0 disables.
    pub charge: f64,
    /// Rest length of links.
    pub link_distance: f64,
    /// The layout's centre: each step translates all nodes so their mean sits here.
    pub center: Vec2,
    /// Radius for collision avoidance (0 disables).
    pub collide_radius: f64,
    /// Barnes–Hut accuracy: cells whose size / distance is below `theta` act as one body
    /// (d3: 0.9). 0 computes every pair exactly (O(n²) per step).
    pub theta: f64,
    /// Pull of every node toward `center` (d3's forceX/forceY strength; 0 disables). Keeps
    /// disconnected components from drifting apart.
    pub gravity: f64,
    /// Friction: the fraction of velocity lost per step (d3: 0.4).
    pub velocity_decay: f64,
}

impl Default for ForceOptions {
    fn default() -> Self {
        ForceOptions {
            iterations: 300,
            seed: 1,
            charge: -30.0,
            link_distance: 30.0,
            center: Vec2::ZERO,
            collide_radius: 0.0,
            theta: 0.9,
            gravity: 0.0,
            velocity_decay: 0.4,
        }
    }
}

/// Force-directed positions for `n` nodes joined by `links` `(a, b, strength)`.
///
/// Starts from a phyllotaxis spiral around `opts.center` rotated by a seed-derived angle (evenly
/// spread, no coincident points), then runs exactly `opts.iterations` steps of the d3-force model:
/// link springs pull linked nodes toward `link_distance` (strength = the link's `strength` × d3's
/// default `1 / min(degree)`, clamped to 1, split by degree so hubs move less), many-body charge
/// (Barnes–Hut quadtree), centring, optional gravity toward the centre and collision.
///
/// Deterministic: fixed iteration count and cooling schedule, seeded jitter only when two nodes
/// coincide exactly, a fixed order of force application and summation. Links with an out-of-range
/// endpoint or `a == b` are ignored; a non-finite strength counts as 1.
pub fn force(n: usize, links: &[(usize, usize, f64)], opts: &ForceOptions) -> Vec<Vec2> {
    let mut rng = Rng::new(opts.seed);
    let rot = rng.range(0.0, m::TAU);
    let c = if opts.center.is_finite() { opts.center } else { Vec2::ZERO };
    let golden = m::PI * (3.0 - 5f64.sqrt());
    let init: Vec<Vec2> = (0..n)
        .map(|i| {
            let r = 10.0 * (0.5 + i as f64).sqrt();
            let (s, co) = m::sin_cos(rot + i as f64 * golden);
            Vec2::new(c.x + r * co, c.y + r * s)
        })
        .collect();
    force_from(&init, links, opts)
}

/// [`force`] starting from given positions (e.g. the previous layout, for stability across data
/// updates). Non-finite starting points are replaced by the centre plus seeded jitter.
pub fn force_from(initial: &[Vec2], links: &[(usize, usize, f64)], opts: &ForceOptions) -> Vec<Vec2> {
    simulate(initial, links, None, None, opts)
}

/// [`force_from`] for nodes of different sizes, in a box: node `i` collides as a circle of radius
/// `radii[i]` plus `opts.collide_radius` (padding; a missing, negative or non-finite radius is 0),
/// and with `bounds` every node is kept inside it after each step — its whole circle where it fits,
/// else centred across — so a network laid out for a chart never leaves its plot. Collisions are
/// resolved twice per step (d3's `forceCollide().iterations(2)`), so circles end up touching at
/// most, not overlapping, even when many-body charge is weak.
///
/// Deterministic like [`force`]; the same inputs give the same bits on every target.
pub fn force_within(initial: &[Vec2], links: &[(usize, usize, f64)], radii: &[f64], bounds: Option<Rect>, opts: &ForceOptions) -> Vec<Vec2> {
    let pad = mag(opts.collide_radius);
    let r: Vec<f64> = (0..initial.len()).map(|i| mag(radii.get(i).copied().unwrap_or(0.0)) + pad).collect();
    let b = bounds.filter(|b| b.x.is_finite() && b.y.is_finite() && b.w.is_finite() && b.h.is_finite() && b.w >= 0.0 && b.h >= 0.0);
    simulate(initial, links, Some(&r), b, opts)
}

/// The simulation behind [`force_from`] (uniform `opts.collide_radius`, unbounded: `radii` and
/// `bounds` are `None`) and [`force_within`].
fn simulate(initial: &[Vec2], links: &[(usize, usize, f64)], radii: Option<&[f64]>, bounds: Option<Rect>, opts: &ForceOptions) -> Vec<Vec2> {
    let n = initial.len();
    let mut rng = Rng::new(opts.seed ^ 0x9E37_79B9_7F4A_7C15);
    let c = if opts.center.is_finite() { opts.center } else { Vec2::ZERO };
    let mut p: Vec<Vec2> = initial
        .iter()
        .map(|q| if q.is_finite() { *q } else { c + Vec2::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)) })
        .collect();
    if n == 0 {
        return p;
    }
    let mut v = vec![Vec2::ZERO; n];
    let links: Vec<(usize, usize, f64)> = links
        .iter()
        .filter(|l| l.0 < n && l.1 < n && l.0 != l.1)
        .map(|l| (l.0, l.1, if l.2.is_finite() { l.2.max(0.0) } else { 1.0 }))
        .collect();
    let mut degree = vec![0usize; n];
    for l in &links {
        degree[l.0] += 1;
        degree[l.1] += 1;
    }
    let strength: Vec<f64> = links.iter().map(|l| (l.2 / degree[l.0].min(degree[l.1]) as f64).min(1.0)).collect();
    let bias: Vec<f64> = links.iter().map(|l| degree[l.0] as f64 / (degree[l.0] + degree[l.1]) as f64).collect();
    let distance = mag(opts.link_distance);
    let charge = finite(opts.charge);
    let theta2 = mag(opts.theta) * mag(opts.theta);
    let radius = mag(opts.collide_radius);
    let gravity = mag(opts.gravity);
    let keep = 1.0 - finite(opts.velocity_decay).clamp(0.0, 1.0);
    let iterations = opts.iterations;
    let alpha_min = 0.001;
    let decay = if iterations > 0 { 1.0 - m::pow(alpha_min, 1.0 / iterations as f64) } else { 0.0 };
    let mut alpha = 1.0;
    let mut jiggle = move || (rng.next_f64() - 0.5) * 1e-6;
    let mut tree = QuadTree::default();
    for _ in 0..iterations {
        alpha += (0.0 - alpha) * decay;
        // Links.
        for (k, &(a, b, _)) in links.iter().enumerate() {
            let mut d = p[b] + v[b] - p[a] - v[a];
            if d.x == 0.0 {
                d.x = jiggle();
            }
            if d.y == 0.0 {
                d.y = jiggle();
            }
            let l = d.len();
            let f = (l - distance) / l * alpha * strength[k];
            let d = d * f;
            v[b] -= d * bias[k];
            v[a] += d * (1.0 - bias[k]);
        }
        // Many-body.
        if charge != 0.0 {
            tree.build(&p);
            for i in 0..n {
                let dv = tree.force_on(i, &p, charge * alpha, theta2, &mut jiggle);
                v[i] += dv;
            }
        }
        // Centre (moves positions, like d3.forceCenter).
        let mean = p.iter().fold(Vec2::ZERO, |s, q| s + *q) / n as f64;
        let shift = mean - c;
        for q in p.iter_mut() {
            *q -= shift;
        }
        // Gravity toward the centre.
        if gravity > 0.0 {
            for i in 0..n {
                v[i] += (c - p[i]) * (gravity * alpha);
            }
        }
        // Collisions.
        match radii {
            None => {
                if radius > 0.0 {
                    collide(&p, &mut v, radius, &mut jiggle);
                }
            }
            Some(rs) => {
                for _ in 0..2 {
                    collide_radii(&p, &mut v, rs, &mut jiggle);
                }
            }
        }
        for i in 0..n {
            v[i] = v[i] * keep;
            p[i] += v[i];
        }
        if let Some(b) = bounds {
            let rs = radii.unwrap_or(&[]);
            for i in 0..n {
                let r = rs.get(i).copied().unwrap_or(0.0);
                p[i] = Vec2::new(keep_in(p[i].x, b.x, b.x1(), r), keep_in(p[i].y, b.y, b.y1(), r));
            }
        }
    }
    p
}

/// `x` kept in `[lo + r, hi - r]`, or the middle when that's empty.
fn keep_in(x: f64, lo: f64, hi: f64, r: f64) -> f64 {
    if hi - lo <= 2.0 * r {
        (lo + hi) / 2.0
    } else {
        x.clamp(lo + r, hi - r)
    }
}

/// d3.forceCollide with a radius per node (strength 1, one pass): overlapping pairs are pushed
/// apart along the line between their predicted positions, the smaller circle moving more (by the
/// other's share of the squared radii). A sweep over predicted x finds the pairs.
fn collide_radii(p: &[Vec2], v: &mut [Vec2], rs: &[f64], jiggle: &mut impl FnMut() -> f64) {
    let n = p.len();
    let q: Vec<Vec2> = (0..n).map(|i| p[i] + v[i]).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| total_cmp(q[a].x, q[b].x).then(a.cmp(&b)));
    let rmax = rs.iter().copied().fold(0.0, f64::max);
    for (k, &i) in order.iter().enumerate() {
        let ri = rs[i];
        for &j in &order[k + 1..] {
            if q[j].x - q[i].x >= ri + rmax {
                break;
            }
            let rr = ri + rs[j];
            if rr <= 0.0 {
                continue;
            }
            let mut d = q[i] - q[j];
            let mut l = d.len2();
            if l >= rr * rr {
                continue;
            }
            if d.x == 0.0 {
                d.x = jiggle();
                l += d.x * d.x;
            }
            if d.y == 0.0 {
                d.y = jiggle();
                l += d.y * d.y;
            }
            let len = l.sqrt();
            let push = d * ((rr - len) / len);
            let (ai, aj) = (ri * ri, rs[j] * rs[j]);
            let share = if ai + aj > 0.0 { aj / (ai + aj) } else { 0.5 };
            v[i] += push * share;
            v[j] -= push * (1.0 - share);
        }
    }
}

/// d3.forceCollide (strength 1, one pass) with a sweep over predicted x instead of a quadtree.
fn collide(p: &[Vec2], v: &mut [Vec2], r: f64, jiggle: &mut impl FnMut() -> f64) {
    let n = p.len();
    let q: Vec<Vec2> = (0..n).map(|i| p[i] + v[i]).collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| total_cmp(q[a].x, q[b].x).then(a.cmp(&b)));
    let rr = 2.0 * r;
    for (k, &i) in order.iter().enumerate() {
        for &j in &order[k + 1..] {
            if q[j].x - q[i].x >= rr {
                break;
            }
            let mut d = q[i] - q[j];
            let mut l = d.len2();
            if l >= rr * rr {
                continue;
            }
            if d.x == 0.0 {
                d.x = jiggle();
                l += d.x * d.x;
            }
            if d.y == 0.0 {
                d.y = jiggle();
                l += d.y * d.y;
            }
            let len = l.sqrt();
            let f = (rr - len) / len;
            let push = d * (f * 0.5);
            v[i] += push;
            v[j] -= push;
        }
    }
}

const EMPTY: u32 = u32::MAX;

/// A Barnes–Hut quadtree rebuilt every step: nodes in an arena, points chained per leaf.
#[derive(Default)]
struct QuadTree {
    nodes: Vec<QNode>,
    next_point: Vec<u32>,
}

#[derive(Clone, Copy)]
struct QNode {
    x0: f64,
    y0: f64,
    size: f64,
    children: [u32; 4],
    /// First point of a leaf's chain (EMPTY for internal nodes or empty leaves).
    point: u32,
    count: u32,
    sum: Vec2,
}

impl QNode {
    fn leaf(x0: f64, y0: f64, size: f64) -> QNode {
        QNode { x0, y0, size, children: [EMPTY; 4], point: EMPTY, count: 0, sum: Vec2::ZERO }
    }
    fn is_leaf(&self) -> bool {
        self.children == [EMPTY; 4]
    }
}

impl QuadTree {
    fn build(&mut self, p: &[Vec2]) {
        self.nodes.clear();
        self.next_point.clear();
        self.next_point.resize(p.len(), EMPTY);
        let (mut lo, mut hi) = (Vec2::new(f64::INFINITY, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for q in p {
            lo.x = lo.x.min(q.x);
            lo.y = lo.y.min(q.y);
            hi.x = hi.x.max(q.x);
            hi.y = hi.y.max(q.y);
        }
        let size = (hi.x - lo.x).max(hi.y - lo.y).max(1e-9) * (1.0 + 1e-9);
        self.nodes.push(QNode::leaf(lo.x, lo.y, size));
        for i in 0..p.len() {
            self.insert(i as u32, p);
        }
    }

    fn quadrant(n: &QNode, q: Vec2) -> usize {
        let h = n.size / 2.0;
        let right = (q.x >= n.x0 + h) as usize;
        let below = (q.y >= n.y0 + h) as usize;
        right | (below << 1)
    }

    fn insert(&mut self, i: u32, p: &[Vec2]) {
        let q = p[i as usize];
        let mut node = 0usize;
        let mut depth = 0;
        loop {
            self.nodes[node].count += 1;
            self.nodes[node].sum += q;
            if self.nodes[node].is_leaf() {
                let first = self.nodes[node].point;
                if first == EMPTY {
                    self.nodes[node].point = i;
                    return;
                }
                // Coincident points (or a deep enough tree) share the leaf.
                if p[first as usize] == q || depth >= 48 {
                    self.next_point[i as usize] = first;
                    self.nodes[node].point = i;
                    return;
                }
                // Split: push the existing chain (coincident points) down into a child.
                let n = self.nodes[node];
                let h = n.size / 2.0;
                let quad = Self::quadrant(&n, p[first as usize]);
                let (mut count, mut sum, mut j) = (0u32, Vec2::ZERO, first);
                while j != EMPTY {
                    count += 1;
                    sum += p[j as usize];
                    j = self.next_point[j as usize];
                }
                let child = self.nodes.len() as u32;
                self.nodes.push(QNode {
                    count,
                    sum,
                    point: first,
                    ..QNode::leaf(n.x0 + h * (quad & 1) as f64, n.y0 + h * (quad >> 1) as f64, h)
                });
                self.nodes[node].point = EMPTY;
                self.nodes[node].children[quad] = child;
            }
            let n = self.nodes[node];
            let quad = Self::quadrant(&n, q);
            if n.children[quad] == EMPTY {
                let h = n.size / 2.0;
                let child = self.nodes.len() as u32;
                self.nodes.push(QNode::leaf(n.x0 + h * (quad & 1) as f64, n.y0 + h * (quad >> 1) as f64, h));
                self.nodes[node].children[quad] = child;
            }
            node = self.nodes[node].children[quad] as usize;
            depth += 1;
        }
    }

    /// Velocity change on point `i` from all charges (`k` = charge × alpha).
    fn force_on(&self, i: usize, p: &[Vec2], k: f64, theta2: f64, jiggle: &mut impl FnMut() -> f64) -> Vec2 {
        let me = p[i];
        let mut dv = Vec2::ZERO;
        let mut stack = vec![0u32];
        let dist_min2 = 1.0;
        while let Some(ni) = stack.pop() {
            let n = &self.nodes[ni as usize];
            if n.count == 0 {
                continue;
            }
            let centroid = n.sum / n.count as f64;
            let mut d = centroid - me;
            let mut l = d.len2();
            let w = n.size;
            let far = theta2 > 0.0 && w * w / theta2 < l;
            if far {
                if d.x == 0.0 {
                    d.x = jiggle();
                    l += d.x * d.x;
                }
                if d.y == 0.0 {
                    d.y = jiggle();
                    l += d.y * d.y;
                }
                if l < dist_min2 {
                    l = (dist_min2 * l).sqrt();
                }
                dv += d * (k * n.count as f64 / l);
                continue;
            }
            if !n.is_leaf() {
                for &c in n.children.iter().rev() {
                    if c != EMPTY {
                        stack.push(c);
                    }
                }
                continue;
            }
            let mut j = n.point;
            while j != EMPTY {
                if j as usize != i {
                    let mut d = p[j as usize] - me;
                    let mut l = d.len2();
                    if d.x == 0.0 {
                        d.x = jiggle();
                        l += d.x * d.x;
                    }
                    if d.y == 0.0 {
                        d.y = jiggle();
                        l += d.y * d.y;
                    }
                    if l < dist_min2 {
                        l = (dist_min2 * l).sqrt();
                    }
                    dv += d * (k / l);
                }
                j = self.next_point[j as usize];
            }
        }
        dv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(n: usize) -> Vec<(usize, usize, f64)> {
        (0..n).map(|i| (i, (i + 1) % n, 1.0)).collect()
    }

    #[test]
    fn deterministic_and_finite() {
        let links = ring(20);
        let a = force(20, &links, &ForceOptions::default());
        let b = force(20, &links, &ForceOptions::default());
        assert_eq!(a, b);
        assert!(a.iter().all(|p| p.is_finite()));
        let c = force(20, &links, &ForceOptions { seed: 2, ..Default::default() });
        assert_ne!(a, c, "the seed matters");
    }

    #[test]
    fn links_settle_near_their_distance_and_centre_holds() {
        let links = ring(12);
        let opts = ForceOptions { link_distance: 40.0, center: Vec2::new(100.0, 50.0), ..Default::default() };
        let p = force(12, &links, &opts);
        let mean = p.iter().fold(Vec2::ZERO, |s, q| s + *q) / 12.0;
        assert!(mean.dist(opts.center) < 1.0, "centred: {mean:?}");
        for &(a, b, _) in &links {
            let d = p[a].dist(p[b]);
            assert!(d > 25.0 && d < 70.0, "link {a}-{b} length {d}");
        }
    }

    #[test]
    fn barnes_hut_close_to_exact() {
        let links: Vec<(usize, usize, f64)> = (1..80).map(|i| (i, i / 3, 1.0)).collect();
        let exact = force(80, &links, &ForceOptions { theta: 0.0, ..Default::default() });
        let bh = force(80, &links, &ForceOptions::default());
        let spread = |p: &[Vec2]| p.iter().map(|q| q.len()).sum::<f64>() / p.len() as f64;
        let (a, b) = (spread(&exact), spread(&bh));
        assert!((a - b).abs() / a < 0.1, "similar spread {a} vs {b}");
    }

    #[test]
    fn collision_separates_nodes() {
        let opts = ForceOptions { charge: 0.0, collide_radius: 5.0, gravity: 0.1, ..Default::default() };
        let p = force(60, &[], &opts);
        let mut min = f64::INFINITY;
        for i in 0..60 {
            for j in i + 1..60 {
                min = min.min(p[i].dist(p[j]));
            }
        }
        assert!(min > 8.0, "nearly non-overlapping: {min}");
    }

    #[test]
    fn sized_nodes_stay_apart_and_inside_the_box() {
        // Two hubs of different sizes with spokes, laid out in a box: no two circles overlap
        // (beyond a hair) and every circle stays inside.
        let n = 40;
        let links: Vec<(usize, usize, f64)> = (2..n).map(|i| (i, i % 2, 1.0)).chain([(0, 1, 1.0)]).collect();
        let radii: Vec<f64> = (0..n).map(|i| if i < 2 { 14.0 } else { 3.0 + (i % 5) as f64 }).collect();
        let b = Rect::new(0.0, 0.0, 400.0, 300.0);
        let init: Vec<Vec2> = (0..n).map(|i| Vec2::new(200.0 + (i as f64 * 7.3) % 50.0, 150.0 + (i as f64 * 3.1) % 40.0)).collect();
        let opts = ForceOptions { charge: -60.0, link_distance: 40.0, center: Vec2::new(200.0, 150.0), gravity: 0.05, ..Default::default() };
        let p = force_within(&init, &links, &radii, Some(b), &opts);
        assert_eq!(p, force_within(&init, &links, &radii, Some(b), &opts), "deterministic");
        for i in 0..n {
            assert!(p[i].x - radii[i] >= -1e-9 && p[i].x + radii[i] <= 400.0 + 1e-9, "node {i} inside across");
            assert!(p[i].y - radii[i] >= -1e-9 && p[i].y + radii[i] <= 300.0 + 1e-9, "node {i} inside down");
            for j in i + 1..n {
                let gap = p[i].dist(p[j]) - radii[i] - radii[j];
                assert!(gap > -0.5, "nodes {i} and {j} overlap by {}", -gap);
            }
        }
        // A tiny box: everything centred, still finite.
        let tight = force_within(&init, &links, &radii, Some(Rect::new(0.0, 0.0, 4.0, 4.0)), &opts);
        assert!(tight.iter().all(|q| q.x == 2.0 && q.y == 2.0));
        // Without bounds and with missing radii it behaves (radii default to 0 plus the padding).
        let free = force_within(&init, &links, &[f64::NAN], None, &ForceOptions { collide_radius: 2.0, ..opts });
        assert!(free.iter().all(|q| q.is_finite()));
    }

    #[test]
    fn degenerate() {
        assert!(force(0, &[], &ForceOptions::default()).is_empty());
        let one = force(1, &[(0, 0, 1.0), (0, 5, 1.0)], &ForceOptions::default());
        assert!(one[0].dist(Vec2::ZERO) < 1e-9);
        let same = force_from(&[Vec2::ZERO; 5], &[], &ForceOptions::default());
        assert!(same.iter().all(|p| p.is_finite()));
        let spread: f64 = same.iter().map(|p| p.len()).sum();
        assert!(spread > 1.0, "coincident nodes pushed apart");
        let nan = force_from(&[Vec2::new(f64::NAN, 0.0), Vec2::ZERO], &[(0, 1, f64::NAN)], &ForceOptions::default());
        assert!(nan.iter().all(|p| p.is_finite()));
        let none = force(5, &ring(5), &ForceOptions { iterations: 0, ..Default::default() });
        assert!(none.iter().all(|p| p.is_finite()));
    }
}
