//! `datars-math` — deterministic numerics and 2D geometry.
//!
//! Everything numeric in the engine goes through this crate so results are bit-identical on every
//! target (docs/11-rendering.md): transcendental functions come from the pure-Rust `libm`, never the
//! platform's; hashing is stable and endian-independent; randomness is seeded.

pub mod affine;
pub mod hash;
pub mod m;
pub mod path;
pub mod rect;
pub mod rng;
pub mod vec2;

pub use affine::Affine;
pub use hash::{Hash64, StableHash};
pub use path::{FillRule, PathData, PathEl};
pub use rect::Rect;
pub use rng::{mix64, Rng};
pub use vec2::Vec2;

/// A total order for f64 (NaN sorts last, -0 == 0), for deterministic sorting.
pub fn total_cmp(a: f64, b: f64) -> std::cmp::Ordering {
    match (a.is_nan(), b.is_nan()) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal),
    }
}

/// Linear interpolation.
#[inline]
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Clamp into [lo, hi], mapping NaN to `lo`.
#[inline]
pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v.is_nan() {
        lo
    } else {
        v.max(lo).min(hi)
    }
}
