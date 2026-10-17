//! The reader's text size (`Engine::set_text_scale`): the theme's text sizes scale, so text and
//! everything measured with it lay out again — and a chart moves to that layout like any other
//! change. Literal sizes stay as written; the size class follows the text; sessions replay it.

use datars_engine::{Engine, TEXT_SCALE_RANGE};
use serde_json::json;

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

/// A label in the theme's size, a box as wide as the label measures, a literal-size note and the
/// size class as text.
fn doc() -> serde_json::Value {
    json!({ "datars": 1, "size": { "width": 800, "height": 200 }, "scene": { "kind": "group", "key": "root", "children": [
        { "kind": "text", "key": "label", "text": "Monthly income", "at": [10, 20], "style": { "size": "$size.label" } },
        { "kind": "shape", "key": "under", "geom": { "type": "rect", "x": 10, "y": 30, "w": "=measure(\"Monthly income\", token(\"size.label\"))", "h": 2 }, "fill": "$ink" },
        { "kind": "text", "key": "note", "text": "fixed", "at": [10, 60], "style": { "size": 12 } },
        { "kind": "text", "key": "class", "text": "=sizeClass + \" \" + textScale", "at": [10, 90] }
    ] } })
}

fn line<'a>(snap: &'a str, key: &str) -> &'a str {
    snap.lines().find(|l| l.contains(&format!("(\"{key}\",)"))).unwrap_or_else(|| panic!("no {key} in {snap}"))
}

fn num_after(line: &str, key: &str) -> f64 {
    line.split(key).nth(1).and_then(|r| r.split([' ', ',', ')']).next()).and_then(|x| x.parse().ok()).unwrap_or(f64::NAN)
}

#[test]
fn theme_text_sizes_scale_and_what_is_measured_with_them_follows() {
    let mut e = engine(doc());
    let before = e.scene().snapshot();
    assert_eq!(num_after(line(&before, "label"), "size="), 11.0);
    let w1 = datars_engine::node_bounds(&e.scene().root.children()[1]).w;
    e.set_text_scale(2.0);
    let after = e.scene().snapshot();
    assert_eq!(num_after(line(&after, "label"), "size="), 22.0, "{after}");
    let w2 = datars_engine::node_bounds(&e.scene().root.children()[1]).w;
    assert!((w2 - 2.0 * w1).abs() < 0.5, "the measured width doubles with the text: {w1} → {w2}");
    // A size the document wrote as a number is the author's: it stays.
    assert_eq!(num_after(line(&after, "note"), "size="), 12.0);
    assert_eq!(e.theme().number("size.title"), Some(34.0));
    assert_eq!(e.text_scale(), 2.0);
    // Back to the theme's own sizes: the same scene as before, exactly.
    e.set_text_scale(1.0);
    assert_eq!(e.scene().snapshot(), before);
}

#[test]
fn the_size_class_is_measured_in_the_readers_text() {
    let mut e = engine(doc());
    assert!(line(&e.scene().snapshot(), "class").contains("\"tablet 1\""));
    // 800 px at twice the text holds what 400 px does at the theme's size.
    e.set_text_scale(2.0);
    assert!(line(&e.scene().snapshot(), "class").contains("\"phone 2\""), "{}", e.scene().snapshot());
    e.resize(1400.0, 200.0, 1.0);
    e.set_text_scale(1.25);
    assert!(line(&e.scene().snapshot(), "class").contains("\"wide 1.25\""));
}

#[test]
fn text_scale_is_clamped_to_a_readable_range() {
    let mut e = engine(doc());
    e.set_text_scale(10.0);
    assert_eq!(e.text_scale(), TEXT_SCALE_RANGE.1);
    e.set_text_scale(0.1);
    assert_eq!(e.text_scale(), TEXT_SCALE_RANGE.0);
    e.set_text_scale(f64::NAN);
    assert_eq!(e.text_scale(), 1.0);
}

#[test]
fn a_new_text_size_moves_to_the_new_layout() {
    let mut e = engine(doc());
    let settled = e.frame(0.0);
    assert!(!settled.animating);
    e.set_clock(1.0);
    e.set_text_scale(1.5);
    let f = e.frame(1.0);
    assert!(f.animating, "the change is a transition, not a jump");
    let mid = e.frame(1.3);
    assert!(mid.animating);
    let end = e.frame(5.0);
    assert!(!end.animating);
    assert_eq!(num_after(line(&e.scene().snapshot(), "label"), "size="), 16.5);
}

