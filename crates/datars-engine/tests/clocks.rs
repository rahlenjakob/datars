//! Clock signals: values that run with time while a settled scene reads them (a turning globe),
//! pausing for transitions, off screen and for reduced motion; renders and goldens see the default.

use datars_engine::Engine;
use datars_scene::{Geom, NodeKind};

fn doc() -> serde_json::Value {
    serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
        "signals": { "spin": { "type": "num", "default": 0, "clock": 10 }, "mode": { "type": "str", "default": "turn" } },
        "program": { "states": [ { "name": "turn", "set": { "mode": "turn" } }, { "name": "still", "set": { "mode": "still" } } ] },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "shape", "key": "dot", "geom": { "type": "rect", "x": "=mode == \"turn\" ? spin : 50", "y": 10, "w": 4, "h": 4 } } ] } })
}

fn engine() -> Engine {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc().to_string()).unwrap());
    e
}

fn dot_x(scene: &datars_scene::Scene) -> f64 {
    let mut x = None;
    scene.root.walk(&Default::default(), &mut |_, n| {
        if let NodeKind::Shape { geom: Geom::Rect { x: rx, .. }, .. } = &n.kind {
            x = Some(*rx);
        }
    });
    x.expect("the dot")
}

#[test]
fn a_clock_runs_while_the_scene_reads_it() {
    let mut e = engine();
    assert_eq!(dot_x(&e.frame(0.0).scene), 0.0);
    let f = e.frame(0.05);
    assert!(f.animating, "a scene that reads a clock keeps frames coming");
    assert!((dot_x(&f.scene) - 0.5).abs() < 1e-9);
    // Steps are capped: a host that slept for a minute moves the clock by 0.1 s, not 60.
    assert!((dot_x(&e.frame(60.0).scene) - 1.5).abs() < 1e-9);
    // Paused off screen (the host stops playback): the value holds.
    e.set_playing(false);
    let held = dot_x(&e.frame(60.05).scene);
    assert!((held - 1.5).abs() < 1e-9, "{held}");
}

#[test]
fn a_state_that_doesnt_read_the_clock_sleeps() {
    let mut e = engine();
    e.frame(0.0);
    e.goto(1);
    let settled = (1..40).map(|i| e.frame(i as f64 * 0.1)).last().unwrap();
    assert_eq!(dot_x(&settled.scene), 50.0);
    assert!(!settled.animating, "nothing reads the clock here: no frame loop");
}

#[test]
fn renders_and_goldens_see_the_default() {
    let mut e = engine();
    let baked = e.bake();
    assert_eq!(dot_x(&baked[0].1), 0.0);
}

/// A card that dodges the data keeps the place it chose when the state settled while only a clock
/// moves the data under it — it doesn't hop from corner to corner as a globe turns.
#[test]
fn a_card_holds_its_place_while_a_clock_moves_the_data() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 200 },
        "signals": { "t": { "type": "num", "default": 0, "clock": 100 } },
        "scene": { "kind": "group", "key": "root", "children": [
            // A block that sweeps from the left edge to the right.
            { "kind": "shape", "key": "mark", "geom": { "type": "rect", "x": "=t % 400", "y": 0, "w": 160, "h": 200 }, "semantics": { "role": "datum", "label": "m" } },
            { "kind": "group", "key": "card", "layout": { "type": "stack", "padding": 10, "align": "start" }, "dodge": ["top-left", "top-right"], "children": [
                { "kind": "group", "key": "box", "size": { "w": 100, "h": 40 }, "children": [
                    { "kind": "text", "key": "t", "text": "Note", "at": [0, 0], "style": { "baseline": "top" } } ] } ] } ] } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let card_x = |s: &datars_scene::Scene| {
        let mut x = None;
        s.root.walk(&Default::default(), &mut |_, n| {
            if n.key.to_string().contains("box") {
                x = Some(n.common.transform.0[4]);
            }
        });
        x.unwrap()
    };
    // At rest the block is on the left, so the card goes right.
    let first = card_x(&e.frame(0.0).scene);
    assert!(first > 200.0, "{first}");
    // Run the clock until the block covers the right: the card stays put.
    let mut t = 0.0;
    for _ in 0..40 {
        t += 0.1;
        let x = card_x(&e.frame(t).scene);
        assert_eq!(x, first, "at t = {t}");
    }
}
