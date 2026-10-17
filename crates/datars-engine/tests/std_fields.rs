//! Columns whose names aren't identifiers — a spreadsheet's headers, a pasted table's: "Avg.
//! nightly rate", "Share (%)" — are read by the standard library's recipes like any other: the
//! marks are drawn from them and their semantics say their values. (Recipes built `d.${name}`,
//! and `d.Avg. nightly rate` reads nothing.)

use datars_engine::Engine;
use serde_json::json;

const RATE: &str = "Avg. nightly rate";

fn engine(doc: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);
    e
}

fn use_(recipe: &str, params: serde_json::Value) -> serde_json::Value {
    json!({ "kind": "use", "recipe": format!("@datars/std/{recipe}"), "params": params })
}

/// Three cities' rates (and a second column with a non-identifier name), the scene given.
fn doc(scene: serde_json::Value) -> serde_json::Value {
    json!({ "datars": 1, "size": { "width": 600, "height": 360 },
        "data": { "t": { "values": { "City": ["Lisbon", "Porto", "Faro"], RATE: [120, 95, 140], "Share (%)": [50, 30, 20], "Night": [1, 2, 3] }, "key": ["City"] } },
        "scene": scene })
}

/// The data marks' labels.
fn data_labels(e: &mut Engine) -> Vec<String> {
    e.semantic_items().into_iter().filter(|i| i.role == "datum").map(|i| i.label).collect()
}

#[test]
fn bars_lines_points_stacks_and_donuts_read_a_column_named_avg_nightly_rate() {
    let charts = [
        ("bar", use_("plot", json!({ "data": "t", "x": "City", "y": RATE, "children": [use_("bar", json!({}))] }))),
        ("line", use_("plot", json!({ "data": "t", "x": "Night", "y": RATE, "xType": "linear", "children": [use_("line", json!({}))] }))),
        ("point", use_("plot", json!({ "data": "t", "x": "Share (%)", "y": RATE, "xType": "linear", "children": [use_("point", json!({}))] }))),
        ("stacked", use_("stacked", json!({ "data": "t", "x": "City", "y": RATE }))),
        ("donut", use_("pie", json!({ "data": "t", "category": "City", "value": RATE, "inner": 0.58 }))),
    ];
    for (name, scene) in charts {
        let mut e = engine(doc(scene));
        let labels = data_labels(&mut e);
        assert_eq!(labels.len(), 3, "{name}: a mark per city, {labels:?}");
        for v in ["120", "95", "140"] {
            assert!(labels.iter().any(|l| l.contains(v)), "{name}: the rate {v} is read: {labels:?}");
        }
        // And drawn where the values put them: no mark collapsed onto zero.
        let summary = e.accessible_summary();
        assert!(summary.starts_with("3 data points"), "{name}: {summary}");
    }
}

const SQUARES: &str = r#"{"type":"FeatureCollection","features":[
  {"type":"Feature","id":"A","properties":{},"geometry":{"type":"Polygon","coordinates":[[[0,0],[1,0],[1,1],[0,1],[0,0]]]}},
  {"type":"Feature","id":"B","properties":{},"geometry":{"type":"Polygon","coordinates":[[[10,10],[11,10],[11,11],[10,11],[10,10]]]}}]}"#;

#[test]
fn a_map_colours_its_regions_from_a_column_named_avg_nightly_rate() {
    let mut e = engine(json!({ "datars": 1, "size": { "width": 300, "height": 300 },
        "data": { "g": { "geojson": serde_json::from_str::<serde_json::Value>(SQUARES).unwrap() },
                  "vals": { "values": { "Region code": ["A", "B"], RATE: [80, 160] }, "key": ["Region code"] } },
        "scene": use_("map", json!({ "source": "g", "data": "vals", "key": "Region code", "value": RATE, "backdrop": false })) }));
    let regions: Vec<(String, Option<f64>)> = e.semantic_items().into_iter().filter(|i| i.role == "region").map(|i| (i.label, i.value)).collect();
    assert_eq!(regions.len(), 2, "{regions:?}");
    assert!(regions.iter().all(|(_, v)| v.is_some()), "each region has its rate: {regions:?}");
    assert!(regions.iter().any(|(l, _)| l.contains("80")) && regions.iter().any(|(l, _)| l.contains("160")), "{regions:?}");
    // Coloured by it: the two regions don't share one fill (a rate of nothing would).
    let snap = e.scene().snapshot();
    let fills: std::collections::BTreeSet<String> = snap.lines().filter(|l| l.contains("role=region")).filter_map(|l| l.split("fill=").nth(1).map(|f| f.split(' ').next().unwrap_or("").to_string())).collect();
    assert_eq!(fills.len(), 2, "{fills:?}\n{snap}");
}
