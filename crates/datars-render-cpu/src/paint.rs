//! Paints as per-pixel shaders producing premultiplied RGBA8 with the op's opacity folded in.
//!
//! Gradients are evaluated at pixel centres: the device point is mapped back to the paint's local
//! space (the op transform's inverse), `t` is computed with basic IEEE operations only, and the
//! colour comes from a 256-entry lookup table interpolated in premultiplied sRGB (so fading to a
//! transparent stop never darkens). Outside the stops, colours clamp (SVG `spreadMethod="pad"`).

use crate::pixel::{premul, unit};
use datars_color::Color;
use datars_math::{Affine, Vec2};
use datars_render::DPaint;

const LUT: usize = 256;

pub enum Shader {
    Solid([u8; 4]),
    Linear { lut: Box<[[u8; 4]; LUT]>, t0: f64, tx: f64, ty: f64 },
    Radial { lut: Box<[[u8; 4]; LUT]>, inv: Affine, c: Vec2, inv_r: f64 },
}

impl Shader {
    /// The shader for `paint` drawn through `xf` (local → device). `None` when it paints nothing
    /// (fully transparent, no stops, or a degenerate transform).
    pub fn new(paint: &DPaint, xf: &Affine, opacity: f32) -> Option<Shader> {
        let op = unit(opacity) as f32;
        let s = match paint {
            DPaint::Solid(c) => Shader::Solid(premul(*c, op)),
            DPaint::Linear { p0, p1, stops } => {
                let stops = sanitize(stops)?;
                let d = *p1 - *p0;
                let len2 = d.dot(d);
                let inv = xf.inverse()?;
                if stops.len() == 1 || !(len2 > 0.0 && len2.is_finite()) {
                    // SVG: a zero-length gradient vector paints with the last stop.
                    Shader::Solid(premul(stops[stops.len() - 1].1, op))
                } else {
                    // t(P) = ((inv·P − p0) · d) / |d|², affine in P.
                    let [ia, ib, ic, id, ie, if_] = inv.0;
                    Shader::Linear {
                        lut: lut(&stops, op),
                        tx: (ia * d.x + ib * d.y) / len2,
                        ty: (ic * d.x + id * d.y) / len2,
                        t0: ((ie - p0.x) * d.x + (if_ - p0.y) * d.y) / len2,
                    }
                }
            }
            DPaint::Radial { c, r, stops } => {
                let stops = sanitize(stops)?;
                let inv = xf.inverse()?;
                if stops.len() == 1 || !(*r > 0.0 && r.is_finite()) {
                    Shader::Solid(premul(stops[stops.len() - 1].1, op))
                } else {
                    Shader::Radial { lut: lut(&stops, op), inv, c: *c, inv_r: 1.0 / *r }
                }
            }
        };
        match &s {
            Shader::Solid(p) if p[3] == 0 => None,
            _ => Some(s),
        }
    }

    /// The colour at the centre of pixel (x, y).
    #[inline]
    pub fn at(&self, x: i32, y: i32) -> [u8; 4] {
        match self {
            Shader::Solid(p) => *p,
            Shader::Linear { lut, t0, tx, ty } => {
                let t = t0 + tx * (x as f64 + 0.5) + ty * (y as f64 + 0.5);
                lut[index(t)]
            }
            Shader::Radial { lut, inv, c, inv_r } => {
                let q = inv.apply(Vec2::new(x as f64 + 0.5, y as f64 + 0.5)) - *c;
                lut[index(q.len() * inv_r)]
            }
        }
    }
}

#[inline]
fn index(t: f64) -> usize {
    if t > 0.0 {
        if t < 1.0 {
            (t * (LUT - 1) as f64 + 0.5) as usize
        } else {
            LUT - 1
        }
    } else {
        0 // also NaN
    }
}

/// Stops with offsets clamped to [0, 1] and made non-decreasing (SVG rules). `None` if empty.
fn sanitize(stops: &[(f32, Color)]) -> Option<Vec<(f64, Color)>> {
    if stops.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(stops.len());
    let mut last = 0.0f64;
    for (t, c) in stops {
        let t = unit(*t).max(last);
        out.push((t, *c));
        last = t;
    }
    Some(out)
}

