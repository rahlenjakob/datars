//! Automatic basemaps (docs/09-geo.md): where a document's cameras look in, a camera-aware
//! archive of the standard layers out — from open data fetched once into the local cache
//! ([`crate::fetch`]), so an author with a story about Rio never hand-makes an extract.
//!
//! 1. [`plan`]: the views (every state settled and the flights between, from the engine's own
//!    cameras) → a [`Cover`] → the [`recipe::auto_job`] tile job. Explorable views add the zooms a
//!    reader can zoom into around them, within a tile budget.
//! 2. [`needs`]: what the job reads — Natural Earth layers, osmdata's simplified land, and one
//!    Overpass cell per street-level [`Level`] and tile ([`crate::overpass`]); [`missing`] and
//!    [`fetch`] fill the cache (the dev loop does it in the background, the build up front).
//! 3. [`Source`]: the job's loader over the cache — the same bytes in give the same archive out, so
//!    a rebuild is offline and byte-identical. A *preview* source skips what isn't cached yet
//!    (`datars dev` shows Natural Earth at once and fills the streets in as cells arrive).

use crate::camera::{CoverOptions, Extent};
use crate::fetch::Cache;
use crate::overpass::{self, Layers, Level};
use crate::recipe::{self, RecipeOptions};
use crate::TileJob;
use datars_geo::{Feature, FeatureCollection, GeoBbox, Geometry, TileId};
use datars_math::Vec2;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

/// How an automatic archive is cut.
#[derive(Clone, Debug)]
pub struct AutoOptions {
    /// Label language (`name:<lang>` in OSM, `NAME_<LANG>` in Natural Earth).
    pub lang: Option<String>,
    /// The job's `about`.
    pub about: String,
    pub cover: CoverOptions,
    /// Zoom levels past its own an explorable view gets real data for (a reader zooming further
    /// sees the last level overzoomed).
    pub explore_zooms: u8,
    /// Tiles an archive may hold before explorable views get fewer extra levels.
    pub max_tiles: u64,
    /// Overpass cells explorable views may add (each a request to a public server — a minute on a
    /// busy day for a dense city): past it they get fewer extra levels.
    pub max_explore_cells: usize,
}

impl Default for AutoOptions {
    fn default() -> AutoOptions {
        AutoOptions {
            lang: None,
            about: String::new(),
            // Settled views take the zoom below theirs too, and the one above when near it: the
            // same document drawn wider or narrower than authored (a phone, a wide screen) asks for
            // a neighbouring zoom, and should find it rather than a coarse ancestor.
            cover: CoverOptions { settled_slack: (1.0, 0.6), ..CoverOptions::default() },
            explore_zooms: 3,
            max_tiles: 6000,
            max_explore_cells: 24,
        }
    }
}

fn cell_count(job: &TileJob) -> usize {
    needs(job).iter().filter(|n| matches!(n, Need::Cell(..))).count()
}

/// The tile job for `views` (settled ones labelled, flight frames not) and the explorable ones
/// among them.
pub fn plan(views: &[Extent], explore: &[Extent], o: &AutoOptions) -> TileJob {
    let base = recipe::auto_job(crate::camera::cover(views, &o.cover), &RecipeOptions { lang: o.lang.clone(), about: o.about.clone() });
    if explore.is_empty() {
        return base;
    }
    let base_cells = cell_count(&base);
    let mut levels = o.explore_zooms;
    loop {
        let mut all = views.to_vec();
        for e in explore {
            // Around the view (a reader pans a little), at each zoom it can zoom into.
            let b = e.bbox;
            let (dx, dy) = ((b[2] - b[0]) * 0.25, (b[3] - b[1]) * 0.25);
            let grown = [(b[0] - dx).max(-180.0), (b[1] - dy).max(-85.0), (b[2] + dx).min(180.0), (b[3] + dy).min(85.0)];
            for k in 1..=levels {
                all.push(Extent { bbox: grown, zoom: e.zoom + k as f64, label: String::new() });
            }
        }
        if levels == 0 {
            return base;
        }
        let cover = crate::camera::cover(&all, &o.cover);
        if crate::camera::tile_count(&cover) <= o.max_tiles {
            let job = recipe::auto_job(cover, &RecipeOptions { lang: o.lang.clone(), about: o.about.clone() });
            if cell_count(&job) <= base_cells + o.max_explore_cells {
                return job;
            }
        }
        levels -= 1;
    }
}

