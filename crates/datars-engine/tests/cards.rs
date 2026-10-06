//! Cards' engine parts: a backdrop sized to its content, placement that dodges data, the band a
//! crowded card takes (the chart makes room), and the story's narration as signals.

use datars_engine::Engine;
use datars_math::Rect;
use datars_scene::{Node, NodeKind};

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e
}

/// Canvas-px bounds of the first node at a key-path suffix.
fn bounds_of(scene: &datars_scene::Scene, suffix: &str) -> Rect {
    fn go(n: &Node, path: String, xf: datars_math::Affine, suffix: &str) -> Option<Rect> {
        let here = format!("{path}/{}", n.key);
        let xf = xf.mul(n.common.transform);
        if here.ends_with(suffix) {
            let local = datars_engine::node_bounds(&Node { common: Default::default(), ..n.clone() });
            let pts = [(local.x, local.y), (local.x1(), local.y1())].map(|(x, y)| xf.apply(datars_math::Vec2::new(x, y)));
            return Some(Rect::new(pts[0].x, pts[0].y, pts[1].x - pts[0].x, pts[1].y - pts[0].y));
        }
        let kids: &[Node] = match &n.kind {
            NodeKind::Group { children } => children,
            _ => &[],
        };
        kids.iter().find_map(|c| go(c, here.clone(), xf, suffix))
    }
    go(&scene.root, String::new(), datars_math::Affine::IDENTITY, suffix).unwrap_or_else(|| panic!("no {suffix}\n{}", scene.snapshot()))
}

fn card(dodge: &[&str]) -> serde_json::Value {
    serde_json::json!({ "kind": "group", "key": "card", "layout": { "type": "stack", "padding": 10, "align": "start" }, "dodge": dodge, "children": [
        { "kind": "group", "key": "box", "size": { "w": 120, "h": "auto" }, "layout": { "type": "rows", "padding": 8 },
          "backdrop": { "fill": "$surface", "stroke": { "paint": "$rule", "width": 1 }, "radius": 4, "padding": 8, "fit": "width" },
          "children": [{ "kind": "text", "key": "t", "text": "=narration.text", "at": [0, 0], "size": { "h": "auto" }, "style": { "baseline": "top", "max_width": 104 } }] }
    ] })
}

fn doc(marks: Vec<serde_json::Value>, dodge: &[&str]) -> serde_json::Value {
    let mut kids = marks;
    kids.push(card(dodge));
    serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 300 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "stack" }, "children": kids },
        "program": { "states": [
            { "name": "one", "narration": { "text": "A short note." } },
            { "name": "two", "narration": { "text": "" } }
        ] }
    })
}

fn bar(key: &str, x: f64, y: f64, w: f64, h: f64) -> serde_json::Value {
    serde_json::json!({ "kind": "shape", "key": key, "geom": { "type": "rect", "x": x, "y": y, "w": w, "h": h }, "fill": "$accent", "semantics": { "role": "datum", "label": key } })
}

#[test]
fn a_card_dodges_the_data_and_its_backdrop_hugs_the_text() {
    // Tall bars on the right: the card's first choice (top-right) is taken, top-left is free.
    let e = &mut engine(doc(vec![bar("a", 250.0, 20.0, 60.0, 260.0), bar("b", 320.0, 40.0, 60.0, 240.0)], &["top-right", "top-left"]));
    let s = e.scene();
    let back = bounds_of(&s, "/(\"box\",)/(\"backdrop\",)");
    assert!((back.x - 10.0).abs() < 1.0 && (back.y - 10.0).abs() < 1.0, "top-left, inset by the padding: {back:?}");
    assert!((back.w - 120.0).abs() < 1.5, "fit width: the box's width: {back:?}");
    assert!(back.h > 20.0 && back.h < 60.0, "hugs one or two lines of text plus padding: {back:?}");
    // Narration signals drive the text; an empty narration leaves no backdrop.
    assert!(s.snapshot().contains("A short note."));
    e.goto(1);
    let s = e.scene_for_state(1);
    assert!(!s.snapshot().contains("backdrop"), "{}", s.snapshot());
}

