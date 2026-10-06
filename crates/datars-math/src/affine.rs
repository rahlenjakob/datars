use crate::Vec2;
use serde::{Deserialize, Serialize};

/// A 2D affine transform `[a c e; b d f]`: x' = a·x + c·y + e, y' = b·x + d·y + f.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine(pub [f64; 6]);

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(x: f64, y: f64) -> Affine {
        Affine([1.0, 0.0, 0.0, 1.0, x, y])
    }
    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }
    pub fn rotate(rad: f64) -> Affine {
        let (s, c) = crate::m::sin_cos(rad);
        Affine([c, s, -s, c, 0.0, 0.0])
    }
    /// `self` then `o` (apply self first): `o * self`.
    pub fn then(self, o: Affine) -> Affine {
        o.mul(self)
    }
    /// Matrix product `self * o` (apply `o` first).
    pub fn mul(self, o: Affine) -> Affine {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = o.0;
        Affine([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }
    #[inline]
    pub fn apply(&self, p: Vec2) -> Vec2 {
        let [a, b, c, d, e, f] = self.0;
        Vec2::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
    }
    /// Apply to a vector (no translation).
    #[inline]
    pub fn apply_vec(&self, v: Vec2) -> Vec2 {
        let [a, b, c, d, ..] = self.0;
        Vec2::new(a * v.x + c * v.y, b * v.x + d * v.y)
    }
    pub fn determinant(&self) -> f64 {
        self.0[0] * self.0[3] - self.0[1] * self.0[2]
    }
    /// The average linear scale factor (sqrt of |det|) — for stroke widths under transforms.
    pub fn scale_factor(&self) -> f64 {
        self.determinant().abs().sqrt()
    }
    pub fn inverse(&self) -> Option<Affine> {
        let det = self.determinant();
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let [a, b, c, d, e, f] = self.0;
        let id = 1.0 / det;
        Some(Affine([d * id, -b * id, -c * id, a * id, (c * f - d * e) * id, (b * e - a * f) * id]))
    }
    pub fn is_identity(&self) -> bool {
        *self == Affine::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compose_and_invert() {
        let t = Affine::translate(10.0, 5.0).mul(Affine::scale(2.0, 3.0));
        let p = t.apply(Vec2::new(1.0, 1.0));
        assert_eq!(p, Vec2::new(12.0, 8.0));
        let back = t.inverse().unwrap().apply(p);
        assert!((back.x - 1.0).abs() < 1e-12 && (back.y - 1.0).abs() < 1e-12);
        let then = Affine::scale(2.0, 3.0).then(Affine::translate(10.0, 5.0));
        assert_eq!(then, t);
    }
}
