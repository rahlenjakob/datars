//! Binning for big data: rows counted into a regular 2-D grid in one pass, a value per bin
//! aggregated from the rows in it, and density thresholds chosen by the share of the rows they
//! enclose (highest-density regions) — what a density map, a hexbin and density contours of a
//! million rows need, at O(rows) once, instead of a mark per row.

use datars_math::{total_cmp, Rect};

/// What a bin holds besides its count: [`bin_values`] folds the rows' values into one per bin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinAgg {
    Count,
    Sum,
    Mean,
    Min,
    Max,
}

impl BinAgg {
    /// From its name (`count`, `sum`, `mean`/`average`, `min`, `max`).
    pub fn parse(s: &str) -> Option<BinAgg> {
        Some(match s {
            "count" => BinAgg::Count,
            "sum" => BinAgg::Sum,
            "mean" | "average" => BinAgg::Mean,
            "min" => BinAgg::Min,
            "max" => BinAgg::Max,
            _ => return None,
        })
    }
}

/// Rows binned into a grid: which cell each row fell in, and how many rows each cell holds.
#[derive(Clone, Debug, PartialEq)]
pub struct GridBins {
    /// Columns (along x) and rows (along y) of the grid.
    pub nx: usize,
    pub ny: usize,
    /// Per input row: its cell (`row * nx + col`), or [`GridBins::OUTSIDE`].
    pub cells: Vec<u32>,
    /// Per cell, row-major: the number of rows in it.
    pub counts: Vec<u32>,
}

impl GridBins {
    /// The cell of a row that isn't in the grid (outside the extent, or not a finite point).
    pub const OUTSIDE: u32 = u32::MAX;

    /// The extent of cell `(col, row)` within `extent` — `row` 0 at `extent.y`, the low end of y.
    pub fn cell_rect(&self, extent: Rect, col: usize, row: usize) -> Rect {
        let (cw, ch) = (extent.w / self.nx.max(1) as f64, extent.h / self.ny.max(1) as f64);
        Rect::new(extent.x + col as f64 * cw, extent.y + row as f64 * ch, cw, ch)
    }

    /// Rows in the grid (every count, summed).
    pub fn total(&self) -> u64 {
        self.counts.iter().map(|&c| c as u64).sum()
    }
}

/// Bins the points `(xs[i], ys[i])` into an `nx × ny` grid of equal cells over `extent`, in one
/// pass. Column 0 starts at `extent.x`, row 0 at `extent.y` (the low end of y: data units, not
/// screen). Cells are half-open `[lo, hi)`, except that the extent's far edges belong to the last
/// column and row — every point of the closed extent lands in exactly one cell.
///
/// Totality: points outside the extent or not finite, and extra entries of the longer slice, are
/// [`GridBins::OUTSIDE`]; a zero-sized grid, one of more than `u32::MAX` cells, or a degenerate
/// extent bins nothing. Deterministic: a row's cell depends on that row alone.
pub fn grid_bins(xs: &[f64], ys: &[f64], extent: Rect, nx: usize, ny: usize) -> GridBins {
    let n = xs.len().min(ys.len());
    let cells_n = nx.checked_mul(ny).filter(|&c| c > 0 && c < GridBins::OUTSIDE as usize);
    let usable = extent.x.is_finite() && extent.y.is_finite() && extent.w.is_finite() && extent.h.is_finite() && extent.w > 0.0 && extent.h > 0.0;
    let Some(total) = cells_n.filter(|_| usable) else {
        return GridBins { nx, ny, cells: vec![GridBins::OUTSIDE; xs.len().max(ys.len())], counts: vec![0; cells_n.unwrap_or(0)] };
    };
    let mut counts = vec![0u32; total];
    let mut cells = vec![GridBins::OUTSIDE; xs.len().max(ys.len())];
    let (sx, sy) = (nx as f64 / extent.w, ny as f64 / extent.h);
    for i in 0..n {
        let (x, y) = (xs[i], ys[i]);
        if !(x.is_finite() && y.is_finite()) {
            continue;
        }
        let (u, v) = ((x - extent.x) * sx, (y - extent.y) * sy);
        if !(0.0..=nx as f64).contains(&u) || !(0.0..=ny as f64).contains(&v) {
            continue;
        }
        let col = (u.floor() as usize).min(nx - 1);
        let row = (v.floor() as usize).min(ny - 1);
        let c = row * nx + col;
        cells[i] = c as u32;
        counts[c] = counts[c].saturating_add(1);
    }
    GridBins { nx, ny, cells, counts }
}

