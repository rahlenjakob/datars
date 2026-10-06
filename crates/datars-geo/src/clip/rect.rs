//! Rectangle clipping (d3-geo `clip/rectangle.js` + Liang–Barsky `clip/line.js`), planar.
//!
//! Lines are cut into the runs that lie inside. Polygons are cut into visible segments and rebuilt
//! by the shared rejoin walk along the rectangle's boundary, so a concave coastline that crosses a
//! tile edge twenty times becomes the right number of separate rings, with holes kept. Requires
//! exteriors with positive signed area (`datars_math::path::signed_area`) and holes negative;
//! the public wrappers in `clip` normalize winding first.

use super::rejoin::{rejoin, SPoint};

const CLIP_MAX: f64 = 1e9;

#[derive(Clone, Copy, Debug)]
pub(crate) struct RectClip {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub eps: f64,
}

impl RectClip {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64, eps: f64) -> RectClip {
        RectClip { x0, y0, x1, y1, eps }
    }

    #[inline]
    pub fn visible(&self, x: f64, y: f64) -> bool {
        self.x0 <= x && x <= self.x1 && self.y0 <= y && y <= self.y1
    }

    /// Which side of the rectangle a boundary point is on (0 left, 1 top, 2 right, 3 bottom,
    /// shifted by direction as in d3).
    fn corner(&self, p: SPoint, direction: i32) -> i32 {
        if (p.x - self.x0).abs() < self.eps {
            if direction > 0 { 0 } else { 3 }
        } else if (p.x - self.x1).abs() < self.eps {
            if direction > 0 { 2 } else { 1 }
        } else if (p.y - self.y0).abs() < self.eps {
            if direction > 0 { 1 } else { 0 }
        } else if direction > 0 {
            3
        } else {
            2
        }
    }

    fn compare_point(&self, a: SPoint, b: SPoint) -> f64 {
        let (ca, cb) = (self.corner(a, 1), self.corner(b, 1));
        if ca != cb {
            (ca - cb) as f64
        } else {
            match ca {
                0 => b.y - a.y,
                1 => a.x - b.x,
                2 => a.y - b.y,
                _ => b.x - a.x,
            }
        }
    }

    fn interpolate(&self, from: Option<SPoint>, to: Option<SPoint>, direction: i32, out: &mut Vec<SPoint>) {
        let emit = |a: i32, out: &mut Vec<SPoint>| {
            out.push(SPoint::new(if a == 0 || a == 3 { self.x0 } else { self.x1 }, if a > 1 { self.y1 } else { self.y0 }));
        };
        match (from, to) {
            (Some(f), Some(t)) => {
                let a0 = self.corner(f, direction);
                let a1 = self.corner(t, direction);
                if a0 != a1 || ((self.compare_point(f, t) < 0.0) ^ (direction > 0)) {
                    let mut a = a0;
                    loop {
                        emit(a, out);
                        a = (a + direction + 4) % 4;
                        if a == a1 {
                            break;
                        }
                    }
                } else {
                    out.push(SPoint::new(t.x, t.y));
                }
            }
            _ => {
                let mut a = 0;
                loop {
                    emit(a, out);
                    a = (a + direction + 4) % 4;
                    if a == 0 {
                        break;
                    }
                }
            }
        }
    }

    /// Winding number of the polygon around the rectangle's bottom-left corner (x0, y1): nonzero
    /// means the whole rectangle is inside when no ring crosses it.
    fn polygon_inside(&self, polygon: &[Vec<SPoint>]) -> i32 {
        let mut winding = 0;
        for ring in polygon {
            for w in ring.windows(2) {
                let (a0, a1, b0, b1) = (w[0].x, w[0].y, w[1].x, w[1].y);
                if a1 <= self.y1 {
                    if b1 > self.y1 && (b0 - a0) * (self.y1 - a1) > (b1 - a1) * (self.x0 - a0) {
                        winding += 1;
                    }
                } else if b1 <= self.y1 && (b0 - a0) * (self.y1 - a1) < (b1 - a1) * (self.x0 - a0) {
                    winding -= 1;
                }
            }
        }
        winding
    }

    /// Cut a polyline (closed rings pass their first point again at the end) into visible runs.
    /// Returns the runs, whether no edge crossed the boundary, and the visibility of the first and
    /// last points.
    fn cut(&self, pts: &[SPoint]) -> (Vec<Vec<SPoint>>, bool, bool, bool) {
        let mut out: Vec<Vec<SPoint>> = Vec::new();
        let mut clean = true;
        let (mut xp, mut yp, mut vp) = (f64::NAN, f64::NAN, false);
        let mut first_visible = false;
        for (i, p) in pts.iter().enumerate() {
            let (mut x, mut y) = (p.x, p.y);
            let v = self.visible(x, y);
            if i == 0 {
                first_visible = v;
                if v {
                    out.push(vec![SPoint::new(x, y)]);
                }
            } else if v && vp {
                if let Some(s) = out.last_mut() {
                    s.push(SPoint::new(x, y));
                }
            } else {
                let mut a = (xp.clamp(-CLIP_MAX, CLIP_MAX), yp.clamp(-CLIP_MAX, CLIP_MAX));
                let mut b = (x.clamp(-CLIP_MAX, CLIP_MAX), y.clamp(-CLIP_MAX, CLIP_MAX));
                (x, y) = b;
                if clip_segment(&mut a, &mut b, self.x0, self.y0, self.x1, self.y1) {
                    if !vp {
                        out.push(vec![SPoint::new(a.0, a.1)]);
                    }
                    if let Some(s) = out.last_mut() {
                        s.push(SPoint::new(b.0, b.1));
                    }
                    clean = false;
                } else if v {
                    out.push(vec![SPoint::new(x, y)]);
                    clean = false;
                }
            }
            xp = x;
            yp = y;
            vp = v;
        }
        (out, clean, first_visible, vp)
    }

    /// Clip an open polyline.
    pub fn clip_line(&self, pts: &[SPoint]) -> Vec<Vec<SPoint>> {
        self.cut(pts).0.into_iter().filter(|s| s.len() > 1).collect()
    }

    /// Clip one polygon (rings without closing duplicates, exteriors positive) into rings.
    pub fn clip_polygon(&self, rings: &[Vec<SPoint>]) -> Vec<Vec<SPoint>> {
        let mut segments: Vec<Vec<SPoint>> = Vec::new();
        let mut polygon: Vec<Vec<SPoint>> = Vec::new();
        let mut clean = true;
        for ring in rings {
            if ring.is_empty() {
                continue;
            }
            let mut closed = ring.clone();
            closed.push(ring[0]);
            let (mut runs, ring_clean, first_v, last_v) = self.cut(&closed);
            clean &= ring_clean;
            if first_v && last_v && runs.len() > 1 {
                let last = runs.pop().unwrap_or_default();
                let first = runs.remove(0);
                runs.push(last.into_iter().chain(first).collect());
            }
            segments.extend(runs);
            polygon.push(closed);
        }
        let start_inside = self.polygon_inside(&polygon) != 0;
        let clean_inside = clean && start_inside;
        let mut out = Vec::new();
        if clean_inside {
            let mut r = Vec::new();
            self.interpolate(None, None, 1, &mut r);
            out.push(r);
        }
        if !segments.is_empty() {
            let cmp = |a: &SPoint, b: &SPoint| datars_math::total_cmp(self.compare_point(*a, *b), 0.0);
            let interp = |f: Option<SPoint>, t: Option<SPoint>, d: i32, o: &mut Vec<SPoint>| self.interpolate(f, t, d, o);
            rejoin(segments, &cmp, start_inside, &interp, self.eps, &mut out);
        }
        out
    }
}

