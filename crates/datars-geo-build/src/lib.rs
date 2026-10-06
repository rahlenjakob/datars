//! `datars-geo-build` — the build-time geo pipeline (docs/09-geo.md). Our own tiles, from open
//! data, served as static files or bundled into apps — no tile service at runtime.
//!
//! - [`atlas`]: a keyed, simplified region set (countries, subdivisions, municipalities) from
//!   Natural Earth / OSM boundary GeoJSON — the choropleth-ready geometry maps join data to.
//! - [`tiles`]: GeoJSON layers → clipped, simplified MVT per zoom → one deduplicated PMTiles archive.
//! - [`osm`]: Overpass JSON (OpenStreetMap, `out geom;`) → water / roads / buildings GeoJSON
//!   (multipolygon water assembled even-odd).
//! - [`camera`]: camera-aware extraction — the tiles a story's views and flights need.
//! - [`recipe`]: the standard basemap layers (what std `basemap` draws) over the local open data,
//!   as a tile job for a camera cover — or over data fetched on demand ([`recipe::auto_job`]).
//! - [`shp`]: polygons out of an ESRI shapefile (osmdata land polygons), only where wanted.
//! - [`fetch`]: build-time acquisition through a local cache — Natural Earth, osmdata, Overpass.
//! - [`overpass`]: OpenStreetMap per cell and zoom band: queries, and answers → layers.
//! - [`coast`]: land polygons from the OSM coastline, per box.
//! - [`auto`]: automatic basemaps — views in, a camera-aware archive out, inputs fetched and cached.

pub mod auto;
pub mod camera;
pub mod coast;
pub mod fetch;
pub mod osm;
pub mod overpass;
pub mod recipe;
pub mod shp;
pub mod zip;

use datars_geo::mvt::{Layer, VectorTile};
use datars_geo::pmtiles::{Writer, WriterOptions};
use datars_geo::simplify::{simplify_geometry, SimplifyMethod};
use datars_geo::{parse_geojson, FeatureCollection, GeoBbox, GeoJsonOptions, TileId};
use std::collections::BTreeMap;

/// Build a compact atlas: ids from `id_prop`, a `name` from `name_props` (first present), geometry
/// simplified by `tolerance` degrees (Visvalingam, ring-safe), coordinates rounded to `decimals`.
pub fn atlas(input: &[u8], id_prop: &str, name_props: &[&str], tolerance: f64, decimals: u32) -> Result<String, String> {
    let fc = parse_geojson(input, &GeoJsonOptions { id_property: Some(id_prop.to_string()) }).map_err(|e| e.to_string())?;
    let mut out = FeatureCollection::default();
    let mut seen = std::collections::BTreeSet::new();
    for f in fc.features {
        let Some(id) = f.id.clone().filter(|i| !i.is_empty() && i != "-99") else { continue };
        if !seen.insert(id.clone()) {
            continue;
        }
        let name = name_props.iter().find_map(|p| f.property_str(p)).unwrap_or_else(|| id.clone());
        let mut g = datars_geo::Feature::new(simplify_geometry(&f.geometry, tolerance, SimplifyMethod::Visvalingam));
        g.id = Some(id.clone());
        g.properties.insert("id".into(), serde_json::Value::String(id));
        g.properties.insert("name".into(), serde_json::Value::String(name));
        out.features.push(g);
    }
    Ok(datars_geo::geojson::to_geojson_string(&out, Some(decimals)))
}

