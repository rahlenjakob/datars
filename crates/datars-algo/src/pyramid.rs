//! Level-of-detail pyramids for point sets of any size (docs/12-delivery.md, "Big data stays
//! interactive"): a quadtree of tiles in which each level holds a seeded, density-preserving
//! sample of the points under it.
//!
//! Every point gets a priority (a hash of its index) and the points are visited in priority
//! order. Level ℓ shows the fraction `f(ℓ) = budget · 4^ℓ / n` of all points (all of them from the
//! level where that reaches 1): a point belongs to the shallowest level whose fraction its
//! priority falls under, in that level's tile containing it. So the points down to level ℓ are a
//! uniform random sample of the data — dense regions stay dense, sparse ones sparse, and no seam
//! shows where tiles meet, because the fraction is the same everywhere — with about `budget` of
//! them in each tile's area at average density. Where the fraction reaches 1 (every remaining
//! point), a tile that fills up (`cap`: a region much denser than average) passes the rest to its
//! children: tiles stay bounded, dense regions get deeper levels, and a view showing every point
//! reads them from there.
//!
//! Positions snap to one global grid `2^(levels + SUB_BITS)` cells across — a cell is 1/65536 of a
//! deepest tile, finer than a pixel at any zoom the pyramid serves — the same grid whichever tile
//! a point lands in, so a point drawn from a coarse tile deep into a zoom stays exactly where it
//! is. Grid coordinates are integers and every level's tile is a shift of them: the result is
//! bit-identical on every target.

use std::collections::BTreeMap;

/// Grid bits below the deepest level (1/65536 of a deepest tile).
pub const SUB_BITS: u8 = 16;
/// The deepest level a pyramid may have. Grid coordinates take `MAX_LEVELS + SUB_BITS` = 40 bits:
/// exact in an f64.
pub const MAX_LEVELS: u8 = 24;
const GRID_BITS: u8 = MAX_LEVELS + SUB_BITS;
/// Levels whose tile counts are kept in a dense array while building (4^11 cells = 16 MB).
const DENSE_LEVELS: u8 = 11;

/// How a pyramid is cut.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PyramidOptions {
    /// Points per tile area at average density.
    pub budget: u32,
    /// Points one tile may hold once its level holds every remaining point; more go down to its
    /// children. (Shallower levels are never capped: a cap there would thin one tile's sample and
    /// not its neighbour's.)
    pub cap: u32,
}

impl Default for PyramidOptions {
    /// 2048 points per tile area: about one point per 20 px² with 256 px tiles.
    fn default() -> PyramidOptions {
        PyramidOptions::with_budget(2048)
    }
}

impl PyramidOptions {
    /// A budget, and a cap of 4× it.
    pub fn with_budget(budget: u32) -> PyramidOptions {
        let budget = budget.max(1);
        PyramidOptions { budget, cap: budget.saturating_mul(4) }
    }
}

/// The fraction of all `n` points drawn down to `level` of a pyramid built with `budget`.
pub fn fraction(budget: u32, n: u64, level: u8) -> f64 {
    let f = budget as f64 * (1u64 << (2 * level.min(MAX_LEVELS) as u32)) as f64 / n.max(1) as f64;
    f.min(1.0)
}

/// The first level that holds every remaining point (its fraction is 1).
pub fn full_level(budget: u32, n: u64) -> u8 {
    (0..=MAX_LEVELS).find(|&l| fraction(budget, n, l) >= 1.0).unwrap_or(MAX_LEVELS)
}

/// Where one point went: its level and its position on the pyramid's grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub level: u8,
    pub gx: u64,
    pub gy: u64,
}

/// A pyramid over points: the square it covers, its depth, and every point's place.
#[derive(Clone, Debug, PartialEq)]
pub struct PointPyramid {
    /// The covered square: level 0 is one tile over it.
    pub x0: f64,
    pub y0: f64,
    pub size: f64,
    /// The deepest level with points.
    pub levels: u8,
    /// Per input point (`None`: a non-finite position).
    pub places: Vec<Option<Place>>,
}

impl PointPyramid {
    /// Grid cells across one tile of level `z`.
    pub fn tile_cells(&self, z: u8) -> u64 {
        1u64 << (self.levels + SUB_BITS - z)
    }
    /// The tile (column, row) of a place, at its level.
    pub fn tile(&self, p: &Place) -> (u32, u32) {
        let c = self.tile_cells(p.level);
        ((p.gx / c) as u32, (p.gy / c) as u32)
    }
    /// A grid coordinate in the input's units (the cell's centre).
    pub fn coord(&self, origin: f64, g: u64) -> f64 {
        origin + (g as f64 + 0.5) * (self.size / (1u64 << (self.levels + SUB_BITS)) as f64)
    }
}

