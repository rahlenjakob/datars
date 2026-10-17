//! What hosts hand screen readers: the chart's name (the document's title, else its root's label)
//! and a summary — the document's description, how much data the chart shows and its range — from
//! the current state, so it follows steps and filters; data marks carry their values to hosts.

use datars_engine::Engine;
use serde_json::json;

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn bars(title: &str, description: &str) -> serde_json::Value {
    json!({ "datars": 1, "title": title, "description": description, "size": { "width": 400, "height": 240 },
        "signals": { "big": { "type": "bool", "default": false } },
        "data": { "t": { "values": { "k": ["a", "b", "c", "d"], "n": [1200, 30, 4500, 7] }, "key": ["k"] } },
        "tables": { "shown": { "from": "t", "ops": [{ "op": "filter", "expr": { "expr": "!big || d.n > 100" } }] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "shown", "x": "k", "y": "n", "title": "Things", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] } } })
}

#[test]
fn the_name_is_the_title_and_the_summary_follows_the_data() {
    let mut e = engine(bars("Things we counted", "How many of each thing"));
    assert_eq!(e.accessible_name(), "Things we counted");
    assert_eq!(e.accessible_summary(), "How many of each thing. 4 data points, from 7 to 4,500.");
    // A filter: the summary says what's shown now.
    e.set_signal("big", datars_engine::SignalValue::Bool(true));
    assert_eq!(e.accessible_summary(), "How many of each thing. 2 data points, from 1,200 to 4,500.");
    // Without a title, the root's label names it; without a description, the data alone.
    let mut e = engine(bars("", ""));
    assert_eq!(e.accessible_name(), "Things");
    assert_eq!(e.accessible_summary(), "4 data points, from 7 to 4,500.");
    let values: Vec<Option<f64>> = e.semantic_items().into_iter().filter(|i| i.role == "datum").map(|i| i.value).collect();
    assert_eq!(values, vec![Some(1200.0), Some(30.0), Some(4500.0), Some(7.0)], "hosts get each mark's value");
}

#[test]
fn a_range_over_only_some_marks_is_not_said() {
    // Two marks say a value, one doesn't: "from … to …" would describe part of the data as all.
    let mut e = engine(json!({ "datars": 1, "size": { "width": 200, "height": 100 }, "scene": { "kind": "group", "key": "root", "children": [
        { "kind": "shape", "key": "a", "geom": { "type": "rect", "x": 0, "y": 0, "w": 10, "h": 10 }, "fill": "$ink", "semantics": { "role": "datum", "label": "A", "value": 3 } },
        { "kind": "shape", "key": "b", "geom": { "type": "rect", "x": 20, "y": 0, "w": 10, "h": 10 }, "fill": "$ink", "semantics": { "role": "datum", "label": "B", "value": 9 } },
        { "kind": "shape", "key": "c", "geom": { "type": "rect", "x": 40, "y": 0, "w": 10, "h": 10 }, "fill": "$ink", "semantics": { "role": "datum", "label": "C" } }
    ] } }));
    assert_eq!(e.accessible_summary(), "3 data points.");
}
