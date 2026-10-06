//! Delaunay triangulation (a port of Delaunator's sweep-hull algorithm on exact predicates) and
//! Voronoi cells clipped to a rectangle.

use crate::predicates::{incircle, orient2d};
use crate::util::rect_area_ok;
use datars_math::{total_cmp, Rect, Vec2};
use serde::{Deserialize, Serialize};

const NONE: usize = usize::MAX;

/// A Delaunay triangulation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Delaunay {
    /// Triangles as point indices, each wound with positive `signed_area` (clockwise on a y-down
    /// screen, the datars convention).
    pub triangles: Vec<[usize; 3]>,
    /// For half-edge `e = 3·t + k` (from `triangles[t][k]` to `triangles[t][(k + 1) % 3]`), the
    /// opposite half-edge in the neighbouring triangle, or `None` on the convex hull.
    pub halfedges: Vec<Option<usize>>,
    /// Convex hull as point indices, in the same winding as the triangles. For collinear input
    /// (no triangles) it's the distinct points in order along the line.
    pub hull: Vec<usize>,
}

impl Delaunay {
    /// The Delaunay neighbours of every point (sorted, no duplicates), indexed like the input;
    /// empty for points that were skipped (non-finite or duplicates).
    pub fn neighbors(&self, n: usize) -> Vec<Vec<usize>> {
        let mut adj = vec![Vec::new(); n];
        for t in &self.triangles {
            for k in 0..3 {
                let (a, b) = (t[k], t[(k + 1) % 3]);
                adj[a].push(b);
                adj[b].push(a);
            }
        }
        if self.triangles.is_empty() {
            for w in self.hull.windows(2) {
                adj[w[0]].push(w[1]);
                adj[w[1]].push(w[0]);
            }
        }
        for l in adj.iter_mut() {
            l.sort_unstable();
            l.dedup();
        }
        adj
    }
}

