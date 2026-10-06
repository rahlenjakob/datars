//! A small synthetic archive for tests: z0–3 over the whole world, with water polygons, two road
//! classes and a few named places — built with the same encoder and writer the build pipeline uses.

use datars_geo::mvt::{from_feature, Layer, VectorTile};
use datars_geo::pmtiles::{Writer, WriterOptions};
use datars_geo::{Feature, Geometry, TileId};
use datars_math::Vec2;

fn square(lon: f64, lat: f64, r: f64) -> Geometry {
    Geometry::Polygon(vec![vec![Vec2::new(lon - r, lat - r), Vec2::new(lon + r, lat - r), Vec2::new(lon + r, lat + r), Vec2::new(lon - r, lat + r), Vec2::new(lon - r, lat - r)]])
}

pub(crate) fn features() -> Vec<(&'static str, Feature)> {
    let road = |a: (f64, f64), b: (f64, f64), kind: &str| Feature::new(Geometry::LineString(vec![Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)])).with_property("kind", serde_json::json!(kind));
    let place = |lon: f64, lat: f64, name: &str, pop: f64| Feature::new(Geometry::Point(Vec2::new(lon, lat))).with_property("name", serde_json::json!(name)).with_property("pop", serde_json::json!(pop));
    let water = |g: Geometry| Feature::new(g).with_property("kind", serde_json::json!("sea"));
    vec![
        ("water", water(square(18.0, 59.3, 2.0))),
        ("water", water(square(-40.0, 30.0, 10.0))),
        ("roads", road((10.0, 55.0), (24.0, 66.0), "major")),
        ("roads", road((12.0, 57.0), (20.0, 60.0), "major")),
        ("roads", road((14.0, 58.0), (19.0, 62.0), "minor")),
        ("places", place(18.07, 59.33, "Stockholm", 1_000_000.0)),
        ("places", place(18.1, 59.35, "Lidingö", 40_000.0)),
        ("places", place(11.97, 57.71, "Göteborg", 600_000.0)),
    ]
}

/// Every tile z0..=`maxzoom` that holds something, as a PMTiles archive.
pub(crate) fn archive(maxzoom: u8) -> Vec<u8> {
    let feats = features();
    let mut w = Writer::new(WriterOptions::default());
    for z in 0..=maxzoom {
        let n = 1u32 << z;
        for x in 0..n {
            for y in 0..n {
                let t = TileId::new(z, x, y);
                let mut layers: Vec<Layer> = Vec::new();
                for (name, f) in &feats {
                    let Some(mf) = from_feature(f, t, 4096, 64) else { continue };
                    match layers.iter_mut().find(|l| l.name == *name) {
                        Some(l) => l.features.push(mf),
                        None => {
                            let mut l = Layer::new(*name);
                            l.features.push(mf);
                            layers.push(l);
                        }
                    }
                }
                if !layers.is_empty() {
                    w.add_tile(t, VectorTile { layers }.encode());
                }
            }
        }
    }
    w.finish().unwrap()
}

/// Dense tiles: a `n` × `n/2` grid of short roads over the world (thousands of features a tile at
/// low zooms), z0..=`maxzoom` — styling them is the expensive part of a frame.
pub(crate) fn dense_archive(maxzoom: u8, n: usize) -> Vec<u8> {
    let mut w = Writer::new(WriterOptions::default());
    let roads: Vec<Feature> = (0..n)
        .flat_map(|i| (0..n / 2).map(move |j| (i, j)))
        .map(|(i, j)| {
            let lon = -179.0 + 358.0 * i as f64 / n as f64;
            let lat = -79.0 + 158.0 * j as f64 / (n / 2) as f64;
            Feature::new(Geometry::LineString(vec![Vec2::new(lon, lat), Vec2::new(lon + 0.5, lat + 0.3)])).with_property("kind", serde_json::json!("minor"))
        })
        .collect();
    for z in 0..=maxzoom {
        let k = 1u32 << z;
        for x in 0..k {
            for y in 0..k {
                let t = TileId::new(z, x, y);
                let mut l = Layer::new("roads");
                l.features.extend(roads.iter().filter_map(|f| from_feature(f, t, 4096, 64)));
                if !l.features.is_empty() {
                    w.add_tile(t, VectorTile { layers: vec![l] }.encode());
                }
            }
        }
    }
    w.finish().unwrap()
}
