//! Routes: the path an element's centre travels. A route is an *offset* added to the straight
//! line, and every offset is exactly zero at both ends (progress 0 and 1), so settled states stay
//! exact whatever the detour (docs/05 §Route).
//!
//! Offsets live in the element's placement space (its parent's coordinates for in-place
//! elements, the root content space for elements flying between containers) and are applied as
//! a translation before the element's own transform.

use crate::easing::bounce_out;
use datars_math::{m, Vec2};
use serde::{Deserialize, Serialize};

fn arc_h() -> f64 {
    0.45
}
fn turns() -> f64 {
    0.45
}
fn hop_h() -> f64 {
    24.0
}
fn drift() -> f64 {
    40.0
}
fn bounce() -> f64 {
    1.0
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Route {
    /// No detour.
    #[default]
    Straight,
    /// Swing along an arc; `height` is the bulge as a fraction of the travel distance (bends up
    /// for sideways moves).
    Arc {
        #[serde(default = "arc_h")]
        height: f64,
    },
    /// Along x first, then y: a clean re-sort.
    Elbow,
    /// Swirl around the scene centre; `turns` is the peak rotation in full turns.
    Spiral {
        #[serde(default = "turns")]
        turns: f64,
    },
    /// Blow outward from the scene centre, then land.
    Explode,
    /// Hop up by `height` (placement units, ≈ px) while travelling — for waves.
    Hop {
        #[serde(default = "hop_h")]
        height: f64,
    },
    /// A swarm: each element drifts its own seeded way by up to `amount` (≈ px).
    Drift {
        #[serde(default)]
        seed: u64,
        #[serde(default = "drift")]
        amount: f64,
    },
    /// Lift, then drop into place; `bounce` ∈ [0, 1] blends a gravity fall into a bouncing one.
    Drop {
        #[serde(default = "bounce")]
        bounce: f64,
    },
}

/// Plan-level context for routes, in the element's placement space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteCx {
    /// The scene centre.
    pub mid: Vec2,
    /// The scene's smaller dimension.
    pub span: f64,
}

/// A 64-bit finalizer (splitmix64) — spreads key hashes into uniform bits.
pub(crate) fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform [0, 1) from hash bits.
pub(crate) fn unit(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

impl Route {
    pub fn is_straight(&self) -> bool {
        matches!(self, Route::Straight)
    }

    /// The offset from the straight line at progress `k` for an element travelling from centre
    /// `a` to centre `b`; `hash` is the element's stable key hash (drift directions, explode
    /// jitter). Exactly zero for `k ≤ 0` and `k ≥ 1`.
    pub fn offset(&self, a: Vec2, b: Vec2, k: f64, hash: u64, cx: &RouteCx) -> Vec2 {
        if !(k > 0.0 && k < 1.0) {
            return Vec2::ZERO;
        }
        let bump = m::sin(m::PI * k);
        let p = a.lerp(b, k);
        let v = match self {
            Route::Straight => Vec2::ZERO,
            Route::Arc { height } => {
                let d = b - a;
                let s = if d.x >= 0.0 { 1.0 } else { -1.0 };
                Vec2::new(d.y * height * bump * s, -d.x.abs() * height * bump)
            }
            Route::Elbow => {
                let kx = (2.0 * k).clamp(0.0, 1.0);
                let ky = (2.0 * k - 1.0).clamp(0.0, 1.0);
                Vec2::new(a.x + (b.x - a.x) * kx - p.x, a.y + (b.y - a.y) * ky - p.y)
            }
            Route::Spiral { turns } => {
                let th = turns * m::TAU * bump;
                let v = p - cx.mid;
                let (s, c) = m::sin_cos(th);
                Vec2::new(v.x * c - v.y * s - v.x, v.x * s + v.y * c - v.y)
            }
            Route::Explode => {
                let h = unit(mix64(hash));
                // Outward along one direction per element (from the centre through the middle of
                // its trip): measured from the moving point instead, the direction would flip —
                // and the element jump — where a path crosses the centre.
                let v = a.lerp(b, 0.5) - cx.mid;
                let len = v.len();
                let u = if len > 1e-6 {
                    v / len
                } else {
                    let (s, c) = m::sin_cos(h * m::TAU);
                    Vec2::new(c, s)
                };
                u * (cx.span * (0.35 + 0.35 * h) * bump)
            }
            Route::Hop { height } => Vec2::new(0.0, -height * bump),
            Route::Drift { seed, amount } => {
                let h1 = unit(mix64(hash ^ mix64(*seed)));
                let h2 = unit(mix64(hash.rotate_left(17) ^ seed.wrapping_add(0x51ED)));
                let (s, c) = m::sin_cos(h1 * m::TAU);
                let r = amount * (0.25 + 0.75 * h2) * bump;
                Vec2::new(c * r, s * r)
            }
            Route::Drop { bounce } => {
                let hgt = cx.span * 0.2;
                let y = if k < 0.4 {
                    -hgt * m::sin(m::PI / 2.0 * k / 0.4)
                } else {
                    let s = (k - 0.4) / 0.6;
                    let b = bounce.clamp(0.0, 1.0);
                    let fall = s * s + (bounce_out(s) - s * s) * b;
                    -hgt * (1.0 - fall)
                };
                Vec2::new(0.0, y)
            }
        };
        if v.is_finite() {
            v
        } else {
            Vec2::ZERO
        }
    }
}