/// Delaunay triangulation of `points`: no point lies strictly inside any triangle's circumcircle.
///
/// Delaunator's O(n log n) sweep: seed triangle near the centre, points added in order of distance
/// from it, each connected to the visible hull edges, then edges flipped until locally Delaunay.
/// Robust: orientation and in-circle tests use exact predicates, so grids, collinear runs and
/// cocircular sets triangulate correctly (among cocircular points any valid triangulation may be
/// chosen). Deterministic: sorts break ties by coordinates then index; no hashing.
///
/// Non-finite points and exact duplicates are skipped (they appear in no triangle). Fewer than
/// three distinct points, or all collinear, give no triangles and a `hull` of the distinct points in
/// order along the line.
pub fn delaunay(points: &[Vec2]) -> Delaunay {
    let n = points.len();
    // Delaunator's conventions on mirrored coordinates (x, −y): its triangles come out
    // counter-clockwise in its frame, which is clockwise on our y-down screen.
    let coords: Vec<(f64, f64)> = points.iter().map(|p| (p.x, -p.y)).collect();
    let ids: Vec<usize> = (0..n).filter(|&i| points[i].is_finite()).collect();
    let mut out = Delaunay::default();
    if ids.is_empty() {
        return out;
    }
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &i in &ids {
        let (x, y) = coords[i];
        minx = minx.min(x);
        miny = miny.min(y);
        maxx = maxx.max(x);
        maxy = maxy.max(y);
    }
    let (cx, cy) = ((minx + maxx) / 2.0, (miny + maxy) / 2.0);
    let dist = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (a.0 - b.0, a.1 - b.1);
        dx * dx + dy * dy
    };
    // Seed: the point nearest the centre, its nearest neighbour, and the point making the smallest
    // circumcircle with them (ties by index — the loops keep the first minimum).
    let mut i0 = ids[0];
    let mut best = f64::INFINITY;
    for &i in &ids {
        let d = dist((cx, cy), coords[i]);
        if d < best {
            i0 = i;
            best = d;
        }
    }
    let p0 = coords[i0];
    let mut i1 = NONE;
    best = f64::INFINITY;
    for &i in &ids {
        let d = dist(p0, coords[i]);
        if i != i0 && d > 0.0 && d < best {
            i1 = i;
            best = d;
        }
    }
    if i1 == NONE {
        out.hull = vec![i0];
        return out;
    }
    let p1 = coords[i1];
    let mut i2 = NONE;
    let mut min_r = f64::INFINITY;
    for &i in &ids {
        if i == i0 || i == i1 {
            continue;
        }
        let r = circumradius(p0, p1, coords[i]);
        if r < min_r {
            i2 = i;
            min_r = r;
        }
    }
    if i2 == NONE || !min_r.is_finite() {
        out.hull = collinear_hull(&ids, &coords);
        return out;
    }
    let (mut i1, mut i2) = (i1, i2);
    if orient2d(p0.0, p0.1, coords[i1].0, coords[i1].1, coords[i2].0, coords[i2].1) < 0.0 {
        std::mem::swap(&mut i1, &mut i2);
    }
    let (p1, p2) = (coords[i1], coords[i2]);
    let center = circumcenter(p0, p1, p2);

    // Sort by distance from the seed circumcentre; ties by coordinates (so duplicates are adjacent)
    // then index.
    let mut order = ids.clone();
    let dists: Vec<f64> = coords.iter().map(|&p| dist(p, center)).collect();
    order.sort_by(|&a, &b| {
        total_cmp(dists[a], dists[b])
            .then(total_cmp(coords[a].0, coords[b].0))
            .then(total_cmp(coords[a].1, coords[b].1))
            .then(a.cmp(&b))
    });

    let hash_size = ((ids.len() as f64).sqrt().ceil() as usize).max(1);
    let mut b = Builder {
        coords: &coords,
        triangles: Vec::with_capacity(ids.len() * 6),
        halfedges: Vec::with_capacity(ids.len() * 6),
        hull_prev: vec![0; n],
        hull_next: vec![0; n],
        hull_tri: vec![0; n],
        hull_hash: vec![NONE; hash_size],
        hull_start: i0,
        center,
        edge_stack: Vec::new(),
    };
    b.hull_next[i0] = i1;
    b.hull_prev[i2] = i1;
    b.hull_next[i1] = i2;
    b.hull_prev[i0] = i2;
    b.hull_next[i2] = i0;
    b.hull_prev[i1] = i0;
    b.hull_tri[i0] = 0;
    b.hull_tri[i1] = 1;
    b.hull_tri[i2] = 2;
    for &i in &[i0, i1, i2] {
        let k = b.hash_key(coords[i]);
        b.hull_hash[k] = i;
    }
    b.add_triangle(i0, i1, i2, NONE, NONE, NONE);
    let mut hull_size = 3usize;
    let mut prev: Option<(f64, f64)> = None;
    for &i in &order {
        let (x, y) = coords[i];
        // Skip duplicates (and near-duplicates, as Delaunator does).
        if let Some((xp, yp)) = prev {
            if (x - xp).abs() <= f64::EPSILON && (y - yp).abs() <= f64::EPSILON {
                continue;
            }
        }
        prev = Some((x, y));
        if i == i0 || i == i1 || i == i2 {
            continue;
        }
        // Find a visible hull edge, starting from the angular hash.
        let key = b.hash_key((x, y));
        let mut start = b.hull_start;
        for j in 0..hash_size {
            let s = b.hull_hash[(key + j) % hash_size];
            if s != NONE && s != b.hull_next[s] {
                start = s;
                break;
            }
        }
        start = b.hull_prev[start];
        let mut e = start;
        loop {
            let q = b.hull_next[e];
            if orient2d(x, y, coords[e].0, coords[e].1, coords[q].0, coords[q].1) < 0.0 {
                break;
            }
            e = q;
            if e == start {
                e = NONE;
                break;
            }
        }
        if e == NONE {
            continue; // on the hull already (a duplicate of a hull point, in effect)
        }
        // The first triangle from the new point, then flips.
        let t = b.add_triangle(e, i, b.hull_next[e], NONE, NONE, b.hull_tri[e]);
        b.hull_tri[i] = b.legalize(t + 2);
        b.hull_tri[e] = t;
        hull_size += 1;
        // Walk forward along the hull adding triangles.
        let mut nn = b.hull_next[e];
        loop {
            let q = b.hull_next[nn];
            if orient2d(x, y, coords[nn].0, coords[nn].1, coords[q].0, coords[q].1) >= 0.0 {
                break;
            }
            let t = b.add_triangle(nn, i, q, b.hull_tri[i], NONE, b.hull_tri[nn]);
            b.hull_tri[i] = b.legalize(t + 2);
            b.hull_next[nn] = nn; // removed from the hull
            hull_size -= 1;
            nn = q;
        }
        // And backward.
        if e == start {
            loop {
                let q = b.hull_prev[e];
                if orient2d(x, y, coords[q].0, coords[q].1, coords[e].0, coords[e].1) >= 0.0 {
                    break;
                }
                let t = b.add_triangle(q, i, e, NONE, b.hull_tri[e], b.hull_tri[q]);
                b.legalize(t + 2);
                b.hull_tri[q] = t;
                b.hull_next[e] = e;
                hull_size -= 1;
                e = q;
            }
        }
        b.hull_start = e;
        b.hull_prev[i] = e;
        b.hull_next[e] = i;
        b.hull_prev[nn] = i;
        b.hull_next[i] = nn;
        let k = b.hash_key((x, y));
        b.hull_hash[k] = i;
        let k = b.hash_key(coords[e]);
        b.hull_hash[k] = e;
    }
    let mut hull = Vec::with_capacity(hull_size);
    let mut e = b.hull_start;
    for _ in 0..hull_size {
        hull.push(e);
        e = b.hull_next[e];
    }
    out.triangles = b.triangles.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
    out.halfedges = b.halfedges.iter().map(|&h| (h != NONE).then_some(h)).collect();
    out.hull = hull;
    out
}

