//! Box layout (rows, columns, grid): what takes room. A child whose `when` is false isn't there —
//! it takes no share of the space and leaves no gap.

use datars_engine::Engine;
use datars_scene::{Node, NodeKind};

fn scene(layout: serde_json::Value, children: Vec<serde_json::Value>) -> datars_scene::Scene {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
        "scene": { "kind": "group", "key": "root", "layout": layout, "children": children } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.scene()
}

/// A box child: a group holding one rect that fills its box (`size` fixes the main axis).
fn cell(key: &str, when: bool, size: Option<serde_json::Value>) -> serde_json::Value {
    let mut c = serde_json::json!({ "kind": "group", "key": key, "children": [
        { "kind": "shape", "key": "r", "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": "=box.h" }, "fill": "$accent" }] });
    if !when {
        c["when"] = serde_json::json!("=1 == 2");
    }
    if let Some(s) = size {
        c["size"] = s;
    }
    c
}

/// (x, y, w, h) of a child's rect in the root's space.
fn rect_of(s: &datars_scene::Scene, key: &str) -> Option<(f64, f64, f64, f64)> {
    let g = s.root.children().iter().find(|n| n.key == datars_scene::Key::name(key))?;
    let r: &Node = g.children().first()?;
    let NodeKind::Shape { geom: datars_scene::Geom::Rect { w, h, .. }, .. } = &r.kind else { return None };
    let o = g.common.transform.apply(datars_math::Vec2::ZERO);
    Some((o.x, o.y, *w, *h))
}

#[test]
fn a_hidden_column_leaves_no_gap() {
    let w = |px: f64| Some(serde_json::json!({ "w": px }));
    let s = scene(serde_json::json!({ "type": "columns", "gap": 10 }), vec![cell("a", true, w(20.0)), cell("b", false, w(20.0)), cell("c", true, w(20.0))]);
    assert_eq!(rect_of(&s, "a").map(|r| r.0), Some(0.0));
    assert!(rect_of(&s, "b").is_none(), "hidden");
    assert_eq!(rect_of(&s, "c").map(|r| r.0), Some(30.0), "right after a and one gap: {}", s.snapshot());
}

#[test]
fn a_hidden_row_leaves_no_gap() {
    let h = |px: f64| Some(serde_json::json!({ "h": px }));
    let s = scene(serde_json::json!({ "type": "rows", "gap": 8 }), vec![cell("a", true, h(20.0)), cell("b", false, h(20.0)), cell("c", true, h(20.0))]);
    assert_eq!(rect_of(&s, "c").map(|r| r.1), Some(28.0), "{}", s.snapshot());
}

#[test]
fn a_hidden_fill_child_takes_no_share() {
    // Three fill children in 200 px with 10 px gaps: two shown split 190 px, not 180 − 10.
    let s = scene(serde_json::json!({ "type": "columns", "gap": 10 }), vec![cell("a", true, None), cell("b", false, None), cell("c", true, None)]);
    assert_eq!(rect_of(&s, "a"), Some((0.0, 0.0, 95.0, 100.0)), "{}", s.snapshot());
    assert_eq!(rect_of(&s, "c"), Some((105.0, 0.0, 95.0, 100.0)));
}

#[test]
fn a_hidden_grid_cell_is_taken_by_the_next_child() {
    let s = scene(serde_json::json!({ "type": "grid", "columns": 2, "gap": 0 }), vec![cell("a", true, None), cell("b", false, None), cell("c", true, None)]);
    // Two children in two columns: one row, c beside a.
    assert_eq!(rect_of(&s, "c"), Some((100.0, 0.0, 100.0, 100.0)), "{}", s.snapshot());
}

#[test]
fn a_recipe_sized_by_its_expansion_takes_only_that_room() {
    // `std/title` asks to be as tall as its lines; its use node says nothing. The rect below gets
    // the rest — as it does when the document is pre-expanded — not half the box.
    let title = serde_json::json!({ "kind": "use", "recipe": "@datars/std/title", "params": { "text": "Rainfall" } });
    let s = scene(serde_json::json!({ "type": "rows" }), vec![title, cell("plot", true, None)]);
    let (_, y, _, h) = rect_of(&s, "plot").expect("the plot");
    assert!(y > 10.0 && y < 40.0 && (y + h - 100.0).abs() < 1e-9, "one line of title, then the plot: y={y} h={h}\n{}", s.snapshot());
}

/// A flow places its children inside its padding, as rows, columns and grids do: the first line
/// at the padded top-left, wrapping at the padded right edge. (It kept each child's own vertical
/// origin, so the top padding was never applied.)
#[test]
fn a_flow_keeps_inside_its_padding() {
    let chip = |key: &str, w: f64| serde_json::json!({ "kind": "group", "key": key, "children": [
        { "kind": "shape", "key": "r", "geom": { "type": "rect", "x": 0, "y": 0, "w": w, "h": 12 }, "fill": "$accent" }] });
    let s = scene(serde_json::json!({ "type": "flow", "gap": 10, "padding": [10, 6, 4, 8] }), vec![chip("a", 80.0), chip("b", 80.0), chip("c", 10.0)]);
    assert_eq!(rect_of(&s, "a"), Some((8.0, 10.0, 80.0, 12.0)), "top and left padding: {}", s.snapshot());
    assert_eq!(rect_of(&s, "b"), Some((98.0, 10.0, 80.0, 12.0)), "one gap along");
    // 188 + 10 would fit the 200 px box but crosses the right padding (at 194): a new line, one
    // line (12) and a gap below the padded top.
    assert_eq!(rect_of(&s, "c"), Some((8.0, 32.0, 10.0, 12.0)));
}
