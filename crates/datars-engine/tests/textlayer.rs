//! The text layer: every text a frame draws, where it draws it — the boxes a web page lays real,
//! selectable text in over the canvas. Checked against the layout the glyphs were drawn from (and
//! against `hit_test`'s bounds), for plain, wrapped, turned, faded, clipped and draggable text.

use datars_engine::textlayer::TextOut;
use datars_engine::Engine;

/// A header (title, a wrapped note, a turned label, a faded one) over an explorable view.
fn engine() -> Engine {
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 400, "height": 300 },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows" }, "children": [
            { "kind": "group", "key": "head", "size": { "h": 100 }, "children": [
                { "kind": "text", "key": "title", "text": "Selectable chart title", "at": [20, 30], "style": { "size": 20, "weight": 600 },
                  "semantics": { "role": "title", "label": "Selectable chart title" } },
                { "kind": "text", "key": "note", "text": "one two three four five six", "at": [20, 40], "style": { "size": 12, "max_width": 70, "baseline": "top" } },
                { "kind": "text", "key": "side", "text": "Turned", "at": [380, 50], "rotate": -90, "style": { "size": 12, "align": "middle" } },
                { "kind": "text", "key": "gone", "text": "Faded", "at": [200, 90], "opacity": 0 }] },
            { "kind": "group", "key": "stage", "size": { "h": 200 }, "children": [
                { "kind": "view", "key": "map", "camera": { "fit": { "bbox": [0, 0, 100, 50] }, "padding": 0, "explore": "cam" }, "children": [
                    { "kind": "text", "key": "town", "text": "Borlänge", "at": [50, 25], "style": { "size": 12, "align": "middle", "baseline": "middle" } },
                    { "kind": "text", "key": "far", "text": "Far away", "at": [500, 25] }] },
                { "kind": "text", "key": "caption", "text": "Over the map", "at": [10, 20] }] }] } });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn texts(e: &mut Engine) -> Vec<TextOut> {
    let frame = e.frame(0.0);
    e.text_layer(&frame.scene)
}

fn find<'a>(ts: &'a [TextOut], key: &str) -> Option<&'a TextOut> {
    ts.iter().find(|t| t.path.ends_with(&format!("(\"{key}\",)")))
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 2e-3
}

#[test]
fn every_readable_text_in_tree_order_and_nothing_else() {
    let ts = texts(&mut engine());
    let keys: Vec<&str> = ts.iter().map(|t| t.path.rsplit('/').next().unwrap()).collect();
    assert_eq!(keys, ["(\"title\",)", "(\"note\",)", "(\"side\",)", "(\"town\",)", "(\"caption\",)"], "a faded text and one clipped away by its view aren't there");
    assert_eq!(ts[0].path, "(\"root\",)/(\"head\",)/(\"title\",)", "key paths as hit_test and explain spell them");
    assert!(ts.iter().all(|t| !t.place), "authored text, not a map's place names");
}

#[test]
fn a_line_box_is_where_the_glyphs_are_drawn() {
    let mut e = engine();
    let ts = texts(&mut e);
    let title = find(&ts, "title").unwrap();
    assert_eq!((title.text.as_str(), title.role.as_deref()), ("Selectable chart title", Some("title")));
    assert_eq!((title.family.as_str(), title.weight, title.faces.clone()), ("Inter", 600, vec!["Inter-600".to_string()]));
    assert_eq!((title.size, title.rotate, title.opacity), (20.0, 0.0, 1.0));
    let [l] = title.lines.as_slice() else { panic!("{:?}", title.lines) };
    assert_eq!(l.text, "Selectable chart title");
    assert!(close(l.x, 20.0) && close(l.y + l.baseline, 30.0), "starts at its origin, on its baseline: {l:?}");
    // The same box an editor's hit test reports for the drawn text.
    let hit = e.hit_test(30.0, 25.0).into_iter().find(|h| h.kind == "text").expect("the title under the pointer");
    for (got, want) in [l.x, l.y, l.w, l.h].into_iter().zip(hit.bounds) {
        assert!(close(got, want), "{l:?} vs {:?}", hit.bounds);
    }
    assert_eq!(title.bounds, [l.x, l.y, l.w, l.h]);
    // Many points at once (a host placing a card): what the one-point test says at each.
    let pts = [datars_math::Vec2::new(30.0, 25.0), datars_math::Vec2::new(5.0, 5.0), datars_math::Vec2::new(200.0, 100.0)];
    let many = e.hit_test_many(&pts);
    for (p, hits) in pts.iter().zip(&many) {
        assert_eq!(hits.iter().map(|h| &h.path).collect::<Vec<_>>(), e.hit_test(p.x, p.y).iter().map(|h| &h.path).collect::<Vec<_>>());
    }
    assert!(l.w > 150.0 && l.w < 260.0 && l.h > 22.0 && l.h < 26.0 && l.baseline > 0.75 * l.h, "20 px Inter, ascent over descent: {l:?}");
}

