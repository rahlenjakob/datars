//! Layout that holds up in a phone's narrow box, engine side: labels kept inside a map view (the
//! other side of their point, or none when the point is out of view), labels kept off each other
//! (`declutter`), sizes that are expressions over the parent box, columns that stack as rows when
//! narrow, tick counts that are expressions, a word's measured width, and rotated text measured
//! by the room it covers.

use datars_engine::Engine;
use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Node, NodeKind, Scene};

fn scene(doc: serde_json::Value) -> Scene {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    let s = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    s
}

/// Every shown text: what it says and where it's drawn (canvas px), through views and cameras.
fn texts(s: &Scene) -> Vec<(String, Rect)> {
    fn go(n: &Node, parent: Affine, out: &mut Vec<(String, Rect)>) {
        if !n.common.visible {
            return;
        }
        let xf = n.common.placed(parent);
        match &n.kind {
            NodeKind::Text(t) if !t.text.is_empty() => {
                // Screen-sized glyphs at the transformed origin, shifted by the offset.
                let o = xf.apply(t.origin) + t.offset;
                out.push((t.text.clone(), Rect::new(o.x + t.bounds.x, o.y + t.bounds.y, t.bounds.w, t.bounds.h)));
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
                children.iter().for_each(|c| go(c, inner, out));
            }
            _ => n.children().iter().for_each(|c| go(c, xf, out)),
        }
    }
    let mut out = Vec::new();
    go(&s.root, Affine::IDENTITY, &mut out);
    out
}

fn text<'a>(all: &'a [(String, Rect)], s: &str) -> Option<&'a Rect> {
    all.iter().find(|(t, _)| t == s).map(|(_, r)| r)
}

/// A map-like view (camera ×2 on (100, 100)) in a 300 × 200 box, with a dot and a label set right
/// of it (`offset`) at each point.
fn view_with_labels(points: &[(&str, f64, f64)], contain: bool, declutter: bool) -> serde_json::Value {
    let kids: Vec<serde_json::Value> = points
        .iter()
        .map(|(name, x, y)| serde_json::json!({ "kind": "text", "key": name, "text": name, "at": [x, y], "offset": [8, 0],
            "style": { "size": 12, "baseline": "middle", "contain": contain } }))
        .collect();
    serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 200 },
        "scene": { "kind": "view", "key": "map", "camera": { "x": 100, "y": 100, "zoom": 2 }, "children": [
            { "kind": "group", "key": "labels", "declutter": declutter, "children": kids }
        ] } })
}

#[test]
fn a_contained_label_in_a_map_takes_the_other_side_of_its_point_near_the_edge() {
    // (170, 100) through the camera: x = (170 − 100)·2 + 150 = 290, 10 px from the right edge — the
    // label right of the dot would run out of the view; (100, 100) is in the middle.
    let s = scene(view_with_labels(&[("Tullinge", 170.0, 100.0), ("Alby", 100.0, 100.0)], true, false));
    let all = texts(&s);
    let t = text(&all, "Tullinge").expect("shown");
    assert!(t.x1() <= 290.0 + 0.5 && t.x >= 0.0, "left of its dot, inside the view: {t:?}");
    assert!((t.x1() - (290.0 - 8.0)).abs() < 1.0, "as far from the dot as it was, on the other side: {t:?}");
    let a = text(&all, "Alby").unwrap();
    assert!((a.x - 158.0).abs() < 1.0, "a label with room stays right of its dot: {a:?}");
    // Not contained: it runs out of the view as before.
    let s = scene(view_with_labels(&[("Tullinge", 170.0, 100.0)], false, false));
    assert!(text(&texts(&s), "Tullinge").unwrap().x1() > 300.0);
}

#[test]
fn a_contained_label_whose_point_is_out_of_view_is_left_out() {
    // (22, 100) → x = −6: the point is past the left edge, its label would show cut — worse than none.
    let s = scene(view_with_labels(&[("Ipanema", 22.0, 100.0), ("Centro", 100.0, 100.0)], true, false));
    let all = texts(&s);
    assert!(text(&all, "Ipanema").is_none(), "{all:?}");
    assert!(text(&all, "Centro").is_some());
}

#[test]
fn decluttered_labels_take_the_other_side_of_their_point_or_are_left_out() {
    // Three names on nearly the same spot (1 content unit = 2 px apart), each right of its point.
    let pts = [("Vasa Museum", 100.0, 100.0), ("Nationalmuseum", 101.0, 100.5), ("Skansen", 100.5, 101.0)];
    let s = scene(view_with_labels(&pts, false, true));
    let all = texts(&s);
    let first = text(&all, "Vasa Museum").expect("the first keeps its place");
    assert!((first.x - 158.0).abs() < 1.0, "{first:?}");
    let second = text(&all, "Nationalmuseum").expect("the second fits left of its point");
    assert!(second.x1() <= 152.0 + 0.5, "{second:?}");
    assert!(first.intersect(second).is_none_or(|i| i.w * i.h < 1.0));
    assert!(text(&all, "Skansen").is_none(), "no room on either side: left out ({all:?})");
    // Without declutter they pile up.
    let all = texts(&scene(view_with_labels(&pts, false, false)));
    assert_eq!(all.len(), 3);
}

