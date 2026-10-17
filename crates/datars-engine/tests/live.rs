//! Live sources that merge (docs/06 §Live data): arrivals are rows — upserted by key, retained by
//! count or duration — and the transitions to them never outlast the feed.

use datars_engine::Engine;
use datars_scene::{Node, NodeKind};

fn engine(live: serde_json::Value, slot: bool) -> Engine {
    let source = if slot {
        serde_json::json!({ "slot": "ticks", "key": ["t"], "live": live, "sample": [{ "t": 0, "p": 1 }] })
    } else {
        serde_json::json!({ "values": [{ "t": 0, "p": 1 }], "key": ["t"], "live": live })
    };
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "ticks": source },
        // One bar per row, its height the price: what's on screen is the table.
        "scene": { "kind": "repeat", "from": "ticks", "template": { "kind": "shape", "geom": { "type": "rect", "x": "=d.t / 1000", "y": 0, "w": 4, "h": "=d.p" }, "fill": "$accent" } }
    });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

/// The bars' (x, h) once the scene has settled.
fn bars(e: &mut Engine, at: f64) -> Vec<(f64, f64)> {
    let f = e.frame(at);
    let mut out = Vec::new();
    fn walk(n: &Node, out: &mut Vec<(f64, f64)>) {
        if let NodeKind::Shape { geom: datars_scene::Geom::Rect { x, h, .. }, .. } = &n.kind {
            out.push((*x, *h));
        }
        for c in n.children() {
            walk(c, out);
        }
    }
    walk(&f.scene.root, &mut out);
    out
}

