//! Automatic basemaps in the build (docs/09-geo.md): a `tiles` source whose URL is `"auto"` gets
//! an archive cut to the document's own cameras — every state's views and the flights between
//! them, as the engine evaluates them ([`datars_engine::Engine::tile_views`]) — filled with the
//! standard basemap layers from open data fetched once into the local cache
//! ([`datars_geo_build::fetch`]). The author writes a story about Rio; the build fetches Rio.
//!
//! The archive is written next to the document as `<source>.auto.pmtiles` (where every host looks
//! for it, [`datars_ir::tiles_file`]), with the job that made it beside it
//! (`<source>.auto.job.json`, the lockfile: an archive whose job matches the current plan is
//! current, and is used as it is — offline, byte for byte). `publish` and `bundle` ship it under a
//! content-hashed name ([`rewrite`]).

use datars_engine::TileView;
use datars_geo_build::auto::{self as ga, AutoOptions};
use datars_geo_build::camera::Extent;
/// The acquisition side (needs, fetching, building from a [`Source`]), for tools that drive it
/// themselves (`datars dev` fetches in the background).
pub use datars_geo_build::auto as geo;
pub use datars_geo_build::auto::{Need, Source};
pub use datars_geo_build::fetch::Cache;
pub use datars_geo_build::TileJob;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Frames sampled per flight between two states: enough that consecutive frames' views overlap
/// at every zoom they pass.
pub const FLIGHT_SAMPLES: usize = 48;

/// The document's automatic tiles sources.
pub fn sources(doc: &datars_ir::Doc) -> Vec<String> {
    doc.data.iter().filter(|(_, s)| matches!(&s.from, datars_ir::SourceKind::Tiles(u) if u == datars_ir::AUTO_TILES)).map(|(n, _)| n.clone()).collect()
}

/// What one automatic source must hold.
#[derive(Clone, Debug)]
pub struct Plan {
    pub source: String,
    /// The archive's file name next to the document.
    pub file: String,
    pub job: TileJob,
    pub lang: Option<String>,
}

/// The label language of a document: its locale's language (`pt-BR` → `pt`).
pub fn lang(doc: &datars_ir::Doc) -> Option<String> {
    let l = doc.locale.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
    (!l.is_empty()).then_some(l)
}

/// Plan every automatic source from the views a loaded document's engine reports. `extra` are
/// views seen elsewhere (where readers explored in `datars dev`), covered like explorable ones.
pub fn plan_views(doc: &datars_ir::Doc, views: &[TileView], extra: &[TileView]) -> Vec<Plan> {
    let lang = lang(doc);
    let name = if doc.title.is_empty() { doc.id.clone() } else { doc.title.clone() };
    sources(doc).into_iter().map(|source| {
        let ext = |v: &TileView| Extent { bbox: v.bbox, zoom: v.zoom, label: v.state.clone() };
        let mine: Vec<Extent> = views.iter().filter(|v| v.source == source).map(ext).collect();
        // Views readers reached are covered as they are (reaching further reports again).
        let explore: Vec<Extent> = views.iter().filter(|v| v.source == source && v.explore).map(ext).collect();
        let mut all = mine;
        all.extend(extra.iter().filter(|v| v.source == source).map(|v| Extent { label: format!("seen {}", v.state), ..ext(v) }));
        let about = format!(
            "Automatic basemap for “{name}”: the views its states settle on and the flights between them (camera-aware), \
             from OpenStreetMap (© OpenStreetMap contributors, ODbL) and Natural Earth via the local geodata cache. \
             Made by datars (`datars render|publish|dev`) whenever the document's cameras change — do not edit."
        );
        let o = AutoOptions { lang: lang.clone(), about, ..AutoOptions::default() };
        Plan { file: datars_ir::tiles_file(&source, datars_ir::AUTO_TILES), job: ga::plan(&all, &explore, &o), source, lang: lang.clone() }
    }).collect()
}

