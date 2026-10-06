//! Outlines and path morphs, for every `Geom`.
//!
//! Any geometry flattens to closed rings. Two shapes of different kinds morph through matched
//! outlines: both resampled to the same point count (level of detail by on-screen size, 32–256
//! points), wound clockwise on screen and started at the point straight above their centre, so
//! vertex *i* of either outline are neighbours.
//!
//! - **Disc** (default): the centre travels on a straight line while the outline blends
//!   relative to it, passing through an area-matched disc mid-flight (`round = 0.85·sin πt`) — a
//!   bar rounds into a blob, flies, and unfolds into its slice. Convex to convex through a disc,
//!   so shapes never fold over themselves (the "inside-out" bug class).
//! - **Resample**: the aligned outlines lerp point by point (good for convex-ish shapes).
//!
//! Multipolygons: the largest part morphs; the other parts shrink to (grow from) their centroids
//! inside the same path, so a country's islands fade into the travelling mainland.

use crate::interp::lerp_geom;
use crate::rules::{MorphStrategy, Partition};
use datars_math::path::{resample_closed, signed_area};
use datars_math::{m, Affine, PathData, Rect, Vec2};
use datars_scene::Geom;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

pub type Ring = Vec<Vec2>;

/// Closed rings of `g` under `xf`, flattened within `tol` (in `g`'s units), largest |area|
/// first. Open geometry becomes a there-and-back ring so lines can morph with shapes.
pub fn rings(g: &Geom, xf: &Affine, tol: f64) -> Vec<Ring> {
    let path = g.to_path();
    let mut out: Vec<(f64, usize, Ring)> = Vec::new();
    for (i, (pts, closed)) in path.flatten(tol).into_iter().enumerate() {
        let mut r: Ring = pts.iter().map(|p| xf.apply(*p)).collect();
        if closed {
            if r.len() > 1 && r.first() == r.last() {
                r.pop();
            }
        } else if r.len() > 2 {
            let back: Vec<Vec2> = r[1..r.len() - 1].iter().rev().copied().collect();
            r.extend(back);
        }
        if r.iter().all(|p| p.is_finite()) && !r.is_empty() {
            out.push((signed_area(&r).abs(), i, r));
        }
    }
    out.sort_by(|a, b| datars_math::total_cmp(b.0, a.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, _, r)| r).collect()
}

pub fn perimeter(r: &[Vec2]) -> f64 {
    if r.len() < 2 {
        return 0.0;
    }
    let mut s: f64 = r.windows(2).map(|w| w[0].dist(w[1])).sum();
    s += r[r.len() - 1].dist(r[0]);
    s
}

/// Outline point count for an on-screen perimeter: one point per ~4 px, in [32, 256], a
/// multiple of 4 (so rects and discs keep their corners and quadrants aligned).
pub fn lod_points(perimeter_px: f64) -> usize {
    let n = if perimeter_px.is_finite() { (perimeter_px / 4.0).ceil() as usize } else { 32 };
    let n = n.clamp(32, 256);
    n.div_ceil(4) * 4
}

/// Vertex average (the centre an evenly resampled outline rotates about).
pub fn centroid(o: &[Vec2]) -> Vec2 {
    if o.is_empty() {
        return Vec2::ZERO;
    }
    let n = o.len() as f64;
    let s = o.iter().fold(Vec2::ZERO, |acc, p| acc + *p);
    s / n
}

/// A pseudo-angle of `p` around `c`, clockwise from straight up, in [0, 4): monotone in the true
/// angle (a "diamond angle"), so finding the start vertex needs no `atan2`.
fn from_top(c: Vec2, p: Vec2) -> f64 {
    let (x, y) = (-(p.y - c.y), p.x - c.x); // x: up, y: right (clockwise on screen)
    let d = x.abs() + y.abs();
    if d.is_nan() || d <= 0.0 {
        return 0.0;
    }
    if y >= 0.0 {
        if x >= 0.0 {
            y / d
        } else {
            1.0 - x / d
        }
    } else if x < 0.0 {
        2.0 - y / d
    } else {
        3.0 + x / d
    }
}

/// Rotate a ring to start at its vertex closest (clockwise) to straight above its centre.
fn start_at_top(o: &mut Ring) {
    let c = centroid(o);
    let start = (0..o.len()).min_by(|&i, &j| datars_math::total_cmp(from_top(c, o[i]), from_top(c, o[j])).then(i.cmp(&j))).unwrap_or(0);
    o.rotate_left(start);
}

