//! Property interpolators. Every one returns its first endpoint exactly at progress 0 and its
//! second exactly at 1 (the plan relies on this for exact settled states), and never produces NaN
//! from finite inputs.

use datars_color::{Color, Oklab};
use datars_math::{lerp, m, Affine, Rect, Vec2};
use datars_scene::{Clip, Common, Geom, Paint, Stop, Stroke};
use datars_theme::{Ink, ResolvedTheme};
use std::sync::Arc;

/// An affine transform decomposed as translate · rotate · [[sx, sh], [0, sy]].
#[derive(Clone, Copy, Debug, PartialEq)]
struct Dec {
    tx: f64,
    ty: f64,
    rot: f64,
    sx: f64,
    sy: f64,
    sh: f64,
}

/// `None` when the linear part is zero (a fully collapsed ghost): its rotation is undefined and
/// borrowed from the other endpoint.
fn decompose(x: &Affine) -> Option<Dec> {
    let [a, b, c, d, e, f] = x.0;
    let sx = m::hypot(a, b);
    if sx > 1e-12 {
        let rot = m::atan2(b, a);
        let (s, co) = m::sin_cos(rot);
        Some(Dec { tx: e, ty: f, rot, sx, sy: -s * c + co * d, sh: co * c + s * d })
    } else {
        let sy = m::hypot(c, d);
        (sy > 1e-12).then(|| Dec { tx: e, ty: f, rot: m::atan2(-c, d), sx: 0.0, sy, sh: 0.0 })
    }
}

fn recompose(d: &Dec) -> Affine {
    let (s, c) = m::sin_cos(d.rot);
    Affine([c * d.sx, s * d.sx, c * d.sh - s * d.sy, s * d.sh + c * d.sy, d.tx, d.ty])
}

/// Interpolate transforms by decomposition (translate, shortest-arc rotation, scale, skew), so a
/// rotating element keeps its size. Equal linear parts only interpolate the translation (exact
/// for the common pure-translation case). Eased progress outside [0, 1] (back, elastic, spring)
/// extrapolates, as geometry does, so a moved group overshoots like a moved shape; scales stay
/// non-negative, as sizes do (no mirror flip while an entering ghost backs up).
pub fn lerp_affine(a: &Affine, b: &Affine, t: f64) -> Affine {
    if t == 0.0 || a == b {
        return *a;
    }
    if t == 1.0 {
        return *b;
    }
    let (pa, pb) = (a.0, b.0);
    if pa[..4] == pb[..4] {
        return Affine([pa[0], pa[1], pa[2], pa[3], lerp(pa[4], pb[4], t), lerp(pa[5], pb[5], t)]);
    }
    let (da, db) = match (decompose(a), decompose(b)) {
        (Some(x), Some(y)) => (x, y),
        (None, Some(y)) => (Dec { tx: pa[4], ty: pa[5], rot: y.rot, sx: 0.0, sy: 0.0, sh: 0.0 }, y),
        (Some(x), None) => (x, Dec { tx: pb[4], ty: pb[5], rot: x.rot, sx: 0.0, sy: 0.0, sh: 0.0 }),
        (None, None) => return Affine([0.0, 0.0, 0.0, 0.0, lerp(pa[4], pb[4], t), lerp(pa[5], pb[5], t)]),
    };
    let drot = m::rem_euclid(db.rot - da.rot + m::PI, m::TAU) - m::PI;
    let r = recompose(&Dec {
        tx: lerp(da.tx, db.tx, t),
        ty: lerp(da.ty, db.ty, t),
        rot: da.rot + drot * t,
        sx: lerp_size(da.sx, db.sx, t),
        sy: lerp_size(da.sy, db.sy, t),
        sh: lerp(da.sh, db.sh, t),
    });
    if r.0.iter().all(|v| v.is_finite()) {
        r
    } else if t < 0.5 {
        *a
    } else {
        *b
    }
}

pub(crate) fn lerp_rect(a: &Rect, b: &Rect, t: f64) -> Rect {
    if t <= 0.0 {
        return *a;
    }
    if t >= 1.0 {
        return *b;
    }
    Rect::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t), lerp(a.w, b.w, t), lerp(a.h, b.h, t))
}

