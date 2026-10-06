//! Sign-exact geometric predicates (Shewchuk): a floating-point fast path with a proven error
//! bound, falling back to exact arithmetic on floating-point expansions when the sign is in doubt.
//!
//! Exactness is what makes Delaunay triangulation robust on real data: grid-aligned, collinear and
//! cocircular points are the norm in charts, and there the naive determinants round to the wrong
//! sign. Everything here is plain IEEE-754 add/sub/mul (no FMA), so results are identical on every
//! target.

const EPS: f64 = f64::EPSILON / 2.0; // 2^-53
const CCW_ERR_A: f64 = (3.0 + 16.0 * EPS) * EPS;
const ICC_ERR_A: f64 = (10.0 + 96.0 * EPS) * EPS;
const SPLITTER: f64 = 134_217_729.0; // 2^27 + 1

/// Orientation in the robust-predicates convention: `(ay − cy)(bx − cx) − (ax − cx)(by − cy)`.
/// Positive when a, b, c turn clockwise in y-up axes (counter-clockwise on a y-down screen),
/// negative for the opposite turn, zero when collinear. The sign is always exact.
pub(crate) fn orient2d(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64) -> f64 {
    let left = (ay - cy) * (bx - cx);
    let right = (ax - cx) * (by - cy);
    let det = left - right;
    let sum = (left + right).abs();
    if det.abs() >= CCW_ERR_A * sum {
        return det;
    }
    let l = mul(&diff(ay, cy), &diff(bx, cx));
    let r = mul(&diff(ax, cx), &diff(by, cy));
    sign(&sub(&l, &r))
}

/// The in-circle determinant of Delaunator's `inCircle`, with an exact sign: for a triangle a, b, c
/// with `orient2d(a, b, c) > 0`, negative exactly when p lies strictly inside its circumcircle.
#[allow(clippy::too_many_arguments)]
pub(crate) fn incircle(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> f64 {
    let (adx, ady, bdx, bdy, cdx, cdy) = (ax - px, ay - py, bx - px, by - py, cx - px, cy - py);
    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;
    let alift = adx * adx + ady * ady;
    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let blift = bdx * bdx + bdy * bdy;
    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;
    let clift = cdx * cdx + cdy * cdy;
    let det = alift * (bdxcdy - cdxbdy) + blift * (cdxady - adxcdy) + clift * (adxbdy - bdxady);
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * alift
        + (cdxady.abs() + adxcdy.abs()) * blift
        + (adxbdy.abs() + bdxady.abs()) * clift;
    if det.abs() > ICC_ERR_A * permanent {
        return det;
    }
    // Exact: the same determinant over expansions. Differences of grid-aligned coordinates are
    // exact single floats, which keeps these expansions short in the common degenerate case.
    let (adx, ady, bdx, bdy, cdx, cdy) = (diff(ax, px), diff(ay, py), diff(bx, px), diff(by, py), diff(cx, px), diff(cy, py));
    let lift = |x: &[f64], y: &[f64]| add(&mul(x, x), &mul(y, y));
    let minor = |x1: &[f64], y1: &[f64], x2: &[f64], y2: &[f64]| sub(&mul(x1, y2), &mul(x2, y1));
    let ta = mul(&lift(&adx, &ady), &minor(&bdx, &bdy, &cdx, &cdy));
    let tb = mul(&lift(&bdx, &bdy), &minor(&cdx, &cdy, &adx, &ady));
    let tc = mul(&lift(&cdx, &cdy), &minor(&adx, &ady, &bdx, &bdy));
    sign(&add(&add(&ta, &tb), &tc))
}

// ── Expansion arithmetic (little-endian, non-overlapping, zero-eliminated) ─────────────

#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let bv = x - a;
    let av = x - bv;
    (x, (a - av) + (b - bv))
}

#[inline]
fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    (x, b - (x - a))
}

#[inline]
fn two_diff(a: f64, b: f64) -> (f64, f64) {
    let x = a - b;
    let bv = a - x;
    let av = x + bv;
    (x, (a - av) + (bv - b))
}

#[inline]
fn split(a: f64) -> (f64, f64) {
    let c = SPLITTER * a;
    let hi = c - (c - a);
    (hi, a - hi)
}

#[inline]
fn two_product(a: f64, b: f64) -> (f64, f64) {
    let x = a * b;
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    let err = x - ah * bh - al * bh - ah * bl;
    (x, al * bl - err)
}

/// `a − b` exactly, as an expansion.
fn diff(a: f64, b: f64) -> Vec<f64> {
    let (x, y) = two_diff(a, b);
    let mut e = Vec::with_capacity(2);
    if y != 0.0 {
        e.push(y);
    }
    if x != 0.0 || e.is_empty() {
        e.push(x);
    }
    e
}

