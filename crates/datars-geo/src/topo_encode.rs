//! Polygon features → a compact TopoJSON topology for delivery: coordinates as integers in units
//! of 10^-d (delta-encoded), and every border between neighbours stored once. Decoding divides by
//! 10^d (see `topojson`), which gives back exactly the doubles the decimal text parsed to — so a
//! chart drawn from the topology is bit-identical to one drawn from the original GeoJSON (P1).
//!
//! Every ring keeps its start point and direction: rings are cut at shared-border junctions *and*
//! at every ring's start (for every ring passing through it, so neighbours still share arcs), and
//! rebuilt in their original order.

use crate::geometry::{FeatureCollection, Geometry, Polygon};
use std::collections::{BTreeMap, BTreeSet};

type Pt = (i64, i64);

/// The fewest decimals (≤ 7) at which every coordinate of `fc` is exact, if any.
pub fn decimals_of(fc: &FeatureCollection) -> Option<u32> {
    (0..=7).find(|&d| {
        let s = pow10(d);
        let mut ok = true;
        for f in &fc.features {
            f.geometry.for_each_coord(|p| ok &= quantize(p.x, s).is_some() && quantize(p.y, s).is_some());
        }
        ok
    })
}

fn pow10(d: u32) -> f64 {
    (0..d).fold(1.0, |a, _| a * 10.0)
}

/// `x` as an integer count of 10^-d, if dividing back gives exactly `x`.
fn quantize(x: f64, scale: f64) -> Option<i64> {
    let q = (x * scale).round();
    (q.is_finite() && q.abs() < 9.0e15 && q / scale == x).then_some(q as i64)
}

