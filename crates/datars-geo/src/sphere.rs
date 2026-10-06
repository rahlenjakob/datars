//! Spherical helpers shared by projections, clipping and measures (ported from d3-geo): cartesian
//! conversion, rotations, the spherical winding/area sum and point-in-polygon on the sphere.
//! Angles are radians unless a name says degrees.

use crate::geometry::Polygon;
use datars_math::{m, Vec2};

pub(crate) const EPS: f64 = 1e-6;
pub(crate) const EPS2: f64 = 1e-12;
pub(crate) const PI: f64 = m::PI;
pub(crate) const HALF_PI: f64 = m::PI / 2.0;
pub(crate) const QUARTER_PI: f64 = m::PI / 4.0;
pub(crate) const TAU: f64 = m::TAU;
pub(crate) const RAD: f64 = m::PI / 180.0;

/// asin/acos clamped to their domain, so rounding just past ±1 can't produce NaN.
#[inline]
pub(crate) fn asin(x: f64) -> f64 {
    if x > 1.0 {
        HALF_PI
    } else if x < -1.0 {
        -HALF_PI
    } else {
        m::asin(x)
    }
}

#[inline]
pub(crate) fn acos(x: f64) -> f64 {
    if x > 1.0 {
        0.0
    } else if x < -1.0 {
        PI
    } else {
        m::acos(x)
    }
}

pub(crate) type V3 = [f64; 3];

#[inline]
pub(crate) fn cartesian(l: f64, p: f64) -> V3 {
    let cp = m::cos(p);
    [cp * m::cos(l), cp * m::sin(l), m::sin(p)]
}

#[inline]
pub(crate) fn spherical(c: V3) -> (f64, f64) {
    (m::atan2(c[1], c[0]), asin(c[2]))
}

#[inline]
pub(crate) fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
pub(crate) fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[inline]
pub(crate) fn scale(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

#[inline]
pub(crate) fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub(crate) fn normalize(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        scale(a, 1.0 / l)
    } else {
        a
    }
}

/// Wrap a longitude into [-π, π].
#[inline]
pub(crate) fn wrap_lambda(l: f64) -> f64 {
    if l.abs() > PI {
        l - (l / TAU).round() * TAU
    } else {
        l
    }
}

/// A rotation of the sphere by (λ, φ, γ) — d3's `projection.rotate`: first around the polar axis
/// by λ, then around the y axis by φ and the x axis by γ.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rotation {
    dl: f64,
    pg: Option<[f64; 4]>, // cos φ, sin φ, cos γ, sin γ
}

impl Rotation {
    pub fn new(deg: [f64; 3]) -> Rotation {
        let dl = m::fmod(deg[0] * RAD, TAU);
        let (dp, dg) = (deg[1] * RAD, deg[2] * RAD);
        let pg = (dp != 0.0 || dg != 0.0).then(|| [m::cos(dp), m::sin(dp), m::cos(dg), m::sin(dg)]);
        Rotation { dl, pg }
    }

    pub fn forward(&self, l: f64, p: f64) -> (f64, f64) {
        let l = wrap_lambda(l + self.dl);
        match self.pg {
            None => (l, p),
            Some([cdp, sdp, cdg, sdg]) => {
                let cp = m::cos(p);
                let (x, y, z) = (m::cos(l) * cp, m::sin(l) * cp, m::sin(p));
                let k = z * cdp + x * sdp;
                (m::atan2(y * cdg - k * sdg, x * cdp - z * sdp), asin(k * cdg + y * sdg))
            }
        }
    }

    pub fn inverse(&self, l: f64, p: f64) -> (f64, f64) {
        let (l, p) = match self.pg {
            None => (l, p),
            Some([cdp, sdp, cdg, sdg]) => {
                let cp = m::cos(p);
                let (x, y, z) = (m::cos(l) * cp, m::sin(l) * cp, m::sin(p));
                let k = z * cdg - y * sdg;
                (m::atan2(y * cdg + z * sdg, x * cdp + k * sdp), asin(k * cdp - x * sdp))
            }
        };
        (wrap_lambda(l - self.dl), p)
    }
}

/// d3's spherical ring sum (Cagnoli excess of the triangles south pole–p0–p1), closing edge
/// included. Twice this (plus 2τ if negative) is the area to the right of the ring's direction.
pub(crate) fn ring_sum(ring: &[(f64, f64)]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let (l00, p00) = ring[0];
    let mut l0 = l00;
    let ph0 = p00 / 2.0 + QUARTER_PI;
    let (mut cp0, mut sp0) = (m::cos(ph0), m::sin(ph0));
    let mut sum = 0.0;
    for &(l, p) in ring[1..].iter().chain(std::iter::once(&(l00, p00))) {
        let ph = p / 2.0 + QUARTER_PI;
        let dl = l - l0;
        let sd = if dl >= 0.0 { 1.0 } else { -1.0 };
        let ad = sd * dl;
        let (cp, sp) = (m::cos(ph), m::sin(ph));
        let k = sp0 * sp;
        let u = cp0 * cp + k * m::cos(ad);
        let v = k * sd * m::sin(ad);
        sum += m::atan2(v, u);
        l0 = l;
        cp0 = cp;
        sp0 = sp;
    }
    sum
}

