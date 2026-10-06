//! Recipes' own motion: the defaults a recipe declares (a line draws on as it enters) apply where
//! the recipe is used, under the document's rules.

use datars_engine::Engine;

fn doc(rules: serde_json::Value) -> datars_ir::Doc {
    let j = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 200 },
        "signals": { "show": { "type": "bool", "default": false } },
        "data": { "t": { "values": { "x": [1, 2, 3, 4], "y": [3, 1, 4, 2] }, "key": ["x"] } },
        "motion": { "rules": rules },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "x", "y": "y",
            "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": {}, "when": "=show" }] } },
        "program": { "states": [{ "name": "empty", "set": { "show": false } }, { "name": "line", "set": { "show": true } }] }
    });
    datars_ir::Doc::from_json(&j.to_string()).unwrap()
}

/// The line's trim mid-transition, if it has one.
fn mid_trim(rules: serde_json::Value) -> Option<String> {
    let mut e = Engine::new();
    let diags = e.load(doc(rules));
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e.set_clock(1.0);
    e.goto(1);
    e.frame(1.0);
    let f = e.frame(1.3);
    assert!(f.animating);
    let snap = f.scene.snapshot();
    snap.lines().find(|l| l.contains("polyline")).map(|l| l.split_once("trim=").map(|(_, t)| t.split(' ').next().unwrap_or("").to_string()).unwrap_or_default())
}

#[test]
fn an_entering_line_draws_on() {
    let t = mid_trim(serde_json::json!([])).expect("a line mid-flight");
    let (a, b) = t.split_once("..").map(|(a, b)| (a.parse::<f64>().unwrap_or(-1.0), b.parse::<f64>().unwrap_or(-1.0))).unwrap_or((-1.0, -1.0));
    assert!(a == 0.0 && b > 0.0 && b < 1.0, "drawn from its start, part of the way: trim={t}");
}

#[test]
fn the_document_has_the_last_word() {
    // The document's own enter rule for polylines replaces the recipe's draw-on with a fade.
    let t = mid_trim(serde_json::json!([{ "select": { "kind": "polyline" }, "enter": { "opacity": 0 } }])).expect("a line mid-flight");
    assert!(t.is_empty(), "no trim under the document's rule: {t}");
}
