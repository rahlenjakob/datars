//! Land from the OpenStreetMap coastline, for one box — the answer to "where is the sea" at street
//! zoom without the 1.3 GB planet shapefile: the coastline ways near the cameras come from the same
//! Overpass cells as the streets (a few hundred KB per city), and this turns them into polygons.
//!
//! OSM tags every coastline way with the land on its left. Joined end to end into chains and cut
//! to the box, each chain crossing the box is a piece from an entry on the box's edge to an exit;
//! a land polygon runs along a piece, then counter-clockwise along the edge to the next entry, and
//! so on until it closes (the method of `osmcoastline`, per box). Rings wholly inside the box are
//! islands (counter-clockwise) or enclosed water (clockwise), added even-odd. When no chain crosses
//! the box, its edge is all land or all sea: the side of the nearest coastline says which, and
//! where there's no coastline near enough to trust, the caller's coarse test (Natural Earth) does.

use datars_geo::{GeoBbox, Polygon};
use datars_math::Vec2;
use std::collections::BTreeMap;

/// A point as an exact key (OSM coordinates have 7 decimals; shared nodes are bit-identical).
fn pkey(p: Vec2) -> (i64, i64) {
    ((p.x * 1e7).round() as i64, (p.y * 1e7).round() as i64)
}

/// Ways joined end to start into chains (closed ones start and end on the same point). The
/// coastline is directed, so ways join only head to tail.
pub fn chains(ways: &[Vec<Vec2>]) -> Vec<Vec<Vec2>> {
    let mut by_start: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    let mut by_end: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, w) in ways.iter().enumerate().filter(|(_, w)| w.len() >= 2) {
        by_start.entry(pkey(w[0])).or_default().push(i);
        by_end.entry(pkey(w[w.len() - 1])).or_default().push(i);
    }
    let mut used = vec![false; ways.len()];
    let take = |m: &BTreeMap<(i64, i64), Vec<usize>>, k: (i64, i64), used: &[bool]| m.get(&k).and_then(|v| v.iter().copied().find(|&j| !used[j]));
    let mut out = Vec::new();
    for i in 0..ways.len() {
        if used[i] || ways[i].len() < 2 {
            continue;
        }
        used[i] = true;
        let mut c = ways[i].clone();
        while pkey(c[0]) != pkey(c[c.len() - 1]) {
            let Some(j) = take(&by_start, pkey(c[c.len() - 1]), &used) else { break };
            used[j] = true;
            c.extend_from_slice(&ways[j][1..]);
        }
        while pkey(c[0]) != pkey(c[c.len() - 1]) {
            let Some(j) = take(&by_end, pkey(c[0]), &used) else { break };
            used[j] = true;
            let mut head = ways[j].clone();
            head.extend_from_slice(&c[1..]);
            c = head;
        }
        out.push(c);
    }
    out
}

/// A piece of a chain inside the box: from an entry to an exit on its edge (perimeter positions,
/// counter-clockwise from the south-west corner, 0 ≤ t < 4).
#[derive(Clone, Debug)]
struct Piece {
    pts: Vec<Vec2>,
    t_in: f64,
    t_out: f64,
}

#[derive(Clone, Copy, Debug)]
struct Box2 {
    w: f64,
    s: f64,
    e: f64,
    n: f64,
}

impl Box2 {
    fn inside(&self, p: Vec2) -> bool {
        p.x > self.w && p.x < self.e && p.y > self.s && p.y < self.n
    }

    /// The perimeter position of a point on edge `edge` (0 left, 1 right, 2 bottom, 3 top).
    fn perim(&self, p: Vec2, edge: usize) -> f64 {
        let (fw, fh) = (self.e - self.w, self.n - self.s);
        match edge {
            2 => ((p.x - self.w) / fw).clamp(0.0, 1.0),
            1 => 1.0 + ((p.y - self.s) / fh).clamp(0.0, 1.0),
            3 => 2.0 + ((self.e - p.x) / fw).clamp(0.0, 1.0),
            _ => 3.0 + ((self.n - p.y) / fh).clamp(0.0, 1.0),
        }
        .rem_euclid(4.0)
    }

