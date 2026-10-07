//! Direct-manipulation intents: brushing a range (continuous and band scales), clearing it with a
//! click, and exploring a view (wheel zoom keeps the content under the cursor still; drag pans).

use datars_engine::{Engine, Pointer};
use datars_expr::Value;
use datars_math::Vec2;
use datars_scene::{Node, NodeKind};

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e
}

fn num(e: &Engine, s: &str) -> f64 {
    match e.signal(s) {
        Some(Value::Num(n)) => n,
        other => panic!("{s}: {other:?}"),
    }
}

fn brushable(scale: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "scene": { "kind": "group", "key": "area", "scales": { "x": scale },
            "on": { "brush": { "brush": "sel" } },
            "children": [{ "kind": "shape", "key": "bg", "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 100 }, "fill": "$surface" }] }
    })
}

#[test]
fn brushing_a_continuous_scale_selects_a_data_range() {
    let mut e = engine(brushable(serde_json::json!({ "type": "linear", "domain": [0, 100], "range": [0, 200] })));
    e.pointer(Pointer::Down { x: 120.0, y: 50.0 });
    e.pointer(Pointer::Move { x: 70.0, y: 50.0 });
    e.pointer(Pointer::Move { x: 20.0, y: 50.0 });
    e.pointer(Pointer::Up { x: 20.0, y: 50.0 });
    assert_eq!(e.signal("sel.active"), Some(Value::Bool(true)));
    assert!((num(&e, "sel.lo") - 10.0).abs() < 1e-9, "{}", num(&e, "sel.lo"));
    assert!((num(&e, "sel.hi") - 60.0).abs() < 1e-9, "{}", num(&e, "sel.hi"));
    // A click (no drag) clears it.
    e.pointer(Pointer::Down { x: 50.0, y: 50.0 });
    e.pointer(Pointer::Up { x: 50.0, y: 50.0 });
    assert_eq!(e.signal("sel.active"), Some(Value::Bool(false)));
}

#[test]
fn brushing_a_band_scale_selects_the_bands_covered() {
    let mut e = engine(brushable(serde_json::json!({ "type": "band", "domain": ["a", "b", "c", "d"], "range": [0, 200], "padding": 0 })));
    // Bands are 50 px wide; centres at 25, 75, 125, 175.
    e.pointer(Pointer::Down { x: 60.0, y: 50.0 });
    e.pointer(Pointer::Move { x: 140.0, y: 50.0 });
    e.pointer(Pointer::Up { x: 140.0, y: 50.0 });
    assert_eq!(e.signal("sel.active"), Some(Value::Bool(true)));
    let keys = match e.signal("sel") {
        Some(Value::Str(s)) => s.split('\u{1f}').map(String::from).collect::<Vec<_>>(),
        other => panic!("{other:?}"),
    };
    assert_eq!(keys, vec!["b", "c"]);
}

fn explorable() -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "scene": { "kind": "view", "key": "map", "camera": { "fit": { "bbox": [0, 0, 100, 100] }, "padding": 0, "explore": "cam" },
            "children": [{ "kind": "shape", "key": "land", "geom": { "type": "rect", "x": 0, "y": 0, "w": 100, "h": 100 }, "fill": "$accent" }] }
    })
}

fn content_at(e: &mut Engine, p: Vec2) -> Vec2 {
    let f = e.frame(1.0);
    fn find(n: &Node) -> Option<&Node> {
        if matches!(n.kind, NodeKind::View { .. }) {
            return Some(n);
        }
        n.children().iter().find_map(find)
    }
    let view = find(&f.scene.root).expect("view");
    let NodeKind::View { viewport, camera: Some(cam), .. } = &view.kind else { panic!("no camera") };
    cam.transform(*viewport).inverse().unwrap().apply(p)
}

#[test]
fn wheel_zoom_keeps_the_content_under_the_cursor() {
    let mut e = engine(explorable());
    let at = Vec2::new(150.0, 60.0);
    let before = content_at(&mut e, at);
    e.pointer(Pointer::Wheel { x: at.x, y: at.y, delta: -500.0 });
    assert!(num(&e, "cam.zoom") > 2.0, "zoomed in ×{}", num(&e, "cam.zoom"));
    let after = content_at(&mut e, at);
    assert!((before.x - after.x).abs() < 1e-9 && (before.y - after.y).abs() < 1e-9, "{before:?} → {after:?}");
}

