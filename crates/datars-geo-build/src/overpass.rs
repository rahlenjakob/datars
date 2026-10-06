//! OpenStreetMap at street zoom, from the Overpass API: what to ask for per zoom band, in which
//! cells, and how an answer becomes the layers std `basemap` draws.
//!
//! **Cells.** Data is asked for per Web-Mercator tile at a fixed zoom per [`Level`] — not per view —
//! so a camera nudged by a few pixels, a second story in the same city, or a reader panning an
//! explorable map in `datars dev` reuses what's cached and fetches only new cells. A cell's query
//! text (and so its cache key) depends on nothing but the level and the tile.
//!
//! **Levels.** Each street zoom band asks for what it draws: highways and big roads at z10, down to
//! residential streets at z12, buildings from z13. Higher levels use smaller cells, so the dense
//! data is fetched only near the views that show it.
//!
//! Answers use `out geom;` — every way carries its coordinates, so nothing is resolved by node id.
//! Multipolygon relations (lakes with islands, forests) are assembled even-odd from their member
//! ways, like `osm::convert`.

use crate::osm::assemble_rings;
use datars_geo::{Feature, GeoBbox, Geometry, TileId};
use datars_math::Vec2;
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// How much of OpenStreetMap a zoom band needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// z10: motorways to secondary roads, water, rivers and canals, cities and towns.
    Z10,
    /// z11: + tertiary roads, parks and woods, suburbs and villages.
    Z11,
    /// z12: + residential streets, quarters.
    Z12,
    /// z13 and up: + buildings, neighbourhoods.
    Z13,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Z10, Level::Z11, Level::Z12, Level::Z13];

    /// The level a tile zoom draws from (`None` below street zoom: Natural Earth's business).
    pub fn for_zoom(z: u8) -> Option<Level> {
        match z {
            0..=9 => None,
            10 => Some(Level::Z10),
            11 => Some(Level::Z11),
            12 => Some(Level::Z12),
            _ => Some(Level::Z13),
        }
    }

    pub fn zoom(self) -> u8 {
        match self {
            Level::Z10 => 10,
            Level::Z11 => 11,
            Level::Z12 => 12,
            Level::Z13 => 13,
        }
    }

    /// The zoom of the tiles its cells are: one below its own for the sparse levels, so a view (a
    /// few tiles across) needs a handful of cells; the dense ones (every street, every building)
    /// come in cells of their own zoom, so a city's worth isn't fetched for a neighbourhood.
    pub fn cell_zoom(self) -> u8 {
        match self {
            Level::Z10 => 9,
            Level::Z11 => 10,
            Level::Z12 => 12,
            Level::Z13 => 13,
        }
    }

    fn highways(self) -> &'static str {
        match self {
            Level::Z10 => "motorway|trunk|primary|secondary",
            Level::Z11 => "motorway|trunk|primary|secondary|tertiary",
            Level::Z12 | Level::Z13 => "motorway|trunk|primary|secondary|tertiary|residential|unclassified|living_street|pedestrian",
        }
    }

    fn places(self) -> &'static str {
        match self {
            Level::Z10 => "city|town",
            Level::Z11 => "city|town|suburb|village",
            Level::Z12 => "city|town|suburb|village|quarter",
            Level::Z13 => "city|town|suburb|village|quarter|neighbourhood",
        }
    }
}

/// The cells (tiles at the level's cell zoom) touching any of `regions`, sorted.
pub fn cells(level: Level, regions: &[GeoBbox]) -> Vec<TileId> {
    let mut set = std::collections::BTreeSet::new();
    for r in regions {
        set.extend(datars_geo::tiles_covering(r, level.cell_zoom()));
    }
    set.into_iter().collect()
}

