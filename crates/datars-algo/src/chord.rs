//! Chord layout: groups around a circle, each as long as the flows touching it, and a ribbon per
//! flow between the two groups it joins (d3.chordDirected, a ribbon per link rather than per pair).

use crate::util::mag;
use datars_math::{m, PathData, Vec2};

/// One group's arc: angles in radians, clockwise from 12 o'clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChordGroup {
    pub a0: f64,
    pub a1: f64,
    /// The sum of the flows touching the group (out and in; a flow to itself counts twice).
    pub value: f64,
}

/// One link's ribbon: the arc it occupies on its source group and on its target group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChordRibbon {
    pub source: (f64, f64),
    pub target: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChordLayout {
    /// Indexed like the groups `0..n`.
    pub groups: Vec<ChordGroup>,
    /// Indexed like `links`.
    pub ribbons: Vec<ChordRibbon>,
}

/// Chord layout of `n` groups joined by `links` `(source, target, value)`.
///
/// Groups go clockwise in index order from 12 o'clock, `pad` radians apart (the first starts at
/// `pad / 2`, so the gaps sit symmetrically), each spanning a share of the rest ∝ its value — the
/// sum of the links touching it, so every link is as wide at both ends. Within a group the link
/// ends are ordered so ribbons don't cross: ends toward the group just counter-clockwise come
/// first, those toward the next group clockwise last, and several links between the same two
/// groups nest (ascending link order at the lower-indexed group, descending at the other).
///
/// Links with an endpoint out of range are left out (a zero ribbon at 0); non-finite or negative
/// values count as 0; `pad` is clamped so the gaps take at most half the circle. When every value
/// is 0 the groups split the circle evenly and the ribbons are empty. Deterministic.
pub fn chord(n: usize, links: &[(usize, usize, f64)], pad: f64) -> ChordLayout {
    let mut ribbons = vec![ChordRibbon { source: (0.0, 0.0), target: (0.0, 0.0) }; links.len()];
    if n == 0 {
        return ChordLayout { groups: Vec::new(), ribbons };
    }
    let val = |k: usize| if links[k].0 < n && links[k].1 < n { mag(links[k].2) } else { 0.0 };
    // Each group's link ends: (offset of the other group clockwise, nesting rank, link, is source).
    let mut ends: Vec<Vec<(usize, i64, usize, bool)>> = vec![Vec::new(); n];
    for (k, &(s, t, _)) in links.iter().enumerate() {
        if s >= n || t >= n {
            continue;
        }
        let rank = |here: usize, other: usize| if here <= other { k as i64 } else { -(k as i64) };
        ends[s].push(((t + n - s) % n, rank(s, t), k, true));
        ends[t].push(((s + n - t) % n, rank(t, s), k, false));
    }
    let value: Vec<f64> = ends.iter().map(|e| e.iter().map(|x| val(x.2)).sum()).collect();
    let total: f64 = value.iter().sum();
    let pad = mag(pad).min(m::PI / n as f64);
    let (k, dx) = if total > 0.0 { ((m::TAU - pad * n as f64).max(0.0) / total, pad) } else { (0.0, m::TAU / n as f64) };
    let mut groups = Vec::with_capacity(n);
    let mut x = if total > 0.0 { pad / 2.0 } else { 0.0 };
    for (g, list) in ends.iter_mut().enumerate() {
        // Far counter-clockwise neighbours first (offset n-1), the next group clockwise last
        // (offset 1), self-links (offset 0) at the very end.
        list.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(b.3.cmp(&a.3)));
        let x0 = x;
        for &(_, _, l, is_source) in list.iter() {
            let span = (x, x + val(l) * k);
            x = span.1;
            if is_source {
                ribbons[l].source = span;
            } else {
                ribbons[l].target = span;
            }
        }
        groups.push(ChordGroup { a0: x0, a1: x, value: value[g] });
        x += dx;
    }
    ChordLayout { groups, ribbons }
}

