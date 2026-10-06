//! MVT → what templates read: per layer, a table of feature properties (so filters and styles
//! are ordinary expressions over `d`) and each feature's geometry as a path in tile units.
//!
//! Geometry stays in tile-local coordinates, normalized to [`UNITS`] across the tile whatever the
//! layer's extent, so it's decoded once and placed by a transform (Mercator) or reprojected once
//! per tile (other projections). Integer MVT coordinates are exact in f64: decoding is bit-exact
//! on every platform.

use datars_data::{Column, Table};
use datars_geo::mvt::{GeomType, Value as MvtValue, VectorTile};
use datars_geo::TileId;
use datars_math::{PathData, Vec2};
use datars_scene::Geom;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Tile-local units across a tile (the conventional MVT extent).
pub(crate) const UNITS: f64 = 4096.0;

pub(crate) struct DecodedTile {
    pub id: TileId,
    pub layers: Vec<DecodedLayer>,
}

pub(crate) struct DecodedLayer {
    pub name: String,
    /// One row per feature: the feature's properties plus `$type`, `$id`, `$x`, `$y`.
    pub table: Arc<Table>,
    /// Per row: the geometry in tile units (points are move-only paths).
    pub geoms: Arc<Vec<Geom>>,
    /// Per row: the anchor point (a point feature's point; else its first vertex or ring centre).
    pub anchors: Arc<Vec<Vec2>>,
}

impl DecodedTile {
    pub fn layers<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a DecodedLayer> + 'a {
        self.layers.iter().filter(move |l| l.name == name)
    }
}

pub(crate) fn decode(id: TileId, bytes: &[u8]) -> Result<DecodedTile, String> {
    let vt = VectorTile::decode(bytes).map_err(|e| e.to_string())?;
    let mut layers = Vec::with_capacity(vt.layers.len());
    for l in vt.layers {
        let k = UNITS / l.extent.max(1) as f64;
        let mut geoms = Vec::with_capacity(l.features.len());
        let mut anchors = Vec::with_capacity(l.features.len());
        let mut kinds: Vec<Option<Arc<str>>> = Vec::with_capacity(l.features.len());
        let mut ids = Vec::with_capacity(l.features.len());
        // Properties by key, in key order (deterministic columns); rows missing a key get null.
        let mut props: BTreeMap<String, Vec<Option<MvtValue>>> = BTreeMap::new();
        let mut kept = 0usize;
        for f in l.features {
            let Some((geom, anchor, kind)) = geometry(f.geom_type, &f.geometry, k) else { continue };
            for (key, v) in f.properties {
                let col = props.entry(key).or_insert_with(|| vec![None; kept]);
                col.resize(kept, None);
                col.push(Some(v));
            }
            kept += 1;
            geoms.push(geom);
            anchors.push(anchor);
            kinds.push(Some(Arc::from(kind)));
            ids.push(f.id.map(|v| v as f64).unwrap_or(f64::NAN));
        }
        let mut columns: Vec<(String, Column)> = vec![
            ("$type".into(), Column::Str(kinds)),
            ("$id".into(), Column::Num(ids)),
            ("$x".into(), Column::Num(anchors.iter().map(|a| a.x).collect())),
            ("$y".into(), Column::Num(anchors.iter().map(|a| a.y).collect())),
        ];
        for (key, mut vals) in props {
            vals.resize(kept, None);
            columns.push((key, column(&vals)));
        }
        let table = Table::from_columns(&l.name, columns).map_err(|e| e.to_string())?;
        layers.push(DecodedLayer { name: l.name, table: Arc::new(table), geoms: Arc::new(geoms), anchors: Arc::new(anchors) });
    }
    Ok(DecodedTile { id, layers })
}

/// A property column: numbers if every present value is numeric, booleans if every one is a
/// boolean, else strings (numbers written out).
fn column(vals: &[Option<MvtValue>]) -> Column {
    let present = || vals.iter().flatten();
    if present().all(|v| v.as_f64().is_some()) {
        return Column::Num(vals.iter().map(|v| v.as_ref().and_then(|v| v.as_f64()).unwrap_or(f64::NAN)).collect());
    }
    if present().all(|v| matches!(v, MvtValue::Bool(_))) {
        return Column::Bool(vals.iter().map(|v| matches!(v, Some(MvtValue::Bool(true)))).collect());
    }
    Column::Str(
        vals.iter()
            .map(|v| {
                v.as_ref().map(|v| match v {
                    MvtValue::String(s) => Arc::from(s.as_str()),
                    MvtValue::Bool(b) => Arc::from(if *b { "true" } else { "false" }),
                    other => Arc::from(crate::env::str_of(&datars_expr::Value::Num(other.as_f64().unwrap_or(f64::NAN))).as_str()),
                })
            })
            .collect(),
    )
}

