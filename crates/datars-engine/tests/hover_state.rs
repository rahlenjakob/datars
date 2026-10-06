//! `hover()` and the cursor: an element lights up while the pointer is over it (and fades back
//! when it leaves), only the elements some expression asks about cost a new scene, and the engine
//! says which cursor to show — a pointing hand over what a click acts on, grab over a view that
//! pans, a crosshair over a brushable area.

use datars_engine::{Cursor, Engine, Pointer};
use datars_scene::{Key, KeyPath};

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.set_clock(0.0);
    e.frame(0.0);
    e
}

/// A frame a while later: whatever the last input started has finished.
fn settle(e: &mut Engine, t: &mut f64) -> datars_engine::FrameOutput {
    *t += 1.0;
    e.set_clock(*t);
    e.frame(*t)
}

/// Two buttons, each a group that acts on a click, with a pickable hit area and a highlight that
/// shows while it's hovered; and a mark below that asks nothing.
fn buttons() -> serde_json::Value {
    let button = |key: &str, y: f64| {
        serde_json::json!({ "kind": "group", "key": key, "on": { "activate": { "set": "picked", "value": key } },
            "semantics": { "role": "control", "label": key },
            "children": [
                { "kind": "shape", "key": "hit", "pickable": true, "geom": { "type": "rect", "x": 0, "y": y, "w": 100, "h": 30 }, "fill": "$surface" },
                { "kind": "shape", "key": "glow", "geom": { "type": "rect", "x": 0, "y": y, "w": 100, "h": 30 }, "fill": "$accent", "opacity": "=hover() ? 0.2 : 0" }
            ] })
    };
    serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "signals": { "picked": { "type": "str", "default": "" } },
        "scene": { "kind": "group", "key": "root", "motion": [{ "duration": 0.9 }], "children": [
            button("a", 0.0), button("b", 40.0),
            { "kind": "shape", "key": "mark", "geom": { "type": "rect", "x": 0, "y": 120, "w": 100, "h": 30 }, "fill": "$ink",
              "semantics": { "role": "datum", "label": "a mark" }, "on": { "inspect": { "set": "picked", "value": "mark" } } }
        ] }
    })
}

fn glow(out: &datars_engine::FrameOutput, button: &str) -> f64 {
    let path = KeyPath(vec![Key::name("root"), Key::name(button), Key::name("glow")]);
    out.scene.find(&path).map(|n| n.common.opacity).unwrap_or_else(|| panic!("no glow in {button}"))
}

#[test]
fn an_element_shows_hovered_while_the_pointer_is_over_it() {
    let mut e = engine(buttons());
    let mut t = 0.0;
    let out = settle(&mut e, &mut t);
    assert_eq!((glow(&out, "a"), glow(&out, "b")), (0.0, 0.0));

    e.pointer(Pointer::Move { x: 50.0, y: 15.0 });
    let out = settle(&mut e, &mut t);
    assert_eq!((glow(&out, "a"), glow(&out, "b")), (0.2, 0.0), "over a");

    e.pointer(Pointer::Move { x: 50.0, y: 55.0 });
    let out = settle(&mut e, &mut t);
    assert_eq!((glow(&out, "a"), glow(&out, "b")), (0.0, 0.2), "over b");

    e.pointer(Pointer::Move { x: 150.0, y: 190.0 });
    let out = settle(&mut e, &mut t);
    assert_eq!((glow(&out, "a"), glow(&out, "b")), (0.0, 0.0), "over nothing");

    e.pointer(Pointer::Move { x: 50.0, y: 15.0 });
    settle(&mut e, &mut t);
    e.pointer(Pointer::Leave);
    let out = settle(&mut e, &mut t);
    assert_eq!(glow(&out, "a"), 0.0, "left the chart");
}

#[test]
fn a_hover_state_fades_in_quickly_whatever_the_documents_motion() {
    let mut e = engine(buttons());
    let mut t = 0.0;
    settle(&mut e, &mut t);
    e.pointer(Pointer::Move { x: 50.0, y: 15.0 });
    let first = e.frame(t);
    assert!(first.animating, "hovering starts a transition");
    e.set_clock(t + 0.1);
    let mid = e.frame(t + 0.1);
    let g = glow(&mid, "a");
    assert!(g > 0.0 && g < 0.2, "half way, though the document's transitions take 0.9 s: {g}");
    e.set_clock(t + 0.25);
    assert_eq!(glow(&e.frame(t + 0.25), "a"), 0.2, "done within a fifth of a second");
}

