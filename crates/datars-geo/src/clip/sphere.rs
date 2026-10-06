//! Clipping on the sphere, before projection (d3-geo `clip/antimeridian.js`, `clip/circle.js`,
//! `clip/index.js`): cutting at the antimeridian for world projections and small-circle clipping
//! at the horizon for globes. Works on rotated coordinates in radians.

use super::rejoin::{point_equal, rejoin, SPoint};
use crate::sphere::{self, cartesian, cross, dot, normalize, polygon_contains, spherical, EPS, HALF_PI, PI, RAD, TAU};
use datars_math::m;
use std::cmp::Ordering;

/// Which edge a spherical clip cuts along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum SphereClip {
    /// Cut lines and polygons at λ = ±π (cylindrical, pseudo-cylindrical and conic projections).
    Antimeridian,
    /// Keep what lies within `radius` (radians) of the rotated centre (0, 0): the visible
    /// hemisphere of an orthographic globe when the radius is 90°.
    Circle { radius: f64 },
}

/// A line cut into visible segments, plus d3's "clean" code: bit 0 = no intersections; bit 1 =
/// the first and last segments should be joined (the ring started and ended visible).
struct Cut {
    segments: Vec<Vec<SPoint>>,
    clean: u8,
}

impl SphereClip {
    pub fn visible(&self, l: f64, p: f64) -> bool {
        match *self {
            SphereClip::Antimeridian => true,
            SphereClip::Circle { radius } => m::cos(l) * m::cos(p) > m::cos(radius),
        }
    }

    /// The point from which "inside" is judged for the whole-edge fallback.
    fn start(&self) -> (f64, f64) {
        match *self {
            SphereClip::Antimeridian => (-PI, -HALF_PI),
            SphereClip::Circle { radius } => {
                if m::cos(radius) > 0.0 {
                    (0.0, -radius)
                } else {
                    (-PI, radius - PI)
                }
            }
        }
    }

    fn cut(&self, pts: &[(f64, f64)]) -> Cut {
        match *self {
            SphereClip::Antimeridian => cut_antimeridian(pts),
            SphereClip::Circle { radius } => CircleClip::new(radius).cut(pts),
        }
    }

    fn interpolate(&self, from: Option<SPoint>, to: Option<SPoint>, direction: i32, out: &mut Vec<SPoint>) {
        match *self {
            SphereClip::Antimeridian => interpolate_antimeridian(from, to, direction, out),
            SphereClip::Circle { radius } => circle_stream(radius, 6.0 * RAD, direction, from, to, out),
        }
    }

    /// The whole clip edge as a clockwise ring: the world's outline (antimeridian) or the
    /// horizon circle — d3's `Sphere`.
    pub fn outline(&self) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        self.interpolate(None, None, 1, &mut out);
        out.into_iter().map(|p| (p.x, p.y)).collect()
    }

    /// Clip an open line into its visible pieces.
    pub fn clip_line(&self, pts: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
        self.cut(pts)
            .segments
            .into_iter()
            .filter(|s| s.len() > 1)
            .map(|s| s.into_iter().map(|p| (p.x, p.y)).collect())
            .collect()
    }

    /// Clip a polygon (rings without closing duplicates, d3 winding) into rings.
    pub fn clip_polygon(&self, polygon: &[Vec<(f64, f64)>]) -> Vec<Vec<(f64, f64)>> {
        let mut out: Vec<Vec<SPoint>> = Vec::new();
        let mut segments: Vec<Vec<SPoint>> = Vec::new();
        for ring in polygon {
            if ring.is_empty() {
                continue;
            }
            let mut closed = ring.clone();
            closed.push(ring[0]);
            let Cut { segments: mut ring_segments, clean } = self.cut(&closed);
            let n = ring_segments.len();
            if n == 0 {
                continue;
            }
            if clean & 1 != 0 {
                // No intersections: the ring passes through whole (minus its closing point).
                let seg = &ring_segments[0];
                if seg.len() > 1 {
                    out.push(seg[..seg.len() - 1].to_vec());
                }
                continue;
            }
            if n > 1 && clean & 2 != 0 {
                let last = ring_segments.pop().unwrap_or_default();
                let first = ring_segments.remove(0);
                ring_segments.push(last.into_iter().chain(first).collect());
            }
            segments.extend(ring_segments.into_iter().filter(|s| s.len() > 1));
        }
        let start_inside = polygon_contains(polygon, self.start());
        if !segments.is_empty() {
            let cmp = |a: &SPoint, b: &SPoint| compare_intersection(a, b);
            let interp = |f: Option<SPoint>, t: Option<SPoint>, d: i32, o: &mut Vec<SPoint>| self.interpolate(f, t, d, o);
            rejoin(segments, &cmp, start_inside, &interp, EPS, &mut out);
        } else if start_inside {
            let mut ring = Vec::new();
            self.interpolate(None, None, 1, &mut ring);
            out.push(ring);
        }
        out.into_iter().map(|r| r.into_iter().map(|p| (p.x, p.y)).collect()).collect()
    }
}

