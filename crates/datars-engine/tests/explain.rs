//! `explain`: why an element exists and looks the way it does.

use datars_engine::Engine;

#[test]
fn a_bar_is_explained_by_its_recipes_row_and_expressions() {
    let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/votes/doc.json")).unwrap();
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&json).unwrap());
    let found = e.explain("(\"S\",)");
    // The bar, its value label and the axis tick — each once, as drawn (not the layout's
    // measuring passes).
    assert_eq!(found.len(), 3, "{:#?}", found.iter().map(|f| &f.origin.path).collect::<Vec<_>>());
    let bar = found.iter().find(|f| f.origin.path.ends_with("(\"marks\",)/(\"S\",)")).expect("the bar");
    assert!(bar.node.contains("rect"), "{}", bar.node);
    assert_eq!(bar.origin.recipes, ["@datars/std/plot", "@datars/std/bar"]);
    let row = bar.origin.row.as_ref().expect("drawn for a row");
    assert_eq!(row.table, "votes");
    assert_eq!(row.fields["party"], "S");
    let fill = bar.origin.values.iter().find(|v| v.field == "fill").expect("the fill expression");
    assert_eq!(fill.expr, "scale.color(d.party)");
    assert!(fill.value.as_str().is_some_and(|c| bar.node.contains(&format!("fill={c}"))), "the value is what's drawn: {} vs {}", fill.value, bar.node);
    assert!(bar.bounds.is_some_and(|b| b[3] > 100.0), "a tall bar: {:?}", bar.bounds);

    // A full path finds exactly one; unknown keys find none; ordinary resolution records nothing.
    assert_eq!(e.explain(&bar.origin.path).len(), 1);
    assert!(e.explain("(\"nope\",)").is_empty());
}

/// One instance of an `instances` node (a custom scatter, a dot cloud) explains like a repeat's
/// mark: its row and every expression's value for that row, outlined where it's drawn. Instances
/// share one node, so they once had no origin: the dev page's Explain tab did nothing on a dot.
#[test]
fn an_instance_is_explained_by_its_row_and_expressions() {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "pts": { "values": { "id": ["a", "b", "c"], "v": [10, 20, 30] }, "key": ["id"] } },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "instances", "key": "dots", "from": "pts", "x": "=d.v * 5", "y": 50, "r": 4,
              "fill": "=d.v > 15 ? '$accent' : '$ink'", "semantics": { "role": "datum" } }] } });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e.frame(0.0);

    // The path the hit test reports for the dot at x = 100 is the one the dev page asks about.
    let hit = e.hit_test(100.0, 50.0).into_iter().find(|h| h.kind == "instance").expect("the dot under the pointer");
    let found = e.explain(&hit.path);
    assert_eq!(found.len(), 1, "{:#?}", found.iter().map(|f| &f.origin.path).collect::<Vec<_>>());
    let dot = &found[0];
    assert_eq!(dot.origin.path, hit.path);
    let row = dot.origin.row.as_ref().expect("drawn for a row");
    assert_eq!((row.table.as_str(), row.index), ("pts", 1));
    assert_eq!(row.fields["id"], "b");
    let x = dot.origin.values.iter().find(|v| v.field == "x").expect("the x expression");
    assert_eq!((x.expr.as_str(), x.value.as_f64()), ("d.v * 5", Some(100.0)));
    let fill = dot.origin.values.iter().find(|v| v.field == "fill").expect("the fill expression");
    assert_eq!(fill.value, "$accent");
    let b = dot.bounds.expect("outlined where it's drawn");
    assert!((b[0] + b[2] / 2.0 - 100.0).abs() < 0.5 && (b[1] + b[3] / 2.0 - 50.0).abs() < 0.5, "{b:?}");

    // Its key alone finds it too (as `datars explain --key` asks); the node itself is still one
    // explanation, not one per instance.
    assert_eq!(e.explain("(\"b\",)").len(), 1);
    let node = e.explain("(\"root\",)/(\"dots\",)");
    assert_eq!(node.len(), 1);
    assert!(node[0].origin.row.is_none());
}
