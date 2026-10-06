//! Convert raw Overpass JSON (OpenStreetMap, fetched with `out geom;`) into the
//! GeoJSON our normal pipeline already understands.
//!
//! `out geom;` inlines each way/relation-member's node coordinates, so we never
//! resolve node references: a way *is* its coordinate list. We split elements
//! into three FeatureCollections — water (polygons), roads (lines), buildings
//! (polygons) — and write them next to the Natural Earth inputs.
//!
//! Water areas are frequently multipolygon *relations* (island-dotted bays like
//! Stockholm's). We collect all their member rings into one polygon feature and
//! rely on the renderer's even-odd fill, so inner rings (islands) become holes.

use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};
use serde_json::{json, Value as Json};


/// Paths of the GeoJSON files produced by [`convert`].
pub struct OsmPaths {
    pub water: String,
    pub roads: String,
    pub buildings: String,
}

/// Read one or more Overpass JSON files and write water/roads/buildings GeoJSON
/// into `data_dir`.
pub fn convert(inputs: &[String], data_dir: &str) -> Result<OsmPaths, String> {
    let mut water: Vec<Json> = Vec::new();
    let mut roads: Vec<Json> = Vec::new();
    let mut buildings: Vec<Json> = Vec::new();

    for path in inputs {
        if !std::path::Path::new(path).exists() {
            eprintln!("  ! OSM input {path} not found, skipping");
            continue;
        }
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let doc: Json = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let Some(elements) = doc.get("elements").and_then(Json::as_array) else {
            continue;
        };
        for el in elements {
            classify(el, &mut water, &mut roads, &mut buildings);
        }
    }

    let water_path = format!("{data_dir}/osm_water.geojson");
    let roads_path = format!("{data_dir}/osm_roads.geojson");
    let buildings_path = format!("{data_dir}/osm_buildings.geojson");
    write_fc(&water_path, water)?;
    write_fc(&roads_path, roads)?;
    write_fc(&buildings_path, buildings)?;

    Ok(OsmPaths {
        water: water_path,
        roads: roads_path,
        buildings: buildings_path,
    })
}

fn classify(el: &Json, water: &mut Vec<Json>, roads: &mut Vec<Json>, buildings: &mut Vec<Json>) {
    let etype = el.get("type").and_then(Json::as_str).unwrap_or("");
    let tags = el.get("tags");
    let has_tag = |k: &str| tags.and_then(|t| t.get(k)).is_some();
    let tag = |k: &str| tags.and_then(|t| t.get(k)).and_then(Json::as_str);

    match etype {
        "way" => {
            let coords = geometry_to_coords(el.get("geometry"));
            if coords.len() < 2 {
                return;
            }
            if has_tag("building") {
                buildings.push(polygon_feature(vec![closed(coords)], json!({})));
            } else if tag("natural") == Some("water") {
                water.push(polygon_feature(vec![closed(coords)], json!({})));
            } else if has_tag("highway") {
                roads.push(line_feature(
                    coords,
                    json!({
                        "type": tag("highway").unwrap_or("road"),
                        "name": tag("name").unwrap_or(""),
                    }),
                ));
            }
        }
        "relation"
            // Water multipolygons: member ways are boundary *segments* that must
            // be stitched into closed rings before they mean anything.
            if tag("natural") == Some("water") => {
                if let Some(members) = el.get("members").and_then(Json::as_array) {
                    let segments: Vec<Vec<[f64; 2]>> = members
                        .iter()
                        .map(|m| geometry_to_coords(m.get("geometry")))
                        .filter(|r| r.len() >= 2)
                        .collect();
                    let rings = assemble_rings(segments);
                    if let Some(f) = rings_to_multipolygon_feature(rings) {
                        water.push(f);
                    }
                }
            }
        _ => {}
    }
}

/// Turn a set of closed rings (arbitrary order, mixed outer/inner) into a valid
/// MultiPolygon GeoJSON feature using even-odd (XOR): overlapping/nested rings
/// resolve correctly, so islands become holes and the exterior is unambiguous —
/// no reliance on ring order. This is what makes multipolygon water robust.
fn rings_to_multipolygon_feature(rings: Vec<Vec<[f64; 2]>>) -> Option<Json> {
    let mut acc: MultiPolygon<f64> = MultiPolygon(Vec::new());
    for ring in &rings {
        if ring.len() < 4 {
            continue;
        }
        let ls = LineString::from(
            ring.iter().map(|p| Coord { x: p[0], y: p[1] }).collect::<Vec<_>>(),
        );
        let piece = MultiPolygon(vec![Polygon::new(ls, vec![])]);
        acc = acc.xor(&piece);
    }
    if acc.0.is_empty() {
        return None;
    }
    let polys: Vec<Json> = acc
        .0
        .iter()
        .map(|poly| {
            let mut ring_list = vec![ls_to_json(poly.exterior())];
            for hole in poly.interiors() {
                ring_list.push(ls_to_json(hole));
            }
            Json::Array(ring_list)
        })
        .collect();
    Some(json!({
        "type": "Feature",
        "properties": {},
        "geometry": { "type": "MultiPolygon", "coordinates": polys }
    }))
}

