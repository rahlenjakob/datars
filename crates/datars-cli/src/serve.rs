//! `datars publish --to <dir>` and `datars serve`: self-hosted delivery with no account and no
//! third-party service (docs/12-delivery.md). The layout is the one a managed publishing service
//! serves too (docs/16-licensing.md), so a host can point at either:
//!
//! - `<dir>/c/<alias>` — the live manifest (republishing rewrites it; embeds follow it)
//! - `<dir>/chunks/<hash>` — content-addressed chunks (`:` in hashes written as `_`), immutable
//!
//! Any static host serves this. `datars serve` is a small development server for it, plus the web
//! runtime under `/runtime/` and an index page embedding every alias. It answers HTTP `Range`
//! requests (tile archives are read that way, like from any CDN or bucket), and `/view/<doc.json>`
//! opens a document straight from the served directory (`datars serve public` →
//! `/view/examples/descent/doc.json`).
//!
//! **Replay directories** simulate live feeds: a request for `x.json` next to a folder `x.json.d/`
//! serves that folder's files in name order, one every `DATARS_REPLAY_PERIOD` seconds (default 2)
//! since the server started, then stays on the last. The folder wins over an `x.json` file (the
//! snapshot `datars publish` copies next to the chart).

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};

/// Write a bundle into a static delivery directory under `alias`. Returns (chunks written, chunks
/// already present).
pub fn publish(bundle: &datars_bundle::Bundle, dir: &Path, alias: &str) -> Result<(usize, usize), String> {
    if alias.is_empty() || !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("alias `{alias}`: use letters, digits, - and _"));
    }
    std::fs::create_dir_all(dir.join("chunks")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir.join("c")).map_err(|e| e.to_string())?;
    let (mut written, mut present) = (0, 0);
    for (hash, bytes) in &bundle.chunks {
        let p = dir.join("chunks").join(hash.replacen(':', "_", 1));
        if p.exists() {
            present += 1;
        } else {
            std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
            written += 1;
        }
    }
    // The manifest last: readers never see a manifest whose chunks aren't there yet.
    let tmp = dir.join("c").join(format!(".{alias}.tmp"));
    std::fs::write(&tmp, bundle.manifest.to_json()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("c").join(alias)).map_err(|e| e.to_string())?;
    Ok((written, present))
}

/// Copy the files a document references by relative URL but doesn't carry in its bundle — tile
/// archives (fetched by range) and URL sources (live refreshes) — to where a browser resolves those
/// URLs relative to `c/<alias>` (`../../assets/x.pmtiles` → `assets/x.pmtiles`). Returns the copies.
pub fn copy_referenced(doc_json: &str, doc_dir: &Path, dir: &Path) -> Result<Vec<String>, String> {
    let doc: serde_json::Value = serde_json::from_str(doc_json).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for src in doc["data"].as_object().into_iter().flat_map(|m| m.values()) {
        let Some(url) = src.get("tiles").or_else(|| src.get("url")).and_then(|u| u.as_str()) else { continue };
        if url.contains("://") || url.starts_with('/') {
            continue;
        }
        // Resolve "c/<url>" the way a URL resolver does: `..` above the root stays at the root.
        let mut parts: Vec<&str> = vec!["c"];
        for seg in url.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                s => parts.push(s),
            }
        }
        let from = doc_dir.join(url);
        if !from.is_file() {
            continue;
        }
        let to = parts.iter().fold(dir.to_path_buf(), |p, s| p.join(s));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", from.display()))?;
        out.push(parts.join("/"));
    }
    Ok(out)
}

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()) {
        Some("js" | "mjs") => "text/javascript",
        Some("wasm") => "application/wasm",
        Some("html") => "text/html; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("pmtiles") => "application/vnd.pmtiles",
        // A site's own files (the showcase: styles, images, fonts, captions, video).
        Some("css") => "text/css; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff2") => "font/woff2",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        Some("vtt") => "text/vtt; charset=utf-8",
        Some("mp4") => "video/mp4",
        Some("csv") => "text/csv; charset=utf-8",
        Some("geojson") => "application/geo+json",
        _ if p.parent().and_then(|d| d.file_name()).is_some_and(|n| n == "c") => "application/json",
        _ => "application/octet-stream",
    }
}

