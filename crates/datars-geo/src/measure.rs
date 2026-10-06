//! Spherical measures on lon/lat geometry (ported from d3-geo): area, centroid, bounds (with
//! antimeridian handling), containment, distances and great-circle interpolation. Edges are
//! great-circle arcs, consistent with the projection pipeline. Winding is normalized first, so
//! both RFC 7946 and d3-style rings work.

use crate::geometry::{GeoBbox, Geometry, Polygon};
use crate::sphere::{self, cartesian, cross, normalize, spherical, EPS, EPS2, RAD, TAU};
use datars_math::{m, Vec2};

/// Mean Earth radius (IUGG), metres.
pub const EARTH_RADIUS_M: f64 = 6_371_008.8;

fn polygons_radians(g: &Geometry) -> Vec<Vec<Vec<(f64, f64)>>> {
    g.polygons().into_iter().map(sphere::polygon_radians).filter(|p| !p.is_empty()).collect()
}

/// Spherical area in steradians (4π = the whole sphere). Holes subtract. Points and lines: 0.
pub fn area(g: &Geometry) -> f64 {
    polygons_radians(g)
        .iter()
        .map(|poly| {
            let s: f64 = poly.iter().map(|r| sphere::ring_sum(r)).sum();
            2.0 * if s < 0.0 { TAU + s } else { s }
        })
        .sum()
}

/// Area in square metres on the mean-radius sphere.
pub fn area_m2(g: &Geometry) -> f64 {
    area(g) * EARTH_RADIUS_M * EARTH_RADIUS_M
}

/// Great-circle angle between two lon/lat points, radians (haversine).
pub fn angular_distance(a: Vec2, b: Vec2) -> f64 {
    let (l0, p0, l1, p1) = (a.x * RAD, a.y * RAD, b.x * RAD, b.y * RAD);
    let hs = |x: f64| {
        let s = m::sin(x / 2.0);
        s * s
    };
    2.0 * sphere::asin((hs(p1 - p0) + m::cos(p0) * m::cos(p1) * hs(l1 - l0)).sqrt())
}

/// Great-circle distance in metres (haversine on the mean-radius sphere).
pub fn distance(a: Vec2, b: Vec2) -> f64 {
    angular_distance(a, b) * EARTH_RADIUS_M
}

/// Total great-circle length (lines, and polygon perimeters) in radians.
pub fn length(g: &Geometry) -> f64 {
    let run = |pts: &[Vec2]| pts.windows(2).map(|w| angular_distance(w[0], w[1])).sum::<f64>();
    let lines: f64 = g.lines().iter().map(|l| run(l)).sum();
    let rings: f64 = g
        .polygons()
        .iter()
        .flat_map(|p| p.iter())
        .map(|r| {
            let closing = match (r.first(), r.last()) {
                (Some(a), Some(b)) if a != b => angular_distance(*b, *a),
                _ => 0.0,
            };
            run(r) + closing
        })
        .sum();
    lines + rings
}

/// A point `t` (0..1) of the way along the great circle from `a` to `b`.
pub fn interpolate(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    Geodesic::new(a, b).at(t)
}

/// `n` segments (`n + 1` points) along the great circle from `a` to `b` — for routes and flight
/// arcs. Longitudes stay in [-180, 180]; the projection pipeline cuts at the antimeridian.
pub fn geodesic(a: Vec2, b: Vec2, n: usize) -> Vec<Vec2> {
    let g = Geodesic::new(a, b);
    let n = n.max(1);
    (0..=n).map(|i| if i == n { b } else { g.at(i as f64 / n as f64) }).collect()
}

struct Geodesic {
    a: Vec2,
    d: f64,
    k: f64,
    kx0: f64,
    ky0: f64,
    sy0: f64,
    kx1: f64,
    ky1: f64,
    sy1: f64,
}

impl Geodesic {
    fn new(a: Vec2, b: Vec2) -> Geodesic {
        let (x0, y0, x1, y1) = (a.x * RAD, a.y * RAD, b.x * RAD, b.y * RAD);
        let (cy0, sy0, cy1, sy1) = (m::cos(y0), m::sin(y0), m::cos(y1), m::sin(y1));
        let d = angular_distance(a, b);
        Geodesic {
            a,
            d,
            k: m::sin(d),
            kx0: cy0 * m::cos(x0),
            ky0: cy0 * m::sin(x0),
            sy0,
            kx1: cy1 * m::cos(x1),
            ky1: cy1 * m::sin(x1),
            sy1,
        }
    }

