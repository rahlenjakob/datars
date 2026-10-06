//! `datars-build` — the publish compiler (docs/12-delivery.md). Partially evaluates a document into a
//! bundle with variants an installed runtime chooses from:
//!
//! | tier | contents | runtime needs |
//! |---|---|---|
//! | T0 | SVG poster of the first state, accessible text (summary, per-state descriptions) | nothing |
//! | T1 | a scene per state (resolved here), program, motion rules, narration | `core` |
//! | T2 | the document with recipes pre-expanded + its data | `core`, `graph` |
//! | T3 | the source document (+ package sources) | `core`, `graph`, `sandbox` |

pub mod auto;
pub mod fonts;
pub mod points;

use datars_bundle::{Builder, Bundle, Requires, Tier};
use datars_engine::{Engine, Request};
use std::collections::{BTreeMap, BTreeSet};

pub struct Options {
    pub revision: String,
    /// Oldest runtime version to support (compiled into `requires`).
    pub runtime_floor: String,
    /// Sign with this ed25519 secret key.
    pub sign: Option<[u8; 32]>,
    pub tiers: Vec<Tier>,
}

impl Default for Options {
    fn default() -> Self {
        Options { revision: "dev".into(), runtime_floor: "0.1".into(), sign: None, tiers: vec![Tier::T0, Tier::T1, Tier::T2, Tier::T3] }
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub decisions: Vec<String>,
    pub bytes_by_tier: BTreeMap<String, u64>,
    /// Compressed (gzip -9) bytes per tier: what a reader downloads (the size targets' unit).
    pub gzip_by_tier: BTreeMap<String, u64>,
    /// Every chunk: (kind, raw bytes, gzip bytes).
    pub chunks: Vec<(String, u64, u64)>,
}

/// gzip -9 size: HTTP serves chunks compressed; the size targets are in these bytes.
pub fn gzip_len(bytes: &[u8]) -> u64 {
    use std::io::Write;
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    let _ = e.write_all(bytes);
    e.finish().map(|v| v.len() as u64).unwrap_or(bytes.len() as u64)
}

fn meta(pairs: &[(&str, serde_json::Value)]) -> BTreeMap<String, serde_json::Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// Compile a document (JSON IR) into a bundle. Requests for external data stay unfulfilled; see
/// [`build_with`].
pub fn build(doc_json: &str, opts: &Options) -> Result<(Bundle, Report), String> {
    build_with(doc_json, opts, &|_| None)
}

/// Fetch the tile ranges every state's views need (the poster and baked scenes show the basemap),
/// the way a host would: resolve, serve the requests, repeat until nothing is missing.
fn settle_tiles(engine: &mut Engine, fetch: &dyn Fn(&Request) -> Option<Vec<u8>>) {
    let n = engine.state_names().len();
    for _ in 0..8 {
        for i in 0..n {
            engine.scene_for_state(i);
        }
        let ranges: Vec<Request> = engine.requests().into_iter().filter(|r| matches!(r, Request::Range { .. })).collect();
        if ranges.is_empty() {
            return;
        }
        let mut served = 0;
        for r in ranges {
            if let (Request::Range { name, offset, .. }, Some(bytes)) = (&r, fetch(&r)) {
                if engine.provide_range(name, *offset, &bytes).is_ok() {
                    served += 1;
                }
            }
        }
        if served == 0 {
            return;
        }
    }
}

/// Polygon GeoJSON (atlases, region files) ships as a decimal topology when that is smaller:
/// shared borders stored once, integer deltas. Chosen only after checking the topology decodes to
/// exactly the same features — ids, properties and every coordinate — so charts are bit-identical.
fn compact_geo(name: &str, bytes: Vec<u8>, report: &mut Report) -> Vec<u8> {
    let opts = datars_geo::GeoJsonOptions::default();
    let looks_geojson = bytes.windows(17).any(|w| w == b"FeatureCollection");
    if !looks_geojson {
        return bytes;
    }
    let Ok(fc) = datars_geo::parse_geojson(&bytes, &opts) else { return bytes };
    let Some(d) = datars_geo::topo_encode::decimals_of(&fc) else { return bytes };
    let Some(topo) = datars_geo::topo_encode::encode_decimal(&fc, d, name) else { return bytes };
    let same = datars_geo::parse_topojson(topo.as_bytes(), &opts).ok().and_then(|t| t.features(name)).is_some_and(|back| back == fc);
    let (before, after) = (gzip_len(&bytes), gzip_len(topo.as_bytes()));
    if !same || after >= before {
        return bytes;
    }
    report.decisions.push(format!("geo `{name}`: {} KB → {} KB gzipped as a decimal topology (shared borders, {d} decimals, exact)", before / 1024, after / 1024));
    topo.into_bytes()
}

/// Compile a document, fetching what it requests (atlases, data files) through `fetch`. Fetched
/// bytes are baked into T1 scenes and shipped as `data` chunks with T2/T3 — content-addressed, so
/// an atlas is downloaded once for every bundle that uses it.
pub fn build_with(doc_json: &str, opts: &Options, fetch: &dyn Fn(&Request) -> Option<Vec<u8>>) -> Result<(Bundle, Report), String> {
    let doc = datars_ir::Doc::from_json(doc_json)?;
    let mut engine = Engine::new();
    let diags = engine.load(doc.clone());
    let mut report = Report::default();
    for d in diags {
        report.decisions.push(format!("diagnostic: {}", d.message));
    }
    let mut b = Builder::new();
    let mut data_chunks = Vec::new();
    // Fonts: where each came from (credits), the document's font sources, and whether text can
    // arrive after publishing (host data, live feeds, streamed tiles) — then script subsets ship too.
    let mut font_from: BTreeMap<String, String> = BTreeMap::new();
    let mut font_sources: BTreeMap<String, (String, Vec<u8>)> = BTreeMap::new();
    let mut data_bytes: Vec<Vec<u8>> = Vec::new();
    let mut runtime_text = doc.data.values().any(|s| s.live.is_some());
    for req in engine.requests() {
        let name = match &req {
            Request::Source { name, .. } | Request::Slot { name, .. } | Request::Atlas { name, .. } => name.clone(),
            Request::Range { name, url, .. } => {
                // Tile archives are never shipped: the runtime reads the ranges its views need.
                report.decisions.push(format!("tiles `{name}`: streamed from {url} at runtime (range requests)"));
                runtime_text = true;
                continue;
            }
        };
        if matches!(req, Request::Slot { .. }) {
            report.decisions.push(format!("data slot `{name}`: filled by the host at runtime"));
            runtime_text = true;
            continue;
        }
        let url = match &req {
            Request::Source { url, .. } => url.clone(),
            _ => String::new(),
        };
        let is_font = name.starts_with("font:") || matches!(doc.data.get(&name).map(|s| &s.from), Some(datars_ir::SourceKind::Font(_)));
        match fetch(&req) {
            Some(bytes) if is_font => {
                let bytes = match fonts::acquire::decode_container(&bytes) {
                    Ok(b) => b,
                    Err(e) => {
                        report.decisions.push(format!("warning: font `{name}` ({url}): {e}"));
                        continue;
                    }
                };
                engine.provide(&name, &bytes)?;
                match name.strip_prefix("font:") {
                    Some(id) => {
                        font_from.insert(id.to_string(), url);
                    }
                    None => {
                        font_sources.insert(name.clone(), (url, bytes));
                    }
                }
            }
            Some(bytes) => {
                let bytes = compact_geo(&name, bytes, &mut report);
                engine.provide(&name, &bytes)?;
                report.decisions.push(format!("data `{name}`: {} KB gzipped, shipped with T2/T3 (shared by hash)", gzip_len(&bytes) / 1024));
                data_bytes.push(bytes.clone());
                data_chunks.push(b.chunk("data", bytes, meta(&[("source", name.into())])));
            }
            None if is_font => report.decisions.push(format!("warning: font `{name}`: {url} could not be acquired; its text falls back (see `datars check`)")),
            None => {
                runtime_text = true;
                report.decisions.push(format!("data `{name}`: not available at build time ({req:?}); the runtime must fetch it"));
            }
        }
    }
    let floor = format!(">={}", opts.runtime_floor);
    settle_tiles(&mut engine, fetch);

    // Shared across tiers: the accessible text and the poster.
    let baked = engine.bake();
    let expanded = engine.expanded_doc();
    // Every face the chart draws with, cut to the characters it can show (docs/12-delivery.md).
    let doc_value = engine_text(serde_json::from_str(doc_json).unwrap_or_default());
    let expanded_value = engine_text(expanded.as_ref().map(|d| serde_json::to_value(d).unwrap_or_default()).unwrap_or_default());
    let case_changes = [&doc_value, &expanded_value].iter().any(|v| v.to_string().contains("toUpperCase") || v.to_string().contains("toLowerCase"));
    let interactive = doc.signals.values().any(|s| s.control.is_some()) || has_intents(&expanded_value) || has_intents(&doc_value);
    let full = {
        let mut c = fonts::coverage::Coverage::new();
        c.add_doc_text(&doc_value);
        c.add_doc_text(&expanded_value);
        for d in &data_bytes {
            c.add_bytes(d);
        }
        for (_, s) in &baked {
            c.add_scene(s);
        }
        // Month and weekday names only where dates are formatted (time axes, `formatDate`).
        let dates = [&doc_value, &expanded_value].iter().any(|v| {
            let s = v.to_string();
            s.contains("formatDate") || s.contains("\"time\"") || s.contains("\"utc\"") || s.contains("%b") || s.contains("%B") || s.contains("%a") || s.contains("%A")
        });
        c.add_locale(&doc.locale, dates);
        c.finish(case_changes)
    };
    let mut faces = faces_to_ship(&engine, &baked, &font_from, &font_sources);
    // What each face drew in the baked states. When that's everything it can draw — a program
    // of states, no interaction, no runtime text — a face gets only those characters (and the
    // number characters tweens pass through); the body face and fallback fonts, which draw data,
    // and every face of an interactive chart get the full set.
    let mut drawn: BTreeMap<String, BTreeSet<char>> = BTreeMap::new();
    for (_, s) in &baked {
        for (id, cs) in fonts::coverage::scene_text(s) {
            drawn.entry(id).or_default().extend(cs);
        }
    }
    let body: BTreeSet<String> = ["font.body", "font.number"].iter().filter_map(|t| engine.theme().font(t)).filter_map(|f| engine.fonts().face_id(&f.stack().join(", "), f.weight, f.italic)).collect();
    let resized = size_dependent_faces(&mut engine, &drawn);
    for f in &mut faces {
        f.chars = if interactive || runtime_text || f.source.is_some() || body.contains(&f.id) || resized.contains(&f.id) {
            full.clone()
        } else {
            let mut cs: BTreeSet<char> = drawn.get(&f.id).cloned().unwrap_or_default();
            // A number this face draws may tween through other digits (in the same format).
            if cs.iter().any(|c| c.is_ascii_digit()) {
                cs.extend("0123456789".chars());
            }
            cs.insert(' ');
            fonts::coverage::close(cs, case_changes)
        };
    }
    let packed = fonts::pack(&faces, runtime_text, &mut b);
    report.decisions.extend(packed.decisions.iter().cloned());
    // Fonts ship with every playable variant (T1's baked glyph runs need their outlines too);
    // script subsets only with the variants that resolve text on the device.
    let font_chunks = packed.eager.clone();
    let mut a11y = Vec::new();
    for (i, (name, _)) in baked.iter().enumerate() {
        engine.goto(i);
        let items: Vec<serde_json::Value> = engine.semantics().into_iter().map(|(role, label, depth)| serde_json::json!({ "role": role, "label": label, "depth": depth })).collect();
        a11y.push(serde_json::json!({ "state": name, "narration": engine.narration(), "items": items }));
    }
    engine.goto(0);
    let a11y_json = serde_json::json!({ "title": doc.title, "description": doc.description, "states": a11y });
    let text = b.chunk("a11y", serde_json::to_vec(&a11y_json).unwrap_or_default(), BTreeMap::new());
    // Point pyramids draw the poster from a tenth of their sample: it only stands in until the
    // runtime draws the real thing, and every dot costs bytes in an SVG.
    engine.set_point_scale(0.1);
    let poster_svg = datars_headless::svg_state_with(&mut engine, 0, &datars_render_svg::SvgOptions::poster());
    engine.set_point_scale(1.0);
    // Which mode the poster was drawn in (from its background, so a dark-only theme counts as
    // dark): a runtime showing the chart in the other mode skips it rather than flash it.
    let poster_mode = {
        let scene = engine.scene_for_state(0);
        let bg = engine.display_list(&scene).background;
        if bg.a >= 0.5 && bg.luminance() < 0.3 { "dark" } else { "light" }
    };
    let poster_svg = poster_svg.replacen("<svg ", &format!("<svg data-mode=\"{poster_mode}\" "), 1);
    let poster = b.chunk("poster", poster_svg.into_bytes(), meta(&[("format", "svg".into()), ("state", 0.into())]));
    let program = serde_json::json!({ "program": doc.program, "motion": doc.motion, "size": doc.size, "theme": doc.theme, "locale": doc.locale, "title": doc.title });

    if opts.tiers.contains(&Tier::T0) {
        b.variant(Tier::T0, Requires::default(), &[poster.clone(), text.clone()], serde_json::json!({ "title": doc.title }));
    }
    // Cost model: baked scenes are worth shipping when they're small (runtimes without the graph
    // module play them, and nothing has to resolve on the device). When they dwarf the source —
    // dense geometry resolved into every state — the variant costs more than it saves.
    let baked_json: Vec<String> = baked.iter().map(|(_, s)| s.to_json()).collect();
    let baked_gz: u64 = baked_json.iter().map(|j| gzip_len(j.as_bytes())).sum();
    let source_gz = gzip_len(doc_json.as_bytes()) + data_chunks.iter().filter_map(|h| b.bytes(h)).map(gzip_len).sum::<u64>();
    let t1 = opts.tiers.contains(&Tier::T1) && (baked_gz <= 64 * 1024 || baked_gz <= 3 * source_gz);
    if opts.tiers.contains(&Tier::T1) && !t1 {
        report.decisions.push(format!("T1 skipped: baked scenes {} KB vs source + data {} KB (gzipped)", baked_gz / 1024, source_gz / 1024));
    }
    if t1 {
        let mut chunks = vec![poster.clone(), text.clone()];
        chunks.extend(font_chunks.iter().cloned());
        let shell = b.chunk("program", serde_json::to_vec(&program).unwrap_or_default(), BTreeMap::new());
        chunks.push(shell);
        for (i, ((name, _), json)) in baked.iter().zip(&baked_json).enumerate() {
            let h = b.chunk("scene", json.clone().into_bytes(), meta(&[("state", name.clone().into()), ("index", i.into())]));
            chunks.push(h);
        }
        report.decisions.push(format!("T1: {} state(s) baked", baked.len()));
        b.variant(Tier::T1, Requires { runtime: floor.clone(), modules: vec!["core".into()], ..Default::default() }, &chunks, serde_json::json!({ "states": baked.iter().map(|(n, _)| n).collect::<Vec<_>>() }));
    }
    if opts.tiers.contains(&Tier::T2) {
        match expanded {
            Ok(expanded) => {
                let h = b.chunk("doc", expanded.to_json().into_bytes(), meta(&[("expanded", true.into())]));
                let mut chunks = vec![poster.clone(), text.clone(), h];
                chunks.extend(data_chunks.iter().cloned());
                chunks.extend(font_chunks.iter().cloned());
                chunks.extend(packed.lazy.iter().cloned());
                b.variant(Tier::T2, Requires { runtime: floor.clone(), modules: vec!["core".into(), "graph".into()], ..Default::default() }, &chunks, serde_json::json!({}));
                report.decisions.push("T2: recipes pre-expanded (no sandbox at runtime)".into());
            }
            Err(e) => report.decisions.push(format!("T2 skipped: {e}")),
        }
    }
    if opts.tiers.contains(&Tier::T3) {
        let h = b.chunk("doc", doc.to_json().into_bytes(), meta(&[("expanded", false.into())]));
        let mut chunks = vec![poster, text, h];
        chunks.extend(data_chunks.iter().cloned());
        chunks.extend(font_chunks.iter().cloned());
        chunks.extend(packed.lazy.iter().cloned());
        // Pinned to this build's std: a runtime with another one plays T2 instead (same output).
        b.variant(Tier::T3, Requires { runtime: floor, modules: vec!["core".into(), "graph".into(), "sandbox".into()], packages: datars_engine::bundle::packages() }, &chunks, serde_json::json!({}));
    }
    let id = if doc.id.is_empty() { "doc".to_string() } else { doc.id.clone() };
    let mut bundle = b.finish(&id, &opts.revision);
    bundle.manifest.title = (!doc.title.is_empty()).then(|| doc.title.clone());
    bundle.manifest.size = Some([doc.size.width, doc.size.height]);
    bundle.manifest.fonts = packed.credits;
    if let Some(key) = &opts.sign {
        bundle.manifest.sign(key);
    }
    let gz: BTreeMap<&String, u64> = bundle.chunks.iter().map(|(h, c)| (h, gzip_len(c))).collect();
    for v in &bundle.manifest.variants {
        let entry: serde_json::Value = serde_json::from_slice(&bundle.chunks[&v.entry]).unwrap_or_default();
        // Lazily loaded chunks (script subsets) aren't part of a first load.
        let hashes: Vec<&str> = entry["chunks"].as_array().into_iter().flatten().filter_map(|h| h.as_str()).filter(|h| !bundle.manifest.chunk(h).is_some_and(|c| c.lazy)).collect();
        let total: u64 = hashes.iter().filter_map(|h| bundle.chunks.get(*h)).map(|c| c.len() as u64).sum();
        let total_gz: u64 = hashes.iter().filter_map(|h| bundle.chunks.get_key_value(*h)).map(|(k, _)| gz[k]).sum::<u64>() + gz.get(&v.entry).copied().unwrap_or(0);
        report.bytes_by_tier.insert(format!("{:?}", v.tier), total);
        report.gzip_by_tier.insert(format!("{:?}", v.tier), total_gz);
    }
    for c in &bundle.manifest.chunks {
        if let Some(bytes) = bundle.chunks.get(&c.hash) {
            report.chunks.push((c.kind.clone(), bytes.len() as u64, gz[&c.hash]));
        }
    }
    Ok((bundle, report))
}

/// Sizes, relative to the authored one, a build probes for text that changes with size: phones,
/// narrower and wider columns, other aspect ratios.
const PROBE_SIZES: [(f64, f64); 4] = [(0.45, 0.8), (0.7, 0.9), (1.4, 1.1), (2.0, 1.4)];

/// Faces that draw characters at some probed size that they don't at the authored size — labels
/// shown when they fit, names that wrap or truncate. Hosts resize views, so such a face's text
/// isn't bounded by what the build baked; the caller gives it the full coverage.
fn size_dependent_faces(engine: &mut Engine, drawn: &BTreeMap<String, BTreeSet<char>>) -> BTreeSet<String> {
    let vp = engine.viewport();
    let mut out = BTreeSet::new();
    for (sx, sy) in PROBE_SIZES {
        engine.resize((vp.width * sx).round(), (vp.height * sy).round(), vp.dpr);
        for (_, s) in engine.bake() {
            for (id, cs) in fonts::coverage::scene_text(&s) {
                if !drawn.get(&id).is_some_and(|d| cs.is_subset(d)) {
                    out.insert(id);
                }
            }
        }
    }
    engine.resize(vp.width, vp.height, vp.dpr);
    out
}

/// A document without the text the engine never draws: its title, description and narration
/// are the host's (page chrome, narration cards, the accessible description).
fn engine_text(mut doc: serde_json::Value) -> serde_json::Value {
    if let Some(o) = doc.as_object_mut() {
        o.remove("title");
        o.remove("description");
        if let Some(states) = o.get_mut("program").and_then(|p| p.get_mut("states")).and_then(|s| s.as_array_mut()) {
            for st in states.iter_mut().filter_map(|s| s.as_object_mut()) {
                st.remove("narration");
            }
        }
    }
    doc
}

/// Does a (pre-expanded) document bind intents — clicks, drags, brushes — that can change what
/// the chart shows between the states the build baked?
fn has_intents(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(o) => o.iter().any(|(k, x)| (k == "on" && x.as_object().is_some_and(|m| !m.is_empty())) || has_intents(x)),
        serde_json::Value::Array(a) => a.iter().any(has_intents),
        _ => false,
    }
}

