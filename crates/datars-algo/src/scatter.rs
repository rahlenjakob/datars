//! Evenly spread points inside a polygon (dot-density maps, shape-filling unit charts).

use crate::util::{pow8, rings_bbox, IndexedHeap};
use datars_math::path::{resample_closed, signed_area};
use datars_math::{Rng, Vec2};

/// Exactly `n` points inside the polygon formed by `rings` (even-odd: outer rings with holes),
/// spread evenly like blue noise, deterministic for a given `seed`.
///
/// Method (Yuksel's *sample elimination*): generate about 4n candidates on a jittered grid
/// restricted to the polygon, then repeatedly remove the candidate most crowded by its neighbours
/// until `n` remain. The result has no clumps and no visible grid, like Poisson-disk sampling, but
/// with an exact count — what a dot-density map needs (one dot per k people) and what unit charts
/// that fill a shape (a word, a silhouette) need (one dot per unit).
///
/// Output is in candidate order (row-major: top to bottom, left to right), so consecutive indices
/// are spatially coherent. Cost: O(n log n) plus a point-in-polygon test per candidate against only
/// the edges crossing its horizontal band.
///
/// Totality: `n == 0` or no ring with ≥ 3 finite points → empty. A polygon with zero area gives
/// points spread along its outline instead (the best "inside" available). Non-finite vertices are
/// dropped.
pub fn scatter_in(rings: &[Vec<Vec2>], n: usize, seed: u64) -> Vec<Vec2> {
    let rings: Vec<Vec<Vec2>> =
        rings.iter().map(|r| r.iter().copied().filter(|p| p.is_finite()).collect::<Vec<_>>()).filter(|r| r.len() >= 3).collect();
    if n == 0 || rings.is_empty() {
        return Vec::new();
    }
    let Some((lo, hi)) = rings_bbox(&rings) else { return Vec::new() };
    let index = EdgeIndex::new(&rings, lo.y, hi.y);
    let guess: f64 = rings.iter().map(|r| signed_area(r).abs()).sum();
    let target = 4 * n;
    let mut spacing = if guess > 0.0 { (guess / target as f64).sqrt() } else { 0.0 };
    let mut cands: Vec<Vec2> = Vec::new();
    let mut used = spacing;
    if spacing > 0.0 && spacing.is_finite() {
        let cap = 64 * target + 65_536;
        for _ in 0..12 {
            let Some(found) = jittered_candidates(&index, lo, hi, spacing, seed, cap) else { break };
            cands = found;
            used = spacing;
            let got = cands.len();
            if got >= n && got <= 16 * n {
                break;
            }
            // Too few (holes, thin shapes) or too many (self-overlapping rings): rescale.
            let ratio = (got.max(1) as f64 / target as f64).sqrt();
            spacing *= if got < n { ratio.clamp(0.05, 0.7) } else { ratio.max(1.3) };
            if spacing.is_nan() || spacing <= 0.0 {
                break;
            }
        }
    }
    if cands.len() < n {
        // No usable interior: spread along the longest outline.
        let longest = rings.iter().max_by(|a, b| datars_math::total_cmp(perimeter(a), perimeter(b))).unwrap_or(&rings[0]);
        let along = resample_closed(longest, n);
        if along.iter().all(|p| p.is_finite()) {
            return along;
        }
        // Coordinates so large that lengths overflow: cycle through the vertices.
        return (0..n).map(|i| longest[i % longest.len()]).collect();
    }
    let area = cands.len() as f64 * used * used;
    eliminate(&cands, n, area)
}

fn perimeter(r: &[Vec2]) -> f64 {
    (0..r.len()).map(|i| r[i].dist(r[(i + 1) % r.len()])).sum()
}

/// Horizontal bands over the polygon's height, each listing the edges that cross it, so a
/// point-in-polygon test only looks at a handful of edges.
struct EdgeIndex {
    edges: Vec<(Vec2, Vec2)>,
    bands: Vec<Vec<u32>>,
    y0: f64,
    band_h: f64,
}

