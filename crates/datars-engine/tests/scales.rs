//! Scale behaviour that charts rely on across interactions.

use datars_engine::Engine;
use datars_expr::Value;

#[test]
fn categorical_colours_survive_filters() {
    // A cross-filter leaves one category; it must keep its colour, not take the palette's first.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 100 },
        "data": { "src": { "values": { "k": ["a", "b", "c"], "v": [1, 2, 3] }, "key": ["k"] } },
        "tables": { "shown": { "from": "src", "ops": [{ "op": "filter", "expr": { "expr": "only == '' || d.k == only" } }] } },
        "signals": { "only": { "type": "str", "default": "" } },
        "scene": { "kind": "group", "key": "g", "scales": { "color": { "type": "categorical", "domain": { "data": "shown", "field": "k" }, "range": "$categorical" } },
            "children": [{ "kind": "repeat", "from": "shown", "template": { "kind": "shape",
                "geom": { "type": "rect", "x": "=d.v * 50", "y": 0, "w": 40, "h": 40 }, "fill": "=scale.color(d.k)" } }] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let before = e.scene().snapshot();
    let line_c = |snap: &str| snap.lines().find(|l| l.contains("(\"c\",)")).map(String::from).unwrap_or_default();
    let c_before = line_c(&before);
    assert!(c_before.contains("fill="), "{before}");
    e.set_signal("only", Value::Str("c".into()));
    let after = e.scene().snapshot();
    assert!(!after.contains("(\"a\",)"), "filtered: {after}");
    assert_eq!(line_c(&after), c_before, "c keeps its colour");
}

/// `pow` maps through `|x|^exponent` (it mapped like `linear` — there was no exponent): 2 squares,
/// 0.5 is `sqrt`, none is linear; `invert` undoes it.
#[test]
fn pow_scales_use_their_exponent() {
    let at = |scale: serde_json::Value, expr: &str| -> f64 {
        let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
            "scene": { "kind": "group", "key": "g", "scales": { "x": scale },
                "children": [{ "kind": "shape", "key": "m", "geom": { "type": "rect", "x": expr, "y": 0, "w": 1, "h": 1 }, "fill": "$accent" }] } });
        let mut e = Engine::new();
        let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
        assert!(diags.is_empty(), "{diags:?}");
        let s = e.scene();
        let m = s.root.children().first().expect("the mark");
        let datars_scene::NodeKind::Shape { geom: datars_scene::Geom::Rect { x, .. }, .. } = &m.kind else { panic!("{}", s.snapshot()) };
        *x
    };
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let pow = |exponent: serde_json::Value| serde_json::json!({ "type": "pow", "exponent": exponent, "domain": [0, 10], "range": [0, 100] });
    assert!(close(at(pow(serde_json::json!(2)), "=scale.x(5)"), 25.0), "(5/10)² of 100");
    assert!(close(at(pow(serde_json::json!(2)), "=scale.x.invert(25)"), 5.0));
    assert!(close(at(pow(serde_json::json!(0.5)), "=scale.x(2.5)"), at(serde_json::json!({ "type": "sqrt", "domain": [0, 10], "range": [0, 100] }), "=scale.x(2.5)")), "pow 0.5 is sqrt");
    assert!(close(at(serde_json::json!({ "type": "pow", "domain": [0, 10], "range": [0, 100] }), "=scale.x(5)"), 50.0), "exponent 1 by default");
}
