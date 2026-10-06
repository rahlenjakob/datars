//! Easing curves: pure functions of normalized time with `apply(0) == 0` and `apply(1) == 1`
//! exactly, so settled states are bit-exact whatever curve a rule picks.
//!
//! Every curve is seekable — including springs, whose damped harmonic oscillator is evaluated in
//! closed form and normalized to end at 1 — so scroll can scrub them and video can render them
//! (P2). Curves serialize as the strings authors write (`"cubic-in-out"`,
//! `"cubic-bezier(.2,.8,.2,1)"`, `"spring(170, 26)"`, `"steps(4)"`), which is how they appear in
//! the document IR.

use datars_math::m;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The named curve families (easings.net shapes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Family {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

impl Family {
    pub const ALL: [Family; 10] = [
        Family::Quad,
        Family::Cubic,
        Family::Quart,
        Family::Quint,
        Family::Sine,
        Family::Expo,
        Family::Circ,
        Family::Back,
        Family::Elastic,
        Family::Bounce,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Family::Quad => "quad",
            Family::Cubic => "cubic",
            Family::Quart => "quart",
            Family::Quint => "quint",
            Family::Sine => "sine",
            Family::Expo => "expo",
            Family::Circ => "circ",
            Family::Back => "back",
            Family::Elastic => "elastic",
            Family::Bounce => "bounce",
        }
    }

    fn from_name(s: &str) -> Option<Family> {
        Family::ALL.iter().copied().find(|f| f.name() == s)
    }

    /// Curves that never decrease (no overshoot, no bounce).
    pub fn is_monotone(self) -> bool {
        !matches!(self, Family::Back | Family::Elastic | Family::Bounce)
    }

    /// The direction a bare family name means: the overshooting families read as "out" (they
    /// land with character), the rest as "in-out".
    fn default_dir(self) -> Dir {
        match self {
            Family::Back | Family::Elastic | Family::Bounce => Dir::Out,
            _ => Dir::InOut,
        }
    }

    /// The ease-in form; out and in-out are derived by reflection so all three share one shape.
    fn ease_in(self, x: f64) -> f64 {
        const C1: f64 = 1.701_58;
        const C3: f64 = C1 + 1.0;
        const C4: f64 = m::TAU / 3.0;
        match self {
            Family::Quad => x * x,
            Family::Cubic => x * x * x,
            Family::Quart => x * x * x * x,
            Family::Quint => x * x * x * x * x,
            Family::Sine => 1.0 - m::cos(x * m::PI / 2.0),
            Family::Expo => {
                if x <= 0.0 {
                    0.0
                } else {
                    m::exp2(10.0 * x - 10.0)
                }
            }
            Family::Circ => 1.0 - (1.0 - x * x).max(0.0).sqrt(),
            Family::Back => C3 * x * x * x - C1 * x * x,
            Family::Elastic => {
                if x <= 0.0 {
                    0.0
                } else if x >= 1.0 {
                    1.0
                } else {
                    -m::exp2(10.0 * x - 10.0) * m::sin((x * 10.0 - 10.75) * C4)
                }
            }
            Family::Bounce => 1.0 - bounce_out(1.0 - x),
        }
    }
}

pub(crate) fn bounce_out(x: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if x < 1.0 / D {
        N * x * x
    } else if x < 2.0 / D {
        let u = x - 1.5 / D;
        N * u * u + 0.75
    } else if x < 2.5 / D {
        let u = x - 2.25 / D;
        N * u * u + 0.9375
    } else {
        let u = x - 2.625 / D;
        N * u * u + 0.984_375
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dir {
    In,
    Out,
    InOut,
}

impl Dir {
    fn suffix(self) -> &'static str {
        match self {
            Dir::In => "in",
            Dir::Out => "out",
            Dir::InOut => "in-out",
        }
    }
}

/// Where the jumps of a `steps()` curve fall (CSS `step-position`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StepPosition {
    JumpStart,
    #[default]
    JumpEnd,
    JumpNone,
    JumpBoth,
}

impl StepPosition {
    fn name(self) -> &'static str {
        match self {
            StepPosition::JumpStart => "jump-start",
            StepPosition::JumpEnd => "jump-end",
            StepPosition::JumpNone => "jump-none",
            StepPosition::JumpBoth => "jump-both",
        }
    }
    fn parse(s: &str) -> Option<StepPosition> {
        Some(match s {
            "jump-start" | "start" => StepPosition::JumpStart,
            "jump-end" | "end" => StepPosition::JumpEnd,
            "jump-none" => StepPosition::JumpNone,
            "jump-both" => StepPosition::JumpBoth,
            _ => return None,
        })
    }
}

