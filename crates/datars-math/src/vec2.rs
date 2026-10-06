use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    #[inline]
    pub const fn new(x: f64, y: f64) -> Vec2 {
        Vec2 { x, y }
    }
    #[inline]
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    #[inline]
    pub fn cross(self, o: Vec2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    #[inline]
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    #[inline]
    pub fn len2(self) -> f64 {
        self.x * self.x + self.y * self.y
    }
    pub fn normalize(self) -> Vec2 {
        let l = self.len();
        if l > 0.0 { self / l } else { Vec2::ZERO }
    }
    /// Perpendicular (rotated +90° in a y-down screen space: (x, y) → (-y, x)).
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }
    #[inline]
    pub fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        Vec2::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    #[inline]
    pub fn dist(self, o: Vec2) -> f64 {
        (self - o).len()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    /// A point at `angle` (radians, clockwise from 12 o'clock, y down) and radius `r` around `c`.
    pub fn polar(c: Vec2, r: f64, angle: f64) -> Vec2 {
        let (s, co) = crate::m::sin_cos(angle);
        Vec2::new(c.x + r * s, c.y - r * co)
    }
}

impl Add for Vec2 { type Output = Vec2; #[inline] fn add(self, o: Vec2) -> Vec2 { Vec2::new(self.x + o.x, self.y + o.y) } }
impl Sub for Vec2 { type Output = Vec2; #[inline] fn sub(self, o: Vec2) -> Vec2 { Vec2::new(self.x - o.x, self.y - o.y) } }
impl Mul<f64> for Vec2 { type Output = Vec2; #[inline] fn mul(self, k: f64) -> Vec2 { Vec2::new(self.x * k, self.y * k) } }
impl Div<f64> for Vec2 { type Output = Vec2; #[inline] fn div(self, k: f64) -> Vec2 { Vec2::new(self.x / k, self.y / k) } }
impl Neg for Vec2 { type Output = Vec2; #[inline] fn neg(self) -> Vec2 { Vec2::new(-self.x, -self.y) } }
impl AddAssign for Vec2 { #[inline] fn add_assign(&mut self, o: Vec2) { self.x += o.x; self.y += o.y; } }
impl SubAssign for Vec2 { #[inline] fn sub_assign(&mut self, o: Vec2) { self.x -= o.x; self.y -= o.y; } }

impl From<(f64, f64)> for Vec2 {
    fn from(p: (f64, f64)) -> Vec2 {
        Vec2::new(p.0, p.1)
    }
}
