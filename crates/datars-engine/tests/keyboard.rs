//! The keyboard's way into what a pointer drags — a brush (Enter, the arrows, Enter, Escape) and a
//! pannable view (the arrows, + and −, 0) — and what an interaction changed, said in words for the
//! reader's live region, in the document's language.

use datars_engine::Engine;
use datars_expr::Value;

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

fn brushable(scale: serde_json::Value, locale: &str) -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "locale": locale, "size": { "width": 200, "height": 100 },
        "scene": { "kind": "group", "key": "area", "scales": { "x": scale }, "semantics": { "role": "group", "label": "Prices" },
            "on": { "brush": { "brush": "sel" } },
            "children": [{ "kind": "shape", "key": "bg", "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 100 }, "fill": "$surface" }] }
    })
}

/// The keyboard-operable item a host finds among the semantics: (path, label).
fn operable(e: &mut Engine, kind: &str) -> (String, String) {
    let i = e.semantic_items().into_iter().find(|i| i.kind.as_deref() == Some(kind)).unwrap_or_else(|| panic!("no {kind} item"));
    assert!(i.actionable && i.role == "control");
    (i.path, i.label)
}

#[test]
fn a_brush_is_a_keyboard_selection_too() {
    let mut e = engine(brushable(serde_json::json!({ "type": "linear", "domain": [0, 100], "range": [0, 200] }), "en"));
    let (path, label) = operable(&mut e, "brush");
    assert_eq!(label, "Prices, range selection: nothing selected", "named by the part it's in, saying what it holds");
    assert!(!e.key(&path, "ArrowRight"), "arrows before Enter are the host's (they walk the data)");
    assert!(e.key(&path, "Enter"));
    assert_eq!(e.take_announcement().as_deref(), Some("Selecting 0 to 10."));
    assert_eq!(e.take_announcement(), None, "said once");
    assert_eq!((e.signal("sel.active"), num(&e, "sel.lo"), num(&e, "sel.hi")), (Some(Value::Bool(true)), 0.0, 10.0));
    e.key(&path, "ArrowRight");
    e.key(&path, "ArrowRight");
    assert_eq!(e.take_announcement().as_deref(), Some("0 to 30."));
    e.key(&path, "Shift+ArrowRight");
    assert_eq!((num(&e, "sel.lo"), num(&e, "sel.hi")), (10.0, 40.0), "Shift and an arrow move it");
    e.key(&path, "ArrowLeft");
    assert_eq!(num(&e, "sel.hi"), 30.0);
    assert!(e.key(&path, "Enter"));
    assert_eq!(e.take_announcement().as_deref(), Some("Selected 10 to 30."));
    assert_eq!(operable(&mut e, "brush").1, "Prices, range selection: 10 to 30");
    // Again: Enter edits the one there is; Escape puts it back.
    e.key(&path, "Enter");
    e.key(&path, "ArrowRight");
    assert_eq!(num(&e, "sel.hi"), 40.0);
    assert!(e.key(&path, "Escape"));
    assert_eq!(e.take_announcement().as_deref(), Some("Selection cancelled."));
    assert_eq!((num(&e, "sel.lo"), num(&e, "sel.hi")), (10.0, 30.0));
    // Escape with nothing under way clears it; with nothing there it's the host's.
    assert!(e.key(&path, "Escape"));
    assert_eq!(e.take_announcement().as_deref(), Some("Selection cleared."));
    assert_eq!(e.signal("sel.active"), Some(Value::Bool(false)));
    assert!(!e.key(&path, "Escape"));
}

#[test]
fn a_band_brush_steps_a_band_at_a_time_and_says_them_by_name() {
    let mut doc = brushable(serde_json::json!({ "type": "band", "domain": ["a", "b", "c", "d"], "range": [0, 200], "padding": 0 }), "sv-SE");
    doc["keys"] = serde_json::json!({ "a": { "name": "Äpplen" }, "b": { "name": "Päron" } });
    let mut e = engine(doc);
    let (path, label) = operable(&mut e, "brush");
    assert_eq!(label, "Prices, intervallval: inget valt");
    e.key(&path, "Enter");
    assert_eq!(e.take_announcement().as_deref(), Some("Väljer Äpplen."));
    e.key(&path, "ArrowRight");
    e.key(&path, "ArrowRight");
    e.key(&path, "Enter");
    assert_eq!(e.take_announcement().as_deref(), Some("Valt Äpplen, Päron, c."));
    match e.signal("sel") {
        Some(Value::Str(s)) => assert_eq!(s.split('\u{1f}').collect::<Vec<_>>(), ["a", "b", "c"]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_view_pans_and_zooms_from_the_keyboard() {
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "scene": { "kind": "view", "key": "map", "camera": { "fit": { "bbox": [0, 0, 100, 100] }, "padding": 0, "explore": "cam" },
            "children": [{ "kind": "shape", "key": "land", "geom": { "type": "rect", "x": 0, "y": 0, "w": 100, "h": 100 }, "fill": "$accent" }] }
    }));
    let (path, _) = operable(&mut e, "pan");
    assert!(e.key(&path, "ArrowRight"));
    assert_eq!(e.take_announcement().as_deref(), Some("Moved right."));
    e.frame(5.0);
    assert!((num(&e, "cam.x") - 60.0).abs() < 1e-9, "a tenth of the view: {}", num(&e, "cam.x"));
    assert!(e.key(&path, "+"));
    assert_eq!(e.take_announcement().as_deref(), Some("Zoom 1.3 ×."));
    assert!((num(&e, "cam.zoom") - 1.25).abs() < 1e-9);
    e.frame(10.0);
    assert!(e.key(&path, "0"));
    e.frame(15.0);
    assert_eq!((num(&e, "cam.x"), num(&e, "cam.zoom")), (50.0, 1.0), "0: the whole again");
    assert!(!e.key(&path, "Tab"), "other keys are the host's");
}