/// One key of a keyframed curve: at time `t` the curve has value `v`; `ease` shapes the segment
/// from this key to the next (the CSS convention).
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub t: f64,
    pub v: f64,
    pub ease: Easing,
}

/// An easing curve.
#[derive(Clone, Debug, PartialEq)]
pub enum Easing {
    Linear,
    Named(Family, Dir),
    /// CSS-compatible `cubic-bezier(x1, y1, x2, y2)`; `x1`/`x2` are clamped into [0, 1].
    CubicBezier(f64, f64, f64, f64),
    /// A damped harmonic oscillator released at distance 1 from its target with velocity `v0`
    /// (progress per second, positive = towards the target). Evaluated in closed form over its
    /// settling time and normalized so it ends exactly at 1.
    Spring { stiffness: f64, damping: f64, mass: f64, v0: f64 },
    Steps(u32, StepPosition),
    Keyframed(Vec<Keyframe>),
}

impl Default for Easing {
    /// Cubic in-out: the default of every motion rule.
    fn default() -> Self {
        Easing::Named(Family::Cubic, Dir::InOut)
    }
}

impl Easing {
    pub const CUBIC_IN_OUT: Easing = Easing::Named(Family::Cubic, Dir::InOut);

    /// CSS `ease`.
    pub fn css_ease() -> Easing {
        Easing::CubicBezier(0.25, 0.1, 0.25, 1.0)
    }

    pub fn spring(stiffness: f64, damping: f64) -> Easing {
        Easing::Spring { stiffness, damping, mass: 1.0, v0: 0.0 }
    }

    /// The curve at normalized time `t`. `apply(t ≤ 0) == 0` and `apply(t ≥ 1) == 1` exactly;
    /// values in between may leave [0, 1] for overshooting curves (back, elastic, springs).
    pub fn apply(&self, t: f64) -> f64 {
        if t.is_nan() || t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        let v = match self {
            Easing::Linear => t,
            Easing::Named(f, d) => match d {
                Dir::In => f.ease_in(t),
                Dir::Out => 1.0 - f.ease_in(1.0 - t),
                Dir::InOut => {
                    if t < 0.5 {
                        f.ease_in(2.0 * t) / 2.0
                    } else {
                        1.0 - f.ease_in(2.0 - 2.0 * t) / 2.0
                    }
                }
            },
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(*x1, *y1, *x2, *y2, t),
            Easing::Spring { stiffness, damping, mass, v0 } => match SpringModel::new(*stiffness, *damping, *mass, *v0) {
                Some(s) => s.normalized(t),
                None => t,
            },
            Easing::Steps(n, pos) => steps(*n, *pos, t),
            Easing::Keyframed(keys) => keyframed(keys, t),
        };
        if v.is_finite() {
            v
        } else {
            t
        }
    }

    /// Whether the curve never decreases on [0, 1] and stays within [0, 1]. Opacity of entering
    /// and exiting elements follows the curve only when it is monotone (otherwise linear time),
    /// so an overshooting easing can never make a fading element flash.
    pub fn is_monotone(&self) -> bool {
        match self {
            Easing::Linear | Easing::Steps(..) => true,
            Easing::Named(f, _) => f.is_monotone(),
            Easing::CubicBezier(_, y1, _, y2) => (0.0..=1.0).contains(y1) && (0.0..=1.0).contains(y2),
            Easing::Spring { stiffness, damping, mass, v0 } => {
                let m = if *mass > 0.0 { *mass } else { 1.0 };
                *stiffness > 0.0 && *damping >= 2.0 * (stiffness * m).sqrt() && *v0 >= 0.0
            }
            Easing::Keyframed(keys) => {
                keys.windows(2).all(|w| w[1].v >= w[0].v && w[0].ease.is_monotone())
                    && keys.iter().all(|k| (0.0..=1.0).contains(&k.v))
            }
        }
    }