/// Encode polygonal features (Polygon / MultiPolygon, or empty) as a topology with one
/// `GeometryCollection` named `object`. `None` when a geometry isn't polygonal or a coordinate
/// has more than `decimals` decimals (the decode wouldn't be exact).
pub fn encode_decimal(fc: &FeatureCollection, decimals: u32, object: &str) -> Option<String> {
    let scale = pow10(decimals);
    // Rings as integer cycles (closing point dropped), per feature and polygon.
    let mut rings: Vec<Vec<Pt>> = Vec::new();
    enum Shape {
        Null,
        Polygon(Vec<usize>),
        Multi(Vec<Vec<usize>>),
    }
    let mut shapes = Vec::with_capacity(fc.features.len());
    let add_polygon = |p: &Polygon, rings: &mut Vec<Vec<Pt>>| -> Option<Vec<usize>> {
        let mut ids = Vec::with_capacity(p.len());
        for r in p {
            if r.len() < 4 || r.first() != r.last() {
                return None;
            }
            let cyc: Option<Vec<Pt>> = r[..r.len() - 1].iter().map(|v| Some((quantize(v.x, scale)?, quantize(v.y, scale)?))).collect();
            ids.push(rings.len());
            rings.push(cyc?);
        }
        Some(ids)
    };
    for f in &fc.features {
        shapes.push(match &f.geometry {
            Geometry::Polygon(p) => Shape::Polygon(add_polygon(p, &mut rings)?),
            Geometry::MultiPolygon(ps) => Shape::Multi(ps.iter().map(|p| add_polygon(p, &mut rings)).collect::<Option<_>>()?),
            Geometry::Collection(g) if g.is_empty() => Shape::Null,
            _ => return None,
        });
    }

    // Junctions: points met with different neighbours (where borders part), and ring starts.
    let mut seen: BTreeMap<Pt, (Pt, Pt)> = BTreeMap::new();
    let mut junctions: BTreeSet<Pt> = BTreeSet::new();
    for cyc in &rings {
        let m = cyc.len();
        junctions.insert(cyc[0]);
        for i in 0..m {
            let (a, b) = (cyc[(i + m - 1) % m], cyc[(i + 1) % m]);
            let pair = if a <= b { (a, b) } else { (b, a) };
            match seen.get(&cyc[i]) {
                Some(p) if *p != pair => {
                    junctions.insert(cyc[i]);
                }
                Some(_) => {}
                None => {
                    seen.insert(cyc[i], pair);
                }
            }
        }
    }

    // Cut rings into arcs at junctions; store each arc once (a neighbour uses it reversed).
    let mut arcs: Vec<Vec<Pt>> = Vec::new();
    let mut index: BTreeMap<Vec<Pt>, usize> = BTreeMap::new();
    let mut ring_arcs: Vec<Vec<i64>> = Vec::with_capacity(rings.len());
    for cyc in &rings {
        let m = cyc.len();
        let cuts: Vec<usize> = (0..m).filter(|&i| junctions.contains(&cyc[i])).collect();
        let mut refs = Vec::with_capacity(cuts.len());
        for (k, &from) in cuts.iter().enumerate() {
            let to = cuts.get(k + 1).copied().unwrap_or(m);
            let arc: Vec<Pt> = (from..=to).map(|i| cyc[i % m]).collect();
            let rev: Vec<Pt> = arc.iter().rev().copied().collect();
            let r = if let Some(&i) = index.get(&arc) {
                i as i64
            } else if let Some(&i) = index.get(&rev) {
                !(i as i64)
            } else {
                index.insert(arc.clone(), arcs.len());
                arcs.push(arc);
                (arcs.len() - 1) as i64
            };
            refs.push(r);
        }
        ring_arcs.push(refs);
    }

    let geometries: Vec<serde_json::Value> = fc
        .features
        .iter()
        .zip(&shapes)
        .map(|(f, s)| {
            let mut g = serde_json::Map::new();
            match s {
                Shape::Null => {
                    g.insert("type".into(), serde_json::Value::Null);
                }
                Shape::Polygon(rs) => {
                    g.insert("type".into(), "Polygon".into());
                    g.insert("arcs".into(), serde_json::json!(rs.iter().map(|r| &ring_arcs[*r]).collect::<Vec<_>>()));
                }
                Shape::Multi(ps) => {
                    g.insert("type".into(), "MultiPolygon".into());
                    g.insert("arcs".into(), serde_json::json!(ps.iter().map(|rs| rs.iter().map(|r| &ring_arcs[*r]).collect::<Vec<_>>()).collect::<Vec<_>>()));
                }
            }
            if let Some(id) = &f.id {
                g.insert("id".into(), id.clone().into());
            }
            if !f.properties.is_empty() {
                g.insert("properties".into(), serde_json::json!(f.properties));
            }
            serde_json::Value::Object(g)
        })
        .collect();
    let delta: Vec<Vec<[i64; 2]>> = arcs
        .iter()
        .map(|a| {
            let mut prev = (0, 0);
            a.iter()
                .map(|&(x, y)| {
                    let d = [x - prev.0, y - prev.1];
                    prev = (x, y);
                    d
                })
                .collect()
        })
        .collect();
    let topo = serde_json::json!({
        "type": "Topology",
        "transform": { "scale": [1.0 / scale, 1.0 / scale], "translate": [0, 0] },
        "objects": { object: { "type": "GeometryCollection", "geometries": geometries } },
        "arcs": delta,
    });
    Some(topo.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_geojson, parse_topojson, GeoJsonOptions};

    #[test]
    fn neighbours_share_borders_and_decoding_is_exact() {
        // Two squares sharing an edge, with ring starts in the middle of their sides, and an
        // island; coordinates with two decimals.
        let gj = br#"{"type":"FeatureCollection","features":[
          {"type":"Feature","id":"A","properties":{"name":"A","n":1},"geometry":{"type":"Polygon","coordinates":[[[0.5,0],[1.1,0],[1.1,0.33],[1.1,1.07],[0,1.07],[0,0],[0.5,0]]]}},
          {"type":"Feature","id":"B","properties":{"name":"B"},"geometry":{"type":"MultiPolygon","coordinates":[
            [[[1.1,0.33],[1.1,0],[2.2,0],[2.2,1.07],[1.1,1.07],[1.1,0.33]]],
            [[[3,3],[3.01,3],[3.01,3.01],[3,3]]]]}}]}"#;
        let fc = parse_geojson(gj, &GeoJsonOptions::default()).unwrap();
        assert_eq!(decimals_of(&fc), Some(2));
        let topo = encode_decimal(&fc, 2, "regions").unwrap();
        let t = parse_topojson(topo.as_bytes(), &GeoJsonOptions::default()).unwrap();
        let back = t.features("regions").unwrap();
        assert_eq!(back, fc, "same ids, properties and every coordinate bit for bit, ring starts kept");
        // The shared edge x = 1.1 (0 … 1.07) is stored once: A uses it one way, B the other.
        let v: serde_json::Value = serde_json::from_str(&topo).unwrap();
        let refs = v["objects"]["regions"]["geometries"].to_string();
        assert!(refs.contains("-"), "a reversed arc reference: {refs}");
    }

    #[test]
    fn inexact_or_non_polygonal_input_is_refused() {
        let gj = br#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0.123,0],[1,0],[1,1],[0.123,0]]]}}]}"#;
        let fc = parse_geojson(gj, &GeoJsonOptions::default()).unwrap();
        assert!(encode_decimal(&fc, 2, "x").is_none(), "0.123 needs three decimals");
        assert!(encode_decimal(&fc, 3, "x").is_some());
        let line = br#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0,0],[1,1]]}}]}"#;
        assert!(encode_decimal(&parse_geojson(line, &GeoJsonOptions::default()).unwrap(), 2, "x").is_none());
    }
}
