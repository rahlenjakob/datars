//! Comparison and distribution charts through the engine: the `kde` table op (density curves per
//! group), `stack`'s `order`, and the std recipes built on them and on the older ops — histograms
//! whose plot fits the bins, box plots' five numbers, violins turned upright, slope labels pushed
//! apart with room at both ends, pyramids and marimekkos.

use datars_engine::Engine;
use datars_scene::{Geom, Node, NodeKind};
use serde_json::{json, Value};

fn engine(doc: &Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).expect("doc parses"));
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn snapshot(doc: Value) -> String {
    let mut e = engine(&doc);
    let s = e.scene().snapshot();
    assert!(e.diagnostics().is_empty(), "{:?}\n{s}", e.diagnostics());
    s
}

/// A table drawn as one rect per row — `x`, `y`, `w`, `h` expressions over it — read back as
/// `(row key, [x, y, w, h])`.
fn rows(data: Value, ops: Value, cols: [&str; 4]) -> Vec<(String, [f64; 4])> {
    let doc = json!({
        "datars": 1, "size": { "width": 100, "height": 100 },
        "data": { "t": data }, "tables": { "k": { "from": "t", "ops": ops } },
        "scene": { "kind": "group", "key": "root", "children": [{ "kind": "repeat", "from": "k", "template": {
            "kind": "shape", "fill": "$accent",
            "geom": { "type": "rect", "x": format!("={}", cols[0]), "y": format!("={}", cols[1]), "w": format!("={}", cols[2]), "h": format!("={}", cols[3]) } } }] }
    });
    let mut e = engine(&doc);
    let scene = e.scene();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    scene.root.children().iter().map(|n: &Node| match &n.kind {
        NodeKind::Shape { geom: Geom::Rect { x, y, w, h, .. }, .. } => (n.key.to_string(), [*x, *y, *w, *h]),
        _ => (n.key.to_string(), [f64::NAN; 4]),
    }).collect()
}

fn trapezoid(pts: &[(f64, f64)]) -> f64 {
    pts.windows(2).map(|w| (w[1].0 - w[0].0) * (w[0].1 + w[1].1) / 2.0).sum()
}

fn two_groups() -> Value {
    json!({ "values": { "id": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10], "g": ["a", "a", "a", "a", "a", "b", "b", "b", "b", "b"], "v": [1.0, 2.0, 2.5, 3.0, 4.0, 6.0, 7.5, 8.0, 8.5, 12.0] }, "key": ["id"] })
}

#[test]
fn the_kde_op_gives_each_group_a_density_curve_on_one_grid() {
    let r = rows(two_groups(), json!([{ "op": "kde", "field": "v", "groupby": ["g"], "steps": 41, "extend": 3 }]), ["d.value", "d.density", "d.__sample", "1"]);
    assert_eq!(r.len(), 82, "41 samples per group");
    // Keyed (group, sample), groups in order of first appearance.
    assert_eq!(r[0].0, "(\"a\", 0)");
    assert_eq!(r[41].0, "(\"b\", 0)");
    let (a, b): (Vec<_>, Vec<_>) = (r[..41].iter().map(|x| (x.1[0], x.1[1])).collect::<Vec<_>>(), r[41..].iter().map(|x| (x.1[0], x.1[1])).collect::<Vec<_>>());
    assert_eq!(a.iter().map(|p| p.0).collect::<Vec<_>>(), b.iter().map(|p| p.0).collect::<Vec<_>>(), "one grid shared by both groups");
    // The grid spans the whole table's values, widened by three of the widest group's bandwidths.
    assert!(a[0].0 < 1.0 && a[40].0 > 12.0, "{:?} … {:?}", a[0], a[40]);
    for (name, curve) in [("a", &a), ("b", &b)] {
        let area = trapezoid(curve);
        assert!((area - 1.0).abs() < 0.02, "group {name}: area {area}");
        assert!(curve.iter().all(|p| p.1 >= 0.0));
    }
    // a's mass sits around 2.5, b's around 8.
    let peak = |c: &[(f64, f64)]| c.iter().fold((0.0, f64::MIN), |m, p| if p.1 > m.1 { *p } else { m }).0;
    assert!((peak(&a) - 2.5).abs() < 1.0 && (peak(&b) - 8.0).abs() < 1.5, "{} {}", peak(&a), peak(&b));
}

