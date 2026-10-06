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