/// `e + b` (Shewchuk's GROW-EXPANSION with zero elimination).
fn grow(e: &[f64], b: f64) -> Vec<f64> {
    let mut h = Vec::with_capacity(e.len() + 1);
    let mut q = b;
    for &x in e {
        let (s, t) = two_sum(q, x);
        if t != 0.0 {
            h.push(t);
        }
        q = s;
    }
    if q != 0.0 || h.is_empty() {
        h.push(q);
    }
    h
}

fn add(e: &[f64], f: &[f64]) -> Vec<f64> {
    let (small, big) = if e.len() < f.len() { (e, f) } else { (f, e) };
    let mut h = big.to_vec();
    for &x in small {
        h = grow(&h, x);
    }
    h
}

fn sub(e: &[f64], f: &[f64]) -> Vec<f64> {
    let neg: Vec<f64> = f.iter().map(|x| -x).collect();
    add(e, &neg)
}

/// `e · b` (SCALE-EXPANSION with zero elimination).
fn scale(e: &[f64], b: f64) -> Vec<f64> {
    let mut h = Vec::with_capacity(2 * e.len());
    let Some((&e0, rest)) = e.split_first() else { return vec![0.0] };
    let (mut q, t) = two_product(e0, b);
    if t != 0.0 {
        h.push(t);
    }
    for &x in rest {
        let (p1, p0) = two_product(x, b);
        let (s, t) = two_sum(q, p0);
        if t != 0.0 {
            h.push(t);
        }
        let (s2, t2) = fast_two_sum(p1, s);
        if t2 != 0.0 {
            h.push(t2);
        }
        q = s2;
    }
    if q != 0.0 || h.is_empty() {
        h.push(q);
    }
    h
}

fn mul(e: &[f64], f: &[f64]) -> Vec<f64> {
    let mut acc = vec![0.0];
    for &x in f {
        acc = add(&acc, &scale(e, x));
    }
    acc
}

/// The sign of an expansion as −1, 0 or 1: the sign of its most significant (last) component.
fn sign(e: &[f64]) -> f64 {
    match e.last() {
        Some(&x) if x > 0.0 => 1.0,
        Some(&x) if x < 0.0 => -1.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orient_signs() {
        // (0,0) → (1,0) → (0,1) is counter-clockwise in y-up axes: negative in this convention.
        assert!(orient2d(0.0, 0.0, 1.0, 0.0, 0.0, 1.0) < 0.0);
        assert!(orient2d(0.0, 0.0, 0.0, 1.0, 1.0, 0.0) > 0.0);
        assert_eq!(orient2d(0.0, 0.0, 1.0, 1.0, 2.0, 2.0), 0.0);
    }

    #[test]
    fn orient_is_exact_near_degeneracy() {
        // Points on the line y = x, nudged by one ulp: the naive determinant gets these wrong.
        let x = 0.5 + f64::EPSILON * 4.0;
        for k in 0..64 {
            let px = 0.5 + k as f64 * f64::EPSILON;
            let o = orient2d(px, px, 12.0, 12.0, 24.0, 24.0);
            assert_eq!(o, 0.0, "exactly collinear {px}");
        }
        let above = orient2d(x, x + f64::EPSILON, 12.0, 12.0, 24.0, 24.0);
        let below = orient2d(x, x - f64::EPSILON, 12.0, 12.0, 24.0, 24.0);
        assert!(above != 0.0 && below != 0.0 && above.signum() != below.signum());
    }

    #[test]
    fn incircle_signs_and_cocircular() {
        // Triangle with orient2d > 0: (0,0), (0,1), (1,0).
        let (ax, ay, bx, by, cx, cy) = (0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
        assert!(orient2d(ax, ay, bx, by, cx, cy) > 0.0);
        assert!(incircle(ax, ay, bx, by, cx, cy, 0.5, 0.5) < 0.0, "inside");
        assert!(incircle(ax, ay, bx, by, cx, cy, 3.0, 3.0) > 0.0, "outside");
        assert_eq!(incircle(ax, ay, bx, by, cx, cy, 1.0, 1.0), 0.0, "cocircular square corner");
        // Large offsets: cocircular grid points far from the origin are still exactly 0.
        let o = 1e9;
        assert_eq!(incircle(o, o, o, o + 1.0, o + 1.0, o, o + 1.0, o + 1.0), 0.0);
        let t = 1e-3;
        assert_eq!(incircle(t, t, t, 2.0 * t, 2.0 * t, t, 2.0 * t, 2.0 * t), 0.0);
    }

    #[test]
    fn expansions_are_exact() {
        let e = add(&diff(1e16, 1.0), &[1.0]);
        assert_eq!(sign(&sub(&e, &[1e16])), 0.0);
        let p = mul(&[3.0], &diff(0.1, 0.3));
        assert!(sign(&p) < 0.0);
    }
}
