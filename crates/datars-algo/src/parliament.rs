//! Parliament (hemicycle) seat placement.

use crate::util::mag;
use crate::waffle::apportion;
use datars_math::{m, total_cmp, Vec2};
use serde::{Deserialize, Serialize};

/// The result of [`parliament`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParliamentLayout {
    /// `(party index, seat centre)` for every seat, ordered left → right around the arc; a party's
    /// seats are contiguous and parties follow input order.
    pub seats: Vec<(usize, Vec2)>,
    /// The largest seat radius that keeps neighbouring seats (along a row and between rows) from
    /// overlapping. Draw somewhat smaller (e.g. × 0.8) for visible gaps.
    pub seat_radius: f64,
    /// Number of rows used.
    pub rows: usize,
}

/// Places `Σ seats` seats on concentric half-circle rows between `r_inner` and `r_outer` around
/// `center`, opening upward (y down: the arc is above `center`), and assigns them to parties left
/// to right.
///
/// Rows sit at the middles of `rows` equal bands of the annulus, so every seat centre is strictly
/// between the two radii. Seats per row are proportional to the row's arc length (largest
/// remainder, so they sum exactly). Each row's seats are spread evenly from the left end of the
/// arc (on the baseline) to the right end; then all seats are sorted by angle, left first (ties:
/// inner row first), and handed to parties in order — each party gets a wedge.
///
/// `rows: None` picks the row count that makes seat spacing along the rows match the spacing
/// between rows (≈ square packing): `round(√(total · thickness / (π · mean radius)))`.
///
/// More rows than that crowd the rows together while their seats drift apart along them, until
/// the nearest neighbours are in the next row and the arcs dissolve into radial spokes. So with
/// more rows than the annulus packs squarely, the inner radius shrinks (to no less than
/// [`MIN_INNER`] × the outer one) until seats along a row sit no farther apart than the rows.
///
/// Totality: zero seats → empty layout; radii are sanitised (swapped if reversed; a zero-thickness
/// annulus gets one row). Deterministic.
pub fn parliament(seats: &[usize], rows: Option<usize>, center: Vec2, r_inner: f64, r_outer: f64) -> ParliamentLayout {
    let total: usize = seats.iter().fold(0usize, |a, &s| a.saturating_add(s));
    if total == 0 {
        return ParliamentLayout::default();
    }
    let c = if center.is_finite() { center } else { Vec2::ZERO };
    let (mut r0, mut r1) = (mag(r_inner), mag(r_outer));
    if r1 < r0 {
        std::mem::swap(&mut r0, &mut r1);
    }
    let auto = auto_rows(total, r0, r1);
    let rows = rows.unwrap_or(auto).clamp(1, total);
    if rows > auto && r1 > 0.0 {
        // Square packing: spacing along a row, π·mean·rows/total, equals the spacing between rows,
        // (r1 − r0)/rows, when r0 = r1·(1 − k)/(1 + k) with k = π·rows²/(2·total).
        let rf = rows as f64;
        let k = m::PI * rf * rf / (2.0 * total as f64);
        r0 = r0.min(r1 * ((1.0 - k) / (1.0 + k)).max(MIN_INNER));
    }
    let thickness = r1 - r0;
    let dr = thickness / rows as f64;
    let radii: Vec<f64> = (0..rows).map(|k| r0 + (k as f64 + 0.5) * dr).collect();
    // A zero-radius row (a point) still takes seats if it's the only row.
    let weights: Vec<f64> = radii.iter().map(|&r| if r > 0.0 { r } else { 1.0 }).collect();
    let per_row = apportion(&weights, total);
    let mut pos: Vec<(f64, usize, Vec2)> = Vec::with_capacity(total);
    let mut min_gap = if rows > 1 { dr } else { f64::INFINITY };
    for (k, &cnt) in per_row.iter().enumerate() {
        let r = radii[k];
        for j in 0..cnt {
            let a = if cnt > 1 { m::PI * (1.0 - j as f64 / (cnt - 1) as f64) } else { m::PI / 2.0 };
            let (s, co) = m::sin_cos(a);
            pos.push((a, k, Vec2::new(c.x + r * co, c.y - r * s)));
        }
        if cnt > 1 {
            let chord = 2.0 * r * m::sin(m::PI / (cnt - 1) as f64 / 2.0);
            min_gap = min_gap.min(chord);
        }
    }
    pos.sort_by(|a, b| total_cmp(b.0, a.0).then(a.1.cmp(&b.1)));
    let mut out = Vec::with_capacity(total);
    let mut it = pos.into_iter();
    for (party, &n) in seats.iter().enumerate() {
        for _ in 0..n {
            if let Some((_, _, p)) = it.next() {
                out.push((party, p));
            }
        }
    }
    let seat_radius = if min_gap.is_finite() { min_gap / 2.0 } else { thickness.max(r1) / 2.0 };
    ParliamentLayout { seats: out, seat_radius, rows }
}

/// The smallest inner radius (a fraction of the outer one) [`parliament`] shrinks to for many
/// rows: the middle keeps room for a label (the seat count) instead of filling to a point.
pub const MIN_INNER: f64 = 0.12;

