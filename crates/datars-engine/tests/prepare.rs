//! Transitions prepared in idle time (`Engine::prepare`): the next and previous steps are resolved
//! and planned while the reader reads, and a step then plays exactly what it would have anyway.

use datars_engine::Engine;

fn doc() -> datars_ir::Doc {
    let j = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 120 },
        "signals": { "k": { "type": "num", "default": 1 } },
        "data": { "t": { "values": { "c": ["a", "b", "c", "d"], "v": [3, 1, 4, 2] }, "key": ["c"] } },
        "scene": { "kind": "repeat", "from": "t", "template": { "kind": "shape",
            "geom": { "type": "rect", "x": "=index * 60", "y": 10, "w": 40, "h": "=d.v * k * 10" }, "fill": "$accent" } },
        "program": { "states": [{ "name": "one", "set": { "k": 1 } }, { "name": "two", "set": { "k": 2 } }, { "name": "three", "set": { "k": 0.5 } }] }
    });
    datars_ir::Doc::from_json(&j.to_string()).unwrap()
}

/// Settle on the first state, maybe prepare, step on, and film the transition.
fn film(prepare: bool) -> (Vec<u64>, usize) {
    let mut e = Engine::new();
    e.load(doc());
    e.frame(0.0);
    e.frame(10.0);
    let mut work = 0;
    if prepare {
        while e.prepare() {
            work += 1;
            assert!(work < 10, "prepare finishes");
        }
    }
    e.set_clock(20.0);
    e.event("next");
    let hashes = (0..12).map(|k| e.frame(20.0 + k as f64 * 0.1).hash()).collect();
    (hashes, work)
}

#[test]
fn a_prepared_step_plays_exactly_the_same() {
    let (cold, _) = film(false);
    let (warm, work) = film(true);
    assert_eq!(work, 1, "on the first state there is only a next step to prepare");
    assert_eq!(cold, warm, "frame for frame");
}

#[test]
fn nothing_is_prepared_mid_transition_or_for_a_changed_scene() {
    let mut e = Engine::new();
    e.load(doc());
    e.frame(0.0);
    e.set_clock(1.0);
    e.event("next");
    e.frame(1.0);
    assert!(!e.prepare(), "not while moving");
    e.frame(10.0);
    assert!(e.prepare(), "settled on the middle state: next…");
    assert!(e.prepare(), "…and previous");
    assert!(!e.prepare(), "then nothing");
    // A signal changes the scene on screen: the plans are stale, and preparing starts over.
    e.set_signal_json("k", &serde_json::json!(3));
    e.frame(11.0);
    e.frame(20.0);
    assert!(e.prepare());
}