#[test]
fn std_charts_lay_out_again_around_bigger_labels() {
    let plot = json!({ "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "k", "y": "n", "title": "Things",
        "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] } });
    let d = json!({ "datars": 1, "size": { "width": 480, "height": 300 }, "data": { "t": { "values": { "k": ["alpha", "beta", "gamma"], "n": [3000, 1000, 2000] }, "key": ["k"] } }, "scene": plot });
    let mut e = engine(d);
    let area = |e: &mut Engine| {
        let s = e.scene().snapshot();
        let l = s.lines().find(|l| l.contains("group (\"center\",)")).map(str::to_string).unwrap_or_else(|| panic!("{s}"));
        let t = l.split("transform=[").nth(1).unwrap().split(']').next().unwrap().split(' ').map(|v| v.parse::<f64>().unwrap()).collect::<Vec<_>>();
        (t[4], s)
    };
    let (x1, s1) = area(&mut e);
    e.set_text_scale(2.0);
    let (x2, s2) = area(&mut e);
    assert!(x2 > x1 + 20.0, "the y axis's wider labels push the plot area right: {x1} → {x2}");
    let tick = |s: &str| s.lines().find(|l| l.contains("text \"3000\"")).map(|l| num_after(l, "size=")).unwrap_or(f64::NAN);
    assert_eq!(tick(&s1) * 2.0, tick(&s2), "{s2}");
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
}

/// A data table wider than its box at the reader's size keeps its columns on the canvas: its
/// text grows only as far as they fit (not below the theme's size); with room, all the way.
#[test]
fn a_table_grows_its_text_only_as_far_as_its_columns_fit() {
    let table = |w: f64| json!({ "datars": 1, "size": { "width": w, "height": 300 },
        "data": { "t": { "values": { "shop": ["Harbour", "Old Town", "Market Hall"], "city": ["Portsmouth", "York", "Bristol"], "revenue": [412000, 356000, 298000] }, "key": ["shop"] } },
        "scene": { "kind": "use", "recipe": "@datars/std/dataTable", "params": { "data": "t", "key": "shop", "columns": [
            { "field": "shop" }, { "field": "city" }, { "field": "revenue", "format": "$,.0f" }] } } });
    let size_of = |e: &mut Engine, text: &str| {
        let s = e.scene().snapshot();
        s.lines().find(|l| l.contains(&format!("text \"{text}\""))).map(|l| num_after(l, "size=")).unwrap_or(f64::NAN)
    };
    let right_edge = |e: &mut Engine| {
        let scene = e.scene();
        let mut max: f64 = 0.0;
        fn walk(n: &datars_scene::Node, xf: datars_math::Affine, max: &mut f64) {
            let xf = n.common.placed(xf);
            if let datars_scene::NodeKind::Text(t) = &n.kind {
                *max = max.max(datars_engine::text_rect(t, &xf).x1());
            }
            n.children().iter().for_each(|c| walk(c, xf, max));
        }
        walk(&scene.root, datars_math::Affine::IDENTITY, &mut max);
        max
    };
    // Wide: the reader's whole size.
    let mut e = engine(table(900.0));
    e.set_text_scale(2.0);
    assert_eq!(size_of(&mut e, "Harbour"), 26.0);
    // Narrow: smaller than 2 ×, never smaller than the theme's, and nothing past the edge.
    let mut e = engine(table(300.0));
    let base = size_of(&mut e, "Harbour");
    e.set_text_scale(2.0);
    let fitted = size_of(&mut e, "Harbour");
    assert!(fitted > base && fitted < 2.0 * base, "{base} → {fitted}");
    assert!(right_edge(&mut e) <= 300.5, "the columns fit: {}", right_edge(&mut e));
}

/// Fixed-size content taller than the box at the reader's size says how tall it needs to be;
/// at the theme's size (it fits) it says nothing.
#[test]
fn content_taller_than_the_box_says_its_height() {
    let rows: Vec<serde_json::Value> = ["Sales", "Members", "Shops"].iter().map(|l| json!({ "kind": "use", "key": l, "recipe": "@datars/std/progress", "params": { "value": 0.5, "label": l, "showGoal": true }, "size": { "h": "auto" } })).collect();
    let mut e = engine(json!({ "datars": 1, "size": { "width": 640, "height": 200 }, "scene": { "kind": "group", "key": "root", "layout": { "type": "rows", "gap": 14, "padding": 12 }, "children": rows } }));
    assert_eq!(e.content_height(), None);
    e.set_text_scale(2.0);
    let h = e.content_height().expect("taller than 200 px at 2 ×");
    assert!(h > 250.0, "{h}");
    e.resize(640.0, h, 1.0);
    assert_eq!(e.content_height(), None, "and fits at that height");
}

/// The weight and width of the text keyed `key` in the settled scene.
fn text_of(e: &mut Engine, key: &str) -> (u16, f64) {
    fn find(n: &datars_scene::Node, key: &str) -> Option<(u16, f64)> {
        if let datars_scene::NodeKind::Text(t) = &n.kind {
            if format!("{:?}", n.key).contains(&format!("\"{key}\"")) {
                return Some((t.style.weight, t.bounds.w));
            }
        }
        n.children().iter().find_map(|c| find(c, key))
    }
    find(&e.scene().root, key).unwrap_or_else(|| panic!("no text {key}"))
}

fn weight_of(e: &mut Engine, key: &str) -> u16 {
    text_of(e, key).0
}

#[test]
fn bold_text_draws_every_text_heavier_and_lays_out_again() {
    let mut d = doc();
    d["scene"]["children"][2]["style"]["weight"] = json!(700);
    let mut e = engine(d);
    let (regular, before) = text_of(&mut e, "label");
    assert_eq!((regular, weight_of(&mut e, "note")), (400, 700));
    e.start_recording();
    e.set_bold_text(true);
    assert_eq!((weight_of(&mut e, "label"), weight_of(&mut e, "note")), (600, 900), "regular as semibold; bold capped");
    let s = e.take_recording().unwrap();
    assert!(text_of(&mut e, "label").1 > before, "measured in the heavier face: wider");
    let mut r = Engine::replay(&s, |_, _| {}).unwrap();
    assert_eq!(weight_of(&mut r, "label"), 600, "a session replays it");
    e.set_bold_text(false);
    assert_eq!(weight_of(&mut e, "label"), 400);
}