/// Order of intersection points along the clip edge (shared by both spherical clips).
fn compare_intersection(a: &SPoint, b: &SPoint) -> Ordering {
    let key = |p: &SPoint| if p.x < 0.0 { p.y - HALF_PI - EPS } else { HALF_PI - p.y };
    datars_math::total_cmp(key(a), key(b))
}

// ---- antimeridian ----------------------------------------------------------------------------

fn cut_antimeridian(pts: &[(f64, f64)]) -> Cut {
    let mut segs: Vec<Vec<SPoint>> = vec![Vec::new()];
    let (mut l0, mut p0, mut s0) = (f64::NAN, f64::NAN, f64::NAN);
    let mut clean = 1u8;
    for &(l, p1) in pts {
        let mut l1 = l;
        let s1 = if l1 > 0.0 { PI } else { -PI };
        let delta = (l1 - l0).abs();
        if (delta - PI).abs() < EPS {
            // The line crosses a pole: run along the pole to the other side.
            p0 = if (p0 + p1) / 2.0 > 0.0 { HALF_PI } else { -HALF_PI };
            let cur = segs.last_mut().expect("segment");
            cur.push(SPoint::new(l0, p0));
            cur.push(SPoint::new(s0, p0));
            segs.push(vec![SPoint::new(s1, p0), SPoint::new(l1, p0)]);
            clean = 0;
        } else if s0 != s1 && delta >= PI {
            // The line crosses the antimeridian.
            if (l0 - s0).abs() < EPS {
                l0 -= s0 * EPS;
            }
            if (l1 - s1).abs() < EPS {
                l1 -= s1 * EPS;
            }
            p0 = antimeridian_intersect(l0, p0, l1, p1);
            segs.last_mut().expect("segment").push(SPoint::new(s0, p0));
            segs.push(vec![SPoint::new(s1, p0)]);
            clean = 0;
        }
        l0 = l1;
        p0 = p1;
        segs.last_mut().expect("segment").push(SPoint::new(l0, p0));
        s0 = s1;
    }
    Cut { segments: segs, clean: 2 - clean }
}

/// Latitude where the great circle through two points crosses the antimeridian.
fn antimeridian_intersect(l0: f64, p0: f64, l1: f64, p1: f64) -> f64 {
    let s = m::sin(l0 - l1);
    if s.abs() > EPS {
        let (cp0, cp1) = (m::cos(p0), m::cos(p1));
        m::atan((m::sin(p0) * cp1 * m::sin(l1) - m::sin(p1) * cp0 * m::sin(l0)) / (cp0 * cp1 * s))
    } else {
        (p0 + p1) / 2.0
    }
}