    fn at(&self, t: f64) -> Vec2 {
        if self.d == 0.0 || self.k == 0.0 {
            return self.a;
        }
        let t = t * self.d;
        let (bb, aa) = (m::sin(t) / self.k, m::sin(self.d - t) / self.k);
        let x = aa * self.kx0 + bb * self.kx1;
        let y = aa * self.ky0 + bb * self.ky1;
        let z = aa * self.sy0 + bb * self.sy1;
        Vec2::new(m::atan2(y, x) / RAD, m::atan2(z, (x * x + y * y).sqrt()) / RAD)
    }
}

/// Spherical point-in-geometry for polygons (holes excluded). Lines and points contain nothing.
pub fn contains(g: &Geometry, lonlat: Vec2) -> bool {
    let p = (lonlat.x * RAD, lonlat.y * RAD);
    polygons_radians(g).iter().any(|poly| sphere::polygon_contains(poly, p))
}

/// Point in one lon/lat polygon (spherical).
pub fn polygon_contains(poly: &Polygon, lonlat: Vec2) -> bool {
    let rings = sphere::polygon_radians(poly);
    !rings.is_empty() && sphere::polygon_contains(&rings, (lonlat.x * RAD, lonlat.y * RAD))
}

/// The spherical centroid (d3 `geoCentroid`): area-weighted for polygons, falling back to
/// length-weighted for lines and the mean of points. `None` for empty or degenerate input.
pub fn centroid(g: &Geometry) -> Option<Vec2> {
    let mut acc = Centroid::default();
    for p in g.points() {
        acc.point(cartesian(p.x * RAD, p.y * RAD));
    }
    for l in g.lines() {
        let pts: Vec<[f64; 3]> = l.iter().map(|p| cartesian(p.x * RAD, p.y * RAD)).collect();
        acc.line(&pts);
    }
    for poly in polygons_radians(g) {
        for r in poly {
            let pts: Vec<[f64; 3]> = r.iter().map(|&(l, p)| cartesian(l, p)).collect();
            acc.ring(&pts);
        }
    }
    acc.result()
}

#[derive(Default)]
struct Centroid {
    w0: f64,
    x0: [f64; 3],
    w1: f64,
    x1: [f64; 3],
    x2: [f64; 3],
}

impl Centroid {
    fn point(&mut self, c: [f64; 3]) {
        self.w0 += 1.0;
        for (x, c) in self.x0.iter_mut().zip(c) {
            *x += (c - *x) / self.w0;
        }
    }

    fn line(&mut self, pts: &[[f64; 3]]) {
        let Some(&first) = pts.first() else { return };
        self.point(first);
        let mut p0 = first;
        for &p in &pts[1..] {
            let c = cross(p0, p);
            let w = m::atan2(sphere::dot(c, c).sqrt(), sphere::dot(p0, p));
            self.w1 += w;
            for i in 0..3 {
                self.x1[i] += w * (p0[i] + p[i]);
            }
            self.point(p);
            p0 = p;
        }
    }

    fn ring(&mut self, pts: &[[f64; 3]]) {
        let Some(&first) = pts.first() else { return };
        self.point(first);
        let mut p0 = first;
        for &p in pts[1..].iter().chain(std::iter::once(&first)) {
            let c = cross(p0, p);
            let mm = sphere::dot(c, c).sqrt();
            let w = sphere::asin(mm);
            let v = if mm != 0.0 { -w / mm } else { 0.0 };
            for i in 0..3 {
                self.x2[i] += v * c[i];
                self.x1[i] += w * (p0[i] + p[i]);
            }
            self.w1 += w;
            self.point(p);
            p0 = p;
        }
    }

    fn result(&self) -> Option<Vec2> {
        let len = |v: [f64; 3]| sphere::dot(v, v).sqrt();
        let mut v = self.x2;
        if len(v) < EPS2 {
            v = if self.w1 < EPS { self.x0 } else { self.x1 };
            if len(v) < EPS2 {
                return None;
            }
        }
        let mm = len(v);
        Some(Vec2::new(m::atan2(v[1], v[0]) / RAD, sphere::asin(v[2] / mm) / RAD))
    }
}

