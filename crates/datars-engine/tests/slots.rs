//! Data slots: a chart that ships without its data. The sample shows until the host provides the
//! user's rows; wrong rows are refused with the reason, and the chart keeps what it had.

use datars_engine::{Engine, Request};

fn doc() -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "spending": { "slot": "spending", "key": ["category"], "types": { "amount": "num" },
            "sample": [{ "category": "Food", "amount": 10 }, { "category": "Rent", "amount": 30 }] } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "repeat", "from": "spending", "template": { "kind": "text", "text": "=d.category + ' ' + d.amount", "at": [0, 0] } },
            // A composite key from expressions: (category, "total").
            { "kind": "repeat", "from": "spending", "template": { "kind": "shape", "key": ["=d.category", "total"], "geom": { "type": "rect", "x": 0, "y": 0, "w": "=d.amount", "h": 4 } } }
        ] }
    })
}

#[test]
fn the_sample_shows_until_the_host_provides_rows() {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc().to_string()).unwrap());
    assert!(e.requests().iter().any(|r| matches!(r, Request::Slot { name, .. } if name == "spending")), "the host is asked for the slot");
    let s = e.scene().snapshot();
    assert!(s.contains("Food 10") && s.contains("Rent 30"), "{s}");
    assert!(s.contains("(\"Food\", \"total\")"), "composite keys from expressions: {s}");

    e.provide("spending", br#"[{"category":"Food","amount":42},{"category":"Travel","amount":7}]"#).unwrap();
    let s = e.scene().snapshot();
    assert!(s.contains("Food 42") && s.contains("Travel 7") && !s.contains("Rent"), "{s}");
    assert!(!e.requests().iter().any(|r| matches!(r, Request::Slot { .. })));
}

#[test]
fn wrong_rows_are_refused_with_the_reason() {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc().to_string()).unwrap());
    let err = e.provide("spending", br#"[{"cat":"Food","sum":1}]"#).unwrap_err();
    assert!(err.contains("no column `amount`, `category`"), "{err}");
    let err = e.provide("spending", b"category,amount\nFood,lots\n").unwrap_err();
    assert!(err.contains("`amount` should hold num"), "{err}");
    assert!(e.scene().snapshot().contains("Food 10"), "the sample stays");
}

#[test]
fn recipe_settings_that_dont_exist_are_diagnosed_with_a_suggestion() {
    let doc = serde_json::json!({ "datars": 1, "data": { "t": { "values": { "k": ["a", "b"], "v": [1, 2] }, "key": ["k"] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "k", "y": "v", "titel": "Hi",
            "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": { "lables": true } }] } } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let _ = e.scene();
    let d: Vec<String> = e.diagnostics().iter().map(|d| d.message.clone()).collect();
    assert!(d.iter().any(|m| m.contains("has no setting titel (did you mean `title`?)")), "{d:?}");
    assert!(d.iter().any(|m| m.contains("has no setting lables (did you mean `labels`?)")), "{d:?}");
    // What a plot hands its marks (data, encodings) is not a typo.
    assert!(!d.iter().any(|m| m.contains("has no setting data") || m.contains("setting x")), "{d:?}");
}

#[test]
fn hosts_set_keysets_and_ranges_as_json() {
    let doc = serde_json::json!({ "datars": 1,
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "text", "key": "sel", "text": "=`${picked.size()} picked, a: ${picked.has('a')}`", "at": [0, 0] },
            { "kind": "text", "key": "range", "text": "=`${span.active} ${span.lo}-${span.hi}`", "at": [0, 20] }
        ] } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.set_signal_json("picked", &serde_json::json!(["a", "c"]));
    e.set_signal_json("span", &serde_json::json!({ "lo": 2, "hi": 5 }));
    let s = e.scene().snapshot();
    assert!(s.contains("2 picked, a: true") && s.contains("true 2-5"), "{s}");
    e.set_signal_json("picked", &serde_json::Value::Null);
    assert!(e.scene().snapshot().contains("0 picked"));
}
