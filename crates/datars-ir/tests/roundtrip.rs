//! Templates flatten their kind's fields into one JSON object with the common fields (`key`,
//! `opacity`, `clip`, `size`, …). A kind field that shares a common field's name serializes twice
//! and can't be read back — bundles re-serialize documents, so that breaks delivery. Every kind,
//! with every common field set, must round-trip.

use datars_ir::Template;

const COMMON: &str = r#""id": "t", "key": "k", "when": "=true", "opacity": 0.5, "z": 2, "clip": "box",
  "isolate": true, "pickable": true, "size": {"w": 10, "h": "auto"}, "prov": "test",
  "semantics": {"role": "datum", "label": "L"}, "layout": {"type": "rows", "gap": 4}"#;

const KINDS: &[&str] = &[
    r#""kind": "group", "children": []"#,
    r#""kind": "view", "camera": {"fit": {"keys": ["a"]}, "padding": 4}, "children": []"#,
    r#""kind": "shape", "geom": {"type": "rect", "x": 0, "y": 0, "w": 1, "h": 1}, "fill": "$accent", "stroke": {"paint": "$ink", "width": 1}"#,
    r#""kind": "text", "text": "hi", "at": [1, 2], "style": {"size": 11, "ink": "$ink"}, "rotate": 0.5, "offset": [0, -8]"#,
    r#""kind": "repeat", "from": "t", "template": {"kind": "group", "children": []}"#,
    r#""kind": "instances", "from": "t", "x": "=d.x", "y": "=d.y", "r": 3, "instance_key": "=d.id", "instance_opacity": "=d.o", "fill": "$mark""#,
    r#""kind": "use", "recipe": "@datars/std/bar", "params": {"labels": true}"#,
    r#""kind": "image", "asset": "a.png", "rect": [0, 0, 10, 10]"#,
    r#""kind": "tiles", "source": "base", "tile_size": 384, "layers": [{"layer": "water", "template": {"kind": "shape", "geom": {"type": "feature"}, "fill": "$map.water"}},
       {"layer": "places", "id": "towns", "filter": "=d.rank < 5", "minzoom": 3, "labels": true, "priority": "=d.pop", "merge": false,
        "template": {"kind": "text", "text": "=d.name", "at": ["=d.$x", "=d.$y"]}}]"#,
];

#[test]
fn every_kind_round_trips_with_every_common_field() {
    for kind in KINDS {
        let src = format!("{{{COMMON}, {kind}}}");
        let t: Template = serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse {kind}: {e}"));
        let json = serde_json::to_string(&t).unwrap();
        let back: Template = serde_json::from_str(&json).unwrap_or_else(|e| panic!("re-parse {kind}: {e}\n{json}"));
        assert_eq!(t, back, "{kind}");
        // Nothing is dropped either: the common fields survive.
        for f in ["\"opacity\":0.5", "\"clip\":\"box\"", "\"key\":\"k\""] {
            assert!(json.contains(f), "{kind}: {f} lost in {json}");
        }
    }
}

#[test]
fn layout_padding_accepts_css_shorthand() {
    for (src, want) in [("12", [12.0; 4]), ("[4, 8]", [4.0, 8.0, 4.0, 8.0]), ("[1, 2, 3]", [1.0, 2.0, 3.0, 2.0]), ("[1, 2, 3, 4]", [1.0, 2.0, 3.0, 4.0])] {
        let l: datars_ir::Layout = serde_json::from_str(&format!("{{\"type\": \"rows\", \"padding\": {src}}}")).unwrap();
        assert_eq!(l.padding, want, "{src}");
    }
}