/// The query for one cell at one level. Its text is its cache key, so it depends on nothing else;
/// the bbox is written at fixed precision (tile edges, not user input).
pub fn query(level: Level, cell: TileId) -> String {
    let b = cell.lonlat_bounds();
    let bbox = format!("{:.7},{:.7},{:.7},{:.7}", b.south, b.west, b.north, b.east);
    let mut q = format!("[out:json][timeout:{}][bbox:{bbox}];\n(\n", crate::fetch::OVERPASS_TIMEOUT_S);
    if level == Level::Z13 {
        // The building level adds only what z12 cells don't have: the z13 bands read streets,
        // water, parks and the coast from those (see `recipe::auto_job`).
        q.push_str("  way[\"building\"];\n");
        q.push_str(&format!("  node[\"place\"~\"^({})$\"][\"name\"];\n", level.places()));
        q.push_str(");\nout geom;\n");
        return q;
    }
    q.push_str("  way[\"natural\"=\"coastline\"];\n");
    q.push_str("  way[\"natural\"=\"water\"]; relation[\"natural\"=\"water\"];\n");
    q.push_str("  way[\"waterway\"=\"riverbank\"]; relation[\"waterway\"=\"riverbank\"];\n");
    q.push_str("  way[\"landuse\"~\"^(reservoir|basin)$\"];\n");
    q.push_str("  way[\"waterway\"~\"^(river|canal)$\"];\n");
    if level >= Level::Z11 {
        q.push_str("  way[\"leisure\"~\"^(park|garden)$\"]; relation[\"leisure\"=\"park\"];\n");
        q.push_str("  way[\"landuse\"~\"^(forest|grass|recreation_ground|village_green)$\"]; relation[\"landuse\"=\"forest\"];\n");
        q.push_str("  way[\"natural\"=\"wood\"]; relation[\"natural\"=\"wood\"];\n");
    } else {
        // Woods and forests shape a city seen whole (Rio's Tijuca, Stockholm's islands); parks and
        // gardens wait for z11.
        q.push_str("  way[\"landuse\"=\"forest\"]; relation[\"landuse\"=\"forest\"];\n");
        q.push_str("  way[\"natural\"=\"wood\"]; relation[\"natural\"=\"wood\"];\n");
    }
    q.push_str(&format!("  way[\"highway\"~\"^({})(_link)?$\"];\n", level.highways()));
    q.push_str(&format!("  node[\"place\"~\"^({})$\"][\"name\"];\n", level.places()));
    q.push_str(");\nout geom;\n");
    q
}

/// What a cell holds, by layer. Features are keyed by OSM element (`w123`, `r45`, `n6`) so cells
/// that share a long road or a lake merge without doubles.
#[derive(Clone, Debug, Default)]
pub struct Layers {
    /// Lines with `type` (the `highway` value).
    pub roads: BTreeMap<String, Feature>,
    /// Water polygons (`kind` = lake).
    pub water: BTreeMap<String, Feature>,
    /// River and canal centrelines (`kind` = river).
    pub waterways: BTreeMap<String, Feature>,
    /// Parks, gardens, woods and grass (`kind` = park).
    pub parks: BTreeMap<String, Feature>,
    pub buildings: BTreeMap<String, Feature>,
    /// Points with `name`, `kind` (the `place` value), `rank` and, where OSM has it, `pop`.
    pub places: BTreeMap<String, Feature>,
    /// Coastline ways as tagged: land on the left ([`crate::coast`] turns them into land).
    pub coastline: BTreeMap<String, Vec<Vec2>>,
    /// When the data was current (`osm3s.timestamp_osm_base`).
    pub timestamp: Option<String>,
}

