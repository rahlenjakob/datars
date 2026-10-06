//! The standard basemap recipe: the layers std `basemap` draws (`land`, `water`, `rivers`,
//! `boundaries`, `roads` classed by `kind`, `buildings`, `places` with `name`/`rank`/`pop`), made
//! from the local open data per zoom band, for the tiles of a camera [`Cover`].
//!
//! The bands follow `assets/tiles/descent.job.json`, which was tuned by hand: Natural Earth 50m for
//! the world, 10m for countries, the osmdata coastline polygons from regional zooms on, and
//! OpenStreetMap extracts (roads, water, buildings) at street zoom where one exists. Where there is
//! no extract, Natural Earth fills in (its highways and lakes) and the view is reported
//! [`unserved`] — a street-level view without streets is a gap to name, not something to fake.

use crate::camera::{Cover, CoverOptions, Extent};
use crate::{Classify, LayerSpec, TileJob};
use std::collections::BTreeMap;
use std::path::Path;

/// From this zoom a view wants street-level data (OSM roads); below it Natural Earth carries it.
pub const STREET_ZOOM: u8 = 10;
/// From this zoom land comes from the osmdata coastline polygons (Natural Earth 10m is a 1:10M
/// coastline: blocky past about zoom 6).
pub const COASTLINE_ZOOM: u8 = 7;

pub const NE50_LAND: &str = "ne_50m_land.geojson";
pub const NE10_LAND: &str = "ne_10m_land.geojson";
pub const NE50_LAKES: &str = "ne_50m_lakes.geojson";
pub const NE10_LAKES: &str = "ne_10m_lakes.geojson";
pub const NE10_RIVERS: &str = "ne_10m_rivers_lake_centerlines.geojson";
pub const NE50_BORDERS: &str = "ne_50m_admin_0_boundary_lines_land.geojson";
pub const NE10_BORDERS: &str = "ne_10m_admin_0_boundary_lines_land.geojson";
pub const NE10_ROADS: &str = "ne_10m_roads.geojson";
pub const NE10_PLACES: &str = "ne_10m_populated_places.geojson";
pub const OSM_ROADS: &str = "osm_roads.geojson";
pub const OSM_WATER: &str = "osm_water.geojson";
pub const OSM_BUILDINGS: &str = "osm_buildings.geojson";
/// Where osmdata's land-polygons download unpacks in a data directory (lon/lat, split into a grid).
pub const LAND_POLYGONS: &str = "land-polygons/land-polygons-split-4326/land_polygons.shp";

/// What a data directory holds, as far as the recipe cares. Natural Earth is assumed; the rest is
/// optional and changes what the recipe can do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LocalData {
    /// The osmdata land polygons (`.shp`, lon/lat), relative to the data directory.
    pub land_polygons: Option<String>,
    /// Extents `[west, south, east, north]` of the OSM extracts ([`OSM_ROADS`], [`OSM_WATER`],
    /// [`OSM_BUILDINGS`], as `datars-geo-build osm` writes them).
    pub osm_roads: Option<[f64; 4]>,
    pub osm_water: Option<[f64; 4]>,
    pub osm_buildings: Option<[f64; 4]>,
}

impl LocalData {
    /// Look at a data directory: Natural Earth GeoJSON and whichever optional inputs it holds.
    pub fn scan(dir: &Path) -> Result<LocalData, String> {
        let extent = |name: &str| -> Result<Option<[f64; 4]>, String> {
            let p = dir.join(name);
            if !p.exists() {
                return Ok(None);
            }
            let fc = crate::load_file(dir, name, &[])?;
            let b = fc.iter().filter_map(|f| datars_geo::measure::bbox(&f.geometry)).reduce(|a, b| datars_geo::GeoBbox::new(a.west.min(b.west), a.south.min(b.south), a.east.max(b.east), a.north.max(b.north)));
            Ok(b.map(|b| [b.west, b.south, b.east, b.north]))
        };
        Ok(LocalData {
            land_polygons: dir.join(LAND_POLYGONS).exists().then(|| LAND_POLYGONS.to_string()),
            osm_roads: extent(OSM_ROADS)?,
            osm_water: extent(OSM_WATER)?,
            osm_buildings: extent(OSM_BUILDINGS)?,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct RecipeOptions {
    /// Label language: place names from Natural Earth's `NAME_<LANG>` (`sv`, `en` …), the local
    /// `NAME` where there's none.
    pub lang: Option<String>,
    /// The job's `about` (what it's for, how to rebuild it).
    pub about: String,
}

/// A view the local data can't serve at the zoom it's drawn at.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unserved {
    /// The view's label (a scene).
    pub label: String,
    pub bbox: [f64; 4],
    pub zoom: u8,
    /// What it lacks.
    pub missing: String,
}

fn area(b: [f64; 4]) -> f64 {
    (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0)
}

fn intersect(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])]
}

