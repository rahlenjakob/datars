//! `datars` — the command line (docs/14-devtools-and-agents.md). Every command prints human text by
//! default and JSON with `--json`; images are written to files named in the output.

mod dev;
mod eject;
mod serve;
mod perf;
mod tools;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Args {
    cmd: String,
    pos: Vec<String>,
    flags: BTreeMap<String, String>,
}

/// Flags that never take a value (so `--hash doc.json` doesn't read `doc.json` as the value).
const BOOL_FLAGS: &[&str] = &["json", "update", "no-pixels", "hash", "help", "explain", "watch", "write", "timeline", "cpu"];

fn parse() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_else(|| "help".into());
    let (mut pos, mut flags) = (Vec::new(), BTreeMap::new());
    let rest: Vec<String> = it.collect();
    let mut i = 0;
    while i < rest.len() {
        let a = &rest[i];
        if let Some(f) = a.strip_prefix("--") {
            if let Some((k, v)) = f.split_once('=') {
                flags.insert(k.to_string(), v.to_string());
            } else if !BOOL_FLAGS.contains(&f) && i + 1 < rest.len() && !rest[i + 1].starts_with("--") {
                flags.insert(f.to_string(), rest[i + 1].clone());
                i += 1;
            } else {
                flags.insert(f.to_string(), "true".into());
            }
        } else {
            pos.push(a.clone());
        }
        i += 1;
    }
    Args { cmd, pos, flags }
}

const HELP: &str = "datars — deterministic data graphics