/// Geographic bounds (d3 `geoBounds`): the smallest longitude interval containing the geometry,
/// so a feature spanning the antimeridian gets `west > east` instead of a world-wide box.
/// Latitude bounds include great-circle bulges between vertices; polygons containing a pole
/// reach ±90°. `None` for empty geometry.
pub fn bbox(g: &Geometry) -> Option<GeoBbox> {
    let mut b = Bounds::new();
    for p in g.points() {
        b.point(p.x, p.y);
    }
    for l in g.lines() {
        b.line(l.iter().map(|p| (p.x, p.y)));
    }
    for p in g.polygons() {
        b.polygon(&sphere::polygon_oriented(p));
    }
    b.result()
}

/// Longitude distance going east from `a` to `b` in degrees, with ±180° apart counting as 360.
fn angle(a: f64, b: f64) -> f64 {
    let d = b - a;
    if d < 0.0 { d + 360.0 } else { d }
}

fn range_contains(r: [f64; 2], x: f64) -> bool {
    if r[0] <= r[1] { r[0] <= x && x <= r[1] } else { x < r[0] || r[1] < x }
}

struct Bounds {
    l0: f64,
    p0: f64,
    l1: f64,
    p1: f64,
    ranges: Vec<[f64; 2]>,
    prev: Option<([f64; 3], f64)>,
}

impl Bounds {
    fn new() -> Bounds {
        Bounds { l0: f64::INFINITY, p0: f64::INFINITY, l1: f64::NEG_INFINITY, p1: f64::NEG_INFINITY, ranges: Vec::new(), prev: None }
    }

    fn lat(&mut self, phi: f64) {
        self.p0 = self.p0.min(phi);
        self.p1 = self.p1.max(phi);
    }

    fn point(&mut self, lambda: f64, phi: f64) {
        self.l0 = lambda;
        self.l1 = lambda;
        self.ranges.push([lambda, lambda]);
        self.lat(phi);
    }

    /// One vertex of a line or ring (degrees), extending the current range.
    fn line_point(&mut self, lambda: f64, phi: f64) {
        let p = cartesian(lambda * RAD, phi * RAD);
        match self.prev {
            Some((p0, lambda2)) => {
                let normal = cross(p0, p);
                let equatorial = [normal[1], -normal[0], 0.0];
                let inflection = spherical(normalize(cross(equatorial, normal)));
                let delta = lambda - lambda2;
                let sign = if delta > 0.0 { 1.0 } else { -1.0 };
                let mut lambdai = inflection.0 / RAD * sign;
                let antimeridian = delta.abs() > 180.0;
                if antimeridian ^ (sign * lambda2 < lambdai && lambdai < sign * lambda) {
                    self.p1 = self.p1.max(inflection.1 / RAD);
                } else {
                    lambdai = m::fmod(lambdai + 360.0, 360.0) - 180.0;
                    if antimeridian ^ (sign * lambda2 < lambdai && lambdai < sign * lambda) {
                        self.p0 = self.p0.min(-inflection.1 / RAD);
                    } else {
                        self.lat(phi);
                    }
                }
                if antimeridian {
                    if lambda < lambda2 {
                        if angle(self.l0, lambda) > angle(self.l0, self.l1) {
                            self.l1 = lambda;
                        }
                    } else if angle(lambda, self.l1) > angle(self.l0, self.l1) {
                        self.l0 = lambda;
                    }
                } else if self.l1 >= self.l0 {
                    self.l0 = self.l0.min(lambda);
                    self.l1 = self.l1.max(lambda);
                } else if lambda > lambda2 {
                    if angle(self.l0, lambda) > angle(self.l0, self.l1) {
                        self.l1 = lambda;
                    }
                } else if angle(lambda, self.l1) > angle(self.l0, self.l1) {
                    self.l0 = lambda;
                }
            }
            None => {
                self.l0 = lambda;
                self.l1 = lambda;
                self.ranges.push([lambda, lambda]);
            }
        }
        self.lat(phi);
        self.prev = Some((p, lambda));
    }

    fn close_range(&mut self) {
        if let Some(r) = self.ranges.last_mut() {
            *r = [self.l0, self.l1];
        }
        self.prev = None;
    }

