//! Clipping, planar and spherical.
//!
//! * **Rectangle clipping** (`clip_polygon`, `clip_line`, `clip_geometry`) for tiles and viewports:
//!   polygons are cut into visible segments and rejoined along the rectangle boundary
//!   (Weiler–Atherton style), so concave coastlines crossing an edge many times come out as
//!   separate, valid rings with holes preserved — unlike Sutherland–Hodgman, which stitches the
//!   pieces into one self-intersecting ring. Light and wasm-friendly.
//! * **Boolean clipping** (`clip_polygon_boolean`, `build` feature only) through the `geo` crate's
//!   overlay engine, for the tile build pipeline where inputs can be self-touching or invalid.
//! * **Spherical clipping** (antimeridian cutting, small-circle clipping) is internal to the
//!   projection pipeline (`project`).

mod rejoin;
pub(crate) mod rect;
pub(crate) mod sphere;

pub(crate) use rejoin::SPoint;

use crate::geometry::{Geometry, Line, Polygon, Ring};
use datars_math::path::signed_area;
use datars_math::{Rect, Vec2};

/// Rectangles are normalized to this size before clipping, so the clipper's epsilon is relative
/// (tile edges at zoom 20 in 0..1 world units are 1e-6 apart).
const NORM: f64 = 4096.0;
const NORM_EPS: f64 = 1e-6;

struct Frame {
    r: Rect,
}

impl Frame {
    fn to(&self, p: Vec2) -> SPoint {
        SPoint::new((p.x - self.r.x) / self.r.w * NORM, (p.y - self.r.y) / self.r.h * NORM)
    }
    fn from(&self, p: SPoint) -> Vec2 {
        Vec2::new(self.r.x + p.x / NORM * self.r.w, self.r.y + p.y / NORM * self.r.h)
    }
    fn clipper(&self) -> rect::RectClip {
        rect::RectClip::new(0.0, 0.0, NORM, NORM, NORM_EPS)
    }
}

fn frame(r: Rect) -> Option<Frame> {
    (r.w > 0.0 && r.h > 0.0 && r.w.is_finite() && r.h.is_finite()).then_some(Frame { r })
}

/// Open ring (no closing duplicate) oriented with the requested sign of signed area.
fn oriented_open(ring: &[Vec2], positive: bool) -> Vec<Vec2> {
    let mut r = ring.to_vec();
    if r.len() > 1 && r.first() == r.last() {
        r.pop();
    }
    if (signed_area(&r) > 0.0) != positive {
        r.reverse();
    }
    r
}

fn close(mut r: Vec<Vec2>) -> Ring {
    if let (Some(&a), Some(&b)) = (r.first(), r.last()) {
        if a != b {
            r.push(a);
        }
    }
    r
}

/// Clip a polygon (`[exterior, holes…]`, any winding) to a rectangle. Returns the pieces as
/// polygons with closed rings: exteriors with positive signed area (clockwise on a y-down
/// screen), holes negative.
pub fn clip_polygon(polygon: &Polygon, rect: Rect) -> Vec<Polygon> {
    let Some(f) = frame(rect) else { return Vec::new() };
    let rings: Vec<Vec<SPoint>> = polygon
        .iter()
        .enumerate()
        .filter(|(_, r)| r.len() >= 3)
        .map(|(i, r)| oriented_open(r, i == 0).into_iter().map(|p| f.to(p)).collect())
        .collect();
    if rings.is_empty() {
        return Vec::new();
    }
    let out = f.clipper().clip_polygon(&rings);
    group_rings(out.into_iter().map(|r| close(r.into_iter().map(|p| f.from(p)).collect())).collect())
}

/// Clip a polyline to a rectangle: the runs inside, in order.
pub fn clip_line(line: &[Vec2], rect: Rect) -> Vec<Line> {
    let Some(f) = frame(rect) else { return Vec::new() };
    let pts: Vec<SPoint> = line.iter().map(|p| f.to(*p)).collect();
    f.clipper().clip_line(&pts).into_iter().map(|s| s.into_iter().map(|p| f.from(p)).collect()).collect()
}

/// Clip any geometry to a rectangle. Points outside are dropped; the result keeps the input's
/// kind where possible (a polygon may become a multipolygon).
pub fn clip_geometry(g: &Geometry, rect: Rect) -> Geometry {
    let inside = |p: &Vec2| rect.contains(*p);
    match g {
        Geometry::Point(p) => {
            if inside(p) { g.clone() } else { Geometry::MultiPoint(Vec::new()) }
        }
        Geometry::MultiPoint(ps) => Geometry::MultiPoint(ps.iter().copied().filter(inside).collect()),
        Geometry::LineString(l) => lines_geometry(clip_line(l, rect)),
        Geometry::MultiLineString(ls) => Geometry::MultiLineString(ls.iter().flat_map(|l| clip_line(l, rect)).collect()),
        Geometry::Polygon(p) => polygons_geometry(clip_polygon(p, rect)),
        Geometry::MultiPolygon(pp) => Geometry::MultiPolygon(pp.iter().flat_map(|p| clip_polygon(p, rect)).collect()),
        Geometry::Collection(gs) => Geometry::Collection(gs.iter().map(|g| clip_geometry(g, rect)).collect()),
    }
}