#[test]
fn a_tap_never_leaves_an_element_looking_hovered() {
    let mut e = engine(buttons());
    let mut t = 0.0;
    settle(&mut e, &mut t);
    e.pointer(Pointer::Down { x: 50.0, y: 15.0 });
    e.pointer(Pointer::Tap { x: 50.0, y: 15.0 });
    let out = settle(&mut e, &mut t);
    assert_eq!(glow(&out, "a"), 0.0);
    assert_eq!(e.signal("picked"), Some(datars_expr::Value::Str("a".into())), "the tap still clicks");
}

#[test]
fn moving_over_elements_nothing_asks_about_starts_no_transition() {
    let mut e = engine(buttons());
    let mut t = 0.0;
    settle(&mut e, &mut t);
    // The mark isn't under any `hover()`: moving on and off it changes nothing drawn.
    e.pointer(Pointer::Move { x: 50.0, y: 135.0 });
    e.pointer(Pointer::Move { x: 150.0, y: 190.0 });
    t += 0.01;
    e.set_clock(t);
    assert!(!e.frame(t).animating);
}

#[test]
fn the_cursor_says_what_the_pointer_can_do() {
    let mut e = engine(buttons());
    e.pointer(Pointer::Move { x: 50.0, y: 15.0 });
    assert_eq!(e.cursor(), Cursor::Pointer, "a button");
    e.pointer(Pointer::Move { x: 50.0, y: 135.0 });
    assert_eq!(e.cursor(), Cursor::Default, "a mark that only shows a tooltip");
    e.pointer(Pointer::Move { x: 150.0, y: 190.0 });
    assert_eq!(e.cursor(), Cursor::Default, "nothing");
    e.pointer(Pointer::Leave);
    assert_eq!(e.cursor(), Cursor::Default);

    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "scene": { "kind": "group", "key": "area", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 200] } },
            "on": { "brush": { "brush": "sel" } },
            "children": [{ "kind": "shape", "key": "bg", "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 100 }, "fill": "$surface" }] }
    }));
    e.pointer(Pointer::Move { x: 50.0, y: 50.0 });
    assert_eq!(e.cursor(), Cursor::Crosshair, "a brushable area");

    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "scene": { "kind": "view", "key": "map", "camera": { "fit": { "bbox": [0, 0, 100, 100] }, "padding": 0, "explore": "cam" },
            "children": [{ "kind": "shape", "key": "bg", "geom": { "type": "rect", "x": 0, "y": 0, "w": 100, "h": 100 }, "fill": "$surface" }] }
    }));
    e.pointer(Pointer::Move { x: 100.0, y: 100.0 });
    assert_eq!(e.cursor(), Cursor::Grab, "a view that pans");
    e.pointer(Pointer::Down { x: 100.0, y: 100.0 });
    e.pointer(Pointer::Move { x: 120.0, y: 100.0 });
    assert_eq!(e.cursor(), Cursor::Grabbing, "while panning");
    e.pointer(Pointer::Up { x: 120.0, y: 100.0 });
    assert_eq!(e.cursor(), Cursor::Grab, "after");
}

#[test]
fn every_element_of_a_repeat_can_be_hovered() {
    // Forty rows made by one template: each asks `hover()` of its own group, one after another.
    let rows: Vec<i64> = (0..40).collect();
    let mut e = engine(serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 400 },
        "data": { "rows": { "values": { "i": rows }, "key": ["i"] } },
        "signals": { "picked": { "type": "num", "default": -1 } },
        "scene": { "kind": "group", "key": "root", "motion": [{ "duration": 0.1 }], "children": [
            { "kind": "repeat", "from": "rows", "template": { "kind": "group", "key": "=d.i", "on": { "activate": { "set": "picked", "value": "=d.i" } },
              "semantics": { "role": "control", "label": "=format(d.i, 'd')" }, "children": [
                { "kind": "shape", "key": "hit", "pickable": true, "geom": { "type": "rect", "x": 0, "y": "=d.i * 10", "w": 100, "h": 10 }, "fill": "$surface" },
                { "kind": "shape", "key": "glow", "geom": { "type": "rect", "x": 0, "y": "=d.i * 10", "w": 100, "h": 10 }, "fill": "$accent", "opacity": "=hover() ? 0.3 : 0" } ] } }
        ] }
    }));
    let mut t = 0.0;
    for i in [0i64, 1, 2, 17, 18, 39] {
        e.pointer(Pointer::Move { x: 50.0, y: i as f64 * 10.0 + 5.0 });
        let out = settle(&mut e, &mut t);
        let lit: Vec<i64> = (0..40).filter(|&k| out.scene.find(&KeyPath(vec![Key::name("root"), Key::one(k), Key::name("glow")])).is_some_and(|n| n.common.opacity > 0.0)).collect();
        assert_eq!(lit, vec![i], "over row {i}");
    }
}