/// A size-like quantity: never negative when neither endpoint is.
#[inline]
pub(crate) fn lerp_size(a: f64, b: f64, t: f64) -> f64 {
    let v = lerp(a, b, t);
    if a >= 0.0 && b >= 0.0 {
        v.max(0.0)
    } else {
        v
    }
}

#[inline]
pub(crate) fn lerp_v(a: Vec2, b: Vec2, t: f64) -> Vec2 {
    a.lerp(b, t)
}

/// Interpolate the properties every node has. `e` is eased progress (may overshoot), `op` the
/// progress opacity follows (monotone for enters and exits), `u` linear progress (discrete
/// properties switch at `u = ½`).
pub(crate) fn lerp_common(a: &Common, b: &Common, e: f64, op: f64, u: f64) -> Common {
    if u <= 0.0 {
        return a.clone();
    }
    if u >= 1.0 {
        return b.clone();
    }
    let eff = |c: &Common| if c.visible { c.opacity } else { 0.0 };
    let (oa, ob) = (eff(a), eff(b));
    let late = u >= 0.5;
    Common {
        transform: lerp_affine(&a.transform, &b.transform, e),
        opacity: if oa == ob { oa } else { lerp(oa, ob, op).clamp(0.0, 1.0) },
        visible: a.visible || b.visible,
        z: if late { b.z } else { a.z },
        clip: match (&a.clip, &b.clip) {
            (x, y) if x == y => x.clone(),
            (Some(Clip::Rect { rect: ra }), Some(Clip::Rect { rect: rb })) => Some(Clip::Rect { rect: lerp_rect(ra, rb, e) }),
            _ => {
                if late {
                    b.clip.clone()
                } else {
                    a.clip.clone()
                }
            }
        },
        trim: match (a.trim, b.trim) {
            (None, None) => None,
            (x, y) => {
                let (ta, tb) = (x.unwrap_or([0.0, 1.0]), y.unwrap_or([0.0, 1.0]));
                Some([lerp(ta[0], tb[0], e).clamp(0.0, 1.0), lerp(ta[1], tb[1], e).clamp(0.0, 1.0)])
            }
        },
        blend: if late { b.blend } else { a.blend },
        isolate: a.isolate || b.isolate,
        pin: if late { b.pin } else { a.pin },
    }
}

/// Colours in OKLab, premultiplied by alpha ([`Oklab::mix`]).
#[inline]
pub(crate) fn lerp_color(a: Color, b: Color, t: f64) -> Color {
    a.lerp_oklab(b, t)
}

/// Two colours mixed like [`Color::lerp_oklab`] (bit for bit), with both ends converted once.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SolidMix {
    a: Color,
    b: Color,
    la: Oklab,
    lb: Oklab,
}