usage: datars <command> <doc.json|doc.ts> [options]

  render    <doc> [--state N|name] [--size WxH] [--dpr 2] [--mode dark] [--out f.png|f.svg|f.pdf]   render a state (CPU reference, or vector)
  film      <doc> --from A --to B [--frames 8] [--out prefix]                   filmstrip + motion trails of a transition
  video     <doc> [--fps 30] [--hold 2.5] [--dpr 1] [--size WxH] [--out f.mp4]    the program as a film (ffmpeg) + WebVTT captions
  publish   <doc> [--alias name] [--to dir] [--sign key]                        static delivery layout: c/<alias> + chunks/ (any static host); --sign: signed manifest
  keygen    [--out file]                                                       an ed25519 signing key for --sign (prints its public key, for <datars-view publishers>)
  basemap   <doc> [--build]                                                     an automatic basemap (tiles \"auto\"): views, tiles per zoom, data to fetch
  serve     [dir] [--port 8787]                                                 serve a delivery dir + the web runtime, with an index page
  replay    <session.json> [--frames 8] [--out f.png]                            replay a recorded host session exactly, as a filmstrip
  budgets   [--update]                                                          bundle + runtime sizes (gzip) against budgets.json
  profile   <doc> [--dpr 2] [--timeline] [--cpu] [--json]                      states cold/warm, then every transition stepped at 60 fps: stall, per-frame engine + GPU raster
  dev       <doc.ts|doc.json> [--port 8788]                                     live page: rebuild on save, diagnostics, lint, states
  lint      <doc> [--size WxH[,WxH]]                                            identity, encoding, legibility, accessibility, theme checks (in those boxes)
  explain   <doc> --key '(\"SE\",)' [--state N]                             why an element exists: recipes, template, data row, expression values
  data      profile <file.csv|file.json>                                        column types, missing values, ranges, candidate keys, hints
  new       <dir> [--template chart|story|explorable|map]                        a working document to start from
  describe  [std/<recipe>]                                                      std recipes, or one recipe's params, defaults and tokens
  docs      [--out dir]                                                         regenerate docs/reference (std.md, ir.schema.json) and llms.txt
  schema                                                                        the IR as a JSON Schema (what a doc.json may contain)
  diff      <docA> <docB> [--state N] | <doc> --states A,B                      element-level scene diff (added, removed, changed)
  migrate   <doc.json> [--write]                                                upgrade a document to the current IR; report ignored fields
  eject     std/<recipe> [--to recipes/]                                        copy a std recipe into the project as editable TypeScript
  gpu       [filter] [--dpr 2]                                                  GPU (wgpu) vs CPU reference over the examples (ΔE)
  inspect   <doc> [--state N]                                                   scene snapshot (text)
  states    <doc>                                                               list program states
  check     <doc>                                                               diagnostics (incl. fonts with no source)
  fonts     <doc>                                                               font tokens: family, weight, source, licence, cache
  semantics <doc> [--state N]                                                   the accessibility tree
  bundle    <doc> [--out f.datars] [--sign key]                                 publish: document → bundle (T0–T3 variants)
  bundle    inspect <f.datars>                                                  variants, chunk sizes, and which variant each runtime plays
  test      [filter] [--update] [--samples 64] [--no-pixels]                    visual + motion tests over examples/
  theme     <theme.json> [--mode light|dark|high-contrast]                      resolve and validate a theme
  theme     <theme.json | name> --specimen out.png                              std charts, palettes and inks in light, dark, high contrast
  help

  --json    machine-readable output
  --data    name=file[,name=file]   fill data slots from files (a user's rows instead of the sample)";

fn repo_root() -> PathBuf {
    let mut p = std::env::current_exe().unwrap_or_default();
    while p.pop() {
        if p.join("public/scripts/doc-to-json.mjs").exists() {
            return p.join("public");
        }
        if p.join("scripts/doc-to-json.mjs").exists() {
            return p;
        }
    }
    PathBuf::from(".")
}

/// Read a document: JSON directly, or a TypeScript doc compiled with Node (packages/sdk + std).
/// `--data name=file[,name=file…]`: fill data slots (or replace sources) from files, as a host app
/// would on the device — to preview a chart with a real user's rows instead of its sample.
fn provide_data(e: &mut datars_engine::Engine, spec: Option<&String>) -> Result<(), String> {
    for item in spec.map(|s| s.split(',').collect::<Vec<_>>()).unwrap_or_default() {
        let (name, file) = item.split_once('=').ok_or_else(|| format!("--data {item}: expected name=file"))?;
        let bytes = std::fs::read(file).map_err(|err| format!("--data {file}: {err}"))?;
        e.provide(name, &bytes)?;
    }
    Ok(())
}

/// The compiler a user's project gets (its own @datars/sdk, std and esbuild), carried in the binary.
const COMPILE_TS: &str = include_str!("compile_ts.mjs");

/// Is this document in a project that installs @datars/sdk (not a file in the datars repo)?
fn in_user_project(path: &str) -> bool {
    let start = std::fs::canonicalize(path).ok().and_then(|p| p.parent().map(Path::to_path_buf));
    start.is_some_and(|d| d.ancestors().any(|a| a.join("node_modules/@datars/sdk/package.json").is_file()))
}

/// The web runtime to serve for a document or folder: the project's installed @datars/web, else
/// the repo's build.
fn web_runtime(near: &Path) -> PathBuf {
    let start = std::fs::canonicalize(near).unwrap_or_else(|_| near.to_path_buf());
    start.ancestors().map(|a| a.join("node_modules/@datars/web/dist")).find(|d| d.join("datars.js").is_file()).unwrap_or_else(|| repo_root().join("packages/web/dist"))
}

fn read_doc(path: &str) -> Result<String, String> {
    if path.ends_with(".ts") {
        // A user's project compiles with its own packages; inside the repo, the sources.
        let script = if in_user_project(path) {
            let f = std::env::temp_dir().join(format!("datars-compile-ts-{}.mjs", env!("CARGO_PKG_VERSION")));
            if std::fs::read_to_string(&f).ok().as_deref() != Some(COMPILE_TS) {
                std::fs::write(&f, COMPILE_TS).map_err(|e| format!("{}: {e}", f.display()))?;
            }
            f
        } else {
            repo_root().join("scripts/doc-to-json.mjs")
        };
        let out = std::process::Command::new("node").arg(&script).arg(path).output().map_err(|e| format!("running node: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

/// The directory a document's relative URLs resolve against.
fn doc_dir(path: &str) -> &Path {
    Path::new(path).parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."))
}

/// A document's automatic basemaps (`tiles: "auto"`), current next to it: built from the geodata
/// cache when its cameras changed, fetching what the cache lacks (docs/09-geo.md).
fn auto_basemaps(path: &str, json: &str) -> Result<Vec<datars_build::auto::Archive>, String> {
    datars_build::auto::ensure_default(json, doc_dir(path))
}

/// The document as `publish`/`bundle` compile it: automatic sources pointing at the archive's
/// shipped name (`url(source, content hash)`), and a fetch that reads those from where they are.
struct Shipped {
    json: String,
    /// (url in the document, file on disk).
    files: Vec<(String, PathBuf)>,
}

fn ship_basemaps(path: &str, json: &str, url: &dyn Fn(&str, &str) -> String) -> Result<Shipped, String> {
    let mut urls = BTreeMap::new();
    let mut files = Vec::new();
    for a in auto_basemaps(path, json)? {
        let bytes = std::fs::read(&a.path).map_err(|e| format!("{}: {e}", a.path.display()))?;
        let u = url(&a.source, &datars_build::auto::content_hash(&bytes)[..12]);
        urls.insert(a.source.clone(), u.clone());
        files.push((u, a.path));
    }
    let json = if urls.is_empty() { json.to_string() } else { datars_build::auto::rewrite(json, &urls)? };
    // Point pyramids over data sources (`instances` with `lod`) ship as archives read by range,
    // not as rows or generators: written next to the document by content (like automatic
    // basemaps; any project, not only the datars repo), named by `url` like the basemaps
    // (docs/12-delivery.md, "Big data stays interactive").
    let (archives, notes) = datars_build::points::archives(&json, Some(doc_dir(path)))?;
    for n in notes {
        eprintln!("note: {n}");
    }
    let mut points = BTreeMap::new();
    for a in archives {
        let hash = datars_build::auto::content_hash(&a.bytes)[..12].to_string();
        let dir = doc_dir(path).to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let file = dir.join(format!("{}.{hash}.pmtiles", a.source));
        if !file.is_file() {
            std::fs::write(&file, &a.bytes).map_err(|e| format!("{}: {e}", file.display()))?;
        }
        let u = url(&a.source, &hash);
        points.insert(a.source.clone(), u.clone());
        files.push((u, file));
    }
    let json = if points.is_empty() { json } else { datars_build::points::rewrite(&json, &points)? };
    Ok(Shipped { json, files })
}

impl Shipped {
    /// Shipped basemap ranges from their files; everything else (data, fonts) the way `load` does.
    fn fetch(&self, r: &datars_engine::Request, dir: &Path, fonts: &datars_build::fonts::acquire::Fonts) -> Option<Vec<u8>> {
        if let datars_engine::Request::Range { url, offset, length, .. } = r {
            if let Some((_, file)) = self.files.iter().find(|(u, _)| u == url) {
                return datars_headless::read_range(file, *offset, *length);
            }
        }
        datars_build::fonts::acquire::fetch_with(fonts, Some(dir))(r)
    }
}

/// Font acquisition for every command that loads a document: Google Fonts and font URLs come
/// through the disk cache (downloaded once, at build time — never by a runtime).
fn fonts() -> datars_build::fonts::acquire::Fonts {
    datars_build::fonts::acquire::Fonts::from_env()
}

/// Load a document the way the build step sees it: data and font files from disk next to it,
/// remote fonts acquired through the cache — so previews draw what bundles ship.
fn load(json: &str, dir: Option<&Path>) -> Result<datars_engine::Engine, String> {
    let f = fonts();
    let fetch = datars_build::fonts::acquire::fetch_with(&f, dir);
    datars_headless::load_at_with(json, dir, &fetch)
}

fn state_index(engine: &datars_engine::Engine, s: Option<&String>) -> usize {
    match s {
        None => 0,
        // A state's name first (states named "2025" and "2036" are years, not indices), then an index.
        Some(v) => engine.state_names().iter().position(|n| n == v).or_else(|| v.parse().ok()).unwrap_or(0),
    }
}

fn run(a: Args) -> Result<(), String> {
    let json = a.flags.contains_key("json");
    if a.cmd == "help" || a.cmd == "--help" || a.cmd == "-h" || a.flags.contains_key("help") {
        println!("{HELP}");
        return Ok(());
    }
    if a.cmd == "theme" {
        let arg = a.pos.first().ok_or("usage: datars theme <theme.json | built-in name> [--mode dark] [--specimen out.png]")?;
        if let Some(out) = a.flags.get("specimen") {
            // Light, dark and high contrast side by side: the theme as every std chart wears it.
            let inline: Option<serde_json::Value> = std::fs::read_to_string(arg).ok().map(|t| serde_json::from_str(&t)).transpose().map_err(|e| format!("{arg}: {e}"))?;
            let name = inline.as_ref().and_then(|t| t["name"].as_str()).unwrap_or(arg).to_string();
            let doc = tools::specimen_doc(&name, inline.as_ref());
            let mut engine = load(&doc.to_string(), None)?;
            let dpr: f64 = a.flags.get("dpr").and_then(|d| d.parse().ok()).unwrap_or(1.0);
            let modes = [datars_theme::Mode::Light, datars_theme::Mode::Dark, datars_theme::Mode::HighContrast];
            let panels: Vec<_> = modes
                .iter()
                .map(|m| {
                    engine.set_mode(*m);
                    let scene = engine.scene();
                    datars_headless::render_scene(&engine, &scene, dpr)
                })
                .collect();
            let (w, h, gap) = (panels[0].width, panels[0].height, 12);
            let total = w * 3 + gap * 2;
            let mut sheet = datars_render_cpu::Pixmap { width: total, height: h, data: [128u8, 128, 128, 255].repeat((total * h) as usize) };
            for (i, p) in panels.iter().enumerate() {
                datars_headless::blit(&mut sheet, p, i as u32 * (w + gap), 0, 1);
            }
            if let Some(dir) = Path::new(out).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(out, sheet.to_png()).map_err(|e| e.to_string())?;
            for d in engine.diagnostics() {
                eprintln!("diagnostic: {}", d.message);
            }
            println!("{out}  (light · dark · high contrast)");
            return Ok(());
        }
        let src = std::fs::read_to_string(arg).map_err(|e| e.to_string())?;
        let t = datars_theme::Theme::from_json(&src)?;
        let mut set = datars_theme::ThemeSet::with_builtins();
        let name = t.name.clone();
        set.add(t);
        let mode = match a.flags.get("mode").map(|s| s.as_str()) {
            Some("dark") => datars_theme::Mode::Dark,
            Some("high-contrast") => datars_theme::Mode::HighContrast,
            _ => datars_theme::Mode::Light,
        };
        let (r, diags) = datars_theme::resolve(&set.chain(&name)?, mode, &[]);
        let findings = datars_theme::validate(&r);
        if json {
            println!("{}", serde_json::json!({ "tokens": r.to_json(), "diagnostics": diags.iter().map(|d| format!("{}: {}", d.token, d.message)).collect::<Vec<_>>(), "findings": findings.iter().map(|f| serde_json::json!({"severity": format!("{:?}", f.severity), "check": f.check, "message": f.message})).collect::<Vec<_>>() }));
        } else {
            for d in &diags {
                println!("diagnostic  {}: {}", d.token, d.message);
            }
            for f in &findings {
                println!("{:?}  {}  {}", f.severity, f.check, f.message);
            }
            println!("{} tokens resolved, {} findings", r.colors.len() + r.palettes.len() + r.numbers.len(), findings.len());
        }
        return Ok(());
    }
    if a.cmd == "test" {
        let root = std::env::current_dir().map_err(|e| e.to_string())?;
        let root = if root.join("examples").exists() { root } else { repo_root() };
        let opts = datars_test::Options {
            root,
            filter: a.pos.first().cloned().unwrap_or_default(),
            update: a.flags.contains_key("update"),
            samples: a.flags.get("samples").and_then(|s| s.parse().ok()).unwrap_or(64),
            pixels: !a.flags.contains_key("no-pixels"),
        };
        let r = datars_test::run(&opts);
        if json {
            println!("{}", serde_json::to_string_pretty(&r).unwrap_or_default());
        } else {
            for f in &r.failures {
                println!("FAIL {}  {}\n  {}", f.example, f.what, f.detail.replace('\n', "\n  "));
                for a in &f.artifacts {
                    println!("  → {a}");
                }
            }
            println!(
                "{} examples · {} states · {} transitions · {} frames checked · {} failures · {} ms{}",
                r.examples, r.states, r.transitions, r.frames_checked, r.failures.len(), r.millis,
                if r.updated.is_empty() { String::new() } else { format!(" · goldens written: {}", r.updated.join(", ")) }
            );
        }
        return if r.failures.is_empty() { Ok(()) } else { Err(format!("{} failure(s)", r.failures.len())) };
    }
    if a.cmd == "gpu" {
        let filter = a.pos.first().map(String::as_str).unwrap_or("");
        let dpr: f64 = a.flags.get("dpr").and_then(|d| d.parse().ok()).unwrap_or(2.0);
        let root = repo_root();
        let states = datars_test::gpu::run(&root, filter, dpr, &root.join("out/gpu"))?;
        let fails = states.iter().filter(|s| !s.pass).count();
        for s in &states {
            println!(
                "{} {:<11} {:<10} mean ΔE {:.3}  p99 ΔE {:>5.2}  visible {:.3}%  solid {:>3}{}",
                if s.pass { "ok  " } else { "FAIL" },
                s.example, s.state, s.mean_de, s.p99_de, s.visible * 100.0, s.solid,
                s.heatmap.as_ref().map(|h| format!("\n     → {h}")).unwrap_or_default()
            );
        }
        println!("{} states · {} over the limits (mean ΔE ≤ {}, visible ≤ {}%, solid ≤ {})", states.len(), fails, datars_test::gpu::MAX_MEAN_DE, datars_test::gpu::MAX_VISIBLE * 100.0, datars_test::gpu::MAX_SOLID);
        return if fails == 0 { Ok(()) } else { Err(format!("{fails} state(s) differ between GPU and CPU")) };
    }
    if a.cmd == "dev" {
        let path = a.pos.first().ok_or("a document path is required (doc.ts or doc.json)")?;
        let port = a.flags.get("port").cloned().unwrap_or_else(|| "8788".into());
        return dev::dev(Path::new(path), &web_runtime(Path::new(path)), &format!("127.0.0.1:{port}"), read_doc);
    }
    if a.cmd == "eject" {
        let what = a.pos.first().ok_or("which recipe? e.g. `datars eject std/bar`")?;
        let name = what.strip_prefix("std/").or_else(|| what.strip_prefix("@datars/std/")).unwrap_or(what);
        let src = eject::eject(&repo_root().join("packages/std/src"), name)?;
        let dir = PathBuf::from(a.flags.get("to").cloned().unwrap_or_else(|| "recipes".into()));
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let out = dir.join(format!("{name}.ts"));
        if out.exists() && !a.flags.contains_key("force") {
            return Err(format!("{} exists (--force to overwrite)", out.display()));
        }
        std::fs::write(&out, src).map_err(|e| e.to_string())?;
        println!("{}\nimport {{ {name} }} from \"./{}/{name}\" in your doc; it's embedded when the doc is built", out.display(), dir.display());
        return Ok(());
    }
    if a.cmd == "lint" {
        let path = a.pos.first().ok_or("a document path is required")?;
        let mut e = load(&read_doc(path)?, Path::new(path).parent())?;
        provide_data(&mut e, a.flags.get("data"))?;
        // `--size 390x600[,360x640…]`: lint the layouts a reader gets in those boxes (a chart
        // authored for a wide column, read on a phone), each finding marked with its size.
        let sizes: Vec<(f64, f64)> = match a.flags.get("size") {
            Some(list) => list
                .split(',')
                .map(|sz| {
                    let (w, h) = sz.split_once('x').ok_or("--size WxH[,WxH…]")?;
                    Ok((w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?))
                })
                .collect::<Result<_, String>>()?,
            None => Vec::new(),
        };
        let findings = if sizes.is_empty() {
            datars_devtools::lint(&mut e)
        } else {
            let mut all = Vec::new();
            for (w, h) in &sizes {
                e.resize(*w, *h, 1.0);
                for mut f in datars_devtools::lint(&mut e) {
                    f.message = format!("{w}×{h} {}", f.message);
                    all.push(f);
                }
            }
            all
        };
        if json {
            println!("{}", serde_json::json!(findings.iter().map(|f| serde_json::json!({ "rule": f.rule, "severity": f.severity, "message": f.message, "fix": f.fix })).collect::<Vec<_>>()));
        } else if findings.is_empty() {
            println!("ok — no lint findings");
        } else {
            for f in &findings {
                println!("{:<8} {:<14} {}\n         fix: {}", f.severity, f.rule, f.message, f.fix);
            }
        }
        return if findings.iter().any(|f| f.severity == "error") { Err("lint errors".into()) } else { Ok(()) };
    }
    if a.cmd == "budgets" {
        // Size budgets (docs/12-delivery.md §Budgets in CI): every example's bundle, gzipped —
        // the first load a full runtime picks (T3, else T2, else T1) and the T0 fallback — plus
        // the web runtime when it's built. A regression fails; `--update` records the current
        // sizes with 15 % headroom.
        let root = repo_root();
        let path = root.join("budgets.json");
        let old: serde_json::Value = std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let mut now = serde_json::Map::new();
        let mut names: Vec<String> = std::fs::read_dir(root.join("examples")).map_err(|e| e.to_string())?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in &names {
            let doc = root.join("examples").join(name).join("doc.json");
            let Ok(json) = std::fs::read_to_string(&doc) else { continue };
            let (_, report) = datars_build::build_with(&json, &datars_build::Options::default(), &datars_build::fonts::acquire::fetch_with(&fonts(), doc.parent()))?;
            let g = &report.gzip_by_tier;
            let first = g.get("T3").or_else(|| g.get("T2")).or_else(|| g.get("T1")).copied().unwrap_or(0);
            now.insert(format!("{name}/first"), first.into());
            now.insert(format!("{name}/fallback"), g.get("T0").copied().unwrap_or(0).into());
        }
        for (name, file) in [("runtime/web-core", "datars_core_bg.wasm"), ("runtime/web-core-gl", "datars_core_gl_bg.wasm"), ("runtime/web-full", "datars_host_web_bg.wasm")] {
            if let Ok(bytes) = std::fs::read(root.join("packages/web/dist/wasm").join(file)) {
                now.insert(name.into(), datars_build::gzip_len(&bytes).into());
            }
        }
        let mut fails = 0;
        println!("{:<24} {:>10} {:>10}", "", "gzip", "budget");
        for (k, v) in &now {
            let v = v.as_u64().unwrap_or(0);
            let budget = old.get("budgets").and_then(|b| b.get(k)).and_then(|b| b.as_u64());
            let over = budget.is_some_and(|b| v > b);
            fails += over as usize;
            println!("{k:<24} {v:>10} {:>10}{}", budget.map(|b| b.to_string()).unwrap_or_else(|| "—".into()), if over { "  OVER" } else { "" });
        }
        if a.flags.contains_key("update") {
            let budgets: serde_json::Map<String, serde_json::Value> = now.iter().map(|(k, v)| (k.clone(), ((v.as_u64().unwrap_or(0) as f64 * 1.15).ceil() as u64).into())).collect();
            let doc = serde_json::json!({ "note": "gzip bytes; datars budgets checks, --update rewrites with 15% headroom. Targets: docs/12-delivery.md §Size targets.", "budgets": budgets });
            std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
            println!("budgets written: {}", path.display());
            return Ok(());
        }
        return if fails == 0 { Ok(()) } else { Err(format!("{fails} budget(s) exceeded")) };
    }
    if a.cmd == "replay" {
        // A recorded session (from a host: View.take_recording()) replayed exactly, as a filmstrip.
        let path = a.pos.first().ok_or("a session file is required")?;
        let session = datars_engine::session::Session::from_json(&std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?)?;
        let frames = session.inputs.iter().filter(|i| matches!(i, datars_engine::session::Input::Frame { .. })).count();
        let want: usize = a.flags.get("frames").and_then(|f| f.parse().ok()).unwrap_or(8).max(1);
        let step = frames.div_ceil(want).max(1);
        let (mut k, mut shots) = (0usize, Vec::new());
        let engine = datars_engine::Engine::replay(&session, |_, f| {
            if let Some(f) = f {
                if k % step == 0 || k + 1 == frames {
                    shots.push(f.display.clone());
                }
                k += 1;
            }
        })?;
        let dpr: f64 = a.flags.get("dpr").and_then(|d| d.parse().ok()).unwrap_or(1.0);
        let imgs: Vec<_> = shots.iter().map(|l| datars_render_cpu::render(l, engine.fonts(), dpr)).collect();
        let (w, h) = imgs.first().map(|p| (p.width, p.height)).unwrap_or((1, 1));
        let mut strip = datars_render_cpu::Pixmap { width: w * imgs.len() as u32 + 8 * (imgs.len() as u32).saturating_sub(1), height: h, data: vec![235; ((w * imgs.len() as u32 + 8 * (imgs.len() as u32).saturating_sub(1)) * h * 4) as usize] };
        for (i, img) in imgs.iter().enumerate() {
            datars_headless::blit(&mut strip, img, i as u32 * (w + 8), 0, 1);
        }
        let out = a.flags.get("out").cloned().unwrap_or_else(|| "out/replay-strip.png".into());
        if let Some(dir) = Path::new(&out).parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&out, strip.to_png()).map_err(|e| e.to_string())?;
        println!("{} inputs, {frames} frames replayed; final state `{}`\n{out}", session.inputs.len(), engine.state());
        return Ok(());
    }
    if a.cmd == "new" {
        let dir = a.pos.first().ok_or("usage: datars new <dir> [--template chart|story|explorable|map]")?;
        let t = a.flags.get("template").map(|s| s.as_str()).unwrap_or("chart");
        let f = tools::new_doc(Path::new(dir), t)?;
        println!("{f}\n  datars dev {f}    # live preview");
        return Ok(());
    }
    if a.cmd == "docs" {
        // Reference docs and the agents' index, generated from the recipes and this help text.
        let root = a.flags.get("out").map(PathBuf::from).unwrap_or_else(repo_root);
        let all = datars_devtools::describe_std(&datars_engine::Engine::new());
        std::fs::create_dir_all(root.join("docs/reference")).map_err(|e| e.to_string())?;
        std::fs::write(root.join("docs/reference/std.md"), tools::reference_md(&all)).map_err(|e| e.to_string())?;
        std::fs::write(root.join("llms.txt"), tools::llms_txt(&all, HELP)).map_err(|e| e.to_string())?;
        std::fs::write(root.join("docs/reference/ir.schema.json"), tools::schema_json()).map_err(|e| e.to_string())?;
        println!("{}\n{}\n{}", root.join("docs/reference/std.md").display(), root.join("docs/reference/ir.schema.json").display(), root.join("llms.txt").display());
        return Ok(());
    }
    if a.cmd == "schema" {
        print!("{}", tools::schema_json());
        return Ok(());
    }
    if a.cmd == "describe" {
        print!("{}", tools::describe(&datars_devtools::describe_std(&datars_engine::Engine::new()), a.pos.first().map(|s| s.as_str()), json)?);
        return Ok(());
    }
    if a.cmd == "migrate" {
        let path = a.pos.first().ok_or("usage: datars migrate <doc.json> [--write]")?;
        let (v, notes, ignored) = tools::migrate(&std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?)?;
        if json {
            println!("{}", serde_json::json!({ "changes": notes, "ignored": ignored }));
        } else {
            for n in &notes {
                println!("~ {n}");
            }
            for f in &ignored {
                println!("? {f}: not part of the IR (ignored)");
            }
            if notes.is_empty() && ignored.is_empty() {
                println!("current — nothing to migrate");
            }
        }
        if a.flags.contains_key("write") && !notes.is_empty() {
            std::fs::write(path, serde_json::to_string_pretty(&v).unwrap_or_default() + "\n").map_err(|e| e.to_string())?;
            println!("written: {path}");
        }
        return Ok(());
    }
    if a.cmd == "bundle" && a.pos.first().map(|s| s.as_str()) == Some("inspect") {
        let file = a.pos.get(1).ok_or("usage: datars bundle inspect <file.datars>")?;
        let v = tools::bundle_inspect(&std::fs::read(file).map_err(|e| format!("{file}: {e}"))?)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        } else {
            print!("{}", tools::bundle_text(&v));
        }
        return Ok(());
    }
    if a.cmd == "diff" {
        // `datars diff A B [--state N]` (two documents) or `datars diff doc --states A B`.
        let scene = |path: &str, state: Option<&String>| -> Result<datars_scene::Scene, String> {
            let mut e = load(&read_doc(path)?, Path::new(path).parent())?;
            let i = state_index(&e, state);
            Ok(e.scene_for_state(i))
        };
        let (sa, sb) = match (a.pos.first(), a.pos.get(1), a.flags.get("states")) {
            (Some(doc), None, Some(states)) => {
                let (x, y) = states.split_once(',').or_else(|| states.split_once(' ')).ok_or("--states A,B")?;
                (scene(doc, Some(&x.to_string()))?, scene(doc, Some(&y.to_string()))?)
            }
            (Some(x), Some(y), _) => (scene(x, a.flags.get("state"))?, scene(y, a.flags.get("state"))?),
            _ => return Err("usage: datars diff <docA> <docB> [--state N] | datars diff <doc> --states A,B".into()),
        };
        let d = tools::diff(&sa, &sb);
        if json {
            println!("{}", serde_json::to_string_pretty(&d).unwrap_or_default());
        } else {
            print!("{}", tools::diff_text(&d));
        }
        return Ok(());
    }
    if a.cmd == "data" {
        // `datars data profile <file>`: what a CSV/JSON file holds before it is charted.
        let (Some("profile"), Some(file)) = (a.pos.first().map(|s| s.as_str()), a.pos.get(1)) else { return Err("usage: datars data profile <file.csv|file.json>".into()) };
        let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
        // Values that name countries are checked against the built-in atlas.
        let mut ids = datars_devtools::profile::IdSets::new();
        if let Ok(atlas) = std::fs::read(repo_root().join("assets/atlas/countries.geojson")) {
            let v: serde_json::Value = serde_json::from_slice(&atlas).unwrap_or_default();
            let set = v["features"].as_array().into_iter().flatten().filter_map(|f| f["properties"]["id"].as_str().map(String::from)).collect();
            ids.insert("countries".into(), set);
        }
        let name = Path::new(file).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let p = datars_devtools::profile::profile_bytes(&name, &bytes, &ids)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&p).unwrap_or_default());
        } else {
            print!("{}", datars_devtools::profile::to_text(&p));
        }
        return Ok(());
    }
    if a.cmd == "basemap" {
        // What an automatic basemap holds: the views it's cut to, tiles per zoom, the data it reads
        // (cached or not), and whether the archive next to the document is current. `--fetch`
        // (or `--build`) fills the cache and builds it.
        let path = a.pos.first().ok_or("usage: datars basemap <doc> [--build]")?;
        let src_json = read_doc(path)?;
        let doc = datars_ir::Doc::from_json(&src_json)?;
        let mut e = datars_headless::load_at(&src_json, Some(doc_dir(path)))?;
        let views = e.tile_views(datars_build::auto::FLIGHT_SAMPLES);
        let cache = datars_build::auto::Cache::from_env();
        let mut out = Vec::new();
        for p in datars_build::auto::plan_views(&doc, &views, &[]) {
            let needs = datars_build::auto::geo::needs(&p.job);
            let missing = datars_build::auto::geo::missing(&cache, &needs);
            let mut per_zoom: BTreeMap<u32, u64> = BTreeMap::new();
            for r in &p.job.cover {
                *per_zoom.entry(r[0]).or_default() += (r[3] - r[1] + 1) as u64 * (r[4] - r[2] + 1) as u64;
            }
            let settled: Vec<serde_json::Value> = views.iter().filter(|v| v.source == p.source && !v.state.is_empty()).map(|v| serde_json::json!({ "state": v.state, "zoom": (v.zoom * 100.0).round() / 100.0, "bbox": v.bbox, "explore": v.explore })).fold(Vec::new(), |mut acc, v| { if !acc.contains(&v) { acc.push(v); } acc });
            let file = doc_dir(path).join(&p.file);
            out.push(serde_json::json!({
                "source": p.source, "file": file, "current": datars_build::auto::is_current(&file, &p),
                "lang": p.lang, "views": settled, "tiles_by_zoom": per_zoom.iter().map(|(z, n)| [*z as u64, *n]).collect::<Vec<_>>(),
                "needs": needs.len(), "missing": missing.iter().map(|n| n.describe()).collect::<Vec<_>>(), "cache": cache.root,
            }));
        }
        if a.flags.contains_key("build") || a.flags.contains_key("fetch") {
            auto_basemaps(path, &src_json)?;
        }
        if json {
            println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        } else if out.is_empty() {
            println!("no automatic basemap (a tiles source `\"auto\"`) in {path}");
        } else {
            for s in &out {
                println!("basemap `{}` → {} ({})", s["source"].as_str().unwrap_or(""), s["file"].as_str().unwrap_or(""), if s["current"].as_bool() == Some(true) { "current" } else { "to build" });
                for v in s["views"].as_array().into_iter().flatten() {
                    println!("  {:<16} z{:<6} {}{}", v["state"].as_str().unwrap_or(""), v["zoom"], v["bbox"], if v["explore"].as_bool() == Some(true) { "  (explorable)" } else { "" });
                }
                let zooms: Vec<String> = s["tiles_by_zoom"].as_array().into_iter().flatten().map(|zn| format!("z{}:{}", zn[0], zn[1])).collect();
                println!("  tiles  {}", zooms.join(" "));
                let missing = s["missing"].as_array().map(|m| m.len()).unwrap_or(0);
                println!("  data   {} inputs, {} to fetch into {}", s["needs"], missing, s["cache"].as_str().unwrap_or(""));
            }
        }
        return Ok(());
    }
    if a.cmd == "keygen" {
        return keygen(a.flags.get("out").map(String::as_str).unwrap_or("datars-signing.key"), a.flags.contains_key("json"));
    }
    if a.cmd == "serve" {
        let dir = a.pos.first().map(PathBuf::from).unwrap_or_else(|| repo_root().join("out/site"));
        let port = a.flags.get("port").cloned().unwrap_or_else(|| "8787".into());
        return serve::serve(&dir, &web_runtime(&dir), &format!("127.0.0.1:{port}"));
    }
    let path = a.pos.first().ok_or("a document path is required (see `datars help`)")?;
    let source = read_doc(path)?;
    // Automatic basemaps are made (or found current) before anything draws them; `publish` and
    // `bundle` ship them under their own names below.
    if !matches!(a.cmd.as_str(), "states" | "publish" | "bundle") {
        auto_basemaps(path, &source)?;
    }
    let mut engine = load(&source, std::path::Path::new(path).parent())?;
    provide_data(&mut engine, a.flags.get("data"))?;
    if let Some(sz) = a.flags.get("size") {
        let (w, h) = sz.split_once('x').ok_or("--size WxH")?;
        engine.resize(w.parse().map_err(|_| "bad width")?, h.parse().map_err(|_| "bad height")?, 1.0);
    }
    // `--mode dark|high-contrast`: the same scenes under another mode (inks are late-bound).
    match a.flags.get("mode").map(|s| s.as_str()) {
        Some("dark") => engine.set_mode(datars_theme::Mode::Dark),
        Some("high-contrast") => engine.set_mode(datars_theme::Mode::HighContrast),
        _ => {}
    }
    let dpr: f64 = a.flags.get("dpr").and_then(|d| d.parse().ok()).unwrap_or(2.0);
    let stem = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "doc".into());
    let stem = if stem == "doc" { Path::new(path).parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or(stem) } else { stem };
    match a.cmd.as_str() {
        "fonts" => {
            let rows = tools::fonts_report(&engine);
            let cache = datars_build::fonts::acquire::default_cache_dir().map(|d| d.join("fonts"));
            if json {
                println!("{}", serde_json::json!({ "fonts": rows, "cache": cache }));
            } else {
                for r in &rows {
                    let source = r["source"].as_str().unwrap_or("(no source)");
                    let face = r["face"].as_str().unwrap_or("-");
                    let status = if r["loaded"].as_bool() == Some(true) { format!("{face}, {}", r["licence"].as_str().unwrap_or("?")) } else { format!("NOT LOADED — drawn with {face}") };
                    println!("{:<14} {:<28} {:<4} {:<44} {status}", r["token"].as_str().unwrap_or(""), r["family"].as_array().map(|f| f.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(), r["weight"], source);
                }
                if let Some(c) = &cache {
                    println!("cache: {}", c.display());
                }
            }
        }
        "states" => {
            let names = engine.state_names();
            if json {
                println!("{}", serde_json::json!(names));
            } else {
                for (i, n) in names.iter().enumerate() {
                    println!("{i}  {n}");
                }
            }
        }
        "check" => {
            // Every state: a table or scale only one state uses must be checked too.
            for i in 0..engine.state_names().len().max(1) {
                let _ = engine.scene_for_state(i);
            }
            let d: Vec<String> = engine.diagnostics().iter().map(|d| d.message.clone()).collect();
            if json {
                println!("{}", serde_json::json!({ "diagnostics": d }));
            } else if d.is_empty() {
                println!("ok — no diagnostics");
            } else {
                for m in &d {
                    println!("• {m}");
                }
            }
            if !d.is_empty() {
                return Err(format!("{} diagnostic(s)", d.len()));
            }
        }
        "explain" => {
            // Why an element exists and looks the way it does: recipes, template, data row,
            // expression values, intents.
            let key = a.flags.get("key").or(a.pos.get(1)).ok_or("usage: datars explain <doc> --key '(\"SE\",)' [--state N]")?;
            let s = state_index(&engine, a.flags.get("state"));
            engine.goto(s);
            let found = engine.explain(key);
            if json {
                println!("{}", serde_json::to_string_pretty(&found).unwrap_or_default());
            } else {
                for e in &found {
                    println!("{}", e.origin.path);
                    println!("  drawn     {}", e.node);
                    if let Some(b) = e.bounds {
                        println!("  bounds    x={:.1} y={:.1} w={:.1} h={:.1}", b[0], b[1], b[2], b[3]);
                    }
                    println!("  from      {}", if e.origin.recipes.is_empty() { "the document".to_string() } else { e.origin.recipes.join(" → ") });
                    if let Some(r) = &e.origin.row {
                        let fields: Vec<String> = r.fields.iter().map(|(k, v)| format!("{k}={v}")).collect();
                        println!("  row       {} #{}  {}", r.table, r.index, fields.join("  "));
                    }
                    println!("  template  {}", e.origin.template);
                    for v in &e.origin.values {
                        println!("    {:<16} {}  →  {}", v.field, v.expr, v.value);
                    }
                    if !e.intents.is_empty() {
                        println!("  intents   {}", e.intents.join(", "));
                    }
                    println!();
                }
            }
            if found.is_empty() {
                return Err(format!("no element at `{key}` in state {s} (paths as `datars inspect` prints them; baked T1 scenes can't be explained)"));
            }
        }
        "inspect" => {
            let s = state_index(&engine, a.flags.get("state"));
            let scene = engine.scene_for_state(s);
            print!("{}", scene.snapshot());
            eprintln!("hash {}", scene.hash_hex());
        }
        "semantics" => {
            let s = state_index(&engine, a.flags.get("state"));
            engine.goto(s);
            for (role, label, depth) in engine.semantics() {
                println!("{}{role}: {label}", "  ".repeat(depth));
            }
        }
        "render" => {
            let s = state_index(&engine, a.flags.get("state"));
            let out = a.flags.get("out").cloned().unwrap_or_else(|| format!("out/{stem}-{s}.png"));
            if let Some(dir) = Path::new(&out).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let mut pixel_hash = None;
            if out.ends_with(".svg") {
                std::fs::write(&out, datars_headless::svg_state(&mut engine, s)).map_err(|e| e.to_string())?;
            } else if out.ends_with(".pdf") {
                std::fs::write(&out, datars_headless::pdf_state(&mut engine, s)).map_err(|e| e.to_string())?;
            } else {
                let px = datars_headless::render_state(&mut engine, s, dpr);
                pixel_hash = Some(format!("{:016x}", px.hash()));
                std::fs::write(&out, px.to_png()).map_err(|e| e.to_string())?;
            }
            // `--hash`: the pixel identity, to compare targets (wasm, iOS, Android) with this one.
            if a.flags.contains_key("hash") {
                if let Some(h) = &pixel_hash {
                    println!("{h}");
                }
            }
            let diags = engine.diagnostics().len();
            if json {
                println!("{}", serde_json::json!({ "out": out, "state": s, "diagnostics": engine.diagnostics().iter().map(|d| &d.message).collect::<Vec<_>>() }));
            } else {
                println!("{out}{}", if diags > 0 { format!("  ({diags} diagnostics — run `datars check`)") } else { String::new() });
            }
        }
        "profile" => {
            // Where the time goes, per state and transition (release build; wall clock, median of 5).
            let fps: f64 = a.flags.get("fps").and_then(|f| f.parse().ok()).unwrap_or(60.0);
            let p = perf::profile(&mut engine, dpr, fps, a.flags.contains_key("cpu"));
            if json {
                println!("{}", serde_json::to_string_pretty(&p.to_json()).unwrap_or_default());
                return Ok(());
            }
            println!("{:<28} {:>10} {:>10} {:>10} {:>10}", "", "cold ms", "warm ms", "flatten", "cpu px");
            for s in &p.states {
                println!("state {:<22} {:>10.2} {:>10.2} {:>10.2} {:>10.2}   ({} nodes, {} ops)", s.name, s.cold, s.warm, s.flatten, s.cpu_raster, s.nodes, s.ops);
            }
            // Every transition, stepped frame by frame as a 60 Hz display shows it (perf.rs).
            println!("\ntransitions (stepped at {fps} fps, {} raster, dpr {dpr}):", p.raster);
            for t in &p.transitions {
                perf::print(t, p.raster, a.flags.contains_key("timeline"));
            }
            tools::profile_explore(&source, doc_dir(path), &mut engine, &load)?;
        }
        "video" => {
            // The program as a film: CPU frames piped to ffmpeg (H.264), captions as WebVTT.
            let fps: f64 = a.flags.get("fps").and_then(|f| f.parse().ok()).unwrap_or(30.0);
            let hold: f64 = a.flags.get("hold").and_then(|f| f.parse().ok()).unwrap_or(2.5);
            let dpr: f64 = a.flags.get("dpr").and_then(|d| d.parse().ok()).unwrap_or(1.0);
            let out = a.flags.get("out").cloned().unwrap_or_else(|| format!("out/{stem}.mp4"));
            if let Some(dir) = Path::new(&out).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let vp = engine.viewport();
            // H.264 in yuv420p needs even dimensions.
            let (w, h) = ((((vp.width * dpr).round() as u32) + 1) & !1, (((vp.height * dpr).round() as u32) + 1) & !1);
            let mut ff = std::process::Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"])
                .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-crf", "18", "-movflags", "+faststart", &out])
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("running ffmpeg (needed for video): {e}"))?;
            let mut stdin = ff.stdin.take().ok_or("ffmpeg stdin")?;
            let mut err: Option<String> = None;
            let mut frames = 0usize;
            let mut buf: Vec<u8> = Vec::new();
            let cues = {
                datars_headless::film(&mut engine, fps, hold, |f| {
                    if err.is_some() {
                        return;
                    }
                    let px = datars_render_cpu::render(f.list, f.fonts, dpr);
                    buf.clear();
                    // Pad to the even frame size with the last row/column.
                    for y in 0..h {
                        let sy = y.min(px.height - 1);
                        let row = &px.data[(sy * px.width * 4) as usize..((sy + 1) * px.width * 4) as usize];
                        buf.extend_from_slice(row);
                        for _ in px.width..w {
                            buf.extend_from_slice(&row[row.len() - 4..]);
                        }
                    }
                    for _ in 0..f.repeat {
                        if let Err(e) = std::io::Write::write_all(&mut stdin, &buf) {
                            err = Some(e.to_string());
                            return;
                        }
                        frames += 1;
                    }
                })
            };
            drop(stdin);
            let status = ff.wait().map_err(|e| e.to_string())?;
            if let Some(e) = err {
                return Err(format!("ffmpeg: {e}"));
            }
            if !status.success() {
                return Err("ffmpeg failed".into());
            }
            let vtt = Path::new(&out).with_extension("vtt");
            std::fs::write(&vtt, datars_headless::webvtt(&cues)).map_err(|e| e.to_string())?;
            println!("{out}  ({frames} frames, {:.1} s at {fps} fps, {w}×{h})\n{}", frames as f64 / fps, vtt.display());
        }
        "film" => {
            let from = state_index(&engine, a.flags.get("from"));
            let to = a.flags.get("to").map(|v| state_index(&engine, Some(v))).unwrap_or(from + 1);
            let frames: usize = a.flags.get("frames").and_then(|f| f.parse().ok()).unwrap_or(6);
            let prefix = a.flags.get("out").cloned().unwrap_or_else(|| format!("out/{stem}-{from}-{to}"));
            if let Some(dir) = Path::new(&prefix).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            let strip = datars_headless::filmstrip(&mut engine, from, to, frames, dpr.min(1.0));
            std::fs::write(format!("{prefix}-strip.png"), strip.to_png()).map_err(|e| e.to_string())?;
            let tr = datars_headless::trails(&mut engine, from, to, 24, dpr);
            std::fs::write(format!("{prefix}-trails.png"), tr.to_png()).map_err(|e| e.to_string())?;
            if json {
                println!("{}", serde_json::json!({ "strip": format!("{prefix}-strip.png"), "trails": format!("{prefix}-trails.png") }));
            } else {
                println!("{prefix}-strip.png\n{prefix}-trails.png");
            }
        }
        "bundle" => {
            let out = a.flags.get("out").cloned().unwrap_or_else(|| format!("out/{stem}.datars"));
            if let Some(dir) = Path::new(&out).parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            // Automatic basemaps and point archives travel next to the bundle file (`<stem>.<source>.pmtiles`).
            let out_stem = Path::new(&out).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| stem.clone());
            let shipped = ship_basemaps(path, &source, &|src, _| format!("{out_stem}.{src}.pmtiles"))?;
            let f = fonts();
            let (mut bundle, report) = datars_build::build_with(&shipped.json, &datars_build::Options::default(), &|r| shipped.fetch(r, doc_dir(path), &f))?;
            let signer = sign_bundle(&mut bundle, &a.flags)?;
            std::fs::write(&out, datars_bundle::to_single_file(&bundle)).map_err(|e| e.to_string())?;
            for (url, file) in &shipped.files {
                let to = Path::new(&out).parent().unwrap_or(Path::new(".")).join(url);
                std::fs::copy(file, &to).map_err(|e| format!("{}: {e}", to.display()))?;
            }
            if json {
                println!("{}", serde_json::json!({ "out": out, "bytes": report.bytes_by_tier, "gzip": report.gzip_by_tier, "chunks": report.chunks, "decisions": report.decisions, "publisher": signer }));
            } else {
                println!("{out}");
                if let Some(p) = &signer {
                    println!("  signed by {p}");
                }
                for (t, b) in &report.bytes_by_tier {
                    println!("  {t}: {b} bytes, {} gzipped", report.gzip_by_tier.get(t).copied().unwrap_or(0));
                }
                if a.flags.contains_key("explain") {
                    for (kind, raw, gz) in &report.chunks {
                        println!("    {kind:<10} {raw:>8} bytes  {gz:>7} gzipped");
                    }
                }
                for d in &report.decisions {
                    println!("  · {d}");
                }
            }
        }
        "publish" => {
            let dir = a.flags.get("to").map(PathBuf::from).unwrap_or_else(|| repo_root().join("out/site"));
            let alias = a.flags.get("alias").cloned().unwrap_or_else(|| stem.clone());
            let opts = datars_build::Options { revision: a.flags.get("revision").cloned().unwrap_or_else(|| "dev".into()), ..Default::default() };
            // Automatic basemaps and point archives ship content-addressed beside the charts (`tiles/<source>.<hash>.pmtiles`),
            // so republishing never breaks a page that has the old one open.
            let shipped = ship_basemaps(path, &source, &|src, hash| format!("../tiles/{src}.{hash}.pmtiles"))?;
            let f = fonts();
            let (mut bundle, report) = datars_build::build_with(&shipped.json, &opts, &|r| shipped.fetch(r, doc_dir(path), &f))?;
            let signer = sign_bundle(&mut bundle, &a.flags)?;
            let (written, present) = serve::publish(&bundle, &dir, &alias)?;
            let mut copied = serve::copy_referenced(&source, doc_dir(path), &dir)?;
            for (url, file) in &shipped.files {
                let rel = url.trim_start_matches("../");
                let to = dir.join(rel);
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                std::fs::copy(file, &to).map_err(|e| format!("{}: {e}", to.display()))?;
                copied.push(rel.to_string());
            }
            if json {
                println!("{}", serde_json::json!({ "alias": alias, "dir": dir, "written": written, "present": present, "copied": copied, "gzip": report.gzip_by_tier, "publisher": signer }));
            } else {
                println!("{}/c/{alias}  ({written} chunks written, {present} already there)", dir.display());
                if let Some(p) = &signer {
                    println!("  signed by {p}");
                }
                for c in &copied {
                    println!("  + {c} (fetched by URL at runtime)");
                }
            }
        }
        other => return Err(format!("unknown command `{other}` (see `datars help`)")),
    }
    Ok(())
}