/// One layer of a tile build. Several specs may write the same layer `name` (different sources
/// per zoom band — Natural Earth for the world, OSM for the city): their features share the layer.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct LayerSpec {
    pub name: String,
    pub input: String,
    #[serde(default)]
    pub minzoom: u8,
    #[serde(default = "z14")]
    pub maxzoom: u8,
    /// Simplification in screen pixels at each zoom.
    #[serde(default = "px")]
    pub simplify_px: f64,
    /// Properties to keep, after renaming and classifying (empty = all scalar properties).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keep: Vec<String>,
    /// [west, south, east, north]: only tiles touching it get this layer (camera-aware extracts:
    /// each zoom band covers the part of the camera path that shows it; a source that exists only
    /// somewhere — a city's OSM extract — is used only there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    /// [west, south, east, north]: tiles touching it don't get this layer — the complement of
    /// another spec's `bbox`, so a coarse source fills in exactly where the detailed one isn't
    /// (Natural Earth roads outside the city's OSM extract, not doubled inside it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outside: Option<[f64; 4]>,
    /// Features to include: every `[property, op, value]` must hold (`== != < <= > >= in`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter: Vec<(String, String, serde_json::Value)>,
    /// Property renames (`{"NAME": "name", "POP_MAX": "pop"}`), applied first, in key order. A
    /// missing, null or empty source leaves the target alone, so `{"NAME": "name", "NAME_SV":
    /// "name"}` takes the Swedish name where there is one and the local name elsewhere.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rename: BTreeMap<String, String>,
    /// A property mapped through a table into another (source schema → ours); unmatched features
    /// are dropped unless `default` is given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classify: Option<Classify>,
    /// Constant properties every feature gets (`{"kind": "lake"}`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, serde_json::Value>,
    /// Polygons smaller than this many square pixels at a zoom are left out of its tiles (the
    /// archipelago's ten thousand islets don't all need to be in a z6 tile).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_area_px: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Classify {
    pub from: String,
    pub to: String,
    pub map: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

fn compare(a: &serde_json::Value, op: &str, b: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    let num = |v: &J| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()));
    match op {
        "in" => b.as_array().is_some_and(|l| l.iter().any(|x| compare(a, "==", x))),
        "==" | "!=" => {
            let eq = match (num(a), num(b)) {
                (Some(x), Some(y)) => x == y,
                _ => a == b,
            };
            eq == (op == "==")
        }
        _ => match (num(a), num(b)) {
            (Some(x), Some(y)) => match op {
                "<" => x < y,
                "<=" => x <= y,
                ">" => x > y,
                ">=" => x >= y,
                _ => false,
            },
            _ => false,
        },
    }
}

/// A source feature in this layer's schema, or `None` if the layer leaves it out.
fn prepare(f: &datars_geo::Feature, spec: &LayerSpec) -> Option<datars_geo::Feature> {
    let mut g = f.clone();
    for (from, to) in &spec.rename {
        match g.properties.remove(from) {
            Some(v) if !(v.is_null() || v.as_str() == Some("")) => {
                g.properties.insert(to.clone(), v);
            }
            _ => {}
        }
    }
    for (prop, op, value) in &spec.filter {
        let v = g.properties.get(prop).cloned().unwrap_or(serde_json::Value::Null);
        if !compare(&v, op, value) {
            return None;
        }
    }
    if let Some(c) = &spec.classify {
        let key = g.properties.get(&c.from).map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()));
        let class = key.and_then(|k| c.map.get(&k).cloned()).or_else(|| c.default.clone())?;
        g.properties.insert(c.to.clone(), serde_json::Value::String(class));
    }
    for (k, v) in &spec.set {
        g.properties.insert(k.clone(), v.clone());
    }
    if !spec.keep.is_empty() {
        g.properties.retain(|k, _| spec.keep.contains(k));
    }
    g.properties.retain(|_, v| v.is_string() || v.is_number() || v.is_boolean());
    Some(g)
}

/// Drop polygons whose exterior covers less than `min_px2` square pixels at zoom `z` (area in
/// Web-Mercator pixels, so the threshold means the same at every latitude).
fn drop_small(g: &datars_geo::Geometry, z: u8, min_px2: f64) -> Option<datars_geo::Geometry> {
    use datars_geo::Geometry as G;
    if min_px2 <= 0.0 {
        return Some(g.clone());
    }
    let size = 512.0 * (1u64 << z) as f64;
    let px_area = |p: &datars_geo::Polygon| -> f64 {
        let pts: Vec<datars_math::Vec2> = p.first().map(|r| r.iter().map(|q| datars_geo::tile::lonlat_to_world(*q) * size).collect()).unwrap_or_default();
        datars_math::path::signed_area(&pts).abs()
    };
    match g {
        G::Polygon(p) => (px_area(p) >= min_px2).then(|| g.clone()),
        G::MultiPolygon(pp) => {
            let kept: Vec<datars_geo::Polygon> = pp.iter().filter(|p| px_area(p) >= min_px2).cloned().collect();
            (!kept.is_empty()).then_some(G::MultiPolygon(kept))
        }
        other => Some(other.clone()),
    }
}

