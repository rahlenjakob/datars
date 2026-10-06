//! Projecting geometry: the d3-geo pipeline without streams.
//!
//! ```text
//! degrees ─► radians ─► rotate ─► spherical clip ─► resample + project ─► rectangle clip
//!                                 (antimeridian     (adaptive, in          (clip_extent,
//!                                  or horizon)       projected space)       Albers USA insets)
//! ```
//!
//! Edges between vertices are great-circle arcs. **Adaptive resampling** subdivides an edge until
//! its projected image is within `tolerance_px` of straight, so long edges curve correctly under
//! any projection (a 2-point route across the Atlantic becomes an arc; a meridian under a conic
//! stays straight without wasted points). Polygon winding on input doesn't matter: rings are
//! normalized on the sphere before clipping, and come out with exteriors of positive signed area
//! (clockwise on screen) and holes negative, ready for non-zero or even-odd fill.

use crate::clip::sphere::SphereClip;
use crate::clip::{group_rings, rect::RectClip, SPoint};
use crate::geometry::{Geometry, Line, Polygon, Ring};
use crate::projection::{Compiled, Projection};
use crate::sphere::{self, cartesian, dot, V3, EPS};
use datars_math::path::signed_area;
use datars_math::{m, PathData, Rect, Vec2};

/// Maximum subdivision depth per edge (2^16 pieces).
const MAX_DEPTH: u32 = 16;
/// Edges longer than 30° are always split, however straight they project.
const COS_MIN_DISTANCE: f64 = 0.866_025_403_784_438_6; // cos 30°

/// Geometry in projected space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProjectedGeometry {
    pub points: Vec<Vec2>,
    /// Open polylines.
    pub lines: Vec<Line>,
    /// Closed rings (first == last): exteriors with positive signed area, holes negative.
    pub rings: Vec<Ring>,
}

impl ProjectedGeometry {
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.lines.is_empty() && self.rings.is_empty()
    }

    /// Rings grouped into polygons (`[exterior, holes…]`) by winding and containment — what
    /// triangulation wants.
    pub fn polygons(&self) -> Vec<Polygon> {
        group_rings(self.rings.clone())
    }

    /// Rings as closed subpaths, lines as open ones (points are left to instanced symbols).
    pub fn to_path(&self) -> PathData {
        let mut p = PathData::new();
        for r in &self.rings {
            let n = if r.len() > 1 && r.first() == r.last() { r.len() - 1 } else { r.len() };
            if n >= 2 {
                p.move_to(r[0]);
                for q in &r[1..n] {
                    p.line_to(*q);
                }
                p.close();
            }
        }
        for l in &self.lines {
            if let Some((a, rest)) = l.split_first() {
                p.move_to(*a);
                for q in rest {
                    p.line_to(*q);
                }
            }
        }
        p
    }

    pub fn bounds(&self) -> Option<Rect> {
        let mut r = Rect::empty();
        for p in self.points.iter().chain(self.lines.iter().flatten()).chain(self.rings.iter().flatten()) {
            if p.is_finite() {
                r = r.include(*p);
            }
        }
        (!r.is_empty()).then_some(r)
    }

    /// Total ring area (holes subtract), in projected units².
    pub fn area(&self) -> f64 {
        self.rings.iter().map(|r| signed_area(r)).sum()
    }
}

/// Project a geometry with adaptive resampling (`tolerance_px` in projected units; d3's default
/// precision is 0.5; 0 disables resampling), spherical clipping and `clip_extent`.
pub fn project(g: &Geometry, projection: &Projection, tolerance_px: f64) -> ProjectedGeometry {
    let mut out = ProjectedGeometry::default();
    let delta2 = if tolerance_px > 0.0 { tolerance_px * tolerance_px } else { 0.0 };
    for part in projection.parts() {
        Pipeline { c: &part, delta2 }.geometry(g, &mut out);
    }
    out
}

/// Projected rings (closed) followed by lines: the flat form for stroking or even-odd filling.
pub fn project_geometry(g: &Geometry, projection: &Projection, tolerance_px: f64) -> Vec<Vec<Vec2>> {
    let p = project(g, projection, tolerance_px);
    p.rings.into_iter().chain(p.lines).collect()
}

/// A geometry as a path in projected space.
pub fn to_path(g: &Geometry, projection: &Projection, tolerance_px: f64) -> PathData {
    project(g, projection, tolerance_px).to_path()
}

/// The outline of the whole sphere in projected space (d3's `Sphere`): the edge of the world for
/// world projections, the horizon disc for a globe, each inset's box for Albers USA. Use it for
/// ocean fills and map frames — a world-sized polygon is ambiguous on the sphere, this isn't.
/// Empty for planar projections.
pub fn sphere(projection: &Projection, tolerance_px: f64) -> ProjectedGeometry {
    let mut out = ProjectedGeometry::default();
    let delta2 = if tolerance_px > 0.0 { tolerance_px * tolerance_px } else { 0.0 };
    for part in projection.parts() {
        let Some(clip) = part.preclip else { continue };
        let pipe = Pipeline { c: &part, delta2 };
        let ring = pipe.resample(&clip.outline(), true);
        let rings = match part.extent {
            Some(e) => {
                let spts = vec![ring.iter().map(|p| SPoint::new(p.x, p.y)).collect::<Vec<_>>()];
                rect_clip(e).clip_polygon(&spts).into_iter().map(|r| r.into_iter().map(|p| Vec2::new(p.x, p.y)).collect()).collect()
            }
            None => vec![ring],
        };
        for mut r in rings {
            if r.len() >= 3 {
                r.push(r[0]);
                out.rings.push(r);
            }
        }
    }
    out
}

