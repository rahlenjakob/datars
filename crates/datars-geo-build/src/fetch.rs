//! Build-time acquisition of open geodata, through a local cache (docs/09-geo.md): Natural
//! Earth layers, the osmdata simplified land polygons, and OpenStreetMap features per cell from
//! the Overpass API, with mirror failover and retries.
//!
//! Everything lands in one directory — `$DATARS_GEO_CACHE`, else `$XDG_CACHE_HOME/datars/geo`,
//! else `~/.cache/datars/geo`:
//!
//! ```text
//! ne/<layer>.geojson                          Natural Earth (downloaded once, or copied from a seed)
//! osmdata/simplified_land_polygons.shp        coastline land for zooms 7–9 (23 MB zip, once)
//! overpass/<key>.json.gz                      one Overpass answer per query (a cell at a detail level)
//! overpass/<key>.query                        the query that made it, for people
//! ```
//!
//! A query's key is a hash of its text, which names its cell, so a rebuild is offline and the
//! same bytes in give the same archive out. Nothing here runs at runtime: archives built from the
//! cache are static files the engine reads by range.
//!
//! **Seeds:** directories in `$DATARS_GEO_SEED` (`:`-separated) are looked in first for Natural
//! Earth files, so a machine that has them never downloads them again.
//!
//! **HTTP** goes through `curl` (on every macOS, Linux and Windows 10+): no TLS stack in the
//! workspace for a build-time download. [`Http`] is a trait so tests answer from recorded
//! fixtures, never the network. `$DATARS_GEO_OFFLINE=1` forbids the network outright.
//!
//! **Whose servers:** the public Overpass instances are shared and donated. They're asked one
//! request at a time, within a daily budget kept in the cache (`overpass/usage.json`,
//! `$DATARS_OVERPASS_BUDGET`); a product or a batch uses its own instance (`$DATARS_OVERPASS_URL`,
//! used alone) or a static server of pre-cut cells (`$DATARS_GEO_CELLS`) — see docs/09-geo.md.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Who we are to the servers we ask (Overpass etiquette: a real User-Agent with a contact).
pub const USER_AGENT: &str = concat!("datars-geo-build/", env!("CARGO_PKG_VERSION"), " (+https://github.com/rahlenjakob/datars)");

const NE_BASE: &str = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/master/geojson";
const LAND_SIMPLIFIED_URL: &str = "https://osmdata.openstreetmap.de/download/simplified-land-polygons-complete-3857.zip";
/// Relative to the cache root.
pub const LAND_SIMPLIFIED: &str = "osmdata/simplified_land_polygons.shp";

/// Overpass API endpoints, tried in order until one answers (`$DATARS_OVERPASS_URL` first).
pub const OVERPASS_MIRRORS: &[&str] = &[
    "https://overpass-api.de/api/interpreter",
    "https://maps.mail.ru/osm/tools/overpass/api/interpreter",
    "https://overpass.private.coffee/api/interpreter",
    "https://overpass.kumi.systems/api/interpreter",
];

/// Seconds an Overpass query may run on the server (`[timeout:…]`); curl waits a little longer.
/// A cell that takes longer than this is too big a cell, not a slow server to wait for.
pub const OVERPASS_TIMEOUT_S: u32 = 240;

/// Least time between two Overpass requests.
const OVERPASS_SPACING: Duration = Duration::from_millis(1100);

/// The daily budget on the public instances: half of what their usage policy calls harmless for
/// one user (10,000 queries, 1 GB), since other tools on the same machine share it. A dense city
/// at street level is a few hundred MB of answers, once: the budget is for authors trying things,
/// not for a product or a CI farm — those use an instance of their own (`$DATARS_OVERPASS_URL`,
/// or pre-cut cells, `$DATARS_GEO_CELLS`). `$DATARS_OVERPASS_BUDGET=<queries>,<megabytes>`.
fn public_budget() -> (u64, u64) {
    let env = std::env::var("DATARS_OVERPASS_BUDGET").ok();
    let mut parts = env.as_deref().unwrap_or("").split(',').map(|s| s.trim().parse::<u64>().ok());
    let q = parts.next().flatten().unwrap_or(5000);
    let mb = parts.next().flatten().unwrap_or(500);
    (q, mb << 20)
}