impl EdgeIndex {
    fn new(rings: &[Vec<Vec2>], y0: f64, y1: f64) -> EdgeIndex {
        let mut edges = Vec::new();
        for r in rings {
            for i in 0..r.len() {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                if a.y != b.y {
                    edges.push((a, b));
                }
            }
        }
        let nb = ((edges.len() as f64).sqrt().ceil() as usize).clamp(1, 4096);
        let band_h = ((y1 - y0) / nb as f64).max(f64::MIN_POSITIVE);
        let mut bands = vec![Vec::new(); nb];
        for (k, (a, b)) in edges.iter().enumerate() {
            let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
            let b0 = (((lo - y0) / band_h).floor().max(0.0) as usize).min(nb - 1);
            let b1 = (((hi - y0) / band_h).floor().max(0.0) as usize).min(nb - 1);
            for band in bands.iter_mut().take(b1 + 1).skip(b0) {
                band.push(k as u32);
            }
        }
        EdgeIndex { edges, bands, y0, band_h }
    }

    fn contains(&self, p: Vec2) -> bool {
        let nb = self.bands.len();
        let b = (((p.y - self.y0) / self.band_h).floor().max(0.0) as usize).min(nb - 1);
        let mut inside = false;
        for &k in &self.bands[b] {
            let (a, c) = self.edges[k as usize];
            if (a.y > p.y) != (c.y > p.y) && p.x < (c.x - a.x) * (p.y - a.y) / (c.y - a.y) + a.x {
                inside = !inside;
            }
        }
        inside
    }

    fn band_of(&self, y: f64) -> usize {
        (((y - self.y0) / self.band_h).floor().max(0.0) as usize).min(self.bands.len() - 1)
    }

