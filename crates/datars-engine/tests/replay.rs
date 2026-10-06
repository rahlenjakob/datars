//! Sessions replay bit-exactly: record a session with data, frames, pointer drags, program events,
//! a mode switch and a resize; serialize it; replay it on a fresh engine; every frame's display
//! list hash matches, mid-transition frames included.

use datars_engine::{session::Session, Engine, Pointer};
use datars_theme::Mode;

const DOC: &str = r#"{
  "datars": 1, "size": { "width": 200, "height": 100 },
  "data": { "t": { "url": "t.json", "key": ["k"] } },
  "signals": { "w": { "type": "num", "default": 20 } },
  "scene": { "kind": "group", "key": "area", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 200] } },
    "on": { "brush": { "brush": "sel" } },
    "children": [
      { "kind": "shape", "key": "bg", "geom": { "type": "rect", "x": 0, "y": 0, "w": 200, "h": 100 }, "fill": "$surface" },
      { "kind": "repeat", "from": "t", "template": { "kind": "shape",
        "geom": { "type": "rect", "x": "=scale.x(d.v)", "y": 10, "w": "=w", "h": 30 },
        "fill": "=!sel.active || (d.v >= sel.lo && d.v <= sel.hi) ? '$accent' : '$muted'" } }
    ] },
  "program": { "states": [ { "name": "a", "set": { "w": 20 } }, { "name": "b", "set": { "w": 60 } } ] }
}"#;

#[test]
fn a_recorded_session_replays_bit_exactly() {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(DOC).unwrap());
    e.start_recording();
    e.provide("t", br#"[{"k": "a", "v": 10}, {"k": "b", "v": 40}, {"k": "c", "v": 80}]"#).unwrap();
    let mut hashes = Vec::new();
    let mut t = 0.0;
    let mut tick = |e: &mut Engine, n: usize, hashes: &mut Vec<u64>| {
        for _ in 0..n {
            t += 1.0 / 30.0;
            hashes.push(e.frame(t).hash());
        }
    };
    tick(&mut e, 3, &mut hashes);
    e.pointer(Pointer::Down { x: 60.0, y: 50.0 });
    tick(&mut e, 2, &mut hashes);
    e.pointer(Pointer::Move { x: 120.0, y: 50.0 });
    tick(&mut e, 4, &mut hashes);
    e.pointer(Pointer::Move { x: 180.0, y: 50.0 });
    e.pointer(Pointer::Up { x: 180.0, y: 50.0 });
    tick(&mut e, 10, &mut hashes);
    e.event("next");
    tick(&mut e, 12, &mut hashes); // mid-transition frames
    e.set_mode(Mode::Dark);
    e.resize(300.0, 120.0, 2.0);
    tick(&mut e, 30, &mut hashes);
    let session = e.take_recording().expect("recording");
    let json = session.to_json();
    let back = Session::from_json(&json).unwrap();
    assert_eq!(back, session, "the session serializes losslessly");

    let mut replayed = Vec::new();
    Engine::replay(&back, |_, f| {
        if let Some(f) = f {
            replayed.push(f.hash());
        }
    })
    .unwrap();
    assert_eq!(replayed.len(), hashes.len());
    let first_diff = hashes.iter().zip(&replayed).position(|(a, b)| a != b);
    assert_eq!(first_diff, None, "frames differ from frame {first_diff:?}");
    // And the session wasn't trivial: the brush and the event changed what was drawn.
    let distinct: std::collections::BTreeSet<_> = hashes.iter().collect();
    assert!(distinct.len() > 10, "{} distinct frames", distinct.len());
}
