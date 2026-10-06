//! What the pointer finds: a line's values anywhere along it (`hit: line` instances), and taps —
//! touch has no hover — which inspect what they land on with a finger's reach.

use datars_engine::{Engine, Pointer};
use datars_expr::Value;

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e
}

/// Two lines of three points each, 100 px apart in x: one at y 50, one at y 90.
fn lines() -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 120 },
        "data": { "pts": { "values": { "s": ["a", "a", "a", "b", "b", "b"], "x": [0, 100, 200, 0, 100, 200], "y": [50, 50, 50, 90, 90, 90] }, "key": ["s", "x"] } },
        "scene": { "kind": "group", "key": "chart", "children": [
            { "kind": "repeat", "from": { "groups": "pts", "by": "s" }, "template": { "kind": "group", "key": "=d.s", "children": [
                { "kind": "instances", "key": "points", "from": "@group", "x": "=d.x + 50", "y": "=d.y", "r": 0, "fill": "transparent",
                  "instance_key": "=d.x", "label": "=d.s + ' ' + d.x", "hit": "line" }
            ] } }
        ] }
    })
}

#[test]
fn a_line_is_found_anywhere_along_it_with_its_nearest_point() {
    let mut e = engine(lines());
    // On line a, between its first two points (x 50 and 150): the nearer one's value.
    assert_eq!(e.pointer(Pointer::Move { x: 90.0, y: 52.0 }).as_deref(), Some("a 0"));
    assert_eq!(e.pointer(Pointer::Move { x: 110.0, y: 48.0 }).as_deref(), Some("a 100"));
    // Nearer line b: its value.
    assert_eq!(e.pointer(Pointer::Move { x: 240.0, y: 84.0 }).as_deref(), Some("b 200"));
    // Out of reach of both (16 px): nothing.
    assert_eq!(e.pointer(Pointer::Move { x: 100.0, y: 25.0 }), None);
    assert_eq!(e.pointer(Pointer::Move { x: 290.0, y: 50.0 }), None);
}

#[test]
fn a_tap_inspects_with_a_fingers_reach_and_a_tap_on_nothing_clears_it() {
    let mut e = engine(lines());
    // 25 px above line a: beyond a mouse's reach, within a finger's.
    assert_eq!(e.pointer(Pointer::Move { x: 150.0, y: 25.0 }), None);
    e.pointer(Pointer::Down { x: 150.0, y: 25.0 });
    assert_eq!(e.pointer(Pointer::Tap { x: 150.0, y: 25.0 }).as_deref(), Some("a 100"));
    assert!(matches!(e.signal("inspected"), Some(Value::Str(_))), "a tap inspects, as hovering does");
    e.pointer(Pointer::Down { x: 290.0, y: 115.0 });
    assert_eq!(e.pointer(Pointer::Tap { x: 290.0, y: 115.0 }), None);
    assert_eq!(e.signal("inspected"), None, "a tap on nothing clears it");
}

#[test]
fn std_line_charts_show_values_along_the_line() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 200 },
        "data": { "t": { "values": { "year": [2000, 2010, 2020], "v": [1, 3, 2] } } },
        "scene": { "kind": "group", "key": "plot", "scales": {
                "x": { "type": "linear", "domain": [2000, 2020], "range": [0, 400] },
                "y": { "type": "linear", "domain": [0, 4], "range": [200, 0] } },
            "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": { "data": "t", "x": "year", "y": "v" } }] }
    });
    let mut e = engine(doc);
    // The line runs from 2000 (1, at x 0) to 2010 (3, at x 200): hovered short of halfway, 2000's
    // value; past it, 2010's — not nothing between the points.
    let at = e.pointer(Pointer::Move { x: 90.0, y: 108.0 }).expect("the line has values along it");
    assert!(at.ends_with(": 1") && at.contains("2"), "{at}");
    let at = e.pointer(Pointer::Move { x: 110.0, y: 92.0 }).expect("past halfway: the next point");
    assert!(at.ends_with(": 3"), "{at}");
}