/// Plan a document's automatic sources (loading it, with its data from `dir`, to evaluate its
/// cameras). Empty when it has none.
pub fn plan(doc_json: &str, dir: Option<&Path>) -> Result<Vec<Plan>, String> {
    let doc = datars_ir::Doc::from_json(doc_json)?;
    if sources(&doc).is_empty() {
        return Ok(Vec::new());
    }
    let mut e = datars_headless::load_at(doc_json, dir)?;
    let views = e.tile_views(FLIGHT_SAMPLES);
    Ok(plan_views(&doc, &views, &[]))
}

/// An automatic archive on disk.
#[derive(Clone, Debug)]
pub struct Archive {
    pub source: String,
    pub path: PathBuf,
    pub job: TileJob,
    /// Built now (not current before).
    pub rebuilt: bool,
    pub bytes: u64,
}

/// The job file beside an archive (`basemap.auto.pmtiles` → `basemap.auto.job.json`).
pub fn job_path(archive: &Path) -> PathBuf {
    archive.with_extension("job.json")
}

/// Whether `archive` was made for `plan` (its job beside it says so).
pub fn is_current(archive: &Path, plan: &Plan) -> bool {
    let job: Option<TileJob> = std::fs::read(job_path(archive)).ok().and_then(|b| serde_json::from_slice(&b).ok());
    archive.is_file() && job.is_some_and(|j| ga::same_plan(&j, &plan.job))
}

/// Make sure every automatic source of the document has a current archive next to it in `dir`:
/// build what's stale from the cache, fetching what the cache lacks (unless it's offline).
pub fn ensure(doc_json: &str, dir: &Path, cache: Arc<Cache>, log: &(dyn Fn(&str) + Sync)) -> Result<Vec<Archive>, String> {
    let mut out = Vec::new();
    for p in plan(doc_json, Some(dir))? {
        let path = dir.join(&p.file);
        if is_current(&path, &p) {
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push(Archive { source: p.source, path, job: p.job, rebuilt: false, bytes });
            continue;
        }
        log(&format!("basemap `{}`: building {} ({} tiles; data cached in {})", p.source, p.file, datars_geo_build::camera::tile_count(&p.job.cover), cache.root.display()));
        let src = Source::new(cache.clone(), p.lang.clone(), true);
        let progress = |i: usize, n: usize, need: &datars_geo_build::auto::Need| log(&format!("  [{}/{n}] {}", i + 1, need.describe()));
        let (built, partial) = match ga::build(p.job.clone(), &src, &progress) {
            Ok(b) => (b, false),
            // The day's budget for the shared public Overpass servers is spent: build from what
            // the cache has — streets fetched before, Natural Earth elsewhere — rather than fail,
            // and don't record the plan as done, so the next build fetches the rest.
            Err(e) if e.contains("today's budget") => {
                log(&format!("warning: basemap `{}`: {e}\n  building from the geodata cache for now (streets fetched before, Natural Earth elsewhere); the next build fetches the rest", p.source));
                let cached = Source::new(cache.clone(), p.lang.clone(), false);
                (ga::build(p.job, &cached, &progress)?, true)
            }
            Err(e) => return Err(e),
        };
        std::fs::write(&path, &built.bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if partial {
            let _ = std::fs::remove_file(job_path(&path));
        } else {
            std::fs::write(job_path(&path), datars_geo_build::job_json(&built.job)).map_err(|e| e.to_string())?;
        }
        log(&format!("basemap `{}`: {} ({} KB, {} tiles{})", p.source, path.display(), built.bytes.len() / 1024, built.stats.tiles, if built.job.data_version.is_empty() { String::new() } else { format!(", {}", built.job.data_version) }));
        out.push(Archive { source: p.source, bytes: built.bytes.len() as u64, path, job: built.job, rebuilt: true });
    }
    Ok(out)
}

/// [`ensure`] with the environment's geodata cache ([`Cache::from_env`]), reporting on stderr —
/// what tools call before they draw a document (nothing happens without automatic sources).
pub fn ensure_default(doc_json: &str, dir: &Path) -> Result<Vec<Archive>, String> {
    let doc = datars_ir::Doc::from_json(doc_json)?;
    if sources(&doc).is_empty() {
        return Ok(Vec::new());
    }
    ensure(doc_json, dir, Arc::new(Cache::from_env()), &|m| eprintln!("{m}"))
}

/// A short content hash of an archive, for immutable published names.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut h = datars_math::Hash64::new();
    h.bytes(bytes);
    format!("{:016x}", h.finish())
}