/// One thing a job reads that must be in the cache first.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Need {
    NaturalEarth(String),
    LandSimplified,
    /// An Overpass cell at a level.
    Cell(Level, TileId),
}

impl Need {
    pub fn describe(&self) -> String {
        match self {
            Need::NaturalEarth(l) => format!("Natural Earth {l}"),
            Need::LandSimplified => "osmdata simplified land polygons".into(),
            Need::Cell(l, t) => format!("OSM z{} cell {}/{}/{}", l.zoom(), t.z, t.x, t.y),
        }
    }

    pub fn is_cached(&self, cache: &Cache) -> bool {
        match self {
            Need::NaturalEarth(l) => cache.has_natural_earth(l),
            Need::LandSimplified => cache.has_land_simplified(),
            Need::Cell(l, t) => cache.has_overpass(&overpass::query(*l, *t)),
        }
    }
}

/// An input name → (the OSM layer, its level), for `osm:<layer>:<zoom>`.
fn osm_input(input: &str) -> Option<(&str, Level)> {
    let rest = input.strip_prefix("osm:")?;
    let (layer, z) = rest.rsplit_once(':')?;
    Some((layer, Level::for_zoom(z.parse().ok()?)?))
}

/// Everything `job` reads, in a sensible fetch order: Natural Earth, then land, then cells by level.
pub fn needs(job: &TileJob) -> Vec<Need> {
    let mut out = BTreeSet::new();
    let boxes = input_boxes(job);
    for (input, regions) in crate::input_regions(job) {
        if regions.as_ref().is_some_and(|r| r.is_empty()) {
            continue;
        }
        if let Some(stem) = input.strip_suffix(".geojson") {
            out.insert(Need::NaturalEarth(stem.to_string()));
        } else if input == recipe::OSMDATA_LAND {
            out.insert(Need::LandSimplified);
        } else if let Some((layer, level)) = osm_input(&input) {
            // Where no coastline is near enough to say, land is decided by Natural Earth.
            if layer == "land" {
                out.insert(Need::NaturalEarth(ne_land_stem().into()));
            }
            let b = boxes.get(&input).cloned().unwrap_or_default();
            out.extend(overpass::cells(level, &if layer == "land" { grown(&b) } else { b }).into_iter().map(|t| Need::Cell(level, t)));
        }
    }
    out.into_iter().collect()
}

fn ne_land_stem() -> &'static str {
    recipe::NE10_LAND.trim_end_matches(".geojson")
}

/// Per input, the boxes of the cover's tile ranges at the zooms its layers use — the exact tiles,
/// not the coarse union a loader is told about, so a flight's diagonal doesn't pull in the whole
/// rectangle around it.
pub fn input_boxes(job: &TileJob) -> BTreeMap<String, Vec<GeoBbox>> {
    let mut out: BTreeMap<String, Vec<GeoBbox>> = BTreeMap::new();
    for spec in &job.layers {
        let boxes = out.entry(spec.input.clone()).or_default();
        for r in job.cover.iter().filter(|r| (spec.minzoom as u32..=spec.maxzoom as u32).contains(&r[0])) {
            let (a, b) = (TileId::new(r[0] as u8, r[1], r[2]).lonlat_bounds(), TileId::new(r[0] as u8, r[3], r[4]).lonlat_bounds());
            let bx = GeoBbox::new(a.west, b.south, b.east, a.north);
            if !boxes.contains(&bx) {
                boxes.push(bx);
            }
        }
    }
    out
}

/// Land is assembled over boxes grown a little past the tiles that need it (so no tile edge is a
/// polygon edge).
fn grown(boxes: &[GeoBbox]) -> Vec<GeoBbox> {
    boxes.iter().map(|r| {
        let (dx, dy) = (r.width() * 0.02, r.height() * 0.02);
        GeoBbox::new((r.west - dx).max(-180.0), (r.south - dy).max(-85.06), (r.east + dx).min(180.0), (r.north + dy).min(85.06))
    }).collect()
}

/// What isn't in the cache yet.
pub fn missing(cache: &Cache, needs: &[Need]) -> Vec<Need> {
    needs.iter().filter(|n| !n.is_cached(cache)).cloned().collect()
}

