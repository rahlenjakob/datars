//! `datars-mcp` — datars for coding agents over the Model Context Protocol (stdio, JSON-RPC 2.0).
//! Every tool is deterministic and needs no GPU or browser; images are written to files whose
//! paths are returned, so an agent can look at them (docs/14-devtools-and-agents.md).

use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn tools() -> Value {
    let doc = json!({ "type": "string", "description": "Path to a document (.json IR or .ts using @datars/sdk)" });
    json!([
        { "name": "check", "description": "Load a document and report diagnostics (codes, messages).", "inputSchema": { "type": "object", "properties": { "doc": doc }, "required": ["doc"] } },
        { "name": "render", "description": "Render one state with the CPU reference rasterizer; returns the image (attached), its path and the scene hash.", "inputSchema": { "type": "object", "properties": { "doc": doc, "state": { "type": "integer" }, "width": { "type": "number" }, "height": { "type": "number" }, "dpr": { "type": "number", "description": "device pixel ratio (default 1)" } }, "required": ["doc"] } },
        { "name": "film", "description": "Filmstrip and motion-trail images of a transition between two states.", "inputSchema": { "type": "object", "properties": { "doc": doc, "from": { "type": "integer" }, "to": { "type": "integer" }, "frames": { "type": "integer" } }, "required": ["doc"] } },
        { "name": "inspect", "description": "The scene of a state as a stable text snapshot (keys, geometry, inks, roles).", "inputSchema": { "type": "object", "properties": { "doc": doc, "state": { "type": "integer" } }, "required": ["doc"] } },
        { "name": "lint", "description": "Data-graphics lint: identity, encoding, legibility, accessibility, theme contrast.", "inputSchema": { "type": "object", "properties": { "doc": doc }, "required": ["doc"] } },
        { "name": "semantics", "description": "The accessibility tree of a state (what a screen reader reads).", "inputSchema": { "type": "object", "properties": { "doc": doc, "state": { "type": "integer" } }, "required": ["doc"] } },
        { "name": "explain", "description": "Why an element exists and looks the way it does: the recipes that expanded into it, its template, the data row, every expression with its value, bounds and intents. `key` is a key path as `inspect` prints it, or its end (`(\"SE\",)`).", "inputSchema": { "type": "object", "properties": { "doc": doc, "key": { "type": "string" }, "state": { "type": "integer" } }, "required": ["doc", "key"] } },
        { "name": "diff", "description": "Element-level scene diff: nodes added, removed and changed (with their before/after snapshot lines) between two states of a document (`from`, `to`), or between two documents (`doc`, `other`) at `state`.", "inputSchema": { "type": "object", "properties": { "doc": doc, "other": { "type": "string" }, "from": { "type": "integer" }, "to": { "type": "integer" }, "state": { "type": "integer" } }, "required": ["doc"] } },
        { "name": "data_profile", "description": "Profile a CSV/JSON data file before charting it: column types, missing values, cardinality, ranges, candidate keys, and hints (years as numbers, atlas ids, log-scale spans, categories).", "inputSchema": { "type": "object", "properties": { "file": { "type": "string" } }, "required": ["file"] } },
        { "name": "describe_recipes", "description": "Every standard-library recipe with its parameters, docs and tokens.", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "run_tests", "description": "Run the visual + motion suite over examples/ (snapshots, dense transition sweeps, invariants).", "inputSchema": { "type": "object", "properties": { "filter": { "type": "string" }, "update": { "type": "boolean" } } } },
        { "name": "bundle", "description": "Publish a document to a bundle (.datars) and report variants, bytes, gzip bytes per tier and per chunk, and the compiler's decisions.", "inputSchema": { "type": "object", "properties": { "doc": doc, "out": { "type": "string" } }, "required": ["doc"] } },
        { "name": "replay", "description": "Replay a recorded host session (JSON from View.take_recording / Engine::take_recording) exactly; returns a filmstrip (attached) and the final state.", "inputSchema": { "type": "object", "properties": { "session": { "type": "string", "description": "path to the session JSON" }, "frames": { "type": "integer" } }, "required": ["session"] } }
    ])
}

fn root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    for p in cwd.ancestors() {
        if p.join("scripts/doc-to-json.mjs").exists() {
            return p.to_path_buf();
        }
        if p.join("public/scripts/doc-to-json.mjs").exists() {
            return p.join("public");
        }
    }
    cwd
}

fn read_doc(path: &str) -> Result<String, String> {
    if path.ends_with(".ts") {
        let out = std::process::Command::new("node").arg(root().join("scripts/doc-to-json.mjs")).arg(path).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))
}