/// Settled views (those with a label) drawn at street zoom where the local OSM street extract
/// covers less than half the view: Natural Earth's highways and big lakes are all they'll show
/// on the coastline (and where OSM's coastline closes a harbour's mouth — Sydney's — the harbour
/// is inland water, so it draws as land too).
pub fn unserved(views: &[Extent], data: &LocalData) -> Vec<Unserved> {
    views.iter().filter(|v| !v.label.is_empty()).filter_map(|v| {
        let z = v.zoom.round().max(0.0) as u8;
        if z < STREET_ZOOM {
            return None;
        }
        let share = data.osm_roads.map_or(0.0, |o| area(intersect(v.bbox, o)) / area(v.bbox).max(1e-12));
        (share < 0.5).then(|| Unserved { label: v.label.clone(), bbox: v.bbox, zoom: z, missing: "OpenStreetMap streets or inland water".into() })
    }).collect()
}

/// Samples per flight between two views: enough that consecutive samples overlap at every zoom.
pub const FLIGHT_SAMPLES: usize = 48;

/// A camera-aware extract's job in one call. `chains` are sequences of settled views a camera flies
/// through in order (a story's consecutive map scenes), each view drawn in a `w`×`h` px map: the
/// views and the flights between neighbours give the cover, the recipe fills it, and the street-
/// level views the data can't serve are recorded in `unserved`.
pub fn camera_job(chains: &[Vec<Extent>], w: f64, h: f64, data: &LocalData, o: &RecipeOptions, c: &CoverOptions) -> TileJob {
    let mut extents: Vec<Extent> = Vec::new();
    for chain in chains {
        for (i, v) in chain.iter().enumerate() {
            if i > 0 {
                extents.extend(crate::camera::flight(&chain[i - 1], v, w, h, FLIGHT_SAMPLES));
            }
            extents.push(v.clone());
        }
    }
    let mut job = recipe_job(crate::camera::cover(&extents, c), data, o);
    job.unserved = unserved(&extents, data);
    job
}

/// A view as JSON: a box a camera fits (`bbox`), or a centre and tile zoom (`center`, `zoom`).
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct ViewSpec {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub bbox: Option<[f64; 4]>,
    #[serde(default)]
    pub center: Option<[f64; 2]>,
    #[serde(default)]
    pub zoom: Option<f64>,
}

/// `datars-geo-build camera` input: the map's size in px, label language, and chains of views.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CameraSpec {
    pub size: [f64; 2],
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub about: String,
    pub chains: Vec<Vec<ViewSpec>>,
}

impl CameraSpec {
    /// The chains as extents (a view with neither a box nor a centre and zoom is an error).
    pub fn extents(&self) -> Result<Vec<Vec<Extent>>, String> {
        let [w, h] = self.size;
        self.chains.iter().map(|chain| chain.iter().map(|v| match (v.bbox, v.center, v.zoom) {
            (Some(b), _, _) => Ok(Extent::fit(b, w, h).named(v.label.clone())),
            (None, Some([lon, lat]), Some(z)) => Ok(Extent::centered(lon, lat, z, w, h).named(v.label.clone())),
            _ => Err(format!("view `{}`: needs a bbox, or a center and a zoom", v.label)),
        }).collect()).collect()
    }
}

fn classes(pairs: &[(&str, &str)]) -> Classify {
    Classify { from: "type".into(), to: "kind".into(), map: pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(), default: None }
}

