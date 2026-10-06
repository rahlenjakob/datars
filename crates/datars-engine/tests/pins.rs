//! Pinned nodes: the anchor follows the cameras above, the content keeps screen size around it.
//! A pinned text is anchored at its `at` (a place name at a map point stays at that point under
//! any zoom); it once was anchored at its parent's origin and drawn `at` screen px away from it —
//! far off screen under a map camera.

use datars_engine::Engine;

/// A view whose camera zooms ×4 on (50, 50), with a text at (60, 40) in content units.
fn engine(pin: bool) -> Engine {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 200 },
        "scene": { "kind": "view", "key": "map", "camera": { "x": 50, "y": 50, "zoom": 4 }, "children": [
            { "kind": "text", "key": "name", "text": "Borlänge", "at": [60, 40], "pin": pin,
              "style": { "size": 12, "align": "middle", "baseline": "middle" } }] } });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e
}

/// The drawn text's bounds (CSS px), found under a point.
fn text_at(e: &Engine, x: f64, y: f64) -> Option<[f64; 4]> {
    e.hit_test(x, y).into_iter().find(|h| h.kind == "text").map(|h| h.bounds)
}

#[test]
fn a_pinned_text_stays_at_its_point_at_screen_size() {
    // (60, 40) through the camera: ((60 − 50)·4 + 100, (40 − 50)·4 + 100).
    let (x, y) = (140.0, 60.0);
    let zoomed = text_at(&engine(false), x, y).expect("the unpinned text is drawn at its point");
    let mut pinned_e = engine(true);
    let pinned = text_at(&pinned_e, x, y).expect("the pinned text is drawn at its point too");
    let centre = |b: [f64; 4]| (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
    let (cx, cy) = centre(pinned);
    assert!((cx - x).abs() < 1.0 && (cy - y).abs() < 2.0, "centred on its point: {pinned:?}");
    assert!((zoomed[2] / pinned[2] - 4.0).abs() < 0.05, "the camera scales the unpinned text, not the pinned one: {zoomed:?} vs {pinned:?}");
    // `explain` reports where it's drawn.
    let ex = pinned_e.explain("(\"name\",)");
    let b = ex.first().and_then(|x| x.bounds).expect("bounds");
    assert!((b[0] - pinned[0]).abs() < 1e-6 && (b[1] - pinned[1]).abs() < 1e-6 && (b[2] - pinned[2]).abs() < 1e-6, "{b:?} vs {pinned:?}");
}

/// `explain`'s bounds count a node's own transform once (they counted it twice: a group moved
/// 10 px right was reported 20 px right).
#[test]
fn explain_counts_a_nodes_own_transform_once() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "group", "key": "moved", "transform": { "translate": [10, 5] }, "children": [
                { "kind": "shape", "key": "box", "geom": { "type": "rect", "x": 0, "y": 0, "w": 30, "h": 20 }, "fill": "$accent" }] }] } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    for q in ["(\"moved\",)", "(\"box\",)"] {
        let b = e.explain(q).first().and_then(|x| x.bounds);
        assert_eq!(b, Some([10.0, 5.0, 30.0, 20.0]), "{q}");
    }
}

/// A camera fit to `keys` frames the nodes with those keys. Pinned nodes count by their origin —
/// only when they're among the keys: a label pinned at every town once made a fit to two towns
/// frame all of them.
#[test]
fn a_fit_by_keys_ignores_pinned_nodes_with_other_keys() {
    let camera = |keys: serde_json::Value| {
        let dot = |k: &str, x: f64, y: f64| serde_json::json!({ "kind": "shape", "key": k, "geom": { "type": "circle", "cx": x, "cy": y, "r": 5 }, "fill": "$accent" });
        let label = |k: &str, x: f64, y: f64| serde_json::json!({ "kind": "text", "key": format!("label-{k}"), "text": k, "at": [x, y], "pin": true, "style": { "size": 12 } });
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 200 },
            "scene": { "kind": "view", "key": "map", "camera": { "fit": { "keys": keys }, "padding": 0 }, "children": [
                dot("a", 10.0, 10.0), dot("b", 30.0, 30.0), dot("c", 190.0, 190.0),
                { "kind": "group", "key": "labels", "children": [label("a", 10.0, 10.0), label("b", 30.0, 30.0), label("c", 190.0, 190.0)] }] } });
        let mut e = Engine::new();
        let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        assert!(diags.is_empty(), "{diags:?}");
        let s = e.scene();
        let datars_scene::NodeKind::View { camera: Some(c), .. } = &s.root.kind else { panic!("a fit camera: {}", s.snapshot()) };
        (c.x, c.y, c.zoom)
    };
    // a and b span (5, 5)–(35, 35): 30 px of content in 200 px.
    let (x, y, zoom) = camera(serde_json::json!(["a", "b"]));
    assert!((x - 20.0).abs() < 1e-9 && (y - 20.0).abs() < 1e-9 && (zoom - 200.0 / 30.0).abs() < 1e-9, "only a and b: ({x}, {y}) ×{zoom}");
    // A pinned node that is one of the keys still counts, by its origin.
    let (x, _, zoom) = camera(serde_json::json!(["a", "label-c"]));
    assert!((x - 97.5).abs() < 1e-9 && (zoom - 200.0 / 185.0).abs() < 1e-9, "a and c's label: x {x} ×{zoom}");
}
