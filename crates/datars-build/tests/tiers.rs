//! Tier equivalence: the publish compiler's variants are optimizations, never different charts.
//! For every example, T1 (baked scenes), T2 (pre-expanded doc) and T3 (source), each opened with
//! the capabilities that select it, give the golden scene for every state. Every bundle is opened
//! in an engine with no built-in fonts — what the web runtime is — so each proves it carries the
//! fonts it draws with.

use datars_bundle::{Capabilities, Tier};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn caps(modules: &[&str]) -> Capabilities {
    let mut c = datars_engine::bundle::capabilities(true, Vec::new());
    c.modules = modules.iter().map(|m| m.to_string()).collect();
    c
}

#[test]
fn every_tier_renders_the_golden_scenes() {
    let mut checked = 0;
    for name in ["votes", "riksdag", "business", "flows", "shapes", "warming", "dashboard", "inflation"] {
        let json = std::fs::read_to_string(root().join(format!("examples/{name}/doc.json"))).unwrap();
        let golden: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join(format!("tests/golden/{name}/golden.json"))).unwrap()).unwrap();
        let (bundle, _) = datars_build::build(&json, &datars_build::Options::default()).unwrap();
        for (modules, want) in [(&["core"][..], Tier::T1), (&["core", "graph"][..], Tier::T2), (&["core", "graph", "sandbox"][..], Tier::T3)] {
            let mut engine = datars_engine::Engine::without_builtin_fonts();
            let tier = datars_build::open(&mut engine, &bundle, &caps(modules)).unwrap_or_else(|e| panic!("{name} {want:?}: {e}"));
            assert_eq!(tier, want, "{name}: {modules:?} selects {want:?}");
            for (i, st) in golden["states"].as_array().unwrap().iter().enumerate() {
                let scene = engine.scene_for_state(i);
                assert_eq!(scene.hash_hex(), st["scene"].as_str().unwrap(), "{name} / {} via {tier:?}", st["name"]);
                checked += 1;
            }
        }
    }
    assert!(checked >= 50, "{checked}");
}

/// A T3 variant expands on the device with the runtime's std. A runtime whose std differs from the
/// publisher's (an app that shipped an older release) must not draw a different chart: it takes
/// the pre-expanded T2 variant, which is the author's output.
#[test]
fn a_runtime_with_another_std_plays_the_pre_expanded_variant() {
    let json = std::fs::read_to_string(root().join("examples/votes/doc.json")).unwrap();
    let golden: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join("tests/golden/votes/golden.json")).unwrap()).unwrap();
    let (bundle, _) = datars_build::build(&json, &datars_build::Options::default()).unwrap();
    let t3 = bundle.manifest.variants.iter().find(|v| v.tier == Tier::T3).unwrap();
    assert_eq!(t3.requires.packages, datars_engine::bundle::packages(), "T3 is pinned to the publisher's packages");

    let mut older = caps(&["core", "graph", "sandbox"]);
    older.packages.insert("@datars/std".into(), "b3:an-older-std".into());
    let mut engine = datars_engine::Engine::without_builtin_fonts();
    assert_eq!(datars_build::open(&mut engine, &bundle, &older).unwrap(), Tier::T2);
    for (i, st) in golden["states"].as_array().unwrap().iter().enumerate() {
        assert_eq!(engine.scene_for_state(i).hash_hex(), st["scene"].as_str().unwrap());
    }
    let only_t3 = datars_bundle::Manifest { variants: vec![t3.clone()], ..bundle.manifest.clone() };
    let why = only_t3.select(&older).unwrap_err().to_string();
    assert!(why.contains("T3: built with a different @datars/std"), "{why}");
}