fn call(name: &str, a: &Value) -> Result<Value, String> {
    let doc_path = a["doc"].as_str().unwrap_or("");
    let out_dir = root().join("out/mcp");
    let _ = std::fs::create_dir_all(&out_dir);
    let stem = doc_path.replace(['/', '.'], "_");
    let fonts = datars_build::fonts::acquire::Fonts::from_env();
    let load = || -> Result<datars_engine::Engine, String> {
        let json = read_doc(doc_path)?;
        let dir = std::path::Path::new(doc_path).parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
        // Automatic basemaps (`tiles: "auto"`) are made before anything draws them.
        datars_build::auto::ensure_default(&json, dir)?;
        datars_headless::load_at_with(&json, Some(dir), &datars_build::fonts::acquire::fetch_with(&fonts, Some(dir)))
    };
    Ok(match name {
        "check" => {
            let mut e = load()?;
            let _ = e.scene();
            json!({ "states": e.state_names(), "diagnostics": e.diagnostics().iter().map(|d| d.message.clone()).collect::<Vec<_>>() })
        }
        "render" => {
            let mut e = load()?;
            if let (Some(w), Some(h)) = (a["width"].as_f64(), a["height"].as_f64()) {
                e.resize(w, h, 1.0);
            }
            let s = a["state"].as_u64().unwrap_or(0) as usize;
            let scene = e.scene_for_state(s);
            let px = datars_headless::render_scene(&e, &scene, a["dpr"].as_f64().unwrap_or(1.0));
            let p = out_dir.join(format!("{stem}-{s}.png"));
            std::fs::write(&p, px.to_png()).map_err(|e| e.to_string())?;
            json!({ "image": p, "scene_hash": scene.hash_hex(), "diagnostics": e.diagnostics().iter().map(|d| d.message.clone()).collect::<Vec<_>>() })
        }
        "film" => {
            let mut e = load()?;
            let from = a["from"].as_u64().unwrap_or(0) as usize;
            let to = a["to"].as_u64().map(|t| t as usize).unwrap_or(from + 1);
            let frames = a["frames"].as_u64().unwrap_or(6) as usize;
            let strip = datars_headless::filmstrip(&mut e, from, to, frames, 1.0);
            let trails = datars_headless::trails(&mut e, from, to, 24, 1.5);
            let sp = out_dir.join(format!("{stem}-{from}-{to}-strip.png"));
            let tp = out_dir.join(format!("{stem}-{from}-{to}-trails.png"));
            std::fs::write(&sp, strip.to_png()).map_err(|e| e.to_string())?;
            std::fs::write(&tp, trails.to_png()).map_err(|e| e.to_string())?;
            json!({ "filmstrip": sp, "trails": tp })
        }
        "replay" => {
            let path = a["session"].as_str().ok_or("`session` (a path) is required")?;
            let session = datars_engine::session::Session::from_json(&std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?)?;
            let frames = session.inputs.iter().filter(|i| matches!(i, datars_engine::session::Input::Frame { .. })).count();
            let want = a["frames"].as_u64().unwrap_or(6).max(1) as usize;
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
            let imgs: Vec<_> = shots.iter().map(|l| datars_render_cpu::render(l, engine.fonts(), 1.0)).collect();
            // Half size: a strip is for seeing what happened, and agent context is precious.
            let (w, h) = imgs.first().map(|p| (p.width / 2, p.height / 2)).unwrap_or((1, 1));
            let n = imgs.len() as u32;
            let mut strip = datars_render_cpu::Pixmap { width: w * n + 4 * n.saturating_sub(1), height: h, data: vec![235; ((w * n + 4 * n.saturating_sub(1)) * h * 4) as usize] };
            for (i, img) in imgs.iter().enumerate() {
                datars_headless::blit(&mut strip, img, i as u32 * (w + 4), 0, 2);
            }
            let p = out_dir.join("replay-strip.png");
            std::fs::write(&p, strip.to_png()).map_err(|e| e.to_string())?;
            json!({ "strip": p, "inputs": session.inputs.len(), "frames": frames, "final_state": engine.state(), "diagnostics": engine.diagnostics().iter().map(|d| d.message.clone()).collect::<Vec<_>>() })
        }
        "inspect" => {
            let mut e = load()?;
            let s = a["state"].as_u64().unwrap_or(0) as usize;
            let scene = e.scene_for_state(s);
            json!({ "snapshot": scene.snapshot(), "hash": scene.hash_hex() })
        }
        "lint" => {
            let mut e = load()?;
            json!({ "findings": datars_devtools::lint(&mut e) })
        }
        "semantics" => {
            let mut e = load()?;
            e.goto(a["state"].as_u64().unwrap_or(0) as usize);
            json!({ "tree": e.semantics().into_iter().map(|(r, l, d)| json!({ "role": r, "label": l, "depth": d })).collect::<Vec<_>>() })
        }
        "explain" => {
            let mut e = load()?;
            e.goto(a["state"].as_u64().unwrap_or(0) as usize);
            json!({ "elements": e.explain(a["key"].as_str().unwrap_or("")) })
        }
        "diff" => {
            let mut e = load()?;
            let (a, b) = match a["other"].as_str() {
                Some(other) => {
                    let s = a["state"].as_u64().unwrap_or(0) as usize;
                    let dir = std::path::Path::new(other).parent();
                    let mut o = datars_headless::load_at_with(&read_doc(other)?, dir, &datars_build::fonts::acquire::fetch_with(&fonts, dir))?;
                    (e.scene_for_state(s), o.scene_for_state(s))
                }
                None => {
                    let from = a["from"].as_u64().unwrap_or(0) as usize;
                    let to = a["to"].as_u64().map(|t| t as usize).unwrap_or(from + 1);
                    (e.scene_for_state(from), e.scene_for_state(to))
                }
            };
            scene_diff(&a, &b)
        }
        "data_profile" => {
            let file = a["file"].as_str().ok_or("`file` (a path) is required")?;
            let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let mut ids = datars_devtools::profile::IdSets::new();
            if let Ok(atlas) = std::fs::read(root().join("assets/atlas/countries.geojson")) {
                let v: Value = serde_json::from_slice(&atlas).unwrap_or_default();
                ids.insert("countries".into(), v["features"].as_array().into_iter().flatten().filter_map(|f| f["properties"]["id"].as_str().map(String::from)).collect());
            }
            serde_json::to_value(datars_devtools::profile::profile_bytes(file, &bytes, &ids)?).unwrap_or_default()
        }
        "describe_recipes" => json!(datars_devtools::describe_std(&datars_engine::Engine::new())),
        "run_tests" => {
            let r = datars_test::run(&datars_test::Options { root: root(), filter: a["filter"].as_str().unwrap_or("").into(), update: a["update"].as_bool().unwrap_or(false), samples: 64, pixels: true });
            serde_json::to_value(&r).unwrap_or_default()
        }
        "bundle" => {
            let dir = std::path::Path::new(doc_path).parent();
            let (b, report) = datars_build::build_with(&read_doc(doc_path)?, &datars_build::Options::default(), &datars_build::fonts::acquire::fetch_with(&fonts, dir))?;
            let out = a["out"].as_str().map(PathBuf::from).unwrap_or_else(|| out_dir.join(format!("{stem}.datars")));
            std::fs::write(&out, datars_bundle::to_single_file(&b)).map_err(|e| e.to_string())?;
            json!({ "out": out, "bytes_by_tier": report.bytes_by_tier, "gzip_by_tier": report.gzip_by_tier, "chunks": report.chunks, "decisions": report.decisions })
        }
        _ => return Err(format!("unknown tool {name}")),
    })
}

