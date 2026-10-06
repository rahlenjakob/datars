use datars_scene::*;

fn sample() -> Scene {
    let bars = ["S", "SD", "M"]
        .iter()
        .enumerate()
        .map(|(i, k)| {
            Node::shape(Key::one(*k), Geom::rect(20.0 + i as f64 * 60.0, 100.0 - i as f64 * 20.0, 40.0, 100.0 + i as f64 * 20.0))
                .fill(Paint::Solid(datars_theme::Ink::palette("categorical", i as u32)))
                .semantics(Semantics::new(Role::Datum, format!("{k}: {}", 30 - i * 5)))
        })
        .collect();
    Scene::new(240.0, 220.0, Node::group(Key::name("plot"), bars))
}

#[test]
fn json_round_trip_and_stable_hash() {
    let s = sample();
    let j = s.to_json();
    let back = Scene::from_json(&j).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.hash(), s.hash());
    let mut t = s.clone();
    if let NodeKind::Group { children } = &mut t.root.kind {
        children[0].common.opacity = 0.5;
    }
    assert_ne!(t.hash(), s.hash());
}

#[test]
fn snapshot_reads_like_the_docs() {
    let snap = sample().snapshot();
    assert!(snap.contains("group (\"plot\",)"), "{snap}");
    assert!(snap.contains("(\"SD\",) rect x=80.00 y=80.00 w=40.00 h=120.00 fill=$categorical[1] role=datum \"SD: 25\""), "{snap}");
}

#[test]
fn find_by_key_path() {
    let s = sample();
    let p = KeyPath(vec![Key::name("plot"), Key::one("M")]);
    assert!(matches!(s.find(&p).unwrap().kind, NodeKind::Shape { .. }));
}
