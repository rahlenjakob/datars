//! The standard library's engine-drawn controls, driven as a reader would drive them: a segmented
//! choice, a dropdown (opened, chosen from, closed by a click elsewhere), a switch, checkboxes for a
//! key set, a two-thumb range whose thumbs can't cross, and a button. Every one is also reachable
//! without a pointer (hosts offer them as buttons and adjustable controls), and none shows a
//! tooltip.

use datars_engine::{Engine, Pointer, SemanticItem};
use datars_expr::Value;
use std::cell::Cell;

thread_local! {
    /// The clock: each interaction settles a second later, as a reader's next click would come.
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

fn settle(e: &mut Engine) {
    let t = NOW.with(|n| {
        n.set(n.get() + 1.0);
        n.get()
    });
    e.set_clock(t);
    e.frame(t);
}

fn engine() -> Engine {
    let regions = serde_json::json!(["North", "South", "East"]);
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 520 },
        "keys": { "North": { "name": "The north" } },
        "signals": {
            "quarter": { "type": "str", "default": "q1" }, "labels": { "type": "bool", "default": true },
            "shown": { "type": "keyset", "default": regions }, "lo": { "type": "num", "default": 10 }, "hi": { "type": "num", "default": 40 },
            "region": { "type": "str", "default": "North" }
        },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows", "gap": 16, "padding": 10 }, "children": [
            { "kind": "use", "key": "quarter", "recipe": "@datars/std/segmented", "params": { "signal": "quarter", "options": ["q1", "q2"], "labels": ["Q1", "Q2"], "label": "Quarter" } },
            { "kind": "use", "key": "region", "recipe": "@datars/std/select", "params": { "signal": "region", "options": regions, "label": "Region" } },
            { "kind": "use", "key": "labels", "recipe": "@datars/std/toggle", "params": { "signal": "labels", "label": "Labels" } },
            { "kind": "use", "key": "shown", "recipe": "@datars/std/checklist", "params": { "signal": "shown", "options": regions } },
            { "kind": "use", "key": "range", "recipe": "@datars/std/range", "params": { "lo": "lo", "hi": "hi", "min": 0, "max": 50, "step": 1, "label": "Sales" } },
            { "kind": "use", "key": "reset", "recipe": "@datars/std/button", "params": { "label": "Show all", "set": "shown", "value": regions } },
            { "kind": "shape", "key": "chart", "size": { "h": 120 }, "geom": { "type": "rect", "x": 0, "y": 0, "w": "=box.w", "h": 120 }, "fill": "$surface",
              "semantics": { "role": "datum", "label": "chart" }, "on": { "activate": { "set": "region", "value": "clicked the chart" } } }
        ] }
    });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    settle(&mut e);
    e
}

fn item(e: &mut Engine, label: &str) -> SemanticItem {
    let items = e.semantic_items();
    items.iter().find(|i| i.label == label).cloned().unwrap_or_else(|| panic!("no item {label:?} in {:?}", items.iter().map(|i| &i.label).collect::<Vec<_>>()))
}

fn click(e: &mut Engine, x: f64, y: f64) {
    e.pointer(Pointer::Down { x, y });
    e.pointer(Pointer::Up { x, y });
    settle(e);
}

fn click_item(e: &mut Engine, label: &str) {
    let r = item(e, label).rect;
    click(e, r.x + r.w / 2.0, r.y + r.h / 2.0);
}