/// Liang–Barsky: clip segment a–b to the rectangle in place; false if it misses entirely.
fn clip_segment(a: &mut (f64, f64), b: &mut (f64, f64), x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
    let (ax, ay) = *a;
    let (dx, dy) = (b.0 - ax, b.1 - ay);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    // Each edge: p·t <= q form, following d3's branch structure.
    let mut edge = |r: f64, d: f64, lower: bool| -> bool {
        if d == 0.0 {
            return if lower { r <= 0.0 } else { r >= 0.0 };
        }
        let r = r / d;
        if (d < 0.0) == lower {
            if r < t0 {
                return false;
            }
            if r < t1 {
                t1 = r;
            }
        } else {
            if r > t1 {
                return false;
            }
            if r > t0 {
                t0 = r;
            }
        }
        true
    };
    if !edge(x0 - ax, dx, true) || !edge(x1 - ax, dx, false) || !edge(y0 - ay, dy, true) || !edge(y1 - ay, dy, false) {
        return false;
    }
    if t0 > 0.0 {
        *a = (ax + t0 * dx, ay + t0 * dy);
    }
    if t1 < 1.0 {
        *b = (ax + t1 * dx, ay + t1 * dy);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn liang_barsky() {
        let mut a = (-5.0, 5.0);
        let mut b = (15.0, 5.0);
        assert!(clip_segment(&mut a, &mut b, 0.0, 0.0, 10.0, 10.0));
        assert_eq!((a, b), ((0.0, 5.0), (10.0, 5.0)));
        let (mut a, mut b) = ((-5.0, -5.0), (-1.0, -1.0));
        assert!(!clip_segment(&mut a, &mut b, 0.0, 0.0, 10.0, 10.0));
        let (mut a, mut b) = ((2.0, 2.0), (3.0, 3.0));
        assert!(clip_segment(&mut a, &mut b, 0.0, 0.0, 10.0, 10.0));
        assert_eq!((a, b), ((2.0, 2.0), (3.0, 3.0)));
        // Vertical segment outside on x.
        let (mut a, mut b) = ((-1.0, 0.0), (-1.0, 5.0));
        assert!(!clip_segment(&mut a, &mut b, 0.0, 0.0, 10.0, 10.0));
    }
}