/// The network, as the cache needs it.
pub trait Http: Send + Sync {
    /// GET `url` into the file `out` (an error on any HTTP error status).
    fn download(&self, url: &str, out: &Path) -> Result<(), String>;
    /// POST one form field (`field=value`, URL-encoded) to `url`: the status and the body.
    fn post_form(&self, url: &str, field: &str, value: &str) -> Result<(u16, Vec<u8>), String>;
}

/// [`Http`] through the `curl` command.
pub struct Curl {
    /// Seconds a request may take before it's abandoned.
    pub timeout_s: u32,
}

impl Default for Curl {
    fn default() -> Curl {
        Curl { timeout_s: 600 }
    }
}

fn curl(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new("curl").args(args).output().map_err(|e| format!("running curl (needed to fetch map data): {e}"))?;
    if !out.status.success() {
        return Err(format!("curl: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

impl Http for Curl {
    fn download(&self, url: &str, out: &Path) -> Result<(), String> {
        let t = self.timeout_s.to_string();
        let path = out.to_string_lossy();
        curl(&["-fsSL", "--retry", "3", "--retry-delay", "5", "--connect-timeout", "30", "--max-time", &t, "-A", USER_AGENT, "-o", &path, url]).map(|_| ())
    }

    fn post_form(&self, url: &str, field: &str, value: &str) -> Result<(u16, Vec<u8>), String> {
        // The query goes through a file: it can be longer than a command line allows.
        let dir = std::env::temp_dir();
        let stamp = format!("{}-{:x}", std::process::id(), crate::fetch::key(value));
        let (q, body) = (dir.join(format!("datars-q-{stamp}.txt")), dir.join(format!("datars-r-{stamp}.json")));
        std::fs::write(&q, value).map_err(|e| e.to_string())?;
        // The server's own limit plus the transfer: a mirror that hangs past it is given up on.
        let t = (OVERPASS_TIMEOUT_S + 60).min(self.timeout_s).to_string();
        let form = format!("{field}@{}", q.display());
        // `--compressed`: Overpass answers gzip when asked (a tenth of the bytes over the wire).
        let r = curl(&["-sS", "--compressed", "--connect-timeout", "15", "--max-time", &t, "-A", USER_AGENT, "--data-urlencode", &form, "-o", &body.to_string_lossy(), "-w", "%{http_code}", url]);
        let _ = std::fs::remove_file(&q);
        let status = r.map(|s| String::from_utf8_lossy(&s).trim().parse::<u16>().unwrap_or(0));
        let bytes = std::fs::read(&body).unwrap_or_default();
        let _ = std::fs::remove_file(&body);
        Ok((status?, bytes))
    }
}

/// FNV-1a over a string: cache keys (stable across machines and versions, unlike std's hasher).
pub fn key(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The default cache directory (see the module docs).
pub fn default_root() -> PathBuf {
    if let Ok(d) = std::env::var("DATARS_GEO_CACHE") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    let base = std::env::var("XDG_CACHE_HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from).or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".cache"))).unwrap_or_else(std::env::temp_dir);
    base.join("datars").join("geo")
}

/// Seed directories from the environment (see the module docs).
pub fn env_seeds() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::env::var("DATARS_GEO_SEED").map(|s| std::env::split_paths(&s).collect()).unwrap_or_default();
    v.retain(|p| p.is_dir());
    v
}

/// The local store of build inputs, and how to fill it.
pub struct Cache {
    pub root: PathBuf,
    /// Looked in (read-only) before downloading Natural Earth.
    pub seeds: Vec<PathBuf>,
    /// Never touch the network: missing inputs are errors.
    pub offline: bool,
    /// A static server of pre-cut OSM cells to use instead of Overpass ([`Cache::osm_cell`]).
    pub cells_url: Option<String>,
    http: Box<dyn Http>,
    mirrors: Vec<String>,
    /// Queries and bytes a day on the public instances (`None`: an instance of your own).
    budget: Option<(u64, u64)>,
    ledger: Mutex<()>,
    last_overpass: Mutex<Option<Instant>>,
    /// Mirrors resting after refusing a request, until when.
    cooldown: Mutex<std::collections::BTreeMap<String, Instant>>,
    spacing: Duration,
    /// Retries per mirror round and the pause between rounds.
    rounds: usize,
    backoff: Duration,
    log: Box<dyn Fn(&str) + Send + Sync>,
}

impl Cache {
    /// The cache the environment names, fetching with `curl`, logging to stderr.
    pub fn from_env() -> Cache {
        let mut c = Cache::new(default_root(), Box::new(Curl::default()));
        c.seeds = env_seeds();
        c.offline = std::env::var("DATARS_GEO_OFFLINE").is_ok_and(|v| !v.is_empty() && v != "0");
        c.cells_url = std::env::var("DATARS_GEO_CELLS").ok().filter(|s| !s.is_empty());
        c
    }

    pub fn new(root: PathBuf, http: Box<dyn Http>) -> Cache {
        // An instance of your own (`$DATARS_OVERPASS_URL`) is used alone and without a budget;
        // otherwise the public ones, within the daily budget.
        let (mirrors, budget) = match std::env::var("DATARS_OVERPASS_URL").ok().filter(|s| !s.is_empty()) {
            Some(own) => (vec![own], None),
            None => (OVERPASS_MIRRORS.iter().map(|s| s.to_string()).collect(), Some(public_budget())),
        };
        Cache { root, seeds: Vec::new(), offline: false, cells_url: None, http, mirrors, budget, ledger: Mutex::new(()), last_overpass: Mutex::new(None), cooldown: Mutex::default(), spacing: OVERPASS_SPACING, rounds: 3, backoff: Duration::from_secs(20), log: Box::new(|m| eprintln!("{m}")) }
    }

    /// Where progress goes (stderr by default).
    pub fn with_log(mut self, log: Box<dyn Fn(&str) + Send + Sync>) -> Cache {
        self.log = log;
        self
    }

    /// Overpass endpoints to try, in order, and the pause between rounds of them (tests point at
    /// their fixtures and don't pause, between rounds or requests).
    pub fn with_mirrors(mut self, mirrors: Vec<String>, backoff: Duration) -> Cache {
        self.mirrors = mirrors;
        self.budget = None;
        self.spacing = backoff.min(OVERPASS_SPACING);
        self.backoff = backoff;
        self
    }

    fn say(&self, m: &str) {
        (self.log)(m)
    }

    /// A Natural Earth layer (`ne_10m_land` …) as GeoJSON: from the cache, a seed, or GitHub.
    pub fn natural_earth(&self, layer: &str) -> Result<PathBuf, String> {
        let file = format!("{layer}.geojson");
        let out = self.root.join("ne").join(&file);
        if present(&out) {
            return Ok(out);
        }
        if let Some(seed) = self.seeds.iter().map(|s| s.join(&file)).find(|p| present(p)) {
            // Seeding: the cache keeps its own copy (a hard link where it can), so it's complete
            // without the seed next time.
            create_parent(&out)?;
            let tmp = out.with_extension("part");
            let _ = std::fs::remove_file(&tmp);
            if std::fs::hard_link(&seed, &tmp).is_err() {
                std::fs::copy(&seed, &tmp).map_err(|e| format!("{}: {e}", seed.display()))?;
            }
            std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
            return Ok(out);
        }
        self.network(&format!("Natural Earth {layer}"))?;
        create_parent(&out)?;
        self.say(&format!("  fetching Natural Earth {layer} …"));
        let tmp = out.with_extension("part");
        self.http.download(&format!("{NE_BASE}/{file}"), &tmp)?;
        parse_check_geojson(&tmp).inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })?;
        std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
        Ok(out)
    }

    /// Whether a Natural Earth layer is at hand without the network.
    pub fn has_natural_earth(&self, layer: &str) -> bool {
        let file = format!("{layer}.geojson");
        present(&self.root.join("ne").join(&file)) || self.seeds.iter().any(|s| present(&s.join(&file)))
    }

    /// osmdata's simplified land polygons (Web Mercator shapefile, for zooms 7–9): downloaded and
    /// unpacked once.
    pub fn land_simplified(&self) -> Result<PathBuf, String> {
        let out = self.root.join(LAND_SIMPLIFIED);
        if present(&out) {
            return Ok(out);
        }
        self.network("the osmdata simplified land polygons")?;
        create_parent(&out)?;
        self.say("  fetching osmdata simplified land polygons (≈ 25 MB, once) …");
        let zip = out.with_extension("zip.part");
        self.http.download(LAND_SIMPLIFIED_URL, &zip)?;
        let r = crate::zip::extract_suffix(&zip, ".shp", &out.with_extension("shp.part"));
        let _ = std::fs::remove_file(&zip);
        r?;
        std::fs::rename(out.with_extension("shp.part"), &out).map_err(|e| e.to_string())?;
        Ok(out)
    }

    pub fn has_land_simplified(&self) -> bool {
        present(&self.root.join(LAND_SIMPLIFIED))
    }

    fn overpass_path(&self, query: &str) -> PathBuf {
        self.root.join("overpass").join(format!("{:016x}.json.gz", key(query)))
    }

    /// A cached Overpass answer, if there is one (no network).
    pub fn overpass_cached(&self, query: &str) -> Option<Vec<u8>> {
        let f = std::fs::File::open(self.overpass_path(query)).ok()?;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(f).read_to_end(&mut out).ok()?;
        Some(out)
    }

    pub fn has_overpass(&self, query: &str) -> bool {
        present(&self.overpass_path(query))
    }

    /// One OpenStreetMap cell (`query` is its Overpass query, and its cache key): from the cache,
    /// else from the static cell server `$DATARS_GEO_CELLS` names when set — a URL template with
    /// `{level}`, `{z}`, `{x}`, `{y}` serving the same answers pre-cut (from planet or regional
    /// extracts, on hardware of our own) — else from the public Overpass API. Whatever the source,
    /// the answer is checked the same way and cached under the same key.
    pub fn osm_cell(&self, query: &str, level: u8, z: u8, x: u32, y: u32, what: &str) -> Result<Vec<u8>, String> {
        if let Some(b) = self.overpass_cached(query) {
            return Ok(b);
        }
        let Some(template) = &self.cells_url else { return self.overpass(query, what) };
        self.network(what)?;
        let url = template.replace("{level}", &level.to_string()).replace("{z}", &z.to_string()).replace("{x}", &x.to_string()).replace("{y}", &y.to_string());
        self.say(&format!("  cell {what} ← {}", host(&url)));
        let tmp = self.overpass_path(query).with_extension("download");
        create_parent(&tmp)?;
        let got = self.http.download(&url, &tmp).and_then(|_| std::fs::read(&tmp).map_err(|e| e.to_string()));
        let _ = std::fs::remove_file(&tmp);
        let mut body = got?;
        // A static server may store cells gzipped.
        if body.starts_with(&[0x1f, 0x8b]) {
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(&body[..]).read_to_end(&mut out).map_err(|e| format!("{url}: {e}"))?;
            body = out;
        }
        check_overpass(200, &body).map_err(|e| format!("{url}: {e}"))?;
        self.store_overpass(query, &body)?;
        Ok(body)
    }

    /// An Overpass answer: from the cache, else asked of each mirror in turn (a few rounds, with
    /// a pause between), spaced out politely, checked to be a complete result before it's kept.
    pub fn overpass(&self, query: &str, what: &str) -> Result<Vec<u8>, String> {
        if let Some(b) = self.overpass_cached(query) {
            return Ok(b);
        }
        self.network(what)?;
        let mut last = String::from("no mirrors");
        for round in 1..=self.rounds {
            for m in &self.mirrors {
                // A mirror that just said "too many requests" or "busy" rests a while (Overpass
                // frees a slot some time after each query, longer after long ones).
                let (rest, others_free) = {
                    let c = self.cooldown.lock().unwrap_or_else(|e| e.into_inner());
                    let now = Instant::now();
                    (c.get(m).copied().filter(|t| *t > now), self.mirrors.iter().any(|o| o != m && c.get(o).is_none_or(|t| *t <= now)))
                };
                if let Some(t) = rest {
                    if others_free {
                        continue;
                    }
                    std::thread::sleep(t.saturating_duration_since(Instant::now()));
                }
                if self.budget.is_some() {
                    self.within_budget()?;
                }
                self.pace();
                self.say(&format!("  overpass {what} ← {}", host(m)));
                let answer = self.http.post_form(m, "data", query);
                if let Ok((_, body)) = &answer {
                    self.spend(body.len() as u64);
                }
                match answer.and_then(|(status, body)| check_overpass(status, &body).map(|_| body)) {
                    Ok(body) => {
                        self.store_overpass(query, &body)?;
                        return Ok(body);
                    }
                    Err(e) => {
                        self.say(&format!("    {}: {e}", host(m)));
                        // The usage policy: after a 429 or 406, pause at least 30 s.
                        let pause = if e.contains("429") || e.contains("406") { (self.backoff * 2).max(if self.backoff.is_zero() { Duration::ZERO } else { Duration::from_secs(30) }) } else { self.backoff / 2 };
                        if let Ok(mut c) = self.cooldown.lock() {
                            c.insert(m.clone(), Instant::now() + pause);
                        }
                        last = format!("{}: {e}", host(m));
                    }
                }
            }
            if round < self.rounds {
                self.say(&format!("    every mirror failed (round {round}/{}); waiting {} s", self.rounds, self.backoff.as_secs()));
                std::thread::sleep(self.backoff);
            }
        }
        Err(format!("Overpass: {what}: {last}"))
    }

    /// Today's use of the public instances (`overpass/usage.json`: the UTC day, queries, bytes).
    fn usage(&self) -> (u64, u64, u64) {
        let day = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0);
        let v: serde_json::Value = std::fs::read(self.root.join("overpass/usage.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        if v["day"].as_u64() != Some(day) {
            return (day, 0, 0);
        }
        (day, v["queries"].as_u64().unwrap_or(0), v["bytes"].as_u64().unwrap_or(0))
    }

    /// Refuse a request past the daily budget for the public instances.
    fn within_budget(&self) -> Result<(), String> {
        let Some((max_q, max_b)) = self.budget else { return Ok(()) };
        let (_, q, b) = self.usage();
        if q >= max_q || b >= max_b {
            return Err(format!(
                "today's budget for the public Overpass instances is spent ({q} queries, {} MB; the budget is {max_q} and {} MB, $DATARS_OVERPASS_BUDGET): \
                 they are shared, donated servers (policy: under 10,000 queries and 1 GB a day, commercial use on your own or a paid instance). \
                 Continue tomorrow, or point $DATARS_OVERPASS_URL at an instance of your own, or $DATARS_GEO_CELLS at a cell server",
                b >> 20,
                max_b >> 20
            ));
        }
        Ok(())
    }

    /// A daily budget for the public instances (tests set their own).
    pub fn with_budget(mut self, queries: u64, bytes: u64) -> Cache {
        self.budget = Some((queries, bytes));
        self
    }

    fn spend(&self, bytes: u64) {
        if self.budget.is_none() {
            return;
        }
        let _one = self.ledger.lock();
        let (day, q, b) = self.usage();
        let path = self.root.join("overpass/usage.json");
        let _ = create_parent(&path);
        let _ = std::fs::write(&path, serde_json::json!({ "day": day, "queries": q + 1, "bytes": b + bytes }).to_string());
    }

    fn store_overpass(&self, query: &str, body: &[u8]) -> Result<(), String> {
        let out = self.overpass_path(query);
        create_parent(&out)?;
        let tmp = out.with_extension("part");
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(body).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, enc.finish().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        std::fs::write(out.with_extension("").with_extension("query"), query).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &out).map_err(|e| e.to_string())
    }

    /// Wait until the last Overpass request is far enough in the past.
    fn pace(&self) {
        let mut last = self.last_overpass.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *last {
            let since = t.elapsed();
            if since < self.spacing {
                std::thread::sleep(self.spacing - since);
            }
        }
        *last = Some(Instant::now());
    }

    fn network(&self, what: &str) -> Result<(), String> {
        if self.offline {
            return Err(format!("{what} is not in the cache ({}) and the network is off (DATARS_GEO_OFFLINE)", self.root.display()));
        }
        Ok(())
    }
}

fn host(url: &str) -> &str {
    url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url)
}

/// A usable Overpass answer: HTTP 200, JSON with `elements`, and no `remark` saying the query ran
/// out of time or memory (Overpass still answers 200 then — with a partial result, which must
/// never be cached as if it were the whole cell).
pub fn check_overpass(status: u16, body: &[u8]) -> Result<(), String> {
    if status != 200 {
        let hint = match status {
            429 => " (too many requests)",
            504 => " (server busy)",
            _ => "",
        };
        return Err(format!("HTTP {status}{hint}"));
    }
    let v: serde_json::Value = serde_json::from_slice(body).map_err(|_| "not an Overpass JSON answer".to_string())?;
    if !v.get("elements").is_some_and(|e| e.is_array()) {
        return Err("no `elements` in the answer".into());
    }
    if let Some(r) = v.get("remark").and_then(|r| r.as_str()) {
        if r.contains("error") || r.contains("timed out") || r.contains("out of memory") {
            return Err(format!("incomplete answer: {r}"));
        }
    }
    Ok(())
}

/// When the OSM data behind an Overpass answer was current (`osm3s.timestamp_osm_base`).
pub fn overpass_timestamp(body: &[u8]) -> Option<String> {
    // Only the head is needed: the timestamp comes before the elements.
    let head = &body[..body.len().min(2048)];
    let s = String::from_utf8_lossy(head);
    let i = s.find("\"timestamp_osm_base\"")?;
    let rest = &s[i + 20..];
    let a = rest.find('"')? + 1;
    let b = rest[a..].find('"')?;
    Some(rest[a..a + b].to_string())
}

fn present(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.is_file() && m.len() > 0).unwrap_or(false)
}