#[test]
fn the_kde_op_trims_to_each_group_and_takes_a_bandwidth() {
    let trimmed = rows(two_groups(), json!([{ "op": "kde", "field": "v", "groupby": ["g"], "steps": 11, "trim": true }]), ["d.value", "d.density", "1", "1"]);
    assert_eq!((trimmed[0].1[0], trimmed[10].1[0]), (1.0, 4.0), "a's own extent");
    assert_eq!((trimmed[11].1[0], trimmed[21].1[0]), (6.0, 12.0), "b's own extent");
    // A wider kernel: a lower peak over the same grid.
    let peak = |bw: f64| {
        let r = rows(two_groups(), json!([{ "op": "kde", "field": "v", "groupby": ["g"], "steps": 11, "bandwidth": bw, "extent": [0, 14] }]), ["d.value", "d.density", "1", "1"]);
        r[..11].iter().map(|x| x.1[1]).fold(0.0, f64::max)
    };
    assert!(peak(3.0) < peak(0.5), "{} vs {}", peak(3.0), peak(0.5));
    // No groupby: one curve over every row.
    let one = rows(two_groups(), json!([{ "op": "kde", "field": "v", "steps": 5 }]), ["d.value", "d.density", "1", "1"]);
    assert_eq!(one.len(), 5);
    assert_eq!(one[0].0, "(0,)");
}

#[test]
fn the_stack_op_orders_series_as_asked() {
    let data = json!({ "values": { "s": ["small", "big", "mid", "small", "big", "mid"], "x": [1, 1, 1, 2, 2, 2], "v": [1, 10, 5, 1, 12, 4] }, "key": ["s", "x"] });
    let base = |order: &str| {
        let r = rows(data.clone(), json!([{ "op": "stack", "x": "x", "series": "s", "value": "v", "order": order }]), ["d.y0", "d.y1", "1", "1"]);
        r.into_iter().filter(|(k, _)| k.ends_with(" 1)")).map(|(k, v)| (k, v[0])).collect::<Vec<_>>()
    };
    let at = |v: &[(String, f64)], s: &str| v.iter().find(|(k, _)| k.contains(s)).map(|x| x.1).unwrap();
    let input = base("input");
    assert_eq!((at(&input, "small"), at(&input, "big"), at(&input, "mid")), (0.0, 1.0, 11.0), "in the data's order");
    let desc = base("descending");
    assert_eq!((at(&desc, "big"), at(&desc, "mid"), at(&desc, "small")), (0.0, 10.0, 15.0), "largest total at the baseline");
    let asc = base("ascending");
    assert_eq!((at(&asc, "small"), at(&asc, "mid"), at(&asc, "big")), (0.0, 1.0, 6.0));
}

fn plot(data: Value, params: Value, children: Value) -> Value {
    let mut p = json!({ "data": "t", "children": children });
    p.as_object_mut().unwrap().extend(params.as_object().unwrap().clone());
    json!({ "datars": 1, "size": { "width": 480, "height": 320 }, "data": { "t": data }, "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": p } })
}

fn use_(recipe: &str, params: Value) -> Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params })
}

fn num_after(line: &str, key: &str) -> f64 {
    line.split(key).nth(1).and_then(|r| r.split([' ', ',', ')']).next()).and_then(|x| x.parse().ok()).unwrap_or(f64::NAN)
}

/// The lines of a snapshot inside every group with `key`.
fn sections<'a>(snap: &'a str, key: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = snap.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate().filter(|(_, l)| l.contains(&format!("group (\"{key}\",)"))) {
        let depth = l.len() - l.trim_start().len();
        out.extend(lines[i + 1..].iter().take_while(|m| m.len() - m.trim_start().len() > depth).copied());
    }
    out
}

/// The lines of a snapshot inside the group with `key` (by indentation).
fn section<'a>(snap: &'a str, key: &str) -> Vec<&'a str> {
    let lines: Vec<&str> = snap.lines().collect();
    let Some(i) = lines.iter().position(|l| l.contains(&format!("group (\"{key}\",)"))) else { return Vec::new() };
    let depth = lines[i].len() - lines[i].trim_start().len();
    lines[i + 1..].iter().take_while(|l| l.len() - l.trim_start().len() > depth).copied().collect()
}

fn minutes() -> Value {
    let v: Vec<f64> = vec![3.0, 7.0, 8.0, 12.0, 13.0, 14.0, 17.0, 18.0, 19.0, 19.5, 22.0, 24.0, 27.0, 31.0, 44.0];
    json!({ "values": { "id": (0..v.len()).collect::<Vec<_>>(), "m": v }, "key": ["id"] })
}