/// Put one need in the cache (network allowed unless the cache is offline).
pub fn fetch(cache: &Cache, need: &Need) -> Result<(), String> {
    match need {
        Need::NaturalEarth(l) => cache.natural_earth(l).map(|_| ()),
        Need::LandSimplified => cache.land_simplified().map(|_| ()),
        Need::Cell(l, t) => cache.osm_cell(&overpass::query(*l, *t), l.zoom(), t.z, t.x, t.y, &need.describe()).map(|_| ()),
    }
}

/// Requests in flight at once: one — the public Overpass instances ask for no parallel requests
/// (a static cell server of our own could take more).
pub const FETCH_WORKERS: usize = 1;

/// Put all of `needs` in the cache, `workers` at a time. Everything is tried (what arrives stays
/// cached for next time); the first failure is the result.
pub fn fetch_all(cache: &Cache, needs: &[Need], workers: usize, progress: &(dyn Fn(usize, usize, &Need) + Sync)) -> Result<(), String> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let first_err: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..workers.max(1).min(needs.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let Some(n) = needs.get(i) else { break };
                progress(i, needs.len(), n);
                if let Err(e) = fetch(cache, n) {
                    first_err.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert(e);
                }
            });
        }
    });
    match first_err.into_inner().unwrap_or_else(|p| p.into_inner()) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// How current the OpenStreetMap data behind `needs` is (from the cached answers), as the job's
/// `data_version`.
pub fn data_version(cache: &Cache, needs: &[Need]) -> String {
    let stamps: BTreeSet<String> = needs.iter().filter_map(|n| match n {
        Need::Cell(l, t) => cache.overpass_cached(&overpass::query(*l, *t)).and_then(|b| crate::fetch::overpass_timestamp(&b)),
        _ => None,
    }).collect();
    match (stamps.first(), stamps.last()) {
        (Some(a), Some(b)) if a == b => format!("OpenStreetMap {a}"),
        (Some(a), Some(b)) => format!("OpenStreetMap {a} … {b}"),
        _ => String::new(),
    }
}

/// A job's loader over the cache. Keeps what it has read (cells parsed, Natural Earth, land) across
/// builds, so a dev server rebuilding as cells arrive only reads the new ones.
pub struct Source {
    pub cache: Arc<Cache>,
    lang: Option<String>,
    /// Fetch what's missing (a build) or skip it (a preview).
    pub network: bool,
    cells: RefCell<BTreeMap<(Level, TileId), Rc<Layers>>>,
    ne: RefCell<BTreeMap<String, Rc<FeatureCollection>>>,
    osmdata: RefCell<Option<(String, Rc<FeatureCollection>)>>,
    /// The job being built's tile ranges per input ([`input_boxes`]): where OSM is read.
    boxes: RefCell<BTreeMap<String, Vec<GeoBbox>>>,
    /// What a preview build left out (not cached yet).
    pub skipped: RefCell<BTreeSet<Need>>,
}

impl Source {
    pub fn new(cache: Arc<Cache>, lang: Option<String>, network: bool) -> Source {
        Source { cache, lang, network, cells: RefCell::default(), ne: RefCell::default(), osmdata: RefCell::default(), boxes: RefCell::default(), skipped: RefCell::default() }
    }

    /// The job about to be loaded for (its tile ranges say where OSM cells are read).
    pub fn set_job(&self, job: &TileJob) {
        *self.boxes.borrow_mut() = input_boxes(job);
    }

    /// Where a level is read: the job's ranges, or (loading without a job) the regions asked for.
    fn boxes_for(&self, input: &str, regions: &[GeoBbox]) -> Vec<GeoBbox> {
        match self.boxes.borrow().get(input) {
            Some(b) => b.clone(),
            None => regions.to_vec(),
        }
    }

    /// The label language changed (another document): parsed cells are per language.
    pub fn set_lang(&mut self, lang: Option<String>) {
        if lang != self.lang {
            self.lang = lang;
            self.cells.borrow_mut().clear();
        }
    }