fn interpolate_antimeridian(from: Option<SPoint>, to: Option<SPoint>, direction: i32, out: &mut Vec<SPoint>) {
    let d = direction as f64;
    match (from, to) {
        (Some(f), Some(t)) => {
            if (f.x - t.x).abs() > EPS {
                let l = if f.x < t.x { PI } else { -PI };
                let phi = d * l / 2.0;
                out.extend([SPoint::new(-l, phi), SPoint::new(0.0, phi), SPoint::new(l, phi)]);
            } else {
                out.push(SPoint::new(t.x, t.y));
            }
        }
        _ => {
            let phi = d * HALF_PI;
            out.extend([
                SPoint::new(-PI, phi),
                SPoint::new(0.0, phi),
                SPoint::new(PI, phi),
                SPoint::new(PI, 0.0),
                SPoint::new(PI, -phi),
                SPoint::new(0.0, -phi),
                SPoint::new(-PI, -phi),
                SPoint::new(-PI, 0.0),
                SPoint::new(-PI, phi),
            ]);
        }
    }
}

// ---- small circle ----------------------------------------------------------------------------

struct CircleClip {
    radius: f64,
    cr: f64,
    small: bool,
    not_hemisphere: bool,
}

enum Hit {
    None,
    One(SPoint),
    Two(SPoint, SPoint),
}

impl CircleClip {
    fn new(radius: f64) -> CircleClip {
        let cr = m::cos(radius);
        CircleClip { radius, cr, small: cr > 0.0, not_hemisphere: cr.abs() > EPS }
    }

    fn visible(&self, l: f64, p: f64) -> bool {
        m::cos(l) * m::cos(p) > self.cr
    }

    /// Where a point lies relative to the small circle's bounding box (4 bits).
    fn code(&self, l: f64, p: f64) -> u8 {
        let r = if self.small { self.radius } else { PI - self.radius };
        let mut c = 0;
        if l < -r {
            c |= 1;
        } else if l > r {
            c |= 2;
        }
        if p < -r {
            c |= 4;
        } else if p > r {
            c |= 8;
        }
        c
    }

    /// Intersect the great circle through a and b with the clip circle.
    fn intersect(&self, a: SPoint, b: SPoint, two: bool) -> Hit {
        let pa = cartesian(a.x, a.y);
        let pb = cartesian(b.x, b.y);
        let n1 = [1.0, 0.0, 0.0];
        let n2 = cross(pa, pb);
        let n2n2 = dot(n2, n2);
        let n1n2 = n2[0];
        let det = n2n2 - n1n2 * n1n2;
        if det == 0.0 {
            return if two { Hit::None } else { Hit::One(a) };
        }
        let c1 = self.cr * n2n2 / det;
        let c2 = -self.cr * n1n2 / det;
        let n1xn2 = cross(n1, n2);
        let aa = sphere::add(sphere::scale(n1, c1), sphere::scale(n2, c2));
        let u = n1xn2;
        let w = dot(aa, u);
        let uu = dot(u, u);
        let t2 = w * w - uu * (dot(aa, aa) - 1.0);
        if t2 < 0.0 {
            return Hit::None;
        }
        let t = t2.sqrt();
        let q = spherical(sphere::add(sphere::scale(u, (-w - t) / uu), aa));
        let q = SPoint::new(q.0, q.1);
        if !two {
            return Hit::One(q);
        }
        let (mut l0, mut l1, mut p0, mut p1) = (a.x, b.x, a.y, b.y);
        if l1 < l0 {
            std::mem::swap(&mut l0, &mut l1);
        }
        let delta = l1 - l0;
        let polar = (delta - PI).abs() < EPS;
        let meridian = polar || delta < EPS;
        if !polar && p1 < p0 {
            std::mem::swap(&mut p0, &mut p1);
        }
        let between = if meridian {
            if polar {
                ((p0 + p1) > 0.0) ^ (q.y < if (q.x - l0).abs() < EPS { p0 } else { p1 })
            } else {
                p0 <= q.y && q.y <= p1
            }
        } else {
            (delta > PI) ^ (l0 <= q.x && q.x <= l1)
        };
        if between {
            let q1 = spherical(sphere::add(sphere::scale(u, (-w + t) / uu), aa));
            Hit::Two(q, SPoint::new(q1.0, q1.1))
        } else {
            Hit::None
        }
    }

