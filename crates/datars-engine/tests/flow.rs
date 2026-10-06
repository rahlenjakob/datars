//! The `flow` layout: a repeat's nodes placed by their widths, wrapping at the box edge.

use datars_engine::Engine;

#[test]
fn nodes_flow_by_width_and_wrap() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b", "c", "d"], "w": [30, 20, 45, 10] }, "key": ["k"] } },
        "scene": { "kind": "group", "key": "chips", "layout": { "type": "flow", "gap": 10 }, "children": [
            { "kind": "repeat", "from": "t", "template": { "kind": "shape", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=d.w", "h": 12 }, "fill": "$accent" } }
        ] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let snap = e.scene().snapshot();
    let at = |k: &str| {
        let l = snap.lines().find(|l| l.trim_start().starts_with(&format!("(\"{k}\",)"))).unwrap_or_default();
        l.split_once("transform=[").map(|(_, t)| t.split(' ').skip(4).take(2).map(|v| v.trim_end_matches(']').parse::<f64>().unwrap()).collect::<Vec<_>>()).unwrap_or(vec![0.0, 0.0])
    };
    assert_eq!(at("a"), [0.0, 0.0], "{snap}");
    assert_eq!(at("b"), [40.0, 0.0], "30 wide + 10 gap");
    assert_eq!(at("c"), [0.0, 22.0], "70 + 45 would cross 100: a new line, 12 high + 10 gap");
    assert_eq!(at("d"), [55.0, 22.0]);
}