/// The row count that packs `total` seats about squarely between `r0` and `r1`.
fn auto_rows(total: usize, r0: f64, r1: f64) -> usize {
    let (thickness, mean) = (r1 - r0, (r0 + r1) / 2.0);
    if thickness > 0.0 && mean > 0.0 {
        (total as f64 * thickness / (m::PI * mean)).sqrt().round() as usize
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_seat_count_inside_annulus_and_ordered() {
        let seats = [120, 45, 3, 0, 181];
        let c = Vec2::new(400.0, 400.0);
        let l = parliament(&seats, None, c, 120.0, 380.0);
        assert_eq!(l.seats.len(), 349);
        for (party, n) in seats.iter().enumerate() {
            assert_eq!(l.seats.iter().filter(|s| s.0 == party).count(), *n);
        }
        for &(_, p) in &l.seats {
            let d = p.dist(c);
            assert!(d > 120.0 && d < 380.0, "seat at radius {d}");
            assert!(p.y <= c.y + 1e-9, "above the baseline");
        }
        // Parties are contiguous and go left → right: party indices never decrease along the list,
        // and angles never increase.
        for w in l.seats.windows(2) {
            assert!(w[0].0 <= w[1].0);
            let ang = |p: Vec2| m::atan2(c.y - p.y, p.x - c.x);
            assert!(ang(w[0].1) >= ang(w[1].1) - 1e-12);
        }
        // Seats of radius seat_radius don't overlap.
        let r = l.seat_radius;
        assert!(r > 0.0);
        for i in 0..l.seats.len() {
            for j in i + 1..l.seats.len() {
                assert!(l.seats[i].1.dist(l.seats[j].1) >= 2.0 * r - 1e-9, "{i} {j}");
            }
        }
        assert!(l.rows > 3 && l.rows < 15, "auto rows {}", l.rows);
    }

    #[test]
    fn explicit_rows_and_degenerate() {
        let l = parliament(&[5, 5], Some(2), Vec2::ZERO, 10.0, 20.0);
        assert_eq!(l.rows, 2);
        assert_eq!(l.seats.len(), 10);
        assert!(parliament(&[], None, Vec2::ZERO, 1.0, 2.0).seats.is_empty());
        assert!(parliament(&[0, 0], None, Vec2::ZERO, 1.0, 2.0).seats.is_empty());
        let one = parliament(&[1], None, Vec2::new(5.0, 5.0), 0.0, 0.0);
        assert_eq!(one.seats.len(), 1);
        assert!(one.seats[0].1.is_finite());
        let swapped = parliament(&[10], Some(50), Vec2::ZERO, 20.0, 10.0);
        assert_eq!(swapped.rows, 10, "rows clamp to the seat count");
        // Ten rows for ten seats: the annulus widens inward, never past MIN_INNER.
        assert!(swapped.seats.iter().all(|s| s.1.len() > 20.0 * MIN_INNER && s.1.len() < 20.0));
    }

    /// Mean spacing of seats along their rows, and the spacing between rows.
    fn spacings(l: &ParliamentLayout, c: Vec2) -> (f64, f64) {
        let mut radii: Vec<f64> = l.seats.iter().map(|s| (s.1.dist(c) * 1e6).round() / 1e6).collect();
        radii.sort_by(|a, b| total_cmp(*a, *b));
        radii.dedup();
        let along: Vec<f64> = radii
            .iter()
            .map(|&r| {
                let n = l.seats.iter().filter(|s| (s.1.dist(c) - r).abs() < 1e-5).count();
                m::PI * r / (n.max(2) - 1) as f64
            })
            .collect();
        (along.iter().sum::<f64>() / along.len() as f64, radii[1] - radii[0])
    }

    #[test]
    fn many_rows_keep_their_arcs() {
        // A 151-seat house in the box a 640 × 320 chart gives it, asked for more rows than the
        // annulus packs squarely: the rows move inward, so seats along a row stay about as close
        // as the rows are (before, 2–2.4× as far apart: radial spokes, not arcs). Ten rows would
        // need an inner radius below MIN_INNER: nearly square.
        let c = Vec2::new(300.0, 290.0);
        let auto = parliament(&[18, 41, 12, 36, 29, 15], None, c, 106.0, 278.0);
        let (along, between) = spacings(&auto, c);
        assert!(along / between > 0.8 && along / between < 1.25, "auto rows ({}) pack squarely: {along} vs {between}", auto.rows);
        for (rows, most) in [(9, 1.25), (10, 1.5)] {
            let l = parliament(&[18, 41, 12, 36, 29, 15], Some(rows), c, 106.0, 278.0);
            assert_eq!((l.rows, l.seats.len()), (rows, 151));
            let (along, between) = spacings(&l, c);
            assert!(along / between < most, "{rows} rows: {along} along a row vs {between} between rows");
            assert!(l.seats.iter().all(|s| s.1.dist(c) > 278.0 * MIN_INNER && s.1.dist(c) < 278.0));
        }
        // Fewer rows than automatic keep the annulus given: long arcs, spaced apart.
        let five = parliament(&[151], Some(5), c, 106.0, 278.0);
        assert!(five.seats.iter().all(|s| s.1.dist(c) > 106.0));
    }
}