/// Atlases ship as decimal topologies (shared borders, integer deltas) — a delivery optimization
/// that must not change a pixel: the world choropleth through T2 and T3 gives the golden scenes.
#[test]
fn a_compacted_atlas_draws_the_golden_scenes() {
    let json = std::fs::read_to_string(root().join("examples/renewables/doc.json")).unwrap();
    let golden: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join("tests/golden/renewables/golden.json")).unwrap()).unwrap();
    let dir = root().join("examples/renewables");
    let (bundle, report) = datars_build::build_with(&json, &datars_build::Options::default(), &|r| datars_headless::fetch_from_disk(r, Some(&dir))).unwrap();
    assert!(report.decisions.iter().any(|d| d.starts_with("geo `world`") && d.contains("decimal topology")), "{:#?}", report.decisions);
    for (modules, want) in [(&["core", "graph"][..], Tier::T2), (&["core", "graph", "sandbox"][..], Tier::T3)] {
        let mut engine = datars_engine::Engine::without_builtin_fonts();
        assert_eq!(datars_build::open(&mut engine, &bundle, &caps(modules)).unwrap(), want);
        for (i, st) in golden["states"].as_array().unwrap().iter().enumerate() {
            assert_eq!(engine.scene_for_state(i).hash_hex(), st["scene"].as_str().unwrap(), "renewables / {} via {want:?}", st["name"]);
        }
    }
}

/// Fonts ship with every playable variant — T1 too: its baked glyph runs name the faces, and the
/// pixels need their outlines. The theme's default family (Inter), a document's own font (Noto
/// Sans Hebrew) and a theme's project fonts (Newsreader) travel as subsets with their licences;
/// each tier, opened in a runtime with no fonts of its own, draws the golden pixels.
#[test]
fn fonts_ship_with_every_playable_variant() {
    for name in ["hebrew", "votes", "serif"] {
        let json = std::fs::read_to_string(root().join(format!("examples/{name}/doc.json"))).unwrap();
        let golden: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root().join(format!("tests/golden/{name}/golden.json"))).unwrap()).unwrap();
        let dir = root().join(format!("examples/{name}"));
        let (bundle, report) = datars_build::build_with(&json, &datars_build::Options::default(), &|r| datars_headless::fetch_from_disk(r, Some(&dir))).unwrap();
        let fonts: Vec<&String> = report.decisions.iter().filter(|d| d.starts_with("font `")).collect();
        assert!(!fonts.is_empty() && fonts.iter().all(|d| d.contains("glyphs for the chart's text")), "{name}: {:#?}", report.decisions);
        assert!(bundle.manifest.fonts.iter().all(|f| f.licence == "OFL-1.1") && !bundle.manifest.fonts.is_empty(), "{name}: {:?}", bundle.manifest.fonts);
        for (modules, want) in [(&["core"][..], Tier::T1), (&["core", "graph"][..], Tier::T2), (&["core", "graph", "sandbox"][..], Tier::T3)] {
            let mut engine = datars_engine::Engine::without_builtin_fonts();
            assert_eq!(datars_build::open(&mut engine, &bundle, &caps(modules)).unwrap(), want);
            assert!(engine.requests().is_empty(), "{name} via {want:?}: nothing left to fetch: {:?}", engine.requests());
            for (i, st) in golden["states"].as_array().unwrap().iter().enumerate() {
                let scene = engine.scene_for_state(i);
                let px = datars_headless::render_scene(&engine, &scene, 1.0);
                assert_eq!(format!("{:016x}", px.hash()), st["pixels"].as_str().unwrap(), "{name} / {} via {want:?}", st["name"]);
            }
            assert!(engine.missing_chars().is_empty(), "{name} via {want:?}: every character has a glyph: {:?}", engine.missing_chars());
        }
    }
}