/// Distinct collinear points in order along their line.
fn collinear_hull(ids: &[usize], coords: &[(f64, f64)]) -> Vec<usize> {
    let (x0, y0) = coords[ids[0]];
    let key = |i: usize| {
        let dx = coords[i].0 - x0;
        if dx != 0.0 {
            dx
        } else {
            coords[i].1 - y0
        }
    };
    let mut order = ids.to_vec();
    order.sort_by(|&a, &b| total_cmp(key(a), key(b)).then(a.cmp(&b)));
    let mut hull: Vec<usize> = Vec::with_capacity(order.len());
    for i in order {
        if hull.last().is_none_or(|&h| coords[h] != coords[i]) {
            hull.push(i);
        }
    }
    hull
}

fn circumradius(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (ex, ey) = (c.0 - a.0, c.1 - a.1);
    let bl = dx * dx + dy * dy;
    let cl = ex * ex + ey * ey;
    let d = 0.5 / (dx * ey - dy * ex);
    let x = (ey * bl - dy * cl) * d;
    let y = (dx * cl - ex * bl) * d;
    let r = x * x + y * y;
    if r.is_nan() {
        f64::INFINITY
    } else {
        r
    }
}

fn circumcenter(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (ex, ey) = (c.0 - a.0, c.1 - a.1);
    let bl = dx * dx + dy * dy;
    let cl = ex * ex + ey * ey;
    let d = 0.5 / (dx * ey - dy * ex);
    (a.0 + (ey * bl - dy * cl) * d, a.1 + (dx * cl - ex * bl) * d)
}

struct Builder<'a> {
    coords: &'a [(f64, f64)],
    triangles: Vec<usize>,
    halfedges: Vec<usize>,
    hull_prev: Vec<usize>,
    hull_next: Vec<usize>,
    hull_tri: Vec<usize>,
    hull_hash: Vec<usize>,
    hull_start: usize,
    center: (f64, f64),
    edge_stack: Vec<usize>,
}