fn z14() -> u8 {
    14
}
fn px() -> f64 {
    1.0
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct TileJob {
    /// What the extract is for and how to rebuild it (for people; not read by the build).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub about: String,
    #[serde(default)]
    pub minzoom: u8,
    #[serde(default = "z14")]
    pub maxzoom: u8,
    /// [west, south, east, north] — only tiles inside are written (camera-aware extracts pass the
    /// region the camera path covers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// Views the local data couldn't serve at their zoom ([`recipe::unserved`]): recorded with the
    /// extract so whoever uses it can say what's missing instead of guessing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unserved: Vec<recipe::Unserved>,
    /// How current the data is (`OpenStreetMap 2026-09-20T10:11:12Z`): written to the archive's
    /// metadata too. Filled in after the inputs are fetched, so it isn't part of the plan.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub data_version: String,
    /// Tile ranges `[z, x0, y0, x1, y1]` (inclusive): when given, only these tiles are written —
    /// every one of them, empty or not, so a covered patch of open sea draws as sea rather than as
    /// an overzoomed ancestor's coarse coastline ([`camera::cover`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cover: camera::Cover,
    pub layers: Vec<LayerSpec>,
}

/// A job as JSON the way `assets/tiles/*.job.json` are written: one line per top-level field, per
/// layer and per cover range, so a rebuilt job diffs line by line.
pub fn job_json(job: &TileJob) -> String {
    let v = serde_json::to_value(job).unwrap_or_default();
    let Some(obj) = v.as_object() else { return String::new() };
    let fields: Vec<String> = obj.iter().map(|(k, v)| {
        let val = match v.as_array() {
            Some(items) if !items.is_empty() => format!("[\n    {}\n  ]", items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",\n    ")),
            _ => v.to_string(),
        };
        format!("  {}: {val}", serde_json::Value::String(k.clone()))
    }).collect();
    format!("{{\n{}\n}}\n", fields.join(",\n"))
}

#[derive(Debug, Default)]
pub struct TileStats {
    pub tiles: usize,
    pub features: usize,
    pub bytes: usize,
    /// Per zoom: tiles and their encoded (uncompressed) bytes — where an extract's size goes.
    pub by_zoom: BTreeMap<u8, (usize, usize)>,
}

/// Where a job's features come from: `(input, regions)` → the input's features. `regions` (lon/lat
/// boxes) are where the job will use them — a hint a loader may use to skip the rest (a planet
/// shapefile), or ignore; empty means everywhere.
pub type Loader<'a> = dyn Fn(&str, &[GeoBbox]) -> Result<FeatureCollection, String> + 'a;

/// A loader over GeoJSON bytes (`read` supplies them by input name).
pub fn geojson_loader<'a>(read: &'a dyn Fn(&str) -> Result<Vec<u8>, String>) -> impl Fn(&str, &[GeoBbox]) -> Result<FeatureCollection, String> + 'a {
    move |input: &str, _: &[GeoBbox]| parse_geojson(&read(input)?, &GeoJsonOptions::default()).map_err(|e| format!("{input}: {e}"))
}

/// The files of a data directory: `.shp` (polygons, read only where `regions` reach) or GeoJSON.
pub fn load_file(dir: &std::path::Path, input: &str, regions: &[GeoBbox]) -> Result<FeatureCollection, String> {
    let path = dir.join(input);
    if input.ends_with(".shp") {
        let f = std::fs::File::open(&path).map_err(|e| format!("{input}: {e}"))?;
        shp::read_polygons(std::io::BufReader::with_capacity(1 << 20, f), regions).map_err(|e| format!("{input}: {e}"))
    } else {
        let bytes = std::fs::read(&path).map_err(|e| format!("{input}: {e}"))?;
        parse_geojson(&bytes, &GeoJsonOptions::default()).map_err(|e| format!("{input}: {e}"))
    }
}

