//! How bare names resolve in expressions.

use datars_engine::Engine;

#[test]
fn a_column_never_hides_a_signal() {
    // A `time` column next to a `time` signal: bare `time` is the signal, `d.time` the row.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "signals": { "time": { "type": "num", "default": 7 } },
        "data": { "t": { "values": { "k": ["a"], "time": [3], "v": [1] }, "key": ["k"] } },
        "scene": { "kind": "repeat", "from": "t", "template": { "kind": "group", "children": [
            { "kind": "text", "key": "sig", "text": "=format(time, 'd')", "at": [0, 20] },
            { "kind": "text", "key": "row", "text": "=format(d.time, 'd')", "at": [0, 40] },
            { "kind": "text", "key": "bare", "text": "=format(v, 'd')", "at": [0, 60] }
        ] } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let snap = e.scene().snapshot();
    let text = |key: &str| snap.lines().find(|l| l.contains(&format!("(\"{key}\",) text"))).map(String::from).unwrap_or_default();
    assert!(text("sig").contains("text \"7\""), "the signal: {snap}");
    assert!(text("row").contains("text \"3\""), "the row: {snap}");
    assert!(text("bare").contains("text \"1\""), "a bare name without a signal still reads the row: {snap}");
}

#[test]
fn aggregates_read_derived_tables_nobody_asked_for_yet() {
    // `table.max` over a derived table that no mark draws: computed on demand, not null.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b", "c"], "v": [1, 5, 3] }, "key": ["k"] } },
        "tables": { "big": { "from": "t", "ops": [{ "op": "filter", "expr": "=d.v > 2" }] } },
        "scene": { "kind": "group", "children": [
            { "kind": "text", "key": "max", "text": "=format(table.max('big', 'v'), 'd')", "at": [0, 20] },
            { "kind": "text", "key": "n", "text": "=format(table.count('big'), 'd')", "at": [0, 40] }
        ] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let snap = e.scene().snapshot();
    assert!(snap.contains("(\"max\",) text \"5\""), "{snap}");
    assert!(snap.contains("(\"n\",) text \"2\""), "{snap}");
}

#[test]
fn a_font_arriving_snaps_the_text_into_place() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 40 },
        "data": { "he": { "font": "NotoSansHebrew-Regular.ttf" } },
        "scene": { "kind": "text", "key": "t", "text": "שלום", "at": [0, 20] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(e.requests().iter().any(|r| matches!(r, datars_engine::Request::Source { name, .. } if name == "he")), "the font is requested");
    e.frame(1.0);
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/fonts/NotoSansHebrew-Regular.ttf")).unwrap();
    e.provide("he", &bytes).unwrap();
    assert!(e.requests().is_empty());
    let f = e.frame(1.0);
    assert!(!f.animating, "no transition from missing glyphs to real ones");
    assert!(f.scene.to_json().contains("Noto Sans Hebrew"), "the glyph runs use the new face");
}
