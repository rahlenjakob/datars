//! `datars dev <doc.ts|doc.json>`: the edit loop in a browser. Rebuilds the document when it (or
//! a local recipe under `recipes/`) changes, and the page reloads itself; diagnostics, lint findings
//! and the program's states sit next to the chart. Std-only: a polling watcher and a tiny server.
//!
//! **Automatic basemaps** (`tiles: "auto"`, docs/09-geo.md) never make the author wait: a worker
//! thread plans the archive from the document's cameras, builds it at once from what the geodata
//! cache holds (Natural Earth shows immediately; street cells not fetched yet are left out), serves
//! it from memory under a versioned URL, then fetches the missing OpenStreetMap cells one by one and
//! rebuilds as they land — the page swaps each new archive in (`setTilesUrl`: the view keeps its
//! state and where it was explored to). Where a reader pans or zooms an explorable map, the page
//! reports the views (`/__views`) and the worker fetches data for them too. Publishing is
//! unaffected: `datars publish` plans from the document alone, from the same cache.

use datars_build::auto::{Cache, Need, Plan, Source};
use datars_engine::TileView;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime};

#[derive(Default)]
struct Build {
    version: u64,
    json: String,
    report: String,
    /// The document's fonts by the URL it names them with (project files, Google Fonts, font
    /// URLs, `datars:` defaults), acquired the way the build step does — the page's runtime
    /// fetches them from `/__font?src=…`, so a preview draws what a bundle ships.
    fonts: std::collections::BTreeMap<String, Vec<u8>>,
}

/// Archive versions kept per source: a page mid-way through reading one keeps getting its bytes.
const KEEP_VERSIONS: usize = 12;
/// Views reported by pages, kept (most recent last).
const KEEP_VIEWS: usize = 48;

/// One build of an archive: its version number and bytes.
type Version = (u64, Arc<Vec<u8>>);

/// The automatic basemaps of the current build, served from memory.
#[derive(Default)]
struct Basemaps {
    /// source → versions (oldest first).
    archives: BTreeMap<String, Vec<Version>>,
    /// Every publish of any archive is a generation: generation → source → URL. A page loads the
    /// document of one generation and swaps URLs from there (`/doc.json?g=N`).
    generations: BTreeMap<u64, BTreeMap<String, String>>,
    generation: u64,
    /// source → what the report says (cells cached, what's being fetched, errors).
    status: BTreeMap<String, serde_json::Value>,
}

impl Basemaps {
    fn url(source: &str, version: u64) -> String {
        format!("/__tiles/{source}/{version}.pmtiles")
    }

    fn publish(&mut self, source: &str, bytes: Vec<u8>) {
        let versions = self.archives.entry(source.to_string()).or_default();
        let v = versions.last().map_or(1, |l| l.0 + 1);
        versions.push((v, Arc::new(bytes)));
        if versions.len() > KEEP_VERSIONS {
            versions.remove(0);
        }
        let mut urls = self.generations.get(&self.generation).cloned().unwrap_or_default();
        urls.insert(source.to_string(), Basemaps::url(source, v));
        self.generation += 1;
        self.generations.insert(self.generation, urls);
        while self.generations.len() > KEEP_VERSIONS * 4 {
            let Some(&first) = self.generations.keys().next() else { break };
            self.generations.remove(&first);
        }
    }

    fn urls(&self, g: Option<u64>) -> BTreeMap<String, String> {
        g.and_then(|g| self.generations.get(&g)).or_else(|| self.generations.get(&self.generation)).cloned().unwrap_or_default()
    }

    fn bytes(&self, source: &str, version: u64) -> Option<Arc<Vec<u8>>> {
        self.archives.get(source)?.iter().find(|(v, _)| *v == version).map(|(_, b)| b.clone())
    }
}

/// Work for the basemap worker.
enum Job {
    /// A new build of the document (its JSON).
    Doc(String),
    /// Views a page reported (where a reader explored).
    Views(Vec<TileView>),
}

/// Files a document can use: its source, data and recipes. Anything else in the folder (a log
/// the server itself writes there, editor swap files, automatic basemaps `datars render` made)
/// must not trigger rebuilds.
fn watched(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    !name.ends_with(".auto.job.json") && matches!(p.extension().and_then(|e| e.to_str()), Some("ts" | "js" | "mjs" | "json" | "csv" | "tsv" | "geojson" | "topojson"))
}

/// Newest modification time under the document's directory (the doc, its data, `recipes/`).
fn stamp(doc: &Path) -> SystemTime {
    let dir = doc.parent().unwrap_or(Path::new("."));
    let mut newest = std::fs::metadata(doc).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut scan = |d: &Path| {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                if e.path().is_file() && watched(&e.path()) && m > newest {
                    newest = m;
                }
            }
        }
    };
    scan(dir);
    scan(&dir.join("recipes"));
    newest
}

type Fonts = std::collections::BTreeMap<String, Vec<u8>>;

/// A built document loaded into an engine the way `rebuild` loads it (fonts and files from the
/// document's folder and the font cache): what the Profile tab measures.
fn load(json: &str, doc: &Path) -> Result<datars_engine::Engine, String> {
    let acquire = datars_build::fonts::acquire::Fonts::from_env();
    let fetch = datars_build::fonts::acquire::fetch_with(&acquire, doc.parent());
    datars_headless::load_at_with(json, doc.parent(), &fetch)
}

/// The engine profile of build `version` at `dpr` — every state resolved cold and warm, every
/// transition stepped at 60 fps on a headless GPU (`datars profile --json`) — with how long it
/// took. Profiles run one at a time and the last is kept, so a page asking again gets it at once.
fn profile(sh: &Shared, doc: &Path, version: u64, json: &str, dpr: f64) -> String {
    let Ok(mut last) = sh.profile.lock() else { return String::new() };
    if let Some((v, d, out)) = last.as_ref() {
        if *v == version && *d == dpr {
            return out.clone();
        }
    }
    let t0 = Instant::now();
    let out = match load(json, doc) {
        Ok(mut e) => {
            let mut p = crate::perf::profile(&mut e, dpr, 60.0, false).to_json();
            p["version"] = version.into();
            p["ms"] = ms(t0).into();
            p.to_string()
        }
        Err(e) => serde_json::json!({ "error": e, "version": version }).to_string(),
    };
    *last = Some((version, dpr, out.clone()));
    out
}

fn rebuild(doc: &Path, read: &dyn Fn(&str) -> Result<String, String>) -> (String, String, Fonts) {
    let t0 = Instant::now();
    let path = doc.display().to_string();
    let json = match read(&path) {
        Ok(j) => j,
        Err(e) => return (String::new(), serde_json::json!({ "error": e, "path": path, "ms": ms(t0) }).to_string(), Fonts::new()),
    };
    let acquire = datars_build::fonts::acquire::Fonts::from_env();
    let fetch = datars_build::fonts::acquire::fetch_with(&acquire, doc.parent());
    let fonts = std::cell::RefCell::new(Fonts::new());
    let record = |r: &datars_engine::Request| {
        let bytes = fetch(r)?;
        if let datars_engine::Request::Source { name, url } = r {
            if name.starts_with("font:") || is_font_url(url) {
                fonts.borrow_mut().insert(url.clone(), bytes.clone());
            }
        }
        Some(bytes)
    };
    let report = match datars_headless::load_at_with(&json, doc.parent(), &record) {
        Ok(mut e) => {
            let states = e.state_names();
            let lint: Vec<serde_json::Value> = datars_devtools::lint(&mut e).into_iter().map(|f| serde_json::json!({ "severity": f.severity, "rule": f.rule, "message": f.message, "fix": f.fix })).collect();
            let _ = e.scene();
            // An automatic basemap is served by this server, not read from next to the document.
            let auto: Vec<String> = datars_build::auto::sources(e.doc()).iter().map(|s| format!("tiles `{s}`:")).collect();
            let diags: Vec<String> = e.diagnostics().iter().map(|d| d.message.clone()).filter(|m| !auto.iter().any(|a| m.starts_with(a.as_str()))).collect();
            let d = e.doc();
            // What the page's header and document panel show: the build, and the document at a glance.
            let sources: Vec<serde_json::Value> = d.data.iter().map(|(name, s)| serde_json::json!({ "name": name, "from": source_kind(&s.from), "key": s.key })).collect();
            serde_json::json!({ "states": states, "diagnostics": diags, "lint": lint, "path": path, "ms": ms(t0), "title": d.title, "size": [d.size.width, d.size.height], "sources": sources, "recipes": used_recipes(&json) })
        }
        Err(e) => serde_json::json!({ "error": e, "path": path, "ms": ms(t0) }),
    };
    (json, report.to_string(), fonts.into_inner())
}