struct Pipeline<'a> {
    c: &'a Compiled,
    delta2: f64,
}

#[derive(Clone, Copy)]
struct RPoint {
    v: Vec2,
    l: f64,
    c: V3,
}

impl Pipeline<'_> {
    fn geometry(&self, g: &Geometry, out: &mut ProjectedGeometry) {
        match g {
            Geometry::Point(p) => self.point(*p, out),
            Geometry::MultiPoint(ps) => ps.iter().for_each(|p| self.point(*p, out)),
            Geometry::LineString(l) => self.line(l, out),
            Geometry::MultiLineString(ls) => ls.iter().for_each(|l| self.line(l, out)),
            Geometry::Polygon(p) => self.polygon(p, out),
            Geometry::MultiPolygon(pp) => pp.iter().for_each(|p| self.polygon(p, out)),
            Geometry::Collection(gs) => gs.iter().for_each(|g| self.geometry(g, out)),
        }
    }

    fn point(&self, p: Vec2, out: &mut ProjectedGeometry) {
        if let Some(v) = self.c.forward(p) {
            out.points.push(v);
        }
    }

    fn line(&self, line: &[Vec2], out: &mut ProjectedGeometry) {
        let pieces: Vec<Vec<Vec2>> = match &self.c.preclip {
            None => vec![line.iter().map(|p| self.c.project_rotated(p.x, p.y)).collect()],
            Some(clip) => {
                let rotated: Vec<(f64, f64)> = line.iter().map(|p| self.c.rotate(*p)).collect();
                clip.clip_line(&rotated).iter().map(|s| self.resample(s, false)).collect()
            }
        };
        for piece in pieces {
            match self.c.extent {
                Some(e) => {
                    let rc = rect_clip(e);
                    let pts: Vec<SPoint> = piece.iter().map(|p| SPoint::new(p.x, p.y)).collect();
                    out.lines.extend(rc.clip_line(&pts).into_iter().map(|s| s.into_iter().map(|p| Vec2::new(p.x, p.y)).collect()));
                }
                None if piece.len() > 1 => out.lines.push(piece),
                None => {}
            }
        }
    }

    fn polygon(&self, poly: &Polygon, out: &mut ProjectedGeometry) {
        let rings: Vec<Vec<Vec2>> = match &self.c.preclip {
            None => planar_rings(poly, self.c),
            Some(clip) => self.sphere_rings(poly, clip),
        };
        let rings = match self.c.extent {
            Some(e) => {
                let spts: Vec<Vec<SPoint>> = rings.iter().map(|r| r.iter().map(|p| SPoint::new(p.x, p.y)).collect()).collect();
                rect_clip(e).clip_polygon(&spts).into_iter().map(|r| r.into_iter().map(|p| Vec2::new(p.x, p.y)).collect()).collect()
            }
            None => rings,
        };
        for mut r in rings {
            if r.len() < 3 {
                continue;
            }
            r.push(r[0]);
            out.rings.push(r);
        }
    }

    /// Rewind on the sphere, rotate, clip, then resample each resulting ring (open rings out).
    fn sphere_rings(&self, poly: &Polygon, clip: &SphereClip) -> Vec<Vec<Vec2>> {
        let rewound = sphere::polygon_radians(poly);
        if rewound.is_empty() {
            return Vec::new();
        }
        let rotated: Vec<Vec<(f64, f64)>> = rewound
            .iter()
            .map(|r| r.iter().map(|&(l, p)| self.c.rotate_radians(l, p)).collect())
            .collect();
        clip.clip_polygon(&rotated).iter().map(|r| self.resample(r, true)).collect()
    }

    /// Project a run of rotated points, subdividing edges adaptively (d3 `resample`).
    fn resample(&self, pts: &[(f64, f64)], closed: bool) -> Vec<Vec2> {
        if self.delta2 <= 0.0 {
            return pts.iter().map(|&(l, p)| self.c.project_rotated(l, p)).collect();
        }
        let mut out = Vec::with_capacity(pts.len());
        let mut first: Option<RPoint> = None;
        let mut prev: Option<RPoint> = None;
        for &(l, p) in pts {
            let cur = RPoint { v: self.c.project_rotated(l, p), l, c: cartesian(l, p) };
            if let Some(a) = prev {
                self.resample_edge(&a, &cur, MAX_DEPTH, &mut out);
            }
            out.push(cur.v);
            first.get_or_insert(cur);
            prev = Some(cur);
        }
        if closed {
            if let (Some(a), Some(b)) = (prev, first) {
                self.resample_edge(&a, &b, MAX_DEPTH, &mut out);
            }
        }
        out
    }

    fn resample_edge(&self, a: &RPoint, b: &RPoint, depth: u32, out: &mut Vec<Vec2>) {
        let d = b.v - a.v;
        let d2 = d.len2();
        if d2.is_nan() || d2 <= 4.0 * self.delta2 || depth == 0 {
            return;
        }
        let (sa, sb, sc) = (a.c[0] + b.c[0], a.c[1] + b.c[1], a.c[2] + b.c[2]);
        let mm = (sa * sa + sb * sb + sc * sc).sqrt();
        if mm == 0.0 {
            return; // antipodal: the great circle is undefined
        }
        let c = sc / mm;
        let phi2 = sphere::asin(c);
        let lambda2 = if (c.abs() - 1.0).abs() < EPS || (a.l - b.l).abs() < EPS { (a.l + b.l) / 2.0 } else { m::atan2(sb, sa) };
        let p = self.c.project_rotated(lambda2, phi2);
        let d_2 = p - a.v;
        let dz = d.y * d_2.x - d.x * d_2.y;
        if dz * dz / d2 > self.delta2 // perpendicular distance
            || ((d.x * d_2.x + d.y * d_2.y) / d2 - 0.5).abs() > 0.3 // midpoint near an end
            || dot(a.c, b.c) < COS_MIN_DISTANCE
        {
            let mid = RPoint { v: p, l: lambda2, c: [sa / mm, sb / mm, c] };
            self.resample_edge(a, &mid, depth - 1, out);
            out.push(p);
            self.resample_edge(&mid, b, depth - 1, out);
        }
    }
}

