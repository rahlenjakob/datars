//! A host's brand tokens at runtime. Colours late-bind, but sizes, radii, strokes and fonts are
//! read when a scene resolves: changing them re-resolves the current state and moves there.

use datars_engine::Engine;
use datars_scene::NodeKind;

fn title_size(scene: &datars_scene::Scene) -> f64 {
    let mut size = None;
    scene.root.walk(&Default::default(), &mut |_, n| {
        if let NodeKind::Text(t) = &n.kind {
            size = t.runs.first().map(|r| r.size);
        }
    });
    size.expect("a title")
}

#[test]
fn a_hosts_sizes_and_fonts_reach_the_scene() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 120 },
        "scene": { "kind": "text", "key": "title", "at": [10, 40], "text": "Revenue", "style": { "font": "font.title", "size": "$size.title" } } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert_eq!(title_size(&e.frame(0.0).scene), 17.0);
    e.set_host_tokens([("size.title".to_string(), serde_json::json!(26))].into());
    assert!(e.frame(0.1).animating, "the change plays as a transition");
    assert_eq!(title_size(&e.frame(10.0).scene), 26.0, "the brand's title size, once the change has played");
    // Colours alone don't re-resolve: the scene keeps its inks.
    e.set_host_tokens([("size.title".to_string(), serde_json::json!(26)), ("accent".to_string(), serde_json::json!("#b3261e"))].into());
    assert!(!e.frame(10.1).animating, "a colour change is late-bound, not a transition");
}