    fn line(&mut self, pts: impl Iterator<Item = (f64, f64)>) {
        for (l, p) in pts {
            self.line_point(l, p);
        }
        self.close_range();
    }

    /// A polygon given as rewound rings, in degrees and radians (no closing duplicates). As in d3, the
    /// longitude winding (`delta_sum`, with d3's deliberate ±360 on antimeridian jumps) and the
    /// area sum accumulate over all rings: a ring winding around a pole spans every longitude and
    /// reaches that pole; a polygon larger than a hemisphere is the whole globe.
    fn polygon(&mut self, poly: &[sphere::OrientedRing]) {
        let mut sum = 0.0;
        let mut delta_sum = 0.0;
        for (deg, ring) in poly {
            let degs: Vec<(f64, f64)> = deg.iter().map(|p| (p.x, p.y)).collect();
            let mut prev_l: Option<f64> = None;
            for &(l, p) in degs.iter().chain(degs.first()) {
                if let Some(l2) = prev_l {
                    let d = l - l2;
                    delta_sum += if d.abs() > 180.0 { d + if d > 0.0 { 360.0 } else { -360.0 } } else { d };
                }
                prev_l = Some(l);
                self.line_point(l, p);
            }
            if delta_sum.abs() > EPS {
                self.l0 = -180.0;
                self.l1 = 180.0;
            }
            self.close_range();
            sum += sphere::ring_sum(ring);
        }
        if sum < 0.0 {
            self.l0 = -180.0;
            self.l1 = 180.0;
            self.p0 = -90.0;
            self.p1 = 90.0;
        } else if delta_sum > EPS {
            self.p1 = 90.0;
        } else if delta_sum < -EPS {
            self.p0 = -90.0;
        }
        if let Some(r) = self.ranges.last_mut() {
            *r = [self.l0, self.l1];
        }
    }

    fn result(mut self) -> Option<GeoBbox> {
        if self.ranges.is_empty() || !self.p0.is_finite() {
            return None;
        }
        self.ranges.sort_by(|a, b| datars_math::total_cmp(a[0], b[0]));
        let mut merged: Vec<[f64; 2]> = vec![self.ranges[0]];
        for &b in &self.ranges[1..] {
            let a = merged.last_mut().expect("nonempty");
            if range_contains(*a, b[0]) || range_contains(*a, b[1]) {
                if angle(a[0], b[1]) > angle(a[0], a[1]) {
                    a[1] = b[1];
                }
                if angle(b[0], a[1]) > angle(a[0], a[1]) {
                    a[0] = b[0];
                }
            } else {
                merged.push(b);
            }
        }
        // The box is the complement of the largest gap between merged ranges.
        let (mut best, mut west, mut east) = (f64::NEG_INFINITY, self.l0, self.l1);
        let n = merged.len();
        let mut a = merged[n - 1];
        for &b in &merged {
            let d = angle(a[1], b[0]);
            if d > best {
                best = d;
                west = b[0];
                east = a[1];
            }
            a = b;
        }
        Some(GeoBbox::new(west, self.p0, east, self.p1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vec2 {
        Vec2::new(x, y)
    }

    #[test]
    fn haversine_known_values() {
        // One degree of latitude ≈ 111.195 km on the mean sphere.
        assert!((distance(v(0.0, 0.0), v(0.0, 1.0)) - 111_195.08).abs() < 0.1);
        // Stockholm–Gothenburg ≈ 398 km.
        let d = distance(v(18.0686, 59.3293), v(11.9746, 57.7089));
        assert!((d - 398_000.0).abs() < 3_000.0, "{d}");
        assert!((angular_distance(v(0.0, 0.0), v(180.0, 0.0)) - m::PI).abs() < 1e-12);
    }

    #[test]
    fn geodesic_interpolation() {
        let pts = geodesic(v(0.0, 0.0), v(90.0, 0.0), 3);
        assert_eq!(pts.len(), 4);
        assert!((pts[1] - v(30.0, 0.0)).len() < 1e-9);
        // London → New York passes north of both.
        let route = geodesic(v(-0.1, 51.5), v(-74.0, 40.7), 10);
        assert!(route[5].y > 51.5);
        assert_eq!(interpolate(v(1.0, 2.0), v(1.0, 2.0), 0.5), v(1.0, 2.0));
    }
}