/// Milliseconds since `t0`, to 0.1.
fn ms(t0: Instant) -> f64 {
    (t0.elapsed().as_secs_f64() * 10_000.0).round() / 10.0
}

/// Where a data source's rows come from, in a few words (`inline`, `data.csv`, `atlas countries`).
fn source_kind(from: &datars_ir::SourceKind) -> String {
    use datars_ir::SourceKind as K;
    let file = |u: &str| u.rsplit('/').next().unwrap_or(u).to_string();
    match from {
        K::Values(_) => "inline values".into(),
        K::Csv(_) => "inline CSV".into(),
        K::Url(u) => file(u),
        K::Slot(s) => format!("slot `{s}` (the app fills it)"),
        K::Geojson(_) => "inline GeoJSON".into(),
        K::Topojson(_) => "inline TopoJSON".into(),
        K::Atlas(a) => format!("atlas `{a}`"),
        K::Tiles(t) => format!("tiles {}", file(t)),
        K::Font(f) => format!("font {}", file(f)),
        K::Generate(g) => format!("{} generated rows", g.rows),
    }
}

/// The recipes a document uses, each once, in the order they first appear.
fn used_recipes(json: &str) -> Vec<String> {
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("kind").and_then(|k| k.as_str()) == Some("use") {
                    if let Some(r) = o.get("recipe").and_then(|r| r.as_str()) {
                        if !out.iter().any(|x| x == r) {
                            out.push(r.to_string());
                        }
                    }
                }
                o.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
        walk(v.get("scene").unwrap_or(&serde_json::Value::Null), &mut out);
    }
    out
}

/// A font the page's runtime can't fetch itself, or a font file by path.
fn is_font_url(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    u.starts_with("google:") || u.starts_with("datars:") || [".ttf", ".otf", ".woff", ".woff2", ".ttc"].iter().any(|e| u.split('?').next().unwrap_or("").ends_with(e))
}

