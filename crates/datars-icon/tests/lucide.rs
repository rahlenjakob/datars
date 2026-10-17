//! The default set as vendored: every icon reads, in the node's ink only, with nothing skipped.

use datars_icon::{parse_svg, Set};
use datars_scene::IconInk;

fn lucide() -> Set {
    Set::from_json(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icons/lucide/icons.json")).unwrap()).unwrap()
}

#[test]
fn every_lucide_icon_reads_in_the_nodes_ink() {
    let set = lucide();
    assert_eq!(set.licence, "ISC");
    assert!(set.len() > 1500, "{} icons", set.len());
    for name in set.names() {
        let icon = parse_svg(&set.svg(name).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(icon.notes.is_empty(), "{name}: {:?}", icon.notes);
        assert!(icon.slots.is_empty(), "{name} has colours: {:?}", icon.slots);
        for l in &icon.layers {
            assert!(l.fill.is_none_or(|f| f == IconInk::Current) && l.stroke.as_ref().is_none_or(|s| s.ink == IconInk::Current), "{name}");
        }
        let back = datars_icon::Icon::from_json(&icon.to_json()).unwrap();
        assert_eq!(back.layers, icon.layers, "{name} round-trips");
    }
}

#[test]
fn icons_are_found_by_what_they_show() {
    let set = lucide();
    assert_eq!(set.search("heart").first().copied(), Some("heart"));
    assert!(set.search("person").contains(&"user"), "{:?}", &set.search("person")[..5.min(set.search("person").len())]);
}
