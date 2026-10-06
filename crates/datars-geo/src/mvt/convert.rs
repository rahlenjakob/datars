//! Between MVT features (tile-local integers) and lon/lat `Geometry`.
//!
//! Decoding maps `0..extent` tile coordinates through the tile's Web-Mercator square back to
//! lon/lat. Encoding (for the build pipeline) projects to the tile, clips to the tile plus a
//! buffer with the rectangle clipper, quantizes, and emits rings in MVT winding (exteriors
//! positive in y-down tile space, holes negative) — which is what the clipper produces.

use super::{Feature as MvtFeature, GeomType, Value};
use crate::clip::{clip_line, clip_polygon};
use crate::geometry::{Feature, Geometry, Polygon};
use crate::tile::{lonlat_to_world, world_to_lonlat, TileId};
use datars_math::{Rect, Vec2};

/// Integer paths in tile-local coordinates, as `mvt::Feature::geometry` holds them.
pub type TilePaths = Vec<Vec<(i32, i32)>>;

fn scale(tile: TileId) -> f64 {
    (1u64 << tile.z.min(62)) as f64
}

/// Tile-local coordinates → lon/lat.
pub fn local_to_lonlat(p: (i32, i32), tile: TileId, extent: u32) -> Vec2 {
    let (e, s) = (extent.max(1) as f64, scale(tile));
    world_to_lonlat(Vec2::new((tile.x as f64 + p.0 as f64 / e) / s, (tile.y as f64 + p.1 as f64 / e) / s))
}

/// lon/lat → tile-local coordinates (unquantized).
pub fn lonlat_to_local(p: Vec2, tile: TileId, extent: u32) -> Vec2 {
    let w = lonlat_to_world(p);
    let (e, s) = (extent.max(1) as f64, scale(tile));
    Vec2::new((w.x * s - tile.x as f64) * e, (w.y * s - tile.y as f64) * e)
}

/// An MVT feature's geometry in lon/lat. Polygons are grouped by winding per the spec.
pub fn to_geometry(f: &MvtFeature, tile: TileId, extent: u32) -> Geometry {
    let ll = |p: &(i32, i32)| local_to_lonlat(*p, tile, extent);
    match f.geom_type {
        GeomType::Point => {
            let pts: Vec<Vec2> = f.geometry.iter().flatten().map(ll).collect();
            if pts.len() == 1 { Geometry::Point(pts[0]) } else { Geometry::MultiPoint(pts) }
        }
        GeomType::LineString => {
            let mut ls: Vec<Vec<Vec2>> = f.geometry.iter().filter(|l| l.len() > 1).map(|l| l.iter().map(ll).collect()).collect();
            if ls.len() == 1 { Geometry::LineString(ls.remove(0)) } else { Geometry::MultiLineString(ls) }
        }
        GeomType::Polygon => {
            let mut ps: Vec<Polygon> = f
                .polygons()
                .iter()
                .map(|p| {
                    p.iter()
                        .map(|r| {
                            let mut ring: Vec<Vec2> = r.iter().map(ll).collect();
                            ring.push(ring[0]);
                            ring
                        })
                        .collect()
                })
                .collect();
            if ps.len() == 1 { Geometry::Polygon(ps.remove(0)) } else { Geometry::MultiPolygon(ps) }
        }
        GeomType::Unknown => Geometry::default(),
    }
}

/// An MVT feature as a `Feature` (id stringified, properties as JSON).
pub fn to_feature(f: &MvtFeature, tile: TileId, extent: u32) -> Feature {
    Feature {
        id: f.id.map(|i| i.to_string()),
        properties: f.properties.iter().map(|(k, v)| (k.clone(), v.to_json())).collect(),
        geometry: to_geometry(f, tile, extent),
    }
}

fn quantize(pts: impl Iterator<Item = Vec2>) -> Vec<(i32, i32)> {
    let mut out: Vec<(i32, i32)> = Vec::new();
    for p in pts {
        let q = (p.x.round() as i32, p.y.round() as i32);
        if out.last() != Some(&q) {
            out.push(q);
        }
    }
    out
}

fn area2(r: &[(i32, i32)]) -> i128 {
    let n = r.len();
    (0..n).map(|i| {
        let (a, b) = (r[i], r[(i + 1) % n]);
        a.0 as i128 * b.1 as i128 - b.0 as i128 * a.1 as i128
    }).sum()
}