    fn corner(&self, k: i64) -> Vec2 {
        match k.rem_euclid(4) {
            0 => Vec2::new(self.w, self.s),
            1 => Vec2::new(self.e, self.s),
            2 => Vec2::new(self.e, self.n),
            _ => Vec2::new(self.w, self.n),
        }
    }

    /// Liang–Barsky: the part of segment a→b inside, as (u0, entry edge, u1, exit edge).
    fn clip(&self, a: Vec2, b: Vec2) -> Option<(f64, Option<usize>, f64, Option<usize>)> {
        let d = b - a;
        let p = [-d.x, d.x, -d.y, d.y];
        let q = [a.x - self.w, self.e - a.x, a.y - self.s, self.n - a.y];
        let (mut u0, mut u1, mut e0, mut e1) = (0.0f64, 1.0f64, None, None);
        for i in 0..4 {
            if p[i] == 0.0 {
                if q[i] < 0.0 {
                    return None;
                }
                continue;
            }
            let r = q[i] / p[i];
            if p[i] < 0.0 {
                if r > u1 {
                    return None;
                }
                if r > u0 {
                    u0 = r;
                    e0 = Some(i);
                }
            } else {
                if r < u0 {
                    return None;
                }
                if r < u1 {
                    u1 = r;
                    e1 = Some(i);
                }
            }
        }
        Some((u0, e0, u1, e1))
    }

    /// The point at `u` along a→b, snapped exactly onto `edge`.
    fn on_edge(&self, a: Vec2, b: Vec2, u: f64, edge: usize) -> Vec2 {
        let p = a + (b - a) * u;
        match edge {
            0 => Vec2::new(self.w, p.y.clamp(self.s, self.n)),
            1 => Vec2::new(self.e, p.y.clamp(self.s, self.n)),
            2 => Vec2::new(p.x.clamp(self.w, self.e), self.s),
            _ => Vec2::new(p.x.clamp(self.w, self.e), self.n),
        }
    }

    fn ring(&self) -> Vec<Vec2> {
        (0..=4).map(|k| self.corner(k)).collect()
    }
}

/// A chain cut to the box: its pieces (those that enter and leave; ends dangling inside the box —
/// broken data — are dropped).
fn cut(bx: &Box2, chain: &[Vec2], out: &mut Vec<Piece>) {
    let mut cur: Option<(Vec<Vec2>, Option<f64>)> = None;
    for w in chain.windows(2) {
        let (a, b) = (w[0], w[1]);
        let Some((u0, e0, u1, e1)) = bx.clip(a, b) else { continue };
        if let Some(e) = e0 {
            // Entering (a is outside): a fresh piece from the edge.
            let p = bx.on_edge(a, b, u0, e);
            cur = Some((vec![p], Some(bx.perim(p, e))));
        } else if cur.is_none() {
            cur = Some((vec![a], None)); // starts inside: dangling
        }
        let Some((pts, t_in)) = cur.as_mut() else { continue };
        match e1 {
            Some(e) => {
                let p = bx.on_edge(a, b, u1, e);
                pts.push(p);
                let (pts, t_in) = (std::mem::take(pts), *t_in);
                cur = None;
                if let Some(t_in) = t_in {
                    if pts.len() > 2 || pkey(pts[0]) != pkey(pts[pts.len() - 1]) {
                        out.push(Piece { pts, t_in, t_out: bx.perim(p, e) });
                    }
                }
            }
            None => pts.push(b),
        }
    }
}

/// Twice the signed area (counter-clockwise positive, y up).
fn area2(r: &[Vec2]) -> f64 {
    r.windows(2).map(|w| w[0].x * w[1].y - w[1].x * w[0].y).sum()
}