const NE_HIGHWAYS: &[(&str, &str)] = &[("Major Highway", "highway"), ("Beltway", "highway")];
const NE_ROADS: &[(&str, &str)] = &[("Major Highway", "highway"), ("Beltway", "highway"), ("Secondary Highway", "major"), ("Bypass", "major"), ("Road", "minor")];
const OSM_Z10: &[(&str, &str)] = &[("motorway", "highway"), ("trunk", "highway"), ("motorway_link", "highway"), ("trunk_link", "highway"), ("primary", "major"), ("secondary", "major")];
const OSM_Z11: &[(&str, &str)] = &[
    ("motorway", "highway"), ("trunk", "highway"), ("motorway_link", "highway"), ("trunk_link", "highway"),
    ("primary", "major"), ("primary_link", "major"), ("secondary", "major"), ("secondary_link", "major"), ("tertiary", "minor"),
];
const OSM_Z12: &[(&str, &str)] = &[
    ("motorway", "highway"), ("trunk", "highway"), ("motorway_link", "highway"), ("trunk_link", "highway"),
    ("primary", "major"), ("primary_link", "major"), ("secondary", "major"), ("secondary_link", "major"),
    ("tertiary", "minor"), ("tertiary_link", "minor"), ("residential", "minor"), ("unclassified", "minor"), ("living_street", "minor"), ("pedestrian", "minor"),
];

/// Zoom bands (inclusive): where sources or thresholds change. The last runs to the job's max.
const BANDS: &[(u8, u8)] = &[(0, 1), (2, 2), (3, 4), (5, 5), (6, 6), (7, 9), (10, 10), (11, 11), (12, u8::MAX)];

/// The tile job for a cover: the standard layers per zoom band, only for bands the cover reaches.
pub fn recipe_job(cover: Cover, data: &LocalData, o: &RecipeOptions) -> TileJob {
    let maxzoom = cover.iter().map(|r| r[0] as u8).max().unwrap_or(0);
    let zooms: std::collections::BTreeSet<u8> = cover.iter().map(|r| r[0] as u8).collect();
    let mut layers = Vec::new();
    for &(z0, z1) in BANDS {
        let z1 = z1.min(maxzoom);
        if z0 > z1 || zooms.range(z0..=z1).next().is_none() {
            continue;
        }
        band(&mut layers, z0, z1, data, o);
    }
    TileJob { about: o.about.clone(), minzoom: 0, maxzoom, bbox: None, attribution: Some("© OpenStreetMap contributors, Natural Earth".into()), unserved: Vec::new(), data_version: String::new(), cover, layers }
}

/// The osmdata simplified land polygons (Web Mercator), as an automatic job's input.
pub const OSMDATA_LAND: &str = "osmdata:land";

/// An automatic job's OpenStreetMap input: `osm:<layer>:<level zoom>` (see [`crate::auto`]).
pub fn osm_input(layer: &str, level: crate::overpass::Level) -> String {
    format!("osm:{layer}:{}", level.zoom())
}

/// Zoom bands of an automatic job: those of [`recipe_job`], with street zoom split per Overpass
/// level (each asks for what it draws).
const AUTO_BANDS: &[(u8, u8)] = &[(0, 1), (2, 2), (3, 4), (5, 5), (6, 6), (7, 9), (10, 10), (11, 11), (12, 12), (13, u8::MAX)];

/// The standard layers for a cover from data fetched on demand ([`crate::auto`]): Natural Earth to
/// zoom 9 (with osmdata's simplified coastline polygons from 7), OpenStreetMap from 10 — land from
/// its coastline, water, rivers, parks, roads by class, buildings from 13, places in `o.lang`.
/// Every street-level view is served: the data is fetched for wherever the cover reaches.
pub fn auto_job(cover: Cover, o: &RecipeOptions) -> TileJob {
    use crate::overpass::Level;
    let maxzoom = cover.iter().map(|r| r[0] as u8).max().unwrap_or(0);
    let zooms: std::collections::BTreeSet<u8> = cover.iter().map(|r| r[0] as u8).collect();
    let mut layers = Vec::new();
    for &(z0, z1) in AUTO_BANDS {
        let z1 = z1.min(maxzoom);
        if z0 > z1 || zooms.range(z0..=z1).next().is_none() {
            continue;
        }
        match Level::for_zoom(z0) {
            None => {
                // Natural Earth below street zoom, as in the hand-tuned bands; osmdata's simplified
                // coastline from zoom 7.
                let data = LocalData { land_polygons: None, ..Default::default() };
                let start = layers.len();
                band(&mut layers, z0, z1, &data, o);
                if z0 >= COASTLINE_ZOOM {
                    for l in &mut layers[start..] {
                        if l.name == "land" {
                            l.input = OSMDATA_LAND.into();
                        }
                    }
                }
            }
            Some(level) => street_band(&mut layers, z0, z1, level),
        }
    }
    TileJob { about: o.about.clone(), minzoom: 0, maxzoom, bbox: None, attribution: Some("© OpenStreetMap contributors, Natural Earth".into()), unserved: Vec::new(), data_version: String::new(), cover, layers }
}