/// `datars keygen`: a new ed25519 signing key, written as hex to `out` (never over an existing file;
/// owner-only on Unix). The public key it prints is what a page names in `<datars-view publishers>`.
fn keygen(out: &str, json: bool) -> Result<(), String> {
    if Path::new(out).exists() {
        return Err(format!("{out} exists: a signing key is never overwritten (choose another --out)"));
    }
    let mut secret = [0u8; 32];
    getrandom::getrandom(&mut secret).map_err(|e| format!("no randomness from the OS: {e}"))?;
    let text: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(out).map_err(|e| format!("{out}: {e}"))?;
        writeln!(f, "{text}").map_err(|e| format!("{out}: {e}"))?;
    }
    #[cfg(not(unix))]
    std::fs::write(out, format!("{text}\n")).map_err(|e| format!("{out}: {e}"))?;
    let public = datars_bundle::public_key_for(&secret);
    if json {
        println!("{}", serde_json::json!({ "key": out, "publisher": public }));
    } else {
        println!("{out}  (secret: keep it out of version control)\n{public}\n\nSign with `datars publish doc.ts --sign {out}`; pages that only play your charts:\n  <datars-view src=\"…\" publishers=\"{public}\"></datars-view>");
    }
    Ok(())
}

/// `--sign <key file>` (or `DATARS_SIGNING_KEY`, the key's hex, for CI): sign the bundle's manifest.
/// Returns the publisher id it's signed with.
fn sign_bundle(bundle: &mut datars_bundle::Bundle, flags: &BTreeMap<String, String>) -> Result<Option<String>, String> {
    let hex = match flags.get("sign") {
        Some(file) => std::fs::read_to_string(file).map_err(|e| format!("--sign {file}: {e}"))?,
        None => match std::env::var("DATARS_SIGNING_KEY") {
            Ok(k) if !k.trim().is_empty() => k,
            _ => return Ok(None),
        },
    };
    let hex = hex.trim();
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("a signing key is 64 hex digits (see `datars keygen`)".into());
    }
    let mut secret = [0u8; 32];
    for (i, b) in secret.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
    }
    bundle.manifest.sign(&secret);
    Ok(bundle.manifest.publisher.clone())
}

fn main() -> ExitCode {
    match run(parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