impl SolidMix {
    pub(crate) fn new(a: Color, b: Color) -> SolidMix {
        SolidMix { a, b, la: a.to_oklab(), lb: b.to_oklab() }
    }
    pub(crate) fn at(&self, t: f64) -> Color {
        if t <= 0.0 {
            return self.a;
        }
        if t >= 1.0 {
            return self.b;
        }
        self.la.mix(self.lb, t).to_color()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Grad {
    radial: bool,
    ga: [f64; 4],
    gb: [f64; 4],
    at_a: Vec<f64>,
    at_b: Vec<f64>,
    ca: Vec<Color>,
    cb: Vec<Color>,
}

/// How a paint interpolates, decided (and its inks resolved) once at plan time.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PaintPlan {
    /// Equal paints (equal inks stay inks — a token stays a token mid-flight).
    Same,
    /// Different solid inks, resolved against the plan's theme and mixed in OKLab — converted
    /// once here, not every frame (thousands of dots changing colour at once).
    Solid(Box<SolidMix>),
    /// Gradients (or a solid against a gradient): geometry and stops interpolate.
    Grad(Box<Grad>),
    /// Absent → present: the ink's alpha rises (tokens stay tokens).
    FadeIn,
    /// Present → absent.
    FadeOut,
    /// Incompatible paints: switch at the midpoint.
    Step,
}

fn grad_parts(p: &Paint) -> Option<(bool, [f64; 4], &[Stop])> {
    match p {
        Paint::Linear { linear, stops } => Some((false, *linear, stops)),
        Paint::Radial { radial, stops } => Some((true, [radial[0], radial[1], radial[2], 0.0], stops)),
        Paint::Solid(_) => None,
    }
}

pub(crate) fn plan_paint(a: Option<&Paint>, b: Option<&Paint>, theme: &ResolvedTheme) -> PaintPlan {
    match (a, b) {
        (None, None) => PaintPlan::Same,
        (Some(x), Some(y)) if x == y => PaintPlan::Same,
        (None, Some(_)) => PaintPlan::FadeIn,
        (Some(_), None) => PaintPlan::FadeOut,
        (Some(Paint::Solid(ia)), Some(Paint::Solid(ib))) => PaintPlan::Solid(Box::new(SolidMix::new(ia.resolve(theme), ib.resolve(theme)))),
        (Some(x), Some(y)) => {
            let (gx, gy) = (grad_parts(x), grad_parts(y));
            let res = |s: &[Stop]| -> (Vec<f64>, Vec<Color>) { (s.iter().map(|s| s.at).collect(), s.iter().map(|s| s.ink.resolve(theme)).collect()) };
            match (gx, gy) {
                (Some((ra, ga, sa)), Some((rb, gb, sb))) if ra == rb && sa.len() == sb.len() => {
                    let ((at_a, ca), (at_b, cb)) = (res(sa), res(sb));
                    PaintPlan::Grad(Box::new(Grad { radial: ra, ga, gb, at_a, at_b, ca, cb }))
                }
                (None, Some((rb, gb, sb))) => {
                    let c = x.representative(theme);
                    let (at_b, cb) = res(sb);
                    PaintPlan::Grad(Box::new(Grad { radial: rb, ga: gb, gb, at_a: at_b.clone(), at_b, ca: vec![c; cb.len()], cb }))
                }
                (Some((ra, ga, sa)), None) => {
                    let c = y.representative(theme);
                    let (at_a, ca) = res(sa);
                    PaintPlan::Grad(Box::new(Grad { radial: ra, ga, gb: ga, at_b: at_a.clone(), at_a, cb: vec![c; ca.len()], ca }))
                }
                _ => PaintPlan::Step,
            }
        }
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

pub(crate) fn paint_at(plan: &PaintPlan, a: Option<&Paint>, b: Option<&Paint>, e: f64, u: f64) -> Option<Paint> {
    if u <= 0.0 {
        return a.cloned();
    }
    if u >= 1.0 {
        return b.cloned();
    }
    match plan {
        PaintPlan::Same => a.cloned(),
        PaintPlan::Solid(m) => Some(Paint::Solid(Ink::Color(m.at(e)))),
        PaintPlan::FadeIn => b.map(|p| p.fade(clamp01(e) as f32)),
        PaintPlan::FadeOut => a.map(|p| p.fade(1.0 - clamp01(e) as f32)),
        PaintPlan::Step => {
            if u < 0.5 {
                a.cloned()
            } else {
                b.cloned()
            }
        }
        PaintPlan::Grad(g) => {
            let t = clamp01(e);
            let geo: Vec<f64> = (0..4).map(|i| lerp(g.ga[i], g.gb[i], e)).collect();
            let stops = (0..g.ca.len())
                .map(|i| Stop { at: lerp(g.at_a[i], g.at_b[i], t), ink: Ink::Color(lerp_color(g.ca[i], g.cb[i], t)) })
                .collect();
            Some(if g.radial {
                Paint::Radial { radial: [geo[0], geo[1], geo[2].max(0.0)], stops }
            } else {
                Paint::Linear { linear: [geo[0], geo[1], geo[2], geo[3]], stops }
            })
        }
    }
}

/// Stroke interpolation plan.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StrokePlan {
    pub paint: PaintPlan,
}

pub(crate) fn plan_stroke(a: Option<&Stroke>, b: Option<&Stroke>, theme: &ResolvedTheme) -> StrokePlan {
    StrokePlan { paint: plan_paint(a.map(|s| &s.paint), b.map(|s| &s.paint), theme) }
}

pub(crate) fn stroke_at(plan: &StrokePlan, a: Option<&Stroke>, b: Option<&Stroke>, e: f64, u: f64) -> Option<Stroke> {
    if u <= 0.0 {
        return a.cloned();
    }
    if u >= 1.0 {
        return b.cloned();
    }
    match (a, b) {
        (None, None) => None,
        (Some(x), Some(y)) if x == y => Some(x.clone()),
        (Some(x), Some(y)) => {
            let late = u >= 0.5;
            let base = if late { y } else { x };
            Some(Stroke {
                paint: paint_at(&plan.paint, Some(&x.paint), Some(&y.paint), e, u).unwrap_or_else(|| base.paint.clone()),
                width: lerp_size(x.width, y.width, e),
                dash: match (&x.dash, &y.dash) {
                    (Some(da), Some(db)) if da.len() == db.len() => Some(da.iter().zip(db).map(|(p, q)| lerp_size(*p, *q, e)).collect()),
                    _ => base.dash.clone(),
                },
                cap: base.cap,
                join: base.join,
                non_scaling: base.non_scaling,
            })
        }
        (None, Some(y)) => {
            let k = clamp01(e);
            Some(Stroke { width: y.width * k, paint: y.paint.fade(k as f32), ..y.clone() })
        }
        (Some(x), None) => {
            let k = 1.0 - clamp01(e);
            Some(Stroke { width: x.width * k, paint: x.paint.fade(k as f32), ..x.clone() })
        }
    }
}

fn lerp_pts(a: &[Vec2], b: &[Vec2], t: f64) -> Arc<[Vec2]> {
    a.iter().zip(b).map(|(p, q)| p.lerp(*q, t)).collect()
}

/// Same-kind geometry: interpolate parameters. `None` when the pair is not parametric-compatible
/// (different kinds, symbol kinds, point counts or path structures).
pub(crate) fn lerp_geom(a: &Geom, b: &Geom, e: f64, u: f64) -> Option<Geom> {
    if u <= 0.0 {
        return Some(a.clone());
    }
    if u >= 1.0 {
        return Some(b.clone());
    }
    let l = |x: f64, y: f64| lerp(x, y, e);
    let s = |x: f64, y: f64| lerp_size(x, y, e);
    Some(match (a, b) {
        (Geom::Rect { x, y, w, h, r }, Geom::Rect { x: x2, y: y2, w: w2, h: h2, r: r2 }) => Geom::Rect {
            x: l(*x, *x2),
            y: l(*y, *y2),
            w: s(*w, *w2),
            h: s(*h, *h2),
            r: [s(r[0], r2[0]), s(r[1], r2[1]), s(r[2], r2[2]), s(r[3], r2[3])],
        },
        (Geom::Ellipse { cx, cy, rx, ry }, Geom::Ellipse { cx: cx2, cy: cy2, rx: rx2, ry: ry2 }) => {
            Geom::Ellipse { cx: l(*cx, *cx2), cy: l(*cy, *cy2), rx: s(*rx, *rx2), ry: s(*ry, *ry2) }
        }
        (Geom::Arc { cx, cy, r0, r1, a0, a1 }, Geom::Arc { cx: cx2, cy: cy2, r0: r02, r1: r12, a0: a02, a1: a12 }) => Geom::Arc {
            cx: l(*cx, *cx2),
            cy: l(*cy, *cy2),
            r0: s(*r0, *r02),
            r1: s(*r1, *r12),
            a0: l(*a0, *a02),
            a1: l(*a1, *a12),
        },
        (Geom::Segment { x1, y1, x2, y2 }, Geom::Segment { x1: p1, y1: q1, x2: p2, y2: q2 }) => {
            Geom::Segment { x1: l(*x1, *p1), y1: l(*y1, *q1), x2: l(*x2, *p2), y2: l(*y2, *q2) }
        }
        (Geom::Symbol { kind, x, y, size }, Geom::Symbol { kind: k2, x: x2, y: y2, size: s2 }) if kind == k2 => {
            Geom::Symbol { kind: *kind, x: l(*x, *x2), y: l(*y, *y2), size: s(*size, *s2) }
        }
        (Geom::Polyline { pts, closed, curve }, Geom::Polyline { pts: p2, closed: c2, curve: cv2 }) if pts.len() == p2.len() && closed == c2 => {
            Geom::Polyline { pts: lerp_pts(pts, p2, e), closed: *closed, curve: if u < 0.5 { *curve } else { *cv2 } }
        }
        (Geom::Area { top, base, curve }, Geom::Area { top: t2, base: b2, curve: cv2 }) if top.len() == t2.len() && base.len() == b2.len() => {
            Geom::Area { top: lerp_pts(top, t2, e), base: lerp_pts(base, b2, e), curve: if u < 0.5 { *curve } else { *cv2 } }
        }
        (Geom::Path { path: pa }, Geom::Path { path: pb }) => {
            if pa == pb {
                return Some(a.clone());
            }
            return lerp_path(pa, pb, e).map(Geom::path);
        }
        _ => return None,
    })
}

/// Paths with the same element structure interpolate control points.
pub(crate) fn same_path_structure(a: &datars_math::PathData, b: &datars_math::PathData) -> bool {
    use datars_math::PathEl as E;
    a.els.len() == b.els.len()
        && a.els.iter().zip(&b.els).all(|(x, y)| {
            matches!((x, y), (E::Move { .. }, E::Move { .. }) | (E::Line { .. }, E::Line { .. }) | (E::Quad { .. }, E::Quad { .. }) | (E::Cubic { .. }, E::Cubic { .. }) | (E::Close, E::Close))
        })
}

/// `b` is `a` moved: the one vector every point (and control point) of path `a` shifts by to land
/// on `b`'s, if there is one and it isn't zero (paths only: lines keep their point plans, which
/// the dots riding them anchor to). A map re-fitted to a box a few px taller moves every
/// region by the same amount; interpolated point by point that is exactly a translation, so it can
/// be drawn as `a` under a moving transform — its meshes kept, not rebuilt every frame.
pub(crate) fn translation_between(a: &Geom, b: &Geom) -> Option<Vec2> {
    use datars_math::PathEl as E;
    let (pa, pb): (Vec<Vec2>, Vec<Vec2>) = match (a, b) {
        (Geom::Path { path: x }, Geom::Path { path: y }) if same_path_structure(x, y) => {
            let pts = |p: &datars_math::PathData| -> Vec<Vec2> {
                p.els
                    .iter()
                    .flat_map(|e| match *e {
                        E::Move { p } | E::Line { p } => vec![p],
                        E::Quad { c, p } => vec![c, p],
                        E::Cubic { c1, c2, p } => vec![c1, c2, p],
                        E::Close => vec![],
                    })
                    .collect()
            };
            (pts(x), pts(y))
        }
        _ => return None,
    };
    let (&a0, &b0) = (pa.first()?, pb.first()?);
    let d = b0 - a0;
    if d == Vec2::ZERO {
        return None;
    }
    // Projection arithmetic leaves last-digit differences: equal within a tolerance relative to
    // the coordinates' size (far below anything a frame shows).
    let tol = |p: Vec2| 1e-9 * (1.0 + p.x.abs().max(p.y.abs()));
    pa.iter().zip(&pb).all(|(&p, &q)| {
        let r = q - (p + d);
        r.x.abs().max(r.y.abs()) <= tol(q)
    })
    .then_some(d)
}

fn lerp_path(a: &datars_math::PathData, b: &datars_math::PathData, t: f64) -> Option<datars_math::PathData> {
    use datars_math::PathEl as E;
    if !same_path_structure(a, b) {
        return None;
    }
    let els = a
        .els
        .iter()
        .zip(&b.els)
        .map(|(x, y)| match (*x, *y) {
            (E::Move { p }, E::Move { p: q }) => E::Move { p: p.lerp(q, t) },
            (E::Line { p }, E::Line { p: q }) => E::Line { p: p.lerp(q, t) },
            (E::Quad { c, p }, E::Quad { c: c2, p: q }) => E::Quad { c: c.lerp(c2, t), p: p.lerp(q, t) },
            (E::Cubic { c1, c2, p }, E::Cubic { c1: d1, c2: d2, p: q }) => E::Cubic { c1: c1.lerp(d1, t), c2: c2.lerp(d2, t), p: p.lerp(q, t) },
            _ => E::Close,
        })
        .collect();
    Some(datars_math::PathData { els })
}

/// Fast OKLab → sRGB for column fills (10⁵ instances per frame): the linear → sRGB transfer uses
/// a 4096-entry table with linear interpolation instead of `pow` (deterministic: the table is
/// built from `datars_math::m`).
pub(crate) fn oklab_to_color_fast(l: f64, a: f64, b: f64, alpha: f64) -> Color {
    let l_ = l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_ = l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_ = l - 0.089_484_177_5 * a - 1.291_485_548_0 * b;
    let (lc, mc, sc) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_662_1 * lc - 3.307_711_591_3 * mc + 0.230_969_929_2 * sc;
    let g = -1.268_438_004_6 * lc + 2.609_757_401_1 * mc - 0.341_319_396_5 * sc;
    let bl = -0.004_196_086_3 * lc - 0.703_418_614_7 * mc + 1.707_614_701_0 * sc;
    let lut = srgb_lut();
    let tf = |v: f64| -> f32 {
        let v = v.clamp(0.0, 1.0) * (LUT_N - 1) as f64;
        let i = (v as usize).min(LUT_N - 2);
        let f = v - i as f64;
        (lut[i] + (lut[i + 1] - lut[i]) * f) as f32
    };
    Color { r: tf(r), g: tf(g), b: tf(bl), a: alpha.clamp(0.0, 1.0) as f32 }
}

const LUT_N: usize = 4096;

fn srgb_lut() -> &'static [f64; LUT_N] {
    static LUT: std::sync::OnceLock<Box<[f64; LUT_N]>> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = Box::new([0.0; LUT_N]);
        for (i, v) in t.iter_mut().enumerate() {
            *v = datars_color::to_srgb(i as f64 / (LUT_N - 1) as f64);
        }
        t
    })
}