/// An outline running clockwise on screen (y down), starting at the point straight above its
/// centre — the order the morph's disc takes, so vertex i of any two outlines are neighbours.
pub fn aligned(mut o: Ring) -> Ring {
    if o.len() < 3 {
        return o;
    }
    if signed_area(&o) < 0.0 {
        o.reverse();
    }
    start_at_top(&mut o);
    o
}

/// Unit disc directions for `n` outline points (clockwise from 12 o'clock), shared between
/// morphs with the same point count.
fn disc_dirs(n: usize) -> Arc<[Vec2]> {
    static CACHE: OnceLock<Mutex<BTreeMap<usize, Arc<[Vec2]>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut g = match cache.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g.entry(n)
        .or_insert_with(|| {
            (0..n)
                .map(|i| {
                    let (s, c) = m::sin_cos(m::TAU * i as f64 / n as f64);
                    Vec2::new(s, -c)
                })
                .collect()
        })
        .clone()
}

/// `n` points around a closed ring. When the ring has at most `n` vertices every one is kept
/// (corners stay sharp mid-morph) and the rest are spread along the edges by length (largest
/// remainder, ties by edge order); otherwise points are spaced evenly by arc length.
pub fn resample_ring(pts: &[Vec2], n: usize) -> Ring {
    let m = pts.len();
    if m < 3 || m > n {
        return resample_closed(pts, n);
    }
    let seg: Vec<f64> = (0..m).map(|i| pts[i].dist(pts[(i + 1) % m])).collect();
    let total: f64 = seg.iter().sum();
    if total.is_nan() || total <= 1e-12 || !total.is_finite() {
        return vec![pts[0]; n];
    }
    let extra = n - m;
    let quotas: Vec<f64> = seg.iter().map(|s| s / total * extra as f64).collect();
    let mut alloc: Vec<usize> = quotas.iter().map(|q| q.floor() as usize).collect();
    let mut rem = extra - alloc.iter().sum::<usize>().min(extra);
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&i, &j| datars_math::total_cmp(quotas[j] - quotas[j].floor(), quotas[i] - quotas[i].floor()).then(i.cmp(&j)));
    for &i in &order {
        if rem == 0 {
            break;
        }
        alloc[i] += 1;
        rem -= 1;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..m {
        let (p, q) = (pts[i], pts[(i + 1) % m]);
        out.push(p);
        let k = alloc[i];
        for j in 1..=k {
            out.push(p.lerp(q, j as f64 / (k + 1) as f64));
        }
    }
    out.truncate(n);
    while out.len() < n {
        out.push(pts[0]);
    }
    out
}

/// A precomputed outline morph between two sets of rings.
#[derive(Clone, Debug, PartialEq)]
pub struct Morph {
    disc: bool,
    /// Main outlines: relative to their centroids (disc) or absolute (resample).
    a: Vec<Vec2>,
    b: Vec<Vec2>,
    ca: Vec2,
    cb: Vec2,
    ra: f64,
    rb: f64,
    dirs: Arc<[Vec2]>,
    extra_a: Vec<(Ring, Vec2)>,
    extra_b: Vec<(Ring, Vec2)>,
}

impl Morph {
    /// Build from rings (largest first) and a point count.
    pub fn new(ra_rings: Vec<Ring>, rb_rings: Vec<Ring>, n: usize, fallback_a: Vec2, fallback_b: Vec2, disc: bool) -> Morph {
        let n = n.max(8);
        let mut ia = ra_rings.into_iter();
        let mut ib = rb_rings.into_iter();
        let main = |r: Option<Ring>, fb: Vec2| -> Ring {
            match r {
                Some(r) if r.len() >= 2 => aligned(resample_ring(&r, n)),
                Some(r) if r.len() == 1 => vec![r[0]; n],
                _ => vec![fb; n],
            }
        };
        let mut oa = main(ia.next(), fallback_a);
        let mut ob = main(ib.next(), fallback_b);
        let area = |o: &[Vec2]| signed_area(o).abs();
        // A degenerate outline (a line's there-and-back ring) has no winding of its own: pick the
        // orientation that keeps the early (late) morph area positive.
        let (aa, ab) = (area(&oa), area(&ob));
        if aa < 1e-9 * ab.max(1e-300) || ab < 1e-9 * aa.max(1e-300) {
            let flip = |o: &Ring| -> Ring {
                let mut r: Ring = o.iter().rev().copied().collect();
                start_at_top(&mut r);
                r
            };
            let score = |x: &Ring, y: &Ring| -> f64 {
                let probe = Morph::build(x.clone(), y.clone(), Vec::new(), Vec::new(), disc);
                [0.01, 0.05, 0.15, 0.85, 0.95, 0.99].iter().map(|&t| signed_area(&probe.main_at(t))).fold(f64::INFINITY, f64::min)
            };
            if aa < ab {
                let alt = flip(&oa);
                if score(&alt, &ob) > score(&oa, &ob) {
                    oa = alt;
                }
            } else {
                let alt = flip(&ob);
                if score(&oa, &alt) > score(&oa, &ob) {
                    ob = alt;
                }
            }
        }
        let extra = |it: std::vec::IntoIter<Ring>| it.filter(|r| r.len() >= 2).map(|r| {
            let c = centroid(&r);
            (r, c)
        }).collect();
        Morph::build(oa, ob, extra(ia), extra(ib), disc)
    }