#[test]
fn dragging_an_explorable_view_pans_it() {
    let mut e = engine(explorable());
    let start = content_at(&mut e, Vec2::new(100.0, 100.0));
    e.pointer(Pointer::Down { x: 100.0, y: 100.0 });
    e.pointer(Pointer::Move { x: 140.0, y: 100.0 });
    e.pointer(Pointer::Up { x: 140.0, y: 100.0 });
    // The content that was under (100, 100) is now under (140, 100).
    let moved = content_at(&mut e, Vec2::new(140.0, 100.0));
    assert!((start.x - moved.x).abs() < 1e-9 && (start.y - moved.y).abs() < 1e-9, "{start:?} vs {moved:?}");
}

#[test]
fn clicking_a_repeated_mark_runs_its_intent() {
    // Intents are recorded under the node's key path; inside a repeat that path must use the
    // row's key (it once used a provisional `#i`, so no click in a repeat did anything).
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b"], "x": [0, 100] }, "key": ["k"] } },
        "signals": { "picked": { "type": "keyset", "default": [] } },
        "scene": { "kind": "repeat", "from": "t", "template": { "kind": "shape",
            "geom": { "type": "rect", "x": "=d.x", "y": 0, "w": 100, "h": 100 }, "fill": "$accent",
            "on": { "activate": { "toggle": "picked", "value": "=d.k" } } } }
    }));
    e.pointer(Pointer::Down { x: 150.0, y: 50.0 });
    let hit = e.pointer(Pointer::Up { x: 150.0, y: 50.0 });
    assert_eq!(e.signal("picked"), Some(Value::Str("b".into())), "hit {hit:?}");
    e.pointer(Pointer::Down { x: 150.0, y: 50.0 });
    e.pointer(Pointer::Up { x: 150.0, y: 50.0 });
    assert_eq!(e.signal("picked"), Some(Value::Str("".into())), "a second click toggles it off");
}

#[test]
fn scrubbing_sets_a_value_under_the_pointer() {
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 40 },
        "signals": { "price": { "type": "num", "default": 50 } },
        "scene": { "kind": "group", "key": "slider", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 200] } },
            "on": { "drag": { "scrub": "price", "step": 5 } },
            "children": [{ "kind": "shape", "key": "track", "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 40 }, "fill": "$surface" }] }
    }));
    e.pointer(Pointer::Down { x: 63.0, y: 20.0 });
    assert_eq!(num(&e, "price"), 30.0, "31.5 snaps to the 5-step: 30");
    e.pointer(Pointer::Move { x: 150.0, y: 20.0 });
    assert_eq!(num(&e, "price"), 75.0);
    e.pointer(Pointer::Up { x: 150.0, y: 20.0 });
    assert_eq!(num(&e, "price"), 75.0);
    // Offered to the host as a native control (keyboards, screen readers): its range and value.
    e.frame(0.0);
    let [c] = e.controls().try_into().unwrap_or_else(|v: Vec<_>| panic!("one control: {v:?}"));
    assert_eq!((c.signal.as_str(), c.min, c.max, c.step, c.value), ("price", 0.0, 100.0, 5.0, 75.0));
    // A chart without one has none (and doesn't make a scene to find out).
    let mut plain = engine(serde_json::json!({ "datars": 1, "size": { "width": 10, "height": 10 }, "scene": { "kind": "group", "key": "r", "children": [] } }));
    plain.frame(0.0);
    assert!(plain.controls().is_empty());
}

#[test]
fn a_bounded_scrub_stops_at_its_bounds() {
    // A range's two thumbs: the low one can't pass the high one, and the other way round.
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 40 },
        "signals": { "lo": { "type": "num", "default": 20 }, "hi": { "type": "num", "default": 60 } },
        "scene": { "kind": "group", "key": "range", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 200] } },
            "children": [
                { "kind": "shape", "key": "lo", "geom": { "type": "rect", "x": 0, "y": 0, "w": 100, "h": 40 }, "fill": "$surface", "on": { "drag": { "scrub": "lo", "max": "=hi" } } },
                { "kind": "shape", "key": "hi", "geom": { "type": "rect", "x": 100, "y": 0, "w": 100, "h": 40 }, "fill": "$surface", "on": { "drag": { "scrub": "hi", "min": "=lo", "max": 90 } } }] }
    }));
    e.pointer(Pointer::Down { x: 50.0, y: 20.0 });
    e.pointer(Pointer::Move { x: 180.0, y: 20.0 });
    assert_eq!(num(&e, "lo"), 60.0, "dragged to 90, the low thumb stops at the high one");
    e.pointer(Pointer::Up { x: 180.0, y: 20.0 });
    e.frame(0.0);
    e.pointer(Pointer::Down { x: 150.0, y: 20.0 });
    e.pointer(Pointer::Move { x: 199.0, y: 20.0 });
    assert_eq!(num(&e, "hi"), 90.0, "a literal bound");
    e.pointer(Pointer::Move { x: 10.0, y: 20.0 });
    assert_eq!(num(&e, "hi"), 60.0, "and the high one stops at the low one");
    e.pointer(Pointer::Up { x: 10.0, y: 20.0 });
    // Native controls offer only where each thumb can go.
    e.frame(0.0);
    let cs = e.controls();
    let range = |s: &str| cs.iter().find(|c| c.signal == s).map(|c| (c.min, c.max)).unwrap();
    assert_eq!(range("lo"), (0.0, 60.0));
    assert_eq!(range("hi"), (60.0, 90.0));
}