fn index(dir: &Path) -> String {
    let mut aliases: Vec<String> = std::fs::read_dir(dir.join("c")).into_iter().flatten().filter_map(|e| e.ok()).filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.') && !n.contains('.')).collect();
    aliases.sort();
    let views: String = aliases.iter().map(|a| format!("<h2>{a}</h2>\n<datars-view src=\"/c/{a}\"></datars-view>\n")).collect();
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>datars serve</title><style>body{{font:15px/1.5 system-ui,sans-serif;max-width:860px;margin:2rem auto;padding:0 1rem}}datars-view{{display:block;margin:0 0 3rem}}</style>\
         <script type=module src='/runtime/datars.js'></script>\n<h1>datars serve</h1>\n{views}"
    )
}

/// A scrollytelling test page: the chart sticks while its tall container scrolls past, and the
/// scroll position scrubs the program.
fn scrolly(alias: &str) -> String {
    let a: String = alias.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>{a} — scrolly</title><style>body{{margin:0;font:15px/1.5 system-ui,sans-serif}}header,footer{{padding:40vh 2rem}}\
         [data-scrub]{{height:400vh}}datars-view{{position:sticky;top:0;display:block;height:100vh}}</style>\
         <script type=module src='/runtime/datars.js'></script>\
         <header><h1>{a}</h1><p>Scroll to move through the story.</p></header>\
         <div data-scrub><datars-view scrub src='/c/{a}' height=600></datars-view></div><footer>The end.</footer>"
    )
}

/// The image server: `/render/<alias>.png|.svg?state=N&width=W&height=H&dpr=D&mode=dark` renders a
/// published chart with the CPU reference (identical to what every runtime draws) — social cards,
/// email, anywhere scripts can't run.
fn render_image(dir: &Path, rest: &str, q: &std::collections::BTreeMap<String, String>) -> Result<(&'static str, Vec<u8>), String> {
    let (alias, ext) = rest.rsplit_once('.').ok_or("use /render/<alias>.png, .svg or .pdf")?;
    if alias.contains("..") || alias.contains('/') {
        return Err("bad alias".into());
    }
    let manifest = std::fs::read(dir.join("c").join(alias)).map_err(|_| format!("no chart `{alias}`"))?;
    let num = |k: &str| q.get(k).and_then(|v| v.parse::<f64>().ok());
    let req = datars_headless::ImageRequest { state: num("state").unwrap_or(0.0) as usize, width: num("width"), height: num("height"), dpr: num("dpr").unwrap_or(1.0), dark: q.get("mode").map(String::as_str) == Some("dark"), svg: ext == "svg", pdf: ext == "pdf" };
    datars_headless::render_published(&manifest, &|h| std::fs::read(dir.join("chunks").join(h.replacen(':', "_", 1))).ok(), &req)
}