/// The document with tiles sources pointing elsewhere (`source → url`): what `publish` and
/// `bundle` compile, so the archive can ship under a name of its own.
pub fn rewrite(doc_json: &str, urls: &BTreeMap<String, String>) -> Result<String, String> {
    let mut v: serde_json::Value = serde_json::from_str(doc_json).map_err(|e| e.to_string())?;
    for (source, url) in urls {
        if let Some(s) = v.get_mut("data").and_then(|d| d.get_mut(source)).and_then(|s| s.as_object_mut()) {
            s.insert("tiles".into(), serde_json::Value::String(url.clone()));
        }
    }
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"{"datars": 1, "size": {"width": 512, "height": 384}, "locale": "pt-BR", "title": "Rio",
        "data": {"base": {"tiles": "auto"}, "other": {"tiles": "fixed.pmtiles"}},
        "signals": {"bounds": {"type": "keyset", "default": [-180, -80, 180, 80]}},
        "scene": {"kind": "view", "key": "map",
          "coord": {"type": "geo", "projection": "web-mercator", "fit": {"bbox": [-180, -85.05, 180, 85.05]}, "padding": 0},
          "camera": {"fit": {"geo": "=bounds"}},
          "children": [{"kind": "tiles", "key": "basemap", "source": "base", "layers": []}]},
        "program": {"states": [{"name": "world"}, {"name": "rio", "set": {"bounds": [-43.3, -23.05, -43.1, -22.9]}}]}}"#;

    #[test]
    fn plans_come_from_the_documents_own_cameras() {
        let doc = datars_ir::Doc::from_json(DOC).unwrap();
        assert_eq!(sources(&doc), vec!["base"]);
        assert_eq!(lang(&doc).as_deref(), Some("pt"));
        let plans = plan(DOC, None).unwrap();
        assert_eq!(plans.len(), 1);
        let p = &plans[0];
        assert_eq!(p.file, "base.auto.pmtiles");
        // The Rio state is about zoom 11 in a 512 px view: street levels there, the world below.
        let zooms: std::collections::BTreeSet<u32> = p.job.cover.iter().map(|r| r[0]).collect();
        assert!(zooms.contains(&0) && zooms.contains(&11), "{zooms:?}");
        assert!(p.job.layers.iter().any(|l| l.input == "osm:roads:11"));
        let rio = datars_geo::TileId::from_lonlat(datars_math::Vec2::new(-43.2, -22.97), 11);
        assert!(datars_geo_build::camera::covers(&p.job.cover, rio));
        // The flight in passes the zooms between.
        assert!((3..=9).all(|z| zooms.contains(&z)), "{zooms:?}");
        // The plan is the lockfile's: the same document plans the same job.
        assert!(ga::same_plan(&p.job, &plan(DOC, None).unwrap()[0].job));
    }

    /// Answers every Overpass query with one empty cell; counts requests. No network.
    struct EmptyCells(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl datars_geo_build::fetch::Http for EmptyCells {
        fn download(&self, url: &str, _: &Path) -> Result<(), String> {
            Err(format!("no downloads in tests: {url}"))
        }
        fn post_form(&self, _: &str, _: &str, _: &str) -> Result<(u16, Vec<u8>), String> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok((200, br#"{"osm3s":{"timestamp_osm_base":"2026-09-20T00:00:00Z"},"elements":[]}"#.to_vec()))
        }
    }

    #[test]
    fn archives_are_made_next_to_the_document_and_reused_while_current() {
        let dir = std::env::temp_dir().join(format!("datars-build-auto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (seed, root) = (dir.join("seed"), dir.join("cache"));
        std::fs::create_dir_all(&seed).unwrap();
        let empty = r#"{"type":"FeatureCollection","features":[]}"#;
        for l in ["ne_50m_land", "ne_10m_land", "ne_50m_lakes", "ne_10m_lakes", "ne_10m_rivers_lake_centerlines", "ne_50m_admin_0_boundary_lines_land", "ne_10m_admin_0_boundary_lines_land", "ne_10m_roads", "ne_10m_populated_places"] {
            std::fs::write(seed.join(format!("{l}.geojson")), empty).unwrap();
        }
        // No coastline polygons at zooms 7–9 in this document? It reaches them in flight: seed them.
        std::fs::create_dir_all(root.join("osmdata")).unwrap();
        std::fs::write(root.join(datars_geo_build::fetch::LAND_SIMPLIFIED), shp_empty()).unwrap();
        let posts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut c = Cache::new(root, Box::new(EmptyCells(posts.clone()))).with_mirrors(vec!["https://overpass.test".into()], std::time::Duration::ZERO).with_log(Box::new(|_| {}));
        c.seeds = vec![seed];
        let cache = Arc::new(c);
        let first = ensure(DOC, &dir, cache.clone(), &|_| {}).unwrap();
        assert_eq!(first.len(), 1);
        assert!(first[0].rebuilt && first[0].path == dir.join("base.auto.pmtiles") && first[0].path.is_file());
        assert!(job_path(&first[0].path).is_file());
        assert_eq!(first[0].job.data_version, "OpenStreetMap 2026-09-20T00:00:00Z");
        let fetched = posts.load(std::sync::atomic::Ordering::SeqCst);
        assert!(fetched > 0, "street cells fetched for the Rio state");
        // Current: used as it is, nothing fetched or built.
        let again = ensure(DOC, &dir, cache.clone(), &|_| {}).unwrap();
        assert!(!again[0].rebuilt);
        assert_eq!(posts.load(std::sync::atomic::Ordering::SeqCst), fetched);
        // Another camera: stale, rebuilt (from the cache where it can).
        let moved = DOC.replace("-43.3, -23.05, -43.1, -22.9", "-43.35, -23.05, -43.15, -22.9");
        assert!(ensure(&moved, &dir, cache, &|_| {}).unwrap()[0].rebuilt);
        // The engine reads the archive for `"auto"`: headless loads find it next to the document.
        let mut e = datars_headless::load_at(&moved, Some(&dir)).unwrap();
        let _ = e.scene_for_state(1);
        assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
        assert!(e.tile_stats().decoded > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A polygon shapefile with no records.
    fn shp_empty() -> Vec<u8> {
        let mut h = vec![0u8; 100];
        h[0..4].copy_from_slice(&9994i32.to_be_bytes());
        h[24..28].copy_from_slice(&50i32.to_be_bytes());
        h[28..32].copy_from_slice(&1000i32.to_le_bytes());
        h[32..36].copy_from_slice(&5i32.to_le_bytes());
        h
    }

    #[test]
    fn rewrites_point_sources_at_shipped_archives() {
        let out = rewrite(DOC, &BTreeMap::from([("base".to_string(), "../tiles/base.0123.pmtiles".to_string())])).unwrap();
        let doc = datars_ir::Doc::from_json(&out).unwrap();
        assert_eq!(doc.data["base"].from, datars_ir::SourceKind::Tiles("../tiles/base.0123.pmtiles".into()));
        assert_eq!(doc.data["other"].from, datars_ir::SourceKind::Tiles("fixed.pmtiles".into()));
        assert!(sources(&doc).is_empty());
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
    }
}
