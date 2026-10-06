//! Opening bundles in a runtime (docs/12-delivery.md): given a loaded variant's chunks, set up the
//! engine — baked scenes for T1, a document for T2/T3. T0 needs no engine (poster + text).

use crate::Engine;
use datars_bundle::{Loader, Manifest, Tier};
use std::collections::BTreeMap;

/// The modules this runtime build offers (for variant selection).
pub fn modules() -> Vec<String> {
    let mut m = vec!["core".to_string(), "graph".to_string()];
    if cfg!(feature = "sandbox") {
        m.push("sandbox".into());
    }
    m
}

/// This runtime's version, for `requires.runtime`.
pub fn runtime_version() -> datars_bundle::Version {
    datars_bundle::Version::parse(crate::VERSION).unwrap_or(datars_bundle::Version(0, 1, 0))
}

pub fn capabilities(allow_script: bool, publishers: Vec<String>) -> datars_bundle::Capabilities {
    datars_bundle::Capabilities { runtime: runtime_version(), modules: modules(), allow_script, publishers, packages: packages() }
}

/// The built-in packages recipes expand with, by content hash: a T3 variant is pinned to the ones
/// it was published with, so every runtime that plays it draws what the author saw.
/// Runtimes without the sandbox have none (they don't play T3).
pub fn packages() -> BTreeMap<String, String> {
    #[cfg(feature = "sandbox")]
    return BTreeMap::from([
        ("@datars/sdk".to_string(), datars_bundle::chunk_hash(crate::expand::SDK_JS.as_bytes())),
        ("@datars/std".to_string(), datars_bundle::chunk_hash(crate::expand::STD_JS.as_bytes())),
    ]);
    #[cfg(not(feature = "sandbox"))]
    BTreeMap::new()
}