/// Nodes added, removed and changed between two scenes, by key path (as `datars diff`).
fn scene_diff(a: &datars_scene::Scene, b: &datars_scene::Scene) -> Value {
    fn lines(n: &datars_scene::Node, path: &str, out: &mut std::collections::BTreeMap<String, String>) {
        let here = if path.is_empty() { n.key.to_string() } else { format!("{path}/{}", n.key) };
        out.insert(here.clone(), n.snapshot_line());
        n.children().iter().for_each(|c| lines(c, &here, out));
    }
    let (mut la, mut lb) = (Default::default(), Default::default());
    lines(&a.root, "", &mut la);
    lines(&b.root, "", &mut lb);
    let la: std::collections::BTreeMap<String, String> = la;
    let lb: std::collections::BTreeMap<String, String> = lb;
    json!({
        "added": lb.iter().filter(|(k, _)| !la.contains_key(*k)).map(|(k, l)| json!({ "path": k, "node": l })).collect::<Vec<_>>(),
        "removed": la.iter().filter(|(k, _)| !lb.contains_key(*k)).map(|(k, l)| json!({ "path": k, "node": l })).collect::<Vec<_>>(),
        "changed": la.iter().filter_map(|(k, l)| lb.get(k).filter(|m| *m != l).map(|m| json!({ "path": k, "from": l, "to": m }))).collect::<Vec<_>>(),
    })
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let id = req.get("id").cloned();
        let method = req["method"].as_str().unwrap_or("");
        let result: Option<Result<Value, String>> = match method {
            "initialize" => Some(Ok(json!({ "protocolVersion": "2024-11-05", "capabilities": { "tools": {} }, "serverInfo": { "name": "datars", "version": datars_engine::VERSION } }))),
            "tools/list" => Some(Ok(json!({ "tools": tools() }))),
            "tools/call" => {
                let name = req["params"]["name"].as_str().unwrap_or("");
                let args = req["params"]["arguments"].clone();
                Some(Ok(match call(name, &args) {
                    Ok(v) => {
                        // Images an agent should see: attached as image content, not just paths.
                        let mut content = vec![json!({ "type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default() })];
                        for key in ["image", "filmstrip", "trails", "strip"] {
                            if let Some(bytes) = v[key].as_str().and_then(|p| std::fs::read(p).ok()) {
                                content.push(json!({ "type": "image", "mimeType": "image/png", "data": datars_engine::session::base64(&bytes) }));
                            }
                        }
                        json!({ "content": content })
                    }
                    Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                }))
            }
            "ping" => Some(Ok(json!({}))),
            _ if id.is_none() => None, // notifications
            _ => Some(Err(format!("method not found: {method}"))),
        };
        if let (Some(id), Some(r)) = (id, result) {
            let msg = match r {
                Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
                Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": e } }),
            };
            let _ = writeln!(stdout, "{msg}");
            let _ = stdout.flush();
        }
    }
}
