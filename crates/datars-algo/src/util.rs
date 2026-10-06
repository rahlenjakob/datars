//! Small shared helpers. Crate-private: the public surface is the algorithms.

use datars_math::{Rect, Vec2};

/// A usable magnitude: finite and positive, otherwise 0. Every algorithm that sizes things by value
/// (pie, treemap, pack, waffle, …) funnels its inputs through this, so NaN, ±∞ and negatives never
/// leak into geometry.
#[inline]
pub(crate) fn mag(v: f64) -> f64 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        0.0
    }
}

/// Finite or 0 (keeps the sign).
#[inline]
pub(crate) fn finite(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Whether a rect has finite coordinates and a non-negative size.
#[inline]
pub(crate) fn rect_ok(r: &Rect) -> bool {
    r.x.is_finite() && r.y.is_finite() && r.w.is_finite() && r.h.is_finite() && r.w >= 0.0 && r.h >= 0.0
}

/// Whether a rect is usable as a non-empty area: finite with positive width and height.
#[inline]
pub(crate) fn rect_area_ok(r: &Rect) -> bool {
    rect_ok(r) && r.w > 0.0 && r.h > 0.0
}

/// `t` raised to the 8th power by repeated squaring (exact, target-independent — unlike `powi`).
#[inline]
pub(crate) fn pow8(t: f64) -> f64 {
    let t2 = t * t;
    let t4 = t2 * t2;
    t4 * t4
}

/// Bounding box of all finite points of all rings as `(min, max)`, or `None` when there are none.
pub(crate) fn rings_bbox(rings: &[Vec<Vec2>]) -> Option<(Vec2, Vec2)> {
    let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
    let mut hi = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut any = false;
    for p in rings.iter().flatten().filter(|p| p.is_finite()) {
        lo.x = lo.x.min(p.x);
        lo.y = lo.y.min(p.y);
        hi.x = hi.x.max(p.x);
        hi.y = hi.y.max(p.y);
        any = true;
    }
    any.then_some((lo, hi))
}

/// An indexed binary max-heap over `f64` keys with decrease/increase-key, ties broken by the larger
/// index first (any fixed rule works; this one keeps results independent of insertion history).
pub(crate) struct IndexedHeap {
    heap: Vec<u32>,
    pos: Vec<u32>,
    key: Vec<f64>,
}

const NOT_IN_HEAP: u32 = u32::MAX;

impl IndexedHeap {
    pub(crate) fn new(keys: Vec<f64>) -> IndexedHeap {
        let n = keys.len();
        let mut h = IndexedHeap { heap: (0..n as u32).collect(), pos: (0..n as u32).collect(), key: keys };
        for i in (0..n / 2).rev() {
            h.sift_down(i);
        }
        h
    }
    #[inline]
    fn above(&self, a: u32, b: u32) -> bool {
        let (ka, kb) = (self.key[a as usize], self.key[b as usize]);
        ka > kb || (ka == kb && a > b)
    }
    fn swap(&mut self, i: usize, j: usize) {
        self.heap.swap(i, j);
        self.pos[self.heap[i] as usize] = i as u32;
        self.pos[self.heap[j] as usize] = j as u32;
    }
    fn sift_up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) / 2;
            if self.above(self.heap[i], self.heap[p]) {
                self.swap(i, p);
                i = p;
            } else {
                break;
            }
        }
    }
    fn sift_down(&mut self, mut i: usize) {
        let n = self.heap.len();
        loop {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let mut m = i;
            if l < n && self.above(self.heap[l], self.heap[m]) {
                m = l;
            }
            if r < n && self.above(self.heap[r], self.heap[m]) {
                m = r;
            }
            if m == i {
                break;
            }
            self.swap(i, m);
            i = m;
        }
    }
    pub(crate) fn pop(&mut self) -> Option<usize> {
        let top = *self.heap.first()?;
        let last = self.heap.len() - 1;
        self.swap(0, last);
        self.heap.pop();
        self.pos[top as usize] = NOT_IN_HEAP;
        if !self.heap.is_empty() {
            self.sift_down(0);
        }
        Some(top as usize)
    }
    pub(crate) fn set(&mut self, i: usize, k: f64) {
        let p = self.pos[i];
        let old = self.key[i];
        self.key[i] = k;
        if p == NOT_IN_HEAP {
            return;
        }
        if k > old {
            self.sift_up(p as usize);
        } else {
            self.sift_down(p as usize);
        }
    }
    pub(crate) fn key(&self, i: usize) -> f64 {
        self.key[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn heap_pops_in_key_order_and_updates() {
        let mut h = IndexedHeap::new(vec![3.0, 1.0, 4.0, 1.0, 5.0]);
        h.set(1, 10.0);
        h.set(4, 0.0);
        let order: Vec<usize> = std::iter::from_fn(|| h.pop()).collect();
        assert_eq!(order, vec![1, 2, 0, 3, 4]);
    }
    #[test]
    fn mag_and_pow8() {
        assert_eq!(mag(f64::NAN), 0.0);
        assert_eq!(mag(-2.0), 0.0);
        assert_eq!(mag(2.0), 2.0);
        assert_eq!(pow8(2.0), 256.0);
    }
}
