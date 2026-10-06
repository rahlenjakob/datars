//! Labels the standard library draws, as the engine lays them out: band-axis names wrapped, turned
//! or thinned only as their measured widths need, numeric ticks that fit a phone, integer tick
//! formats on a linear axis, callouts kept under the title, cards no wider than the box, value labels on
//! grouped and stacked bars (only where they fit), and a reference line's label kept in its box.

use datars_engine::Engine;
use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Node, NodeKind, Scene};

fn scene(size: (f64, f64), data: serde_json::Value, plot: serde_json::Value) -> Scene {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": size.0, "height": size.1 }, "data": data,
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "key": "chart", "params": plot }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let s = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    s
}

/// Every drawn node under a path containing `under`, with its canvas-px bounds.
fn nodes(s: &Scene, under: &str) -> Vec<(Node, Rect)> {
    fn go(n: &Node, path: String, xf: Affine, under: &str, out: &mut Vec<(Node, Rect)>) {
        if !n.common.visible {
            return;
        }
        let here = format!("{path}/{}", n.key);
        let xf = xf.mul(n.common.transform);
        if let NodeKind::Group { children } = &n.kind {
            children.iter().for_each(|c| go(c, here.clone(), xf, under, out));
        } else if here.contains(under) {
            let local = datars_engine::node_bounds(&Node { common: Default::default(), ..n.clone() });
            let [a, b] = [(local.x, local.y), (local.x1(), local.y1())].map(|(x, y)| xf.apply(Vec2::new(x, y)));
            out.push((n.clone(), Rect::new(a.x, a.y, b.x - a.x, b.y - a.y)));
        }
    }
    let mut out = Vec::new();
    go(&s.root, String::new(), Affine::IDENTITY, under, &mut out);
    out
}

/// The non-empty texts under `under`: what they say, and where.
fn texts(s: &Scene, under: &str) -> Vec<(String, Rect)> {
    nodes(s, under)
        .into_iter()
        .filter_map(|(n, r)| match n.kind {
            NodeKind::Text(t) if !t.text.is_empty() => Some((t.text.clone(), r)),
            _ => None,
        })
        .collect()
}

fn overlapping(labels: &[(String, Rect)]) -> Option<(String, String)> {
    labels.iter().enumerate().find_map(|(i, (a, ra))| labels[i + 1..].iter().find(|(_, rb)| ra.intersect(rb).is_some_and(|x| x.w > 0.5 && x.h > 0.5)).map(|(b, _)| (a.clone(), b.clone())))
}

/// Turned (−45°) labels under `under`: their text and anchor, left to right.
fn turned(s: &Scene, under: &str) -> Vec<(String, Vec2)> {
    fn go(n: &Node, path: String, xf: Affine, under: &str, out: &mut Vec<(String, Vec2)>) {
        if !n.common.visible {
            return;
        }
        let here = format!("{path}/{}", n.key);
        let xf = xf.mul(n.common.transform);
        match &n.kind {
            NodeKind::Group { children } => children.iter().for_each(|c| go(c, here.clone(), xf, under, out)),
            NodeKind::Text(t) if here.contains(under) && t.rotate != 0.0 && !t.text.is_empty() => out.push((t.text.clone(), xf.apply(t.origin))),
            _ => {}
        }
    }
    let mut out = Vec::new();
    go(&s.root, String::new(), Affine::IDENTITY, under, &mut out);
    out
}