/// lon/lat geometry → MVT paths for `tile`: projected, clipped to the tile plus `buffer` (tile
/// units), quantized. Collections are flattened by dimension (polygons if any, else lines, else
/// points). `None` if nothing is left.
pub fn from_geometry(g: &Geometry, tile: TileId, extent: u32, buffer: u32) -> Option<(GeomType, TilePaths)> {
    let e = extent.max(1) as f64;
    let b = buffer as f64;
    let rect = Rect::new(-b, -b, e + 2.0 * b, e + 2.0 * b);
    let local = |p: &Vec2| lonlat_to_local(*p, tile, extent);
    let polys = g.polygons();
    if !polys.is_empty() {
        let mut rings = Vec::new();
        for p in polys {
            let lp: Polygon = p.iter().map(|r| r.iter().map(local).collect()).collect();
            for piece in clip_polygon(&lp, rect) {
                for (i, r) in piece.iter().enumerate() {
                    let mut q = quantize(r.iter().copied());
                    if q.len() > 1 && q.first() == q.last() {
                        q.pop();
                    }
                    let a = area2(&q);
                    // Keep rings whose winding survived quantization (exterior > 0, hole < 0).
                    if q.len() >= 3 && ((i == 0 && a > 0) || (i > 0 && a < 0)) {
                        rings.push(q);
                    } else if i == 0 {
                        break; // exterior collapsed: drop its holes too
                    }
                }
            }
        }
        return (!rings.is_empty()).then_some((GeomType::Polygon, rings));
    }
    let lines = g.lines();
    if !lines.is_empty() {
        let paths: Vec<Vec<(i32, i32)>> = lines
            .iter()
            .flat_map(|l| clip_line(&l.iter().map(local).collect::<Vec<_>>(), rect))
            .map(|l| quantize(l.into_iter()))
            .filter(|q| q.len() >= 2)
            .collect();
        return (!paths.is_empty()).then_some((GeomType::LineString, paths));
    }
    let pts: Vec<Vec<(i32, i32)>> = g
        .points()
        .iter()
        .map(|p| {
            let q = local(p);
            (q.x.round(), q.y.round())
        })
        .filter(|&(x, y)| rect.contains(Vec2::new(x, y)))
        .map(|(x, y)| vec![(x as i32, y as i32)])
        .collect();
    (!pts.is_empty()).then_some((GeomType::Point, pts))
}

/// A `Feature` as an MVT feature for `tile` (see `from_geometry`). Numeric ids become MVT ids;
/// properties that MVT can't represent (null, arrays, objects) are dropped. Properties are in key
/// order, so encoding is deterministic.
pub fn from_feature(f: &Feature, tile: TileId, extent: u32, buffer: u32) -> Option<MvtFeature> {
    let (geom_type, geometry) = from_geometry(&f.geometry, tile, extent, buffer)?;
    Some(MvtFeature {
        id: f.id.as_deref().and_then(|s| s.parse::<u64>().ok()),
        geom_type,
        geometry,
        properties: f.properties.iter().filter_map(|(k, v)| Some((k.clone(), Value::from_json(v)?))).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvt::{Layer, VectorTile};

    #[test]
    fn local_lonlat_round_trip() {
        let t = TileId::new(5, 17, 9);
        for p in [(0, 0), (4096, 4096), (123, 3000)] {
            let ll = local_to_lonlat(p, t, 4096);
            let back = lonlat_to_local(ll, t, 4096);
            assert!((back.x - p.0 as f64).abs() < 1e-6 && (back.y - p.1 as f64).abs() < 1e-6);
        }
    }

    #[test]
    fn geometry_encodes_clipped_and_decodes_back() {
        let t = TileId::new(1, 1, 0); // NE quadrant: lon 0..180, lat 0..85
        // A square straddling the tile's west edge (lon 0) with a hole inside the tile.
        let outer = vec![Vec2::new(-20.0, 10.0), Vec2::new(40.0, 10.0), Vec2::new(40.0, 50.0), Vec2::new(-20.0, 50.0), Vec2::new(-20.0, 10.0)];
        let hole = vec![Vec2::new(10.0, 20.0), Vec2::new(20.0, 20.0), Vec2::new(20.0, 30.0), Vec2::new(10.0, 30.0), Vec2::new(10.0, 20.0)];
        let f = Feature::new(Geometry::Polygon(vec![outer, hole])).with_id("42").with_property("name", serde_json::json!("sq"));
        let mf = from_feature(&f, t, 4096, 0).unwrap();
        assert_eq!(mf.id, Some(42));
        assert_eq!(mf.geom_type, GeomType::Polygon);
        assert_eq!(mf.geometry.len(), 2, "exterior + hole");
        assert!(mf.geometry.iter().flatten().all(|p| p.0 >= 0 && p.0 <= 4096 && p.1 >= 0 && p.1 <= 4096));
        let mut layer = Layer::new("regions");
        layer.features.push(mf);
        let bytes = VectorTile { layers: vec![layer] }.encode();
        let back = VectorTile::decode(&bytes).unwrap();
        let feat = to_feature(&back.layers[0].features[0], t, 4096);
        assert_eq!(feat.id.as_deref(), Some("42"));
        assert_eq!(feat.property_str("name").as_deref(), Some("sq"));
        let Geometry::Polygon(p) = &feat.geometry else { panic!("{:?}", feat.geometry) };
        assert_eq!(p.len(), 2);
        let west = p[0].iter().map(|q| q.x).fold(f64::INFINITY, f64::min);
        assert!(west.abs() < 0.1, "clipped at lon 0: {west}");
        assert!(p[1].iter().any(|q| (q.x - 10.0).abs() < 0.1 && (q.y - 20.0).abs() < 0.1));
    }

    #[test]
    fn lines_and_points() {
        let t = TileId::new(0, 0, 0);
        let g = Geometry::LineString(vec![Vec2::new(-10.0, 0.0), Vec2::new(10.0, 0.0)]);
        let (k, paths) = from_geometry(&g, t, 4096, 64).unwrap();
        assert_eq!(k, GeomType::LineString);
        assert_eq!(paths[0].len(), 2);
        let pts = Geometry::MultiPoint(vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 89.9)]);
        let (k, paths) = from_geometry(&pts, t, 4096, 0).unwrap();
        assert_eq!(k, GeomType::Point);
        assert_eq!(paths, vec![vec![(2048, 2048)], vec![(2048, 0)]], "latitudes clamp to the square");
        assert!(from_geometry(&Geometry::default(), t, 4096, 0).is_none());
    }
}