/// One street-zoom band of an automatic job: everything from OpenStreetMap, borders from Natural
/// Earth (OSM's admin boundaries are relations too big to fetch per cell).
fn street_band(out: &mut Vec<LayerSpec>, z0: u8, z1: u8, level: crate::overpass::Level) {
    use crate::overpass::Level;
    let px = if z0 <= 11 { 0.7 } else { 0.6 };
    // The building level's cells hold buildings and place names only; the rest comes from the
    // street level's (all of z12's streets are what z13 and z14 draw too).
    let (buildings, level) = (level, if level == Level::Z13 { Level::Z12 } else { level });
    let mut land = kind(spec("land", &osm_input("land", level), z0, z1, px), "land");
    land.min_area_px = 2.0;
    out.push(land);
    let mut parks = kind(spec("parks", &osm_input("parks", level), z0, z1, px), "park");
    // At z10 only woods and forests are fetched, and a city's worth of them: the big ones kept.
    parks.min_area_px = if level == Level::Z10 { 12.0 } else { 6.0 };
    out.push(parks);
    let mut water = kind(spec("water", &osm_input("water", level), z0, z1, px), "lake");
    water.min_area_px = 3.0;
    out.push(water);
    out.push(kind(spec("rivers", &osm_input("waterways", level), z0, z1, px.max(0.8)), "river"));
    out.push(kind(spec("boundaries", NE10_BORDERS, z0, z1, 0.8), "country"));
    let mut roads = spec("roads", &osm_input("roads", level), z0, z1, px);
    roads.keep = vec!["kind".into()];
    roads.classify = Some(classes(match z0 {
        10 => OSM_Z10,
        11 => OSM_Z11,
        _ => OSM_Z12,
    }));
    out.push(roads);
    if buildings >= Level::Z13 {
        let mut b = kind(spec("buildings", &osm_input("buildings", buildings), z0, z1, 0.6), "building");
        b.min_area_px = 1.5;
        out.push(b);
    }
    let mut p = spec("places", &osm_input("places", buildings), z0, z1, 1.0);
    p.keep = ["name", "rank", "pop", "kind"].map(String::from).to_vec();
    out.push(p);
}

fn spec(name: &str, input: &str, z0: u8, z1: u8, px: f64) -> LayerSpec {
    LayerSpec { name: name.into(), input: input.into(), minzoom: z0, maxzoom: z1, simplify_px: px, ..Default::default() }
}

fn kind(mut s: LayerSpec, k: &str) -> LayerSpec {
    s.keep = vec!["kind".into()];
    s.set = BTreeMap::from([("kind".to_string(), serde_json::json!(k))]);
    s
}

