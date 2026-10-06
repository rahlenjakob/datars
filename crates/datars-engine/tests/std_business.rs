//! The table ops dashboards lean on — rows written into an op (`values`) and 1-D lane packing
//! (`lanes`) — and the standard library's KPI, table, timeline and tile-map recipes through the
//! engine, where what matters is the resolved scene: every tile drawn and keyed, labels that don't
//! collide, rows placed by their sort.

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

fn doc(size: (u32, u32), data: Value, scene: Value) -> Value {
    json!({ "datars": 1, "size": { "width": size.0, "height": size.1 }, "data": data, "scene": scene })
}

/// A text's snapshot line by its content.
fn text_line<'a>(snap: &'a str, text: &str) -> Option<&'a str> {
    snap.lines().find(|l| l.contains(&format!("text \"{text}\"")))
}

/// The `at (x, y)` of a text line.
fn at(line: &str) -> (f64, f64) {
    let r = line.split(" at (").nth(1).and_then(|r| r.split(')').next()).unwrap_or_default();
    let (x, y) = r.split_once(", ").unwrap_or(("nan", "nan"));
    (x.parse().unwrap_or(f64::NAN), y.parse().unwrap_or(f64::NAN))
}

#[test]
fn values_replaces_the_rows_and_takes_a_key() {
    // A lookup table written into the op, then the document's data joined onto it: rows without
    // data stay (a tile with no value), keyed by the op's key.
    let d = json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "v": { "values": { "id": ["b"], "n": [7] }, "key": ["id"] } },
        "tables": { "grid": { "from": "v", "ops": [
            { "op": "values", "values": { "id": ["a", "b", "c"], "col": [0, 1, 2] }, "key": ["id"] },
            { "op": "join", "with": "v", "on": ["id"], "kind": "left" },
        ] } },
        "scene": { "kind": "repeat", "from": "grid", "template": { "kind": "text", "text": "=d.id + ':' + (d.n ?? '-')", "at": ["=d.col * 50", 20] } },
    });
    let s = snapshot(d);
    assert!(s.contains("(\"a\",) text \"a:-\"") && s.contains("(\"b\",) text \"b:7\"") && s.contains("(\"c\",) text \"c:-\""), "{s}");
    // Records work too, and a key that isn't unique is refused (with the op named).
    let bad = json!({
        "datars": 1, "size": { "width": 200, "height": 100 },
        "data": { "v": { "values": { "id": ["b"] }, "key": ["id"] } },
        "tables": { "t": { "from": "v", "ops": [{ "op": "values", "values": [{ "id": "a" }, { "id": "a" }], "key": ["id"] }] } },
        "scene": { "kind": "repeat", "from": "t", "template": { "kind": "text", "text": "=d.id", "at": [0, 0] } },
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&bad.to_string()).unwrap());
    let _ = e.scene();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("op values")), "{:?}", e.diagnostics());
}

#[test]
fn lanes_packs_intervals_and_leaves_out_what_the_cap_refuses() {
    let d = json!({
        "datars": 1, "size": { "width": 400, "height": 100 },
        "data": { "e": { "values": { "k": ["a", "b", "c", "d"], "x": [0, 50, 90, 300] }, "key": ["k"] } },
        "tables": { "l": { "from": "e", "ops": [{ "op": "lanes", "start": "=d.x", "end": "=d.x + 100", "gap": 5, "max": 2 }] } },
        "scene": { "kind": "repeat", "from": "l", "template": { "kind": "text", "text": "=d.k + (d.lane == null ? ' out' : ' ' + d.lane)", "at": ["=d.x", 10] } },
    });
    let s = snapshot(d);
    // a [0,100] lane 0; b [50,150] overlaps: lane 1; c [90,190]: no lane free, the cap is 2: out;
    // d [300,400]: back in lane 0.
    for t in ["a 0", "b 1", "c out", "d 0"] {
        assert!(text_line(&s, t).is_some(), "{t}: {s}");
    }
}