    /// Parse the string forms: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`,
    /// `<family>-in|out|in-out` or a bare family (`cubic`, `back`, …), `cubic-bezier(x1,y1,x2,y2)`,
    /// `spring(stiffness, damping[, mass[, v0]])`, `steps(n[, jump-start|jump-end|jump-none|
    /// jump-both|start|end])`, `step-start`, `step-end`, and `keyframes(t v [ease]; …)`.
    pub fn parse(s: &str) -> Option<Easing> {
        let s = s.trim().to_ascii_lowercase();
        if let Some(args) = call(&s, "cubic-bezier") {
            let v = nums(args)?;
            if v.len() != 4 {
                return None;
            }
            return Some(Easing::CubicBezier(v[0].clamp(0.0, 1.0), v[1], v[2].clamp(0.0, 1.0), v[3]));
        }
        if let Some(args) = call(&s, "spring") {
            let v = nums(args)?;
            if v.len() < 2 || v.len() > 4 {
                return None;
            }
            return Some(Easing::Spring {
                stiffness: v[0],
                damping: v[1],
                mass: v.get(2).copied().unwrap_or(1.0),
                v0: v.get(3).copied().unwrap_or(0.0),
            });
        }
        if let Some(args) = call(&s, "steps") {
            let mut it = args.split(',').map(str::trim);
            let n: u32 = it.next()?.parse().ok()?;
            let pos = match it.next() {
                Some(p) => StepPosition::parse(p)?,
                None => StepPosition::JumpEnd,
            };
            if it.next().is_some() || n == 0 {
                return None;
            }
            return Some(Easing::Steps(n, pos));
        }
        if let Some(args) = call(&s, "keyframes") {
            let mut keys = Vec::new();
            for part in split_top(args, ';') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let mut fields = part.splitn(3, char::is_whitespace);
                let t: f64 = fields.next()?.trim().parse().ok()?;
                let v: f64 = fields.next()?.trim().parse().ok()?;
                let ease = match fields.next() {
                    Some(e) if !e.trim().is_empty() => Easing::parse(e)?,
                    _ => Easing::Linear,
                };
                keys.push(Keyframe { t, v, ease });
            }
            keys.sort_by(|a, b| datars_math::total_cmp(a.t, b.t));
            return Some(Easing::Keyframed(keys));
        }
        Some(match s.as_str() {
            "linear" => Easing::Linear,
            "ease" => Easing::css_ease(),
            "ease-in" => Easing::CubicBezier(0.42, 0.0, 1.0, 1.0),
            "ease-out" => Easing::CubicBezier(0.0, 0.0, 0.58, 1.0),
            "ease-in-out" => Easing::CubicBezier(0.42, 0.0, 0.58, 1.0),
            "step-start" => Easing::Steps(1, StepPosition::JumpStart),
            "step-end" => Easing::Steps(1, StepPosition::JumpEnd),
            other => {
                let (fam, dir) = if let Some(f) = other.strip_suffix("-in-out") {
                    (f, Some(Dir::InOut))
                } else if let Some(f) = other.strip_suffix("-out") {
                    (f, Some(Dir::Out))
                } else if let Some(f) = other.strip_suffix("-in") {
                    (f, Some(Dir::In))
                } else {
                    (other, None)
                };
                let f = Family::from_name(fam)?;
                Easing::Named(f, dir.unwrap_or(f.default_dir()))
            }
        })
    }
}

/// `name(args)` → `args`.
fn call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    s.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

fn nums(args: &str) -> Option<Vec<f64>> {
    args.split(',').map(|a| a.trim().parse::<f64>().ok().filter(|v| v.is_finite())).collect()
}