/// Area (steradians) to the right of the ring's direction of travel — the ring's interior under
/// d3's convention (clockwise exteriors).
pub(crate) fn ring_area_right(ring: &[(f64, f64)]) -> f64 {
    let s = ring_sum(ring);
    2.0 * if s < 0.0 { TAU + s } else { s }
}

/// Whether a ring (radians, no closing duplicate) runs clockwise as the user sees it.
///
/// A ring that stays on one side of the antimeridian means what it looks like on a lon/lat plot,
/// so its planar orientation decides — this keeps polygons larger than a hemisphere (a 240°-wide
/// box) and rings with explicit pole edges (Antarctica) right. Rings that jump across the
/// antimeridian, or are degenerate in the plane (a cap around a pole), can't be read that way;
/// for those the ring is taken to enclose the smaller side of the sphere.
fn ring_is_clockwise(ring: &[(f64, f64)]) -> bool {
    let n = ring.len();
    let at_pole = |p: f64| p.abs() >= HALF_PI - 1e-9;
    let mut jumps = false;
    let mut planar = 0.0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (b.0 - a.0).abs() > PI && !(at_pole(a.1) && at_pole(b.1)) {
            jumps = true;
            break;
        }
        planar += a.0 * b.1 - b.0 * a.1;
    }
    if !jumps && planar.abs() > EPS2 {
        return planar < 0.0; // y (latitude) up: clockwise is negative
    }
    ring_area_right(ring) <= TAU
}

/// `polygon_oriented` on radian rings (tests).
#[cfg(test)]
pub(crate) fn rewind_polygon(poly: &[Vec<(f64, f64)>]) -> Vec<Vec<(f64, f64)>> {
    poly.iter()
        .enumerate()
        .filter(|(_, r)| r.len() >= 3)
        .map(|(i, r)| {
            let mut r = r.clone();
            if ring_is_clockwise(&r) != (i == 0) {
                r.reverse();
            }
            r
        })
        .collect()
}

/// A polygon in degrees → rewound rings in radians (no closing duplicates).
pub(crate) fn polygon_radians(poly: &Polygon) -> Vec<Vec<(f64, f64)>> {
    polygon_oriented(poly).into_iter().map(|(_, r)| r).collect()
}

/// A ring in degrees and the same ring in radians.
pub(crate) type OrientedRing = (Vec<Vec2>, Vec<(f64, f64)>);

/// Orient a polygon's rings for spherical processing: exteriors clockwise (d3's convention),
/// holes the other way. Input winding is free, so RFC 7946 (counter-clockwise) and d3-style data
/// both work (see `ring_is_clockwise` for how "clockwise" is read on the sphere). Returns degree
/// and radian rings (no closing duplicates), so callers working in degrees don't round-trip.
pub(crate) fn polygon_oriented(poly: &Polygon) -> Vec<OrientedRing> {
    let open = |r: &Vec<Vec2>| {
        let mut r = r.clone();
        if r.len() > 1 && r.first() == r.last() {
            r.pop();
        }
        r
    };
    if poly.first().is_none_or(|r| open(r).len() < 3) {
        return Vec::new();
    }
    poly.iter()
        .map(open)
        .enumerate()
        .filter(|(_, r)| r.len() >= 3)
        .map(|(i, mut deg)| {
            let mut rad: Vec<(f64, f64)> = deg.iter().map(|p| (p.x * RAD, p.y * RAD)).collect();
            if ring_is_clockwise(&rad) != (i == 0) {
                rad.reverse();
                deg.reverse();
            }
            (deg, rad)
        })
        .collect()
}

fn longitude(l: f64) -> f64 {
    if l.abs() <= PI {
        l
    } else {
        l.signum() * (m::fmod(l.abs() + PI, TAU) - PI)
    }
}