fn rect_clip(e: Rect) -> RectClip {
    RectClip::new(e.x, e.y, e.x1(), e.y1(), EPS)
}

/// Planar polygons: transform, then orient in projected space (exteriors positive).
fn planar_rings(poly: &Polygon, c: &Compiled) -> Vec<Vec<Vec2>> {
    poly.iter()
        .enumerate()
        .filter(|(_, r)| r.len() >= 3)
        .map(|(i, r)| {
            let mut v: Vec<Vec2> = r.iter().map(|p| c.project_rotated(p.x, p.y)).collect();
            if v.len() > 1 && v.first() == v.last() {
                v.pop();
            }
            if (signed_area(&v) > 0.0) != (i == 0) {
                v.reverse();
            }
            v
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn planar_is_identity() {
        let sq = Geometry::Polygon(vec![vec![v(0.0, 0.0), v(0.0, 10.0), v(10.0, 10.0), v(10.0, 0.0), v(0.0, 0.0)]]);
        let p = project(&sq, &Projection::planar(), 0.5);
        assert_eq!(p.rings.len(), 1);
        assert_eq!(p.rings[0].len(), 5);
        assert!((p.area() - 100.0).abs() < 1e-12, "oriented positive");
        assert!(p.rings[0].contains(&v(10.0, 10.0)));
        let path = p.to_path();
        assert_eq!(path.els.len(), 5, "move, 3 lines, close");
    }

    #[test]
    fn resampling_adds_points_on_long_edges() {
        let route = Geometry::LineString(vec![v(-74.0, 40.7), v(2.35, 48.86)]);
        let proj = Projection::equirectangular();
        let coarse = project(&route, &proj, 0.0);
        let fine = project(&route, &proj, 0.5);
        let finer = project(&route, &proj, 0.05);
        assert_eq!(coarse.lines[0].len(), 2);
        assert!(fine.lines[0].len() >= 5, "{} points", fine.lines[0].len());
        assert!(finer.lines[0].len() > fine.lines[0].len());
        // Every inserted point lies on the great circle (within float noise).
        let (a, b) = (crate::sphere::cartesian(-74.0 * crate::sphere::RAD, 40.7 * crate::sphere::RAD), crate::sphere::cartesian(2.35 * crate::sphere::RAD, 48.86 * crate::sphere::RAD));
        let n = crate::sphere::normalize(crate::sphere::cross(a, b));
        for p in &finer.lines[0] {
            let ll = proj.inverse(*p).unwrap();
            let c = crate::sphere::cartesian(ll.x * crate::sphere::RAD, ll.y * crate::sphere::RAD);
            assert!(crate::sphere::dot(n, c).abs() < 1e-9, "{ll:?} off the great circle");
        }
        // The great circle bulges north (smaller y on screen) of the straight chord.
        let mid = fine.lines[0][fine.lines[0].len() / 2];
        let chord = (coarse.lines[0][0] + coarse.lines[0][1]) / 2.0;
        assert!(mid.y < chord.y - 5.0);
        // A meridian is straight in equirectangular: no extra points beyond the 30° rule.
        let meridian = project(&Geometry::LineString(vec![v(10.0, -20.0), v(10.0, 20.0)]), &proj, 0.5);
        assert!(meridian.lines[0].len() <= 3);
    }
}