    fn build(oa: Ring, ob: Ring, extra_a: Vec<(Ring, Vec2)>, extra_b: Vec<(Ring, Vec2)>, disc: bool) -> Morph {
        let n = oa.len();
        let area = |o: &[Vec2]| signed_area(o).abs();
        let (ca, cb) = (centroid(&oa), centroid(&ob));
        let (ra, rb) = ((area(&oa) / m::PI).sqrt(), (area(&ob) / m::PI).sqrt());
        let dirs = disc_dirs(n);
        let (a, b) = if disc { (oa.iter().map(|p| *p - ca).collect(), ob.iter().map(|p| *p - cb).collect()) } else { (oa, ob) };
        Morph { disc, a, b, ca, cb, ra, rb, dirs, extra_a, extra_b }
    }

    /// The main outline at progress `t` (clamped to [0, 1]).
    pub fn main_at(&self, t: f64) -> Vec<Vec2> {
        self.main_iter(t).collect()
    }

    fn main_iter(&self, t: f64) -> impl ExactSizeIterator<Item = Vec2> + '_ {
        let t = t.clamp(0.0, 1.0);
        let (c, r, round) = if self.disc {
            (self.ca.lerp(self.cb, t), self.ra + (self.rb - self.ra) * t, if t > 0.0 && t < 1.0 { 0.85 * m::sin(m::PI * t) } else { 0.0 })
        } else {
            (Vec2::ZERO, 0.0, 0.0)
        };
        let disc = self.disc;
        self.a.iter().zip(&self.b).enumerate().map(move |(i, (p, q))| {
            let blend = p.lerp(*q, t);
            if disc {
                c + blend.lerp(self.dirs[i] * r, round)
            } else {
                blend
            }
        })
    }

    /// The morphed path at progress `t`.
    pub fn at(&self, t: f64) -> PathData {
        let t = t.clamp(0.0, 1.0);
        let mut p = PathData::polygon_iter(self.main_iter(t));
        let mut part = |r: &Ring, c: Vec2, k: f64| {
            if k > 0.0 {
                p.extend_polygon(r.iter().map(|q| c + (*q - c) * k));
            }
        };
        for (r, c) in &self.extra_a {
            part(r, *c, 1.0 - t);
        }
        for (r, c) in &self.extra_b {
            part(r, *c, t);
        }
        p
    }

    pub fn points(&self) -> usize {
        self.a.len()
    }
}

/// Morph two geometries at `t`: same kinds interpolate parameters; different kinds morph
/// outlines with `strategy` (`Crossfade` falls back to a disc here — crossfades need two nodes).
pub fn morph_geoms(a: &Geom, b: &Geom, t: f64, strategy: MorphStrategy) -> Geom {
    if t <= 0.0 {
        return a.clone();
    }
    if t >= 1.0 {
        return b.clone();
    }
    if let Some(g) = lerp_geom(a, b, t, t) {
        return g;
    }
    let (ra, rb) = (rings(a, &Affine::IDENTITY, 0.1), rings(b, &Affine::IDENTITY, 0.1));
    let per = ra.first().map_or(0.0, |r| perimeter(r)).max(rb.first().map_or(0.0, |r| perimeter(r)));
    let m = Morph::new(ra, rb, lod_points(per), a.center(), b.center(), strategy != MorphStrategy::Resample);
    Geom::path(m.at(t))
}

// ---- partitions (hierarchy splits and merges) ------------------------------------------------

