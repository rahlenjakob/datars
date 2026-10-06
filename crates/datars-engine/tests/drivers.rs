//! Program drivers: autoplay holds and advances, hosts can pause it, scroll scrub is exact both
//! ways, live sources are re-requested on schedule and their updates animate.

use datars_engine::{Engine, Request};

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn story(drivers: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 60 },
        "signals": { "w": { "type": "num", "default": 10 } },
        "scene": { "kind": "shape", "key": "bar", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=w", "h": 20 }, "fill": "$accent" },
        "program": { "states": [
            { "name": "a", "set": { "w": 10 }, "hold": 1 },
            { "name": "b", "set": { "w": 50 }, "hold": 1 },
            { "name": "c", "set": { "w": 90 }, "hold": 1 }
        ], "drivers": drivers }
    })
}

#[test]
fn autoplay_holds_then_advances_and_stops_at_the_end() {
    let mut e = engine(story(serde_json::json!(["autoplay"])));
    let f = e.frame(0.0);
    assert!(!f.animating);
    assert_eq!(f.wake_at, Some(1.0), "the hold of state a ends at t = 1");
    assert_eq!(e.state_index(), 0);
    let f = e.frame(1.0);
    assert_eq!(e.state_index(), 1, "autoplay moved on");
    assert!(f.animating && f.wake_at.is_none());
    let f = e.frame(1.0 + 0.95); // the default 0.9 s transition is over
    assert!(!f.animating);
    let settle = 1.95;
    assert_eq!(f.wake_at, Some(settle + 1.0));
    e.frame(settle + 1.0);
    assert_eq!(e.state_index(), 2);
    let f = e.frame(10.0);
    let f2 = e.frame(20.0);
    assert_eq!(e.state_index(), 2, "the last state stays");
    assert!(!f2.animating && f2.wake_at.is_none(), "nothing more to wake for: {:?} / {:?}", f.wake_at, f2.wake_at);
}

#[test]
fn a_paused_autoplay_does_not_advance() {
    let mut e = engine(story(serde_json::json!(["autoplay"])));
    e.set_playing(false);
    e.frame(0.0);
    let f = e.frame(5.0);
    assert_eq!(e.state_index(), 0);
    assert!(f.wake_at.is_none());
    e.set_playing(true);
    let f = e.frame(5.0);
    assert_eq!(f.wake_at, Some(6.0), "the hold restarts when playing resumes");
}

#[test]
fn without_the_driver_nothing_moves_on_its_own() {
    let mut e = engine(story(serde_json::json!(["steps"])));
    e.frame(0.0);
    let f = e.frame(100.0);
    assert_eq!(e.state_index(), 0);
    assert!(f.wake_at.is_none());
}

#[test]
fn scroll_scrub_is_exact_in_both_directions() {
    let mut e = engine(story(serde_json::json!([{ "scroll": "scrub" }])));
    let (_, _, plan) = e.plan_states(0, 1);
    let mid = e.plan_at(&plan, 0.5).hash();
    let b = e.scene_for_state(1).hash();
    assert_eq!(e.seek(0.5), 0);
    assert_eq!(e.frame(0.0).scene.hash(), mid);
    assert_eq!(e.seek(1.0), 1);
    assert_eq!(e.frame(0.0).scene.hash(), b);
    e.seek(1.7);
    e.seek(0.5); // back up
    assert_eq!(e.frame(0.0).scene.hash(), mid, "scrubbing back shows the same frame");
    assert_eq!(e.seek(9.0), 2, "clamped to the last state");
}

#[test]
fn live_sources_are_rerequested_and_updates_animate() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 60 },
        "data": { "count": { "url": "count.json", "key": ["k"], "live": { "every": 5 } } },
        "scene": { "kind": "repeat", "from": "count", "template": { "kind": "shape", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=d.v", "h": 10 }, "fill": "$accent" } }
    });
    let mut e = engine(doc);
    assert!(matches!(&e.requests()[..], [Request::Source { name, .. }] if name == "count"), "the first fetch");
    e.provide("count", br#"[{"k": "a", "v": 10}]"#).unwrap();
    assert!(e.requests().is_empty());
    let f = e.frame(0.0);
    assert_eq!(f.wake_at, Some(5.0), "the next refresh");
    e.frame(4.0);
    assert!(e.requests().is_empty(), "not yet");
    let f = e.frame(5.0);
    assert!(matches!(&e.requests()[..], [Request::Source { name, .. }] if name == "count"), "refresh due");
    assert_eq!(f.wake_at, Some(10.0));
    e.provide("count", br#"[{"k": "a", "v": 80}]"#).unwrap();
    assert!(e.frame(5.1).animating, "the new value animates in");
    let settled = e.frame(7.0);
    assert!(!settled.animating);
}

#[test]
fn a_live_snapshot_from_the_bundle_is_refreshed_when_the_chart_opens() {
    // A published bundle ships the rows it fetched when it was built: the reader should get
    // today's at once, then every `every` s. Rows the host fetched itself wait the full interval.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 60 },
        "data": { "count": { "url": "https://api.example/count", "key": ["k"], "live": { "every": 3600 } } },
        "scene": { "kind": "repeat", "from": "count", "template": { "kind": "shape", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=d.v", "h": 10 }, "fill": "$accent" } }
    });
    let mut e = engine(doc);
    e.provide_snapshot("count", br#"[{"k": "a", "v": 10}]"#).unwrap();
    assert!(e.requests().is_empty(), "nothing missing");
    let f = e.frame(0.0);
    assert!(matches!(&e.requests()[..], [Request::Source { name, .. }] if name == "count"), "refreshed at once");
    assert_eq!(f.wake_at, Some(3600.0), "then hourly");
    e.provide("count", br#"[{"k": "a", "v": 12}]"#).unwrap();
    assert!(e.frame(0.1).animating, "today's rows move in");
}

#[test]
fn reduced_motion_crossfades_without_travel() {
    let mut e = engine(story(serde_json::json!(["steps"])));
    e.set_reduced_motion(true);
    let (a, b, plan) = e.plan_states(0, 1);
    assert!(plan.duration() <= 0.25 + 1e-9, "short: {}", plan.duration());
    let mid = e.plan_at(&plan, 0.5);
    // Every shape at the midpoint has the start or the end geometry: nothing in between.
    fn shapes(n: &datars_scene::Node, out: &mut Vec<String>) {
        if let datars_scene::NodeKind::Shape { geom, .. } = &n.kind {
            out.push(format!("{geom:?}"));
        }
        for c in n.children() {
            shapes(c, out);
        }
    }
    let (mut ends, mut now) = (Vec::new(), Vec::new());
    shapes(&a.root, &mut ends);
    shapes(&b.root, &mut ends);
    shapes(&mid.root, &mut now);
    assert!(!now.is_empty());
    for g in &now {
        assert!(ends.contains(g), "{g} is an in-between shape");
    }
    // And without the setting the same transition does travel.
    e.set_reduced_motion(false);
    let (_, _, plan) = e.plan_states(0, 1);
    let mut moving = Vec::new();
    shapes(&e.plan_at(&plan, 0.5).root, &mut moving);
    assert!(moving.iter().any(|g| !ends.contains(g)));
}
