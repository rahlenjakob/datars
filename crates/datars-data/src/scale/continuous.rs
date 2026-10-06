//! Continuous scales: a transform (identity, log, pow, sqrt, symlog) applied to the domain before
//! a linear map onto the range.

use datars_math::m;
use serde::{Deserialize, Serialize};

/// Domain and range of a continuous scale. `clamp` keeps outputs inside the range (and inverted
/// values inside the domain).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Continuous {
    pub domain: [f64; 2],
    pub range: [f64; 2],
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clamp: bool,
}

impl Continuous {
    pub fn new(domain: [f64; 2], range: [f64; 2]) -> Continuous {
        Continuous { domain, range, clamp: false }
    }
}

/// The function applied to values before the linear map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Transform {
    #[default]
    Linear,
    /// log_base(x). A domain entirely below zero is mirrored (−log(−x)); zero maps to NaN.
    Log { base: f64 },
    /// sign(x)·|x|^exponent.
    Pow { exponent: f64 },
    Sqrt,
    /// sign(x)·log(1 + |x|/constant): log-like for large values, linear near zero, defined at 0.
    Symlog { constant: f64 },
}

impl Transform {
    pub fn is_linear(&self) -> bool {
        matches!(self, Transform::Linear)
    }

    /// Whether a log transform over this domain works on mirrored (negative) values.
    fn mirrored(self, domain: [f64; 2]) -> bool {
        matches!(self, Transform::Log { .. }) && domain[0] < 0.0 && domain[1] < 0.0
    }

    /// Forward transform of `x` for a scale over `domain`.
    pub fn forward(self, x: f64, domain: [f64; 2]) -> f64 {
        match self {
            Transform::Linear => x,
            Transform::Log { base } => {
                let (x, s) = if self.mirrored(domain) { (-x, -1.0) } else { (x, 1.0) };
                if x > 0.0 {
                    s * log(x, base)
                } else {
                    f64::NAN
                }
            }
            Transform::Pow { exponent } => signed(x, |a| if exponent == 0.5 { a.sqrt() } else if exponent == 1.0 { a } else { m::pow(a, exponent) }),
            Transform::Sqrt => signed(x, f64::sqrt),
            Transform::Symlog { constant } => signed(x, |a| m::ln(1.0 + a / constant)),
        }
    }

    /// Inverse of [`Transform::forward`].
    pub fn inverse(self, y: f64, domain: [f64; 2]) -> f64 {
        match self {
            Transform::Linear => y,
            Transform::Log { base } => {
                let v = m::pow(base, if self.mirrored(domain) { -y } else { y });
                if self.mirrored(domain) {
                    -v
                } else {
                    v
                }
            }
            Transform::Pow { exponent } => signed(y, |a| if exponent == 1.0 { a } else { m::pow(a, 1.0 / exponent) }),
            Transform::Sqrt => signed(y, |a| a * a),
            Transform::Symlog { constant } => signed(y, |a| (m::exp(a) - 1.0) * constant),
        }
    }

    /// Position t of `x` in `domain` after the transform (0 at domain[0], 1 at domain[1]). A
    /// collapsed domain maps everything to 0.5.
    pub fn normalize(self, x: f64, domain: [f64; 2]) -> f64 {
        let (a, b) = (self.forward(domain[0], domain), self.forward(domain[1], domain));
        let y = self.forward(x, domain);
        if a == b {
            return if y.is_nan() { f64::NAN } else { 0.5 };
        }
        (y - a) / (b - a)
    }

    /// The domain value at position t (inverse of [`Transform::normalize`]).
    pub fn denormalize(self, t: f64, domain: [f64; 2]) -> f64 {
        let (a, b) = (self.forward(domain[0], domain), self.forward(domain[1], domain));
        self.inverse(a + (b - a) * t, domain)
    }
}

/// log_base(x), using the dedicated log10 / log2 for those bases so exact powers give exact
/// integers (log10(1e12) = 12, while ln(1e12)/ln(10) is 11.999…).
pub(crate) fn log(x: f64, base: f64) -> f64 {
    if base == 10.0 {
        m::log10(x)
    } else if base == 2.0 {
        m::log2(x)
    } else {
        m::ln(x) / m::ln(base)
    }
}

fn signed(x: f64, f: impl Fn(f64) -> f64) -> f64 {
    if x < 0.0 {
        -f(-x)
    } else {
        f(x)
    }
}

/// Map through a continuous scale.
pub(crate) fn map(tf: Transform, c: &Continuous, x: f64) -> f64 {
    let mut t = tf.normalize(x, c.domain);
    if c.clamp {
        t = t.clamp(0.0, 1.0);
    }
    datars_math::lerp(c.range[0], c.range[1], t)
}

/// Invert a continuous scale.
pub(crate) fn invert(tf: Transform, c: &Continuous, px: f64) -> f64 {
    let (r0, r1) = (c.range[0], c.range[1]);
    let mut t = if r0 == r1 { 0.5 } else { (px - r0) / (r1 - r0) };
    if c.clamp {
        t = t.clamp(0.0, 1.0);
    }
    tf.denormalize(t, c.domain)
}

/// Interpolate domains (in transformed space, so a log scale zooms geometrically) and ranges.
pub(crate) fn lerp(tf: Transform, a: &Continuous, b: &Continuous, t: f64) -> Continuous {
    let range = [datars_math::lerp(a.range[0], b.range[0], t), datars_math::lerp(a.range[1], b.range[1], t)];
    let linear = [datars_math::lerp(a.domain[0], b.domain[0], t), datars_math::lerp(a.domain[1], b.domain[1], t)];
    let domain = if tf.is_linear() || tf.mirrored(a.domain) != tf.mirrored(b.domain) {
        linear
    } else {
        let f = |x: f64, d: [f64; 2]| tf.forward(x, d);
        let d = [
            tf.inverse(datars_math::lerp(f(a.domain[0], a.domain), f(b.domain[0], b.domain), t), a.domain),
            tf.inverse(datars_math::lerp(f(a.domain[1], a.domain), f(b.domain[1], b.domain), t), a.domain),
        ];
        if d.iter().all(|x| x.is_finite()) {
            d
        } else {
            linear
        }
    };
    Continuous { domain, range, clamp: if t < 0.5 { a.clamp } else { b.clamp } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_round_trip() {
        let d = [1.0, 1000.0];
        for tf in [Transform::Linear, Transform::Log { base: 10.0 }, Transform::Pow { exponent: 2.0 }, Transform::Sqrt, Transform::Symlog { constant: 1.0 }] {
            for x in [1.0, 2.5, 10.0, 999.0] {
                let y = tf.inverse(tf.forward(x, d), d);
                assert!((y - x).abs() < 1e-9 * x.max(1.0), "{tf:?} {x} -> {y}");
            }
        }
        let neg = [-1000.0, -1.0];
        let tf = Transform::Log { base: 10.0 };
        assert!((tf.normalize(-10.0, neg) - 2.0 / 3.0).abs() < 1e-12);
        assert!(tf.forward(0.0, d).is_nan());
        assert_eq!(Transform::Symlog { constant: 1.0 }.forward(0.0, d), 0.0);
        assert_eq!(Transform::Sqrt.forward(-4.0, d), -2.0);
    }
}