/// Clip a polygon to an axis-aligned rectangle (Sutherland–Hodgman; the clip region is convex, so
/// concave subjects clip correctly up to degenerate seams).
pub fn clip_rect(poly: &[Vec2], r: Rect) -> Ring {
    let mut out: Ring = poly.to_vec();
    let edges: [(u8, f64); 4] = [(0, r.x), (1, r.x1()), (2, r.y), (3, r.y1())];
    for (side, v) in edges {
        if out.is_empty() {
            break;
        }
        let inside = |p: &Vec2| match side {
            0 => p.x >= v,
            1 => p.x <= v,
            2 => p.y >= v,
            _ => p.y <= v,
        };
        let cross = |p: Vec2, q: Vec2| -> Vec2 {
            let t = match side {
                0 | 1 => (v - p.x) / (q.x - p.x),
                _ => (v - p.y) / (q.y - p.y),
            };
            let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
            p.lerp(q, t)
        };
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (p, q) = (input[i], input[(i + 1) % input.len()]);
            match (inside(&p), inside(&q)) {
                (true, true) => out.push(q),
                (true, false) => out.push(cross(p, q)),
                (false, true) => {
                    out.push(cross(p, q));
                    out.push(q);
                }
                (false, false) => {}
            }
        }
    }
    out
}

/// Cells tiling `b` into `n` pieces.
fn cells(b: Rect, n: usize, mode: Partition) -> Vec<Rect> {
    let n = n.max(1);
    match mode {
        // (`Auto` is settled by the matcher; here it means slices.)
        Partition::Slices | Partition::Auto | Partition::Proportional => {
            if b.h.abs() >= b.w.abs() {
                (0..n).map(|i| Rect::new(b.x, b.y + b.h * i as f64 / n as f64, b.w, b.h / n as f64)).collect()
            } else {
                (0..n).map(|i| Rect::new(b.x + b.w * i as f64 / n as f64, b.y, b.w / n as f64, b.h)).collect()
            }
        }
        Partition::Grid => {
            let aspect = if b.h > 0.0 { b.w / b.h } else { 1.0 };
            let cols = ((n as f64 * aspect).sqrt().round() as usize).clamp(1, n);
            let rows = n.div_ceil(cols);
            let mut out = Vec::with_capacity(n);
            for r in 0..rows {
                let in_row = if r + 1 == rows { n - cols * (rows - 1) } else { cols };
                let y = b.y + b.h * r as f64 / rows as f64;
                for c in 0..in_row {
                    out.push(Rect::new(b.x + b.w * c as f64 / in_row as f64, y, b.w / in_row as f64, b.h / rows as f64));
                }
            }
            out
        }
    }
}

/// Divide `g` into `n` pieces that tile it: slices along its long axis (angle or radius for
/// arcs, arc length for lines) or a grid. Pieces are in `g`'s coordinates, in order along the
/// axis (grids row-major).
/// Like [`partition`], with slices as wide as `weights` (proportional mode, rectangles; other
/// shapes and modes partition evenly).
pub fn partition_weighted(g: &Geom, weights: &[f64], mode: Partition) -> Vec<Geom> {
    let total: f64 = weights.iter().filter(|w| w.is_finite() && **w > 0.0).sum();
    match (g, mode) {
        (Geom::Rect { x, y, w, h, .. }, Partition::Proportional) if total > 0.0 && weights.len() > 1 => {
            let (x0, w0) = if *w < 0.0 { (x + w, -w) } else { (*x, *w) };
            let (y0, h0) = if *h < 0.0 { (y + h, -h) } else { (*y, *h) };
            let mut at = 0.0;
            weights
                .iter()
                .map(|wt| {
                    let share = if wt.is_finite() && *wt > 0.0 { wt / total } else { 0.0 };
                    let r = if h0 >= w0 {
                        // Tall bars stack from the base up (the order a stacked bar reads).
                        Geom::rect(x0, y0 + h0 * (1.0 - at - share), w0, h0 * share)
                    } else {
                        Geom::rect(x0 + w0 * at, y0, w0 * share, h0)
                    };
                    at += share;
                    r
                })
                .collect()
        }
        _ => partition(g, weights.len(), mode),
    }
}