impl Layers {
    /// Everything of `other` this doesn't have yet (the first cell's copy of a shared way wins).
    pub fn merge(&mut self, other: &Layers) {
        fn add<T: Clone>(a: &mut BTreeMap<String, T>, b: &BTreeMap<String, T>) {
            for (k, v) in b {
                a.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        add(&mut self.roads, &other.roads);
        add(&mut self.water, &other.water);
        add(&mut self.waterways, &other.waterways);
        add(&mut self.parks, &other.parks);
        add(&mut self.buildings, &other.buildings);
        add(&mut self.places, &other.places);
        add(&mut self.coastline, &other.coastline);
        self.timestamp = match (self.timestamp.take(), other.timestamp.clone()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
    }
}

fn coords(g: Option<&Json>) -> Vec<Vec2> {
    g.and_then(Json::as_array).map(|a| a.iter().filter_map(|p| Some(Vec2::new(p.get("lon")?.as_f64()?, p.get("lat")?.as_f64()?))).collect()).unwrap_or_default()
}

fn closed_ring(mut r: Vec<Vec2>) -> Option<Vec<Vec2>> {
    if r.len() < 3 {
        return None;
    }
    if r.first() != r.last() {
        // An area way must close; an unclosed one is broken data, not a shape to guess at.
        return None;
    }
    if r.len() < 4 {
        r.push(r[0]);
    }
    Some(r)
}

/// Rings (any order, outer and inner mixed) → polygons, even-odd: islands become holes.
fn even_odd(rings: Vec<Vec<Vec2>>) -> Option<Geometry> {
    use geo::BooleanOps;
    let mut acc = geo::MultiPolygon::<f64>(Vec::new());
    for r in rings.iter().filter(|r| r.len() >= 4) {
        let ls = geo::LineString::from(r.iter().map(|p| geo::Coord { x: p.x, y: p.y }).collect::<Vec<_>>());
        acc = acc.xor(&geo::MultiPolygon(vec![geo::Polygon::new(ls, vec![])]));
    }
    let ring = |ls: &geo::LineString<f64>| ls.coords().map(|c| Vec2::new(c.x, c.y)).collect::<Vec<_>>();
    let polys: Vec<datars_geo::Polygon> = acc.0.iter().map(|p| std::iter::once(ring(p.exterior())).chain(p.interiors().iter().map(ring)).collect()).collect();
    match polys.len() {
        0 => None,
        1 => polys.into_iter().next().map(Geometry::Polygon),
        _ => Some(Geometry::MultiPolygon(polys)),
    }
}

fn is_water(tag: &dyn Fn(&str) -> Option<String>) -> bool {
    tag("natural").as_deref() == Some("water") || tag("waterway").as_deref() == Some("riverbank") || matches!(tag("landuse").as_deref(), Some("reservoir" | "basin"))
}

fn is_park(tag: &dyn Fn(&str) -> Option<String>) -> bool {
    matches!(tag("leisure").as_deref(), Some("park" | "garden")) || matches!(tag("landuse").as_deref(), Some("forest" | "grass" | "recreation_ground" | "village_green")) || tag("natural").as_deref() == Some("wood")
}

/// How prominent a place is, by kind (smaller first; std `basemap` sizes ranks ≤ 2 up).
fn place_rank(kind: &str, pop: Option<f64>) -> i64 {
    match kind {
        "city" if pop.is_some_and(|p| p >= 1_000_000.0) => 1,
        "city" => 2,
        "town" => 5,
        "suburb" => 7,
        "village" | "quarter" => 8,
        _ => 9,
    }
}

/// An Overpass answer as layers. `lang` picks `name:<lang>` over `name` for place labels.
pub fn parse(body: &[u8], lang: Option<&str>) -> Result<Layers, String> {
    let doc: Json = serde_json::from_slice(body).map_err(|e| format!("overpass answer: {e}"))?;
    let mut out = Layers { timestamp: doc.pointer("/osm3s/timestamp_osm_base").and_then(Json::as_str).map(String::from), ..Default::default() };
    let Some(elements) = doc.get("elements").and_then(Json::as_array) else { return Err("overpass answer: no elements".into()) };
    let kind_of = |k: &str| serde_json::json!(k);
    for el in elements {
        let (ty, id) = (el.get("type").and_then(Json::as_str).unwrap_or(""), el.get("id").and_then(Json::as_u64).unwrap_or(0));
        let tags = el.get("tags");
        let tag = |k: &str| tags.and_then(|t| t.get(k)).and_then(Json::as_str).map(String::from);
        match ty {
            "node" => {
                let (Some(lon), Some(lat)) = (el.get("lon").and_then(Json::as_f64), el.get("lat").and_then(Json::as_f64)) else { continue };
                let (Some(kind), Some(local)) = (tag("place"), tag("name")) else { continue };
                let name = lang.and_then(|l| tag(&format!("name:{l}"))).unwrap_or(local);
                let pop = tag("population").and_then(|p| p.replace([',', ' ', '.'], "").parse::<f64>().ok()).filter(|p| *p > 0.0);
                let mut f = Feature::new(Geometry::Point(Vec2::new(lon, lat))).with_property("name", serde_json::json!(name)).with_property("kind", kind_of(&kind)).with_property("rank", serde_json::json!(place_rank(&kind, pop)));
                if let Some(p) = pop {
                    f = f.with_property("pop", serde_json::json!(p));
                }
                out.places.insert(format!("n{id}"), f);
            }
            "way" => {
                let pts = coords(el.get("geometry"));
                if pts.len() < 2 {
                    continue;
                }
                let key = format!("w{id}");
                if tag("natural").as_deref() == Some("coastline") {
                    out.coastline.insert(key, pts);
                } else if tag("building").is_some_and(|b| b != "no") {
                    if let Some(r) = closed_ring(pts) {
                        out.buildings.insert(key, Feature::new(Geometry::Polygon(vec![r])).with_property("kind", kind_of("building")));
                    }
                } else if is_water(&tag) {
                    if let Some(r) = closed_ring(pts) {
                        out.water.insert(key, Feature::new(Geometry::Polygon(vec![r])).with_property("kind", kind_of("lake")));
                    }
                } else if matches!(tag("waterway").as_deref(), Some("river" | "canal")) {
                    out.waterways.insert(key, Feature::new(Geometry::LineString(pts)).with_property("kind", kind_of("river")));
                } else if is_park(&tag) {
                    if let Some(r) = closed_ring(pts) {
                        out.parks.insert(key, Feature::new(Geometry::Polygon(vec![r])).with_property("kind", kind_of("park")));
                    }
                } else if let Some(h) = tag("highway") {
                    out.roads.insert(key, Feature::new(Geometry::LineString(pts)).with_property("type", serde_json::json!(h)));
                }
            }
            "relation" => {
                let (water, park) = (is_water(&tag), is_park(&tag));
                if !water && !park {
                    continue;
                }
                let segments: Vec<Vec<[f64; 2]>> = el.get("members").and_then(Json::as_array).into_iter().flatten()
                    .filter(|m| m.get("type").and_then(Json::as_str) == Some("way"))
                    .map(|m| coords(m.get("geometry")).into_iter().map(|p| [p.x, p.y]).collect::<Vec<_>>())
                    .filter(|s| s.len() >= 2)
                    .collect();
                let rings: Vec<Vec<Vec2>> = assemble_rings(segments).into_iter().map(|r| r.into_iter().map(|p| Vec2::new(p[0], p[1])).collect()).collect();
                let Some(g) = even_odd(rings) else { continue };
                let key = format!("r{id}");
                if water {
                    out.water.insert(key, Feature::new(g).with_property("kind", kind_of("lake")));
                } else {
                    out.parks.insert(key, Feature::new(g).with_property("kind", kind_of("park")));
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The recorded answer the tests of other modules build from.
#[cfg(test)]
pub(crate) fn tests_fixture() -> &'static str {
    tests::COPACABANA
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recorded answer (trimmed) for a cell over Copacabana.
    pub(crate) const COPACABANA: &str = r#"{"version":0.6,"generator":"Overpass API","osm3s":{"timestamp_osm_base":"2026-09-20T10:11:12Z","copyright":"ODbL"},"elements":[
      {"type":"node","id":1,"lat":-22.9711,"lon":-43.1823,"tags":{"place":"suburb","name":"Copacabana","population":"146392"}},
      {"type":"node","id":2,"lat":-22.9068,"lon":-43.1729,"tags":{"place":"city","name":"Rio de Janeiro","name:sv":"Rio de Janeiro","population":"6747815"}},
      {"type":"node","id":3,"lat":-22.95,"lon":-43.16,"tags":{"place":"neighbourhood","name":"Leme","name:pt":"Leme"}},
      {"type":"way","id":10,"tags":{"highway":"primary","name":"Avenida Atlântica"},"geometry":[{"lat":-22.985,"lon":-43.19},{"lat":-22.965,"lon":-43.17}]},
      {"type":"way","id":11,"tags":{"highway":"residential"},"geometry":[{"lat":-22.97,"lon":-43.185},{"lat":-22.975,"lon":-43.18}]},
      {"type":"way","id":12,"tags":{"building":"yes"},"geometry":[{"lat":-22.97,"lon":-43.186},{"lat":-22.97,"lon":-43.185},{"lat":-22.971,"lon":-43.185},{"lat":-22.97,"lon":-43.186}]},
      {"type":"way","id":13,"tags":{"building":"yes"},"geometry":[{"lat":-22.97,"lon":-43.186},{"lat":-22.97,"lon":-43.185}]},
      {"type":"way","id":14,"tags":{"natural":"coastline"},"geometry":[{"lat":-23.14,"lon":-43.40},{"lat":-22.84,"lon":-43.00}]},
      {"type":"way","id":15,"tags":{"natural":"water","name":"Lagoa"},"geometry":[{"lat":-22.97,"lon":-43.21},{"lat":-22.97,"lon":-43.20},{"lat":-22.98,"lon":-43.20},{"lat":-22.98,"lon":-43.21},{"lat":-22.97,"lon":-43.21}]},
      {"type":"way","id":16,"tags":{"leisure":"park"},"geometry":[{"lat":-22.95,"lon":-43.17},{"lat":-22.95,"lon":-43.168},{"lat":-22.952,"lon":-43.168},{"lat":-22.95,"lon":-43.17}]},
      {"type":"way","id":17,"tags":{"waterway":"canal"},"geometry":[{"lat":-22.98,"lon":-43.22},{"lat":-22.99,"lon":-43.23}]},
      {"type":"relation","id":20,"tags":{"type":"multipolygon","natural":"water"},"members":[
        {"type":"way","ref":100,"role":"outer","geometry":[{"lat":0,"lon":0},{"lat":0,"lon":1}]},
        {"type":"way","ref":101,"role":"outer","geometry":[{"lat":0,"lon":1},{"lat":1,"lon":1},{"lat":1,"lon":0},{"lat":0,"lon":0}]},
        {"type":"way","ref":102,"role":"inner","geometry":[{"lat":0.4,"lon":0.4},{"lat":0.4,"lon":0.6},{"lat":0.6,"lon":0.6},{"lat":0.6,"lon":0.4},{"lat":0.4,"lon":0.4}]}]}
    ]}"#;

    #[test]
    fn queries_name_their_cell_and_level_only() {
        let cell = TileId::from_lonlat(Vec2::new(-43.18, -22.97), Level::Z12.cell_zoom());
        let q = query(Level::Z12, cell);
        let b = cell.lonlat_bounds();
        assert!(q.starts_with("[out:json]") && q.contains(&format!("[bbox:{:.7},{:.7},{:.7},{:.7}]", b.south, b.west, b.north, b.east)), "{q}");
        assert!(q.contains("residential") && q.contains("quarter") && q.contains("coastline") && q.ends_with("out geom;\n"));
        assert!(!q.contains("building"), "buildings from z13");
        assert!(query(Level::Z13, cell).contains("way[\"building\"]"));
        assert!(!query(Level::Z10, cell).contains("tertiary") && !query(Level::Z10, cell).contains("leisure"));
        // Same cell, same level: the same text (the cache key); anything else differs.
        assert_eq!(q, query(Level::Z12, cell));
        assert_ne!(crate::fetch::key(&q), crate::fetch::key(&query(Level::Z11, cell)));
    }

    #[test]
    fn cells_cover_regions_at_the_level_cell_zoom() {
        let copacabana = GeoBbox::new(-43.20, -22.99, -43.16, -22.96);
        let c = cells(Level::Z13, &[copacabana]);
        assert!(!c.is_empty() && c.iter().all(|t| t.z == 13));
        assert!(c.len() <= 9, "{}", c.len());
        // Two overlapping regions ask for each cell once, in order.
        let twice = cells(Level::Z13, &[copacabana, copacabana]);
        assert_eq!(c, twice);
        assert_eq!(cells(Level::Z10, &[copacabana]).len(), 1, "one z9 cell holds it");
        assert_eq!(Level::for_zoom(9), None);
        assert_eq!(Level::for_zoom(14), Some(Level::Z13));
    }

    #[test]
    fn answers_become_layers() {
        let l = parse(COPACABANA.as_bytes(), Some("sv")).unwrap();
        assert_eq!(l.timestamp.as_deref(), Some("2026-09-20T10:11:12Z"));
        assert_eq!(l.roads.len(), 2);
        assert_eq!(l.roads["w10"].property_str("type").as_deref(), Some("primary"));
        assert_eq!(l.buildings.len(), 1, "the unclosed building is dropped");
        assert_eq!(l.coastline.len(), 1);
        assert_eq!(l.waterways.len(), 1);
        assert_eq!(l.parks.len(), 1);
        // The lagoon and the multipolygon lake with its island (a hole).
        assert_eq!(l.water.len(), 2);
        match &l.water["r20"].geometry {
            Geometry::Polygon(p) => assert_eq!(p.len(), 2, "outer + island hole"),
            g => panic!("{g:?}"),
        }
        let city = &l.places["n2"];
        assert_eq!(city.property("rank"), Some(&serde_json::json!(1)));
        assert_eq!(city.property("pop"), Some(&serde_json::json!(6747815.0)));
        assert_eq!(l.places["n1"].property("rank"), Some(&serde_json::json!(7)));
        assert!(l.places["n3"].property("pop").is_none());
        // Names in the document's language where OSM has them.
        let pt = parse(COPACABANA.as_bytes(), Some("pt")).unwrap();
        assert_eq!(pt.places["n3"].property_str("name").as_deref(), Some("Leme"));
    }

    #[test]
    fn merged_cells_keep_one_copy_per_element() {
        let a = parse(COPACABANA.as_bytes(), None).unwrap();
        let mut m = a.clone();
        m.merge(&a);
        assert_eq!(m.roads.len(), a.roads.len());
        assert_eq!(m.timestamp, a.timestamp);
    }
}