#[test]
fn a_floating_node_is_drawn_and_picked_above_what_follows_it() {
    // A menu's list (z 1000) inside the first group, over a later sibling: on top, and it's what a
    // click there finds.
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "signals": { "picked": { "type": "str", "default": "" } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "group", "key": "menu", "children": [
                { "kind": "shape", "key": "list", "z": 1000, "geom": { "type": "rect", "x": 0, "y": 0, "w": 100, "h": 150 }, "fill": "#ff0000",
                  "semantics": { "role": "control", "label": "list" }, "on": { "activate": { "set": "picked", "value": "list" } } }] },
            { "kind": "shape", "key": "chart", "geom": { "type": "rect", "x": 0, "y": 50, "w": 200, "h": 150 }, "fill": "#0000ff",
              "semantics": { "role": "datum", "label": "chart" }, "on": { "activate": { "set": "picked", "value": "chart" } } }] }
    }));
    let scene = e.scene();
    let list = e.display_list(&scene);
    let reds: Vec<usize> = list.ops.iter().enumerate().filter(|(_, o)| format!("{o:?}").contains("r: 1.0, g: 0.0, b: 0.0")).map(|(i, _)| i).collect();
    let blues: Vec<usize> = list.ops.iter().enumerate().filter(|(_, o)| format!("{o:?}").contains("r: 0.0, g: 0.0, b: 1.0")).map(|(i, _)| i).collect();
    assert!(!reds.is_empty() && !blues.is_empty() && reds[0] > blues[0], "the list is drawn after the chart: {reds:?} {blues:?}");
    assert_eq!(e.hit_test(50.0, 100.0).first().and_then(|h| h.label.clone()).as_deref(), Some("list"), "topmost first");
    e.pointer(Pointer::Down { x: 50.0, y: 100.0 });
    e.pointer(Pointer::Up { x: 50.0, y: 100.0 });
    assert_eq!(e.signal("picked"), Some(Value::Str("list".into())), "a click there is the list's");
    e.pointer(Pointer::Down { x: 150.0, y: 100.0 });
    e.pointer(Pointer::Up { x: 150.0, y: 100.0 });
    assert_eq!(e.signal("picked"), Some(Value::Str("chart".into())), "beside it, the chart's");
}

#[test]
fn click_intents_are_reachable_without_a_pointer() {
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b"], "x": [0, 100] }, "key": ["k"] } },
        "signals": { "picked": { "type": "keyset", "default": [] } },
        "scene": { "kind": "repeat", "from": "t", "template": { "kind": "shape",
            "geom": { "type": "rect", "x": "=d.x", "y": 0, "w": 100, "h": 100 }, "fill": "$accent",
            "semantics": { "role": "datum", "label": "=d.k" },
            "on": { "activate": { "toggle": "picked", "value": "=d.k" } } } }
    }));
    let items = e.semantic_items();
    let b = items.iter().find(|i| i.label == "b").expect("item b");
    assert!(b.actionable, "{b:?}");
    let path = b.path.clone();
    assert!(e.activate(&path));
    assert_eq!(e.signal("picked"), Some(Value::Str("b".into())));
    assert!(!e.activate("nope"));
}