fn str_signal(e: &Engine, s: &str) -> String {
    match e.signal(s) {
        Some(Value::Str(v)) => v.to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn a_segmented_control_sets_its_signal_and_says_which_is_chosen() {
    let mut e = engine();
    assert!(item(&mut e, "Quarter: Q1, selected").actionable);
    click_item(&mut e, "Quarter: Q2");
    assert_eq!(str_signal(&e, "quarter"), "q2");
    assert!(e.semantic_items().iter().any(|i| i.label == "Quarter: Q2, selected"));
}

#[test]
fn a_select_opens_over_the_chart_chooses_and_closes() {
    let mut e = engine();
    // Closed: the box says the choice (by the key's name), and the list isn't there.
    item(&mut e, "Region: The north");
    assert!(!e.semantic_items().iter().any(|i| i.label == "East"));
    click_item(&mut e, "Region: The north");
    // Open: the list floats over what follows (the switch, the checkboxes, …): choosing East there
    // sets it and closes the list.
    let east = item(&mut e, "East").rect;
    assert!(east.y > item(&mut e, "Region: The north").rect.y, "the list opens below the box");
    click(&mut e, east.x + 20.0, east.y + east.h / 2.0);
    assert_eq!(str_signal(&e, "region"), "East");
    assert!(!e.semantic_items().iter().any(|i| i.label == "South"), "closed after choosing");
    // Open again, click the chart: the list closes and the chart doesn't take the click.
    click_item(&mut e, "Region: East");
    let chart = item(&mut e, "chart").rect;
    click(&mut e, chart.x + chart.w / 2.0, chart.y + chart.h / 2.0);
    assert_eq!(str_signal(&e, "region"), "East", "the click outside only closed the list");
    assert!(!e.semantic_items().iter().any(|i| i.label == "South"));
    // Open, click the chosen option: closes, choice kept.
    click_item(&mut e, "Region: East");
    click_item(&mut e, "East, selected");
    assert_eq!(str_signal(&e, "region"), "East");
    assert!(!e.semantic_items().iter().any(|i| i.label == "South"));
    // Now the chart takes clicks again.
    click(&mut e, chart.x + chart.w / 2.0, chart.y + chart.h / 2.0);
    assert_eq!(str_signal(&e, "region"), "clicked the chart");
}

#[test]
fn a_switch_flips_and_checkboxes_toggle_keys() {
    let mut e = engine();
    click_item(&mut e, "Labels: on");
    assert_eq!(e.signal("labels"), Some(Value::Bool(false)));
    click_item(&mut e, "Labels: off");
    assert_eq!(e.signal("labels"), Some(Value::Bool(true)));
    click_item(&mut e, "The north, checked");
    assert!(e.semantic_items().iter().any(|i| i.label == "The north, not checked"));
    click_item(&mut e, "Show all");
    assert!(e.semantic_items().iter().any(|i| i.label == "The north, checked"), "the button put it back");
}

#[test]
fn a_ranges_thumbs_move_their_ends_and_never_cross() {
    let mut e = engine();
    let lo = item(&mut e, "Sales from: 10").rect;
    let hi = item(&mut e, "Sales to: 40").rect;
    let y = lo.y + lo.h / 2.0;
    // Drag the low thumb far past the high one: it stops there.
    e.pointer(Pointer::Down { x: lo.x + lo.w / 2.0, y });
    e.pointer(Pointer::Move { x: hi.x + hi.w + 60.0, y });
    e.pointer(Pointer::Up { x: hi.x + hi.w + 60.0, y });
    settle(&mut e);
    assert_eq!(e.signal("lo"), Some(Value::Num(40.0)));
    // Both are offered as adjustable controls, each within where it can go.
    let cs = e.controls();
    let bounds = |s: &str| cs.iter().find(|c| c.signal == s).map(|c| (c.min, c.max)).unwrap_or_else(|| panic!("{s}: {cs:?}"));
    assert_eq!(bounds("lo"), (0.0, 40.0));
    assert_eq!(bounds("hi"), (40.0, 50.0));
}

#[test]
fn controls_show_no_tooltips() {
    let mut e = engine();
    for label in ["Quarter: Q2", "Region: The north", "Labels: on", "Show all"] {
        let r = item(&mut e, label).rect;
        assert_eq!(e.pointer(Pointer::Move { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 }), None, "{label}");
    }
    let chart = item(&mut e, "chart").rect;
    assert_eq!(e.pointer(Pointer::Move { x: chart.x + 10.0, y: chart.y + 10.0 }).as_deref(), Some("chart"), "data still does");
}

#[test]
fn a_select_is_offered_to_hosts_for_their_own_picker() {
    // A phone's host shows the platform's picker over the box: it needs the signal, each option's
    // value and what it says, the value now, and where the box is.
    let mut e = engine();
    let cs = e.controls();
    let sel = cs.iter().find(|c| c.kind == "select").unwrap_or_else(|| panic!("{cs:?}"));
    assert_eq!(sel.signal, "region");
    assert_eq!(sel.label, "Region: The north");
    let said: Vec<(&str, &str)> = sel.options.iter().map(|(v, l)| (v.as_str().unwrap(), l.as_str())).collect();
    assert_eq!(said, [("North", "The north"), ("South", "South"), ("East", "East")], "labels by the keys' names");
    assert_eq!(sel.current, serde_json::json!("North"));
    let boxed = item(&mut e, "Region: The north").rect;
    let [x, y, w, h] = sel.rect;
    assert!((x - boxed.x).abs() < 1.0 && (y - boxed.y).abs() < 1.0 && (w - boxed.w).abs() < 1.0 && (h - boxed.h).abs() < 1.0, "the box: {:?} vs {boxed:?}", sel.rect);
    // The picker's choice goes back as the signal.
    e.set_signal_json("region", &serde_json::json!("East"));
    settle(&mut e);
    let cs = e.controls();
    assert_eq!(cs.iter().find(|c| c.kind == "select").unwrap().current, serde_json::json!("East"));
    // Sliders stay sliders.
    assert!(cs.iter().filter(|c| c.kind == "slider").all(|c| c.options.is_empty() && c.min.is_finite()));
}

/// The opacity of the hover wash in the element at `path` (under the root) in the frame shown now.
fn wash(e: &mut Engine, path: &[&str]) -> f64 {
    let t = NOW.with(|n| n.get());
    let out = e.frame(t);
    let mut keys = vec![datars_scene::Key::name("root")];
    keys.extend(path.iter().map(|k| datars_scene::Key::name(*k)));
    out.scene.find(&datars_scene::KeyPath(keys)).map(|n| n.common.opacity).unwrap_or_else(|| panic!("no {path:?}"))
}

fn hover(e: &mut Engine, label: &str) {
    let r = item(e, label).rect;
    e.pointer(Pointer::Move { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 });
    settle(e);
}

#[test]
fn controls_show_where_the_pointer_is_and_take_a_pointing_hand() {
    use datars_engine::Cursor;
    let mut e = engine();
    let option = ["quarter", "option:q2", "hover"];
    assert_eq!(wash(&mut e, &option), 0.0);
    hover(&mut e, "Quarter: Q2");
    assert!(wash(&mut e, &option) > 0.0, "the option under the pointer is washed");
    assert_eq!(e.cursor(), Cursor::Pointer);
    // The chosen option isn't: it's already highlighted.
    hover(&mut e, "Quarter: Q1, selected");
    assert_eq!(wash(&mut e, &["quarter", "option:q1", "hover"]), 0.0);
    assert_eq!(wash(&mut e, &option), 0.0, "and the other has let go");

    hover(&mut e, "Show all");
    assert!(wash(&mut e, &["reset", "hover"]) > 0.0);

    // An open list: its options wash under the pointer; the veil over the rest of the chart closes
    // the list on a click but isn't something to point at.
    click_item(&mut e, "Region: The north");
    hover(&mut e, "South");
    assert!(wash(&mut e, &["region", "list", "option:South", "hover"]) > 0.0);
    assert_eq!(e.cursor(), Cursor::Pointer);
    let chart = item(&mut e, "chart").rect;
    e.pointer(Pointer::Move { x: chart.x + chart.w / 2.0, y: chart.y + chart.h / 2.0 });
    assert_eq!(e.cursor(), Cursor::Default, "over the veil");
    settle(&mut e);
    assert_eq!(wash(&mut e, &["region", "list", "option:South", "hover"]), 0.0);
}