/// A chart that is one line (a series, as std/line draws it) through `pts`.
fn line_chart(pts: &[(f64, f64)]) -> serde_json::Value {
    let d = pts.iter().enumerate().map(|(i, (x, y))| format!("{} {x} {y}", if i == 0 { "M" } else { "L" })).collect::<Vec<_>>().join(" ");
    serde_json::json!({ "kind": "group", "key": "chart", "children": [
        { "kind": "shape", "key": "line", "geom": { "type": "path", "d": d }, "stroke": { "paint": "$accent", "width": 2 }, "semantics": { "role": "series", "label": "a line" } }
    ] })
}

#[test]
fn a_card_keeps_off_lines_too() {
    // A line along the top (through top-left and top-right) and down the right side: the first
    // free corner is bottom-left.
    let e = &mut engine(doc(vec![line_chart(&[(0.0, 30.0), (400.0, 30.0), (360.0, 300.0)])], &["top-right", "top-left", "bottom-right", "bottom-left"]));
    let back = bounds_of(&e.scene(), "/(\"backdrop\",)");
    assert!(back.x < 20.0 && back.y1() > 280.0, "bottom-left: {back:?}");
    // Lines everywhere a card could go (a zigzag across the whole box): it takes a band instead.
    // (Down to the chart's box's bottom, so it rises with the box when the card takes a band.)
    let d = (0..=20).map(|i| format!("{} {} {}", if i == 0 { "M" } else { "L" }, i * 20, if i % 2 == 0 { "0" } else { "${box.h}" })).collect::<Vec<_>>().join(" ");
    let zigzag = serde_json::json!({ "kind": "group", "key": "chart", "children": [
        { "kind": "shape", "key": "line", "geom": { "type": "path", "d": format!("=`{d}`") }, "stroke": { "paint": "$accent", "width": 2 }, "semantics": { "role": "series", "label": "a line" } }
    ] });
    let e = &mut engine(doc(vec![zigzag], &["top-right", "top-left", "bottom-right", "bottom-left"]));
    let s = e.scene();
    let back = bounds_of(&s, "/(\"backdrop\",)");
    let chart = bounds_of(&s, "/(\"chart\",)/(\"line\",)");
    assert!(back.y >= chart.y1() - 1.5, "under the chart: {back:?} vs {chart:?}");
}

#[test]
fn a_card_that_would_hide_a_slice_of_a_small_chart_takes_a_band() {
    // A small mark under every anchor: little of the card is covered (under 4 %), but a good part
    // of what there is to read would be hidden — the card goes below instead.
    // (In the chart's box, so they move up with it when it makes room.)
    let (near, far_x, far_y) = (serde_json::json!(20), serde_json::json!("=box.w - 40"), serde_json::json!("=box.h - 38"));
    let spots = [(near.clone(), near.clone()), (far_x.clone(), near.clone()), (near.clone(), far_y.clone()), (far_x, far_y)];
    let marks = spots.iter().enumerate().map(|(i, (x, y))| {
        serde_json::json!({ "kind": "shape", "key": format!("m{i}"), "geom": { "type": "rect", "x": x, "y": y, "w": 14, "h": 14 }, "fill": "$accent", "semantics": { "role": "datum", "label": "m" } })
    }).collect::<Vec<_>>();
    let chart = serde_json::json!({ "kind": "group", "key": "chart", "children": marks });
    let e = &mut engine(doc(vec![chart], &["top-right", "top-left", "bottom-right", "bottom-left"]));
    let s = e.scene();
    let back = bounds_of(&s, "/(\"backdrop\",)");
    assert!(back.y > 250.0, "banded at the bottom: {back:?}\n{}", s.snapshot());
    let m = bounds_of(&s, "/(\"chart\",)/(\"m2\",)");
    assert!(m.y1() <= back.y + 0.5, "the marks above it: {m:?} vs {back:?}");
}