    /// Features of `input` where `regions` reach (the [`crate::Loader`] contract).
    pub fn load(&self, input: &str, regions: &[GeoBbox]) -> Result<FeatureCollection, String> {
        if let Some(stem) = input.strip_suffix(".geojson") {
            return Ok((*self.natural_earth(stem)?.unwrap_or_default()).clone());
        }
        if input == recipe::OSMDATA_LAND {
            return self.osmdata_land(regions);
        }
        let (layer, level) = osm_input(input).ok_or_else(|| format!("unknown input `{input}`"))?;
        if layer == "land" {
            return self.land(input, level, regions);
        }
        let mut merged = Layers::default();
        for t in overpass::cells(level, &self.boxes_for(input, regions)) {
            if let Some(c) = self.cell(level, t)? {
                merged.merge(&c);
            }
        }
        let pick = |m: &BTreeMap<String, Feature>| FeatureCollection::new(m.values().cloned().collect());
        Ok(match layer {
            "roads" => pick(&merged.roads),
            "water" => pick(&merged.water),
            "waterways" => pick(&merged.waterways),
            "parks" => pick(&merged.parks),
            "buildings" => pick(&merged.buildings),
            "places" => pick(&merged.places),
            other => return Err(format!("unknown OSM layer `{other}`")),
        })
    }

    fn natural_earth(&self, stem: &str) -> Result<Option<Rc<FeatureCollection>>, String> {
        if let Some(fc) = self.ne.borrow().get(stem) {
            return Ok(Some(fc.clone()));
        }
        let need = Need::NaturalEarth(stem.to_string());
        if !self.network && !need.is_cached(&self.cache) {
            self.skipped.borrow_mut().insert(need);
            return Ok(None);
        }
        let path = self.cache.natural_earth(stem)?;
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let fc = Rc::new(datars_geo::parse_geojson(&bytes, &datars_geo::GeoJsonOptions::default()).map_err(|e| format!("{stem}: {e}"))?);
        self.ne.borrow_mut().insert(stem.to_string(), fc.clone());
        Ok(Some(fc))
    }

    fn osmdata_land(&self, regions: &[GeoBbox]) -> Result<FeatureCollection, String> {
        let key = format!("{regions:?}");
        if let Some((k, fc)) = self.osmdata.borrow().as_ref() {
            if *k == key {
                return Ok((**fc).clone());
            }
        }
        if !self.network && !self.cache.has_land_simplified() {
            // A preview before the download: Natural Earth's coarser coast.
            self.skipped.borrow_mut().insert(Need::LandSimplified);
            return Ok(self.natural_earth(ne_land_stem())?.map(|f| (*f).clone()).unwrap_or_default());
        }
        let path = self.cache.land_simplified()?;
        let f = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let fc = crate::shp::read_polygons_in(std::io::BufReader::with_capacity(1 << 20, f), regions, crate::shp::Crs::WebMercator)?;
        let fc = Rc::new(fc);
        *self.osmdata.borrow_mut() = Some((key, fc.clone()));
        Ok((*fc).clone())
    }

    /// A cell's layers: parsed from the cache, fetched first if allowed, or `None` (a preview's gap).
    fn cell(&self, level: Level, t: TileId) -> Result<Option<Rc<Layers>>, String> {
        if let Some(c) = self.cells.borrow().get(&(level, t)) {
            return Ok(Some(c.clone()));
        }
        let q = overpass::query(level, t);
        let need = Need::Cell(level, t);
        let body = match self.cache.overpass_cached(&q) {
            Some(b) => b,
            None if self.network => self.cache.osm_cell(&q, level.zoom(), t.z, t.x, t.y, &need.describe())?,
            None => {
                self.skipped.borrow_mut().insert(need);
                return Ok(None);
            }
        };
        let layers = Rc::new(overpass::parse(&body, self.lang.as_deref())?);
        self.cells.borrow_mut().insert((level, t), layers.clone());
        Ok(Some(layers))
    }

