//! What the reader can't see isn't there: an element faded to (nearly) nothing — its own opacity
//! times its ancestors' — is passed through by the pointer and not read out by screen readers.
//! (A county map faded out over a street map gave tooltips and listed its regions.)

use datars_engine::{Engine, Pointer};
use datars_expr::Value;

/// A street map with a county map over it; `lan` fades the county map in (1) or out (0).
fn layers(lan: f64) -> Engine {
    let region = |key: &str, label: &str| serde_json::json!({ "kind": "shape", "key": key, "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 100 },
        "fill": "$accent", "pickable": true, "semantics": { "role": "region", "label": label } });
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "towns": { "values": { "k": ["a", "b"], "x": [50, 150], "o": [1, 0] }, "key": ["k"] } },
        "signals": { "lan": { "type": "num", "default": lan } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "view", "key": "streets", "camera": { "fit": { "bbox": [0, 0, 200, 100] }, "padding": 0, "explore": "street" },
              "children": [region("street", "Street")] },
            { "kind": "group", "key": "counties", "opacity": "=lan", "children": [
                { "kind": "view", "key": "map", "camera": { "fit": { "bbox": [0, 0, 200, 100] }, "padding": 0, "explore": "county" },
                  "children": [region("county", "County"),
                    { "kind": "instances", "key": "towns", "from": "towns", "instance_key": "=d.k", "x": "=d.x", "y": 50, "r": 10,
                      "instance_opacity": "=d.o", "label": "=`Town ${d.k}`", "semantics": { "role": "datum", "label": "Towns" } }] }] }] }
    });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e
}

fn labels(e: &mut Engine) -> Vec<String> {
    e.semantic_items().into_iter().map(|i| i.label).collect()
}

#[test]
fn a_faded_out_layer_is_not_picked() {
    let mut shown = layers(1.0);
    assert_eq!(shown.pointer(Pointer::Move { x: 100.0, y: 20.0 }).as_deref(), Some("County"));
    let mut faded = layers(0.0);
    assert_eq!(faded.pointer(Pointer::Move { x: 100.0, y: 20.0 }).as_deref(), Some("Street"), "the pointer passes through to what's visible");
    assert_eq!(faded.pointer(Pointer::Move { x: 50.0, y: 50.0 }).as_deref(), Some("Street"), "its instances too");
}

#[test]
fn an_invisible_instance_is_not_picked() {
    let mut e = layers(1.0);
    assert_eq!(e.pointer(Pointer::Move { x: 50.0, y: 50.0 }).as_deref(), Some("Town a"));
    // Town b has opacity 0: the county under it answers.
    assert_eq!(e.pointer(Pointer::Move { x: 150.0, y: 50.0 }).as_deref(), Some("County"));
}

#[test]
fn a_faded_out_layer_is_not_read_out() {
    let all = labels(&mut layers(1.0));
    assert!(all.contains(&"County".to_string()) && all.contains(&"Town a".to_string()), "{all:?}");
    assert!(!all.contains(&"Town b".to_string()), "an invisible instance isn't read: {all:?}");
    let faded = labels(&mut layers(0.0));
    assert_eq!(faded, vec!["Street".to_string()], "only what's visible");
}

#[test]
fn a_faded_out_view_does_not_take_the_wheel() {
    let mut e = layers(0.0);
    e.pointer(Pointer::Wheel { x: 100.0, y: 50.0, delta: -500.0 });
    assert!(matches!(e.signal("street.zoom"), Some(Value::Num(z)) if z > 1.0), "the visible view zooms");
    assert!(e.signal("county.zoom").is_none(), "the invisible one doesn't");
}
