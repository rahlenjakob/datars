//! The edit loop: a new version of a document morphs in from what is on screen.

use datars_engine::Engine;

fn doc(values: [f64; 3], title: &str) -> datars_ir::Doc {
    let j = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 100 },
        "data": { "t": { "values": { "k": ["a", "b", "c"], "v": values }, "key": ["k"] } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "text", "key": "title", "text": title, "at": [0, 12] },
            { "kind": "repeat", "from": "t", "template": { "kind": "shape", "geom": { "type": "rect", "x": "=d.v", "y": 20, "w": 10, "h": 10 }, "fill": "$accent" } }
        ] },
        "program": { "states": [{ "name": "one" }, { "name": "two" }] }
    });
    datars_ir::Doc::from_json(&j.to_string()).unwrap()
}

#[test]
fn a_new_version_morphs_in_and_keeps_the_state() {
    let mut e = Engine::new();
    e.load(doc([10.0, 20.0, 30.0], "Before"));
    e.resize(400.0, 120.0, 2.0);
    e.goto(1);
    e.frame(0.0);
    e.frame(10.0);
    let before = e.frame(10.0).scene;
    e.reload(doc([110.0, 20.0, 30.0], "After"));
    assert_eq!(e.state(), "two", "still in the state of the same name");
    // The morph starts at its first frame.
    assert_eq!(e.frame(10.1).scene, before);
    let mid = e.frame(10.3);
    assert!(mid.animating, "the change animates");
    assert_ne!(mid.scene, before);
    let end = e.frame(100.0);
    assert!(!end.animating);
    assert!(end.scene.snapshot().contains("\"After\""), "{}", end.scene.snapshot());
    assert_eq!((end.scene.width, end.scene.height), (400.0, 120.0), "the host's viewport stays");
}
