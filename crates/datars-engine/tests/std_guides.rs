//! Standard-library guides and charts through the engine, where what matters is the resolved
//! scene: axis titles that are drawn (not decluttered away), a legend's title, reference lines at
//! expression values, calendars over several years, a plot without a y.

use datars_engine::Engine;
use serde_json::{json, Value};

fn snapshot(doc: Value) -> String {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    let s = e.scene().snapshot();
    assert!(e.diagnostics().is_empty(), "{:?}\n{s}", e.diagnostics());
    s
}

fn use_(recipe: &str, params: Value) -> Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params })
}

fn doc(data: Value, scene: Value) -> Value {
    json!({ "datars": 1, "size": { "width": 480, "height": 300 }, "data": data, "scene": scene })
}

fn cities() -> Value {
    json!({ "t": { "values": { "k": ["a", "b", "c"], "n": [3, 1, 2], "s": [0.5, 0.8, 1.0] }, "key": ["k"] } })
}

/// The snapshot line of the text reading `text`.
fn text_line<'a>(snap: &'a str, text: &str) -> Option<&'a str> {
    snap.lines().find(|l| l.contains(&format!("text \"{text}\"")))
}

fn num_after(line: &str, key: &str) -> f64 {
    line.split(key).nth(1).and_then(|r| r.split([' ', ',', ')']).next()).and_then(|x| x.parse().ok()).unwrap_or(f64::NAN)
}

#[test]
fn a_plots_y_title_is_drawn_above_the_axis() {
    let plot = use_("plot", json!({ "data": "t", "x": "k", "y": "n", "title": "Things", "yLabel": "Count", "right": { "y": "s", "domain": [0, 1], "label": "Share" },
        "children": [use_("bar", json!({}))] }));
    let s = snapshot(doc(cities(), plot));
    let y = text_line(&s, "Count").unwrap_or_else(|| panic!("the y title is drawn: {s}"));
    let y2 = text_line(&s, "Share").unwrap_or_else(|| panic!("the right axis's title too: {s}"));
    assert!(!y.contains("hidden") && !y2.contains("hidden"), "{y}\n{y2}");
    // In a row of its own: below the title, above the body (where the axes and plot area are).
    let titles = s.lines().find(|l| l.contains("group (\"axis-titles\",)")).unwrap();
    let body = s.lines().find(|l| l.contains("group (\"body\",)")).unwrap();
    let ty = |l: &str| l.split("transform=[").nth(1).and_then(|t| t.split(']').next()).and_then(|t| t.split(' ').nth(5)).and_then(|v| v.parse::<f64>().ok()).unwrap();
    assert!(ty(titles) > 15.0 && ty(titles) < ty(body), "{titles}\n{body}");
    // The y axis itself carries no title (it would take width up the side).
    let axis: Vec<&str> = s.lines().skip_while(|l| !l.contains("group (\"axis-y\",)")).take_while(|l| !l.contains("group (\"center\",)")).collect();
    assert!(!axis.iter().any(|l| l.contains("\"Count\"")), "{axis:?}");
}

#[test]
fn a_standalone_vertical_axis_title_stands_above_its_top() {
    let scene = json!({ "kind": "group", "key": "root", "scales": { "y": { "type": "linear", "domain": [0, 100], "range": [250, 40] } }, "children": [
        { "kind": "group", "key": "at", "transform": { "translate": [100, 0] }, "children": [use_("axis", json!({ "scale": "y", "orient": "left", "label": "Percent" }))] },
    ] });
    let s = snapshot(doc(json!({}), scene));
    let l = text_line(&s, "Percent").unwrap_or_else(|| panic!("{s}"));
    assert!(!l.contains("hidden"), "not decluttered away: {l}");
    assert!(num_after(l, ", ") <= 32.0, "above the axis's top end (40): {l}");
}

#[test]
fn a_legend_title_is_drawn_above_its_entries() {
    let scene = json!({ "kind": "group", "key": "root", "scales": { "color": { "type": "categorical", "domain": { "data": "t", "field": "k" }, "range": "$categorical" } },
        "layout": { "type": "rows" }, "children": [use_("legend", json!({ "scale": "color", "title": "Kind" }))] });
    let s = snapshot(doc(cities(), scene));
    let title = text_line(&s, "Kind").unwrap_or_else(|| panic!("{s}"));
    assert!(!title.contains("hidden"));
    let items = s.lines().find(|l| l.contains("group (\"items\",)")).unwrap_or_else(|| panic!("{s}"));
    assert!(items.contains("transform=[1.00 0.00 0.00 1.00 0.00 1"), "the entries start a line below: {items}");
    assert_eq!(s.lines().filter(|l| l.contains("role=legend-item")).count(), 3, "{s}");
}