fn lut(stops: &[(f64, Color)], opacity: f32) -> Box<[[u8; 4]; LUT]> {
    // Premultiplied stop colours in f64.
    let pm: Vec<[f64; 4]> = stops
        .iter()
        .map(|(_, c)| {
            let a = unit(c.a) * opacity as f64;
            [unit(c.r) * a, unit(c.g) * a, unit(c.b) * a, a]
        })
        .collect();
    let q = |v: f64| (v * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
    let mut out = Box::new([[0u8; 4]; LUT]);
    let mut k = 0; // stops[k].0 <= t < stops[k + 1].0 once inside
    for (i, px) in out.iter_mut().enumerate() {
        let t = i as f64 / (LUT - 1) as f64;
        let col = if t <= stops[0].0 {
            pm[0]
        } else if t >= stops[stops.len() - 1].0 {
            pm[stops.len() - 1]
        } else {
            while k + 1 < stops.len() && stops[k + 1].0 <= t {
                k += 1;
            }
            let (t0, t1) = (stops[k].0, stops[k + 1].0);
            let f = (t - t0) / (t1 - t0); // t1 > t >= t0, so no division by zero
            let (a, b) = (pm[k], pm[k + 1]);
            [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f, a[3] + (b[3] - a[3]) * f]
        };
        let alpha = q(col[3]);
        *px = [q(col[0]).min(alpha), q(col[1]).min(alpha), q(col[2]).min(alpha), alpha];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_lut_endpoints_and_midpoint() {
        let p = DPaint::Linear {
            p0: Vec2::new(0.0, 0.0),
            p1: Vec2::new(100.0, 0.0),
            stops: vec![(0.0, Color::rgb8(0, 0, 0)), (1.0, Color::rgb8(255, 255, 255))],
        };
        let s = Shader::new(&p, &Affine::IDENTITY, 1.0).unwrap();
        assert_eq!(s.at(-5, 0), [0, 0, 0, 255]);
        assert_eq!(s.at(200, 0), [255, 255, 255, 255]);
        let mid = s.at(49, 7)[0]; // centre x = 49.5 → t = 0.495
        assert!((125..=128).contains(&mid), "{mid}");
    }

    #[test]
    fn hard_stops_switch_colour() {
        let red = Color::rgb8(255, 0, 0);
        let blue = Color::rgb8(0, 0, 255);
        let p = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(10.0, 0.0), stops: vec![(0.0, red), (0.5, red), (0.5, blue), (1.0, blue)] };
        let s = Shader::new(&p, &Affine::IDENTITY, 1.0).unwrap();
        assert_eq!(s.at(2, 0), [255, 0, 0, 255]);
        assert_eq!(s.at(7, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn transparent_or_empty_paints_nothing() {
        assert!(Shader::new(&DPaint::Solid(Color::TRANSPARENT), &Affine::IDENTITY, 1.0).is_none());
        assert!(Shader::new(&DPaint::Solid(Color::BLACK), &Affine::IDENTITY, 0.0).is_none());
        let p = DPaint::Radial { c: Vec2::ZERO, r: 5.0, stops: vec![] };
        assert!(Shader::new(&p, &Affine::IDENTITY, 1.0).is_none());
    }

    #[test]
    fn radial_follows_the_transform() {
        let p = DPaint::Radial { c: Vec2::new(0.0, 0.0), r: 10.0, stops: vec![(0.0, Color::WHITE), (1.0, Color::BLACK)] };
        // Local circle scaled ×2 and moved to (50, 50): radius 20 in device space.
        let xf = Affine::translate(50.0, 50.0).mul(Affine::scale(2.0, 2.0));
        let s = Shader::new(&p, &xf, 1.0).unwrap();
        assert_eq!(s.at(49, 49)[0], 246); // centre pixel: t ≈ 0.035 → LUT entry 9
        assert_eq!(s.at(80, 50), [0, 0, 0, 255]); // beyond r
    }
}