/// A step-triggered story page: the chart sticks beside text steps (one per program state, with
/// the state's narration); each step scrolling into the middle of the viewport moves the chart.
fn steps_page(alias: &str) -> String {
    let a: String = alias.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    let steps: String = (0..6).map(|i| format!("<section class=step><p>Step {}</p></section>", i + 1)).collect();
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>{a} — steps</title><style>body{{margin:0;font:17px/1.5 system-ui,sans-serif;display:grid;grid-template-columns:1fr 1fr}}\
         .text{{padding:0 2rem}}.step{{min-height:80vh;display:flex;align-items:center}}.step p{{background:#fff;border:1px solid #ddd;border-radius:8px;padding:1rem}}\
         datars-view{{position:sticky;top:10vh;display:block}}</style>\
         <script type=module src='/runtime/datars.js'></script>\
         <div class=text><header style='padding:30vh 0'><h1>{a}</h1></header>{steps}</div>\
         <div><datars-view steps='.step' src='/c/{a}' height=480></datars-view></div>"
    )
}

/// A page with one `<datars-view>` for a document in the served directory.
fn doc_page(doc: &str) -> String {
    let src = doc.replace(['"', '<', '>', '&'], "");
    format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>\
         <title>{src}</title><style>body{{font:15px/1.5 system-ui,sans-serif;max-width:960px;margin:2rem auto;padding:0 1rem}}</style>\
         <script type=module src='/runtime/datars.js'></script>\n<datars-view doc=\"/{src}\" height=\"520\"></datars-view>\n"
    )
}

/// `Range: bytes=a-b` / `bytes=a-` / `bytes=-n` → [start, end) within `len`.
pub(crate) fn byte_range(header: &str, len: usize) -> Option<(usize, usize)> {
    let spec = header.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => (len.saturating_sub(n.parse().ok()?), len),
        (a, "") => (a.parse().ok()?, len),
        (a, b) => (a.parse().ok()?, b.parse::<usize>().ok()?.saturating_add(1).min(len)),
    };
    (start < end && start < len).then_some((start, end))
}