    /// Land at a street level: per region, the coastline of its cells assembled into polygons
    /// ([`crate::coast`]); Natural Earth decides where no coastline is near, and stands in for a
    /// region whose cells aren't all cached yet (a preview).
    fn land(&self, input: &str, level: Level, regions: &[GeoBbox]) -> Result<FeatureCollection, String> {
        let ne = self.natural_earth(ne_land_stem())?;
        let ne_land = |p: Vec2| ne.as_ref().is_some_and(|fc| fc.iter().any(|f| datars_geo::measure::bbox(&f.geometry).is_some_and(|b| b.contains(p)) && f.geometry.polygons().iter().any(|poly| datars_geo::clip::polygon_contains_planar(poly, p))));
        let mut out = Vec::new();
        // A range inside another (a close-up's tiles inside its wider zoom's) would only add the
        // same land twice.
        let boxes = grown(&self.boxes_for(input, regions));
        let inside = |a: &GeoBbox, b: &GeoBbox| a.west >= b.west && a.east <= b.east && a.south >= b.south && a.north <= b.north;
        let boxes: Vec<GeoBbox> = boxes.iter().enumerate().filter(|(i, a)| !boxes.iter().enumerate().any(|(j, b)| j != *i && inside(a, b) && (!inside(b, a) || j < *i))).map(|(_, a)| *a).collect();
        for region in boxes {
            let cells = overpass::cells(level, std::slice::from_ref(&region));
            let mut ways: BTreeMap<String, Vec<Vec2>> = BTreeMap::new();
            let mut complete = true;
            for t in &cells {
                match self.cell(level, *t)? {
                    Some(c) => ways.extend(c.coastline.iter().map(|(k, v)| (k.clone(), v.clone()))),
                    None => complete = false,
                }
            }
            if !complete {
                if let Some(fc) = &ne {
                    out.extend(fc.iter().filter(|f| datars_geo::measure::bbox(&f.geometry).is_some_and(|b| overlaps(&b, &region))).cloned());
                }
                continue;
            }
            let data = cells.iter().map(|t| t.lonlat_bounds()).reduce(|a, b| GeoBbox::new(a.west.min(b.west), a.south.min(b.south), a.east.max(b.east), a.north.max(b.north))).unwrap_or(region);
            let ways: Vec<Vec<Vec2>> = ways.into_values().collect();
            for poly in crate::coast::land(region, &ways, data, &ne_land) {
                out.push(Feature::new(Geometry::Polygon(poly)));
            }
        }
        Ok(FeatureCollection::new(out))
    }
}

fn overlaps(a: &GeoBbox, b: &GeoBbox) -> bool {
    !(a.east < b.west || a.west > b.east || a.north < b.south || a.south > b.north)
}

/// An automatic archive: its bytes, the job that made it, and what a preview left out.
pub struct Built {
    pub bytes: Vec<u8>,
    pub job: TileJob,
    pub stats: crate::TileStats,
    /// Inputs a preview skipped (empty for a complete build).
    pub skipped: Vec<Need>,
}

/// Build `job` from `src` (fetching what's missing first when the source may use the network).
pub fn build(mut job: TileJob, src: &Source, progress: &(dyn Fn(usize, usize, &Need) + Sync)) -> Result<Built, String> {
    let all = needs(&job);
    if src.network {
        fetch_all(&src.cache, &missing(&src.cache, &all), FETCH_WORKERS, progress)?;
    }
    src.skipped.borrow_mut().clear();
    src.set_job(&job);
    job.data_version = data_version(&src.cache, &all);
    let (bytes, stats) = crate::tiles(&job, &|input, regions| src.load(input, regions))?;
    let skipped = src.skipped.borrow().iter().cloned().collect();
    Ok(Built { bytes, job, stats, skipped })
}

