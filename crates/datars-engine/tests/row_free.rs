//! Values the resolver keeps for its life (expressions that read only signals, parsed inks) must
//! never leak between rows: a bare name that is a column varies per row.

use datars_engine::Engine;
use datars_expr::Value;

#[test]
fn only_signal_expressions_are_kept_across_rows() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 100 },
        "data": { "src": { "values": { "k": ["a", "b", "c"], "w": [1, 2, 3], "c": ["#ff0000", "#00ff00", "#0000ff"] }, "key": ["k"] } },
        "signals": { "zoom": { "type": "num", "default": 2 } },
        "scene": { "kind": "group", "key": "g", "children": [
            { "kind": "repeat", "from": "src", "template": { "kind": "shape", "key": "=d.k",
                "geom": { "type": "rect", "x": 0, "y": 0, "w": 10, "h": 10 },
                "fill": "=d.c",
                "stroke": { "paint": "#123456", "width": "=w" } } },
            { "kind": "repeat", "from": "src", "template": { "kind": "shape", "key": "='z' + d.k",
                "geom": { "type": "rect", "x": 0, "y": 0, "w": 10, "h": 10 },
                "stroke": { "paint": "$ink", "width": "=zoom >= 3 ? 5 : 1" } } }
        ] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let snap = e.scene().snapshot();
    let line = |snap: &str, k: &str| snap.lines().find(|l| l.contains(&format!("(\"{k}\",)"))).map(String::from).unwrap_or_default();
    // A column read by a bare name: each row its own width and fill.
    for (k, w, c) in [("a", 1, "#ff0000"), ("b", 2, "#00ff00"), ("c", 3, "#0000ff")] {
        let l = line(&snap, k);
        assert!(l.contains(&format!("fill={c} stroke=#123456/{w}.00")), "{k}: {l}\n{snap}");
    }
    // A signal expression: the same for every row, and a new value once the signal changes.
    assert!(["za", "zb", "zc"].iter().all(|k| line(&snap, k).contains("stroke=$ink/1.00")), "{snap}");
    e.set_signal("zoom", Value::Num(4.0));
    let snap = e.scene().snapshot();
    assert!(["za", "zb", "zc"].iter().all(|k| line(&snap, k).contains("stroke=$ink/5.00")), "{snap}");
}
