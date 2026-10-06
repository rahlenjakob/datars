//! Map credits: our tile archives are OpenStreetMap data, whose licence requires the credit on
//! the map — a chart that draws tiles without it is flagged.

use datars_engine::Engine;

fn lint(doc: serde_json::Value) -> Vec<datars_devtools::Finding> {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    datars_devtools::lint(&mut e)
}

fn map(credit: bool) -> serde_json::Value {
    let mut kids = vec![serde_json::json!({ "kind": "group", "key": "map" })];
    if credit {
        kids.push(serde_json::json!({ "kind": "use", "recipe": "@datars/std/attribution", "params": {} }));
    }
    serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 200 },
        "data": { "base": { "tiles": "basemap.pmtiles" } },
        "scene": { "kind": "group", "key": "root", "children": kids } })
}

#[test]
fn tiles_without_the_openstreetmap_credit_are_flagged() {
    let without = lint(map(false));
    assert!(without.iter().any(|f| f.rule == "maps/attribution"), "{without:?}");
    let with = lint(map(true));
    assert!(!with.iter().any(|f| f.rule == "maps/attribution"), "{with:?}");
}

/// Two marks per row keyed alike (a day's candle and its volume bar) keep their identity through a
/// range change: identity is parent and key together, not the shared parent alone.
#[test]
fn two_marks_per_row_keyed_alike_keep_their_identity() {
    let days: Vec<String> = (1..=20).map(|d| format!("2025-03-{d:02}")).collect();
    let n = days.len();
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 240 },
        "data": { "p": { "values": { "date": days, "open": vec![10.0; n], "high": vec![12.0; n], "low": vec![9.0; n], "close": vec![11.0; n], "volume": vec![5.0; n] }, "key": ["date"] } },
        "signals": { "last": { "type": "num", "default": 20 } },
        "tables": { "shown": { "from": "p", "ops": [{ "op": "window", "fn": "rank", "field": "date", "as": "age", "order": "-date" }, { "op": "filter", "expr": "=d.age <= last" }] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "shown", "x": "date", "y": "close", "xType": "band",
            "children": [{ "kind": "use", "recipe": "@datars/std/candlestick", "params": {} }, { "kind": "use", "recipe": "@datars/std/volume", "params": {} }] } },
        "program": { "states": [{ "name": "all", "set": { "last": 20 } }, { "name": "recent", "set": { "last": 8 } }] } });
    let found = lint(doc);
    assert!(!found.iter().any(|f| f.rule == "identity/mismatch"), "{found:?}");
}

/// A control's hit areas take the control's label (what a screen reader announces); a clickable
/// shape with no label anywhere above it is still flagged.
#[test]
fn a_hit_area_under_a_labelled_control_is_labelled() {
    let doc = |label: &str| serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 60 },
        "signals": { "on": { "type": "bool", "default": false } },
        "scene": { "kind": "group", "key": "switch", "semantics": { "role": "control", "label": label }, "on": { "activate": { "set": "on", "value": "=!on" } },
            "children": [{ "kind": "shape", "key": "hit", "pickable": true, "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 60 }, "fill": "transparent" }] } });
    let flagged = |f: &Vec<datars_devtools::Finding>| f.iter().any(|f| f.rule == "a11y/semantics");
    assert!(!flagged(&lint(doc("Labels: off"))), "labelled by its control");
    assert!(flagged(&lint(doc(""))), "no label anywhere");
}