/// A point's place in the pyramid order: a hash of its index.
pub fn priority(i: usize) -> u64 {
    datars_math::mix64(i as u64)
}

/// Build the pyramid of the points (`xs[i]`, `ys[i]`). Points with a non-finite coordinate get
/// no place. Total: empty input is an empty pyramid over the unit square.
pub fn pyramid(xs: &[f64], ys: &[f64], opts: PyramidOptions) -> PointPyramid {
    let n = xs.len().min(ys.len());
    let ok = |i: usize| xs[i].is_finite() && ys[i].is_finite();
    let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut count = 0usize;
    for i in (0..n).filter(|&i| ok(i)) {
        lo_x = lo_x.min(xs[i]);
        lo_y = lo_y.min(ys[i]);
        hi_x = hi_x.max(xs[i]);
        hi_y = hi_y.max(ys[i]);
        count += 1;
    }
    let mut places: Vec<Option<Place>> = vec![None; n];
    if count == 0 {
        return PointPyramid { x0: 0.0, y0: 0.0, size: 1.0, levels: 0, places };
    }
    let span = (hi_x - lo_x).max(hi_y - lo_y);
    let size = if span > 0.0 && span.is_finite() { span } else { 1.0 };
    let (x0, y0) = (lo_x, lo_y);
    // Grid coordinates on the finest grid; a level's tile is a shift of them.
    let full = (1u64 << GRID_BITS) as f64;
    let grid = |v: f64, o: f64| -> u64 { ((((v - o) / size) * full).floor().max(0.0) as u64).min((1u64 << GRID_BITS) - 1) };

    // Visit points in priority order (ties by index: a total order).
    let mut order: Vec<u32> = (0..n as u32).filter(|&i| ok(i as usize)).collect();
    order.sort_by_key(|&i| (priority(i as usize), i));
    // Level fractions as thresholds on the 64-bit priority; 4^ℓ exactly (ℓ ≤ 24: 2^48 fits).
    let threshold: Vec<u128> = (0..=MAX_LEVELS)
        .map(|l| {
            let f = opts.budget as f64 * (1u64 << (2 * l as u32)) as f64 / count as f64;
            if f >= 1.0 {
                u128::MAX
            } else {
                (f * 18_446_744_073_709_551_616.0) as u128
            }
        })
        .collect();
    let full = full_level(opts.budget, count as u64);
    // Tile counts, only where caps apply (levels holding every remaining point).
    let mut dense: Vec<Vec<u32>> = (0..=DENSE_LEVELS).map(|l| if l >= full { vec![0u32; 1usize << (2 * l as u32)] } else { Vec::new() }).collect();
    let mut sparse: BTreeMap<(u8, u64, u64), u32> = BTreeMap::new();
    let mut fine: Vec<(u64, u64)> = vec![(0, 0); n];
    let mut levels = 0u8;
    for &i in &order {
        let i = i as usize;
        let p = priority(i) as u128;
        let (gx, gy) = (grid(xs[i], x0), grid(ys[i], y0));
        fine[i] = (gx, gy);
        let mut l = threshold.iter().position(|&t| p < t).unwrap_or(MAX_LEVELS as usize) as u8;
        while l >= full && l < MAX_LEVELS {
            let shift = GRID_BITS - l;
            let (tx, ty) = (gx >> shift, gy >> shift);
            let c = if l <= DENSE_LEVELS { &mut dense[l as usize][((ty as usize) << l) | tx as usize] } else { sparse.entry((l, tx, ty)).or_insert(0) };
            if *c < opts.cap {
                *c += 1;
                break;
            }
            l += 1;
        }
        places[i] = Some(Place { level: l, gx: 0, gy: 0 });
        levels = levels.max(l);
    }
    // Snap to the pyramid's own grid (as deep as its deepest level needs).
    let down = MAX_LEVELS - levels;
    for (i, p) in places.iter_mut().enumerate() {
        if let Some(p) = p {
            p.gx = fine[i].0 >> down;
            p.gy = fine[i].1 >> down;
        }
    }
    PointPyramid { x0, y0, size, levels, places }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cloud(n: usize) -> (Vec<f64>, Vec<f64>) {
        let mut rng = datars_math::Rng::new(3);
        // A dense core (three quarters of the points) and a sparse halo.
        (0..n)
            .map(|i| {
                let r = if i % 4 == 0 { rng.next_f64() * 100.0 } else { rng.next_f64() * 10.0 };
                let a = rng.next_f64() * std::f64::consts::TAU;
                (r * datars_math::m::cos(a), r * datars_math::m::sin(a))
            })
            .unzip()
    }

    #[test]
    fn every_point_lands_in_the_tile_containing_it() {
        let (xs, ys) = cloud(20_000);
        let o = PyramidOptions::with_budget(128);
        let p = pyramid(&xs, &ys, o);
        let cell = p.size / (1u64 << (p.levels + SUB_BITS)) as f64;
        let mut per_tile: BTreeMap<(u8, u32, u32), u32> = BTreeMap::new();
        for (i, pl) in p.places.iter().enumerate() {
            let pl = pl.expect("finite points are placed");
            // Snapped to within a cell of where it was.
            assert!((p.coord(p.x0, pl.gx) - xs[i]).abs() <= cell);
            assert!((p.coord(p.y0, pl.gy) - ys[i]).abs() <= cell);
            let (tx, ty) = p.tile(&pl);
            *per_tile.entry((pl.level, tx, ty)).or_insert(0) += 1;
        }
        // Capped only where a level holds every remaining point.
        let full = full_level(o.budget, 20_000);
        assert_eq!(full, 4);
        assert!(per_tile.iter().all(|(k, &c)| k.0 < full || c <= o.cap));
        assert!(p.levels > full, "the dense core went deeper than the full level");
    }

    #[test]
    fn levels_are_density_preserving_samples() {
        let (xs, ys) = cloud(40_000);
        let p = pyramid(&xs, &ys, PyramidOptions::with_budget(512));
        // Level 0 holds about `budget` points, a uniform sample: the core keeps its share (three
        // quarters, plus the halo's sliver inside r < 10).
        let root: Vec<usize> = (0..xs.len()).filter(|&i| p.places[i].unwrap().level == 0).collect();
        assert!((400..=600).contains(&root.len()), "{}", root.len());
        let core = root.iter().filter(|&&i| (xs[i] * xs[i] + ys[i] * ys[i]).sqrt() < 10.0).count() as f64 / root.len() as f64;
        assert!((0.68..0.86).contains(&core), "{core}");
    }

    #[test]
    fn below_the_full_level_every_tile_samples_the_same_fraction() {
        // Uniform data with a dense block: at every level short of the full one, a tile's count is
        // its share of the points × the level's fraction — no tile is thinned more than another
        // (a seam where they meet).
        let mut rng = datars_math::Rng::new(8);
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for i in 0..60_000 {
            let dense = i % 3 == 0;
            xs.push(if dense { rng.next_f64() * 10.0 } else { rng.next_f64() * 100.0 });
            ys.push(if dense { rng.next_f64() * 10.0 } else { rng.next_f64() * 100.0 });
        }
        let o = PyramidOptions::with_budget(300);
        let p = pyramid(&xs, &ys, o);
        let full = full_level(o.budget, 60_000);
        assert!(full >= 3);
        let l = full - 1;
        let f = fraction(o.budget, 60_000, l);
        // The tile over the dense block at level `l`, and one over plain ground.
        let n = 1u64 << l;
        let cell = |x: f64, y: f64| ((x / p.size * n as f64) as u64, (y / p.size * n as f64) as u64);
        for (x, y) in [(2.0, 2.0), (80.0, 80.0)] {
            let (tx, ty) = cell(x - p.x0, y - p.y0);
            let inside = |i: usize| cell(xs[i] - p.x0, ys[i] - p.y0) == (tx, ty);
            let all = (0..xs.len()).filter(|&i| inside(i)).count() as f64;
            let drawn = (0..xs.len()).filter(|&i| inside(i) && p.places[i].unwrap().level <= l).count() as f64;
            assert!((drawn / all - f).abs() < 0.2 * f + 0.01, "tile at ({x}, {y}): {drawn} of {all}, want {f}");
        }
        assert!((fraction(300, 60_000, full) - 1.0).abs() < 1e-12 && fraction(300, 60_000, full - 1) < 1.0);
    }

    #[test]
    fn total_and_deterministic() {
        let (xs, ys) = cloud(5_000);
        let o = PyramidOptions::with_budget(100);
        assert_eq!(pyramid(&xs, &ys, o), pyramid(&xs, &ys, o));
        let mut xs2 = xs.clone();
        xs2[7] = f64::NAN;
        let p = pyramid(&xs2, &ys, o);
        assert!(p.places[7].is_none() && p.places.iter().filter(|x| x.is_some()).count() == 4_999);
        let empty = pyramid(&[], &[], o);
        assert!(empty.places.is_empty() && empty.size == 1.0);
        // One point, or all in one place: a unit square, level 0.
        let same = pyramid(&[3.0, 3.0], &[4.0, 4.0], o);
        assert_eq!((same.size, same.levels), (1.0, 0));
    }
}