fn ls_to_json(ls: &LineString<f64>) -> Json {
    Json::Array(ls.coords().map(|c| json!([c.x, c.y])).collect())
}

fn pt_eq(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
}

/// Stitch OSM multipolygon member *segments* into closed rings by joining ways
/// that share endpoints. Returns the closed rings (outer + inner); the renderer
/// fills them even-odd, so islands become holes automatically. Without this a
/// relation's ways are just disconnected arcs and fill into garbage.
pub(crate) fn assemble_rings(mut segments: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
    segments.retain(|s| s.len() >= 2);
    let mut rings: Vec<Vec<[f64; 2]>> = Vec::new();

    while let Some(mut cur) = segments.pop() {
        // Grow `cur` by attaching matching segments at its end until it closes.
        let mut guard = segments.len() + 1;
        loop {
            if cur.len() >= 4 && pt_eq(cur[0], *cur.last().unwrap()) {
                break; // closed
            }
            if guard == 0 {
                break;
            }
            guard -= 1;
            let last = *cur.last().unwrap();
            let Some(pos) = segments
                .iter()
                .position(|s| pt_eq(s[0], last) || pt_eq(*s.last().unwrap(), last))
            else {
                break; // nothing connects; leave open
            };
            let mut seg = segments.remove(pos);
            if pt_eq(*seg.last().unwrap(), last) {
                seg.reverse();
            }
            cur.extend_from_slice(&seg[1..]); // skip the shared point
        }

        if cur.len() >= 4 {
            if !pt_eq(cur[0], *cur.last().unwrap()) {
                cur.push(cur[0]); // best-effort close
            }
            rings.push(cur);
        }
    }
    rings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_segments_into_a_ring() {
        // A square split into 4 separate segments, given out of order & reversed.
        let segs = vec![
            vec![[0.0, 0.0], [1.0, 0.0]],
            vec![[1.0, 1.0], [1.0, 0.0]], // reversed
            vec![[1.0, 1.0], [0.0, 1.0]],
            vec![[0.0, 1.0], [0.0, 0.0]],
        ];
        let rings = assemble_rings(segs);
        assert_eq!(rings.len(), 1);
        let r = &rings[0];
        assert!(pt_eq(r[0], *r.last().unwrap()), "ring must be closed");
        assert!(r.len() >= 5);
    }

    #[test]
    fn separate_loops_become_separate_rings() {
        let segs = vec![
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]],
            vec![[5.0, 5.0], [6.0, 5.0], [6.0, 6.0], [5.0, 5.0]],
        ];
        assert_eq!(assemble_rings(segs).len(), 2);
    }
}

fn geometry_to_coords(geom: Option<&Json>) -> Vec<[f64; 2]> {
    geom.and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| {
                    Some([p.get("lon")?.as_f64()?, p.get("lat")?.as_f64()?])
                })
                .collect()
        })
        .unwrap_or_default()
}

fn closed(mut ring: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if ring.first() != ring.last() {
        if let Some(&first) = ring.first() {
            ring.push(first);
        }
    }
    ring
}

fn polygon_feature(rings: Vec<Vec<[f64; 2]>>, props: Json) -> Json {
    let coords: Vec<Json> = rings
        .into_iter()
        .map(|r| Json::Array(r.into_iter().map(|c| json!([c[0], c[1]])).collect()))
        .collect();
    json!({
        "type": "Feature",
        "properties": props,
        "geometry": { "type": "Polygon", "coordinates": coords }
    })
}

fn line_feature(coords: Vec<[f64; 2]>, props: Json) -> Json {
    let pts: Vec<Json> = coords.into_iter().map(|c| json!([c[0], c[1]])).collect();
    json!({
        "type": "Feature",
        "properties": props,
        "geometry": { "type": "LineString", "coordinates": pts }
    })
}

fn write_fc(path: &str, features: Vec<Json>) -> Result<(), String> {
    let n = features.len();
    let fc = json!({ "type": "FeatureCollection", "features": features });
    std::fs::write(path, serde_json::to_vec(&fc).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    eprintln!("  wrote {path} ({n} features)");
    Ok(())
}