/// Whether `p` is on the land side of the nearest coastline, if a coastline within `trust` of it
/// exists (farther than that, a coastline outside what was fetched might be nearer).
fn side_of_nearest(chains: &[Vec<Vec2>], p: Vec2, trust: f64) -> Option<bool> {
    // (distance², chain, segment, parameter)
    let mut best: Option<(f64, usize, usize, f64)> = None;
    for (ci, c) in chains.iter().enumerate() {
        for (si, w) in c.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let d = b - a;
            let len2 = d.x * d.x + d.y * d.y;
            let t = if len2 > 0.0 { (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + d * t;
            let dist2 = (p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y);
            if best.is_none_or(|bst| dist2 < bst.0) {
                best = Some((dist2, ci, si, t));
            }
        }
    }
    let (d2, ci, si, t) = best?;
    if d2.sqrt() > trust {
        return None;
    }
    let c = &chains[ci];
    let left = |a: Vec2, b: Vec2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x) > 0.0;
    let seg = |i: usize| (c[i], c[i + 1]);
    let closed = pkey(c[0]) == pkey(c[c.len() - 1]);
    let n = c.len() - 1;
    // Nearest to a vertex: decide by both segments meeting there (convex: left of both; reflex:
    // left of either) — the side of just one of them can be wrong near a sharp corner.
    let pair = if t <= 0.0 && (si > 0 || closed) {
        Some((if si > 0 { si - 1 } else { n - 1 }, si))
    } else if t >= 1.0 && (si + 1 < n || closed) {
        Some((si, if si + 1 < n { si + 1 } else { 0 }))
    } else {
        None
    };
    Some(match pair {
        Some((i, j)) => {
            let ((u, v), (_, w)) = (seg(i), seg(j));
            let convex = (v.x - u.x) * (w.y - v.y) - (v.y - u.y) * (w.x - v.x) > 0.0;
            let (l1, l2) = (left(u, v), left(v, w));
            if convex { l1 && l2 } else { l1 || l2 }
        }
        None => {
            let (a, b) = seg(si);
            left(a, b)
        }
    })
}

/// Land polygons inside `bbox` from coastline `ways` (land on their left). `data` is the area the
/// ways were fetched for (it must hold `bbox`); `is_land` answers for a point when no coastline is
/// near enough to say.
pub fn land(bbox: GeoBbox, ways: &[Vec<Vec2>], data: GeoBbox, is_land: &dyn Fn(Vec2) -> bool) -> Vec<Polygon> {
    // Grown by an amount off the 1e-7° grid OSM nodes sit on: no node lands exactly on the edge.
    let eps = 1.234_567e-6;
    let bx = Box2 { w: bbox.west - eps, s: bbox.south - eps, e: bbox.east + eps, n: bbox.north + eps };
    let all = chains(ways);
    let mut pieces = Vec::new();
    let mut inner: Vec<Vec<Vec2>> = Vec::new();
    for c in &all {
        let closed = c.len() >= 4 && pkey(c[0]) == pkey(c[c.len() - 1]);
        if closed {
            match c.iter().position(|p| !bx.inside(*p)) {
                None => inner.push(c.clone()),
                Some(k) => {
                    // Start outside, so every piece enters and leaves.
                    let mut r: Vec<Vec2> = c[k..c.len() - 1].to_vec();
                    r.extend_from_slice(&c[..=k]);
                    cut(&bx, &r, &mut pieces);
                }
            }
        } else {
            cut(&bx, c, &mut pieces);
        }
    }
    let mut rings: Vec<Vec<Vec2>> = Vec::new();
    if pieces.is_empty() {
        let corner = Vec2::new(bx.w, bx.s);
        // How far the fetched data reaches beyond the corner: a coastline nearer than that is the
        // nearest there is.
        let trust = (corner.x - data.west).min(data.east - corner.x).min(corner.y - data.south).min(data.north - corner.y).max(0.0);
        let edge_is_land = side_of_nearest(&all, corner, trust).unwrap_or_else(|| is_land(Vec2::new(bbox.west + bbox.width() * 0.5, bbox.south + bbox.height() * 0.5)));
        if edge_is_land {
            rings.push(bx.ring());
        }
    } else {
        rings = walk(&bx, &pieces);
    }
    rings.extend(inner);
    even_odd(rings)
}