fn overlaps(a: &GeoBbox, b: &GeoBbox) -> bool {
    !(a.east < b.west || a.west > b.east || a.north < b.south || a.south > b.north)
}

fn geo(b: [f64; 4]) -> GeoBbox {
    GeoBbox::new(b[0], b[1], b[2], b[3])
}

/// Boxes merged until none overlap (a coarse union: fewer, larger boxes).
fn merge_boxes(mut boxes: Vec<GeoBbox>) -> Vec<GeoBbox> {
    let mut merged = true;
    while merged {
        merged = false;
        let mut out: Vec<GeoBbox> = Vec::with_capacity(boxes.len());
        for b in boxes {
            match out.iter_mut().find(|o| overlaps(o, &b)) {
                Some(o) => {
                    *o = GeoBbox::new(o.west.min(b.west), o.south.min(b.south), o.east.max(b.east), o.north.max(b.north));
                    merged = true;
                }
                None => out.push(b),
            }
        }
        boxes = out;
    }
    boxes
}

/// The lon/lat box of a cover range.
fn range_bounds(r: &[u32; 5]) -> GeoBbox {
    let (a, b) = (TileId::new(r[0] as u8, r[1], r[2]).lonlat_bounds(), TileId::new(r[0] as u8, r[3], r[4]).lonlat_bounds());
    GeoBbox::new(a.west, b.south, b.east, a.north)
}

/// Where a spec may put features (a loader hint, and with a cover a prefilter): the boxes of the
/// cover ranges at its zooms, cut to its `bbox` and the job's; without a cover just those two boxes
/// (`None`: anywhere).
fn spec_regions(job: &TileJob, spec: &LayerSpec) -> Option<Vec<GeoBbox>> {
    let limits: Vec<GeoBbox> = job.bbox.into_iter().chain(spec.bbox).map(geo).collect();
    let cut = |b: GeoBbox| limits.iter().try_fold(b, |b, l| overlaps(&b, l).then(|| GeoBbox::new(b.west.max(l.west), b.south.max(l.south), b.east.min(l.east), b.north.min(l.north))));
    if job.cover.is_empty() {
        return limits.first().map(|&b| cut(b).into_iter().collect());
    }
    let (z0, z1) = (spec.minzoom.max(job.minzoom) as u32, spec.maxzoom.min(job.maxzoom) as u32);
    Some(merge_boxes(job.cover.iter().filter(|r| (z0..=z1).contains(&r[0])).filter_map(|r| cut(range_bounds(r))).collect()))
}

/// Where a job uses each of its inputs: the regions a loader is asked for (merged boxes; empty: not
/// used at all; `None`: everywhere). What an acquisition step must fetch before [`tiles`] runs.
pub fn input_regions(job: &TileJob) -> BTreeMap<String, Option<Vec<GeoBbox>>> {
    let mut wanted: BTreeMap<String, Option<Vec<GeoBbox>>> = BTreeMap::new();
    for spec in &job.layers {
        let r = spec_regions(job, spec);
        let e = wanted.entry(spec.input.clone()).or_insert_with(|| Some(Vec::new()));
        match (e.as_mut(), r) {
            (Some(v), Some(r)) => v.extend(r.iter().copied()),
            _ => *e = None,
        }
    }
    for v in wanted.values_mut().flatten() {
        *v = merge_boxes(std::mem::take(v));
    }
    wanted
}