    fn one(&self, a: SPoint, b: SPoint) -> Option<SPoint> {
        match self.intersect(a, b, false) {
            Hit::One(p) => Some(p),
            _ => None,
        }
    }

    fn cut(&self, pts: &[(f64, f64)]) -> Cut {
        let mut segs: Vec<Vec<SPoint>> = Vec::new();
        let mut point0: Option<SPoint> = None;
        let (mut c0, mut v0, mut v00) = (0u8, false, false);
        let mut clean = 1u8;
        for &(l, p) in pts {
            let mut point1 = SPoint::new(l, p);
            let v = self.visible(l, p);
            let c = if self.small {
                if v { 0 } else { self.code(l, p) }
            } else if v {
                self.code(l + if l < 0.0 { PI } else { -PI }, p)
            } else {
                0
            };
            if point0.is_none() {
                v00 = v;
                v0 = v;
                if v {
                    segs.push(Vec::new());
                }
            }
            if v != v0 {
                if let Some(p0) = point0 {
                    match self.one(p0, point1) {
                        Some(p2) if !point_equal(p0, p2, EPS) && !point_equal(point1, p2, EPS) => {}
                        _ => point1.m = 1,
                    }
                }
            }
            if v != v0 {
                clean = 0;
                if let Some(p0) = point0 {
                    if v {
                        // outside going in
                        let p2 = self.one(point1, p0).unwrap_or(point1);
                        segs.push(vec![p2]);
                        point0 = Some(p2);
                    } else {
                        // inside going out
                        let p2 = self.one(p0, point1).unwrap_or(p0);
                        if let Some(s) = segs.last_mut() {
                            s.push(SPoint::flagged(p2.x, p2.y, 2));
                        }
                        point0 = Some(p2);
                    }
                }
            } else if self.not_hemisphere && point0.is_some() && (self.small ^ v) {
                // Both ends on the same side, but the segment may still dip across the circle.
                let p0 = point0.expect("checked");
                if c & c0 == 0 {
                    if let Hit::Two(t0, t1) = self.intersect(point1, p0, true) {
                        clean = 0;
                        if self.small {
                            segs.push(vec![t0, t1]);
                        } else {
                            if let Some(s) = segs.last_mut() {
                                s.push(t1);
                            }
                            segs.push(vec![SPoint::flagged(t0.x, t0.y, 3)]);
                        }
                    }
                }
            }
            if v && point0.is_none_or(|p0| !point_equal(p0, point1, EPS)) {
                if segs.is_empty() {
                    segs.push(Vec::new());
                }
                segs.last_mut().expect("segment").push(point1);
            }
            point0 = Some(point1);
            v0 = v;
            c0 = c;
        }
        Cut { segments: segs, clean: clean | (u8::from(v00 && v0) << 1) }
    }
}

/// Points along the clip circle (centred on (0, 0) in rotated space) from one edge point to
/// another, stepping `delta` radians; the whole circle when both are `None`.
fn circle_stream(radius: f64, delta: f64, direction: i32, from: Option<SPoint>, to: Option<SPoint>, out: &mut Vec<SPoint>) {
    let (cr, sr) = (m::cos(radius), m::sin(radius));
    let d = direction as f64;
    let step = d * delta;
    let (t0, t1) = match (from, to) {
        (Some(f), Some(t)) => {
            let mut t0 = circle_radius(cr, f);
            let t1 = circle_radius(cr, t);
            if if direction > 0 { t0 < t1 } else { t0 > t1 } {
                t0 += d * TAU;
            }
            (t0, t1)
        }
        _ => (radius + d * TAU, radius - step / 2.0),
    };
    let mut t = t0;
    let mut guard = 0;
    while (if direction > 0 { t > t1 } else { t < t1 }) && guard < 10_000 {
        let (l, p) = spherical([cr, -sr * m::cos(t), -sr * m::sin(t)]);
        out.push(SPoint::new(l, p));
        t -= step;
        guard += 1;
    }
}