/// One value per bin (`n` bins) from the rows' `values`, rows assigned to bins by `cells`
/// ([`GridBins::cells`], or any per-row bin index; out-of-range entries are ignored). `Count`
/// counts rows; the others fold the finite values (a row with a missing value still counts, but
/// adds nothing). A bin without a finite value is NaN (0 for `Count` and `Sum`).
pub fn bin_values(cells: &[u32], values: &[f64], n: usize, agg: BinAgg) -> Vec<f64> {
    let start = match agg {
        BinAgg::Min => f64::INFINITY,
        BinAgg::Max => f64::NEG_INFINITY,
        _ => 0.0,
    };
    let mut acc = vec![start; n];
    let mut seen = vec![0u32; n];
    for (i, &c) in cells.iter().enumerate() {
        let c = c as usize;
        if c >= n {
            continue;
        }
        if agg == BinAgg::Count {
            acc[c] += 1.0;
            continue;
        }
        let v = values.get(i).copied().unwrap_or(f64::NAN);
        if !v.is_finite() {
            continue;
        }
        seen[c] += 1;
        match agg {
            BinAgg::Sum | BinAgg::Mean => acc[c] += v,
            BinAgg::Min => acc[c] = acc[c].min(v),
            BinAgg::Max => acc[c] = acc[c].max(v),
            BinAgg::Count => {}
        }
    }
    for (a, &k) in acc.iter_mut().zip(&seen) {
        match agg {
            BinAgg::Mean => *a = if k > 0 { *a / k as f64 } else { f64::NAN },
            BinAgg::Min | BinAgg::Max if k == 0 => *a = f64::NAN,
            _ => {}
        }
    }
    acc
}