impl Builder<'_> {
    fn hash_key(&self, p: (f64, f64)) -> usize {
        let (dx, dy) = (p.0 - self.center.0, p.1 - self.center.1);
        let s = dx.abs() + dy.abs();
        let pa = if s > 0.0 {
            let q = dx / s;
            (if dy > 0.0 { 3.0 - q } else { 1.0 + q }) / 4.0
        } else {
            0.0
        };
        let n = self.hull_hash.len();
        ((pa * n as f64).floor().max(0.0) as usize) % n
    }

    fn link(&mut self, a: usize, b: usize) {
        self.halfedges[a] = b;
        if b != NONE {
            self.halfedges[b] = a;
        }
    }

    fn add_triangle(&mut self, i0: usize, i1: usize, i2: usize, a: usize, b: usize, c: usize) -> usize {
        let t = self.triangles.len();
        self.triangles.extend_from_slice(&[i0, i1, i2]);
        self.halfedges.extend_from_slice(&[NONE, NONE, NONE]);
        self.link(t, a);
        self.link(t + 1, b);
        self.link(t + 2, c);
        t
    }

    /// Flips edges from half-edge `a` until every affected pair is locally Delaunay.
    fn legalize(&mut self, mut a: usize) -> usize {
        let mut ar;
        loop {
            let b = self.halfedges[a];
            let a0 = a - a % 3;
            ar = a0 + (a + 2) % 3;
            if b == NONE {
                match self.edge_stack.pop() {
                    Some(e) => {
                        a = e;
                        continue;
                    }
                    None => break,
                }
            }
            let b0 = b - b % 3;
            let al = a0 + (a + 1) % 3;
            let bl = b0 + (b + 2) % 3;
            let p0 = self.triangles[ar];
            let pr = self.triangles[a];
            let pl = self.triangles[al];
            let p1 = self.triangles[bl];
            let c = self.coords;
            let illegal = incircle(c[p0].0, c[p0].1, c[pr].0, c[pr].1, c[pl].0, c[pl].1, c[p1].0, c[p1].1) < 0.0;
            if illegal {
                self.triangles[a] = p1;
                self.triangles[b] = p0;
                let hbl = self.halfedges[bl];
                // The flipped edge was on the hull: fix the hull's triangle reference.
                if hbl == NONE {
                    let mut e = self.hull_start;
                    loop {
                        if self.hull_tri[e] == bl {
                            self.hull_tri[e] = a;
                            break;
                        }
                        e = self.hull_prev[e];
                        if e == self.hull_start {
                            break;
                        }
                    }
                }
                self.link(a, hbl);
                let har = self.halfedges[ar];
                self.link(b, har);
                self.link(ar, bl);
                let br = b0 + (b + 1) % 3;
                self.edge_stack.push(br);
            } else {
                match self.edge_stack.pop() {
                    Some(e) => a = e,
                    None => break,
                }
            }
        }
        ar
    }
}