/// Land rings from pieces: along a piece, then counter-clockwise along the edge to the next entry.
fn walk(bx: &Box2, pieces: &[Piece]) -> Vec<Vec<Vec2>> {
    let n = pieces.len();
    let ccw = |a: f64, b: f64| (b - a).rem_euclid(4.0);
    let mut used = vec![false; n];
    let mut rings = Vec::new();
    for start in 0..n {
        if used[start] {
            continue;
        }
        let mut ring: Vec<Vec2> = Vec::new();
        let mut cur = start;
        for _ in 0..=n {
            used[cur] = true;
            ring.extend_from_slice(&pieces[cur].pts);
            let t = pieces[cur].t_out;
            let Some(next) = (0..n).filter(|&j| !used[j] || j == start).min_by(|&a, &b| ccw(t, pieces[a].t_in).total_cmp(&ccw(t, pieces[b].t_in))) else { break };
            // Corners passed on the way (at perimeter positions 1, 2, 3, 4 ≡ 0).
            let to = t + ccw(t, pieces[next].t_in);
            let mut k = t.floor() as i64 + 1;
            while (k as f64) < to {
                ring.push(bx.corner(k));
                k += 1;
            }
            if next == start {
                ring.push(ring[0]);
                if ring.len() >= 4 {
                    rings.push(ring);
                }
                break;
            }
            cur = next;
        }
    }
    rings
}