/// Split on `sep` outside parentheses.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Solve a CSS cubic bézier deterministically: Newton's method from `s = t` (fixed iteration
/// cap), falling back to bisection when the slope vanishes or the iterate leaves [0, 1].
fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let (x1, x2) = (x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0));
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let sx = |s: f64| ((ax * s + bx) * s + cx) * s;
    let sy = |s: f64| ((ay * s + by) * s + cy) * s;
    let dsx = |s: f64| (3.0 * ax * s + 2.0 * bx) * s + cx;
    const EPS: f64 = 1e-12;
    let mut s = t;
    for _ in 0..8 {
        let x = sx(s) - t;
        if x.abs() < EPS {
            return sy(s);
        }
        let d = dsx(s);
        if d.abs() < 1e-9 {
            break;
        }
        s -= x / d;
        if !(0.0..=1.0).contains(&s) {
            break;
        }
    }
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    s = t;
    for _ in 0..64 {
        let x = sx(s);
        if (x - t).abs() < EPS {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    sy(s)
}

fn steps(n: u32, pos: StepPosition, t: f64) -> f64 {
    let n = n.max(1) as f64;
    let v = match pos {
        StepPosition::JumpEnd => (t * n).floor() / n,
        StepPosition::JumpStart => (t * n).floor().min(n - 1.0) / n + 1.0 / n,
        StepPosition::JumpNone => {
            if n <= 1.0 {
                (t * n).floor()
            } else {
                (t * n).floor() / (n - 1.0)
            }
        }
        StepPosition::JumpBoth => ((t * n).floor() + 1.0) / (n + 1.0),
    };
    v.clamp(0.0, 1.0)
}

fn keyframed(keys: &[Keyframe], t: f64) -> f64 {
    match keys {
        [] => t,
        [k] => k.v,
        _ => {
            if t <= keys[0].t {
                return keys[0].v;
            }
            for w in keys.windows(2) {
                let (a, b) = (&w[0], &w[1]);
                if t < b.t {
                    let span = b.t - a.t;
                    let local = if span > 0.0 { (t - a.t) / span } else { 1.0 };
                    return a.v + (b.v - a.v) * a.ease.apply(local);
                }
            }
            keys[keys.len() - 1].v
        }
    }
}

/// The closed-form damped harmonic oscillator behind `Easing::Spring`: displacement `x(τ)` from
/// the target, starting at `x(0) = 1` with `x'(0) = -v0`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpringModel {
    kind: u8, // 0 under-, 1 critically, 2 over-damped
    w0: f64,
    zeta: f64,
    wd: f64,
    b: f64,
    r1: f64,
    r2: f64,
    c1: f64,
    c2: f64,
    /// Settling time (physical seconds) the normalized curve spans.
    pub settle: f64,
    x_settle: f64,
}

impl SpringModel {
    const EPS: f64 = 1e-3;

    pub fn new(stiffness: f64, damping: f64, mass: f64, v0: f64) -> Option<SpringModel> {
        let mass = if mass > 0.0 && mass.is_finite() { mass } else { 1.0 };
        if !(stiffness > 0.0 && stiffness.is_finite() && damping >= 0.0 && damping.is_finite() && v0.is_finite()) {
            return None;
        }
        let w0 = (stiffness / mass).sqrt();
        let zeta = damping / (2.0 * (stiffness * mass).sqrt());
        let mut s = SpringModel { kind: 0, w0, zeta, wd: 0.0, b: 0.0, r1: 0.0, r2: 0.0, c1: 0.0, c2: 0.0, settle: 1.0, x_settle: 0.0 };
        if (zeta - 1.0).abs() < 1e-6 {
            s.kind = 1;
            s.b = w0 - v0;
            // Solve |1 + bτ|·e^{-w0 τ} = eps by fixed-point iteration (log grows slowly).
            let mut tau = m::ln(1.0 / Self::EPS) / w0;
            for _ in 0..30 {
                tau = m::ln((1.0 + s.b * tau).abs().max(1.0) / Self::EPS) / w0;
            }
            s.settle = tau;
        } else if zeta < 1.0 {
            s.kind = 0;
            s.wd = w0 * (1.0 - zeta * zeta).sqrt();
            s.b = (zeta * w0 - v0) / s.wd;
            s.settle = if zeta > 0.0 {
                m::ln((1.0 + s.b * s.b).sqrt() / Self::EPS) / (zeta * w0)
            } else {
                // Undamped: never settles; span eight periods.
                8.0 * m::TAU / s.wd
            };
        } else {
            s.kind = 2;
            let q = (zeta * zeta - 1.0).sqrt();
            s.r1 = -w0 * (zeta - q);
            s.r2 = -w0 * (zeta + q);
            s.c2 = (-v0 - s.r1) / (s.r2 - s.r1);
            s.c1 = 1.0 - s.c2;
            let t1 = m::ln((s.c1.abs() * 2.0).max(Self::EPS) / Self::EPS) / s.r1.abs();
            let t2 = m::ln((s.c2.abs() * 2.0).max(Self::EPS) / Self::EPS) / s.r2.abs();
            s.settle = t1.max(t2);
        }
        if !s.settle.is_finite() || s.settle <= 0.0 {
            s.settle = 1.0;
        }
        s.settle = s.settle.clamp(1e-6, 1e6);
        s.x_settle = s.x(s.settle);
        Some(s)
    }

