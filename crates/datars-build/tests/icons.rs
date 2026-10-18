//! Icons and pictures travel with charts like fonts: the build step fetches what the chart draws,
//! ships only that (one `icons` chunk, an `image` chunk per picture), credits it in the manifest,
//! and every tier draws it in a runtime that never asks for anything.

use datars_bundle::{Capabilities, Tier};
use datars_engine::Request;

fn caps(modules: &[&str]) -> Capabilities {
    let mut c = datars_engine::bundle::capabilities(true, Vec::new());
    c.modules = modules.iter().map(|m| m.to_string()).collect();
    c
}

const LOGO: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#e30613"/><circle cx="5" cy="5" r="2" fill="#fff"/></svg>"##;

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 2, 2);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 128]).unwrap();
    out
}

fn doc() -> String {
    serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b", "c"], "x": [20, 60, 100] }, "key": ["k"] } },
        "program": { "states": [ { "name": "one" }, { "name": "two", "signals": { "big": true } } ] },
        "signals": { "big": { "type": "bool", "default": false } },
        "scene": { "kind": "group", "children": [
            { "kind": "icon", "icon": "heart", "at": [20, 20], "icon_size": "=big ? 32 : 16", "ink": "$accent", "semantics": { "role": "label", "label": "Liked" } },
            { "kind": "icon", "icon": "logo.svg", "at": [60, 20], "icon_size": 16, "inks": ["$negative", "$paper"] },
            { "kind": "instances", "from": "t", "proto": "icon", "icon": "lucide:user", "x": "=d.x", "y": 60, "r": 8, "fill": "$mark" },
            { "kind": "image", "asset": "pic.png", "rect": [150, 10, 40, 40], "semantics": { "role": "label", "label": "A test card" } }
        ] }
    })
    .to_string()
}

fn fetch(r: &Request) -> Option<Vec<u8>> {
    let Request::Source { url, .. } = r else { return None };
    match url.as_str() {
        "logo.svg" => Some(LOGO.as_bytes().to_vec()),
        "pic.png" => Some(png()),
        u => datars_headless::datars_asset(u),
    }
}

#[test]
fn a_bundle_carries_the_icons_and_pictures_it_draws_and_credits_them() {
    let (bundle, report) = datars_build::build_with(&doc(), &datars_build::Options::default(), &fetch).unwrap();
    let kinds: Vec<&str> = bundle.manifest.chunks.iter().map(|c| c.kind.as_str()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == "icons").count(), 1, "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| **k == "image").count(), 1, "{kinds:?}");
    let icons = bundle.manifest.chunks.iter().find(|c| c.kind == "icons").unwrap();
    let shipped: serde_json::Value = serde_json::from_slice(&bundle.chunks[&icons.hash]).unwrap();
    let keys: Vec<&String> = shipped.as_object().unwrap().keys().collect();
    assert_eq!(keys, vec!["logo.svg", "lucide:heart", "lucide:user"], "only what the chart draws, not the set");
    let lucide = bundle.manifest.assets.iter().find(|a| a.from == "datars:icons/lucide").unwrap();
    assert_eq!((lucide.licence.as_str(), lucide.names.clone()), ("ISC", vec!["heart".to_string(), "user".to_string()]));
    assert!(bundle.manifest.assets.iter().any(|a| a.kind == "image" && a.from == "pic.png"));
    assert!(report.decisions.iter().any(|d| d.contains("no licence on record") && d.contains("logo.svg")), "{:?}", report.decisions);
    // The poster draws them too.
    let poster = bundle.manifest.chunks.iter().find(|c| c.kind == "poster").unwrap();
    let svg = String::from_utf8(bundle.chunks[&poster.hash].clone()).unwrap();
    assert!(svg.contains("<use ") && svg.contains("<image "), "icons as <use>, the picture inline");
}

#[test]
fn every_tier_draws_them_without_asking() {
    let (bundle, _) = datars_build::build_with(&doc(), &datars_build::Options::default(), &fetch).unwrap();
    let mut want = Vec::new();
    for (modules, tier) in [(&["core"][..], Tier::T1), (&["core", "graph"][..], Tier::T2), (&["core", "graph", "sandbox"][..], Tier::T3)] {
        let mut engine = datars_engine::Engine::without_builtin_fonts();
        assert_eq!(datars_build::open(&mut engine, &bundle, &caps(modules)).unwrap(), tier);
        let hashes: Vec<String> = (0..2).map(|i| engine.scene_for_state(i).hash_hex()).collect();
        assert!(engine.requests().iter().all(|r| !matches!(r, Request::Source { name, .. } if name.starts_with("icon:") || name.starts_with("image:"))), "{tier:?} asks: {:?}", engine.requests());
        let scene = engine.scene_for_state(0);
        let list = engine.display_list(&scene);
        assert!(list.ops.iter().any(|o| matches!(o, datars_render::Op::Image { image: Some(_), .. })), "{tier:?}: the picture is there");
        assert!(list.ops.iter().any(|o| matches!(o, datars_render::Op::Instances(i) if matches!(i.proto, datars_scene::Proto::Icon { .. }))), "{tier:?}: icon instances");
        want.push(hashes);
    }
    assert!(want.windows(2).all(|w| w[0] == w[1]), "every tier, the same scenes: {want:?}");
}

#[test]
fn a_document_credits_its_own_files_and_the_tools_read_webp() {
    // A WebP picture (decoded by the tools for the poster and the baked tier), and the document's
    // own credit for it: licence, who, where from — carried into the manifest.
    let webp = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../datars-render/tests/half8.webp")).unwrap();
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 60 },
        "credits": { "half.webp": { "licence": "public-domain", "credit": "NASA/JPL", "source": "https://photojournal.jpl.nasa.gov/catalog/PIA00001" } },
        "scene": { "kind": "image", "asset": "half.webp", "rect": [10, 10, 40, 40], "fit": "cover", "crop": "circle", "semantics": { "role": "label", "label": "Half and half" } }
    })
    .to_string();
    let fetch = |r: &Request| match r {
        Request::Source { url, .. } if url == "half.webp" => Some(webp.clone()),
        _ => None,
    };
    let (bundle, report) = datars_build::build_with(&doc, &datars_build::Options::default(), &fetch).unwrap();
    let pic = bundle.manifest.assets.iter().find(|a| a.kind == "image").unwrap();
    assert_eq!((pic.licence.as_str(), pic.credit.as_deref(), pic.source.as_deref()), ("public-domain", Some("NASA/JPL"), Some("https://photojournal.jpl.nasa.gov/catalog/PIA00001")));
    assert!(!report.decisions.iter().any(|d| d.contains("no licence on record")), "{:?}", report.decisions);
    // The credit reads back from the manifest as written.
    let back = datars_bundle::Manifest::from_json(&bundle.manifest.to_json()).unwrap();
    assert_eq!(back.assets, bundle.manifest.assets);
    // Shipped as the file it is, and drawn into the poster, cut round.
    let image = bundle.manifest.chunks.iter().find(|c| c.kind == "image").unwrap();
    assert_eq!(bundle.chunks[&image.hash], webp);
    let poster = bundle.manifest.chunks.iter().find(|c| c.kind == "poster").unwrap();
    let svg = String::from_utf8(bundle.chunks[&poster.hash].clone()).unwrap();
    assert!(svg.contains("image/webp") && svg.contains("<clipPath"), "{}", &svg[..svg.len().min(600)]);
}
