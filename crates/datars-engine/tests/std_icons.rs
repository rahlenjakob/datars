//! The standard library's icon recipes through the engine: pictograms (a unit per icon, keyed as
//! waffle's, instanced), legends and callouts with icons, and the gauge's named bands and needles.

use datars_engine::Engine;
use datars_scene::{Node, NodeKind, Proto};
use serde_json::{json, Value};

fn engine(size: (u32, u32), scene: Value) -> Engine {
    let doc = json!({ "datars": 1, "size": { "width": size.0, "height": size.1 },
        "data": { "trips": { "values": { "mode": ["Car", "Bus", "Bike"], "n": [12, 7, 3] }, "key": ["mode"] } },
        "scene": scene });
    let mut e = Engine::new();
    e.set_asset_fetch(Box::new(|_, url| datars_icon::set::bundled_svg(url).map(String::into_bytes)));
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn use_(recipe: &str, params: Value) -> Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params })
}

fn walk<'a>(n: &'a Node, out: &mut Vec<&'a Node>) {
    out.push(n);
    n.children().iter().for_each(|c| walk(c, out));
}

fn nodes(e: &mut Engine) -> Vec<Node> {
    let s = e.scene();
    let mut v = Vec::new();
    walk(&s.root, &mut v);
    v.into_iter().cloned().collect()
}

#[test]
fn a_pictogram_is_an_icon_per_unit_in_a_row_per_category() {
    let mut e = engine((600, 300), use_("pictogram", json!({ "data": "trips", "value": "n", "category": "mode", "icons": { "Car": "car", "Bike": "bike" }, "icon": "user" })));
    let all = nodes(&mut e);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let sets: Vec<(String, usize)> = all.iter().filter_map(|n| match &n.kind {
        NodeKind::Instances(i) => match &i.proto {
            Proto::Icon { icon } => Some((icon.name.to_string(), i.len())),
            _ => None,
        },
        _ => None,
    }).collect();
    // One instances node per icon: the default for Bus, the map's for Car and Bike.
    assert_eq!(sets, vec![("lucide:user".to_string(), 7), ("lucide:bike".to_string(), 3), ("lucide:car".to_string(), 12)]);
    // Units are keyed (category, unit), as waffle's: a bar keyed by its category splits into them.
    let NodeKind::Instances(car) = &all.iter().find(|n| matches!(&n.kind, NodeKind::Instances(i) if i.len() == 12)).unwrap().kind else { panic!() };
    assert_eq!(car.keys[0].to_string(), "(\"Car\", Unit(0))");
    // Each category is read once, with its value.
    let said: Vec<String> = e.semantics().into_iter().map(|(_, l, _)| l).collect();
    assert!(said.contains(&"Car: 12".to_string()), "{said:?}");
}

#[test]
fn per_rounds_to_whole_icons_and_says_what_one_is() {
    let mut e = engine((600, 300), use_("pictogram", json!({ "data": "trips", "value": "n", "category": "mode", "per": 4 })));
    let all = nodes(&mut e);
    let n: usize = all.iter().filter_map(|n| if let NodeKind::Instances(i) = &n.kind { Some(i.len()) } else { None }).sum();
    assert_eq!(n, 3 + 2 + 1, "12/4, 7/4 and 3/4 rounded");
    assert!(all.iter().any(|n| matches!(&n.kind, NodeKind::Text(t) if t.text == "One icon: 4")));
}

#[test]
fn a_grid_pictogram_has_a_legend_of_its_icons() {
    let mut e = engine((400, 400), use_("pictogram", json!({ "data": "trips", "value": "n", "category": "mode", "layout": "grid", "columns": 5, "rows": 5, "icon": "user" })));
    let all = nodes(&mut e);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let legend_icons = all.iter().filter(|n| n.key.to_string().contains("icon") && matches!(n.kind, NodeKind::Group { .. })).count();
    assert!(legend_icons >= 3, "an icon per legend entry: {legend_icons}");
}

#[test]
fn a_gauge_names_its_bands_along_the_arc_and_says_the_band() {
    let mut e = engine((480, 320), use_("gauge", json!({ "value": 38, "bands": [25, 45, 55, 75, 100], "names": ["Panic", "Fear", "Neutral", "Greed", "Euphoria"], "arc": 180, "label": "Mood" })));
    let all = nodes(&mut e);
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let curved: Vec<&str> = all.iter().filter_map(|n| match &n.kind {
        NodeKind::Text(t) if t.along.as_ref().is_some_and(|a| !a.is_empty()) => Some(t.text.as_str()),
        _ => None,
    }).collect();
    assert_eq!(curved, vec!["Panic", "Fear", "Neutral", "Greed", "Euphoria"]);
    let said: Vec<String> = e.semantics().into_iter().map(|(_, l, _)| l).collect();
    assert!(said.iter().any(|l| l.contains("38") && l.contains("Fear")), "{said:?}");
    // Rotated instead: straight names, turned.
    let mut e = engine((480, 320), use_("gauge", json!({ "value": 38, "bands": [50, 100], "names": ["Low", "High"], "bandText": "rotated" })));
    let turned = nodes(&mut e).into_iter().filter(|n| matches!(&n.kind, NodeKind::Text(t) if t.along.is_none() && t.rotate != 0.0)).count();
    assert_eq!(turned, 2);
}

#[test]
fn a_band_too_narrow_for_its_name_leaves_it_out() {
    let mut e = engine((200, 140), use_("gauge", json!({ "value": 10, "bands": [2, 100], "names": ["A very long band name", "Rest"] })));
    let texts: Vec<String> = nodes(&mut e).into_iter().filter_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t.text.clone()) } else { None }).collect();
    assert!(!texts.iter().any(|t| t == "A very long band name"), "{texts:?}");
}

#[test]
fn needles_are_wedges_lines_or_arrows() {
    for (style, want) in [("wedge", "path"), ("line", "segment"), ("arrow", "segment")] {
        let mut e = engine((300, 200), use_("gauge", json!({ "value": 60, "bands": [50, 100], "needleStyle": style })));
        let hand = nodes(&mut e).into_iter().find(|n| n.key.to_string().contains("hand")).unwrap();
        assert_eq!(hand.kind_name(), want, "{style}");
        if style == "arrow" {
            assert!(matches!(&hand.kind, NodeKind::Shape { markers: Some(m), .. } if m.end.is_some()));
        }
    }
}

#[test]
fn a_callout_can_point_with_an_icon() {
    let mut e = engine((300, 200), use_("annotate", json!({ "x": 100, "y": 100, "text": "Opened in May", "icon": "map-pin" })));
    let all = nodes(&mut e);
    let pin = all.iter().find(|n| n.key.to_string().contains("icon")).expect("the icon");
    assert_eq!(pin.semantics.as_ref().map(|s| s.role), Some(datars_scene::Role::Decoration), "the note says what it means");
}