fn respond(mut s: TcpStream, dir: &Path, runtime: &Path) {
    let mut line = String::new();
    let mut r = BufReader::new(match s.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    });
    if r.read_line(&mut line).is_err() {
        return;
    }
    let mut range: Option<String> = None;
    let mut gzip = false;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).map(|n| n == 0).unwrap_or(true) || h == "\r\n" || h == "\n" {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            if k.trim().eq_ignore_ascii_case("range") {
                range = Some(v.trim().to_string());
            } else if k.trim().eq_ignore_ascii_case("accept-encoding") {
                gzip = v.split(',').any(|e| e.split(';').next().is_some_and(|e| e.trim() == "gzip"));
            }
        }
    }
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let path = remove_dot_segments(target.split('?').next().unwrap_or("/"));
    let query: std::collections::BTreeMap<String, String> = target.split_once('?').map(|(_, q)| q.split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect()).unwrap_or_default();
    // A folder asked for without its slash (`/site`) goes to `/site/`, as static hosts do: its page's
    // relative URLs (`site.css`, `c/chart`) resolve against the folder, not the parent.
    if !path.ends_with('/') && !path.starts_with("/runtime/") && !path.contains("..") && dir.join(path.trim_start_matches('/')).is_dir() {
        let q = target.split_once('?').map(|(_, q)| format!("?{q}")).unwrap_or_default();
        let _ = s.write_all(format!("HTTP/1.1 301 Moved Permanently\r\nLocation: {path}/{q}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").as_bytes());
        return;
    }
    let mut content_range = None;
    let mut from_file: Option<PathBuf> = None;
    let (status, ct, body): (&str, &str, Vec<u8>) = if path.contains("..") {
        ("400 Bad Request", "text/plain", b"bad path".to_vec())
    } else if path == "/" && !dir.join("index.html").is_file() {
        // A folder with its own page (a site) serves that; otherwise the list of its charts.
        ("200 OK", "text/html; charset=utf-8", index(dir).into_bytes())
    } else if let Some(alias) = path.strip_prefix("/scrolly/") {
        ("200 OK", "text/html; charset=utf-8", scrolly(alias).into_bytes())
    } else if let Some(rest) = path.strip_prefix("/render/") {
        match render_image(dir, rest, &query) {
            Ok((ct, bytes)) => ("200 OK", ct, bytes),
            Err(e) => ("404 Not Found", "text/plain", e.into_bytes()),
        }
    } else if let Some(alias) = path.strip_prefix("/steps/") {
        ("200 OK", "text/html; charset=utf-8", steps_page(alias).into_bytes())
    } else if let Some(doc) = path.strip_prefix("/view/") {
        ("200 OK", "text/html; charset=utf-8", doc_page(doc).into_bytes())
    } else {
        let file: PathBuf = match path.strip_prefix("/runtime/") {
            // A site that ships its own runtime (a copy of @datars/web) serves that one.
            Some(_) if dir.join(path.trim_start_matches('/')).is_file() => dir.join(path.trim_start_matches('/')),
            Some(rest) => runtime.join(rest),
            // A folder serves its index.html (a site's pages: `/`, `/articles/zodiac/`).
            None => {
                let f = dir.join(path.trim_start_matches('/'));
                if f.is_dir() { f.join("index.html") } else { f }
            }
        };
        if !file.exists() && path.ends_with(".auto.pmtiles") {
            build_auto(&file);
        }
        // A range of a plain file is read from disk as a range (tile and point archives are tens
        // of MB; a view asks for a few KB of them at a time).
        let whole = std::fs::metadata(&file).ok().filter(|m| m.is_file()).map(|m| m.len() as usize);
        let ranged = match (range.as_deref(), whole) {
            (Some(h), Some(len)) if !PathBuf::from(format!("{}.d", file.display())).is_dir() => byte_range(h, len).and_then(|(a, b)| datars_headless::read_range(&file, a as u64, (b - a) as u64).map(|bytes| (a, bytes, len))),
            _ => None,
        };
        if let Some((start, bytes, len)) = ranged {
            content_range = Some(format!("bytes {start}-{}/{len}", start + bytes.len() - 1));
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Type: {}\r\nContent-Length: {}\r\nContent-Range: {}\r\nAccept-Ranges: bytes\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: Content-Range\r\nConnection: close\r\n\r\n",
                content_type(&file),
                bytes.len(),
                content_range.unwrap_or_default()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&bytes);
            return;
        }
        let replayed = replay(&file);
        let fixed = replayed.is_err();
        match replayed.or_else(|_| std::fs::read(&file)) {
            Ok(b) => match range.as_deref().map(|h| byte_range(h, b.len())) {
                Some(Some((start, end))) => {
                    content_range = Some(format!("bytes {start}-{}/{}", end - 1, b.len()));
                    ("206 Partial Content", content_type(&file), b[start..end].to_vec())
                }
                Some(None) => {
                    content_range = Some(format!("bytes */{}", b.len()));
                    ("416 Range Not Satisfiable", "text/plain", Vec::new())
                }
                None => {
                    // A replay folder's snapshot changes under the same name: not kept packed.
                    from_file = fixed.then(|| file.clone());
                    ("200 OK", content_type(&file), b)
                }
            },
            Err(_) => ("404 Not Found", "text/plain", b"not found".to_vec()),
        }
    };
    let cache = if immutable(&path) { "public, max-age=31536000, immutable" } else { "no-cache" };
    let ranges = content_range.map(|c| format!("Content-Range: {c}\r\n")).unwrap_or_default();
    // Whole responses go gzipped to a browser that takes it, as static hosts and CDNs send them:
    // the web runtime's wasm is a third of its size on the wire.
    let packed = (gzip && status.starts_with("200")).then(|| gzipped(from_file.as_deref(), ct, &body)).flatten();
    let (body, encoding) = match &packed {
        Some(z) => (z.as_slice(), "Content-Encoding: gzip\r\n"),
        None => (body.as_slice(), ""),
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\n{encoding}Vary: Accept-Encoding\r\n{ranges}Accept-Ranges: bytes\r\nCache-Control: {cache}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Expose-Headers: Content-Range\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(body);
}

/// Content-addressed paths never change: bundle chunks (`…/chunks/<hash>`, wherever a site puts
/// its delivery folder) and a hashed runtime copy (`…/runtime-<hash>/…`, as @datars/vite writes).
fn immutable(path: &str) -> bool {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segs.windows(2).any(|w| w[0] == "chunks") || segs.iter().rev().skip(1).any(|s| s.strip_prefix("runtime-").is_some_and(|h| !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit())))
}