/// Faces a theme names that a runtime has no source for are diagnosed, with what they fall back
/// to; a sourced face arrives through a `font:` request like a data source.
#[test]
fn theme_fonts_arrive_by_request_or_are_diagnosed() {
    let doc = |font: serde_json::Value| {
        serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 60 },
            "theme": { "tokens": { "font.title": font } },
            "scene": { "kind": "text", "key": "t", "at": [10, 30], "text": "Hej", "style": { "font": "$font.title", "size": 20 } } })
        .to_string()
    };
    // No source: the build can't ship it, and says so.
    let mut e = datars_headless::load(&doc(serde_json::json!({ "family": "GT America", "weight": 700 }))).unwrap();
    let _ = e.scene();
    let d: Vec<String> = e.diagnostics().iter().map(|d| d.message.clone()).collect();
    assert!(d.iter().any(|m| m.contains("font 'GT America' (font.title) has no source; falling back to Inter")), "{d:?}");
    // A project file: requested as `font:<face>` and registered under the token's name.
    let dir = root().join("examples/serif");
    let json = doc(serde_json::json!({ "family": "House Serif", "weight": 600, "src": "../../assets/fonts/Newsreader-SemiBold.ttf" }));
    let mut e = datars_engine::Engine::without_builtin_fonts();
    e.load(datars_ir::Doc::from_json(&json).unwrap());
    let reqs = e.requests();
    assert!(reqs.contains(&datars_engine::Request::Source { name: "font:House Serif-600".into(), url: "../../assets/fonts/Newsreader-SemiBold.ttf".into() }), "{reqs:?}");
    for r in reqs {
        if let (datars_engine::Request::Source { name, .. }, Some(bytes)) = (&r, datars_headless::fetch_from_disk(&r, Some(&dir))) {
            e.provide(name, &bytes).unwrap();
        }
    }
    let scene = e.scene();
    let mut fonts = Vec::new();
    scene.root.walk(&Default::default(), &mut |_, n| {
        if let datars_scene::NodeKind::Text(t) = &n.kind {
            fonts.extend(t.runs.iter().map(|r| r.font.to_string()));
        }
    });
    assert_eq!(fonts, vec!["House Serif-600"]);
    assert!(!e.requests().iter().any(|r| matches!(r, datars_engine::Request::Source { name, .. } if name == "font:House Serif-600")), "{:?}", e.requests());
}

/// Charts whose text arrives at runtime (a host data slot) ship each face's scripts as lazily
/// loaded subsets; a runtime that meets a character its eager subsets lack loads the part.
#[test]
fn runtime_text_gets_lazily_loaded_script_subsets() {
    let json = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 80 },
        "data": { "names": { "slot": "names" } },
        "scene": { "kind": "repeat", "key": "r", "from": "names", "template": {
            "kind": "text", "key": "=d.name", "at": [10, 30], "text": "=d.name" } } })
    .to_string();
    let (bundle, report) = datars_build::build(&json, &datars_build::Options::default()).unwrap();
    let lazy: Vec<&datars_bundle::ChunkRef> = bundle.manifest.chunks.iter().filter(|c| c.kind == "font" && c.lazy).collect();
    let parts: Vec<&str> = lazy.iter().filter_map(|c| c.meta.get("part").and_then(|p| p.as_str())).collect();
    assert!(parts.contains(&"latin-ext") && parts.contains(&"cyrillic"), "{parts:?} {:#?}", report.decisions);
    // A runtime without fonts loads T2 over the network (eager chunks only), the host fills the
    // slot with Polish names, the runtime finds Ł missing and asks for the latin-ext part.
    let mut two = bundle.clone();
    two.manifest.variants.retain(|v| v.tier != Tier::T3);
    let glyphs = |core: &mut datars_runtime::Core| {
        let mut out = Vec::new();
        core.engine.scene().root.walk(&Default::default(), &mut |_, n| {
            if let datars_scene::NodeKind::Text(t) = &n.kind {
                out.extend(t.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| g.id)));
            }
        });
        out
    };
    let mut core = datars_runtime::Core::without_builtin_fonts(true, Vec::new());
    let mut need = core.open_manifest(&two.manifest.to_json()).unwrap();
    while let Some(h) = need.pop() {
        assert!(!two.manifest.chunk(&h).unwrap().lazy, "lazy chunks aren't fetched up front");
        need = core.provide_chunk(&h, &two.chunks[&h]).unwrap();
    }
    assert_eq!(core.tier, Some(Tier::T2));
    core.engine.provide("names", r#"[{"name": "Łódź"}, {"name": "Kraków"}]"#.as_bytes()).unwrap();
    let _ = core.frame_pixels(0.0, 1.0);
    assert!(glyphs(&mut core).contains(&0), "Ł has no glyph yet");
    let want = core.requests();
    assert!(!want.is_empty(), "a script part is requested");
    // ó is Latin-1 (the `latin` part), Ł Latin Extended-A: nothing else is fetched.
    let mut asked: Vec<String> = want.iter().map(|h| two.manifest.chunk(h).unwrap().meta["part"].as_str().unwrap().to_string()).collect();
    asked.sort();
    asked.dedup();
    assert_eq!(asked, vec!["latin", "latin-ext"]);
    for h in want {
        core.provide_chunk(&h, &two.chunks[&h]).unwrap();
    }
    let _ = core.frame_pixels(1.0, 1.0);
    let g = glyphs(&mut core);
    assert!(!g.is_empty() && g.iter().all(|&g| g != 0), "every glyph real after the part arrived: {g:?}");
    // A single-file bundle has the part already: it's added without a request.
    let mut core = datars_runtime::Core::without_builtin_fonts(true, Vec::new());
    core.open_file(&datars_bundle::to_single_file(&two)).unwrap();
    core.engine.provide("names", r#"[{"name": "Łódź"}]"#.as_bytes()).unwrap();
    let _ = core.frame_pixels(0.0, 1.0);
    assert!(core.requests().is_empty());
    let g = glyphs(&mut core);
    assert!(!g.is_empty() && g.iter().all(|&g| g != 0), "{g:?}");
}

/// Hosts resize views, and a label that shows only when it fits (or wraps, or truncates) draws
/// other text at other sizes: a face whose text depends on size gets the full coverage, not just
/// what the authored size drew.
#[test]
fn faces_cover_the_text_other_sizes_draw() {
    let json = serde_json::json!({ "datars": 1, "size": { "width": 800, "height": 120 },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "text", "key": "wide", "at": [10, 40], "text": "Apple", "when": "=box.w >= 400", "style": { "font": "font.title", "size": 20 } },
            { "kind": "text", "key": "narrow", "at": [10, 40], "text": "Zebra", "when": "=box.w < 400", "style": { "font": "font.title", "size": 20 } } ] } })
    .to_string();
    let (bundle, _) = datars_build::build(&json, &datars_build::Options::default()).unwrap();
    let mut core = datars_runtime::Core::without_builtin_fonts(true, Vec::new());
    core.open_file(&datars_bundle::to_single_file(&bundle)).unwrap();
    core.engine.resize(300.0, 120.0, 1.0);
    let _ = core.frame_pixels(10.0, 1.0);
    let mut glyphs = Vec::new();
    core.engine.scene().root.walk(&Default::default(), &mut |_, n| {
        if let datars_scene::NodeKind::Text(t) = &n.kind {
            assert_eq!(t.text, "Zebra");
            glyphs.extend(t.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| g.id)));
        }
    });
    assert!(glyphs.len() == 5 && glyphs.iter().all(|&g| g != 0), "{glyphs:?}");
}