#[test]
fn a_tile_map_draws_every_state_keyed_by_its_code() {
    let data = json!({ "u": { "values": { "state": ["CA", "TX", "NY"], "rate": [4.8, 4.0, 4.2] }, "key": ["state"] } });
    let s = snapshot(doc((640, 400), data, use_("tileMap", json!({ "data": "u", "key": "state", "value": "rate" }))));
    let tiles = s.lines().filter(|l| l.contains("role=region")).count();
    assert_eq!(tiles, 51, "fifty states and DC: {s}");
    assert!(s.contains("group (\"CA\",)") && s.contains("group (\"WY\",)"), "keyed by code: {s}");
    // Wyoming has no data: neutral, and said so.
    let wy = s.lines().skip_while(|l| !l.contains("group (\"WY\",)")).nth(1).unwrap();
    assert!(wy.contains("$map.no-data") && wy.contains("Wyoming: no data"), "{wy}");
    // Alaska sits top-left of California, Florida right of Texas.
    let (ak, ca) = (at(text_line(&s, "AK").unwrap()), at(text_line(&s, "CA").unwrap()));
    let (tx, fl) = (at(text_line(&s, "TX").unwrap()), at(text_line(&s, "FL").unwrap()));
    assert!(ak.1 < ca.1 && (ak.0 - ca.0).abs() < 1.0 && fl.0 > tx.0, "{ak:?} {ca:?} {tx:?} {fl:?}");
}

#[test]
fn timeline_labels_never_overlap_each_other() {
    // Six events a year apart on a narrow box: their labels can't all sit side by side.
    let data = json!({ "e": { "values": { "y": [2000, 2001, 2002, 2003, 2004, 2010], "what": ["Event 1", "Event 2", "Event 3", "Event 4", "Event 5", "Event 6"] }, "key": ["y"] } });
    let s = snapshot(doc((360, 400), data, use_("timeline", json!({ "data": "e", "date": "y", "label": "what", "xType": "linear" }))));
    let labels: Vec<(f64, f64, f64)> = (1..=6)
        .map(|w| {
            let l = text_line(&s, &format!("Event {w}")).unwrap_or_else(|| panic!("{w}: {s}"));
            let (x, y) = at(l);
            // A label left of its dot (near the right edge) runs leftwards from it.
            let dot = s.lines().find(|d| d.contains("role=datum") && d.contains(&format!(": Event {w}"))).unwrap();
            let dx: f64 = dot.split("c=(").nth(1).and_then(|c| c.split(',').next()).and_then(|v| v.parse().ok()).unwrap();
            let right = x < dx;
            // An interval along x (a label is about 44 px wide: 50 is a safe bound) and its line.
            (if right { x - 50.0 } else { x }, if right { x } else { x + 50.0 }, y)
        })
        .collect();
    assert!(labels.iter().any(|l| (l.2 - labels[0].2).abs() > 1.0), "they don't all fit one line: {labels:?}");
    for (i, a) in labels.iter().enumerate() {
        for b in &labels[i + 1..] {
            let same_line = (a.2 - b.2).abs() < 1.0;
            assert!(!(same_line && a.0 < b.1 && b.0 < a.1), "two labels on one line overlap: {a:?} {b:?}\n{s}");
        }
    }
}

#[test]
fn a_table_places_rows_by_their_sort() {
    let data = json!({ "t": { "values": { "shop": ["A", "B", "C"], "sales": [10, 30, 20] }, "key": ["shop"] } });
    let cols = json!([{ "field": "shop" }, { "field": "sales", "format": ",.0f" }]);
    let s = snapshot(doc((400, 200), data, use_("dataTable", json!({ "data": "t", "key": "shop", "columns": cols, "sort": "sales" }))));
    let y = |k: &str| {
        let l = s.lines().find(|l| l.contains(&format!("group (\"{k}\",)"))).unwrap_or_else(|| panic!("{k}: {s}"));
        l.split("transform=[").nth(1).and_then(|t| t.split(']').next()).and_then(|t| t.split(' ').nth(5)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)
    };
    assert!(y("B") < y("C") && y("C") < y("A"), "largest first: B {} C {} A {}\n{s}", y("B"), y("C"), y("A"));
    assert!(s.contains("role=datum \"shop: A, sales: 10\""), "a row reads as its cells: {s}");
}

#[test]
fn a_kpi_reads_its_value_and_change() {
    let data = json!({ "m": { "values": { "month": ["2026-07-01", "2026-08-01", "2026-09-01"], "orders": [900, 1000, 1100] }, "key": ["month"], "types": { "month": "date" } } });
    let s = snapshot(doc((300, 160), data, use_("kpi", json!({ "label": "Orders", "data": "m", "x": "month", "y": "orders", "compareLabel": "vs August" }))));
    assert!(s.contains("role=datum \"Orders: 1,100, up 10.0% vs August\""), "{s}");
    let delta = text_line(&s, "+10.0%").unwrap_or_else(|| panic!("{s}"));
    assert!(delta.contains("$positive"), "{delta}");
}