/// A job's plan without what's filled in after fetching — to tell whether an archive on disk was
/// made for the same views and recipe (then it's current; see `datars-build`).
pub fn same_plan(a: &TileJob, b: &TileJob) -> bool {
    let strip = |j: &TileJob| {
        let mut j = j.clone();
        j.data_version.clear();
        crate::job_json(&j)
    };
    strip(a) == strip(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::tests::{temp, Recorded};

    fn view(lon: f64, lat: f64, zoom: f64, label: &str) -> Extent {
        Extent::centered(lon, lat, zoom, 760.0, 480.0).named(label)
    }

    #[test]
    fn plans_cover_views_and_street_bands_read_osm() {
        let views = [view(0.0, 20.0, 1.0, "world"), view(-43.18, -22.97, 14.2, "copacabana")];
        let job = plan(&views, &[], &AutoOptions::default());
        let at = |name: &str, z: u8| -> Vec<&str> { job.layers.iter().filter(|l| l.name == name && (l.minzoom..=l.maxzoom).contains(&z)).map(|l| l.input.as_str()).collect() };
        assert_eq!(job.maxzoom, 14);
        assert_eq!(at("land", 0), vec![recipe::NE50_LAND]);
        // Street zoom: z12 cells for streets, water and the coast, z13 cells for buildings.
        assert_eq!(at("land", 14), vec!["osm:land:12"]);
        assert_eq!(at("roads", 14), vec!["osm:roads:12"]);
        assert_eq!(at("buildings", 14), vec!["osm:buildings:13"]);
        assert!(job.unserved.is_empty(), "automatic archives serve every view");
        // Settled views take the zoom below theirs (a narrower screen) and above (14.2 + 0.6).
        let zooms: BTreeSet<u32> = job.cover.iter().map(|r| r[0]).collect();
        assert!(zooms.contains(&13) && zooms.contains(&14), "{zooms:?}");
        // Needs: Natural Earth, and Overpass cells for the street levels only near Copacabana.
        let n = needs(&job);
        assert!(n.contains(&Need::NaturalEarth("ne_50m_land".into())));
        let cells: Vec<&Need> = n.iter().filter(|x| matches!(x, Need::Cell(..))).collect();
        assert!(!cells.is_empty() && cells.len() < 30, "{cells:?}");
        assert!(cells.iter().all(|c| matches!(c, Need::Cell(Level::Z12 | Level::Z13, t) if (t.lonlat_bounds().center().x - -43.18).abs() < 0.2 && (t.lonlat_bounds().center().y - -22.97).abs() < 0.2)), "{cells:?}");
        assert!(cells.iter().any(|c| matches!(c, Need::Cell(Level::Z13, _))) && cells.iter().any(|c| matches!(c, Need::Cell(Level::Z12, _))));
    }

    #[test]
    fn explorable_views_get_extra_zooms_within_the_budget() {
        let city = view(-43.18, -22.97, 11.0, "rio");
        let generous = AutoOptions { max_explore_cells: 1000, ..AutoOptions::default() };
        let plain = plan(std::slice::from_ref(&city), &[], &generous);
        let explore = plan(std::slice::from_ref(&city), std::slice::from_ref(&city), &generous);
        let max = |j: &TileJob| j.cover.iter().map(|r| r[0]).max().unwrap_or(0);
        assert!(max(&explore) >= max(&plain) + 2, "{} vs {}", max(&explore), max(&plain));
        // Over the tile budget: fewer levels.
        let tight = plan(std::slice::from_ref(&city), std::slice::from_ref(&city), &AutoOptions { max_tiles: crate::camera::tile_count(&plain.cover) + 40, ..generous.clone() });
        assert!(crate::camera::tile_count(&tight.cover) <= crate::camera::tile_count(&plain.cover) + 40 || max(&tight) == max(&plain));
        // Over the cell budget (the default: requests to public servers are what's scarce).
        let default = plan(std::slice::from_ref(&city), std::slice::from_ref(&city), &AutoOptions::default());
        assert!(cell_count(&default) <= cell_count(&plain) + AutoOptions::default().max_explore_cells, "{} vs {}", cell_count(&default), cell_count(&plain));
        assert!(max(&default) < max(&explore), "fewer levels than the generous plan");
    }

    /// A tiny Natural Earth: one land square around Rio, every other layer empty.
    fn seed_ne(dir: &std::path::Path) {
        let empty = r#"{"type":"FeatureCollection","features":[]}"#;
        for l in ["ne_50m_land", "ne_50m_lakes", "ne_10m_lakes", "ne_10m_rivers_lake_centerlines", "ne_50m_admin_0_boundary_lines_land", "ne_10m_admin_0_boundary_lines_land", "ne_10m_roads", "ne_10m_populated_places"] {
            std::fs::write(dir.join(format!("{l}.geojson")), empty).unwrap();
        }
        let land = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[-44,-24],[-42,-24],[-42,-22],[-44,-22],[-44,-24]]]}}]}"#;
        std::fs::write(dir.join("ne_10m_land.geojson"), land).unwrap();
    }

    #[test]
    fn builds_from_recorded_answers_then_offline_and_byte_identical() {
        let (root, seed) = (temp("auto-root"), temp("auto-seed"));
        seed_ne(&seed);
        let http = Recorded::default();
        let answer = crate::overpass::tests_fixture();
        http.posts.lock().unwrap().insert("https://overpass.test/api".into(), vec![(200, answer.as_bytes().to_vec())]);
        let mk = |h: Recorded| {
            let mut c = Cache::new(root.clone(), Box::new(h)).with_mirrors(vec!["https://overpass.test/api".into()], std::time::Duration::ZERO).with_log(Box::new(|_| {}));
            c.seeds = vec![seed.clone()];
            Arc::new(c)
        };
        // A street view only (no world view: the test stays small): z12–14 around Copacabana.
        let views = [view(-43.18, -22.97, 13.6, "copacabana")];
        let o = AutoOptions { cover: CoverOptions { world_zoom: 0, settled_slack: (0.3, 0.1), ..CoverOptions::default() }, ..AutoOptions::default() };
        let job = plan(&views, &[], &o);
        let src = Source::new(mk(http.clone()), Some("en".into()), true);
        let first = build(job.clone(), &src, &|_, _, _| {}).unwrap();
        assert!(first.skipped.is_empty());
        assert!(http.calls.lock().unwrap().iter().any(|c| c.starts_with("POST")), "cells fetched");
        assert_eq!(first.job.data_version, "OpenStreetMap 2026-09-20T10:11:12Z");
        // Roads and places made it into a street tile; the archive says how current it is.
        let t = TileId::from_lonlat(Vec2::new(-43.18, -22.975), 13);
        let tile = datars_geo::pmtiles::read_tile(&first.bytes, t).unwrap().expect("covered");
        let vt = datars_geo::mvt::VectorTile::decode(&tile).unwrap();
        assert!(vt.layer("roads").is_some_and(|l| !l.features.is_empty()), "{:?}", vt.layers.iter().map(|l| &l.name).collect::<Vec<_>>());
        assert!(vt.layer("land").is_some_and(|l| !l.features.is_empty()));
        let meta = datars_geo::pmtiles::Reader::open(&first.bytes[..first.bytes.len().min(16384)]).unwrap();
        let _ = meta;
        // Offline, from the cache alone: the same bytes.
        let offline = Recorded::default();
        let mut c = Cache::new(root.clone(), Box::new(offline.clone())).with_log(Box::new(|_| {}));
        c.seeds = vec![seed.clone()];
        c.offline = true;
        let again = build(job.clone(), &Source::new(Arc::new(c), Some("en".into()), true), &|_, _, _| {}).unwrap();
        assert!(offline.calls.lock().unwrap().is_empty());
        assert_eq!(again.bytes, first.bytes, "byte-identical rebuild");
        assert!(same_plan(&again.job, &job));
        let _ = (std::fs::remove_dir_all(&root), std::fs::remove_dir_all(&seed));
    }

    #[test]
    fn previews_skip_what_is_not_cached() {
        let (root, seed) = (temp("auto-preview"), temp("auto-preview-seed"));
        seed_ne(&seed);
        let mut c = Cache::new(root.clone(), Box::new(Recorded::default())).with_log(Box::new(|_| {}));
        c.seeds = vec![seed.clone()];
        let views = [view(-43.18, -22.97, 12.2, "rio")];
        let o = AutoOptions { cover: CoverOptions { world_zoom: 0, settled_slack: (0.3, 0.1), ..CoverOptions::default() }, ..AutoOptions::default() };
        let job = plan(&views, &[], &o);
        let src = Source::new(Arc::new(c), None, false);
        let b = build(job.clone(), &src, &|_, _, _| panic!("a preview fetches nothing")).unwrap();
        assert!(!b.skipped.is_empty() && b.skipped.iter().all(|n| matches!(n, Need::Cell(..))), "{:?}", b.skipped);
        // Natural Earth's land stands in for the street-level coast meanwhile.
        let t = TileId::from_lonlat(Vec2::new(-43.18, -22.97), 12);
        let vt = datars_geo::mvt::VectorTile::decode(&datars_geo::pmtiles::read_tile(&b.bytes, t).unwrap().unwrap()).unwrap();
        assert!(vt.layer("land").is_some_and(|l| !l.features.is_empty()));
        let _ = (std::fs::remove_dir_all(&root), std::fs::remove_dir_all(&seed));
    }
}