#[test]
fn brushing_an_overview_zooms_the_linked_detail() {
    // examples/prices: std plot's brush on the overview filters the detail's table.
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/prices/doc.json")).unwrap();
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&json).unwrap());
    e.frame(0.0);
    let area = e.explain("(\"overview\",)/(\"body\",)/(\"center\",)/(\"area\",)").pop().expect("the overview's plot area");
    let b = area.bounds.expect("bounds");
    let y = b[1] + b[3] / 2.0;
    e.pointer(Pointer::Down { x: b[0] + b[2] * 0.25, y });
    e.pointer(Pointer::Move { x: b[0] + b[2] * 0.4, y });
    e.pointer(Pointer::Up { x: b[0] + b[2] * 0.5, y });
    assert_eq!(e.signal("range.active"), Some(Value::Bool(true)));
    let (lo, hi) = (num(&e, "range.lo"), num(&e, "range.hi"));
    assert!(lo > 80.0 && lo < 100.0 && hi > 175.0 && hi < 190.0, "a quarter to half of the year: {lo}..{hi}");
    let snap = e.scene().snapshot();
    let summary = snap.lines().find(|l| l.contains("(\"summary\",)")).unwrap_or_default().to_string();
    let days = (hi.floor() - lo.ceil() + 1.0) as i64;
    assert!(summary.contains(&format!("over {days} days")), "the detail follows the brush: {summary}");
}

#[test]
fn a_new_state_flies_to_its_own_camera_after_exploring() {
    // Two states with different fits; the reader pans in the first, then steps on.
    let mut doc = explorable();
    doc["signals"] = serde_json::json!({ "side": { "type": "num", "default": 100 } });
    doc["scene"]["camera"]["fit"] = serde_json::json!({ "bbox": [0, 0, "=side", "=side"] });
    doc["program"] = serde_json::json!({ "states": [{ "name": "all" }, { "name": "corner", "set": { "side": 20 } }] });
    let mut e = engine(doc);
    e.pointer(Pointer::Down { x: 100.0, y: 100.0 });
    e.pointer(Pointer::Move { x: 160.0, y: 100.0 });
    e.pointer(Pointer::Up { x: 160.0, y: 100.0 });
    assert!(e.signal("cam.x").is_some(), "explored");
    assert!(e.event("next"));
    assert_eq!(e.signal("cam.x"), None, "the exploration belongs to the state left behind");
    // The flight starts at its first frame and is over long after.
    e.frame(10.0);
    e.frame(20.0);
    let centre = content_at(&mut e, Vec2::new(100.0, 100.0));
    assert!((centre.x - 10.0).abs() < 1e-6 && (centre.y - 10.0).abs() < 1e-6, "the corner's own fit: {centre:?}");
}

#[test]
fn hosts_read_every_signal_as_they_would_set_it() {
    // A page following the chart (its own readout of a selection, a brush, a camera) reads the
    // values in the shape `set_signal_json` takes: key sets as arrays, nothing as null.
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b"], "x": [0, 100] }, "key": ["k"] } },
        "signals": { "picked": { "type": "keyset", "default": [] }, "sel": { "type": "range" }, "n": { "type": "num", "default": 3 } },
        "scene": { "kind": "group", "key": "area", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 200] } },
            "on": { "brush": { "brush": "sel" } },
            "children": [{ "kind": "repeat", "from": "t", "template": { "kind": "shape",
                "geom": { "type": "rect", "x": "=d.x", "y": 0, "w": 100, "h": 50 }, "fill": "$accent",
                "on": { "activate": { "toggle": "picked", "value": "=d.k" } } } }] }
    }));
    let s = e.signal_values();
    assert_eq!(s["picked"], serde_json::json!([]));
    assert_eq!(s["n"], serde_json::json!(3.0));
    assert_eq!(s["sel"], serde_json::json!([]), "a range brushed on no bands");
    assert_eq!(s["viewport.w"], serde_json::json!(200.0));
    e.pointer(Pointer::Down { x: 150.0, y: 20.0 });
    e.pointer(Pointer::Up { x: 150.0, y: 20.0 });
    e.pointer(Pointer::Down { x: 20.0, y: 40.0 });
    e.pointer(Pointer::Move { x: 120.0, y: 40.0 });
    e.pointer(Pointer::Up { x: 120.0, y: 40.0 });
    let s = e.signal_values();
    assert_eq!(s["picked"], serde_json::json!(["b"]));
    assert_eq!(s["sel.active"], serde_json::json!(true));
    assert!((s["sel.lo"].as_f64().unwrap() - 10.0).abs() < 1e-9, "{:?}", s["sel.lo"]);
    assert!((s["sel.hi"].as_f64().unwrap() - 60.0).abs() < 1e-9, "{:?}", s["sel.hi"]);
    // What a host reads, it can set back.
    e.set_signal_json("picked", &s["picked"]);
    assert_eq!(e.signal_values()["picked"], serde_json::json!(["b"]));
}