#[test]
fn band_labels_wrap_turn_or_thin_as_their_widths_need() {
    // Eight parties on a phone: one- and two-letter names fit their bands, so every one shows.
    let parties = serde_json::json!({ "v": { "values": { "party": ["S", "SD", "M", "V", "C", "KD", "MP", "L"], "share": [30, 20, 19, 7, 7, 5, 5, 5] }, "key": ["party"] } });
    let s = scene((320.0, 300.0), parties, serde_json::json!({ "data": "v", "x": "party", "y": "share", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] }));
    let names: Vec<String> = texts(&s, "axis-x").into_iter().map(|(t, _)| t).collect();
    assert_eq!(names, ["S", "SD", "M", "V", "C", "KD", "MP", "L"], "{}", s.snapshot());
    // The same parties by name on a phone: "Democrats" is wider than a band, so wrapped names
    // would run into each other — they turn 45° instead, every one shown, a line's height apart.
    let names = serde_json::json!({ "v": { "values": { "party": ["Social Democrats", "Sweden Democrats", "Moderates", "Left", "Centre", "Christian Democrats", "Greens", "Liberals"], "share": [30, 20, 19, 7, 7, 5, 5, 5] }, "key": ["party"] } });
    let plot = serde_json::json!({ "data": "v", "x": "party", "y": "share", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] });
    let s = scene((390.0, 600.0), names.clone(), plot.clone());
    let slanted = turned(&s, "axis-x");
    assert_eq!(slanted.len(), 8, "{}", s.snapshot());
    assert!(slanted.windows(2).all(|w| (w[1].1.x - w[0].1.x) * std::f64::consts::FRAC_1_SQRT_2 >= 13.0), "a line's height apart across the slant: {slanted:?}");
    assert!(texts(&s, "axis-x").iter().all(|(_, r)| r.x >= 0.0), "the first name, reaching left, stays on the canvas");
    // On a desktop the same names wrap flat in their bands, as before.
    let s = scene((960.0, 440.0), names, plot);
    assert!(turned(&s, "axis-x").is_empty());
    let flat = texts(&s, "axis-x");
    assert_eq!((flat.len(), overlapping(&flat)), (8, None), "{flat:?}");
    // Forty long names on a phone: turned, every k-th, and those shown never overlap.
    let cats: Vec<String> = (1..=40).map(|i| format!("Category {i}")).collect();
    let n: Vec<f64> = (0..40).map(|i| (i % 3 + 1) as f64).collect();
    let long = serde_json::json!({ "v": { "values": { "k": cats, "n": n }, "key": ["k"] } });
    let s = scene((350.0, 300.0), long, serde_json::json!({ "data": "v", "x": "k", "y": "n", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] }));
    let shown = turned(&s, "axis-x");
    assert!(shown.len() < 40 && shown.len() >= 10, "{shown:?}");
    assert!(shown.windows(2).all(|w| (w[1].1.x - w[0].1.x) * std::f64::consts::FRAC_1_SQRT_2 >= 13.0), "{shown:?}");
}

#[test]
fn a_numeric_axis_across_a_phone_labels_only_ticks_that_fit() {
    // Amounts to 60,000 kr on horizontal bars 250 px across: labels as wide as "60,000 kr" never touch.
    let data = serde_json::json!({ "v": { "values": { "k": ["Housing", "Food", "Transport"], "kr": [58944, 25713, 9787] }, "key": ["k"] } });
    for w in [300.0, 360.0, 960.0] {
        let s = scene((w, 400.0), data.clone(), serde_json::json!({ "data": "v", "x": "kr", "y": "k", "xType": "linear", "yType": "band", "suffix": " kr", "format": ",.0f", "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }] }));
        let mut ticks = texts(&s, "axis-x");
        ticks.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        assert!(ticks.len() >= 3, "{w}: {ticks:?}");
        assert!(ticks.windows(2).all(|p| p[1].1.x - p[0].1.x1() >= 6.0), "{w}: labels at least 6 px apart: {ticks:?}");
    }
}

#[test]
fn a_callout_above_a_peak_stays_under_the_title() {
    let data = serde_json::json!({ "c": { "values": { "year": [2019, 2020, 2021, 2022, 2023], "v": [1.7, 0.5, 2.4, 7.7, 6.0] }, "key": ["year"] } });
    let note = serde_json::json!({ "kind": "use", "recipe": "@datars/std/annotate", "params": { "x": "=scale.x(2022)", "y": "=scale.y(7.7)", "text": "2022 spike", "dx": -40, "dy": -24 } });
    let s = scene((390.0, 600.0), data, serde_json::json!({ "data": "c", "x": "year", "y": "v", "xType": "linear", "title": "Inflation (CPIF), annual % — Sweden", "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": {} }, note] }));
    let title = texts(&s, "(\"title\",)").pop().unwrap().1;
    let callout = texts(&s, "note-").into_iter().find(|(t, _)| t == "2022 spike").unwrap().1;
    assert!(callout.y >= title.y1(), "below the title: {callout:?} vs {title:?}");
}

#[test]
fn a_card_is_never_wider_than_a_phone_box_leaves() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 400 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "stack" }, "children": [
            { "kind": "use", "recipe": "@datars/std/card", "key": "caption", "params": { "text": "Mohamed Abdukardir Ali, nia på listan, passerade spärren med 107 röster.", "width": 320 } }
        ] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let s = e.scene();
    let back = nodes(&s, "backdrop").pop().unwrap().1;
    assert!(back.x >= 10.5 && back.x1() <= 289.5, "inside the margins (its 1 px stroke too): {back:?}");
    let text = texts(&s, "(\"text\",)").pop().unwrap().1;
    assert!(text.x1() <= back.x1() - 13.0, "the text wraps inside the card: {text:?} in {back:?}");
}