fn band(out: &mut Vec<LayerSpec>, z0: u8, z1: u8, data: &LocalData, o: &RecipeOptions) {
    let px = match z0 {
        0..=1 => 1.5,
        2 => 1.2,
        3..=5 => 1.0,
        6..=9 => 0.8,
        10..=11 => 0.7,
        _ => 0.6,
    };
    let street = z0 >= STREET_ZOOM;

    // Land: every covered tile gets it (a tile without its land draws as sea).
    let land = match (&data.land_polygons, z0) {
        (Some(shp), z) if z >= COASTLINE_ZOOM => shp.as_str(),
        (_, z) if z >= 3 => NE10_LAND,
        _ => NE50_LAND,
    };
    let mut l = kind(spec("land", land, z0, z1, px), "land");
    l.min_area_px = if (6..12).contains(&z0) { 3.0 } else { 2.0 };
    out.push(l);

    // Water: lakes from Natural Earth; at street zoom the OSM water where it exists, Natural
    // Earth's (coarse) lakes only outside it — never both over one tile.
    if z0 >= 2 {
        let lakes = if z0 >= 3 { NE10_LAKES } else { NE50_LAKES };
        let mut w = kind(spec("water", lakes, z0, z1, px), "lake");
        w.min_area_px = if z0 <= 5 { 4.0 } else { 3.0 };
        match (street, data.osm_water) {
            (true, Some(ext)) => {
                w.outside = Some(ext);
                let mut osm = kind(spec("water", OSM_WATER, z0, z1, px), "lake");
                osm.min_area_px = 3.0;
                osm.bbox = Some(ext);
                out.push(w);
                out.push(osm);
            }
            _ => out.push(w),
        }
    }

    // Rivers (Natural Earth centrelines), not where OSM water draws the real banks.
    if z0 >= 5 {
        let mut r = kind(spec("rivers", NE10_RIVERS, z0, z1, px.max(0.8)), "river");
        if street {
            r.outside = data.osm_water;
        }
        out.push(r);
    }

    // Country borders.
    let borders = if z0 >= 3 { NE10_BORDERS } else { NE50_BORDERS };
    out.push(kind(spec("boundaries", borders, z0, z1, if z0 >= 3 { 0.8 } else { 1.2 }), "country"));

    // Roads by class: Natural Earth highways, then all its classes; OSM at street zoom where the
    // extract is (Natural Earth outside it).
    if z0 >= 5 {
        let mut ne = spec("roads", NE10_ROADS, z0, z1, px.max(0.8));
        ne.keep = vec!["kind".into()];
        ne.classify = Some(classes(if z0 >= 7 { NE_ROADS } else { NE_HIGHWAYS }));
        match (street, data.osm_roads) {
            (true, Some(ext)) => {
                ne.outside = Some(ext);
                let mut osm = spec("roads", OSM_ROADS, z0, z1, px);
                osm.keep = vec!["kind".into()];
                osm.classify = Some(classes(match z0 {
                    10 => OSM_Z10,
                    11 => OSM_Z11,
                    _ => OSM_Z12,
                }));
                osm.bbox = Some(ext);
                out.push(ne);
                out.push(osm);
            }
            _ => out.push(ne),
        }
    }

    // Buildings where the extract has them.
    if z0 >= 12 {
        if let Some(ext) = data.osm_buildings {
            let mut b = kind(spec("buildings", OSM_BUILDINGS, z0, z1, 0.6), "building");
            b.min_area_px = 1.5;
            b.bbox = Some(ext);
            out.push(b);
        }
    }

    // Place names, the most important first by zoom.
    let mut p = spec("places", NE10_PLACES, z0, z1, 1.0);
    p.keep = ["name", "rank", "pop", "kind"].map(String::from).to_vec();
    p.rename = BTreeMap::from([("NAME".to_string(), "name".to_string()), ("SCALERANK".to_string(), "rank".to_string()), ("POP_MAX".to_string(), "pop".to_string())]);
    if let Some(lang) = &o.lang {
        // Keys sort after `NAME`, so the translation wins where there is one.
        p.rename.insert(format!("NAME_{}", lang.to_uppercase()), "name".into());
    }
    p.set = BTreeMap::from([("kind".to_string(), serde_json::json!(if z0 <= 5 { "city" } else { "town" }))]);
    let max_rank = match z0 {
        0..=1 => Some(1),
        2 => Some(2),
        3..=4 => Some(5),
        5 => Some(6),
        _ => None,
    };
    if let Some(r) = max_rank {
        p.filter = vec![("rank".into(), "<=".into(), serde_json::json!(r))];
    }
    out.push(p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::cover;

    fn stockholm_data() -> LocalData {
        LocalData {
            land_polygons: Some(LAND_POLYGONS.into()),
            osm_roads: Some([17.99, 59.28, 18.18, 59.37]),
            osm_water: Some([16.0, 59.19, 18.19, 59.8]),
            osm_buildings: Some([18.02, 59.3, 18.12, 59.35]),
        }
    }

    #[test]
    fn bands_follow_the_cover_and_pick_sources_by_zoom() {
        let views = [Extent::centered(18.07, 59.33, 11.0, 1000.0, 600.0).named("city")];
        let job = recipe_job(cover(&views, &CoverOptions::default()), &stockholm_data(), &RecipeOptions { lang: Some("sv".into()), ..Default::default() });
        assert_eq!((job.minzoom, job.maxzoom), (0, 11));
        let at = |name: &str, z: u8| -> Vec<&LayerSpec> { job.layers.iter().filter(|l| l.name == name && (l.minzoom..=l.maxzoom).contains(&z)).collect() };
        // World base from Natural Earth 50m; no bands the cover doesn't reach (z2–z10).
        assert_eq!(at("land", 0)[0].input, NE50_LAND);
        assert!(job.layers.iter().all(|l| l.maxzoom <= 1 || l.minzoom >= 11), "{:?}", job.layers.iter().map(|l| (l.minzoom, l.maxzoom)).collect::<Vec<_>>());
        // Street zoom: coastline land everywhere; OSM roads inside the extract, Natural Earth's
        // outside it — complementary boxes, so no tile gets both.
        assert_eq!(at("land", 11)[0].input, LAND_POLYGONS);
        assert!(at("land", 11)[0].bbox.is_none() && at("land", 11)[0].outside.is_none());
        let roads = at("roads", 11);
        let osm = roads.iter().find(|l| l.input == OSM_ROADS).expect("osm roads");
        let ne = roads.iter().find(|l| l.input == NE10_ROADS).expect("ne roads");
        assert_eq!(osm.bbox, ne.outside);
        assert!(osm.classify.as_ref().is_some_and(|c| c.map.get("tertiary").map(String::as_str) == Some("minor")));
        // Swedish names where Natural Earth has them.
        assert_eq!(at("places", 11)[0].rename.get("NAME_SV").map(String::as_str), Some("name"));
        assert!(at("buildings", 11).is_empty(), "buildings from z12");
    }

    #[test]
    fn camera_jobs_cover_the_flight_between_views() {
        let (w, h) = (1000.0, 600.0);
        let chain = vec![Extent::centered(18.07, 59.33, 11.5, w, h).named("stockholm"), Extent::centered(-9.15, 38.7, 11.5, w, h).named("lisbon")];
        let job = camera_job(&[chain], w, h, &stockholm_data(), &RecipeOptions::default(), &CoverOptions::default());
        let zooms: std::collections::BTreeSet<u32> = job.cover.iter().map(|r| r[0]).collect();
        // The flight zooms out to about z3 and back in: every zoom on the way is covered.
        assert!((4..=12).all(|z| zooms.contains(&z)), "{zooms:?}");
        // Mid-flight, over France, at a middle zoom; but not Africa.
        assert!(crate::camera::covers(&job.cover, datars_geo::TileId::from_lonlat(datars_math::Vec2::new(4.0, 49.0), 4)) || crate::camera::covers(&job.cover, datars_geo::TileId::from_lonlat(datars_math::Vec2::new(4.0, 49.0), 5)));
        assert!(!crate::camera::covers(&job.cover, datars_geo::TileId::from_lonlat(datars_math::Vec2::new(20.0, 5.0), 6)));
        // Small: a few hundred tiles, not a continent at street zoom.
        assert!(crate::camera::tile_count(&job.cover) < 1500, "{}", crate::camera::tile_count(&job.cover));
        assert_eq!(job.unserved.iter().map(|u| u.label.as_str()).collect::<Vec<_>>(), vec!["lisbon"]);
    }

    #[test]
    fn camera_specs_read_boxes_and_centres() {
        let spec: CameraSpec = serde_json::from_value(serde_json::json!({
            "size": [1000, 600], "lang": "sv",
            "chains": [[{"label": "sweden", "bbox": [10, 55, 25, 69]}, {"label": "city", "center": [18.07, 59.33], "zoom": 11}]]
        })).unwrap();
        let chains = spec.extents().unwrap();
        assert_eq!(chains[0].len(), 2);
        assert!(chains[0][0].zoom < 5.0 && (chains[0][1].zoom - 11.0).abs() < 1e-9);
        let bad: CameraSpec = serde_json::from_value(serde_json::json!({"size": [1, 1], "chains": [[{"label": "x", "zoom": 3}]]})).unwrap();
        assert!(bad.extents().is_err());
    }

    #[test]
    fn street_views_outside_the_extract_are_unserved() {
        let data = stockholm_data();
        let views = [
            Extent::centered(18.07, 59.33, 11.5, 1000.0, 600.0).named("stockholm"),
            Extent::centered(-9.15, 38.7, 11.0, 1000.0, 600.0).named("lisbon"),
            Extent::centered(-9.15, 38.7, 6.0, 1000.0, 600.0).named("portugal"),
            Extent::centered(-9.15, 38.7, 12.0, 1000.0, 600.0),
        ];
        let u = unserved(&views, &data);
        // Stockholm has streets; Portugal at z6 needs none; the unlabelled flight sample isn't a view.
        assert_eq!(u.iter().map(|u| u.label.as_str()).collect::<Vec<_>>(), vec!["lisbon"]);
        assert_eq!(u[0].zoom, 11);
        // Without any extract, every street-level view is unserved.
        assert_eq!(unserved(&views, &LocalData::default()).len(), 2);
    }
}