/// Rings combined even-odd (so islands in the sea add land and water inside land cuts it out).
fn even_odd(rings: Vec<Vec<Vec2>>) -> Vec<Polygon> {
    use geo::BooleanOps;
    let to_geo = |r: &Vec<Vec2>| geo::MultiPolygon(vec![geo::Polygon::new(geo::LineString::from(r.iter().map(|p| geo::Coord { x: p.x, y: p.y }).collect::<Vec<_>>()), vec![])]);
    let mut rings: Vec<Vec<Vec2>> = rings.into_iter().filter(|r| r.len() >= 4 && area2(r).abs() > 0.0).collect();
    if rings.is_empty() {
        return Vec::new();
    }
    let mut acc = to_geo(&rings.remove(0));
    for r in &rings {
        acc = acc.xor(&to_geo(r));
    }
    let ring = |ls: &geo::LineString<f64>| ls.coords().map(|c| Vec2::new(c.x, c.y)).collect::<Vec<_>>();
    acc.0.iter().map(|p| std::iter::once(ring(p.exterior())).chain(p.interiors().iter().map(ring)).collect()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(pts: &[(f64, f64)]) -> Vec<Vec2> {
        pts.iter().map(|&(x, y)| Vec2::new(x, y)).collect()
    }

    fn area(polys: &[Polygon]) -> f64 {
        polys.iter().map(|p| p.iter().enumerate().map(|(i, r)| if i == 0 { area2(r).abs() } else { -area2(r).abs() }).sum::<f64>() / 2.0).sum()
    }

    const UNIT: GeoBbox = GeoBbox { west: 0.0, south: 0.0, east: 1.0, north: 1.0 };
    const DATA: GeoBbox = GeoBbox { west: -3.0, south: -3.0, east: 4.0, north: 4.0 };
    fn never(_: Vec2) -> bool {
        panic!("the coastline decides here")
    }

    #[test]
    fn a_coast_across_the_box_keeps_the_land_on_its_left() {
        // Heading south along x = 0.5: land to the east.
        let south = v(&[(0.5, 2.0), (0.5, -1.0)]);
        let a = area(&land(UNIT, &[south.clone()], DATA, &never));
        assert!((a - 0.5).abs() < 1e-4, "{a}");
        // The same line heading north: land to the west (still half).
        let mut north = south;
        north.reverse();
        let l = land(UNIT, &[north], DATA, &never);
        let b = datars_geo::measure::bbox(&datars_geo::Geometry::Polygon(l[0].clone())).unwrap();
        assert!(b.east < 0.51 && b.west < 0.0, "{b:?}");
    }

    #[test]
    fn a_peninsula_and_a_bay_through_one_edge() {
        // A U from the east: heading west, south, east — land inside the U (a peninsula).
        let u = v(&[(1.5, 0.8), (0.3, 0.8), (0.3, 0.2), (1.5, 0.2)]);
        assert!((area(&land(UNIT, &[u.clone()], DATA, &never)) - 0.42).abs() < 1e-4);
        // Reversed: the U is water (a bay), the rest of the box land — the walk passes 4 corners.
        let mut bay = u;
        bay.reverse();
        assert!((area(&land(UNIT, &[bay], DATA, &never)) - 0.58).abs() < 1e-4);
    }

    #[test]
    fn ways_join_into_chains_even_when_they_meet_inside_the_box() {
        // The south-running coast in two ways meeting at (0.5, 0.5), given out of order.
        let (a, b) = (v(&[(0.5, 0.5), (0.5, -1.0)]), v(&[(0.5, 2.0), (0.5, 0.5)]));
        assert_eq!(chains(&[a.clone(), b.clone()]).len(), 1);
        assert!((area(&land(UNIT, &[a, b], DATA, &never)) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn islands_and_open_sea_are_decided_by_the_nearest_coast() {
        // A counter-clockwise island inside the box: the edge is sea, the island is land.
        let island = v(&[(0.4, 0.4), (0.6, 0.4), (0.6, 0.6), (0.4, 0.6), (0.4, 0.4)]);
        assert!((area(&land(UNIT, &[island.clone()], DATA, &never)) - 0.04).abs() < 1e-6);
        // A clockwise ring is water inside land: the box less the ring.
        let mut lake = island.clone();
        lake.reverse();
        assert!((area(&land(UNIT, &[lake], DATA, &never)) - 0.96).abs() < 1e-4, "the box grown by its epsilon");
        // A box in the sea off a big island's coast (an island ring well outside the box).
        let big = v(&[(2.0, -2.0), (3.5, -2.0), (3.5, 3.0), (2.0, 3.0), (2.0, -2.0)]);
        assert!(land(UNIT, &[big.clone()], DATA, &never).is_empty());
        // A box inside a big island.
        let around = v(&[(-2.0, -2.0), (3.0, -2.0), (3.0, 3.0), (-2.0, 3.0), (-2.0, -2.0)]);
        assert!((area(&land(UNIT, &[around], DATA, &never)) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn without_a_coastline_near_enough_the_fallback_decides() {
        assert!((area(&land(UNIT, &[], DATA, &|_| true)) - 1.0).abs() < 1e-4);
        assert!(land(UNIT, &[], DATA, &|_| false).is_empty());
        // A coastline far outside what was fetched isn't trusted.
        let far = v(&[(3.9, -2.0), (3.9, 3.0)]);
        let tight = GeoBbox::new(-0.5, -0.5, 1.5, 1.5);
        assert!(land(UNIT, &[far], tight, &|_| false).is_empty());
    }

    #[test]
    fn several_crossings_make_separate_land_polygons() {
        // Two headlands from the south edge (each a north-poking U, land inside heading east–north–west… reversed).
        let a = v(&[(0.1, -0.5), (0.1, 0.5), (0.3, 0.5), (0.3, -0.5)]);
        let b = v(&[(0.6, -0.5), (0.6, 0.5), (0.8, 0.5), (0.8, -0.5)]);
        // Heading north then east then south: land on the left is west/north/east — outside the U,
        // so the land is the box less two inlets.
        let l = land(UNIT, &[a, b], DATA, &never);
        assert!((area(&l) - (1.0 - 0.1 - 0.1)).abs() < 1e-4, "{}", area(&l));
    }
}