/// Voronoi cells of `points`, clipped to `bounds`, indexed like the input.
///
/// Each cell is the part of `bounds` closer to its point than to any other, as a convex polygon
/// (implicitly closed, positive `signed_area` — clockwise on a y-down screen). Built by clipping
/// `bounds` with the perpendicular bisectors toward the point's Delaunay neighbours, which is exact
/// for hull points too (no infinite rays to handle). Points outside `bounds` may get empty cells;
/// non-finite and duplicate points get empty cells (a duplicate's twin keeps the cell). O(n log n).
pub fn voronoi(points: &[Vec2], bounds: Rect) -> Vec<Vec<Vec2>> {
    let n = points.len();
    let mut cells = vec![Vec::new(); n];
    if !rect_area_ok(&bounds) {
        return cells;
    }
    let box_poly = vec![
        Vec2::new(bounds.x, bounds.y),
        Vec2::new(bounds.x1(), bounds.y),
        Vec2::new(bounds.x1(), bounds.y1()),
        Vec2::new(bounds.x, bounds.y1()),
    ];
    let d = delaunay(points);
    let adj = d.neighbors(n);
    let mut in_tri = vec![false; n];
    for t in &d.triangles {
        for &v in t {
            in_tri[v] = true;
        }
    }
    for &h in &d.hull {
        in_tri[h] = true;
    }
    for i in 0..n {
        if !in_tri[i] {
            continue;
        }
        let p = points[i];
        let mut poly = box_poly.clone();
        for &j in &adj[i] {
            let q = points[j];
            // Keep the half-plane closer to p: (x − mid) · (q − p) ≤ 0.
            let dir = q - p;
            let mid = (p + q) / 2.0;
            poly = clip_half_plane(&poly, dir, dir.dot(mid));
            if poly.is_empty() {
                break;
            }
        }
        cells[i] = poly;
    }
    cells
}