#[test]
fn a_crowded_card_takes_a_band_and_the_chart_makes_room() {
    // One bar fills the whole box: nowhere to overlay, so the card goes below and the chart
    // (a group sized by the layout) gets what's left.
    let chart = serde_json::json!({ "kind": "group", "key": "chart", "children": [
        { "kind": "shape", "key": "fill", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent", "semantics": { "role": "datum", "label": "all" } }
    ] });
    let e = &mut engine(doc(vec![chart], &["top-right", "top-left", "bottom-right", "bottom-left"]));
    let s = e.scene();
    let fill = bounds_of(&s, "/(\"chart\",)/(\"fill\",)");
    let back = bounds_of(&s, "/(\"backdrop\",)");
    assert!(fill.h < 300.0 - 20.0, "the chart shrank: {fill:?}");
    assert!(back.y >= fill.y1() - 0.5, "the card sits below the chart, not on it: {back:?} vs {fill:?}");
    assert!(back.y1() <= 300.5, "and inside the canvas: {back:?}");
}

#[test]
fn a_single_anchor_is_a_fixed_place_and_may_be_an_expression() {
    // One anchor (here an expression: the state picks it) over a chart that fills the box: the
    // card stays where it's put, and the chart keeps the whole box.
    let chart = serde_json::json!({ "kind": "group", "key": "chart", "children": [
        { "kind": "shape", "key": "fill", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent", "semantics": { "role": "datum", "label": "all" } }
    ] });
    let e = &mut engine(doc(vec![chart], &["=narration.text == '' ? 'top-left' : 'bottom-right'"]));
    let s = e.scene();
    let fill = bounds_of(&s, "/(\"chart\",)/(\"fill\",)");
    let back = bounds_of(&s, "/(\"backdrop\",)");
    assert!((fill.h - 300.0).abs() < 0.01, "no band: {fill:?}");
    assert!((back.x1() - 390.0).abs() < 1.0 && (back.y1() - 290.0).abs() < 1.0, "bottom-right, over the chart: {back:?}");
}

#[test]
fn a_crowded_card_in_a_chart_area_takes_its_band_inside_that_area() {
    // A title, a chart area (the chart and its card) and a source line in rows: the card's band
    // comes out of the chart area; the title and the source line stay where they are, in order.
    let area = serde_json::json!({ "kind": "group", "key": "stage", "layout": { "type": "stack" }, "children": [
        { "kind": "group", "key": "chart", "children": [
            { "kind": "shape", "key": "fill", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent", "semantics": { "role": "datum", "label": "all" } }
        ] },
        card(&["bottom", "top-left"]),
    ] });
    let text = |key: &str| serde_json::json!({ "kind": "text", "key": key, "text": key, "at": [0, 0], "size": { "h": "auto" }, "style": { "size": 12, "baseline": "top" } });
    let e = &mut engine(serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 300 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows", "gap": 8, "padding": 10 }, "children": [text("title"), area, text("source")] },
        "program": { "states": [{ "name": "one", "narration": { "text": "A short note." } }] }
    }));
    let s = e.scene();
    let title = bounds_of(&s, "/(\"title\",)");
    let fill = bounds_of(&s, "/(\"chart\",)/(\"fill\",)");
    let back = bounds_of(&s, "/(\"box\",)/(\"backdrop\",)");
    let source = bounds_of(&s, "/(\"source\",)");
    assert!(fill.y > title.y1() && fill.y < title.y1() + 20.0, "the chart area follows the title: {fill:?} after {title:?}");
    assert!(back.y >= fill.y1() - 0.5, "the card is under the chart: {back:?} vs {fill:?}");
    assert!(source.y >= back.y1() - 0.5 && source.y1() <= 300.5, "the source line stays last: {source:?} after {back:?}");
}

// ---- std/card ------------------------------------------------------------------------------------

/// A `std/card` with `params` over a stage — a full-bleed region (land on a map covers everything)
/// or nothing — in two states: one narrated with a title (the `corner` signal top-left), one
/// without (bottom-right).
fn std_card(land: bool, params: serde_json::Value) -> Engine {
    let region = serde_json::json!({ "kind": "shape", "key": "land", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent", "semantics": { "role": "region", "label": "land" } });
    let card = serde_json::json!({ "kind": "use", "recipe": "@datars/std/card", "key": "caption", "params": params });
    let kids = if land { vec![region, card] } else { vec![card] };
    engine(serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 300 },
        "signals": { "corner": { "type": "str", "default": "top-left" } },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "stack" }, "children": kids },
        "program": { "states": [
            { "name": "titled", "set": { "corner": "top-left" }, "narration": { "title": "Kicker", "text": "A short note." } },
            { "name": "plain", "set": { "corner": "bottom-right" }, "narration": { "text": "A short note." } }
        ] }
    }))
}