#[test]
fn a_histogram_counts_every_row_and_its_plot_fits_the_bins() {
    let s = snapshot(plot(minutes(), json!({ "x": "m", "xType": "linear" }), json!([use_("histogram", json!({ "step": 10 }))])));
    let bars: Vec<&str> = section(&s, "bins").into_iter().filter(|l| l.contains(" rect ")).collect();
    // [0,10) 3, [10,20) 7, [20,30) 3, [30,40) 1, [40,50) 1.
    assert_eq!(bars.len(), 5, "{s}");
    let labels: Vec<&str> = bars.iter().map(|l| l.rsplit('"').nth(1).unwrap_or("")).collect();
    assert_eq!(labels, vec!["0–10: 3", "10–20: 7", "20–30: 3", "30–40: 1", "40–50: 1"]);
    // Without a y field, the plot still draws a count axis — up to the tallest bin (7).
    let heights: Vec<f64> = bars.iter().map(|l| num_after(l, "h=")).collect();
    // (Snapshots write two decimals: ratios agree to about 1e-4.)
    assert!((heights[1] / heights[0] - 7.0 / 3.0).abs() < 1e-3, "{heights:?}");
    assert!(s.contains("role=tick \"7\"") && !s.contains("role=tick \"44\""), "a y axis to the counts");
    // The x axis spans the bins (0 to 50), not just the values (3 to 44).
    let first_x = num_after(bars[0], "x=");
    assert!(first_x.abs() < 1.0, "the first bin starts where the axis does: {first_x}");
}

#[test]
fn a_box_plot_draws_five_numbers_and_the_outliers_beyond_them() {
    let data = json!({ "values": { "id": [1, 2, 3, 4, 5, 6, 7, 8], "g": ["a", "a", "a", "a", "a", "a", "a", "b"], "v": [1, 2, 3, 4, 5, 6, 30, 9] }, "key": ["id"] });
    let s = snapshot(plot(data, json!({ "x": "g", "y": "v" }), json!([use_("boxplot", json!({}))])));
    let boxes = section(&s, "boxes");
    let a = boxes.iter().find(|l| l.contains("(\"box\",)") && l.contains("\"a:")).expect("a's box");
    // R-7 quartiles of 1…6, 30: 2.5, 4, 5.5; the whisker stops at 6 (30 is beyond 5.5 + 1.5 × 3).
    assert!(a.contains("median 4, middle half 2.5–5.5, whiskers 1–6 (n = 7)"), "{a}");
    let outliers: Vec<&str> = section(&s, "outliers").into_iter().filter(|l| l.contains("ellipse")).collect();
    assert_eq!(outliers.len(), 1, "{s}");
    assert!(outliers[0].contains("a: 30 (beyond the whiskers)"), "{}", outliers[0]);
}

#[test]
fn an_upright_violin_is_an_area_turned_a_quarter() {
    let s = snapshot(plot(two_groups(), json!({ "x": "g", "y": "v" }), json!([use_("violin", json!({ "steps": 20 }))])));
    let outlines: Vec<&str> = s.lines().filter(|l| l.contains("(\"violin\",) area")).collect();
    assert_eq!(outlines.len(), 2, "{s}");
    assert!(outlines.iter().all(|l| l.contains("n=20") && l.contains("transform=[0.00 -1.00 1.00 0.00")), "{outlines:?}");
    assert!(outlines[0].contains("\"a: median 2.5, middle half 2–3 (n = 5)\""), "{}", outlines[0]);
    // Lying down (a band y), no turn.
    let s = snapshot(plot(two_groups(), json!({ "x": "v", "y": "g", "xType": "linear", "yType": "band" }), json!([use_("violin", json!({}))])));
    assert!(s.lines().filter(|l| l.contains("(\"violin\",) area")).all(|l| !l.contains("transform")), "{s}");
}