#[test]
fn a_wrapped_text_has_a_box_per_line() {
    let ts = texts(&mut engine());
    let note = find(&ts, "note").unwrap();
    assert!(note.lines.len() >= 3, "{:?}", note.lines);
    let words: Vec<&str> = note.lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(words.join(" "), "one two three four five six", "the wraps keep every word, and only the words");
    assert!(note.lines.iter().all(|l| !l.br), "wraps, not line breaks");
    assert!(close(note.lines[0].y, 40.0), "baseline top: the first box starts at the origin");
    for w in note.lines.windows(2) {
        assert!(close(w[1].y - w[0].y, 12.0 * 1.25) && close(w[1].x, 20.0), "one line height apart, each starting at the origin: {w:?}");
    }
    assert!(note.lines.iter().all(|l| l.w <= 70.0 + 1e-6), "within its max width");
}

#[test]
fn a_turned_text_turns_about_its_origin() {
    let ts = texts(&mut engine());
    let side = find(&ts, "side").unwrap();
    assert!(close(side.rotate, -std::f64::consts::FRAC_PI_2));
    let [l] = side.lines.as_slice() else { panic!() };
    // Centred on (380, 50), then turned −90°: the box's top-left corner (−w/2, −ascent) lands at
    // (380 − ascent, 50 + w/2); its width runs up the screen, its height to the right.
    assert!(close(l.x, 380.0 - l.baseline) && close(l.y, 50.0 + l.w / 2.0), "{l:?}");
    let [x, y, w, h] = side.bounds;
    assert!(close(w, l.h) && close(h, l.w) && close(x, l.x) && close(y + h, l.y), "upright bounds of the turned box: {:?}", side.bounds);
}

#[test]
fn text_in_a_draggable_view_leaves_the_pointer_to_the_chart() {
    let ts = texts(&mut engine());
    let town = find(&ts, "town").unwrap();
    assert!(town.drag, "a press on a place name in an explorable view pans the view");
    assert!(ts.iter().filter(|t| !t.path.contains("map")).all(|t| !t.drag), "text outside it is text to select");
    let caption = find(&ts, "caption").unwrap();
    let [x, y, ..] = caption.bounds;
    assert!(y > 100.0 && x < 400.0 && !caption.drag, "text drawn over the view, not in it (a title over a full-bleed map), is text to select: {:?}", caption.bounds);
    // Placed through the camera (×4: 100 × 50 units into 400 × 200 px, the view 100 px down),
    // centred on (50, 25); drawn at screen size, as labels under a camera are.
    let [x, y, w, h] = town.bounds;
    assert!(close(x + w / 2.0, 200.0) && close(y + h / 2.0, 200.0), "{:?}", town.bounds);
    assert_eq!(town.size, 12.0);
}

#[test]
fn the_layer_is_the_same_frame_after_frame() {
    let mut e = engine();
    let a = serde_json::to_string(&texts(&mut e)).unwrap();
    let b = serde_json::to_string(&texts(&mut e)).unwrap();
    assert_eq!(a, b, "a host compares layers to know when to rebuild");
}

#[test]
fn a_controls_label_leaves_the_pointer_to_the_chart() {
    // A checkbox's label is part of the checkbox: a click on it toggles, it doesn't start a
    // selection. A title beside it is text to select.
    let doc = serde_json::json!({ "datars": 1, "size": { "width": 300, "height": 80 },
        "signals": { "shown": { "type": "keyset", "default": ["a"] } },
        "scene": { "kind": "group", "key": "root", "layout": { "type": "rows" }, "children": [
            { "kind": "text", "key": "title", "size": { "h": "auto" }, "text": "A title", "at": [0, 0], "style": { "baseline": "top" } },
            { "kind": "use", "key": "pick", "recipe": "@datars/std/checklist", "params": { "signal": "shown", "options": ["a", "b"] } }] } });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let ts = texts(&mut e);
    let label = ts.iter().find(|t| t.text == "a").expect("the option's label");
    assert!(label.drag, "{label:?}");
    assert!(!find(&ts, "title").unwrap().drag);
}