    /// The x-intervals the polygon occupies within the horizontal strip `ya..yb`: the projection of
    /// the strip's piece of the polygon, which is the union of its boundary's projections — the
    /// inside spans of the scanlines at `ya` and `yb`, and the edge pieces between them. Merged and
    /// sorted. This keeps candidate generation proportional to the polygon's area (plus perimeter),
    /// not its bounding box, so rings, spirals and slivers stay cheap.
    fn strip_intervals(&self, ya: f64, yb: f64) -> Vec<(f64, f64)> {
        let mut iv: Vec<(f64, f64)> = Vec::new();
        for y in [ya, yb] {
            let mut xs: Vec<f64> = Vec::new();
            for &k in &self.bands[self.band_of(y)] {
                let (a, c) = self.edges[k as usize];
                if (a.y > y) != (c.y > y) {
                    xs.push((c.x - a.x) * (y - a.y) / (c.y - a.y) + a.x);
                }
            }
            xs.sort_by(|a, b| datars_math::total_cmp(*a, *b));
            for pair in xs.chunks_exact(2) {
                iv.push((pair[0], pair[1]));
            }
        }
        for band in &self.bands[self.band_of(ya)..=self.band_of(yb)] {
            for &k in band {
                let (a, c) = self.edges[k as usize];
                let (s0, s1) = (a.y.min(c.y).max(ya), a.y.max(c.y).min(yb));
                if s0 > s1 {
                    continue;
                }
                let x_at = |y: f64| a.x + (c.x - a.x) * (y - a.y) / (c.y - a.y);
                let (x0, x1) = (x_at(s0), x_at(s1));
                iv.push((x0.min(x1), x0.max(x1)));
            }
        }
        iv.sort_by(|a, b| datars_math::total_cmp(a.0, b.0));
        let mut merged: Vec<(f64, f64)> = Vec::with_capacity(iv.len());
        for (lo, hi) in iv {
            match merged.last_mut() {
                Some(last) if lo <= last.1 => last.1 = last.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        merged
    }
}

/// One jittered sample per grid cell of side `s` for the cells each row's polygon intervals touch,
/// kept when inside. Row-major order; the jitter stream depends only on the seed and the grid.
/// `None` if that would visit more than `cap` cells (the caller then stops refining).
fn jittered_candidates(index: &EdgeIndex, lo: Vec2, hi: Vec2, s: f64, seed: u64, cap: usize) -> Option<Vec<Vec2>> {
    let rows = ((hi.y - lo.y) / s).ceil().max(1.0);
    let cols_total = ((hi.x - lo.x) / s).ceil().max(1.0);
    if rows > cap as f64 {
        return None;
    }
    let (rows, cols_total) = (rows as usize, cols_total.min(usize::MAX as f64 / 2.0) as usize);
    // First pass: the column ranges per row, and the total work.
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    let mut cells = 0usize;
    for r in 0..rows {
        let (ya, yb) = (lo.y + r as f64 * s, (lo.y + (r + 1) as f64 * s).min(hi.y));
        let mut last_c1: Option<usize> = None;
        for (xa, xb) in index.strip_intervals(ya, yb) {
            let mut c0 = (((xa - lo.x) / s).floor().max(0.0) as usize).min(cols_total - 1);
            let c1 = (((xb - lo.x) / s).floor().max(0.0) as usize).min(cols_total - 1);
            if let Some(l) = last_c1 {
                if c0 <= l {
                    c0 = l + 1;
                }
            }
            if c0 > c1 {
                continue;
            }
            spans.push((r, c0, c1));
            cells += c1 - c0 + 1;
            last_c1 = Some(c1);
            if cells > cap {
                return None;
            }
        }
    }
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for (r, c0, c1) in spans {
        let ya = lo.y + r as f64 * s;
        for c in c0..=c1 {
            let p = Vec2::new(lo.x + (c as f64 + rng.next_f64()) * s, ya + rng.next_f64() * s);
            if index.contains(p) {
                out.push(p);
            }
        }
    }
    Some(out)
}

/// Sample elimination (Yuksel 2015): drop the most crowded candidates until `n` remain.
fn eliminate(c: &[Vec2], n: usize, area: f64) -> Vec<Vec2> {
    let m = c.len();
    if m <= n {
        return c.to_vec();
    }
    // Maximal Poisson-disk radius for n points in this area (hexagonal packing), and the
    // neighbourhood 2·r_max over which candidates crowd each other.
    let r_max = (area / (2.0 * 3f64.sqrt() * n as f64)).sqrt();
    let reach = 2.0 * r_max;
    if !(reach > 0.0 && reach.is_finite()) {
        return c.iter().step_by(m / n).take(n).copied().collect();
    }
    let weight = |d: f64| pow8(1.0 - d / reach);
    let grid = Grid::new(c, reach);
    let mut w = vec![0.0; m];
    for i in 0..m {
        grid.for_neighbors(c, i, reach, |j, d| {
            if j > i {
                let v = weight(d);
                w[i] += v;
                w[j] += v;
            }
        });
    }
    let mut heap = IndexedHeap::new(w);
    let mut removed = vec![false; m];
    let mut left = m;
    while left > n {
        let Some(i) = heap.pop() else { break };
        removed[i] = true;
        left -= 1;
        grid.for_neighbors(c, i, reach, |j, d| {
            if !removed[j] {
                let k = heap.key(j) - weight(d);
                heap.set(j, k);
            }
        });
    }
    (0..m).filter(|&i| !removed[i]).map(|i| c[i]).collect()
}

/// A uniform grid over the candidates in CSR form (no hashing).
struct Grid {
    x0: f64,
    y0: f64,
    cell: f64,
    cols: usize,
    rows: usize,
    start: Vec<u32>,
    items: Vec<u32>,
}

impl Grid {
    fn new(c: &[Vec2], cell: f64) -> Grid {
        let (mut lo, mut hi) = (Vec2::new(f64::INFINITY, f64::INFINITY), Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY));
        for p in c {
            lo.x = lo.x.min(p.x);
            lo.y = lo.y.min(p.y);
            hi.x = hi.x.max(p.x);
            hi.y = hi.y.max(p.y);
        }
        let cols = (((hi.x - lo.x) / cell).floor() as usize + 1).min(1 << 20);
        let rows = (((hi.y - lo.y) / cell).floor() as usize + 1).min(1 << 20);
        let cell_of = |p: Vec2| {
            let cx = (((p.x - lo.x) / cell) as usize).min(cols - 1);
            let cy = (((p.y - lo.y) / cell) as usize).min(rows - 1);
            cy * cols + cx
        };
        let mut count = vec![0u32; cols * rows + 1];
        for p in c {
            count[cell_of(*p) + 1] += 1;
        }
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }
        let mut fill = count.clone();
        let mut items = vec![0u32; c.len()];
        for (i, p) in c.iter().enumerate() {
            let k = cell_of(*p);
            items[fill[k] as usize] = i as u32;
            fill[k] += 1;
        }
        Grid { x0: lo.x, y0: lo.y, cell, cols, rows, start: count, items }
    }