#[test]
fn a_std_card_that_doesnt_dodge_stays_at_its_anchor_over_a_map() {
    let e = &mut std_card(true, serde_json::json!({ "text": "=narration.text", "at": "bottom-left", "width": 200, "dodge": false }));
    let s = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let land = bounds_of(&s, "/(\"land\",)");
    let back = bounds_of(&s, "/(\"backdrop\",)");
    assert!((land.h - 300.0).abs() < 0.01, "the map keeps its whole box (no band): {land:?}");
    assert!((back.x - 12.0).abs() < 1.0 && (back.y1() - 288.0).abs() < 1.0, "bottom-left, inset by the margin: {back:?}");
    // The same card dodging finds no free spot over land: it takes a band and the map shrinks.
    let e = &mut std_card(true, serde_json::json!({ "text": "=narration.text", "at": "bottom-left", "width": 200 }));
    assert!(bounds_of(&e.scene(), "/(\"land\",)").h < 280.0);
}

#[test]
fn an_empty_kicker_takes_no_room_in_a_std_card() {
    // Where the text sits in its card, and the card's height: with a kicker that's empty in this
    // state, exactly as without one.
    let inside = |s: &datars_scene::Scene| {
        let (text, back) = (bounds_of(s, "/(\"text\",)"), bounds_of(s, "/(\"backdrop\",)"));
        (text.y - back.y, back.h)
    };
    let e = &mut std_card(false, serde_json::json!({ "kicker": "=narration.title", "text": "=narration.text", "width": 200 }));
    let (titled, plain) = (e.scene_for_state(0), e.scene_for_state(1));
    let bare = std_card(false, serde_json::json!({ "text": "=narration.text", "width": 200 })).scene_for_state(1);
    let (a, b) = (inside(&plain), inside(&bare));
    assert!((a.0 - b.0).abs() < 0.5 && (a.1 - b.1).abs() < 0.5, "no line, no gap: {a:?} vs {b:?}\n{}", plain.snapshot());
    assert!(inside(&titled).0 > a.0 + 10.0, "a kicker that has text pushes the text down");
}

#[test]
fn a_std_card_moves_with_its_anchor_expression() {
    for dodge in [false, true] {
        let e = &mut std_card(false, serde_json::json!({ "text": "=narration.text", "at": "=corner", "width": 200, "dodge": dodge }));
        let (tl, br) = (bounds_of(&e.scene_for_state(0), "/(\"backdrop\",)"), bounds_of(&e.scene_for_state(1), "/(\"backdrop\",)"));
        assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
        assert!((tl.x - 12.0).abs() < 1.0 && (tl.y - 12.0).abs() < 1.0, "dodge {dodge}: top-left first: {tl:?}");
        assert!((br.x1() - 388.0).abs() < 1.0 && (br.y1() - 288.0).abs() < 1.0, "dodge {dodge}: then bottom-right: {br:?}");
        // The same text: the same card, travelling between the corners (not a fade).
        let (_, _, plan) = e.plan_states(0, 1);
        let mid = bounds_of(&e.plan_at(&plan, 0.5), "/(\"backdrop\",)");
        assert!(mid.x > tl.x + 20.0 && mid.x < br.x - 20.0, "dodge {dodge}: halfway there: {mid:?}");
    }
}