pub fn partition(g: &Geom, n: usize, mode: Partition) -> Vec<Geom> {
    let n = n.max(1);
    match g {
        Geom::Rect { x, y, w, h, .. } => {
            let (x0, w0) = if *w < 0.0 { (x + w, -w) } else { (*x, *w) };
            let (y0, h0) = if *h < 0.0 { (y + h, -h) } else { (*y, *h) };
            cells(Rect::new(x0, y0, w0, h0), n, mode).into_iter().map(|c| Geom::rect(c.x, c.y, c.w, c.h)).collect()
        }
        Geom::Arc { cx, cy, r0, r1, a0, a1 } => {
            let (ang, rad) = ((a1 - a0).abs() * (r0 + r1) / 2.0, (r1 - r0).abs());
            // Treat (angle, radius) as a rectangle in polar space; long axis = angle when the arc
            // is longer than it is thick.
            let (w, h) = (a1 - a0, r1 - r0);
            let polar = match mode {
                Partition::Slices | Partition::Auto | Partition::Proportional if ang >= rad => (0..n).map(|i| Rect::new(*a0 + w * i as f64 / n as f64, *r0, w / n as f64, h)).collect(),
                Partition::Slices | Partition::Auto | Partition::Proportional => (0..n).map(|i| Rect::new(*a0, *r0 + h * i as f64 / n as f64, w, h / n as f64)).collect(),
                Partition::Grid => cells(Rect::new(*a0, *r0, w, h), n, mode),
            };
            polar
                .into_iter()
                .map(|c: Rect| Geom::Arc { cx: *cx, cy: *cy, r0: c.y, r1: c.y + c.h, a0: c.x, a1: c.x + c.w })
                .collect()
        }
        Geom::Segment { .. } | Geom::Polyline { closed: false, .. } => {
            let pts: Vec<Vec2> = g.to_path().flatten(0.1).into_iter().flat_map(|(p, _)| p).collect();
            split_polyline(&pts, n).into_iter().map(Geom::polyline).collect()
        }
        _ => {
            let b = g.bounds();
            // Flatten relative to the shape's size so small shapes partition as finely as big ones.
            let tol = (m::hypot(b.w, b.h) * 5e-4).max(1e-4);
            let rs = rings(g, &Affine::IDENTITY, tol);
            cells(b, n, mode)
                .into_iter()
                .map(|c| {
                    let mut p = PathData::new();
                    for r in &rs {
                        let cl = clip_rect(r, c);
                        if cl.len() >= 3 {
                            p.extend(&PathData::polygon(&cl));
                        }
                    }
                    if p.is_empty() {
                        // A cell outside a concave shape: a degenerate piece at the cell centre.
                        let cc = c.center();
                        p = PathData::polygon(&[cc, cc, cc]);
                    }
                    Geom::path(p)
                })
                .collect()
        }
    }
}

/// Split an open polyline into `n` consecutive pieces of equal arc length.
fn split_polyline(pts: &[Vec2], n: usize) -> Vec<Vec<Vec2>> {
    if pts.len() < 2 {
        let p = pts.first().copied().unwrap_or_default();
        return vec![vec![p, p]; n];
    }
    let seg: Vec<f64> = pts.windows(2).map(|w| w[0].dist(w[1])).collect();
    let total: f64 = seg.iter().sum();
    let at = |d: f64| -> (usize, Vec2) {
        let mut acc = 0.0;
        for (i, s) in seg.iter().enumerate() {
            if acc + s >= d || i + 1 == seg.len() {
                let f = if *s > 0.0 { ((d - acc) / s).clamp(0.0, 1.0) } else { 0.0 };
                return (i, pts[i].lerp(pts[i + 1], f));
            }
            acc += s;
        }
        (0, pts[0])
    };
    (0..n)
        .map(|k| {
            let (d0, d1) = (total * k as f64 / n as f64, total * (k + 1) as f64 / n as f64);
            let (i0, p0) = at(d0);
            let (i1, p1) = at(d1);
            let mut v = vec![p0];
            v.extend_from_slice(&pts[i0 + 1..=i1]);
            v.push(p1);
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligned_outlines_start_at_top_clockwise() {
        let r = rings(&Geom::rect(0.0, 0.0, 10.0, 20.0), &Affine::IDENTITY, 0.1);
        let o = aligned(resample_closed(&r[0], 64));
        assert_eq!(o.len(), 64);
        assert!((o[0].x - 5.0).abs() < 1e-9 && o[0].y.abs() < 1e-9, "top-centre first: {:?}", o[0]);
        assert!(signed_area(&o) > 0.0);
    }

    #[test]
    fn partitions_tile_the_rect() {
        for mode in [Partition::Slices, Partition::Grid] {
            let ps = partition(&Geom::rect(0.0, 0.0, 40.0, 120.0), 30, mode);
            assert_eq!(ps.len(), 30);
            let area: f64 = ps.iter().map(|g| match g {
                Geom::Rect { w, h, .. } => w * h,
                _ => 0.0,
            }).sum();
            assert!((area - 4800.0).abs() < 1e-6, "{mode:?}: {area}");
        }
    }

    #[test]
    fn clip_keeps_inside_part() {
        let sq = [Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)];
        let c = clip_rect(&sq, Rect::new(5.0, -1.0, 20.0, 20.0));
        assert!((signed_area(&c) - 50.0).abs() < 1e-9);
    }
}