fn lines_geometry(mut ls: Vec<Line>) -> Geometry {
    if ls.len() == 1 {
        Geometry::LineString(ls.remove(0))
    } else {
        Geometry::MultiLineString(ls)
    }
}

fn polygons_geometry(mut ps: Vec<Polygon>) -> Geometry {
    if ps.len() == 1 {
        Geometry::Polygon(ps.remove(0))
    } else {
        Geometry::MultiPolygon(ps)
    }
}

/// Group closed rings into polygons by winding: rings with positive signed area are exteriors,
/// negative ones holes of the smallest exterior containing them. A hole outside every exterior is
/// kept as an exterior (reversed) rather than lost; zero-area rings are dropped.
pub fn group_rings(rings: Vec<Ring>) -> Vec<Polygon> {
    let mut exteriors: Vec<(f64, Ring)> = Vec::new();
    let mut holes: Vec<Ring> = Vec::new();
    for r in rings {
        let a = signed_area(&r);
        if a > 0.0 {
            exteriors.push((a, r));
        } else if a < 0.0 {
            holes.push(r);
        }
    }
    let mut polys: Vec<Polygon> = exteriors.iter().map(|(_, r)| vec![r.clone()]).collect();
    for h in holes {
        // The exterior containing most of the hole's vertices (smallest on ties) owns it.
        let mut best: Option<(usize, usize, f64)> = None;
        for (i, (a, e)) in exteriors.iter().enumerate() {
            let n = h.iter().filter(|p| ring_contains(e, **p)).count();
            if n > 0 && best.is_none_or(|(_, bn, ba)| n > bn || (n == bn && *a < ba)) {
                best = Some((i, n, *a));
            }
        }
        match best {
            Some((i, _, _)) => polys[i].push(h),
            None => {
                let mut r = h;
                r.reverse();
                polys.push(vec![r]);
            }
        }
    }
    polys
}

