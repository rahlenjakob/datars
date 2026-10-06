//! Layout table ops over geography.

use datars_engine::Engine;

#[test]
fn scatter_in_fills_the_largest_polygon() {
    // A region whose first polygon is a small island: the dots go to the mainland.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 300 },
        "data": {
            "regions": { "url": "regions.geojson", "id": "name" },
            "counts": { "values": { "name": ["A"], "n": [25] }, "key": ["name"] }
        },
        "tables": { "dots": { "from": "counts", "ops": [{ "op": "scatter-in", "geo": "regions", "count": "=d.n", "seed": 7 }] } },
        "scene": { "kind": "repeat", "key": "dots", "from": "dots", "template": { "kind": "shape",
            "geom": { "type": "circle", "cx": "=d.lon", "cy": "=d.lat", "r": 1 }, "fill": "$accent" } }
    });
    let square = |x: f64, y: f64, s: f64| serde_json::json!([[[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]]);
    let geo = serde_json::json!({ "type": "FeatureCollection", "features": [{ "type": "Feature", "properties": { "name": "A" },
        "geometry": { "type": "MultiPolygon", "coordinates": [square(0.0, 0.0, 1.0), square(10.0, 10.0, 10.0)] } }] });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.provide("regions", geo.to_string().as_bytes()).unwrap();
    let snap = e.scene().snapshot();
    let centres: Vec<(f64, f64)> = snap
        .lines()
        .filter_map(|l| l.split_once("c=(")?.1.split_once(')').map(|(c, _)| c.to_string()))
        .filter_map(|c| c.split_once(", ").map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())))
        .collect();
    assert_eq!(centres.len(), 25, "{snap}");
    assert!(centres.iter().all(|&(x, y)| (10.0..=20.0).contains(&x) && (10.0..=20.0).contains(&y)), "all on the mainland: {centres:?}");
}

/// A GeoJSON of unit squares named `A` and `B`, side by side.
fn two_squares() -> String {
    let square = |x: f64| serde_json::json!([[[x, 0.0], [x + 10.0, 0.0], [x + 10.0, 10.0], [x, 10.0], [x, 0.0]]]);
    let feature = |name: &str, x: f64| serde_json::json!({ "type": "Feature", "properties": { "name": name }, "geometry": { "type": "Polygon", "coordinates": square(x) } });
    serde_json::json!({ "type": "FeatureCollection", "features": [feature("A", 0.0), feature("B", 20.0)] }).to_string()
}

fn dots_snapshot(counts: serde_json::Value, op: serde_json::Value) -> String {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 300 },
        "data": { "regions": { "url": "regions.geojson", "id": "name" }, "counts": counts },
        "tables": { "dots": { "from": "counts", "ops": [op] } },
        "scene": { "kind": "repeat", "key": "dots", "from": "dots", "template": { "kind": "shape",
            "geom": { "type": "circle", "cx": "=d.lon", "cy": "=d.lat", "r": 1 }, "fill": "$accent" } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    e.provide("regions", two_squares().as_bytes()).unwrap();
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    e.scene().snapshot()
}

#[test]
fn scatter_in_keys_dots_by_region_and_index() {
    // A region column called `id` (it used to clash with the dots' own id column), and the dots
    // keyed (region, i): each is one of its region's dots, whatever the other regions do.
    let snap = dots_snapshot(serde_json::json!({ "values": { "id": ["A", "B"], "n": [3, 2] }, "key": ["id"] }), serde_json::json!({ "op": "scatter-in", "geo": "regions", "count": "=d.n" }));
    for k in ["(\"A\", 0)", "(\"A\", 2)", "(\"B\", 1)"] {
        assert!(snap.contains(k), "{k} in {snap}");
    }
    assert_eq!(snap.lines().filter(|l| l.contains(" ellipse ")).count(), 5, "{snap}");
    // `key` names the feature-id column when it isn't the table's key.
    let snap = dots_snapshot(
        serde_json::json!({ "values": { "row": [1, 2], "region": ["B", "A"], "n": [2, 1] }, "key": ["row"] }),
        serde_json::json!({ "op": "scatter-in", "geo": "regions", "count": "=d.n", "key": "region" }),
    );
    assert!(snap.contains("(\"B\", 1)") && snap.contains("(\"A\", 0)") && !snap.contains("(\"A\", 1)"), "{snap}");
    // B's dots sit in B's square (x 20…30), A's in A's (0…10).
    for l in snap.lines().filter(|l| l.contains(" ellipse ")) {
        let x: f64 = l.split("c=(").nth(1).and_then(|r| r.split(',').next()).and_then(|x| x.parse().ok()).unwrap();
        assert!(if l.contains("(\"B\",") { (20.0..=30.0).contains(&x) } else { (0.0..=10.0).contains(&x) }, "{l}");
    }
}

#[test]
fn calendar_numbers_its_year_panels() {
    // Days across three years: a panel per year from the first (cx/cy are panel-local).
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 300, "height": 300 },
        "data": { "days": { "values": { "date": ["2023-12-31", "2024-01-01", "2025-06-30"], "n": [1, 2, 3] }, "key": ["date"], "types": { "date": "date" } } },
        "tables": { "cal": { "from": "days", "ops": [{ "op": "calendar", "date": "date", "cell": 10 }] } },
        "scene": { "kind": "repeat", "key": "cal", "from": "cal", "template": { "kind": "text", "text": "=`${d.panel}:${d.cx}:${d.cy}`", "at": [0, 0] } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let snap = e.scene().snapshot();
    let texts: Vec<&str> = snap.lines().filter_map(|l| l.split("text \"").nth(1)?.split('"').next()).collect();
    // 31 Dec 2023 was a Sunday (week 52 of 2023); 1 Jan 2024 a Monday, the first cell of 2024;
    // 30 Jun 2025 a Monday in 2025's panel.
    assert_eq!(texts, vec!["0:520:60", "1:0:0", "2:260:0"], "{snap}");
}