#[test]
fn a_linear_year_axis_reads_as_years() {
    // Decimal years (a month as a fraction) on a linear axis: ticks every half year would read
    // "2021.5". An integer format labels the whole years, each once; a tick count is honoured too.
    let t: Vec<f64> = (0..10).map(|i| 2021.0 + i as f64 * 0.5).collect();
    let data = serde_json::json!({ "r": { "values": { "t": t, "v": [1, 2, 4, 8, 10, 9, 7, 5, 4, 3] } } });
    let plot = |extra: serde_json::Value| {
        let mut p = serde_json::json!({ "data": "r", "x": "t", "y": "v", "xType": "linear", "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": {} }] });
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        p
    };
    let labels = |s: &Scene, axis: &str| texts(s, axis).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
    let s = scene((960.0, 400.0), data.clone(), plot(serde_json::json!({})));
    assert!(labels(&s, "axis-x").iter().any(|l| l.ends_with(".5")), "half years without a format: {:?}", labels(&s, "axis-x"));
    let s = scene((960.0, 400.0), data.clone(), plot(serde_json::json!({ "xFormat": "d" })));
    assert_eq!(labels(&s, "axis-x"), ["2021", "2022", "2023", "2024", "2025"]);
    let s = scene((960.0, 400.0), data.clone(), plot(serde_json::json!({ "xFormat": "d", "xTicks": 2 })));
    assert_eq!(labels(&s, "axis-x"), ["2022", "2024"]);
    // The value axis takes its own format and count the same way.
    let s = scene((960.0, 400.0), data, plot(serde_json::json!({ "yFormat": ".1f", "yTicks": 2 })));
    assert_eq!(labels(&s, "axis-y"), ["0.0", "5.0", "10.0"]);
}

#[test]
fn a_reference_lines_label_turns_back_at_the_right_edge() {
    // Horizontal bars with a line near the right end of a narrow plot: its label goes left of the
    // line, inside the box; a line with room to its right keeps its label there.
    let data = serde_json::json!({ "v": { "values": { "name": ["A", "B"], "votes": [650, 420] }, "key": ["name"] } });
    let at = |value: f64| {
        let rule = serde_json::json!({ "kind": "use", "recipe": "@datars/std/rule", "params": { "axis": "x", "value": value, "label": format!("Threshold: {value}") } });
        let s = scene((300.0, 200.0), data.clone(), serde_json::json!({ "data": "v", "x": "votes", "y": "name", "xType": "linear", "yType": "band", "xDomain": [0, 700], "children": [{ "kind": "use", "recipe": "@datars/std/bar", "params": {} }, rule] }));
        let line = nodes(&s, "rule-x").into_iter().find(|(n, _)| matches!(n.kind, NodeKind::Shape { .. })).map(|(_, r)| r.x + r.w / 2.0).unwrap();
        let (_, label) = texts(&s, "rule-x").pop().unwrap_or_else(|| panic!("{}", s.snapshot()));
        (line, label)
    };
    let (line, label) = at(660.0);
    assert!(label.x1() <= line && label.x1() <= 300.0 && label.x >= 0.0, "left of the line, inside: {label:?} vs {line}");
    let (line, label) = at(100.0);
    assert!(label.x >= line, "right of the line: {label:?} vs {line}");
}