/// Density thresholds by the share of the mass they enclose: for each `share` in `(0, 1]`, the
/// largest value `t` such that the cells `>= t` together hold at least that share of the grid's
/// total (the highest-density region holding it — "half of all rows fall inside this line").
///
/// Returned in the order of `shares`; a larger share gives a lower (or equal) threshold, so the
/// regions nest. Non-finite and negative cells count as empty. A share outside `(0, 1]` (or NaN),
/// or a grid with no mass, gives NaN. Deterministic: a stable sort with a total order.
pub fn mass_thresholds(grid: &[f64], shares: &[f64]) -> Vec<f64> {
    let mut vals: Vec<f64> = grid.iter().copied().filter(|v| v.is_finite() && *v > 0.0).collect();
    vals.sort_by(|a, b| total_cmp(*b, *a));
    let total: f64 = vals.iter().sum();
    shares
        .iter()
        .map(|&s| {
            if !(total > 0.0 && s > 0.0 && s <= 1.0) {
                return f64::NAN;
            }
            let want = s * total;
            let mut acc = 0.0;
            for &v in &vals {
                acc += v;
                // A hair of slack: a sum that reaches the share up to rounding counts.
                if acc >= want * (1.0 - 1e-12) {
                    return v;
                }
            }
            vals.last().copied().unwrap_or(f64::NAN)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contour::contour;
    use crate::density::density;
    use crate::polygon::ring_contains;
    use datars_math::{m, Rng, Vec2};

    /// A standard normal draw (Box–Muller).
    fn normal(rng: &mut Rng) -> f64 {
        let (u, v) = (1.0 - rng.next_f64(), rng.next_f64());
        m::sqrt(-2.0 * m::ln(u)) * m::cos(m::TAU * v)
    }

    fn cloud(n: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
        let mut rng = Rng::new(seed);
        (0..n).map(|_| (rng.range(-10.0, 30.0), rng.range(5.0, 25.0))).unzip()
    }

    #[test]
    fn grid_counts_sum_to_the_rows_and_each_row_is_in_its_cell() {
        let (xs, ys) = cloud(20_000, 7);
        let ext = Rect::new(-10.0, 5.0, 40.0, 20.0);
        let g = grid_bins(&xs, &ys, ext, 37, 19);
        assert_eq!(g.total(), 20_000, "every row inside the extent lands in one cell");
        assert_eq!(g.counts.len(), 37 * 19);
        for (i, &c) in g.cells.iter().enumerate() {
            let (col, row) = (c as usize % 37, c as usize / 37);
            let r = g.cell_rect(ext, col, row);
            assert!(xs[i] >= r.x - 1e-9 && xs[i] <= r.x1() + 1e-9 && ys[i] >= r.y - 1e-9 && ys[i] <= r.y1() + 1e-9, "row {i} in its cell");
        }
        // Same input, same bins; shuffled input, same counts.
        assert_eq!(g, grid_bins(&xs, &ys, ext, 37, 19));
        let (rx, ry): (Vec<f64>, Vec<f64>) = (0..xs.len()).rev().map(|i| (xs[i], ys[i])).unzip();
        assert_eq!(grid_bins(&rx, &ry, ext, 37, 19).counts, g.counts);
    }

    #[test]
    fn grid_edges_outsiders_and_degenerate_input() {
        let ext = Rect::new(0.0, 0.0, 10.0, 10.0);
        let xs = [0.0, 10.0, 5.0, -0.1, f64::NAN, 10.1, 9.999];
        let ys = [0.0, 10.0, 5.0, 5.0, 1.0, 1.0, 0.0];
        let g = grid_bins(&xs, &ys, ext, 2, 2);
        assert_eq!(g.cells[0], 0, "the low corner: first cell");
        assert_eq!(g.cells[1], 3, "the far corner belongs to the last cell");
        assert_eq!(g.cells[2], 3, "a cell's low edge is its own");
        assert!(g.cells[3..6].iter().all(|&c| c == GridBins::OUTSIDE));
        assert_eq!(g.cells[6], 1);
        assert_eq!(g.total(), 4);
        assert_eq!(grid_bins(&xs, &ys, ext, 0, 3).total(), 0);
        assert_eq!(grid_bins(&xs, &ys, Rect::new(0.0, 0.0, 0.0, 1.0), 2, 2).total(), 0);
        assert_eq!(grid_bins(&[1.0, 2.0], &[1.0], ext, 2, 2).cells, vec![0, GridBins::OUTSIDE], "the longer slice's extra rows are outside");
    }

    #[test]
    fn values_fold_per_bin() {
        let cells = [0, 1, 1, 2, GridBins::OUTSIDE, 1];
        let v = [4.0, 1.0, 5.0, f64::NAN, 100.0, 3.0];
        assert_eq!(bin_values(&cells, &v, 4, BinAgg::Count), vec![1.0, 3.0, 1.0, 0.0]);
        assert_eq!(bin_values(&cells, &v, 4, BinAgg::Sum), vec![4.0, 9.0, 0.0, 0.0]);
        assert_eq!(bin_values(&cells, &v, 4, BinAgg::Mean)[..2], [4.0, 3.0]);
        assert!(bin_values(&cells, &v, 4, BinAgg::Mean)[2].is_nan(), "no finite value: NaN");
        assert_eq!(bin_values(&cells, &v, 4, BinAgg::Min)[1], 1.0);
        assert_eq!(bin_values(&cells, &v, 4, BinAgg::Max)[1], 5.0);
        assert!(bin_values(&cells, &v, 4, BinAgg::Max)[3].is_nan());
        assert_eq!(BinAgg::parse("average"), Some(BinAgg::Mean));
        assert_eq!(BinAgg::parse("median"), None);
    }

    #[test]
    fn mass_thresholds_enclose_their_share_and_nest() {
        let grid: Vec<f64> = (0..100).map(|i| (i % 10) as f64).collect();
        let total: f64 = grid.iter().sum();
        let shares = [0.9, 0.5, 0.1, 1.0];
        let t = mass_thresholds(&grid, &shares);
        for (s, th) in shares.iter().zip(&t) {
            let inside: f64 = grid.iter().filter(|v| *v >= th).sum();
            assert!(inside >= s * total - 1e-9, "share {s}: {inside} of {total}");
            // The largest such threshold: one step up holds less than the share.
            let above: f64 = grid.iter().filter(|v| **v > *th).sum();
            assert!(above < s * total, "share {s}");
        }
        assert!(t[0] <= t[1] && t[1] <= t[2], "larger shares, lower thresholds");
        assert!(mass_thresholds(&grid, &[0.0, 1.5, f64::NAN]).iter().all(|v| v.is_nan()));
        assert!(mass_thresholds(&[0.0, f64::NAN, -1.0], &[0.5])[0].is_nan());
    }

    #[test]
    fn density_contours_nest_by_level() {
        // Two crowds of different sizes: the contours of a density of their rows, at thresholds by
        // share, nest — every ring of a higher level lies inside the region of the level below.
        let mut rng = Rng::new(21);
        let pts: Vec<Vec2> = (0..6000)
            .map(|i| {
                let (cx, cy, s) = if i % 3 == 0 { (15.0, 12.0, 3.0) } else { (38.0, 26.0, 5.0) };
                Vec2::new(cx + normal(&mut rng) * s, cy + normal(&mut rng) * s)
            })
            .collect();
        let (w, h) = (60, 40);
        let g = density(&pts, None, 2.0, w, h, Rect::new(0.0, 0.0, w as f64, h as f64));
        let t = mass_thresholds(&g, &[0.9, 0.7, 0.5, 0.3, 0.1]);
        let levels = contour(&g, w, h, &t);
        assert!(levels.iter().all(|(_, rings)| !rings.is_empty()));
        let inside = |rings: &[Vec<Vec2>], p: Vec2| rings.iter().filter(|r| ring_contains(r, p)).count() % 2 == 1;
        for pair in levels.windows(2) {
            let (outer, inner) = (&pair[0].1, &pair[1].1);
            for r in inner {
                for p in r {
                    // A vertex of the inner level on or inside the outer level's region (even-odd
                    // over its rings, so holes count), allowing for interpolation along a shared edge.
                    let near = [Vec2::ZERO, Vec2::new(0.05, 0.0), Vec2::new(-0.05, 0.0), Vec2::new(0.0, 0.05), Vec2::new(0.0, -0.05)];
                    assert!(near.iter().any(|d| inside(outer, *p + *d)), "level nests at {p:?}");
                }
            }
        }
        // Deterministic: the same rows give the same rings, bit for bit.
        let again = contour(&density(&pts, None, 2.0, w, h, Rect::new(0.0, 0.0, w as f64, h as f64)), w, h, &t);
        assert_eq!(levels, again);
    }
}