#[test]
fn slope_labels_are_pushed_apart_and_the_plot_makes_room_at_both_ends() {
    let rows: Vec<Value> = [("A", 10.0, 20.0), ("B", 10.2, 12.0), ("C", 10.4, 12.1), ("D", 30.0, 25.0)]
        .iter()
        .flat_map(|(s, a, b)| [json!({ "s": s, "t": "2020", "v": a }), json!({ "s": s, "t": "2024", "v": b })])
        .collect();
    let data = json!({ "values": rows, "key": ["s", "t"] });
    let s = snapshot(plot(data, json!({ "x": "t", "y": "v", "xType": "point", "color": "s", "axes": "x" }), json!([use_("slope", json!({}))])));
    assert!(s.contains("group (\"start-labels\",)") && s.contains("group (\"end-labels\",)"), "{s}");
    let ys = |key: &str| -> Vec<f64> {
        // `text "…" at (x, y)`: the y.
        let at_y = |l: &str| l.split("at (").nth(1).and_then(|r| r.split(", ").nth(1)).and_then(|y| y.split(')').next()).and_then(|y| y.parse().ok()).unwrap_or(f64::NAN);
        let mut v: Vec<f64> = sections(&s, key).into_iter().filter(|l| l.contains(" text ")).map(at_y).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    };
    let starts = ys("start-labels");
    assert_eq!(starts.len(), 4);
    assert!(starts.windows(2).all(|w| w[1] - w[0] >= 11.0), "start labels a line apart: {starts:?}");
    let text: Vec<&str> = sections(&s, "start-labels").into_iter().filter(|l| l.contains(" text ")).collect();
    assert!(text.iter().any(|l| l.contains("\"A  10\"")), "name and value: {text:?}");
}

#[test]
fn a_pyramid_grows_one_side_left_and_the_other_right_on_one_scale() {
    let data = json!({ "values": [
        { "age": "0–9", "sex": "Men", "n": 100 }, { "age": "0–9", "sex": "Women", "n": 100 },
        { "age": "10–19", "sex": "Men", "n": 50 }, { "age": "10–19", "sex": "Women", "n": 25 }], "key": ["age", "sex"] });
    let doc = json!({ "datars": 1, "size": { "width": 480, "height": 320 }, "data": { "t": data }, "scene": use_("pyramid", json!({ "data": "t", "y": "age", "side": "sex", "value": "n" })) });
    let s = snapshot(doc);
    let bars = |pane: &str| -> Vec<(f64, f64)> { section(&s, pane).into_iter().filter(|l| l.contains(" rect ") && l.contains("role=datum")).map(|l| (num_after(l, "x="), num_after(l, "w="))).collect() };
    let (left, right) = (bars("left"), bars("right"));
    assert_eq!((left.len(), right.len()), (2, 2), "{s}");
    // Left bars end at the pane's inner edge (the same x) and grow leftward; right bars start at 0.
    assert!((left[0].0 + left[0].1 - (left[1].0 + left[1].1)).abs() < 0.02, "{left:?}");
    assert!(right.iter().all(|r| r.0 == 0.0), "{right:?}");
    // One scale: 100 men and 100 women are equally long; 50 men twice 25 women.
    assert!((left[0].1 - right[0].1).abs() < 0.02 && (left[1].1 / right[1].1 - 2.0).abs() < 1e-3, "{left:?} {right:?}");
    // The youngest at the bottom.
    let y = |l: &str| num_after(l, "y=");
    let lines: Vec<&str> = section(&s, "left").into_iter().filter(|l| l.contains(" rect ") && l.contains("role=datum")).collect();
    assert!(y(lines[0]) > y(lines[1]), "0–9 below 10–19");
}

#[test]
fn marimekko_columns_are_as_wide_as_their_share_of_the_total() {
    let data = json!({ "values": [
        { "r": "N", "k": "x", "v": 30 }, { "r": "N", "k": "y", "v": 10 },
        { "r": "S", "k": "x", "v": 5 }, { "r": "S", "k": "y", "v": 15 }], "key": ["k", "r"] });
    let doc = json!({ "datars": 1, "size": { "width": 480, "height": 320 }, "data": { "t": data }, "scene": use_("marimekko", json!({ "data": "t", "x": "r", "series": "k", "value": "v", "gap": 0 })) });
    let s = snapshot(doc);
    let cells: Vec<(String, [f64; 4])> = section(&s, "area").into_iter().filter(|l| l.contains(" rect ")).map(|l| (l.trim().split(") ").next().unwrap_or("").to_string() + ")", [num_after(l, "x="), num_after(l, "y="), num_after(l, "w="), num_after(l, "h=")])).collect();
    assert_eq!(cells.len(), 4, "{s}");
    let get = |k: &str| cells.iter().find(|c| c.0 == k).map(|c| c.1).unwrap_or_else(|| panic!("{k} in {cells:?}"));
    let (nx, sx, ny) = (get("(\"x\", \"N\")"), get("(\"x\", \"S\")"), get("(\"y\", \"N\")"));
    // N holds 40 of 60: two thirds of the width; within N, x is three quarters of the height.
    assert!((nx[2] / (nx[2] + sx[2]) - 2.0 / 3.0).abs() < 1e-3, "{nx:?} {sx:?}");
    assert!((nx[3] / (nx[3] + ny[3]) - 0.75).abs() < 1e-3, "{nx:?} {ny:?}");
}