#[test]
fn upserts_merge_by_key_and_a_span_retires_old_rows() {
    let mut e = engine(serde_json::json!({ "mode": "upsert", "span": 60 }), false);
    assert_eq!(bars(&mut e, 0.0), vec![(0.0, 1.0)]);
    // The open minute moves; a new one joins.
    e.provide("ticks", br#"[{"t": 0, "p": 3}, {"t": 30000, "p": 5}]"#).unwrap();
    assert_eq!(bars(&mut e, 10.0), vec![(0.0, 3.0), (30.0, 5.0)]);
    // 90 s later the first row is more than 60 s older than the newest: it exits.
    e.provide("ticks", br#"[{"t": 90000, "p": 7}]"#).unwrap();
    assert_eq!(bars(&mut e, 20.0), vec![(30.0, 5.0), (90.0, 7.0)]);
}

#[test]
fn a_window_keeps_the_last_rows() {
    let mut e = engine(serde_json::json!({ "mode": "append", "window": 2 }), false);
    e.provide("ticks", br#"[{"t": 1000, "p": 2}, {"t": 2000, "p": 3}]"#).unwrap();
    assert_eq!(bars(&mut e, 10.0), vec![(1.0, 2.0), (2.0, 3.0)]);
}

#[test]
fn a_slots_sample_is_replaced_then_rows_merge() {
    let mut e = engine(serde_json::json!({ "mode": "append" }), true);
    assert_eq!(bars(&mut e, 0.0), vec![(0.0, 1.0)], "the sample until the host's data");
    e.provide("ticks", br#"[{"t": 5000, "p": 2}]"#).unwrap();
    assert_eq!(bars(&mut e, 10.0), vec![(5.0, 2.0)], "the host's first rows replace the sample");
    e.provide("ticks", br#"[{"t": 6000, "p": 4}]"#).unwrap();
    assert_eq!(bars(&mut e, 20.0), vec![(5.0, 2.0), (6.0, 4.0)], "later rows merge");
}

#[test]
fn snapshot_sources_still_replace() {
    let mut e = engine(serde_json::json!({ "every": 5 }), false);
    e.provide("ticks", br#"[{"t": 5000, "p": 2}]"#).unwrap();
    assert_eq!(bars(&mut e, 10.0), vec![(5.0, 2.0)]);
}

#[test]
fn a_live_transition_lasts_no_longer_than_the_time_between_arrivals() {
    let mut e = engine(serde_json::json!({ "mode": "upsert" }), false);
    e.frame(0.0);
    // The first arrival moves for the default 0.9 s (from the frame that first draws it).
    e.provide("ticks", br#"[{"t": 0, "p": 2}]"#).unwrap();
    assert!(e.frame(0.05).animating);
    assert!(e.frame(0.5).animating, "the first one takes its full time");
    // The next comes 0.45 s after the first started moving: it moves in 0.45 s, not 0.9.
    e.provide("ticks", br#"[{"t": 0, "p": 3}]"#).unwrap();
    assert!(e.frame(0.5).animating);
    assert!(e.frame(0.9).animating);
    assert!(!e.frame(1.0).animating, "capped to the feed's pace (uncapped it would run to 1.4)");
    assert_eq!(bars(&mut e, 1.0), vec![(0.0, 3.0)]);
    // Never shorter than a quarter second, however fast the feed.
    e.provide("ticks", br#"[{"t": 0, "p": 4}]"#).unwrap();
    assert!(e.frame(1.0).animating);
    e.provide("ticks", br#"[{"t": 0, "p": 5}]"#).unwrap();
    assert!(e.frame(1.05).animating);
    assert!(e.frame(1.25).animating, "a quarter second at least");
    assert!(!e.frame(1.31).animating);
    assert_eq!(bars(&mut e, 1.4), vec![(0.0, 5.0)]);
}

#[test]
fn signal_transitions_are_not_capped() {
    // Only live arrivals are paced by the feed: a step or a control takes its full time.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "signals": { "h": { "type": "num", "default": 1 } },
        "data": { "ticks": { "values": [{ "t": 0, "p": 1 }], "key": ["t"], "live": { "mode": "upsert" } } },
        "scene": { "kind": "shape", "key": "bar", "geom": { "type": "rect", "x": 0, "y": 0, "w": 4, "h": "=h" }, "fill": "$accent" }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.frame(0.0);
    e.provide("ticks", br#"[{"t": 0, "p": 2}]"#).unwrap();
    e.frame(1.0);
    e.set_signal("h", datars_engine::SignalValue::Num(50.0));
    e.frame(1.2);
    assert!(e.frame(2.0).animating, "a signal's transition runs its 0.9 s");
}

#[test]
fn rows_arrive_as_rows_where_the_response_wraps_them() {
    // The snapshot is an API's response (rows under `prices`); a feed sends just the rows.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "ticks": { "url": "ticks.json", "rows": "prices", "key": ["t"], "live": { "mode": "upsert" } } },
        "scene": { "kind": "repeat", "from": "ticks", "template": { "kind": "shape", "geom": { "type": "rect", "x": "=d.t / 1000", "y": 0, "w": 4, "h": "=d.p" }, "fill": "$accent" } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.provide("ticks", br#"{"prices": [{"t": 1000, "p": 2}]}"#).unwrap();
    e.provide("ticks", br#"[{"t": 2000, "p": 3}]"#).unwrap();
    let b = bars(&mut e, 10.0);
    assert!(b.contains(&(1.0, 2.0)) && b.contains(&(2.0, 3.0)), "{b:?}");
}

#[test]
fn a_column_declared_text_stays_text_when_it_looks_like_dates() {
    // A session named by its day ("2026-10-16") and later one named otherwise ("replay-1"): both
    // are text, so the rows merge.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "ticks": { "values": [{ "day": "2026-10-16", "t": 0, "p": 1 }], "key": ["day", "t"], "types": { "day": "str" }, "live": { "mode": "upsert" } } },
        "scene": { "kind": "repeat", "from": "ticks", "template": { "kind": "shape", "geom": { "type": "rect", "x": "=d.t / 1000", "y": 0, "w": 4, "h": "=d.p" }, "fill": "$accent" } }
    });
    let mut e = Engine::new();
    assert!(e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap()).is_empty());
    e.provide("ticks", br#"[{"day": "replay-1", "t": 0, "p": 2}]"#).unwrap();
    assert_eq!(bars(&mut e, 10.0).len(), 2);
}
