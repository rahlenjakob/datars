//! Polygon helpers shared by `scatter_in`, `polylabel` and `contour`: even-odd containment over
//! rings with holes, distances to edges, and grouping rings into polygons.
//!
//! "Rings" are closed polylines given without repeating the first point (a repeated closing point
//! is harmless). A polygon with holes is just its rings; containment uses the even-odd rule, so
//! ring orientation and nesting order don't matter.

use datars_math::path::signed_area;
use datars_math::{total_cmp, Vec2};

/// Whether `p` is inside one ring (even-odd crossing test; boundary points may go either way).
pub fn ring_contains(ring: &[Vec2], p: Vec2) -> bool {
    let n = ring.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether `p` is inside the polygon formed by `rings` under the even-odd rule (inside an outer
/// ring and not inside a hole).
pub fn contains(rings: &[Vec<Vec2>], p: Vec2) -> bool {
    rings.iter().filter(|r| ring_contains(r, p)).count() % 2 == 1
}

/// Squared distance from `p` to the segment `a..b`.
pub fn segment_distance2(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let l2 = ab.len2();
    let t = if l2 > 0.0 { ((p - a).dot(ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).len2()
}

/// Signed distance from `p` to the polygon's boundary: positive inside (even-odd), negative outside.
/// `-∞` when there are no edges.
pub fn signed_distance(rings: &[Vec<Vec2>], p: Vec2) -> f64 {
    let mut inside = false;
    let mut best = f64::INFINITY;
    for ring in rings {
        let n = ring.len();
        if n < 2 {
            continue;
        }
        let mut j = n - 1;
        for i in 0..n {
            let (a, b) = (ring[i], ring[j]);
            if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
                inside = !inside;
            }
            best = best.min(segment_distance2(p, a, b));
            j = i;
        }
    }
    if best.is_infinite() {
        return f64::NEG_INFINITY;
    }
    let d = best.sqrt();
    if inside {
        d
    } else {
        -d
    }
}

/// Even-odd area of a set of rings (what `contains` considers inside), assuming rings don't cross:
/// each ring counts positively or negatively by how deeply it's nested.
pub fn area(rings: &[Vec<Vec2>]) -> f64 {
    group_rings(rings.to_vec()).iter().map(|poly| {
        let outer = signed_area(&poly[0]).abs();
        let holes: f64 = poly[1..].iter().map(|h| signed_area(h).abs()).sum();
        outer - holes
    }).sum()
}

/// Groups non-crossing rings into polygons `[outer, hole, hole, …]` by nesting depth: a ring
/// nested in an even number of others is an outer ring; one nested in an odd number is a hole of
/// the innermost ring containing it. Output order: by decreasing outer area (ties by input order);
/// rings keep their point order.
pub fn group_rings(rings: Vec<Vec<Vec2>>) -> Vec<Vec<Vec<Vec2>>> {
    let rings: Vec<Vec<Vec2>> = rings.into_iter().filter(|r| r.len() >= 3).collect();
    let areas: Vec<f64> = rings.iter().map(|r| signed_area(r).abs()).collect();
    let mut order: Vec<usize> = (0..rings.len()).collect();
    order.sort_by(|&a, &b| total_cmp(areas[b], areas[a]).then(a.cmp(&b)));
    // For each ring, its parent: the smallest larger ring containing its first vertex.
    let mut parent: Vec<Option<usize>> = vec![None; rings.len()];
    let mut depth = vec![0usize; rings.len()];
    for (k, &i) in order.iter().enumerate() {
        let probe = ring_probe(&rings[i]);
        for &j in order[..k].iter().rev() {
            if ring_contains(&rings[j], probe) {
                parent[i] = Some(j);
                depth[i] = depth[j] + 1;
                break;
            }
        }
    }
    let mut polys: Vec<Vec<Vec<Vec2>>> = Vec::new();
    let mut slot = vec![usize::MAX; rings.len()];
    for &i in &order {
        if depth[i] % 2 == 0 {
            slot[i] = polys.len();
            polys.push(vec![rings[i].clone()]);
        }
    }
    for &i in &order {
        if depth[i] % 2 == 1 {
            if let Some(p) = parent[i] {
                polys[slot[p]].push(rings[i].clone());
            }
        }
    }
    polys
}

/// A point to test a ring's nesting with: the midpoint of its first edge (vertices can sit exactly
/// on another ring, edge midpoints much less often).
fn ring_probe(r: &[Vec2]) -> Vec2 {
    (r[0] + r[1 % r.len()]) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Vec<Vec2> {
        vec![Vec2::new(x, y), Vec2::new(x + s, y), Vec2::new(x + s, y + s), Vec2::new(x, y + s)]
    }

    #[test]
    fn containment_with_holes() {
        let rings = vec![square(0.0, 0.0, 10.0), square(3.0, 3.0, 4.0)];
        assert!(contains(&rings, Vec2::new(1.0, 1.0)));
        assert!(!contains(&rings, Vec2::new(5.0, 5.0)), "in the hole");
        assert!(!contains(&rings, Vec2::new(11.0, 5.0)));
        assert!((area(&rings) - 84.0).abs() < 1e-9);
        let d = signed_distance(&rings, Vec2::new(1.0, 5.0));
        assert!((d - 1.0).abs() < 1e-12);
        assert!((signed_distance(&rings, Vec2::new(5.0, 5.0)) + 2.0).abs() < 1e-12);
        assert_eq!(signed_distance(&[], Vec2::ZERO), f64::NEG_INFINITY);
    }

    #[test]
    fn grouping_by_depth() {
        // Outer, its hole, an island in the hole, and a separate square.
        let rings = vec![square(3.0, 3.0, 4.0), square(20.0, 0.0, 2.0), square(0.0, 0.0, 10.0), square(4.0, 4.0, 1.0)];
        let polys = group_rings(rings);
        assert_eq!(polys.len(), 3);
        assert_eq!(polys[0].len(), 2, "outer with one hole");
        assert_eq!(polys[0][1], square(3.0, 3.0, 4.0));
        assert_eq!(polys[1], vec![square(20.0, 0.0, 2.0)]);
        assert_eq!(polys[2], vec![square(4.0, 4.0, 1.0)], "island is its own polygon");
    }
}