fn create_parent(p: &Path) -> Result<(), String> {
    match p.parent() {
        Some(d) => std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display())),
        None => Ok(()),
    }
}

/// A downloaded file that should be GeoJSON: is it (GitHub answers some errors with 200 pages)?
fn parse_check_geojson(p: &Path) -> Result<(), String> {
    let mut head = [0u8; 256];
    let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut head)).map_err(|e| e.to_string())?;
    let s = String::from_utf8_lossy(&head[..n]);
    if s.contains("FeatureCollection") || s.trim_start().starts_with('{') {
        Ok(())
    } else {
        Err(format!("{}: not GeoJSON", p.display()))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    /// Answers from memory: URL (or `mirror#query-key`) → (status, body); counts every call.
    #[derive(Default, Clone)]
    pub(crate) struct Recorded {
        pub files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
        pub posts: Arc<Mutex<BTreeMap<String, Vec<(u16, Vec<u8>)>>>>,
        pub calls: Arc<Mutex<Vec<String>>>,
    }

    impl Http for Recorded {
        fn download(&self, url: &str, out: &Path) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("GET {url}"));
            let b = self.files.lock().unwrap().get(url).cloned().ok_or(format!("404 {url}"))?;
            std::fs::write(out, b).map_err(|e| e.to_string())
        }
        fn post_form(&self, url: &str, _field: &str, _value: &str) -> Result<(u16, Vec<u8>), String> {
            self.calls.lock().unwrap().push(format!("POST {url}"));
            let mut p = self.posts.lock().unwrap();
            let q = p.get_mut(url).ok_or(format!("connection refused: {url}"))?;
            if q.len() > 1 {
                Ok(q.remove(0))
            } else {
                q.first().cloned().ok_or("empty".into())
            }
        }
    }

    pub(crate) fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("datars-geo-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const GOOD: &str = r#"{"version":0.6,"osm3s":{"timestamp_osm_base":"2026-09-20T10:00:00Z"},"elements":[{"type":"node","id":1,"lat":-22.97,"lon":-43.18,"tags":{"place":"suburb","name":"Copacabana"}}]}"#;

    #[test]
    fn keys_are_stable_hashes_of_the_query() {
        assert_eq!(key(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(key("a"), 0xaf63_dc4c_8601_ec8c, "FNV-1a, the same everywhere");
        assert_ne!(key("[bbox:1,2,3,4]"), key("[bbox:1,2,3,5]"));
    }

    #[test]
    fn overpass_answers_are_checked_before_they_are_kept() {
        assert!(check_overpass(200, GOOD.as_bytes()).is_ok());
        assert!(check_overpass(429, GOOD.as_bytes()).unwrap_err().contains("too many"));
        assert!(check_overpass(200, b"<html>busy</html>").is_err());
        let partial = r#"{"elements":[],"remark":"runtime error: Query timed out in \"query\" at line 3 after 181 seconds."}"#;
        assert!(check_overpass(200, partial.as_bytes()).unwrap_err().contains("incomplete"));
        assert_eq!(overpass_timestamp(GOOD.as_bytes()).as_deref(), Some("2026-09-20T10:00:00Z"));
    }

    #[test]
    fn overpass_fails_over_to_the_next_mirror_and_caches() {
        let http = Recorded::default();
        http.posts.lock().unwrap().insert("https://a/api".into(), vec![(504, b"busy".to_vec())]);
        http.posts.lock().unwrap().insert("https://b/api".into(), vec![(200, GOOD.as_bytes().to_vec())]);
        let root = temp("failover");
        let c = Cache::new(root.clone(), Box::new(http.clone())).with_mirrors(vec!["https://a/api".into(), "https://b/api".into()], Duration::ZERO).with_log(Box::new(|_| {}));
        let q = "[out:json];node(1);out;";
        assert_eq!(c.overpass(q, "test").unwrap(), GOOD.as_bytes());
        assert_eq!(*http.calls.lock().unwrap(), vec!["POST https://a/api", "POST https://b/api"]);
        // Cached (gzipped, with its query beside it): asked again, no request.
        assert!(c.has_overpass(q));
        assert_eq!(c.overpass(q, "test").unwrap(), GOOD.as_bytes());
        assert_eq!(http.calls.lock().unwrap().len(), 2);
        assert!(std::fs::read_dir(root.join("overpass")).unwrap().any(|e| e.unwrap().path().extension().is_some_and(|x| x == "query")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn cells_come_from_a_static_server_of_our_own_when_one_is_named() {
        let http = Recorded::default();
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(GOOD.as_bytes()).unwrap();
        http.files.lock().unwrap().insert("https://cells.test/12/12/1555/2316.json.gz".into(), gz.finish().unwrap());
        let root = temp("cells");
        let mut c = Cache::new(root.clone(), Box::new(http.clone())).with_mirrors(vec!["https://overpass.test".into()], Duration::ZERO).with_log(Box::new(|_| {}));
        c.cells_url = Some("https://cells.test/{level}/{z}/{x}/{y}.json.gz".into());
        assert_eq!(c.osm_cell("q12", 12, 12, 1555, 2316, "a cell").unwrap(), GOOD.as_bytes(), "gunzipped, checked");
        assert_eq!(*http.calls.lock().unwrap(), vec!["GET https://cells.test/12/12/1555/2316.json.gz"], "Overpass never asked");
        // Cached under the query like any answer: the source doesn't change the cache.
        assert_eq!(c.overpass_cached("q12").as_deref(), Some(GOOD.as_bytes()));
        assert!(c.osm_cell("q13", 13, 13, 1, 2, "a missing cell").is_err(), "a server without it is an error, not a fallback to Overpass");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_public_instances_get_a_daily_budget() {
        let http = Recorded::default();
        http.posts.lock().unwrap().insert("https://a/api".into(), vec![(200, GOOD.as_bytes().to_vec())]);
        let root = temp("budget");
        let c = Cache::new(root.clone(), Box::new(http.clone())).with_mirrors(vec!["https://a/api".into()], Duration::ZERO).with_log(Box::new(|_| {})).with_budget(2, 1 << 20);
        assert!(c.overpass("q1", "one").is_ok() && c.overpass("q2", "two").is_ok());
        // Counted in the cache (so every tool on the machine shares it), and then refused.
        let usage: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join("overpass/usage.json")).unwrap()).unwrap();
        assert_eq!(usage["queries"], 2);
        assert_eq!(usage["bytes"], 2 * GOOD.len() as u64);
        let e = c.overpass("q3", "three").unwrap_err();
        assert!(e.contains("budget") && e.contains("DATARS_OVERPASS_URL"), "{e}");
        assert_eq!(http.calls.lock().unwrap().len(), 2);
        // Cached answers cost nothing.
        assert!(c.overpass("q1", "one again").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn partial_answers_are_retried_not_cached() {
        let http = Recorded::default();
        let partial = r#"{"elements":[],"remark":"runtime error: out of memory"}"#;
        http.posts.lock().unwrap().insert("https://a/api".into(), vec![(200, partial.as_bytes().to_vec()), (200, GOOD.as_bytes().to_vec())]);
        let root = temp("partial");
        let c = Cache::new(root.clone(), Box::new(http.clone())).with_mirrors(vec!["https://a/api".into()], Duration::ZERO).with_log(Box::new(|_| {}));
        assert_eq!(c.overpass("q", "test").unwrap(), GOOD.as_bytes(), "the second round's answer");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn offline_caches_never_ask_the_network() {
        let http = Recorded::default();
        let root = temp("offline");
        let mut c = Cache::new(root.clone(), Box::new(http.clone())).with_log(Box::new(|_| {}));
        c.offline = true;
        assert!(c.overpass("q", "a cell").unwrap_err().contains("network is off"));
        assert!(c.natural_earth("ne_50m_land").is_err());
        assert!(http.calls.lock().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn natural_earth_comes_from_a_seed_before_the_network() {
        let http = Recorded::default();
        let (root, seed) = (temp("ne-root"), temp("ne-seed"));
        std::fs::write(seed.join("ne_50m_land.geojson"), r#"{"type":"FeatureCollection","features":[]}"#).unwrap();
        http.files.lock().unwrap().insert(format!("{NE_BASE}/ne_10m_land.geojson"), br#"{"type":"FeatureCollection","features":[]}"#.to_vec());
        let mut c = Cache::new(root.clone(), Box::new(http.clone())).with_log(Box::new(|_| {}));
        c.seeds = vec![seed.clone()];
        assert_eq!(c.natural_earth("ne_50m_land").unwrap(), root.join("ne/ne_50m_land.geojson"), "copied into the cache");
        c.seeds.clear();
        assert!(c.has_natural_earth("ne_50m_land"), "complete without the seed next time");
        assert!(http.calls.lock().unwrap().is_empty(), "seeded");
        assert_eq!(c.natural_earth("ne_10m_land").unwrap(), root.join("ne/ne_10m_land.geojson"));
        assert_eq!(http.calls.lock().unwrap().len(), 1);
        assert!(c.natural_earth("ne_10m_land").is_ok());
        assert_eq!(http.calls.lock().unwrap().len(), 1, "downloaded once");
        let _ = (std::fs::remove_dir_all(&root), std::fs::remove_dir_all(&seed));
    }
}