#[test]
fn sizes_may_be_expressions_over_the_parent_box() {
    let doc = |w: f64| serde_json::json!({ "datars": 1, "size": { "width": w, "height": 100 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "stack", "padding": 12, "align": "start" }, "children": [
            { "kind": "shape", "key": "card", "size": { "w": "=min(260, box.w - 24)", "h": 40 },
              "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent" }
        ] } });
    let width = |w: f64| match &scene(doc(w)).root.children()[0].kind {
        NodeKind::Shape { geom: datars_scene::Geom::Rect { w, .. }, .. } => *w,
        other => panic!("{other:?}"),
    };
    assert!((width(800.0) - 260.0).abs() < 1e-9, "as wide as asked on a desktop");
    assert!((width(250.0) - 226.0).abs() < 1e-9, "never wider than a phone's box leaves");
}

#[test]
fn columns_that_wrap_stack_as_rows_in_a_narrow_box() {
    let doc = |w: f64| serde_json::json!({ "datars": 1, "size": { "width": w, "height": 400 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "columns", "gap": 20, "wrap": 560 }, "children": [
            { "kind": "shape", "key": "a", "size": { "w": "40%" }, "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent" },
            { "kind": "shape", "key": "b", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent" }
        ] } });
    let boxes = |w: f64| -> Vec<Rect> {
        scene(doc(w)).root.children().iter().map(|n| {
            let b = datars_engine::node_bounds(n);
            Rect::new(b.x, b.y, b.w, b.h)
        }).collect()
    };
    let wide = boxes(800.0);
    assert!((wide[0].w - 320.0).abs() < 0.5 && wide[1].x > wide[0].x1(), "side by side: {wide:?}");
    let narrow = boxes(390.0);
    assert!((narrow[0].w - 390.0).abs() < 0.5 && (narrow[1].w - 390.0).abs() < 0.5, "full width each: {narrow:?}");
    assert!((narrow[0].h - 190.0).abs() < 0.5 && narrow[1].y >= narrow[0].y1() + 19.5, "one above the other, equal shares: {narrow:?}");
}

#[test]
fn a_tick_repeat_takes_a_count_expression_and_tells_each_tick_how_many_there_are() {
    let doc = |count: serde_json::Value| serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 100 },
        "scene": { "kind": "group", "key": "root", "scales": { "x": { "type": "linear", "domain": [0, 100], "range": [0, 400] } }, "children": [
            { "kind": "repeat", "from": { "ticks": "x", "count": count }, "template":
                { "kind": "text", "text": "=d.label + '/' + d.count", "at": ["=d.pos", 10] } }
        ] } });
    let labels = |c: serde_json::Value| texts(&scene(doc(c))).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
    assert_eq!(labels(serde_json::json!("=floor(scale.x.max() / 200)")), ["0/3", "50/3", "100/3"]);
    assert_eq!(labels(serde_json::json!(5)).len(), 6, "a number as before");
}

#[test]
fn measure_word_is_the_width_a_label_cannot_wrap_below() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 100 },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "text", "key": "all", "text": "=format(measure('Social Democrats', 11), '.2f')", "at": [0, 10] },
            { "kind": "text", "key": "word", "text": "=format(measure.word('Social Democrats', 11), '.2f')", "at": [0, 30] },
            { "kind": "text", "key": "longest", "text": "=format(measure('Democrats', 11), '.2f')", "at": [0, 50] }
        ] } });
    let all = texts(&scene(doc));
    let num = |i: usize| all[i].0.parse::<f64>().unwrap();
    assert!(num(1) < num(0) * 0.7, "{all:?}");
    assert!((num(1) - num(2)).abs() < 0.5, "its longest word: {all:?}");
}

#[test]
fn rotated_text_takes_the_room_it_covers() {
    // An auto-height row with a label turned −45° under it: the row is as tall as the slant.
    let doc = |rotate: f64| serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 300 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows" }, "children": [
            { "kind": "group", "key": "axis", "size": { "h": "auto" }, "children": [
                { "kind": "text", "key": "name", "text": "Christian Democrats", "at": [150, 8], "rotate": rotate, "style": { "size": 11, "align": "end", "baseline": "middle" } }
            ] },
            { "kind": "shape", "key": "rest", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent" }
        ] } });
    let rest_y = |rotate: f64| scene(doc(rotate)).root.children()[1].common.transform.apply(Vec2::ZERO).y;
    let (flat, turned) = (rest_y(0.0), rest_y(-45.0));
    assert!(flat < 30.0, "{flat}");
    assert!(turned > 70.0 && turned < 100.0, "a 105 px name at 45° reaches ~0.71 × its length down: {turned}");
}