/// `%XX` and `+` decoding for a query value.
fn unescape(q: &str) -> String {
    let b = q.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or(""), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The document as the page gets it: automatic sources pointing at this server's archives.
fn served_doc(json: &str, urls: &BTreeMap<String, String>) -> String {
    if urls.is_empty() {
        return json.to_string();
    }
    datars_build::auto::rewrite(json, urls).unwrap_or_else(|_| json.to_string())
}

/// Views a page reported, as the engine's type.
fn parse_views(body: &[u8]) -> Vec<TileView> {
    let v: serde_json::Value = serde_json::from_slice(body).unwrap_or_default();
    v.as_array().into_iter().flatten().filter_map(|t| {
        let b = t.get("bbox")?.as_array()?;
        let bbox = [b.first()?.as_f64()?, b.get(1)?.as_f64()?, b.get(2)?.as_f64()?, b.get(3)?.as_f64()?];
        let zoom = t.get("zoom")?.as_f64().filter(|z| z.is_finite() && (0.0..=22.0).contains(z))?;
        Some(TileView { source: t.get("source")?.as_str()?.to_string(), bbox, zoom, state: t.get("state").and_then(|s| s.as_str()).unwrap_or("").to_string(), explore: t.get("explore").and_then(|e| e.as_bool()).unwrap_or(false) })
    }).collect()
}

/// Keep what a page reported, without piling up near-duplicates (same place, same zoom).
fn remember(extra: &mut Vec<TileView>, views: Vec<TileView>) -> bool {
    let key = |v: &TileView| (v.source.clone(), (v.zoom * 2.0).round() as i64, (v.bbox[0] * 50.0).round() as i64, (v.bbox[1] * 50.0).round() as i64);
    let mut changed = false;
    for v in views {
        if !extra.iter().any(|e| key(e) == key(&v)) {
            extra.push(v);
            changed = true;
        }
    }
    if extra.len() > KEEP_VIEWS {
        let drop = extra.len() - KEEP_VIEWS;
        extra.drain(..drop);
    }
    changed
}

/// The basemap worker: plan, build from the cache now, fetch what's missing, rebuild as it lands.
fn worker(rx: Receiver<Job>, dir: PathBuf, maps: Arc<Mutex<Basemaps>>, ready: Arc<Condvar>) {
    let cache = Arc::new(Cache::from_env().with_log(Box::new(|_| {})));
    let mut src: Option<Source> = None;
    let (mut doc_json, mut extra, mut plans, mut todo): (String, Vec<TileView>, Vec<Plan>, Vec<Need>) = Default::default();
    let mut last_build = Instant::now();
    let mut fetched = 0usize;
    let set_status = |maps: &Arc<Mutex<Basemaps>>, plans: &[Plan], todo: &[Need], note: &str| {
        if let Ok(mut m) = maps.lock() {
            m.status.clear();
            for p in plans {
                let all = datars_build::auto::geo::needs(&p.job);
                let cells = all.iter().filter(|n| matches!(n, Need::Cell(..))).count();
                let missing = all.iter().filter(|n| matches!(n, Need::Cell(..)) && todo.contains(n)).count();
                m.status.insert(p.source.clone(), serde_json::json!({ "cells": cells, "missing": missing, "note": note }));
            }
        }
    };
    loop {
        let first = if todo.is_empty() { rx.recv().ok() } else { rx.try_recv().ok() };
        let mut replan = false;
        for job in first.into_iter().chain(rx.try_iter()) {
            match job {
                Job::Doc(j) => {
                    replan |= j != doc_json;
                    doc_json = j;
                }
                Job::Views(v) => replan |= remember(&mut extra, v),
            }
        }
        if replan && !doc_json.is_empty() {
            let Ok(doc) = datars_ir::Doc::from_json(&doc_json) else { continue };
            plans = match datars_headless::load_at(&doc_json, Some(&dir)) {
                Ok(mut e) => datars_build::auto::plan_views(&doc, &e.tile_views(datars_build::auto::FLIGHT_SAMPLES), &extra),
                Err(_) => Vec::new(),
            };
            let lang = plans.first().and_then(|p| p.lang.clone());
            match src.as_mut() {
                Some(s) => s.set_lang(lang),
                None => src = Some(Source::new(cache.clone(), lang, false)),
            }
            let mut missing: Vec<Need> = Vec::new();
            for p in &plans {
                for n in datars_build::auto::geo::missing(&cache, &datars_build::auto::geo::needs(&p.job)) {
                    if !missing.contains(&n) {
                        missing.push(n);
                    }
                }
            }
            todo = missing;
            fetched = 0;
            publish_all(&plans, src.as_ref(), &maps, &ready);
            last_build = Instant::now();
            set_status(&maps, &plans, &todo, if todo.is_empty() { "complete" } else { "fetching" });
        }
        let Some(need) = todo.first().cloned() else { continue };
        set_status(&maps, &plans, &todo, &format!("fetching {}", need.describe()));
        let r = datars_build::auto::geo::fetch(&cache, &need);
        todo.remove(0);
        match r {
            Ok(()) => fetched += 1,
            Err(e) => {
                eprintln!("datars dev: basemap: {e}");
                set_status(&maps, &plans, &todo, &format!("error: {e}"));
            }
        }
        // Rebuild once the batch is in, and meanwhile every few seconds so streets fill in as
        // they arrive (Natural Earth and land first: they come first in `todo`).
        if fetched > 0 && (todo.is_empty() || last_build.elapsed() > Duration::from_secs(3)) {
            publish_all(&plans, src.as_ref(), &maps, &ready);
            last_build = Instant::now();
            fetched = 0;
            if todo.is_empty() {
                set_status(&maps, &plans, &todo, "complete");
                println!("datars dev: basemap complete");
            }
        }
    }
}

/// Build every plan from what's cached and serve the results.
fn publish_all(plans: &[Plan], src: Option<&Source>, maps: &Arc<Mutex<Basemaps>>, ready: &Arc<Condvar>) {
    let Some(src) = src else { return };
    for p in plans {
        match datars_build::auto::geo::build(p.job.clone(), src, &|_, _, _| {}) {
            Ok(b) => {
                if let Ok(mut m) = maps.lock() {
                    m.publish(&p.source, b.bytes);
                }
            }
            Err(e) => eprintln!("datars dev: basemap `{}`: {e}", p.source),
        }
    }
    ready.notify_all();
}

const PAGE: &str = r##"<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>datars dev</title>
<link rel=icon href="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 28 28'%3E%3Crect x='3' y='14' width='5.5' height='11' rx='1.6' fill='%234f7cff'/%3E%3Crect x='11.25' y='8' width='5.5' height='17' rx='1.6' fill='%23f5b53d'/%3E%3Crect x='19.5' y='3' width='5.5' height='22' rx='1.6' fill='%23ff6b5b'/%3E%3C/svg%3E">
<style>
:root{--bg:#f4f5f8;--panel:#fff;--ink:#16181d;--ink-2:#5b6270;--rule:#e2e5ea;--chip:#eef0f4;--accent:#4f7cff;--ok:#1a9a52;--warn:#b7791f;--err:#d64545;color-scheme:light}
:root[data-theme=dark]{--bg:#0e1014;--panel:#171a20;--ink:#e8eaee;--ink-2:#98a1b0;--rule:#272b34;--chip:#222630;--accent:#7b9bff;--ok:#3ecf7e;--warn:#e0a84a;--err:#ff7070;color-scheme:dark}
*{box-sizing:border-box}
body{margin:0;font:14px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif;background:var(--bg);color:var(--ink);display:grid;grid-template:52px minmax(0,1fr)/minmax(0,1fr) 420px;height:100vh}
header{grid-column:1/-1;display:flex;align-items:center;gap:16px;padding:0 16px;background:var(--panel);border-bottom:1px solid var(--rule);min-width:0}
.brand{display:flex;align-items:center;gap:8px;font-weight:650;white-space:nowrap}.brand svg{width:22px;height:22px}
.file{display:flex;align-items:center;gap:10px;min-width:0;flex:1}.file code{font:13px ui-monospace,Menlo,monospace;color:var(--ink-2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.pill{display:inline-flex;align-items:center;gap:6px;padding:3px 10px;border-radius:99px;background:var(--chip);font-size:12.5px;white-space:nowrap;transition:background .3s}
.pill::before{content:"";width:8px;height:8px;border-radius:50%;background:var(--ok)}.pill.warn::before{background:var(--warn)}.pill.err::before{background:var(--err)}
.pill.fresh{background:color-mix(in srgb,var(--ok) 22%,var(--chip))}
.tools{display:flex;align-items:center;gap:10px}
.seg{display:inline-flex;border:1px solid var(--rule);border-radius:8px;overflow:hidden}
button{font:inherit;font-size:13px;color:var(--ink);background:var(--panel);border:0;padding:5px 10px;cursor:pointer}
.seg button+button{border-left:1px solid var(--rule)}
.seg button[aria-pressed=true],#stats[aria-pressed=true]{background:var(--chip);font-weight:600}
#stats{border:1px solid var(--rule);border-radius:8px}
main{overflow:auto;padding:28px 32px 40px;display:flex;flex-direction:column;align-items:center;gap:14px}
#col{width:100%;display:flex;flex-direction:column;gap:12px;transition:max-width .25s}
.meta{width:100%;display:flex;flex-wrap:wrap;justify-content:space-between;gap:2px 12px;color:var(--ink-2);font-size:13px}.meta b{color:var(--ink);font-weight:600}
.board{position:relative;width:100%;background:var(--panel);border:1px solid var(--rule);border-radius:14px;overflow:hidden;box-shadow:0 1px 2px rgba(0,0,0,.04),0 8px 24px rgba(0,0,0,.05)}
datars-view{display:block}
#outline{position:absolute;pointer-events:none;border:2px solid var(--accent);border-radius:3px;box-shadow:0 0 0 3px color-mix(in srgb,var(--accent) 25%,transparent)}
.banner{width:100%;padding:10px 14px;border-radius:10px;background:color-mix(in srgb,var(--err) 12%,var(--panel));border:1px solid color-mix(in srgb,var(--err) 40%,transparent);color:var(--err);font-size:13px}
.states{display:flex;flex-wrap:wrap;gap:6px;justify-content:center}
.states button{border:1px solid var(--rule);border-radius:99px;padding:4px 12px}.states button small{color:var(--ink-2);margin-right:6px}
.states button[aria-pressed=true]{background:var(--ink);color:var(--panel);border-color:var(--ink)}.states button[aria-pressed=true] small{color:inherit;opacity:.7}
.hint{margin:0;color:var(--ink-2);font-size:12.5px;text-align:center}
kbd{font:11.5px ui-monospace,Menlo,monospace;padding:1px 5px;border:1px solid var(--rule);border-bottom-width:2px;border-radius:4px;background:var(--panel)}
aside{background:var(--panel);border-left:1px solid var(--rule);display:flex;flex-direction:column;min-height:0}
.tabs{display:flex;gap:2px;padding:8px 10px 0;border-bottom:1px solid var(--rule)}
.tabs button{white-space:nowrap;padding:8px 10px;border-bottom:2px solid transparent;color:var(--ink-2);border-radius:6px 6px 0 0}
.tabs button[aria-selected=true]{color:var(--ink);border-bottom-color:var(--accent);font-weight:600}
.badge{display:inline-block;min-width:18px;margin-left:6px;padding:0 5px;border-radius:99px;background:var(--chip);font-size:11.5px;text-align:center}.badge.err{background:var(--err);color:#fff}.badge.warn{background:var(--warn);color:#fff}
.pane{padding:16px;overflow:auto;flex:1}.pane[hidden]{display:none}
.empty{color:var(--ink-2);text-align:center;padding:28px 12px}.empty b{display:block;color:var(--ink);font-size:15px;margin-bottom:4px}
.empty .ok{color:var(--ok);font-size:28px;line-height:1}
.item{padding:10px 12px;border:1px solid var(--rule);border-left:3px solid var(--warn);border-radius:8px;margin-bottom:8px}
.item.err{border-left-color:var(--err)}.item.info{border-left-color:var(--accent)}
.item .rule{font:12px ui-monospace,Menlo,monospace;color:var(--ink-2)}.item p{margin:4px 0 0}.item .fix{color:var(--ink-2);font-size:13px}
pre{margin:0;white-space:pre-wrap;font:12.5px/1.5 ui-monospace,Menlo,monospace}
h3{margin:0 0 4px;font-size:15px}h4{margin:18px 0 6px;font-size:11.5px;text-transform:uppercase;letter-spacing:.06em;color:var(--ink-2)}
.path{font:12px ui-monospace,Menlo,monospace;color:var(--ink-2);word-break:break-all}
table{width:100%;border-collapse:collapse;font-size:13px}td{padding:5px 6px;border-top:1px solid var(--rule);vertical-align:top}td:first-child{color:var(--ink-2);white-space:nowrap}
td code,.chips code{font:12px ui-monospace,Menlo,monospace}
.expr{padding:7px 0;border-top:1px solid var(--rule)}.expr div{display:flex;justify-content:space-between;gap:12px}.expr .f{color:var(--ink-2)}.expr .v{font-variant-numeric:tabular-nums;text-align:right;overflow-wrap:anywhere}.expr code{display:block;margin-top:2px;font:12px/1.45 ui-monospace,Menlo,monospace;overflow-wrap:anywhere}td.v{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
.chips{display:flex;flex-wrap:wrap;gap:5px}.chips code{padding:2px 7px;border-radius:6px;background:var(--chip)}
.cmd{margin-top:14px;padding:10px 12px;border-radius:8px;background:var(--chip)}.cmd small{display:block;color:var(--ink-2);margin-bottom:4px}
.nums{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px}.nums div{padding:8px 10px;border:1px solid var(--rule);border-radius:8px}.nums b{display:block;font-size:20px;font-weight:650;font-variant-numeric:tabular-nums;line-height:1.2}.nums span{font-size:11.5px;color:var(--ink-2);white-space:nowrap}
.nums b.ok{color:var(--ok)}.nums b.warn{color:var(--warn)}.nums b.err{color:var(--err)}
#pf-chart{display:block;width:100%;height:110px;margin-top:10px;border:1px solid var(--rule);border-radius:8px;background:var(--bg)}
.key{display:flex;flex-wrap:wrap;gap:4px 12px;margin-top:6px;font-size:12px;color:var(--ink-2)}.key i{display:inline-block;width:10px;height:10px;border-radius:2px;margin-right:4px;vertical-align:-1px}
.key .e{background:var(--accent)}.key .r{background:var(--raster)}.key .d{background:var(--err)}
:root{--raster:#f59e0b}:root[data-theme=dark]{--raster:#fbbf24}
.hintp{color:var(--ink-2);font-size:13px;margin:4px 0 0}
details.run{border:1px solid var(--rule);border-radius:8px;margin-bottom:6px}details.run summary{display:flex;align-items:center;gap:8px;padding:7px 10px;cursor:pointer;list-style:none;font-size:13px}details.run summary::-webkit-details-marker{display:none}
details.run summary b{font-weight:600;white-space:nowrap}details.run summary span.s{color:var(--ink-2);margin-left:auto;white-space:nowrap;font-variant-numeric:tabular-nums}
.dot{flex:none;width:8px;height:8px;border-radius:50%;background:var(--ok)}.dot.warn{background:var(--warn)}.dot.err{background:var(--err)}
details.run table{margin:0 10px 8px;width:calc(100% - 20px)}details.run td.v{white-space:normal;text-align:left}
.pf-actions{display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:8px;font-size:13px;color:var(--ink-2)}
button.primary{background:var(--ink);color:var(--panel);border-radius:8px;padding:5px 14px;font-weight:600}button.primary:disabled{opacity:.5;cursor:default}
.spark{display:block}.spark .b{fill:var(--accent)}.spark .x{fill:var(--err)}.spark line{stroke:var(--ink-2);stroke-dasharray:2 2;stroke-width:1}
td.flag,span.flag{color:var(--warn)}th{font-size:11.5px;font-weight:600;color:var(--ink-2);text-align:right;padding:4px 5px;white-space:nowrap}table.eng td{padding:5px 4px}table.eng td:first-child{white-space:normal}th:first-child{text-align:left}
label.overlay{display:block;margin-top:16px;font-size:13px;color:var(--ink-2)}
@media (max-width:900px){body{grid-template:auto auto auto/minmax(0,1fr);height:auto}header{flex-wrap:wrap;padding:10px 16px}aside{border-left:0;border-top:1px solid var(--rule)}main{padding:20px 16px}}
</style>
<script type=module src="/runtime/datars.js"></script>
<header>
  <div class=brand><svg viewBox="0 0 28 28" aria-hidden=true><rect x=3 y=14 width=5.5 height=11 rx=1.6 fill="#4f7cff"/><rect x=11.25 y=8 width=5.5 height=17 rx=1.6 fill="#f5b53d"/><rect x=19.5 y=3 width=5.5 height=22 rx=1.6 fill="#ff6b5b"/></svg>datars dev</div>
  <div class=file><code id=path></code><span id=build class=pill>building…</span></div>
  <div class=tools>
    <div class=seg id=widths role=group aria-label="Preview width"><button data-w=doc>Authored</button><button data-w=768>Tablet</button><button data-w=390>Phone</button><button data-w=fill>Fill</button></div>
    <div class=seg id=modes role=group aria-label="Theme mode"><button data-mode=light>Light</button><button data-mode=dark>Dark</button></div>
    <button id=stats aria-pressed=false title="Frame times, drops and the engine profile">Profile</button>
  </div>
</header>
<main>
  <div id=col>
    <div class=meta><span id=title></span><span id=dims></span></div>
    <div id=banner class=banner hidden></div>
    <div class=board id=board><div id=outline hidden></div></div>
  </div>
  <div class=states id=states role=group aria-label=States></div>
  <p class=hint><kbd>←</kbd> <kbd>→</kbd> step through states · click a mark to explain it · save the file and the chart morphs to the new version</p>
</main>
<aside>
  <div class=tabs role=tablist><button role=tab data-tab=problems aria-selected=true>Problems<span id=nprob class=badge>0</span></button><button role=tab data-tab=explain aria-selected=false>Explain</button><button role=tab data-tab=profile aria-selected=false>Profile<span id=nfps class=badge hidden></span></button><button role=tab data-tab=document aria-selected=false>Document</button></div>
  <div class=pane id=problems></div>
  <div class=pane id=explain hidden><div class=empty><b>Click any mark</b>See why it looks the way it does: the recipes that made it, its data row, and every expression's value.</div></div>
  <div class=pane id=profile hidden>
    <div class=nums><div><b id=pf-fps>idle</b><span>fps</span></div><div><b id=pf-eng>–</b><span>engine ms</span></div><div><b id=pf-ras>–</b><span>raster ms</span></div><div><b id=pf-draws>–</b><span>draws</span></div></div>
    <canvas id=pf-chart></canvas>
    <div class=key><span><i class=e></i>engine</span><span><i class=r></i>raster</span><span><i class=d></i>missed a refresh</span></div>
    <p class=hintp id=pf-budget></p><p class=hintp id=pf-gpu></p>
    <h4>Transitions you played</h4>
    <div id=pf-runs><p class=hintp>Step through the states: each transition is measured here in this browser — frames, drops, the stall before it moves, and its slowest frame.</p></div>
    <h4>Engine profile</h4>
    <div class=pf-actions><button id=pf-run class=primary>Profile</button><label><input type=checkbox id=pf-auto> again on every save</label><span id=pf-status></span></div>
    <div id=pf-engine><p class=hintp>Every state resolved cold and warm, and every transition stepped at 60 fps on a headless GPU, natively — the same numbers as <code>datars profile</code>.</p></div>
    <label class=overlay><input type=checkbox id=pf-overlay> Stats overlay on the chart (<kbd>Shift</kbd>+<kbd>D</kbd>)</label>
  </div>
  <div class=pane id=document hidden></div>
</aside>
<script type=module>
const el = (id) => document.getElementById(id);
const esc = (s) => String(s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const report = async () => (await fetch("/__report")).json();
const prefs = (() => { try { return JSON.parse(localStorage.getItem("datars-dev") ?? "{}"); } catch { return {}; } })();
const save = () => { try { localStorage.setItem("datars-dev", JSON.stringify(prefs)); } catch {} };
// The document is loaded at one generation of the automatic basemaps; newer archives are swapped in.
let r0 = await report();
let tiles = r0.tiles ?? {};
const view = document.createElement("datars-view");
view.setAttribute("doc", `/doc.json?g=${r0.generation ?? 0}`);
view.setAttribute("allow-script", "");
view.setAttribute("no-controls", "");
// Fonts the document names come from the dev server, as the build step acquired them.
view.setAttribute("font-server", "/__font");
el("board").prepend(view);

// ---- the header: width presets, theme mode, stats
let size = r0.size ?? [800, 480];
function setWidth(w) {
  prefs.width = w; save();
  el("col").style.maxWidth = w === "fill" ? "100%" : `${(w === "doc" ? size[0] : +w) + 2}px`;
  el("widths").querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", b.dataset.w === w));
  hideOutline();
}
function setMode(m) {
  prefs.mode = m; save();
  document.documentElement.dataset.theme = m;
  view.setAttribute("mode", m);
  el("modes").querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", b.dataset.mode === m));
}
el("widths").onclick = (e) => e.target.dataset?.w && setWidth(e.target.dataset.w);
el("modes").onclick = (e) => e.target.dataset?.mode && setMode(e.target.dataset.mode);
el("stats").onclick = () => tab(el("profile").hidden ? "profile" : "problems");
el("pf-overlay").onchange = (e) => view.showStats(e.target.checked);
setWidth(prefs.width ?? "doc");
setMode(prefs.mode ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"));

// ---- tabs
function tab(name) {
  document.querySelectorAll("[role=tab]").forEach((b) => b.setAttribute("aria-selected", b.dataset.tab === name));
  for (const p of ["problems", "explain", "profile", "document"]) el(p).hidden = p !== name;
  el("stats").setAttribute("aria-pressed", name === "profile");
  if (name === "profile") { live(); if (!engineProfile && !profiling) runProfile(); }
}
document.querySelector(".tabs").onclick = (e) => { const b = e.target.closest("[role=tab]"); if (b) tab(b.dataset.tab); };

// ---- states: pills under the chart, arrow keys
let states = [], current = 0;
function drawStates() {
  el("states").innerHTML = states.length > 1 ? states.map((s, i) => `<button data-i=${i} aria-pressed=${i === current}><small>${i}</small>${esc(s)}</button>`).join("") : "";
}
el("states").onclick = (e) => { const b = e.target.closest("button"); if (b) view.setAttribute("state", b.dataset.i); };
view.addEventListener("state", (e) => { current = e.detail?.index ?? current; drawStates(); hideOutline(); });
addEventListener("keydown", (e) => {
  if (e.target.closest?.("input,textarea") || e.metaKey || e.ctrlKey || e.altKey) return;
  if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
    const i = Math.max(0, Math.min(states.length - 1, current + (e.key === "ArrowRight" ? 1 : -1)));
    if (i !== current) view.setAttribute("state", i);
    e.preventDefault();
  }
});

// ---- explain: click a mark
function hideOutline() { el("outline").hidden = true; }
view.addEventListener("pick", (e) => {
  const hits = e.detail?.hits ?? [];
  const hit = hits.find((h) => h.role === "datum" || h.role === "region") ?? hits.find((h) => h.role && h.role !== "group") ?? hits[0];
  if (!hit) { hideOutline(); return; }
  const why = view.explain(hit.path)?.[0];
  if (!why) {
    // Say so rather than do nothing: a click that changes nothing reads as a broken tab.
    hideOutline();
    el("explain").innerHTML = `<h3>${esc(hit.label ?? hit.path.split("/").pop())}</h3><div class=path>${esc(hit.path)}</div>
<div class=empty><b>Nothing to explain for this ${esc(hit.kind ?? "element")}</b>It has no source in the document as resolved — a dot of a point pyramid (<code>lod</code>), or a baked scene.</div>`;
    tab("explain");
    return;
  }
  if (why.bounds) {
    const [x, y, w, h] = why.bounds, o = el("outline");
    Object.assign(o.style, { left: `${view.offsetLeft + x - 3}px`, top: `${view.offsetTop + y - 3}px`, width: `${w + 6}px`, height: `${h + 6}px` });
    o.hidden = false;
  }
  const key = hit.path.split("/").pop();
  const show = (v) => (typeof v === "number" ? String(+v.toFixed(3)) : typeof v === "string" ? v : JSON.stringify(v));
  const row = why.row ? `<h4>Data row</h4><p class=path>${esc(why.row.table)} · row ${why.row.index}</p><table>${Object.entries(why.row.fields).map(([k, v]) => `<tr><td>${esc(k)}</td><td class=v>${esc(show(v))}</td></tr>`).join("")}</table>` : "";
  const vals = (why.values ?? []).length ? `<h4>Expressions</h4>${why.values.map((v) => `<div class=expr><div><span class=f>${esc(v.field)}</span><span class=v>${esc(show(v.value))}</span></div><code>${esc(v.expr)}</code></div>`).join("")}` : "";
  el("explain").innerHTML = `<h3>${esc(hit.label ?? key)}</h3><div class=path>${esc(why.path)}</div>
<h4>Made by</h4><div class=chips>${why.recipes?.length ? why.recipes.map((r) => `<code>${esc(r)}</code>`).join("<span>→</span>") : "<span>the document itself</span>"}</div>
${row}${vals}
${why.bounds ? `<h4>Drawn</h4><p class=path>${esc(why.node)}</p>` : ""}
${why.intents?.length ? `<h4>Intents</h4><div class=chips>${why.intents.map((i) => `<code>${esc(i)}</code>`).join("")}</div>` : ""}
<div class=cmd><small>The same in a terminal</small><pre>datars explain ${esc(r0.path ?? "doc.ts")} --key '${esc(key)}' --state ${current}</pre></div>`;
  tab("explain");
});

// ---- profile: live frames, the transitions played here, the engine's own profile
const fmt = (v, d = 1) => (v == null ? "–" : v >= 100 ? String(Math.round(v)) : v.toFixed(d));
const pct = (xs, q) => { if (!xs.length) return 0; const s = [...xs].sort((a, b) => a - b); return s[Math.min(s.length - 1, Math.floor(q * s.length))]; };
const css = (n) => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
let liveOn = false;
function live() {
  if (liveOn) return;
  liveOn = true;
  const step = () => {
    if (el("profile").hidden) { liveOn = false; return; }
    drawLive(view.frameRecord?.());
    requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}
function drawLive(rec) {
  const c = el("pf-chart"), dpr = devicePixelRatio || 1;
  const w = c.clientWidth, h = c.clientHeight;
  if (c.width !== Math.round(w * dpr)) { c.width = Math.round(w * dpr); c.height = Math.round(h * dpr); }
  const g = c.getContext("2d");
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const frames = rec?.frames ?? [];
  // Scaled to the frames, so a light chart's are visible too; the 60 Hz line shows when in range.
  const peak = Math.max(0, ...frames.map((f) => f[0] + f[1]));
  const top = Math.min(50, Math.max(2, peak * 1.3));
  const y = (ms) => h - 4 - Math.min(ms, top) * ((h - 12) / top);
  const n = 180, bw = w / n;
  frames.forEach(([e, r, gap], k) => {
    const x = (n - frames.length + k) * bw;
    g.fillStyle = css("--accent"); g.fillRect(x, y(e), Math.max(1, bw - 1), h - 4 - y(e));
    g.fillStyle = css("--raster"); g.fillRect(x, y(e + r), Math.max(1, bw - 1), y(e) - y(e + r));
    if (gap > 25) { g.fillStyle = css("--err"); g.fillRect(x, 2, Math.max(1, bw - 1), 4); }
  });
  g.fillStyle = css("--ink-2"); g.font = "10.5px ui-monospace, Menlo, monospace";
  if (top >= 1000 / 60) { g.strokeStyle = css("--ink-2"); g.setLineDash([3, 3]); g.beginPath(); g.moveTo(0, y(1000 / 60)); g.lineTo(w, y(1000 / 60)); g.stroke(); g.setLineDash([]); g.fillText("16.7 ms", 4, y(1000 / 60) - 3); }
  else g.fillText(`${fmt(top)} ms`, 4, 12);
  const moving = frames.filter((f) => f[2] > 0).map((f) => f[0] + f[1]);
  el("pf-budget").textContent = moving.length ? `Frames use ${Math.max(1, Math.round((pct(moving, 0.95) / (1000 / 60)) * 100))}% of a 60 Hz frame at p95 (${fmt(pct(moving, 0.95))} ms of 16.7) — on this machine.` : "";
  // The current run's frames only (a run's first frame has no gap before it), once there are a few.
  let from = frames.length;
  while (from > 0 && frames[from - 1][2] > 0) from--;
  const gaps = frames.slice(Math.max(from, frames.length - 30)).map((f) => f[2]).filter((v) => v > 0);
  const fps = rec?.moving && gaps.length >= 4 ? Math.round(1000 / (gaps.reduce((a, b) => a + b, 0) / gaps.length)) : null;
  const last = frames[frames.length - 1];
  const f = el("pf-fps");
  f.textContent = fps ?? (rec?.moving ? "…" : "idle");
  f.className = fps == null ? "" : fps >= 55 ? "ok" : fps >= 40 ? "warn" : "err";
  el("pf-eng").textContent = last ? fmt(last[0]) : "–";
  el("pf-ras").textContent = last ? fmt(last[1]) : "–";
  const st = rec?.stats ?? {}, gpu = st.gpu;
  el("pf-draws").textContent = gpu ? gpu.draws : "–";
  const backend = gpu && ({ BrowserWebGpu: "WebGPU", Gl: "WebGL2", Metal: "Metal", Vulkan: "Vulkan", Dx12: "DirectX 12" }[gpu.backend] ?? gpu.backend);
  const num = (v) => Number(v ?? 0).toLocaleString("en");
  el("pf-gpu").textContent = gpu ? `${backend}, ${gpu.samples}× MSAA, ${gpu.size[0]}×${gpu.size[1]} px · ${num(gpu.meshes)} meshes kept (${num(Math.round(gpu.mesh_bytes / 1024))} KB) · ${num(gpu.tessellations)} tessellated · ${num(gpu.cache_hits)} cache hits` : st.renderer ? `Renderer: ${st.renderer} (the CPU reference)` : "";
}
const runs = [];
const verdict = (dropped, stall) => (dropped === 0 && (stall == null || stall <= 50) ? "" : dropped <= 2 && (stall == null || stall <= 120) ? "warn" : "err");
view.addEventListener("perf", (e) => {
  const s = e.detail;
  if (!s || s.label === "frame") return;
  runs.unshift(s);
  runs.length = Math.min(runs.length, 30);
  const row = (k, v) => `<tr><td>${k}</td><td class=v>${v}</td></tr>`;
  el("pf-runs").innerHTML = runs.map((s, i) => `<details class=run${i === 0 ? " open" : ""}><summary><span class="dot ${verdict(s.dropped, s.stall)}"></span><b>${esc(s.label)}</b><span class=s>${s.fps} fps · ${s.dropped} dropped${s.stall != null ? ` · stall ${s.stall} ms` : ""}</span></summary><table>
${row("frames", `${s.frames} over ${s.ms} ms · min ${s.minFps} fps (worst gap ${s.worst} ms)`)}
${s.stall != null ? row("stall", `${s.stall} ms: input ${s.input} + first frame ${s.first}`) : ""}
${row("engine", `p50 ${s.engine.p50} · p95 ${s.engine.p95} · max ${s.engine.max} ms`)}
${row("raster", `p50 ${s.raster.p50} · p95 ${s.raster.p95} · max ${s.raster.max} ms`)}
${row("between frames", `data max ${s.data.max} · page max ${s.page.max} ms`)}
${row("rebuilt / frame", `p50 ${s.rebuilt.p50.toLocaleString("en")} · max ${s.rebuilt.max.toLocaleString("en")} instances${s.rebuilt.p50 >= 20000 ? " <span class=flag>— heavy on phones</span>" : ""}`)}
${s.slowest ? row("slowest frame", `#${s.slowest.frame}: engine ${s.slowest.engine} + raster ${s.slowest.raster} ms · ${s.slowest.tessellated} meshes tessellated · ${s.slowest.rebuilt} rebuilt${s.slowest.pending ? ` · ${s.slowest.pending} tiles loading` : ""}`) : ""}
${s.pendingFrames ? row("streaming", `${s.pendingFrames} frames with tiles missing (≤ ${s.pendingMax})`) : ""}
</table></details>`).join("");
  const b = el("nfps");
  b.hidden = false; b.textContent = `${s.fps}`; b.className = `badge ${verdict(s.dropped, s.stall)}`;
});
let engineProfile = null, profiling = false, lastProfileMs = 0;
prefs.autoProfile ??= true;
el("pf-auto").checked = prefs.autoProfile;
el("pf-auto").onchange = (e) => { prefs.autoProfile = e.target.checked; save(); };
el("pf-run").onclick = () => runProfile();
function spark(frames) {
  const t = frames.map((f) => f.engine_ms + f.raster_ms), W = 80, H = 22, top = Math.max(20, ...t);
  const bw = W / Math.max(1, t.length);
  const y60 = (H - (1000 / 60 / top) * H).toFixed(1);
  return `<svg class="spark" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" aria-hidden="true">${t.map((v, i) => `<rect class="${v > 1000 / 60 ? "x" : "b"}" x="${(i * bw).toFixed(1)}" y="${(H - (v / top) * H).toFixed(1)}" width="${Math.max(0.6, bw - 0.4).toFixed(1)}" height="${((v / top) * H).toFixed(1)}"></rect>`).join("")}<line x1="0" x2="${W}" y1="${y60}" y2="${y60}"></line></svg>`;
}
async function runProfile() {
  if (profiling) return;
  profiling = true;
  el("pf-run").disabled = true;
  el("pf-status").textContent = "profiling…";
  try {
    const p = await (await fetch(`/__profile?dpr=${Math.min(3, Math.max(1, Math.round(devicePixelRatio || 2)))}`)).json();
    engineProfile = p;
    lastProfileMs = p.ms ?? 0;
    drawProfile();
  } catch (e) {
    el("pf-status").textContent = `failed: ${e}`;
  }
  profiling = false;
  el("pf-run").disabled = false;
}
function drawProfile() {
  const p = engineProfile;
  if (!p) return;
  if (p.error) { el("pf-engine").innerHTML = `<div class="item err"><pre>${esc(p.error)}</pre></div>`; el("pf-status").textContent = ""; return; }
  el("pf-status").textContent = `build ${p.version}${String(p.version) !== String(version) ? " (edited since)" : ""} · ${fmt(p.ms / 1000, 1)} s · ${p.raster} · dpr ${p.dpr}`;
  const states = `<table class=eng><tr><th>State</th><th>resolve cold</th><th>warm</th><th>nodes</th><th>ops</th></tr>${p.states.map((s) => `<tr><td>${esc(s.name)}</td><td class=v>${fmt(s.cold_ms, 2)} ms</td><td class=v>${fmt(s.warm_ms, 2)}</td><td class=v>${s.nodes}</td><td class=v>${s.ops}</td></tr>`).join("")}</table>`;
  const trs = p.transitions.map((t) => {
    const tot = t.frames.map((f) => f.engine_ms + f.raster_ms).slice(1), reb = pct(t.frames.slice(1).map((f) => f.instances_rebuilt), 0.5);
    const cls = verdict(t.dropped, t.stall_ms);
    return `<tr title="${t.frames.length} frames · engine p95 ${fmt(pct(t.frames.map((f) => f.engine_ms), 0.95), 2)} ms · raster p95 ${fmt(pct(t.frames.map((f) => f.raster_ms), 0.95), 2)} ms · ${pct(t.frames.map((f) => f.draws), 0.5)} draws"><td><span class="dot ${cls}" style="display:inline-block;margin-right:6px"></span>${esc(t.from)} → ${esc(t.to)}</td><td>${spark(t.frames)}</td><td class=v>${fmt(t.stall_ms)}</td><td class=v>${fmt(pct(tot, 0.95))}</td><td class="v${t.dropped ? " flag" : ""}">${t.dropped}</td><td class="v${reb >= 20000 ? " flag" : ""}">${reb >= 1000 ? `${Math.round(reb / 1000)}k` : reb}</td></tr>`;
  }).join("");
  el("pf-engine").innerHTML = `${states}<table class=eng style="margin-top:10px"><tr><th>Transition</th><th>frames</th><th>stall</th><th>p95</th><th>drops</th><th>rebuilt</th></tr>${trs}</table><p class=hintp>Times in ms. Native, on this machine's GPU: a browser is 1.5–3× slower, a phone more — so watch dropped frames and instances rebuilt every frame (20k+ is heavy on phones). Hover a row for engine and raster p95 and draws.</p>`;
}

// ---- the report: build status, problems, the document
let builtAt = Date.now(), fresh = 0;
function ago() {
  const s = Math.round((Date.now() - builtAt) / 1000);
  return s < 2 ? "just now" : s < 60 ? `${s} s ago` : `${Math.floor(s / 60)} min ago`;
}
function render(r, v) {
  const diags = r.diagnostics ?? [], lint = r.lint ?? [];
  const errs = diags.length + lint.filter((f) => f.severity === "error").length + (r.error ? 1 : 0);
  const warns = lint.length - lint.filter((f) => f.severity === "error").length;
  const b = el("build");
  b.className = `pill${errs ? " err" : warns ? " warn" : ""}${Date.now() - fresh < 1500 ? " fresh" : ""}`;
  b.textContent = r.error ? `build ${v} failed` : `build ${v} · ${r.ms ?? "?"} ms · ${ago()}`;
  el("path").textContent = r.path ?? "";
  el("banner").hidden = !r.error;
  if (r.error) el("banner").textContent = "This build failed — showing the last one that worked. Details under Problems.";
  if (r.size) size = r.size;
  el("title").innerHTML = r.title ? `<b>${esc(r.title)}</b>` : "";
  el("dims").textContent = r.size ? `${r.size[0]} × ${r.size[1]} authored · ${(r.states ?? []).length} state${(r.states ?? []).length === 1 ? "" : "s"}` : "";
  el("nprob").textContent = errs + warns;
  el("nprob").className = `badge${errs ? " err" : warns ? " warn" : ""}`;
  const items = [
    ...(r.error ? [`<div class="item err"><div class=rule>build</div><pre>${esc(r.error)}</pre></div>`] : []),
    ...diags.map((d) => `<div class="item err"><div class=rule>check</div><p>${esc(d)}</p></div>`),
    ...lint.map((f) => `<div class="item ${f.severity === "error" ? "err" : f.severity === "info" ? "info" : ""}"><div class=rule>lint · ${esc(f.rule)}</div><p>${esc(f.message)}</p>${f.fix ? `<p class=fix>Fix: ${esc(f.fix)}</p>` : ""}</div>`),
  ];
  el("problems").innerHTML = items.length ? items.join("") : `<div class=empty><div class=ok>✓</div><b>No problems</b>Check and lint are clean: data, encodings, legibility, accessibility and theme.</div>`;
  const maps = Object.entries(r.basemaps ?? {});
  el("document").innerHTML = `
<h4>Document</h4><table><tr><td>Title</td><td>${esc(r.title || "—")}</td></tr><tr><td>Size</td><td>${r.size ? `${r.size[0]} × ${r.size[1]}` : "—"}</td></tr><tr><td>States</td><td>${(r.states ?? []).map(esc).join(", ") || "—"}</td></tr></table>
<h4>Data</h4><table>${(r.sources ?? []).map((s) => `<tr><td><code>${esc(s.name)}</code></td><td>${esc(s.from)}${s.key?.length ? ` · key <code>${esc(s.key.join(", "))}</code>` : ""}</td></tr>`).join("") || "<tr><td>none</td></tr>"}</table>
<h4>Recipes</h4><div class=chips>${(r.recipes ?? []).map((x) => `<code>${esc(x.replace("@datars/std/", "std/"))}</code>`).join("") || "<span>none</span>"}</div>
${maps.length ? `<h4>Basemaps</h4><table>${maps.map(([s, m]) => `<tr><td><code>${esc(s)}</code></td><td>${m.cells - m.missing}/${m.cells} OSM cells · ${esc(m.note)}</td></tr>`).join("")}</table>` : ""}
<div class=cmd><small>Ship it</small><pre>datars publish ${esc(r.path ?? "doc.ts")} --alias my-chart --to site/</pre></div>`;
  states = r.states ?? [];
  drawStates();
}
setInterval(() => { const b = el("build"); if (b.textContent.includes(" · ")) b.textContent = b.textContent.replace(/ · [^·]+$/, ` · ${ago()}`); }, 1000);

// Where the views look (a reader panning or zooming, the page wider than authored): the server
// fetches map data for it. Posted whenever it changes, so it's where the view settled.
let seen = "";
const views = () => {
  const v = JSON.stringify(view.tileViews?.() ?? []);
  if (v !== seen && v !== "[]") { seen = v; fetch("/__views", { method: "POST", body: v }).catch(() => {}); }
};
let version = null;
async function poll() {
  try {
    views();
    view.frameRecord?.(); // starts the frame profiler once the chart runs: every transition is measured
    const v = await (await fetch("/__version")).text();
    const r = await report();
    // A new build morphs in from what is on screen; the page reloads only if the view can't. A
    // failed build keeps the last good one on screen.
    if (version !== null && v !== version) {
      builtAt = Date.now(); fresh = Date.now();
      hideOutline();
      if (!r.error && prefs.autoProfile && engineProfile && lastProfileMs < 5000) setTimeout(runProfile, 300);
      if (!r.error) {
        view.setAttribute("doc", `/doc.json?g=${r.generation ?? 0}`);
        tiles = r.tiles ?? {};
        if (await view.reload().catch(() => false)) console.log(`datars dev: build ${v} morphed in`);
        else location.reload();
      }
    } else {
      for (const [source, url] of Object.entries(r.tiles ?? {})) {
        if (tiles[source] === url) continue;
        tiles[source] = url;
        if (view.setTilesUrl) view.setTilesUrl(source, url);
        else { view.setAttribute("doc", `/doc.json?g=${r.generation ?? 0}`); await view.reload(); }
      }
    }
    version = v;
    render(r, v);
  } catch {}
  setTimeout(poll, 600);
}
poll();
</script>"##;

struct Shared {
    doc: PathBuf,
    /// The last engine profile: its build, its dpr, its JSON.
    profile: Mutex<Option<(u64, f64, String)>>,
    build: Arc<Mutex<Build>>,
    maps: Arc<Mutex<Basemaps>>,
    ready: Arc<Condvar>,
    jobs: Mutex<Sender<Job>>,
    runtime: PathBuf,
}

fn query_num(target: &str, key: &str) -> Option<u64> {
    target.split_once('?')?.1.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == key).and_then(|(_, v)| v.parse().ok())
}

fn respond(mut s: TcpStream, sh: &Shared) {
    let mut line = String::new();
    let mut r = BufReader::new(match s.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    if r.read_line(&mut line).is_err() {
        return;
    }
    let (mut range, mut length) = (None, 0usize);
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).map(|n| n == 0).unwrap_or(true) || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "range" => range = Some(v.trim().to_string()),
                "content-length" => length = v.trim().parse().unwrap_or(0),
                _ => {}
            }
        }
    }
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let path = target.split('?').next().unwrap_or("/").to_string();
    let query = target.split_once('?').map(|(_, q)| q.to_string()).unwrap_or_default();
    let b = sh.build.lock().map(|b| (b.version, b.json.clone(), b.report.clone())).unwrap_or_default();
    let mut content_range = None;
    let (status, ct, body): (&str, &str, Vec<u8>) = match path.as_str() {
        "/" => ("200 OK", "text/html; charset=utf-8", PAGE.as_bytes().to_vec()),
        "/doc.json" => {
            let urls = sh.maps.lock().map(|m| m.urls(query_num(&target, "g"))).unwrap_or_default();
            ("200 OK", "application/json", served_doc(&b.1, &urls).into_bytes())
        }
        "/__version" => ("200 OK", "text/plain", b.0.to_string().into_bytes()),
        "/__profile" => {
            let dpr = target.split_once('?').and_then(|(_, q)| q.split('&').find_map(|kv| kv.strip_prefix("dpr="))).and_then(|d| d.parse::<f64>().ok()).filter(|d| (0.5..=4.0).contains(d)).unwrap_or(2.0);
            ("200 OK", "application/json", profile(sh, &sh.doc, b.0, &b.1, dpr).into_bytes())
        }
        "/__report" => {
            let mut rep: serde_json::Value = serde_json::from_str(&b.2).unwrap_or_default();
            if let (Some(o), Ok(m)) = (rep.as_object_mut(), sh.maps.lock()) {
                o.insert("generation".into(), m.generation.into());
                o.insert("tiles".into(), serde_json::json!(m.urls(None)));
                o.insert("basemaps".into(), serde_json::json!(m.status));
            }
            ("200 OK", "application/json", rep.to_string().into_bytes())
        }
        "/__views" => {
            let mut body = vec![0u8; length.min(1 << 20)];
            if r.read_exact(&mut body).is_ok() {
                let views = parse_views(&body);
                if !views.is_empty() {
                    let _ = sh.jobs.lock().map(|j| j.send(Job::Views(views)));
                }
            }
            ("204 No Content", "text/plain", Vec::new())
        }
        p if p.starts_with("/__tiles/") => {
            // `/__tiles/<source>/<version>.pmtiles`, by range; the first archive may still be on its way.
            let rest = &p["/__tiles/".len()..];
            let parsed = rest.split_once('/').and_then(|(s, v)| Some((s.to_string(), v.strip_suffix(".pmtiles")?.parse::<u64>().ok()?)));
            let bytes = parsed.and_then(|(src, v)| {
                let deadline = Instant::now() + Duration::from_secs(120);
                let mut m = sh.maps.lock().ok()?;
                loop {
                    if let Some(b) = m.bytes(&src, v) {
                        return Some(b);
                    }
                    let newest = m.archives.get(&src).and_then(|a| a.last()).map_or(0, |l| l.0);
                    if newest >= v || Instant::now() > deadline {
                        return None;
                    }
                    m = sh.ready.wait_timeout(m, Duration::from_secs(5)).ok()?.0;
                }
            });
            match bytes {
                Some(b) => match range.as_deref().map(|h| crate::serve::byte_range(h, b.len())) {
                    Some(Some((start, end))) => {
                        content_range = Some(format!("bytes {start}-{}/{}", end - 1, b.len()));
                        ("206 Partial Content", "application/vnd.pmtiles", b[start..end].to_vec())
                    }
                    Some(None) => ("416 Range Not Satisfiable", "text/plain", Vec::new()),
                    None => ("200 OK", "application/vnd.pmtiles", b.to_vec()),
                },
                None => ("404 Not Found", "text/plain", b"no such archive".to_vec()),
            }
        }
        // Only the fonts the current document names, as the build acquired them.
        "/__font" => {
            let src = query.split('&').find_map(|kv| kv.strip_prefix("src=")).map(unescape).unwrap_or_default();
            match sh.build.lock().ok().and_then(|b| b.fonts.get(&src).cloned()) {
                Some(bytes) => ("200 OK", "font/ttf", bytes),
                None => ("404 Not Found", "text/plain", b"not a font this document names".to_vec()),
            }
        }
        p if p.starts_with("/runtime/") && !p.contains("..") => {
            let f = sh.runtime.join(&p["/runtime/".len()..]);
            let ct = if p.ends_with(".js") { "text/javascript" } else if p.ends_with(".wasm") { "application/wasm" } else { "application/octet-stream" };
            match std::fs::read(&f) {
                Ok(bytes) => ("200 OK", ct, bytes),
                Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
            }
        }
        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
    };
    let ranges = content_range.map(|c| format!("Content-Range: {c}\r\n")).unwrap_or_default();
    let head = format!("HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\n{ranges}Accept-Ranges: bytes\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len());
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(&body);
}

/// Watch `doc`, rebuild on change, serve the dev page on `addr`.
pub fn dev(doc: &Path, runtime: &Path, addr: &str, read: fn(&str) -> Result<String, String>) -> Result<(), String> {
    let build = Arc::new(Mutex::new(Build::default()));
    let maps = Arc::new(Mutex::new(Basemaps::default()));
    let ready = Arc::new(Condvar::new());
    let (tx, rx) = channel();
    let dir = doc.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new(".")).to_path_buf();
    {
        let (maps, ready) = (maps.clone(), ready.clone());
        std::thread::spawn(move || worker(rx, dir, maps, ready));
    }
    let (json, report, fonts) = rebuild(doc, &read);
    let _ = tx.send(Job::Doc(json.clone()));
    *build.lock().map_err(|e| e.to_string())? = Build { version: 1, json, report, fonts };
    let watcher = {
        let (build, doc, tx) = (build.clone(), doc.to_path_buf(), tx.clone());
        std::thread::spawn(move || {
            let mut last = stamp(&doc);
            loop {
                std::thread::sleep(Duration::from_millis(300));
                let now = stamp(&doc);
                if now != last {
                    last = now;
                    let (json, report, fonts) = rebuild(&doc, &read);
                    let _ = tx.send(Job::Doc(json.clone()));
                    if let Ok(mut b) = build.lock() {
                        b.version += 1;
                        // A failed build keeps the last good document on the page.
                        if !json.is_empty() {
                            b.json = json;
                            b.fonts = fonts;
                        }
                        b.report = report;
                        println!("rebuilt ({})", b.version);
                    }
                }
            }
        })
    };
    let l = TcpListener::bind(addr).map_err(|e| format!("{addr}: {e}"))?;
    println!("datars dev: http://{addr}/ — watching {}", doc.display());
    let shared = Arc::new(Shared { doc: doc.to_path_buf(), profile: Mutex::new(None), build, maps, ready, jobs: Mutex::new(tx), runtime: runtime.to_path_buf() });
    for s in l.incoming().flatten() {
        let sh = shared.clone();
        std::thread::spawn(move || respond(s, &sh));
    }
    drop(watcher);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_tab_measures_each_build_once() {
        let doc = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/votes/doc.json"));
        let json = std::fs::read_to_string(doc).unwrap();
        let (tx, _rx) = channel();
        let sh = Shared { doc: doc.to_path_buf(), profile: Mutex::new(None), build: Arc::default(), maps: Arc::default(), ready: Arc::default(), jobs: Mutex::new(tx), runtime: PathBuf::new() };
        let first = profile(&sh, doc, 1, &json, 1.0);
        let p: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(p["version"], 1);
        assert_eq!(p["states"].as_array().map(Vec::len), Some(4), "every state: {first:.300}");
        assert!(p["transitions"].as_array().is_some_and(|t| t.len() == 4 && t.iter().all(|t| !t["frames"].as_array().unwrap().is_empty())));
        assert_eq!(profile(&sh, doc, 1, &json, 1.0), first, "the same build again: the kept result");
        let next: serde_json::Value = serde_json::from_str(&profile(&sh, doc, 2, &json, 1.0)).unwrap();
        assert_eq!(next["version"], 2, "a new build is measured again");
    }

    #[test]
    fn only_document_inputs_trigger_rebuilds() {
        for f in ["doc.ts", "doc.json", "data.csv", "regions.geojson", "recipes/bar.ts"] {
            assert!(watched(Path::new(f)), "{f}");
        }
        for f in ["dev.log", ".doc.ts.swp", "out.png", "notes", "basemap.auto.job.json", "basemap.auto.pmtiles"] {
            assert!(!watched(Path::new(f)), "{f}");
        }
    }

    #[test]
    fn archives_are_served_by_version_and_generation() {
        let mut m = Basemaps::default();
        m.publish("base", vec![1, 2, 3]);
        m.publish("base", vec![4, 5]);
        assert_eq!(m.urls(None)["base"], "/__tiles/base/2.pmtiles");
        // A page that loaded generation 1 keeps reading version 1 until it swaps.
        assert_eq!(m.urls(Some(1))["base"], "/__tiles/base/1.pmtiles");
        assert_eq!(m.bytes("base", 1).as_deref(), Some(&vec![1, 2, 3]));
        for _ in 0..KEEP_VERSIONS {
            m.publish("base", vec![0]);
        }
        assert!(m.bytes("base", 1).is_none(), "old versions are let go");
        let doc = r#"{"data": {"base": {"tiles": "auto"}}}"#;
        assert!(served_doc(doc, &m.urls(None)).contains("/__tiles/base/"));
    }

    #[test]
    fn reported_views_are_kept_without_near_duplicates() {
        let body = br#"[{"source": "base", "bbox": [-43.2, -23.0, -43.1, -22.9], "zoom": 13.1, "state": "rio", "explore": true},
                        {"source": "base", "bbox": [-43.2, -23.0, -43.1, -22.9], "zoom": 13.12, "state": "rio", "explore": true},
                        {"source": "base", "bbox": [1, 2], "zoom": 3}]"#;
        let v = parse_views(body);
        assert_eq!(v.len(), 2, "the malformed one is dropped");
        let mut kept = Vec::new();
        assert!(remember(&mut kept, v));
        assert_eq!(kept.len(), 1);
        assert!(!remember(&mut kept, parse_views(body)), "nothing new");
    }

    /// The page's runtime asks the dev server for the fonts it can't fetch itself.
    #[test]
    fn fonts_are_served_by_the_url_the_document_names() {
        assert!(is_font_url("google:Source Serif 4:600") && is_font_url("../fonts/X.OTF") && is_font_url("datars:fonts/Inter-Regular.ttf"));
        assert!(!is_font_url("data.csv"));
        assert_eq!(unescape("google%3ASource+Serif+4%3A600"), "google:Source Serif 4:600");
        assert_eq!(unescape("..%2F..%2Fassets%2Ffonts%2FNewsreader-Regular.ttf"), "../../assets/fonts/Newsreader-Regular.ttf");
        assert_eq!(unescape("50%"), "50%");
        let doc = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/serif/doc.json"));
        let (_, report, fonts) = rebuild(doc, &|p| std::fs::read_to_string(p).map_err(|e| e.to_string()));
        assert!(report.contains("\"diagnostics\":[]"), "{report}");
        let names: Vec<&str> = fonts.keys().map(|k| k.as_str()).collect();
        assert_eq!(names, vec!["../../assets/fonts/Newsreader-Regular.ttf", "../../assets/fonts/Newsreader-SemiBold.ttf"]);
    }
}