/// Bars a legend filters: a click on a legend entry hides that fruit, and a total says what's left.
fn filtered() -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 120 },
        "data": { "fruit": { "values": { "name": ["Apples", "Pears", "Plums"], "kg": [42, 18, 27] }, "key": ["name"] } },
        "signals": { "hidden": { "type": "keyset", "default": [] } },
        "tables": { "shown": { "from": "fruit", "ops": [{ "op": "filter", "expr": { "expr": "!hidden.has(d.name)" } }] } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "group", "key": "total", "semantics": { "role": "group", "label": { "expr": "'Total: ' + format(table.sum('shown', 'kg'), 'd') + ' kg'" } }, "children": [] },
            { "kind": "group", "key": "legend", "children": [{ "kind": "repeat", "from": "fruit", "template": { "kind": "shape", "key": "=d.name", "geom": { "type": "rect", "x": "=d.index * 30", "y": 0, "w": 20, "h": 10 }, "fill": "$accent",
                "semantics": { "role": "legend-item", "label": { "expr": "d.name + (hidden.has(d.name) ? ', hidden' : '')" } },
                "on": { "activate": { "toggle": "hidden", "value": "=d.name" } } } }] },
            { "kind": "group", "key": "bars", "children": [{ "kind": "repeat", "from": "shown", "template": { "kind": "shape", "key": "=d.name", "geom": { "type": "rect", "x": "=d.index * 30", "y": 40, "w": 20, "h": "=d.kg" }, "fill": "$accent",
                "semantics": { "role": "datum", "label": { "expr": "d.name + ': ' + d.kg" }, "value": "=d.kg" } } }] }
        ] }
    })
}

#[test]
fn a_click_says_what_it_changed() {
    let mut e = engine(filtered());
    let pears = e.semantic_items().into_iter().find(|i| i.label == "Pears" && i.actionable).expect("a clickable legend entry").path;
    assert!(e.activate(&pears));
    assert_eq!(e.take_announcement().as_deref(), Some("Pears, hidden. Total: 69 kg. Now 2 data points, from 27 to 42."));
    assert!(e.activate(&pears));
    assert_eq!(e.take_announcement().as_deref(), Some("Pears. Total: 87 kg. Now 3 data points, from 18 to 42."));
}

#[test]
fn a_click_that_changes_nothing_a_reader_hears_says_nothing() {
    let mut doc = filtered();
    // The click now only toggles a signal nothing reads.
    doc["scene"]["children"][1]["children"][0]["template"]["on"]["activate"] = serde_json::json!({ "toggle": "unused", "value": "=d.name" });
    doc["signals"]["unused"] = serde_json::json!({ "type": "keyset", "default": [] });
    let mut e = engine(doc);
    let pears = e.semantic_items().into_iter().find(|i| i.label == "Pears" && i.actionable).unwrap().path;
    assert!(e.activate(&pears));
    assert_eq!(e.take_announcement(), None);
}

#[test]
fn keys_are_recorded_and_replay() {
    let mut e = engine(brushable(serde_json::json!({ "type": "linear", "domain": [0, 100], "range": [0, 200] }), "en"));
    e.start_recording();
    e.frame(1.0);
    let (path, _) = operable(&mut e, "brush");
    for k in ["Enter", "ArrowRight", "ArrowRight", "Enter"] {
        e.key(&path, k);
    }
    let s = e.take_recording().unwrap();
    let r = Engine::replay(&s, |_, _| {}).unwrap();
    assert_eq!((num(&r, "sel.lo"), num(&r, "sel.hi")), (0.0, 30.0));
}

#[test]
fn a_brushed_range_is_said_when_the_drag_ends() {
    let mut e = engine(brushable(serde_json::json!({ "type": "linear", "domain": [0, 100], "range": [0, 200] }), "en"));
    e.pointer(datars_engine::Pointer::Down { x: 20.0, y: 50.0 });
    e.pointer(datars_engine::Pointer::Move { x: 120.0, y: 50.0 });
    e.pointer(datars_engine::Pointer::Up { x: 120.0, y: 50.0 });
    assert_eq!(e.take_announcement().as_deref(), Some("Selected 10 to 60."));
}

#[test]
fn a_brush_that_filters_says_how_much_data_is_shown_now() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/prices/doc.json")).unwrap();
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&json).unwrap());
    e.frame(0.0);
    let (path, _) = operable(&mut e, "brush");
    for k in ["Enter", "ArrowRight", "Enter"] {
        assert!(e.key(&path, k), "{k}");
    }
    let said = e.take_announcement().unwrap();
    // (The detail's 100 days now, and the overview's 365, counted as drawn — past the 200 listed.)
    assert_eq!(said, "Selected 1 to 100. Now 465 data points.");
}