/// `body` gzipped, when that's worth it: text, wasm, JSON, fonts and bundle chunks, not formats
/// that are compressed already. A file's is made once and kept (by path, size and modification
/// time), so the runtime's few MB are compressed for the first reader only.
fn gzipped(file: Option<&Path>, ct: &str, body: &[u8]) -> Option<Vec<u8>> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};
    type Key = (PathBuf, usize, Option<std::time::SystemTime>);
    static PACKED: Mutex<BTreeMap<Key, Option<Arc<Vec<u8>>>>> = Mutex::new(BTreeMap::new());
    const KEEP: usize = 512 << 20;
    let packs = ct.starts_with("text/") || ["javascript", "json", "wasm", "svg", "font/ttf", "font/otf", "octet-stream"].iter().any(|t| ct.contains(t));
    if !packs || body.len() < 1024 {
        return None;
    }
    let pack = || {
        let mut z = flate2::write::GzEncoder::new(Vec::with_capacity(body.len() / 3), flate2::Compression::default());
        z.write_all(body).ok()?;
        let z = z.finish().ok()?;
        // Less than a tenth smaller: the reader's inflating isn't worth it.
        (z.len() * 10 < body.len() * 9).then_some(z)
    };
    let Some(file) = file else { return pack() };
    let key: Key = (file.to_path_buf(), body.len(), std::fs::metadata(file).and_then(|m| m.modified()).ok());
    if let Some(hit) = PACKED.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return hit.as_ref().map(|z| z.to_vec());
    }
    let z = pack().map(Arc::new);
    let mut packed = PACKED.lock().unwrap_or_else(|e| e.into_inner());
    if packed.values().flatten().map(|z| z.len()).sum::<usize>() > KEEP {
        packed.clear();
    }
    packed.insert(key, z.clone());
    z.map(|z| z.to_vec())
}

/// A missing automatic basemap beside a document (`/view/examples/rio/doc.json` asks for
/// `examples/rio/basemap.auto.pmtiles`): made on the first request, from the geodata cache
/// (fetching what it lacks), one build at a time; then it's a file like any other.
fn build_auto(file: &Path) {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let Some(doc) = file.parent().map(|d| d.join("doc.json")).filter(|d| d.is_file() && !file.exists()) else { return };
    let Ok(json) = std::fs::read_to_string(&doc) else { return };
    if let Err(e) = crate::auto_basemaps(&doc.to_string_lossy(), &json) {
        eprintln!("{}: automatic basemap: {e}", doc.display());
    }
}

static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// RFC 3986 dot-segment removal, clamped at the root as browsers do (`/c/../../a` → `/a`): a
/// client that sends unresolved relative paths still gets the file, and nothing escapes `dir`.
fn remove_dot_segments(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let trailing = path.ends_with('/') && !out.is_empty();
    format!("/{}{}", out.join("/"), if trailing { "/" } else { "" })
}