pub(crate) fn to_lab(c: Color) -> [f64; 4] {
    let o: Oklab = c.to_oklab();
    [o.l, o.a, o.b, o.alpha]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_endpoints_exact_and_rotation_keeps_scale() {
        let a = Affine::translate(3.0, 4.0);
        let b = Affine::rotate(1.0).then(Affine::scale(2.0, 2.0)).then(Affine::translate(10.0, 0.0));
        assert_eq!(lerp_affine(&a, &b, 0.0), a);
        assert_eq!(lerp_affine(&a, &b, 1.0), b);
        let r = Affine::rotate(0.0);
        let q = Affine::rotate(m::PI * 0.9);
        let mid = lerp_affine(&r, &q, 0.5);
        assert!((mid.scale_factor() - 1.0).abs() < 1e-12, "rotation keeps unit scale");
    }

    #[test]
    fn collapsed_ghost_scales_about_a_fixed_point() {
        let base = Affine::translate(20.0, 0.0);
        let o = Vec2::new(5.0, 100.0);
        let ghost = base.mul(Affine::translate(-o.x, -o.y).then(Affine::scale(0.0, 0.0)).then(Affine::translate(o.x, o.y)));
        for t in [0.1, 0.5, 0.9] {
            let x = lerp_affine(&ghost, &base, t);
            let p = x.apply(o);
            assert!((p.x - 25.0).abs() < 1e-9 && (p.y - 100.0).abs() < 1e-9, "{t}: {p:?}");
        }
    }

    #[test]
    fn fast_oklab_matches_exact() {
        for hex in ["#e8112d", "#1f77b4", "#ffffff", "#000000", "#7f7f7f"] {
            let c = Color::parse(hex).unwrap();
            let [l, a, b, al] = to_lab(c);
            let f = oklab_to_color_fast(l, a, b, al);
            assert!((f.r - c.r).abs() < 2e-3 && (f.g - c.g).abs() < 2e-3 && (f.b - c.b).abs() < 2e-3, "{hex}");
        }
    }
}