/// The signed angle of a point on the circle relative to [cos r, 0, 0].
fn circle_radius(cr: f64, p: SPoint) -> f64 {
    let mut c = cartesian(p.x, p.y);
    c[0] -= cr;
    let c = normalize(c);
    let r = sphere::acos(-c[1]);
    m::fmod((if -c[2] < 0.0 { -r } else { r }) + TAU - EPS, TAU)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
        pts.iter().map(|&(l, p)| (l * RAD, p * RAD)).collect()
    }

    #[test]
    fn antimeridian_splits_lines() {
        let segs = SphereClip::Antimeridian.clip_line(&deg(&[(170.0, 10.0), (-170.0, 20.0)]));
        assert_eq!(segs.len(), 2);
        assert!((segs[0].last().unwrap().0 - PI).abs() < 1e-12);
        assert!((segs[1][0].0 + PI).abs() < 1e-12);
        // Same crossing latitude on both sides, between the endpoints.
        let lat = segs[0].last().unwrap().1 / RAD;
        assert!((segs[1][0].1 / RAD - lat).abs() < 1e-12 && lat > 10.0 && lat < 20.0, "{lat}");
        // A line that doesn't cross stays whole.
        assert_eq!(SphereClip::Antimeridian.clip_line(&deg(&[(10.0, 0.0), (20.0, 0.0)])).len(), 1);
    }

    #[test]
    fn antimeridian_cuts_polygons_into_two_rings() {
        let poly = crate::sphere::rewind_polygon(&[deg(&[(170.0, -10.0), (-170.0, -10.0), (-170.0, 10.0), (170.0, 10.0)])]);
        let rings = SphereClip::Antimeridian.clip_polygon(&poly);
        assert_eq!(rings.len(), 2, "{rings:?}");
        for r in &rings {
            let east = r.iter().all(|p| p.0 >= 170.0 * RAD - 1e-9);
            let west = r.iter().all(|p| p.0 <= -170.0 * RAD + 1e-9);
            assert!(east || west, "each ring stays on one side");
        }
    }

    #[test]
    fn circle_clips_back_hemisphere() {
        let clip = SphereClip::Circle { radius: 90.0 * RAD };
        // Entirely on the back: nothing.
        let back = crate::sphere::rewind_polygon(&[deg(&[(170.0, -10.0), (-170.0, -10.0), (-170.0, 10.0), (170.0, 10.0)])]);
        assert!(clip.clip_polygon(&back).is_empty());
        // Straddling the horizon at λ = 90°: kept part stays within 90° of the centre.
        let straddle = crate::sphere::rewind_polygon(&[deg(&[(60.0, -20.0), (120.0, -20.0), (120.0, 20.0), (60.0, 20.0)])]);
        let rings = clip.clip_polygon(&straddle);
        assert_eq!(rings.len(), 1);
        for &(l, p) in &rings[0] {
            assert!(m::cos(l) * m::cos(p) >= -1e-9, "visible or on the horizon");
        }
        // A line from front to back is cut at the horizon.
        let segs = clip.clip_line(&deg(&[(0.0, 0.0), (180.0, 0.0)]));
        assert_eq!(segs.len(), 1);
        let end = segs[0].last().unwrap();
        assert!((m::cos(end.0) * m::cos(end.1)).abs() < 1e-9, "ends on the horizon");
    }
}
