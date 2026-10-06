//! View cameras. A camera is a (centre, log-scale) pair, so it interpolates the way people
//! perceive zoom; long moves follow the van Wijk–Nuij optimal path (zoom out, pan, zoom in) so a
//! world → street flight reads as one continuous move.

use datars_math::{lerp, m};
use datars_scene::Camera;

/// Curvature of the fly path (van Wijk & Nuij's recommended ρ ≈ 1.42).
const RHO: f64 = 1.42;

fn valid(c: &Camera) -> bool {
    c.zoom > 0.0 && c.zoom.is_finite() && c.x.is_finite() && c.y.is_finite() && c.rotation.is_finite()
}

/// Straight interpolation: centre linearly, zoom geometrically (constant perceived speed),
/// rotation linearly.
pub fn lerp_camera(a: &Camera, b: &Camera, t: f64) -> Camera {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    let zoom = if valid(a) && valid(b) { m::exp(lerp(m::ln(a.zoom), m::ln(b.zoom), t)) } else { lerp(a.zoom, b.zoom, t) };
    Camera { x: lerp(a.x, b.x, t), y: lerp(a.y, b.y, t), zoom, rotation: lerp(a.rotation, b.rotation, t) }
}

/// The camera at `t` along the smooth zoom-and-pan path from `a` to `b` for a viewport `vw` wide
/// (content units scale by `zoom`). Pure; exact at both ends.
pub fn fly(a: &Camera, b: &Camera, t: f64, vw: f64) -> Camera {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    if !(valid(a) && valid(b) && vw > 0.0 && vw.is_finite()) {
        return lerp_camera(a, b, t);
    }
    let rotation = lerp(a.rotation, b.rotation, t);
    // Visible widths in content units.
    let w0 = vw / a.zoom;
    let w1 = vw / b.zoom;
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let u1 = m::hypot(dx, dy);
    if u1 < 1e-12 * w0.max(w1) {
        // Pure zoom: geometric interpolation of the width.
        let w = m::exp(lerp(m::ln(w0), m::ln(w1), t));
        return Camera { x: a.x + dx * t, y: a.y + dy * t, zoom: vw / w, rotation };
    }
    let rho2 = RHO * RHO;
    let b_i = |wi: f64, sign: f64| (w1 * w1 - w0 * w0 + sign * rho2 * rho2 * u1 * u1) / (2.0 * wi * rho2 * u1);
    let r = |bb: f64| m::ln(-bb + (bb * bb + 1.0).sqrt());
    let r0 = r(b_i(w0, 1.0));
    let r1 = r(b_i(w1, -1.0));
    let s_total = (r1 - r0) / RHO;
    if !s_total.is_finite() || s_total.abs() < 1e-12 {
        return lerp_camera(a, b, t);
    }
    let s = t * s_total;
    let u = w0 / rho2 * m::cosh(r0) * m::tanh(RHO * s + r0) - w0 / rho2 * m::sinh(r0);
    let w = w0 * m::cosh(r0) / m::cosh(RHO * s + r0);
    let f = u / u1;
    let c = Camera { x: a.x + dx * f, y: a.y + dy * f, zoom: vw / w, rotation };
    if valid(&c) {
        c
    } else {
        lerp_camera(a, b, t)
    }
}

/// Whether a camera move is long enough (zoom changes by ≥ 2×, or the pan is at least a visible
/// width) that the van Wijk path reads better than a straight one.
pub fn wants_fly(a: &Camera, b: &Camera, vw: f64) -> bool {
    if !(valid(a) && valid(b) && vw > 0.0) {
        return false;
    }
    let ratio = (b.zoom / a.zoom).max(a.zoom / b.zoom);
    let pan = m::hypot(b.x - a.x, b.y - a.y);
    ratio >= 2.0 || pan >= (vw / a.zoom).max(vw / b.zoom)
}

/// Interpolate a view's camera: van Wijk for long moves, straight otherwise.
pub fn interpolate_camera(a: &Camera, b: &Camera, t: f64, vw: f64) -> Camera {
    if wants_fly(a, b, vw) {
        fly(a, b, t, vw)
    } else {
        lerp_camera(a, b, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fly_hits_endpoints_and_zooms_monotonically_in() {
        let world = Camera { x: 500.0, y: 500.0, zoom: 1.0, rotation: 0.0 };
        let city = Camera { x: 550.0, y: 290.0, zoom: 2048.0, rotation: 0.0 };
        assert_eq!(fly(&world, &city, 0.0, 800.0), world);
        assert_eq!(fly(&world, &city, 1.0, 800.0), city);
        let mut last = world.zoom;
        for i in 1..20 {
            let c = fly(&world, &city, i as f64 / 20.0, 800.0);
            assert!(c.zoom >= last * (1.0 - 1e-9), "monotone zoom-in at {i}");
            last = c.zoom;
        }
    }
}