/// The faces a chart draws with: every face in its baked glyph runs, every face its theme's font
/// tokens resolve to (states the build didn't bake — hover emphasis, tweened weights — may use
/// them), and the faces of its font sources. Parts (`~script`) never appear at build time.
fn faces_to_ship(engine: &Engine, baked: &[(String, datars_scene::Scene)], font_from: &BTreeMap<String, String>, font_sources: &BTreeMap<String, (String, Vec<u8>)>) -> Vec<fonts::Face> {
    let db = engine.fonts();
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for (_, s) in baked {
        s.root.walk(&Default::default(), &mut |_, n| {
            if let datars_scene::NodeKind::Text(t) = &n.kind {
                ids.extend(t.runs.iter().map(|r| r.font.to_string()));
            }
        });
    }
    // Theme faces and where their tokens say they come from.
    let mut token_from: BTreeMap<String, String> = BTreeMap::new();
    for spec in engine.theme().fonts.values() {
        if let Some(id) = db.face_id(&spec.stack().join(", "), spec.weight, spec.italic) {
            if let (Some(u), Some(fam)) = (spec.source_url(), spec.primary()) {
                let token_id = if spec.italic { format!("{fam}-{}-Italic", spec.weight) } else { format!("{fam}-{}", spec.weight) };
                if token_id == id {
                    token_from.entry(id.clone()).or_insert(u);
                }
            }
            ids.insert(id);
        }
    }
    let infos = db.faces();
    // Font sources: the faces each file holds. Several files can share a family (a regular and a
    // bold), so a face belongs to the source whose file it came from, not to its family's last one.
    let mut source_of: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (name, (url, bytes)) in font_sources {
        for id in datars_text::FontDb::faces_in(bytes) {
            if infos.iter().any(|f| f.id == id) {
                ids.insert(id.clone());
                source_of.insert(id, (name.clone(), url.clone()));
            }
        }
    }
    ids.into_iter()
        .filter(|id| !id.contains('~'))
        .filter_map(|id| {
            let info = infos.iter().find(|f| f.id == id)?;
            let (bytes, index) = db.face_data(&id)?;
            let source = source_of.get(&id);
            let from = source.map(|(_, u)| u.clone()).or_else(|| font_from.get(&id).cloned()).or_else(|| token_from.get(&id).cloned()).or_else(|| datars_text::bundled_name(bytes).map(|n| format!("datars:fonts/{n}"))).unwrap_or_else(|| "the host".into());
            Some(fonts::Face { id: id.clone(), family: info.family.clone(), weight: info.weight, italic: info.italic, fs_type: info.fs_type, bytes: bytes.to_vec(), index, from, source: source.map(|(n, _)| n.clone()), chars: BTreeSet::new() })
        })
        .collect()
}

/// Open a bundle's chosen variant in an engine (what an installed runtime does after loading).
pub fn open(engine: &mut Engine, bundle: &Bundle, caps: &datars_bundle::Capabilities) -> Result<datars_bundle::Tier, String> {
    let v = bundle.manifest.select(caps).map_err(|e| e.to_string())?.clone();
    let entry: serde_json::Value = serde_json::from_slice(bundle.chunks.get(&v.entry).ok_or("missing entry")?).map_err(|e| e.to_string())?;
    datars_engine::bundle::open_with(engine, &bundle.manifest, v.tier, &entry, &|h| bundle.chunks.get(h).cloned())
}