/// Run a tile job; `load` supplies each input's features (so the pipeline stays testable, and a
/// loader can read just the regions a camera-aware job uses).
pub fn tiles(job: &TileJob, load: &Loader) -> Result<(Vec<u8>, TileStats), String> {
    let mut stats = TileStats::default();
    let clip = job.bbox.map(geo);
    let cover: BTreeMap<u8, Vec<[u32; 5]>> = job.cover.iter().fold(BTreeMap::new(), |mut m, r| {
        m.entry(r[0] as u8).or_insert_with(Vec::new).push(*r);
        m
    });
    // Layers in order of first appearance; specs sharing a name share the layer.
    let mut names: Vec<String> = Vec::new();
    for l in &job.layers {
        if !names.contains(&l.name) {
            names.push(l.name.clone());
        }
    }
    // Where each input is used (a loader hint): the union of its specs' regions.
    let regions: Vec<Option<Vec<GeoBbox>>> = job.layers.iter().map(|s| spec_regions(job, s)).collect();
    let wanted = input_regions(job);
    // tile → layer index (in `names`) → features
    let mut acc: BTreeMap<TileId, BTreeMap<usize, Vec<datars_geo::mvt::Feature>>> = BTreeMap::new();
    let mut inputs: BTreeMap<String, FeatureCollection> = BTreeMap::new();
    for (spec, region) in job.layers.iter().zip(&regions) {
        let li = names.iter().position(|n| *n == spec.name).unwrap_or(0);
        if !inputs.contains_key(&spec.input) {
            let fc = match wanted.get(spec.input.as_str()).cloned().flatten() {
                // Used nowhere (no covered tile in reach): not read at all.
                Some(r) if r.is_empty() => FeatureCollection::default(),
                Some(r) => load(&spec.input, &r)?,
                None => load(&spec.input, &[])?,
            };
            inputs.insert(spec.input.clone(), fc);
        }
        let fc = &inputs[&spec.input];
        let own = spec.bbox.map(geo);
        let outside = spec.outside.map(geo);
        // A tile gets this spec if it's inside the job and the spec's box and not in its `outside`.
        let tile_ok = |bb: &GeoBbox| clip.as_ref().is_none_or(|c| overlaps(bb, c)) && own.as_ref().is_none_or(|r| overlaps(bb, r)) && !outside.as_ref().is_some_and(|o| overlaps(bb, o));
        let covered = |bb: &GeoBbox| job.cover.is_empty() || region.as_ref().is_none_or(|rs| rs.iter().any(|r| overlaps(bb, r)));
        let prepared: Vec<(datars_geo::Feature, GeoBbox)> = fc.iter().filter_map(|f| {
            let bb = datars_geo::measure::bbox(&f.geometry)?;
            let near = clip.as_ref().is_none_or(|c| overlaps(&bb, c)) && own.as_ref().is_none_or(|r| overlaps(&bb, r)) && covered(&bb);
            near.then_some(())?;
            Some((prepare(f, spec)?, bb))
        }).collect();
        stats.features += prepared.len();
        let z0 = spec.minzoom.max(job.minzoom);
        let z1 = spec.maxzoom.min(job.maxzoom);
        for z in z0..=z1 {
            let ranges = cover.get(&z);
            if !job.cover.is_empty() && ranges.is_none() {
                continue;
            }
            let tol = datars_geo::simplify::zoom_tolerance(z as f64, spec.simplify_px);
            for (f, bb) in &prepared {
                // The tiles this feature touches at z: all of them, or (without enumerating a big
                // feature's thousands of tiles) those of its tile range inside the cover.
                let tiles: Vec<TileId> = match ranges {
                    None => datars_geo::tiles_covering(bb, z),
                    Some(rs) => {
                        let nw = TileId::from_lonlat(datars_math::Vec2::new(bb.west, bb.north), z);
                        let se = TileId::from_lonlat(datars_math::Vec2::new(bb.east, bb.south), z);
                        rs.iter().flat_map(|r| {
                            let (ax, bx, ay, by) = (r[1].max(nw.x), r[3].min(se.x), r[2].max(nw.y), r[4].min(se.y));
                            (ax..=bx).flat_map(move |x| (ay..=by).map(move |y| TileId::new(z, x, y)))
                        }).collect()
                    }
                };
                let tiles: Vec<TileId> = tiles.into_iter().filter(|t| tile_ok(&t.lonlat_bounds())).collect();
                if tiles.is_empty() {
                    continue;
                }
                let Some(geometry) = drop_small(&f.geometry, z, spec.min_area_px) else { continue };
                let g = datars_geo::Feature { geometry: simplify_geometry(&geometry, tol, SimplifyMethod::DouglasPeucker), ..f.clone() };
                for tile in tiles {
                    if let Some(mf) = datars_geo::mvt::from_feature(&g, tile, 4096, 64) {
                        acc.entry(tile).or_default().entry(li).or_default().push(mf);
                    }
                }
            }
        }
    }
    // Covered tiles nothing fell in (open sea): written empty, so they draw as sea.
    for r in &job.cover {
        let z = r[0] as u8;
        if z < job.minzoom || z > job.maxzoom {
            continue;
        }
        for x in r[1]..=r[3] {
            for y in r[2]..=r[4] {
                let t = TileId::new(z, x, y);
                if clip.as_ref().is_none_or(|c| overlaps(&t.lonlat_bounds(), c)) {
                    acc.entry(t).or_default();
                }
            }
        }
    }
    let mut opts = WriterOptions::default();
    if let Some(c) = clip {
        opts.bounds = c;
        opts.center = c.center();
    }
    opts.metadata = serde_json::json!({
        "generator": "datars-geo-build",
        "attribution": job.attribution.clone().unwrap_or_else(|| "© OpenStreetMap contributors, Natural Earth".into()),
        "vector_layers": names.iter().map(|n| {
            let specs: Vec<&LayerSpec> = job.layers.iter().filter(|l| l.name == *n).collect();
            serde_json::json!({ "id": n, "minzoom": specs.iter().map(|l| l.minzoom).min(), "maxzoom": specs.iter().map(|l| l.maxzoom).max() })
        }).collect::<Vec<_>>(),
    });
    if !job.data_version.is_empty() {
        opts.metadata["data_version"] = serde_json::json!(job.data_version);
    }
    let mut w = Writer::new(opts);
    for (tile, layers) in acc {
        let mut layers: Vec<Layer> = layers.into_iter().map(|(li, features)| Layer { name: names[li].clone(), version: 2, extent: 4096, features }).collect();
        if layers.is_empty() {
            // A covered tile with nothing in it: one empty layer (a valid tile, not a missing one).
            layers.push(Layer { name: names.first().cloned().unwrap_or_else(|| "empty".into()), version: 2, extent: 4096, features: Vec::new() });
        }
        let vt = VectorTile { layers };
        let enc = vt.encode();
        let z = stats.by_zoom.entry(tile.z).or_default();
        z.0 += 1;
        z.1 += enc.len();
        w.add_tile(tile, enc);
        stats.tiles += 1;
    }
    let bytes = w.finish().map_err(|e| e.to_string())?;
    stats.bytes = bytes.len();
    Ok((bytes, stats))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARES: &str = r#"{"type":"FeatureCollection","features":[
      {"type":"Feature","properties":{"ISO":"AAA","NAME":"Alpha"},"geometry":{"type":"Polygon","coordinates":[[[10,50],[12,50],[12,52],[10,52],[10,50]]]}},
      {"type":"Feature","properties":{"ISO":"BBB","NAME":"Beta"},"geometry":{"type":"Polygon","coordinates":[[[12,50],[14,50],[14,52],[12,52],[12,50]]]}},
      {"type":"Feature","properties":{"ISO":"-99","NAME":"Nowhere"},"geometry":{"type":"Point","coordinates":[0,0]}}]}"#;

    fn squares(_: &str) -> Result<Vec<u8>, String> {
        Ok(SQUARES.as_bytes().to_vec())
    }

    fn decode(bytes: &[u8], t: TileId) -> Option<VectorTile> {
        datars_geo::pmtiles::read_tile(bytes, t).unwrap().map(|b| VectorTile::decode(&b).unwrap())
    }

    #[test]
    fn a_cover_writes_exactly_its_tiles_empty_ones_included() {
        let z4 = TileId::from_lonlat(datars_math::Vec2::new(11.0, 51.0), 4);
        let sea = TileId::from_lonlat(datars_math::Vec2::new(-30.0, 0.0), 4);
        let job = TileJob {
            layers: vec![LayerSpec { name: "land".into(), input: "sq".into(), minzoom: 0, maxzoom: 4, ..Default::default() }],
            maxzoom: 4,
            // z0 everywhere, one land tile and one open-sea tile at z4.
            cover: vec![[0, 0, 0, 0, 0], [4, z4.x, z4.y, z4.x, z4.y], [4, sea.x, sea.y, sea.x, sea.y]],
            ..Default::default()
        };
        let asked = std::cell::RefCell::new(Vec::new());
        let load = |input: &str, regions: &[GeoBbox]| {
            asked.borrow_mut().push((input.to_string(), regions.to_vec()));
            geojson_loader(&squares)(input, regions)
        };
        let (bytes, stats) = tiles(&job, &load).unwrap();
        assert_eq!(stats.tiles, 3, "{stats:?}");
        assert_eq!(decode(&bytes, z4).and_then(|t| t.layer("land").map(|l| l.features.len())), Some(2), "both squares");
        let empty = decode(&bytes, sea).expect("covered sea tile written");
        assert!(empty.layers.iter().all(|l| l.features.is_empty()));
        assert!(decode(&bytes, TileId::new(1, 1, 0)).is_none(), "z1 not covered");
        assert!(decode(&bytes, TileId::new(4, z4.x, z4.y + 1)).is_none(), "nor the z4 neighbours");
        // The loader heard where the job would use the input: the world (z0 covers it).
        let asked = asked.into_inner();
        assert_eq!(asked.len(), 1);
        assert!(asked[0].1.iter().any(|r| r.west <= -179.0 && r.east >= 179.0), "{asked:?}");
    }

    #[test]
    fn job_json_round_trips_one_line_per_item() {
        let layer = LayerSpec { name: "land".into(), input: "a".into(), maxzoom: 3, simplify_px: 1.0, ..Default::default() };
        let job = TileJob { about: "test".into(), maxzoom: 3, cover: vec![[0, 0, 0, 0, 0], [3, 1, 2, 3, 4]], layers: vec![layer], ..Default::default() };
        let s = job_json(&job);
        assert!(s.contains("\n    [3,1,2,3,4]") && s.contains("\n    {\"input\":\"a\""), "{s}");
        assert!(!s.contains("outside") && !s.contains("unserved"), "defaults left out: {s}");
        let back: TileJob = serde_json::from_str(&s).unwrap();
        assert_eq!(job_json(&back), s);
    }

    #[test]
    fn outside_is_the_complement_of_a_bbox() {
        // Detailed Alpha inside [9,49,12.5,53], the coarse source (Beta) only in tiles outside it.
        let job: TileJob = serde_json::from_value(serde_json::json!({
            "layers": [
                {"name": "roads", "input": "sq", "maxzoom": 6, "filter": [["ISO", "==", "AAA"]], "bbox": [9, 49, 11.5, 53]},
                {"name": "roads", "input": "sq", "maxzoom": 6, "filter": [["ISO", "==", "BBB"]], "outside": [9, 49, 11.5, 53]}
            ],
            "maxzoom": 6
        })).unwrap();
        let (bytes, _) = tiles(&job, &geojson_loader(&squares)).unwrap();
        let count = |lon: f64| decode(&bytes, TileId::from_lonlat(datars_math::Vec2::new(lon, 51.0), 6)).and_then(|t| t.layer("roads").map(|l| l.features.len()));
        // z6 tiles are 5.6° wide: 10.5 and 13.5 fall in different ones (x 33 and 34).
        assert_eq!(count(10.5), Some(1), "only the detailed source");
        assert_eq!(count(13.5), Some(1), "only the coarse source");
    }

    #[test]
    fn renames_keep_the_target_when_the_source_is_empty() {
        let fc = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"NAME":"Lisbon","NAME_SV":"Lissabon"},"geometry":{"type":"Point","coordinates":[-9.1,38.7]}},
          {"type":"Feature","properties":{"NAME":"Porto","NAME_SV":""},"geometry":{"type":"Point","coordinates":[-8.6,41.1]}}]}"#;
        let spec: LayerSpec = serde_json::from_value(serde_json::json!({"name": "places", "input": "p", "rename": {"NAME": "name", "NAME_SV": "name"}, "keep": ["name"]})).unwrap();
        let fc = parse_geojson(fc.as_bytes(), &GeoJsonOptions::default()).unwrap();
        let names: Vec<Option<String>> = fc.iter().map(|f| prepare(f, &spec).and_then(|g| g.property_str("name"))).collect();
        assert_eq!(names, vec![Some("Lissabon".to_string()), Some("Porto".to_string())]);
    }

    #[test]
    fn atlas_keys_names_and_drops_unkeyed() {
        let out = atlas(SQUARES.as_bytes(), "ISO", &["NAME"], 0.0, 4).unwrap();
        let fc = parse_geojson(out.as_bytes(), &GeoJsonOptions { id_property: Some("id".into()) }).unwrap();
        assert_eq!(fc.len(), 2);
        assert_eq!(fc.get("BBB").and_then(|f| f.property_str("name")), Some("Beta".to_string()));
    }

    #[test]
    fn tiles_round_trip_through_pmtiles() {
        let job = TileJob { layers: vec![LayerSpec { name: "land".into(), input: "sq".into(), minzoom: 0, maxzoom: 4, simplify_px: 1.0, ..Default::default() }], minzoom: 0, maxzoom: 4, ..Default::default() };
        let (bytes, stats) = tiles(&job, &geojson_loader(&squares)).unwrap();
        assert!(stats.tiles >= 5, "{stats:?}");
        let t = TileId::from_lonlat(datars_math::Vec2::new(11.0, 51.0), 4);
        let tile = datars_geo::pmtiles::read_tile(&bytes, t).unwrap().expect("tile present");
        let vt = VectorTile::decode(&tile).unwrap();
        assert!(vt.layer("land").is_some_and(|l| !l.features.is_empty()));
    }

    #[test]
    fn layers_filter_rename_classify_and_band_by_region() {
        let job: TileJob = serde_json::from_value(serde_json::json!({
            "layers": [
                // Alpha only (a filter), renamed and classified; only where the region reaches.
                {"name": "regions", "input": "sq", "minzoom": 0, "maxzoom": 4, "filter": [["ISO", "in", ["AAA"]]],
                 "rename": {"NAME": "name"}, "classify": {"from": "ISO", "to": "kind", "map": {"AAA": "first"}},
                 "set": {"source": "test"}, "keep": ["name", "kind", "source"], "bbox": [9, 49, 12.5, 53]},
                // The same layer from another spec at other zooms (a second source per band).
                {"name": "regions", "input": "sq", "minzoom": 5, "maxzoom": 5, "filter": [["ISO", "==", "BBB"]]},
                // Too small at z0 (2°×2° is a few px²), big enough at z4.
                {"name": "big", "input": "sq", "minzoom": 0, "maxzoom": 4, "min_area_px": 30, "filter": [["ISO", "!=", "-99"]]}
            ],
            "maxzoom": 5
        })).unwrap();
        let (bytes, _) = tiles(&job, &geojson_loader(&squares)).unwrap();
        let get = |lon: f64, z: u8| {
            let t = TileId::from_lonlat(datars_math::Vec2::new(lon, 51.0), z);
            datars_geo::pmtiles::read_tile(&bytes, t).unwrap().map(|b| VectorTile::decode(&b).unwrap())
        };
        let z4 = get(11.0, 4).expect("tile");
        let regions = z4.layer("regions").unwrap();
        assert_eq!(regions.features.len(), 1);
        let props: BTreeMap<String, String> = regions.features[0].properties.iter().map(|(k, v)| (k.clone(), format!("{v:?}"))).collect();
        assert_eq!(props.keys().cloned().collect::<Vec<_>>(), vec!["kind", "name", "source"]);
        assert!(props["kind"].contains("first") && props["name"].contains("Alpha"));
        assert_eq!(z4.layers.iter().filter(|l| l.name == "regions").count(), 1, "one layer per name");
        assert!(z4.layer("big").is_some());
        assert!(get(11.0, 0).expect("z0").layer("big").is_none(), "below min_area_px at z0");
        assert!(get(13.0, 5).expect("z5").layer("regions").is_some(), "the second spec's band");
    }
}