/// A bundle carries its document fonts as chunks, each answering its own source — two files of one
/// family (a regular and a semibold) included — so hosts are never asked to fetch them by URL.
#[test]
fn hosts_arent_asked_for_what_the_bundle_carries() {
    let fonts = root().join("assets/fonts");
    let json = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 80 },
        "data": { "serif": { "font": "Newsreader-Regular.ttf" }, "serifSemi": { "font": "Newsreader-SemiBold.ttf" }, "rows": { "values": { "name": ["Stockholm"] } } },
        "scene": { "kind": "repeat", "key": "r", "from": "rows", "template": { "kind": "text", "key": "=d.name", "at": [10, 30], "text": "=d.name" } } })
    .to_string();
    let fetch = |r: &datars_engine::Request| match r {
        datars_engine::Request::Source { url, .. } => std::fs::read(fonts.join(url)).ok(),
        _ => None,
    };
    let (bundle, _) = datars_build::build_with(&json, &datars_build::Options::default(), &fetch).unwrap();
    let sources: Vec<String> = bundle.manifest.chunks.iter().filter_map(|c| c.meta.get("source").and_then(|s| s.as_str()).map(String::from)).collect();
    assert!(sources.contains(&"serif".to_string()) && sources.contains(&"serifSemi".to_string()), "{sources:?}");
    let mut two = bundle.clone();
    two.manifest.variants.retain(|v| matches!(v.tier, Tier::T0 | Tier::T2));
    let mut core = datars_runtime::Core::without_builtin_fonts(true, Vec::new());
    let mut need = core.open_manifest(&two.manifest.to_json()).unwrap();
    while let Some(h) = need.pop() {
        need = core.provide_chunk(&h, &two.chunks[&h]).unwrap();
    }
    assert!(core.engine.requests().is_empty(), "{:?}", core.engine.requests());
}
