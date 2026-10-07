//! An editor's loop over a document's own recipe: each reload with edited source runs the edit (the
//! sandbox evaluated the first version once and would otherwise keep it), and a reload that keeps
//! the source re-expands nothing it already had.

use datars_engine::Engine;
use serde_json::json;

/// A one-recipe package: a bar `w` wide (`k` × the `n` param).
fn doc(k: u32, n: u32) -> datars_ir::Doc {
    let src = format!("export const bar = {{ __expand(p) {{ return {{ template: {{ kind: 'shape', key: 'bar', geom: {{ type: 'rect', x: 0, y: 0, w: p.n * {k}, h: 10 }}, fill: '$accent' }} }}; }} }};");
    let j = json!({
        "datars": 1, "size": { "width": 200, "height": 40 },
        "packages": [{ "name": "mypkg", "source": src }],
        "scene": { "kind": "use", "recipe": "mypkg/bar", "params": { "n": n } }
    });
    datars_ir::Doc::from_json(&j.to_string()).unwrap()
}

fn width(e: &mut Engine) -> String {
    let s = e.scene().snapshot();
    s.lines().find(|l| l.contains("rect")).unwrap_or_default().to_string()
}

#[test]
fn an_edited_recipe_runs_on_the_next_reload() {
    let mut e = Engine::new();
    assert!(e.load(doc(2, 10)).is_empty());
    let first = width(&mut e);
    assert!(first.contains("20"), "{first}");
    // The recipe edited: twice as wide.
    assert!(e.load(doc(4, 10)).is_empty());
    let edited = width(&mut e);
    assert!(edited.contains("40"), "the edit runs: {edited}");
    // Back to the first version, and a new param with the same source.
    e.load(doc(2, 10));
    assert!(width(&mut e).contains("20"));
    e.load(doc(2, 30));
    assert!(width(&mut e).contains("60"));
}