/// A ribbon's outline around centre `c` at radius `r`: along the source arc, a quadratic curve
/// through the centre to the target arc, along it, and back — d3.ribbon.
pub fn chord_ribbon(c: Vec2, r: f64, rb: &ChordRibbon) -> PathData {
    let mut p = PathData::new();
    let r = mag(r);
    let (s0, s1) = rb.source;
    let (t0, t1) = rb.target;
    p.arc(c, r, s0, s1, true);
    if (s0, s1) != (t0, t1) {
        p.quad_to(c, Vec2::polar(c, r, t0));
        p.arc(c, r, t0, t1, false);
    }
    p.quad_to(c, Vec2::polar(c, r, s0));
    p.close();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_are_proportional_and_padded() {
        // 0 → 1 (3), 1 → 2 (1), 2 → 0 (2): group values 5, 4, 3.
        let links = [(0, 1, 3.0), (1, 2, 1.0), (2, 0, 2.0)];
        let lay = chord(3, &links, 0.1);
        let v: Vec<f64> = lay.groups.iter().map(|g| g.value).collect();
        assert_eq!(v, vec![5.0, 4.0, 3.0]);
        let k = (m::TAU - 0.3) / 12.0;
        for g in &lay.groups {
            assert!(((g.a1 - g.a0) - g.value * k).abs() < 1e-12);
        }
        assert!((lay.groups[0].a0 - 0.05).abs() < 1e-12);
        assert!((lay.groups[1].a0 - lay.groups[0].a1 - 0.1).abs() < 1e-12, "a pad between groups");
        assert!((lay.groups[2].a1 + 0.05 - m::TAU).abs() < 1e-9, "the last gap closes the circle");
        // Each ribbon as wide at both ends, inside its groups.
        for (rb, &(s, t, w)) in lay.ribbons.iter().zip(&links) {
            assert!(((rb.source.1 - rb.source.0) - w * k).abs() < 1e-12);
            assert!(((rb.target.1 - rb.target.0) - w * k).abs() < 1e-12);
            let (gs, gt) = (lay.groups[s], lay.groups[t]);
            assert!(rb.source.0 >= gs.a0 - 1e-12 && rb.source.1 <= gs.a1 + 1e-12);
            assert!(rb.target.0 >= gt.a0 - 1e-12 && rb.target.1 <= gt.a1 + 1e-12);
        }
        assert_eq!(chord(3, &links, 0.1), lay, "deterministic");
    }

    #[test]
    fn ribbons_sharing_a_group_do_not_cross() {
        // Every pair of five groups linked, some twice, both ways, one to itself. Ribbons between
        // four different groups may have to cross (0–2 and 1–3 on a circle); ribbons that share a
        // group never do. Chords (a, b) and (c, d) cross exactly when one of c, d lies strictly
        // inside (a, b).
        let mut links = vec![(1, 0, 1.0), (3, 0, 0.7), (0, 1, 0.4), (4, 4, 0.5)];
        for a in 0..5 {
            for b in a + 1..5 {
                links.push((a, b, 1.0 + (a * 5 + b) as f64 % 3.0));
            }
        }
        let lay = chord(5, &links, 0.05);
        let mid = |s: (f64, f64)| (s.0 + s.1) / 2.0;
        let chords: Vec<(f64, f64)> = lay.ribbons.iter().map(|r| (mid(r.source).min(mid(r.target)), mid(r.source).max(mid(r.target)))).collect();
        for i in 0..chords.len() {
            for j in i + 1..chords.len() {
                let (li, lj) = (links[i], links[j]);
                if ![lj.0, lj.1].iter().any(|g| *g == li.0 || *g == li.1) {
                    continue;
                }
                let (a, b) = chords[i];
                let (c, d) = chords[j];
                let inside = |x: f64| x > a && x < b;
                assert!(inside(c) == inside(d), "ribbons {i} and {j} cross: {:?} {:?}", chords[i], chords[j]);
            }
        }
    }

    #[test]
    fn degenerate() {
        assert!(chord(0, &[(0, 1, 1.0)], 0.1).groups.is_empty());
        let zero = chord(4, &[(0, 1, 0.0), (2, 9, 5.0), (1, 1, f64::NAN)], 0.1);
        assert!((zero.groups[1].a0 - m::TAU / 4.0).abs() < 1e-12, "no values: an even split");
        assert_eq!(zero.ribbons[1], ChordRibbon { source: (0.0, 0.0), target: (0.0, 0.0) }, "out of range: left out");
        // A self-link takes two spans of its group; huge padding is clamped.
        let own = chord(2, &[(0, 0, 1.0), (0, 1, 1.0)], 100.0);
        let r = own.ribbons[0];
        assert!(r.source.1 <= r.target.0 + 1e-12 || r.target.1 <= r.source.0 + 1e-12);
        assert!(own.groups[0].a1 > own.groups[0].a0);
        let path = chord_ribbon(Vec2::new(100.0, 100.0), 80.0, &own.ribbons[1]);
        let b = path.bounds();
        assert!(b.x >= 20.0 - 1e-6 && b.x1() <= 180.0 + 1e-6 && b.y >= 20.0 - 1e-6 && b.y1() <= 180.0 + 1e-6, "{b:?}");
    }
}
