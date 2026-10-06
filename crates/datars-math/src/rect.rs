use crate::Vec2;
use serde::{Deserialize, Serialize};

/// An axis-aligned rectangle (x, y, w, h).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn from_points(a: Vec2, b: Vec2) -> Rect {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
    /// The empty rect used as a union identity.
    pub fn empty() -> Rect {
        Rect::new(f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY)
    }
    pub fn is_empty(&self) -> bool {
        !(self.w >= 0.0 && self.h >= 0.0) || !self.x.is_finite()
    }
    pub fn x1(&self) -> f64 {
        self.x + self.w
    }
    pub fn y1(&self) -> f64 {
        self.y + self.h
    }
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x1() && p.y >= self.y && p.y <= self.y1()
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = self.x1().max(o.x1());
        let y1 = self.y1().max(o.y1());
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
    pub fn include(&self, p: Vec2) -> Rect {
        if self.is_empty() {
            return Rect::new(p.x, p.y, 0.0, 0.0);
        }
        self.union(&Rect::new(p.x, p.y, 0.0, 0.0))
    }
    pub fn intersects(&self, o: &Rect) -> bool {
        self.x <= o.x1() && o.x <= self.x1() && self.y <= o.y1() && o.y <= self.y1()
    }
    pub fn intersect(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.x1().min(o.x1());
        let y1 = self.y1().min(o.y1());
        (x1 >= x0 && y1 >= y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
    pub fn inset(&self, d: f64) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
}