/// Twice the signed area of a ring (positive = clockwise on screen, y down).
fn area2(r: &[(i32, i32)]) -> i128 {
    let n = r.len();
    (0..n).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128
    }).sum()
}

/// A feature's path in tile units, its anchor, and its kind name. `None` for empty geometry.
fn geometry(kind: GeomType, paths: &[Vec<(i32, i32)>], k: f64) -> Option<(Geom, Vec2, &'static str)> {
    let p = |q: &(i32, i32)| Vec2::new(q.0 as f64 * k, q.1 as f64 * k);
    let mut path = PathData::new();
    match kind {
        GeomType::Point => {
            for q in paths.iter().flatten() {
                path.move_to(p(q));
            }
            let first = paths.iter().flatten().next()?;
            Some((Geom::path(path), p(first), "point"))
        }
        GeomType::LineString => {
            let mut anchor = None;
            for l in paths.iter().filter(|l| l.len() > 1) {
                path.move_to(p(&l[0]));
                for q in &l[1..] {
                    path.line_to(p(q));
                }
                anchor.get_or_insert_with(|| p(&l[l.len() / 2]));
            }
            Some((Geom::path(path), anchor?, "line"))
        }
        GeomType::Polygon => {
            // Rings as the archive wrote them; exteriors made clockwise (positive) so features
            // batched into one path fill as a union under the non-zero rule. Producers disagree on
            // winding, so a feature whose first ring runs the other way is reversed as a whole
            // (its holes with it) — spec-conformant tiles are untouched.
            let rings: Vec<&Vec<(i32, i32)>> = paths.iter().filter(|r| r.len() >= 3).collect();
            let first = rings.first()?;
            let flip = area2(first) < 0;
            let mut b = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
            for (i, r) in rings.iter().enumerate() {
                let n = r.len() - usize::from(r.first() == r.last());
                let pts: Vec<Vec2> = if flip { r[..n].iter().rev().map(p).collect() } else { r[..n].iter().map(p).collect() };
                path.move_to(pts[0]);
                for q in &pts[1..] {
                    path.line_to(*q);
                }
                path.close();
                if i == 0 {
                    for q in &pts {
                        b = (b.0.min(q.x), b.1.min(q.y), b.2.max(q.x), b.3.max(q.y));
                    }
                }
            }
            Some((Geom::path(path), Vec2::new((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0), "polygon"))
        }
        GeomType::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_geo::mvt::{Feature, Layer};

    #[test]
    fn layers_become_tables_and_tile_unit_paths() {
        let mut water = Layer::new("water");
        water.extent = 512; // normalized to 4096 units
        // A counter-clockwise square with a clockwise hole: the producer's opposite convention.
        water.features.push(Feature {
            id: Some(7),
            geom_type: GeomType::Polygon,
            geometry: vec![vec![(0, 0), (0, 100), (100, 100), (100, 0)], vec![(10, 10), (20, 10), (20, 20), (10, 20)]],
            properties: vec![("kind".into(), MvtValue::String("lake".into()))],
        });
        water.features.push(Feature { id: None, geom_type: GeomType::Point, geometry: vec![vec![(256, 128)]], properties: vec![("rank".into(), MvtValue::Int(3))] });
        let bytes = VectorTile { layers: vec![water] }.encode();
        let t = decode(TileId::new(3, 1, 2), &bytes).unwrap();
        let l = &t.layers[0];
        assert_eq!(l.table.len(), 2);
        assert_eq!(l.table.column("$type"), Some(&Column::Str(vec![Some(Arc::from("polygon")), Some(Arc::from("point"))])));
        assert!(matches!(l.table.column("rank"), Some(Column::Num(v)) if v[0].is_nan() && v[1] == 3.0));
        assert!(matches!(l.table.column("kind"), Some(Column::Str(v)) if v[1].is_none()));
        assert_eq!(l.anchors[1], Vec2::new(2048.0, 1024.0));
        let Geom::Path { path } = &l.geoms[0] else { panic!() };
        let flat = path.flatten(0.1);
        assert_eq!(flat.len(), 2);
        assert!(datars_math::path::signed_area(&flat[0].0) > 0.0, "exterior made clockwise");
        assert!(datars_math::path::signed_area(&flat[1].0) < 0.0, "hole reversed with it");
        assert_eq!(path.bounds().w, 800.0, "100 of 512 → 800 of 4096 units");
    }
}
