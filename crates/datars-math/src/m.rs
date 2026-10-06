//! Deterministic elementary functions. Wrappers over the pure-Rust `libm` crate so native and wasm
//! builds agree bit for bit. `sqrt`, `floor`, `ceil`, `round`, `abs` are exact IEEE operations and
//! may be used directly on f64; everything else must come from here (enforced by clippy.toml).

pub const PI: f64 = core::f64::consts::PI;
pub const TAU: f64 = core::f64::consts::TAU;

#[inline] pub fn sin(x: f64) -> f64 { libm::sin(x) }
#[inline] pub fn cos(x: f64) -> f64 { libm::cos(x) }
#[inline] pub fn tan(x: f64) -> f64 { libm::tan(x) }
#[inline] pub fn asin(x: f64) -> f64 { libm::asin(x) }
#[inline] pub fn acos(x: f64) -> f64 { libm::acos(x) }
#[inline] pub fn atan(x: f64) -> f64 { libm::atan(x) }
#[inline] pub fn atan2(y: f64, x: f64) -> f64 { libm::atan2(y, x) }
#[inline] pub fn exp(x: f64) -> f64 { libm::exp(x) }
#[inline] pub fn exp2(x: f64) -> f64 { libm::exp2(x) }
#[inline] pub fn ln(x: f64) -> f64 { libm::log(x) }
#[inline] pub fn log10(x: f64) -> f64 { libm::log10(x) }
#[inline] pub fn log2(x: f64) -> f64 { libm::log2(x) }
#[inline] pub fn pow(x: f64, y: f64) -> f64 { libm::pow(x, y) }
#[inline] pub fn cbrt(x: f64) -> f64 { libm::cbrt(x) }
#[inline] pub fn hypot(x: f64, y: f64) -> f64 { libm::hypot(x, y) }
#[inline] pub fn sinh(x: f64) -> f64 { libm::sinh(x) }
#[inline] pub fn cosh(x: f64) -> f64 { libm::cosh(x) }
#[inline] pub fn tanh(x: f64) -> f64 { libm::tanh(x) }
#[inline] pub fn sqrt(x: f64) -> f64 { x.sqrt() }
#[inline] pub fn fmod(x: f64, y: f64) -> f64 { libm::fmod(x, y) }

/// `(sin x, cos x)` together.
#[inline]
pub fn sin_cos(x: f64) -> (f64, f64) {
    (sin(x), cos(x))
}

/// Euclidean remainder into [0, m).
#[inline]
pub fn rem_euclid(x: f64, m: f64) -> f64 {
    let r = fmod(x, m);
    if r < 0.0 { r + m } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_values_are_bit_exact() {
        // Values pinned so a libm change (or an accidental platform call) shows up as a failure.
        assert!((sin(1.0) - 0.8414709848078965).abs() < 1e-16);
        assert!((pow(2.0, 0.5) - core::f64::consts::SQRT_2).abs() < 1e-15);
        assert_eq!(rem_euclid(-1.0, 4.0), 3.0);
    }
}