/// Sutherland–Hodgman against the half-plane `x · dir ≤ c`.
fn clip_half_plane(poly: &[Vec2], dir: Vec2, c: f64) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(poly.len() + 1);
    let k = poly.len();
    for idx in 0..k {
        let a = poly[idx];
        let b = poly[(idx + 1) % k];
        let (da, db) = (a.dot(dir) - c, b.dot(dir) - c);
        if da <= 0.0 {
            out.push(a);
        }
        if (da < 0.0 && db > 0.0) || (da > 0.0 && db < 0.0) {
            let t = da / (da - db);
            out.push(a.lerp(b, t));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::path::signed_area;
    use datars_math::Rng;

    /// Exact empty-circumcircle check against every point (O(n·t), fine for tests).
    fn assert_delaunay(points: &[Vec2], d: &Delaunay) {
        for (ti, t) in d.triangles.iter().enumerate() {
            let [a, b, c] = t.map(|i| points[i]);
            assert!(signed_area(&[a, b, c]) > 0.0, "triangle {ti} winding");
            for (pi, p) in points.iter().enumerate() {
                if t.contains(&pi) || !p.is_finite() {
                    continue;
                }
                // Mirrored frame, as used internally.
                let inside = incircle(a.x, -a.y, b.x, -b.y, c.x, -c.y, p.x, -p.y) < 0.0;
                assert!(!inside, "point {pi} inside circumcircle of triangle {ti}");
            }
        }
    }

    fn assert_halfedges(d: &Delaunay) {
        let tri = |e: usize| d.triangles[e / 3][e % 3];
        for (e, h) in d.halfedges.iter().enumerate() {
            if let Some(h) = *h {
                assert_eq!(d.halfedges[h], Some(e));
                assert_eq!(tri(e), tri(3 * (h / 3) + (h % 3 + 1) % 3), "opposite half-edges share endpoints");
            }
        }
    }

    fn random_points(n: usize, seed: u64) -> Vec<Vec2> {
        let mut rng = Rng::new(seed);
        (0..n).map(|_| Vec2::new(rng.range(0.0, 1000.0), rng.range(0.0, 1000.0))).collect()
    }

    #[test]
    fn random_points_satisfy_empty_circumcircle() {
        for seed in [1, 2, 3] {
            let pts = random_points(400, seed);
            let d = delaunay(&pts);
            // Euler: t = 2n − 2 − h for points in general position.
            assert_eq!(d.triangles.len(), 2 * pts.len() - 2 - d.hull.len());
            assert_delaunay(&pts, &d);
            assert_halfedges(&d);
            assert_eq!(delaunay(&pts), d, "deterministic");
        }
    }

    #[test]
    fn grid_is_robust() {
        let pts: Vec<Vec2> = (0..20).flat_map(|y| (0..20).map(move |x| Vec2::new(x as f64, y as f64))).collect();
        let d = delaunay(&pts);
        assert_eq!(d.triangles.len(), 2 * 19 * 19, "every cell split in two");
        assert_delaunay(&pts, &d);
        assert_halfedges(&d);
        let area: f64 = d.triangles.iter().map(|t| signed_area(&t.map(|i| pts[i]))).sum();
        assert!((area - 361.0).abs() < 1e-9);
        assert_eq!(d.hull.len(), 76);
        let hull_pts: Vec<Vec2> = d.hull.iter().map(|&i| pts[i]).collect();
        assert!(signed_area(&hull_pts) > 0.0, "hull wound like the triangles");
    }

    #[test]
    fn degenerate_inputs() {
        assert!(delaunay(&[]).triangles.is_empty());
        assert_eq!(delaunay(&[Vec2::new(1.0, 1.0)]).hull, vec![0]);
        let same = delaunay(&[Vec2::new(1.0, 1.0); 5]);
        assert!(same.triangles.is_empty() && same.hull.len() == 1);
        let line: Vec<Vec2> = [3.0, 1.0, 2.0, 1.0, 0.0].iter().map(|&t| Vec2::new(t, 2.0 * t)).collect();
        let l = delaunay(&line);
        assert!(l.triangles.is_empty());
        assert_eq!(l.hull, vec![4, 1, 2, 0], "distinct points along the line");
        // Duplicates and NaNs inside a normal set.
        let mut pts = random_points(50, 9);
        pts.push(pts[3]);
        pts.push(Vec2::new(f64::NAN, 0.0));
        let d = delaunay(&pts);
        assert!(d.triangles.iter().all(|t| !t.contains(&51)));
        assert_delaunay(&pts[..50], &Delaunay { triangles: d.triangles.iter().filter(|t| t.iter().all(|&i| i < 50)).copied().collect(), ..Default::default() });
        let tri = delaunay(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)]);
        assert_eq!(tri.triangles.len(), 1);
    }

    #[test]
    fn voronoi_cells_partition_bounds() {
        let pts = random_points(300, 5);
        let b = Rect::new(0.0, 0.0, 1000.0, 1000.0);
        let cells = voronoi(&pts, b);
        let area: f64 = cells.iter().map(|c| signed_area(c)).sum();
        assert!((area - 1e6).abs() < 1e-3, "cells tile the bounds: {area}");
        for (i, c) in cells.iter().enumerate() {
            assert!(signed_area(c) > 0.0, "cell {i} non-empty, clockwise on screen");
            // Every cell vertex is at least as close to its own point as to any other.
            for v in c {
                let own = v.dist(pts[i]);
                for (j, q) in pts.iter().enumerate() {
                    assert!(own <= v.dist(*q) + 1e-6, "cell {i} vertex closer to {j}");
                }
            }
        }
    }

    #[test]
    fn voronoi_small_cases() {
        let b = Rect::new(0.0, 0.0, 10.0, 10.0);
        let one = voronoi(&[Vec2::new(5.0, 5.0)], b);
        assert!((signed_area(&one[0]) - 100.0).abs() < 1e-9);
        let two = voronoi(&[Vec2::new(2.0, 5.0), Vec2::new(8.0, 5.0)], b);
        assert!((signed_area(&two[0]) - 50.0).abs() < 1e-9 && (signed_area(&two[1]) - 50.0).abs() < 1e-9);
        let line = voronoi(&[Vec2::new(1.0, 1.0), Vec2::new(5.0, 5.0), Vec2::new(9.0, 9.0)], b);
        let total: f64 = line.iter().map(|c| signed_area(c)).sum();
        assert!((total - 100.0).abs() < 1e-9);
        let dup = voronoi(&[Vec2::new(1.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(9.0, 9.0)], b);
        assert_eq!(dup.iter().filter(|c| c.is_empty()).count(), 1);
        assert!(voronoi(&[Vec2::new(1.0, 1.0)], Rect::new(0.0, 0.0, 0.0, 1.0))[0].is_empty());
    }
}