/// The current snapshot of a replay directory (`file` + `.d/`).
fn replay(file: &Path) -> std::io::Result<Vec<u8>> {
    let dir = PathBuf::from(format!("{}.d", file.display()));
    let mut snaps: Vec<PathBuf> = std::fs::read_dir(&dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect();
    snaps.sort();
    let period: f64 = std::env::var("DATARS_REPLAY_PERIOD").ok().and_then(|p| p.parse().ok()).filter(|p: &f64| *p > 0.0).unwrap_or(2.0);
    let elapsed = STARTED.get_or_init(std::time::Instant::now).elapsed().as_secs_f64();
    let i = ((elapsed / period) as usize).min(snaps.len().saturating_sub(1));
    snaps.get(i).map(std::fs::read).unwrap_or_else(|| Err(std::io::ErrorKind::NotFound.into()))
}

/// Serve `dir` (and the web runtime from `runtime`) on `addr` until the process ends.
pub fn serve(dir: &Path, runtime: &Path, addr: &str) -> Result<(), String> {
    let l = TcpListener::bind(addr).map_err(|e| format!("{addr}: {e}"))?;
    STARTED.get_or_init(std::time::Instant::now);
    println!("serving {} on http://{addr}/ (runtime from {})", dir.display(), runtime.display());
    for s in l.incoming().flatten() {
        let (d, r) = (dir.to_path_buf(), runtime.to_path_buf());
        std::thread::spawn(move || respond(s, &d, &r));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_segments_resolve_inside_the_root() {
        assert_eq!(remove_dot_segments("/c/../../assets/tiles/x.pmtiles"), "/assets/tiles/x.pmtiles");
        assert_eq!(remove_dot_segments("/../../etc/passwd"), "/etc/passwd", "clamped: never above the served folder");
        assert_eq!(remove_dot_segments("/a/./b/"), "/a/b/");
        assert_eq!(remove_dot_segments("/"), "/");
    }

    #[test]
    fn a_replay_folder_wins_over_the_published_snapshot() {
        let dir = std::env::temp_dir().join(format!("datars-replay-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("count.json.d")).unwrap();
        std::fs::write(dir.join("count.json"), "snapshot").unwrap();
        std::fs::write(dir.join("count.json.d/00.json"), "feed").unwrap();
        assert_eq!(replay(&dir.join("count.json")).unwrap(), b"feed");
        assert!(replay(&dir.join("other.json")).is_err(), "no folder: the file is served as usual");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn published_charts_render_on_request() {
        let dir = std::env::temp_dir().join(format!("datars-serve-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let doc = r#"{"datars":1,"size":{"width":200,"height":100},"scene":{"kind":"shape","key":"b","geom":{"type":"rect","x":10,"y":10,"w":80,"h":40},"fill":"$accent"}}"#;
        let (bundle, _) = datars_build::build(doc, &datars_build::Options::default()).unwrap();
        publish(&bundle, &dir, "demo").unwrap();
        let q = |s: &str| s.split('&').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let (ct, png) = render_image(&dir, "demo.png", &q("width=300&height=150&dpr=2")).unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(&png[1..4], b"PNG");
        let w = u32::from_be_bytes(png[16..20].try_into().unwrap());
        assert_eq!(w, 600, "width × dpr");
        let (ct, pdf) = render_image(&dir, "demo.pdf", &q("")).unwrap();
        assert_eq!(ct, "application/pdf");
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let (ct, svg) = render_image(&dir, "demo.svg", &q("")).unwrap();
        assert_eq!(ct, "image/svg+xml");
        assert!(String::from_utf8(svg).unwrap().starts_with("<svg"));
        assert!(render_image(&dir, "nope.png", &q("")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn byte_ranges_parse_like_http() {
        assert_eq!(byte_range("bytes=0-16383", 100_000), Some((0, 16384)));
        assert_eq!(byte_range("bytes=0-16383", 1000), Some((0, 1000)), "clamped at the end of the file");
        assert_eq!(byte_range("bytes=500-", 1000), Some((500, 1000)));
        assert_eq!(byte_range("bytes=-100", 1000), Some((900, 1000)));
        assert_eq!(byte_range("bytes=2000-3000", 1000), None);
        assert_eq!(byte_range("items=0-1", 1000), None);
    }

    #[test]
    fn serves_ranges_over_http() {
        let dir = std::env::temp_dir().join(format!("datars-serve-range-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.pmtiles"), (0u8..=255).collect::<Vec<u8>>()).unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let d = dir.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten().take(1) {
                respond(s, &d, &d);
            }
        });
        let mut c = TcpStream::connect(addr).unwrap();
        c.write_all(b"GET /a.pmtiles HTTP/1.1\r\nHost: x\r\nRange: bytes=10-19\r\n\r\n").unwrap();
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut c, &mut out).unwrap();
        let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&out[..split]).to_string();
        assert!(head.starts_with("HTTP/1.1 206"), "{head}");
        assert!(head.contains("Content-Range: bytes 10-19/256"), "{head}");
        assert_eq!(&out[split + 4..], (10u8..20).collect::<Vec<u8>>().as_slice());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_addressed_paths_are_immutable() {
        assert!(immutable("/chunks/b3_abc"));
        assert!(immutable("/datars/chunks/b3_abc"), "a site's delivery folder, wherever it is");
        assert!(immutable("/datars/runtime-a3c124d3b8/wasm/datars_core_bg.wasm"));
        assert!(!immutable("/datars/c/chart"), "manifests are live: republishing rewrites them");
        assert!(!immutable("/runtime/datars.js"));
        assert!(!immutable("/runtime-notes/index.html"));
        assert!(!immutable("/chunks"));
    }

    /// Whole files go gzipped to a client that asks for it; ranges and compressed formats as they are.
    #[test]
    fn gzips_whole_responses_for_clients_that_take_it() {
        let dir = std::env::temp_dir().join(format!("datars-serve-gzip-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let js = "export const x = 1;\n".repeat(400);
        std::fs::write(dir.join("a.js"), &js).unwrap();
        std::fs::write(dir.join("b.png"), vec![7u8; 4000]).unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let d = dir.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten().take(5) {
                respond(s, &d, &d);
            }
        });
        let get = |path: &str, headers: &str| {
            let mut c = TcpStream::connect(addr).unwrap();
            c.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n{headers}\r\n").as_bytes()).unwrap();
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut c, &mut out).unwrap();
            let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
            (String::from_utf8_lossy(&out[..split]).to_string(), out[split + 4..].to_vec())
        };
        let (head, body) = get("/a.js", "Accept-Encoding: gzip, deflate, br\r\n");
        assert!(head.contains("Content-Encoding: gzip") && head.contains("Vary: Accept-Encoding"), "{head}");
        assert!(body.len() < js.len() / 4, "{} of {}", body.len(), js.len());
        let mut plain = String::new();
        std::io::Read::read_to_string(&mut flate2::read::GzDecoder::new(body.as_slice()), &mut plain).unwrap();
        assert_eq!(plain, js);
        assert_eq!(get("/a.js", "Accept-Encoding: gzip\r\n").1, body, "kept packed: the same bytes again");
        let (head, body) = get("/a.js", "");
        assert!(!head.contains("Content-Encoding") && body == js.as_bytes(), "not asked for: as it is");
        let (head, body) = get("/a.js", "Accept-Encoding: gzip\r\nRange: bytes=0-9\r\n");
        assert!(head.starts_with("HTTP/1.1 206") && !head.contains("Content-Encoding") && body == &js.as_bytes()[..10], "{head}");
        assert!(!get("/b.png", "Accept-Encoding: gzip\r\n").0.contains("Content-Encoding"), "PNG is compressed already");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A site in a folder, asked for without the slash, redirects to it (so `site.css` resolves).
    #[test]
    fn a_folder_without_its_slash_redirects() {
        let dir = std::env::temp_dir().join(format!("datars-serve-slash-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("site")).unwrap();
        std::fs::write(dir.join("site/index.html"), "<link rel=stylesheet href=site.css>").unwrap();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let d = dir.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten().take(2) {
                respond(s, &d, &d);
            }
        });
        let get = |path: &str| {
            let mut c = TcpStream::connect(addr).unwrap();
            c.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
            let mut out = String::new();
            std::io::Read::read_to_string(&mut c, &mut out).unwrap();
            out
        };
        let r = get("/site?x=1");
        assert!(r.starts_with("HTTP/1.1 301") && r.contains("Location: /site/?x=1\r\n"), "{r}");
        assert!(get("/site/").starts_with("HTTP/1.1 200"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