/// Even-odd point in ring (planar).
pub fn ring_contains(ring: &[Vec2], p: Vec2) -> bool {
    let n = ring.len();
    let mut inside = false;
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Even-odd point in polygon (planar): inside the exterior and outside every hole.
pub fn polygon_contains_planar(polygon: &Polygon, p: Vec2) -> bool {
    polygon.iter().filter(|r| ring_contains(r, p)).count() % 2 == 1
}

/// Robust boolean intersection with a rectangle via the `geo` crate (build pipeline only).
/// Handles self-touching and overlapping input that the runtime clipper assumes away.
#[cfg(feature = "build")]
pub fn clip_polygon_boolean(polygon: &Polygon, rect: Rect) -> Vec<Polygon> {
    use geo::{BooleanOps, Coord, LineString};
    let finite = |r: &Ring| r.len() >= 4 && r.iter().all(|p| p.is_finite());
    let Some(ext) = polygon.first().filter(|r| finite(r)) else { return Vec::new() };
    let ls = |r: &Ring| LineString::from(r.iter().map(|p| Coord { x: p.x, y: p.y }).collect::<Vec<_>>());
    let subject = geo::Polygon::new(ls(ext), polygon[1..].iter().filter(|r| finite(r)).map(ls).collect());
    let clip = geo::Rect::new(Coord { x: rect.x, y: rect.y }, Coord { x: rect.x1(), y: rect.y1() }).to_polygon();
    let back = |l: &LineString<f64>| l.coords().map(|c| Vec2::new(c.x, c.y)).collect::<Ring>();
    subject
        .intersection(&clip)
        .0
        .iter()
        .map(|p| {
            let mut rings = vec![oriented_closed(back(p.exterior()), true)];
            rings.extend(p.interiors().iter().map(|h| oriented_closed(back(h), false)));
            rings
        })
        .collect()
}

#[cfg(feature = "build")]
fn oriented_closed(r: Ring, positive: bool) -> Ring {
    close(oriented_open(&r, positive))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    fn area(p: &Polygon) -> f64 {
        p.iter().map(|r| signed_area(r)).sum()
    }

    #[test]
    fn concave_comb_splits_into_separate_pieces() {
        // A comb with 5 teeth pointing up; clipping away the spine leaves 5 separate teeth.
        let mut ring = vec![v(0.0, 10.0)];
        for i in 0..5 {
            let x = i as f64 * 2.0;
            ring.extend([v(x, 0.0), v(x + 1.0, 0.0), v(x + 1.0, 8.0)]);
            if i < 4 {
                ring.push(v(x + 2.0, 8.0));
            }
        }
        ring.extend([v(9.0, 10.0), v(0.0, 10.0)]);
        let pieces = clip_polygon(&vec![ring], Rect::new(-1.0, -1.0, 12.0, 6.0));
        assert_eq!(pieces.len(), 5, "{pieces:?}");
        for p in &pieces {
            assert!((area(p) - 5.0).abs() < 1e-9, "each tooth is 1×5");
            assert!(p[0].iter().all(|q| q.y <= 5.0 + 1e-9 && q.y >= -1e-9));
        }
    }

    #[test]
    fn holes_survive_and_are_cut() {
        let outer = vec![v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(0.0, 10.0), v(0.0, 0.0)];
        let hole = vec![v(3.0, 3.0), v(7.0, 3.0), v(7.0, 7.0), v(3.0, 7.0), v(3.0, 3.0)];
        // Whole: the hole stays a hole.
        let out = clip_polygon(&vec![outer.clone(), hole.clone()], Rect::new(-1.0, -1.0, 12.0, 12.0));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 2);
        assert!((area(&out[0]) - 84.0).abs() < 1e-9);
        // Cut through the hole: one C-shaped piece, no separate hole.
        let out = clip_polygon(&vec![outer.clone(), hole.clone()], Rect::new(-1.0, -1.0, 6.0, 12.0));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].len(), 1);
        assert!((area(&out[0]) - (50.0 - 8.0)).abs() < 1e-9, "{}", area(&out[0]));
        // Rect inside the hole: nothing.
        assert!(clip_polygon(&vec![outer.clone(), hole.clone()], Rect::new(4.0, 4.0, 2.0, 2.0)).is_empty());
        // Rect inside the solid part: the whole rect.
        let inner = clip_polygon(&vec![outer, hole], Rect::new(0.5, 0.5, 2.0, 2.0));
        assert_eq!(inner.len(), 1);
        assert!((area(&inner[0]) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn winding_of_input_does_not_matter() {
        let cw = vec![v(0.0, 0.0), v(4.0, 0.0), v(4.0, 4.0), v(0.0, 4.0), v(0.0, 0.0)];
        let mut ccw = cw.clone();
        ccw.reverse();
        let r = Rect::new(2.0, -1.0, 5.0, 6.0);
        let a = clip_polygon(&vec![cw], r);
        let b = clip_polygon(&vec![ccw], r);
        assert!((area(&a[0]) - 8.0).abs() < 1e-9 && (area(&b[0]) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn works_at_tiny_scales() {
        // A z20-sized tile in 0..1 world units.
        let s = 1.0 / (1u64 << 20) as f64;
        let r = Rect::new(0.5, 0.5, s, s);
        let tri = vec![v(0.5 - s, 0.5 - s), v(0.5 + 3.0 * s, 0.5 + 0.5 * s), v(0.5 - s, 0.5 + 2.0 * s), v(0.5 - s, 0.5 - s)];
        let out = clip_polygon(&vec![tri], r);
        assert_eq!(out.len(), 1);
        assert!(out[0][0].iter().all(|p| r.inset(-1e-15).contains(*p)));
    }

    #[test]
    fn lines_split_into_runs() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let line = vec![v(1.0, 1.0), v(5.0, 5.0), v(20.0, 5.0), v(25.0, 5.0), v(9.0, 9.0), v(8.0, 8.0)];
        let runs = clip_line(&line, r);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0][0], v(1.0, 1.0));
        assert!((runs[0].last().unwrap().x - 10.0).abs() < 1e-9);
        assert_eq!(*runs[1].last().unwrap(), v(8.0, 8.0));
        let g = clip_geometry(&Geometry::MultiPoint(vec![v(1.0, 1.0), v(11.0, 1.0)]), r);
        assert_eq!(g.points(), vec![v(1.0, 1.0)]);
    }

    #[test]
    fn grouping_and_containment() {
        let ext = vec![v(0.0, 0.0), v(10.0, 0.0), v(10.0, 10.0), v(0.0, 10.0), v(0.0, 0.0)];
        let mut hole = vec![v(2.0, 2.0), v(4.0, 2.0), v(4.0, 4.0), v(2.0, 4.0), v(2.0, 2.0)];
        hole.reverse();
        let far = vec![v(20.0, 0.0), v(21.0, 0.0), v(21.0, 1.0), v(20.0, 0.0)];
        let polys = group_rings(vec![hole.clone(), ext.clone(), far]);
        assert_eq!(polys.len(), 2);
        assert_eq!(polys[0].len(), 2, "hole attached to its exterior");
        assert!(polygon_contains_planar(&polys[0], v(1.0, 1.0)));
        assert!(!polygon_contains_planar(&polys[0], v(3.0, 3.0)));
    }

    #[cfg(feature = "build")]
    #[test]
    fn boolean_clip_matches_runtime_clip() {
        let u = vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 3.0), v(2.0, 3.0), v(2.0, 0.0), v(3.0, 0.0), v(3.0, 4.0), v(0.0, 4.0), v(0.0, 0.0)];
        let r = Rect::new(-1.0, 1.5, 5.0, 5.0);
        let a: f64 = clip_polygon_boolean(&vec![u.clone()], r).iter().map(area).sum();
        let b: f64 = clip_polygon(&vec![u], r).iter().map(area).sum();
        assert!((a - b).abs() < 1e-9, "{a} vs {b}");
    }
}
