//! A pyramid of point tiles: the placement from `datars_algo::pyramid` (which rows go to which
//! level and tile, snapped to the pyramid's grid) turned into tiles that carry their rows' stored
//! columns — what the frame pass draws from, and what an archive holds.

use datars_algo::pyramid::{self as algo, PyramidOptions, SUB_BITS};
use datars_data::Column;
use datars_geo::TileId;
use std::collections::BTreeMap;
use std::sync::Arc;

pub use algo::MAX_LEVELS;

/// The square the pyramid covers, in the rows' own units: level 0 is one tile over all of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extent {
    pub x0: f64,
    pub y0: f64,
    pub size: f64,
}

/// What every tile of a pyramid says about the whole (tiles are self-describing, so a reader
/// needs nothing but the tiles).
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub extent: Extent,
    /// The deepest level with rows.
    pub levels: u8,
    /// Rows in the whole pyramid.
    pub rows: u64,
    /// Rows per tile area at average density (the build's budget).
    pub budget: u32,
}

impl Header {
    /// Size of one grid cell in the rows' units.
    pub fn cell(&self) -> f64 {
        self.extent.size / (1u64 << (self.levels + SUB_BITS)) as f64
    }
    /// Grid cells across one tile of level `z`.
    pub fn tile_cells(&self, z: u8) -> u64 {
        1u64 << (self.levels + SUB_BITS - z)
    }
    /// A grid coordinate → the rows' units (the cell's centre), as `datars_algo` computes it.
    pub fn at(&self, origin: f64, g: u64) -> f64 {
        origin + (g as f64 + 0.5) * self.cell()
    }
}

/// One tile's rows: grid positions, source row numbers, and the stored columns — in row order.
#[derive(Clone, Debug, PartialEq)]
pub struct PointTile {
    pub id: TileId,
    pub header: Header,
    pub gx: Vec<u64>,
    pub gy: Vec<u64>,
    /// Row numbers in the source table (ascending).
    pub row: Vec<u64>,
    pub columns: Vec<(String, Column)>,
}

impl PointTile {
    pub fn len(&self) -> usize {
        self.row.len()
    }
    pub fn is_empty(&self) -> bool {
        self.row.is_empty()
    }
    /// Positions in the rows' units.
    pub fn xs(&self) -> Vec<f64> {
        self.gx.iter().map(|&g| self.header.at(self.header.extent.x0, g)).collect()
    }
    pub fn ys(&self) -> Vec<f64> {
        self.gy.iter().map(|&g| self.header.at(self.header.extent.y0, g)).collect()
    }
    /// The tile's rows as a table: `x`, `y`, `$row` and the stored columns (what an instance
    /// template reads as `d`).
    pub fn table(&self, name: &str) -> datars_data::Table {
        let mut cols: Vec<(String, Column)> = vec![
            ("x".into(), Column::Num(self.xs())),
            ("y".into(), Column::Num(self.ys())),
            ("$row".into(), Column::Num(self.row.iter().map(|&r| r as f64).collect())),
        ];
        cols.extend(self.columns.iter().filter(|(n, _)| n != "x" && n != "y" && n != "$row").cloned());
        datars_data::Table::from_columns(name, cols).unwrap_or_else(|_| datars_data::Table::new(name))
    }
}

/// How a pyramid is cut (see `datars_algo::pyramid`).
pub type Options = PyramidOptions;

/// A built pyramid: every tile that holds rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Pyramid {
    pub header: Header,
    pub tiles: BTreeMap<TileId, Arc<PointTile>>,
}

/// Build the pyramid of rows at (`xs[i]`, `ys[i]`), storing `columns` (taken per tile). Rows with
/// a non-finite position are left out.
pub fn build(xs: &[f64], ys: &[f64], columns: &[(String, &Column)], opts: &Options) -> Pyramid {
    let p = algo::pyramid(xs, ys, *opts);
    let rows = p.places.iter().filter(|x| x.is_some()).count() as u64;
    let header = Header { extent: Extent { x0: p.x0, y0: p.y0, size: p.size }, levels: p.levels, rows, budget: opts.budget };
    // Rows grouped by tile, ascending within each.
    let mut placed: Vec<(u8, u32, u32, usize)> = p
        .places
        .iter()
        .enumerate()
        .filter_map(|(i, pl)| {
            pl.map(|pl| {
                let (tx, ty) = p.tile(&pl);
                (pl.level, ty, tx, i)
            })
        })
        .collect();
    placed.sort_unstable();
    let mut tiles = BTreeMap::new();
    let mut start = 0;
    while start < placed.len() {
        let (l, ty, tx, _) = placed[start];
        let end = placed[start..].iter().position(|q| (q.0, q.1, q.2) != (l, ty, tx)).map_or(placed.len(), |e| start + e);
        let members: Vec<usize> = placed[start..end].iter().map(|q| q.3).collect();
        let at = |i: usize| p.places[i].unwrap_or(algo::Place { level: 0, gx: 0, gy: 0 });
        let t = PointTile {
            id: TileId::new(l, tx, ty),
            header: header.clone(),
            gx: members.iter().map(|&i| at(i).gx).collect(),
            gy: members.iter().map(|&i| at(i).gy).collect(),
            row: members.iter().map(|&i| i as u64).collect(),
            columns: columns.iter().map(|(name, c)| (name.clone(), c.take(&members))).collect(),
        };
        tiles.insert(t.id, Arc::new(t));
        start = end;
    }
    Pyramid { header, tiles }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_hold_their_rows_and_columns() {
        let mut rng = datars_math::Rng::new(5);
        let xs: Vec<f64> = (0..10_000).map(|_| rng.range(-1.0, 1.0)).collect();
        let ys: Vec<f64> = (0..10_000).map(|_| rng.range(-1.0, 1.0)).collect();
        let v = Column::Num((0..10_000).map(|i| i as f64 * 2.0).collect());
        let p = build(&xs, &ys, &[("v".into(), &v)], &Options::with_budget(200));
        let mut seen = vec![false; 10_000];
        for t in p.tiles.values() {
            let cells = p.header.tile_cells(t.id.z);
            for (k, &r) in t.row.iter().enumerate() {
                assert!(!seen[r as usize]);
                seen[r as usize] = true;
                assert_eq!((t.gx[k] / cells, t.gy[k] / cells), (t.id.x as u64, t.id.y as u64));
                assert_eq!(t.columns[0].1.f64_at(k), r as f64 * 2.0);
                assert!((t.xs()[k] - xs[r as usize]).abs() <= p.header.cell());
            }
            assert!(t.row.windows(2).all(|w| w[0] < w[1]));
            let table = t.table("pts");
            assert_eq!(table.column_names(), ["x", "y", "$row", "v"]);
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(p.header.rows, 10_000);
    }
}