#[test]
fn rules_and_spans_take_expression_values() {
    // At the mean (2) of 1…3 on a linear y from 0 to 3: two thirds of the way up.
    let plot = use_("plot", json!({ "data": "t", "x": "k", "y": "n", "yDomain": [0, 3], "nice": false, "children": [
        use_("rule", json!({ "value": { "expr": "table.mean('t', 'n')" }, "label": "Mean" })),
        use_("span", json!({ "axis": "y", "from": "=table.min('t', 'n')", "to": { "expr": "table.max('t', 'n')" } })),
    ] }));
    let s = snapshot(doc(cities(), plot));
    let rule = s.lines().skip_while(|l| !l.contains("group (\"rule-y-table.mean('t', 'n')\",)")).nth(1).unwrap_or_else(|| panic!("{s}"));
    let base = s.lines().find(|l| l.contains("(\"domain\",) segment (0.00, 0.00)→(0.00, ")).map(|l| num_after(l, "→(0.00, ")).unwrap();
    let y = num_after(rule, "segment (0.00, ");
    assert!((y - base / 3.0).abs() < 0.5, "the rule at 2 of 3: y {y} in {base}\n{s}");
    // The span from 1 to 3: from the top down to a third of the way up.
    let span = s.lines().find(|l| l.contains(" rect ") && l.contains("opacity=0.07")).unwrap_or_else(|| panic!("{s}"));
    assert!(num_after(span, "y=").abs() < 0.5 && (num_after(span, "h=") - base * 2.0 / 3.0).abs() < 0.5, "{span}");
}

#[test]
fn a_calendar_stacks_its_years_each_labelled() {
    let date: Vec<String> = (0..730)
        .map(|i| {
            // Day i after 1 January 2024 as ISO text.
            let (mut y, mut d) = (2024, i);
            loop {
                let len = if y % 4 == 0 { 366 } else { 365 };
                if d < len {
                    break;
                }
                d -= len;
                y += 1;
            }
            let months = [31, if y % 4 == 0 { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let mut m = 0;
            while d >= months[m] {
                d -= months[m];
                m += 1;
            }
            format!("{y}-{:02}-{:02}", m + 1, d + 1)
        })
        .collect();
    let n: Vec<f64> = (0..730).map(|i| (i % 7) as f64).collect();
    let data = json!({ "days": { "values": { "date": date, "n": n }, "key": ["date"], "types": { "date": "date" } } });
    let s = snapshot(doc(data, use_("calendar", json!({ "data": "days", "date": "date", "value": "n" }))));
    let (a, b) = (text_line(&s, "2024").unwrap_or_else(|| panic!("{s}")), text_line(&s, "2025").unwrap_or_else(|| panic!("{s}")));
    let (ya, yb) = (num_after(a, "at (0.00, "), num_after(b, "at (0.00, "));
    assert!(ya == 0.0 && yb > 0.0, "a row per year, 2025 under 2024: {a}\n{b}");
    // 7 weekday rows and a cell's gap per year, both years inside the box's 300 px.
    let cell = yb / 8.0;
    assert!(cell > 4.0 && 15.0 * cell <= 300.0 + 1e-6, "cell {cell}");
}

#[test]
fn a_plot_without_y_draws_a_one_dimensional_swarm() {
    let data = json!({ "t": { "values": { "k": ["a", "b", "c", "d"], "v": [1.0, 1.1, 1.15, 3.0] }, "key": ["k"] } });
    let s = snapshot(doc(data, use_("plot", json!({ "data": "t", "x": "v", "xType": "linear", "children": [use_("swarm", json!({}))] }))));
    assert!(s.contains("group (\"axis-x\",)") && !s.contains("group (\"axis-y\",)"), "an x axis only: {s}");
    assert!(s.contains("instances circle ×4"), "{s}");
    assert!(!s.contains("group (\"grid"), "no gridlines across a y that isn't there: {s}");
}