    fn for_neighbors(&self, c: &[Vec2], i: usize, reach: f64, mut f: impl FnMut(usize, f64)) {
        let p = c[i];
        let cx = (((p.x - self.x0) / self.cell) as usize).min(self.cols - 1);
        let cy = (((p.y - self.y0) / self.cell) as usize).min(self.rows - 1);
        let r2 = reach * reach;
        for y in cy.saturating_sub(1)..=(cy + 1).min(self.rows - 1) {
            for x in cx.saturating_sub(1)..=(cx + 1).min(self.cols - 1) {
                let k = y * self.cols + x;
                for &j in &self.items[self.start[k] as usize..self.start[k + 1] as usize] {
                    let j = j as usize;
                    if j == i {
                        continue;
                    }
                    let d2 = (c[j] - p).len2();
                    if d2 < r2 {
                        f(j, d2.sqrt());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::polygon::contains;

    fn square(x: f64, y: f64, s: f64) -> Vec<Vec2> {
        vec![Vec2::new(x, y), Vec2::new(x + s, y), Vec2::new(x + s, y + s), Vec2::new(x, y + s)]
    }

    fn min_dist(p: &[Vec2]) -> f64 {
        let mut best = f64::INFINITY;
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                best = best.min(p[i].dist(p[j]));
            }
        }
        best
    }

    #[test]
    fn exact_count_inside_and_not_in_holes() {
        let rings = vec![square(0.0, 0.0, 100.0), square(30.0, 30.0, 40.0)];
        for n in [1, 7, 100, 1000] {
            let pts = scatter_in(&rings, n, 42);
            assert_eq!(pts.len(), n);
            for p in &pts {
                assert!(contains(&rings, *p), "{p:?} outside or in the hole");
            }
        }
    }

    #[test]
    fn deterministic_and_seed_dependent() {
        let rings = vec![square(0.0, 0.0, 50.0)];
        let a = scatter_in(&rings, 300, 7);
        assert_eq!(a, scatter_in(&rings, 300, 7));
        assert_ne!(a, scatter_in(&rings, 300, 8));
    }

    #[test]
    fn evenly_spread() {
        // 400 points in a 100 × 100 square: hexagonal packing would allow ~5.4 apart; blue noise
        // should keep well clear of clumps (a uniform random set would have pairs < 0.5 apart).
        let pts = scatter_in(&[square(0.0, 0.0, 100.0)], 400, 1);
        let d = min_dist(&pts);
        assert!(d > 2.5, "min distance {d}");
        // Coverage: every 20 × 20 block gets its share (16 ± 6).
        for by in 0..5 {
            for bx in 0..5 {
                let k = pts
                    .iter()
                    .filter(|p| p.x >= bx as f64 * 20.0 && p.x < (bx + 1) as f64 * 20.0 && p.y >= by as f64 * 20.0 && p.y < (by + 1) as f64 * 20.0)
                    .count();
                assert!((10..=22).contains(&k), "block ({bx},{by}) has {k}");
            }
        }
    }

    #[test]
    fn thin_and_concave_shapes() {
        // An L shape and a thin diagonal sliver.
        let l = vec![vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 90.0),
            Vec2::new(100.0, 90.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ]];
        let pts = scatter_in(&l, 250, 3);
        assert_eq!(pts.len(), 250);
        assert!(pts.iter().all(|p| contains(&l, *p)));
        let sliver = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(1000.0, 999.0), Vec2::new(999.0, 1000.0)]];
        let pts = scatter_in(&sliver, 50, 3);
        assert_eq!(pts.len(), 50);
        assert!(pts.iter().all(|p| contains(&sliver, *p)));
    }

    #[test]
    fn degenerate() {
        assert!(scatter_in(&[], 10, 1).is_empty());
        assert!(scatter_in(&[square(0.0, 0.0, 1.0)], 0, 1).is_empty());
        let flat = vec![vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(5.0, 0.0)]];
        let pts = scatter_in(&flat, 5, 1);
        assert_eq!(pts.len(), 5, "zero-area polygon: points along the outline");
        let nan = vec![vec![Vec2::new(f64::NAN, 0.0), Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0)]];
        let pts = scatter_in(&nan, 3, 1);
        assert_eq!(pts.len(), 3);
        assert!(pts.iter().all(|p| p.is_finite()));
    }
}