/// The bars (datum shapes) under the plot's marks.
fn bars(s: &Scene) -> Vec<Rect> {
    nodes(s, "(\"marks\",)").into_iter().filter(|(n, _)| n.semantics.as_ref().is_some_and(|m| m.role == datars_scene::Role::Datum)).map(|(_, r)| r).collect()
}

#[test]
fn grouped_bars_are_labelled_where_the_labels_fit() {
    let data = |n: usize| {
        let (mut k, mut year, mut v) = (Vec::new(), Vec::new(), Vec::new());
        for i in 0..n {
            for (j, y) in ["2022", "2026"].iter().enumerate() {
                k.push(format!("Area {i}"));
                year.push(*y);
                v.push(4.0 + ((i * 2 + j) % 5) as f64 * 1.3);
            }
        }
        serde_json::json!({ "v": { "values": { "k": k, "year": year, "v": v }, "key": ["k", "year"] } })
    };
    let plot = serde_json::json!({ "data": "v", "x": "k", "y": "v", "color": "year", "children": [{ "kind": "use", "recipe": "@datars/std/grouped", "params": { "series": "year", "labels": true, "format": ".1f" } }] });
    let s = scene((640.0, 400.0), data(3), plot.clone());
    let (labels, bars) = (texts(&s, "(\"labels\",)"), bars(&s));
    assert_eq!((labels.len(), bars.len()), (6, 6), "{}", s.snapshot());
    for (t, r) in &labels {
        let cx = r.x + r.w / 2.0;
        assert!(bars.iter().any(|b| cx > b.x && cx < b.x1() && r.y1() <= b.y + 1.0 && r.y1() > b.y - 12.0), "{t} sits on top of its bar: {r:?} {bars:?}");
    }
    assert!(labels.iter().any(|(t, _)| t == "5.3"), "formatted: {labels:?}");
    // Twenty areas on a phone: bars far narrower than their labels, so no labels at all.
    let s = scene((300.0, 300.0), data(20), plot);
    assert_eq!(texts(&s, "(\"labels\",)"), [], "{}", s.snapshot());
}

#[test]
fn stacked_segments_are_labelled_where_the_labels_fit() {
    // One horizontal bar of three segments: the sliver is too small for its label.
    let data = serde_json::json!({ "v": { "values": { "row": ["All", "All", "All"], "party": ["A", "B", "C"], "seats": [100, 5, 244] }, "key": ["party"] } });
    let plot = serde_json::json!({ "data": "v", "x": "seats", "y": "row", "xType": "linear", "yType": "band", "color": "party", "children": [{ "kind": "use", "recipe": "@datars/std/stacked", "params": { "series": "party", "labels": true, "format": ".0f" } }] });
    let s = scene((400.0, 200.0), data.clone(), plot);
    let (labels, segs) = (texts(&s, "(\"labels\",)"), bars(&s));
    assert_eq!(labels.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["100", "244"], "{}", s.snapshot());
    for (t, r) in &labels {
        assert!(segs.iter().any(|b| r.x >= b.x && r.x1() <= b.x1() && r.y >= b.y && r.y1() <= b.y1()), "{t} inside its segment: {r:?} {segs:?}");
    }
    // Upright: the same, stacked up one column.
    let plot = serde_json::json!({ "data": "v", "x": "row", "y": "seats", "color": "party", "children": [{ "kind": "use", "recipe": "@datars/std/stacked", "params": { "series": "party", "labels": true, "format": ".0f", "suffix": " seats" } }] });
    let s = scene((300.0, 400.0), data, plot);
    let (labels, segs) = (texts(&s, "(\"labels\",)"), bars(&s));
    assert_eq!(labels.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["100 seats", "244 seats"], "{}", s.snapshot());
    assert!(labels.iter().all(|(_, r)| segs.iter().any(|b| r.x >= b.x && r.x1() <= b.x1() && r.y >= b.y && r.y1() <= b.y1())));
}
