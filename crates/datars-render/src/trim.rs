use datars_math::{PathData, Vec2};

/// The portion of a path between fractions `t0` and `t1` of its total length (line reveals,
/// draw-on outlines, tracks drawn up to the clock). Curves are flattened at `tolerance`.
pub fn trim_path(path: &PathData, t0: f64, t1: f64, tolerance: f64) -> PathData {
    let (t0, t1) = (t0.clamp(0.0, 1.0), t1.clamp(0.0, 1.0));
    if t0 <= 0.0 && t1 >= 1.0 {
        return path.clone();
    }
    let mut out = PathData::new();
    if t1 <= t0 {
        return out;
    }
    let polys: Vec<Vec<Vec2>> = path
        .flatten(tolerance)
        .into_iter()
        .map(|(mut pts, closed)| {
            if closed && pts.len() > 1 {
                pts.push(pts[0]);
            }
            pts
        })
        .collect();
    let total: f64 = polys.iter().map(|p| p.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>()).sum();
    if total <= 0.0 {
        return out;
    }
    let (a, b) = (t0 * total, t1 * total);
    let mut acc = 0.0;
    for pts in polys {
        let mut started = false;
        for w in pts.windows(2) {
            let l = w[0].dist(w[1]);
            let (s0, s1) = (acc, acc + l);
            acc = s1;
            if s1 < a || s0 > b || l <= 0.0 {
                started = false;
                continue;
            }
            let fa = ((a - s0) / l).clamp(0.0, 1.0);
            let fb = ((b - s0) / l).clamp(0.0, 1.0);
            let p0 = w[0].lerp(w[1], fa);
            let p1 = w[0].lerp(w[1], fb);
            if !started {
                out.move_to(p0);
                started = true;
            }
            out.line_to(p1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn half_a_line() {
        let p = PathData::polyline(&[Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0)]);
        let h = trim_path(&p, 0.0, 0.5, 0.1);
        let b = h.bounds();
        assert_eq!((b.x, b.w, b.h), (0.0, 10.0, 0.0));
    }
}