/// Open a variant whose entry and chunks are available through `get(hash)`.
pub fn open_with(engine: &mut Engine, manifest: &Manifest, tier: Tier, entry: &serde_json::Value, get: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Tier, String> {
    // Document, data and fonts in, then one resolve (not one per font and per source).
    engine.batch(|engine| open_parts(engine, manifest, tier, entry, get))
}

fn open_parts(engine: &mut Engine, manifest: &Manifest, tier: Tier, entry: &serde_json::Value, get: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<Tier, String> {
    let chunks = |kind: &str| -> Vec<(BTreeMap<String, serde_json::Value>, Vec<u8>)> {
        entry["chunks"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|h| h.as_str())
            .filter_map(|h| Some((manifest.chunk(h)?, get(h)?)))
            .filter(|(c, _)| c.kind == kind)
            .map(|(c, b)| (c.meta.clone(), b))
            .collect()
    };
    match tier {
        Tier::T0 => Ok(Tier::T0),
        Tier::T1 => {
            let (_, prog) = chunks("program").into_iter().next().ok_or("missing program chunk")?;
            let p: serde_json::Value = serde_json::from_slice(&prog).map_err(|e| e.to_string())?;
            let doc: datars_ir::Doc = serde_json::from_value(serde_json::json!({
                "datars": 1, "title": p["title"], "size": p["size"], "theme": p["theme"], "locale": p["locale"],
                "program": p["program"], "motion": p["motion"], "scene": { "kind": "group" }
            }))
            .map_err(|e| e.to_string())?;
            let mut scenes: Vec<(u64, datars_scene::Scene)> = chunks("scene")
                .into_iter()
                .map(|(m, b)| Ok((m.get("index").and_then(|i| i.as_u64()).unwrap_or(0), datars_scene::Scene::from_json(std::str::from_utf8(&b).map_err(|e| e.to_string())?)?)))
                .collect::<Result<_, String>>()?;
            scenes.sort_by_key(|s| s.0);
            engine.load_baked(doc, scenes.into_iter().map(|s| s.1).collect());
            // Baked glyph runs name their faces; the fonts must be there to draw them.
            for (m, bytes) in chunks("font") {
                add_font_chunk(engine, &m, &bytes, true)?;
            }
            Ok(Tier::T1)
        }
        Tier::T2 | Tier::T3 => {
            let (_, d) = chunks("doc").into_iter().next().ok_or("missing doc chunk")?;
            let doc = datars_ir::Doc::from_json_trusted(std::str::from_utf8(&d).map_err(|e| e.to_string())?)?;
            engine.load(doc);
            // Data the compiler fetched (atlases, files, a live source's snapshot): content-addressed,
            // so shared across bundles.
            for (m, bytes) in chunks("data") {
                if let Some(source) = m.get("source").and_then(|s| s.as_str()) {
                    engine.provide_snapshot(source, &bytes)?;
                }
            }
            for (m, bytes) in chunks("font") {
                add_font_chunk(engine, &m, &bytes, false)?;
            }
            Ok(tier)
        }
    }
}

/// Give the engine a font chunk: a face a theme token names (`face`, `family`, `weight`,
/// `italic` in its metadata) or a document's fallback font (`source`: the data source it
/// answers). T1 has no document sources, so fallback fonts are added directly there.
pub fn add_font_chunk(engine: &mut Engine, meta: &BTreeMap<String, serde_json::Value>, bytes: &[u8], baked: bool) -> Result<(), String> {
    let source = meta.get("source").and_then(|s| s.as_str());
    let part = meta.get("part").and_then(|p| p.as_str());
    // A document font source answers its request (T2/T3) and becomes a fallback family.
    if let (Some(source), None, false) = (source, part, baked) {
        return engine.provide(source, bytes);
    }
    match meta.get("family").and_then(|f| f.as_str()) {
        Some(family) => {
            let weight = meta.get("weight").and_then(|w| w.as_u64()).unwrap_or(400) as u16;
            let italic = meta.get("italic").and_then(|i| i.as_bool()).unwrap_or(false);
            engine.add_face_part(bytes, family, weight, italic, part, source.is_some())
        }
        None => engine.add_font(bytes),
    }
}

/// A variant's lazily loaded font chunks (script subsets for data that arrives at runtime) whose
/// Unicode ranges cover any of `chars` — what a runtime fetches when text needs glyphs the eager
/// subsets lack ([`Engine::missing_chars`]).
pub fn lazy_fonts_for(manifest: &Manifest, entry: &serde_json::Value, chars: &[char]) -> Vec<String> {
    entry["chunks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|h| h.as_str())
        .filter_map(|h| manifest.chunk(h))
        .filter(|c| c.kind == "font" && c.lazy)
        .filter(|c| {
            let ranges = c.meta.get("unicodes").and_then(|u| u.as_str()).unwrap_or("");
            chars.iter().any(|&ch| in_unicode_ranges(ranges, ch))
        })
        .map(|c| c.hash.clone())
        .collect()
}

/// Is `ch` in a CSS `unicode-range` list (`"U+0000-00FF,U+0131,U+2000-206F"`)?
pub fn in_unicode_ranges(ranges: &str, ch: char) -> bool {
    let c = ch as u32;
    ranges.split(',').filter_map(|r| {
        let r = r.trim().trim_start_matches("U+").trim_start_matches("u+");
        let (a, b) = r.split_once('-').unwrap_or((r, r));
        Some((u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
    }).any(|(a, b)| (a..=b).contains(&c))
}

/// Open a ready loader's variant.
pub fn open_loader(engine: &mut Engine, loader: &Loader) -> Result<Tier, String> {
    let entry = loader.entry().ok_or("the loader has no entry chunk yet")?.clone();
    open_with(engine, &loader.manifest, loader.variant.tier, &entry, &|h| loader.chunk(h).map(|b| b.to_vec()))
}

/// The T0 parts of a loaded variant: poster SVG and accessible text JSON.
pub fn fallback(loader: &Loader) -> (Option<String>, Option<serde_json::Value>) {
    let poster = loader.find("poster", None).and_then(|b| String::from_utf8(b.to_vec()).ok());
    let text = loader.find("a11y", None).and_then(|b| serde_json::from_slice(&b).ok());
    (poster, text)
}