/// Point in spherical polygon (d3 `polygonContains`): rings in radians under d3's winding
/// convention, no closing duplicates required.
pub(crate) fn polygon_contains(polygon: &[Vec<(f64, f64)>], point: (f64, f64)) -> bool {
    let lambda = longitude(point.0);
    let mut phi = point.1;
    let sin_phi = m::sin(phi);
    let normal = [m::sin(lambda), -m::cos(lambda), 0.0];
    let mut angle = 0.0;
    let mut winding = 0i32;
    let mut sum = 0.0;
    if sin_phi == 1.0 {
        phi = HALF_PI + EPS;
    } else if sin_phi == -1.0 {
        phi = -HALF_PI - EPS;
    }
    for ring in polygon {
        let n = ring.len();
        if n == 0 {
            continue;
        }
        let mut point0 = ring[n - 1];
        let mut lambda0 = longitude(point0.0);
        let p0 = point0.1 / 2.0 + QUARTER_PI;
        let (mut sin_phi0, mut cos_phi0) = (m::sin(p0), m::cos(p0));
        for &point1 in ring {
            let lambda1 = longitude(point1.0);
            let p1 = point1.1 / 2.0 + QUARTER_PI;
            let (sin_phi1, cos_phi1) = (m::sin(p1), m::cos(p1));
            let delta = lambda1 - lambda0;
            let sign = if delta >= 0.0 { 1.0 } else { -1.0 };
            let abs_delta = sign * delta;
            let antimeridian = abs_delta > PI;
            let k = sin_phi0 * sin_phi1;
            sum += m::atan2(k * sign * m::sin(abs_delta), cos_phi0 * cos_phi1 + k * m::cos(abs_delta));
            angle += if antimeridian { delta + sign * TAU } else { delta };
            if antimeridian ^ (lambda0 >= lambda) ^ (lambda1 >= lambda) {
                let arc = normalize(cross(cartesian(point0.0, point0.1), cartesian(point1.0, point1.1)));
                let inter = normalize(cross(normal, arc));
                let s = if antimeridian ^ (delta >= 0.0) { -1.0 } else { 1.0 };
                let phi_arc = s * asin(inter[2]);
                if phi > phi_arc || (phi == phi_arc && (arc[0] != 0.0 || arc[1] != 0.0)) {
                    winding += if antimeridian ^ (delta >= 0.0) { 1 } else { -1 };
                }
            }
            lambda0 = lambda1;
            sin_phi0 = sin_phi1;
            cos_phi0 = cos_phi1;
            point0 = point1;
        }
    }
    (angle < -EPS || (angle < EPS && sum < -EPS2)) ^ (winding & 1 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deg_ring(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
        pts.iter().map(|&(l, p)| (l * RAD, p * RAD)).collect()
    }

    #[test]
    fn rotation_round_trips() {
        for rot in [[0.0, 0.0, 0.0], [96.0, 0.0, 0.0], [-10.0, -50.0, 0.0], [30.0, 20.0, 45.0], [200.0, -95.0, 10.0]] {
            let r = Rotation::new(rot);
            for &(l, p) in &[(0.0, 0.0), (1.0, 0.5), (-3.0, -1.2), (2.5, 1.4)] {
                let (a, b) = r.forward(l, p);
                let (l2, p2) = r.inverse(a, b);
                assert!((wrap_lambda(l2 - l)).abs() < 1e-12 && (p2 - p).abs() < 1e-12, "{rot:?} {l},{p}");
            }
        }
    }

    #[test]
    fn rewinding_makes_small_clockwise_exteriors() {
        // Counter-clockwise (RFC 7946) square: reversed; clockwise: kept.
        let ccw = deg_ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let r = rewind_polygon(std::slice::from_ref(&ccw));
        let reversed: Vec<_> = ccw.iter().rev().copied().collect();
        assert_eq!(r[0], reversed);
        assert!(ring_area_right(&r[0]) < 0.1);
        let cw: Vec<_> = ccw.iter().rev().copied().collect();
        assert_eq!(rewind_polygon(std::slice::from_ref(&cw))[0], cw);
        // A 10°×10° box at the equator with great-circle edges: 0.0303822 sr (independent triangulation).
        assert!((ring_area_right(&cw) - 0.030_382_156_674_6).abs() < 1e-9, "{}", ring_area_right(&cw));
    }

    #[test]
    fn spherical_containment() {
        let cw = rewind_polygon(&[deg_ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])]);
        assert!(polygon_contains(&cw, (5.0 * RAD, 5.0 * RAD)));
        assert!(!polygon_contains(&cw, (15.0 * RAD, 5.0 * RAD)));
        assert!(!polygon_contains(&cw, (-175.0 * RAD, -5.0 * RAD)));
        // Across the antimeridian.
        let am = rewind_polygon(&[deg_ring(&[(170.0, -10.0), (-170.0, -10.0), (-170.0, 10.0), (170.0, 10.0)])]);
        assert!(polygon_contains(&am, (179.0 * RAD, 0.0)));
        assert!(polygon_contains(&am, (-179.0 * RAD, 0.0)));
        assert!(!polygon_contains(&am, (0.0, 0.0)));
    }
}