    /// Displacement from the target at physical time `tau`.
    pub fn x(&self, tau: f64) -> f64 {
        match self.kind {
            0 => m::exp(-self.zeta * self.w0 * tau) * (m::cos(self.wd * tau) + self.b * m::sin(self.wd * tau)),
            1 => m::exp(-self.w0 * tau) * (1.0 + self.b * tau),
            _ => self.c1 * m::exp(self.r1 * tau) + self.c2 * m::exp(self.r2 * tau),
        }
    }

    /// Progress at normalized time `t` ∈ [0, 1]: `1 − x(t·T)`, with the residual `x(T)` removed
    /// linearly so the curve lands exactly on 1.
    pub fn normalized(&self, t: f64) -> f64 {
        1.0 - (self.x(t * self.settle) - t * self.x_settle)
    }
}

/// The settling time of a spring in seconds (to within 0.1 % of its target) — a natural duration
/// for rules that want a spring to take as long as it physically would.
pub fn spring_settle_time(stiffness: f64, damping: f64, mass: f64, v0: f64) -> f64 {
    SpringModel::new(stiffness, damping, mass, v0).map_or(0.0, |s| s.settle)
}

fn fmt_num(v: f64) -> String {
    format!("{v}")
}

impl fmt::Display for Easing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Easing::Linear => write!(f, "linear"),
            Easing::Named(fam, dir) => write!(f, "{}-{}", fam.name(), dir.suffix()),
            Easing::CubicBezier(a, b, c, d) => {
                write!(f, "cubic-bezier({},{},{},{})", fmt_num(*a), fmt_num(*b), fmt_num(*c), fmt_num(*d))
            }
            Easing::Spring { stiffness, damping, mass, v0 } => {
                if *mass == 1.0 && *v0 == 0.0 {
                    write!(f, "spring({}, {})", fmt_num(*stiffness), fmt_num(*damping))
                } else {
                    write!(f, "spring({}, {}, {}, {})", fmt_num(*stiffness), fmt_num(*damping), fmt_num(*mass), fmt_num(*v0))
                }
            }
            Easing::Steps(n, pos) => {
                if *pos == StepPosition::JumpEnd {
                    write!(f, "steps({n})")
                } else {
                    write!(f, "steps({n}, {})", pos.name())
                }
            }
            Easing::Keyframed(keys) => {
                write!(f, "keyframes(")?;
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, "; ")?;
                    }
                    write!(f, "{} {}", fmt_num(k.t), fmt_num(k.v))?;
                    if k.ease != Easing::Linear {
                        write!(f, " {}", k.ease)?;
                    }
                }
                write!(f, ")")
            }
        }
    }
}

impl Serialize for Easing {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Easing {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Easing, D::Error> {
        let s = String::deserialize(d)?;
        Easing::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("not an easing: {s}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_curves_are_reflections() {
        let e = Easing::Named(Family::Cubic, Dir::Out);
        assert!((e.apply(0.5) - 0.875).abs() < 1e-12);
        let io = Easing::CUBIC_IN_OUT;
        assert!((io.apply(0.5) - 0.5).abs() < 1e-12);
        assert!((io.apply(0.25) - 0.0625).abs() < 1e-12);
    }

    #[test]
    fn spring_model_starts_at_rest_distance() {
        for (k, c) in [(170.0, 26.0), (100.0, 5.0), (100.0, 40.0)] {
            let s = SpringModel::new(k, c, 1.0, 0.0).unwrap();
            assert!((s.x(0.0) - 1.0).abs() < 1e-12);
            assert!(s.x(s.settle).abs() < 2e-3, "{k} {c}: {}", s.x(s.settle));
        }
    }

    #[test]
    fn parses_leading_dot_numbers() {
        assert_eq!(Easing::parse("cubic-bezier(.2,.8,.2,1)"), Some(Easing::CubicBezier(0.2, 0.8, 0.2, 1.0)));
    }
}
