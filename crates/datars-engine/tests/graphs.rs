//! Graph and hierarchy table ops run through documents: force, communities and lookup over a
//! nodes table and a links table; tree and partition over a parent-child table; chord groups and
//! ribbons over a links table.

use datars_engine::Engine;

/// Loads a document whose scene writes one text per row of `table` (the expression `text`), and
/// returns those texts in row order.
fn texts(data: serde_json::Value, tables: serde_json::Value, table: &str, text: &str) -> Vec<String> {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 300 },
        "data": data, "tables": tables,
        "scene": { "kind": "repeat", "key": "rows", "from": table, "template": { "kind": "text", "text": format!("={text}"), "at": [0, 0] } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    assert!(e.diagnostics().is_empty(), "{:?}", e.diagnostics());
    let snap = e.scene().snapshot();
    snap.lines().filter_map(|l| Some(l.split("text \"").nth(1)?.split('"').next()?.to_string())).collect()
}

fn nums(s: &str) -> Vec<f64> {
    s.split(':').map(|x| x.parse::<f64>().unwrap_or(f64::NAN)).collect()
}

fn graph() -> serde_json::Value {
    // Two triangles joined by one link, and a loner; one link names a node that isn't there.
    serde_json::json!({
        "nodes": { "values": { "id": ["a", "b", "c", "d", "e", "f", "g"], "size": [9, 4, 4, 9, 4, 4, 2] }, "key": ["id"] },
        "links": { "values": { "s": ["a", "a", "b", "d", "d", "e", "c", "a"], "t": ["b", "c", "c", "e", "f", "f", "d", "zz"], "w": [1, 1, 1, 2, 2, 2, 1, 5] } }
    })
}

#[test]
fn force_lays_out_nodes_inside_the_box_and_lookup_finds_link_ends() {
    let tables = serde_json::json!({
        "laid": { "from": "nodes", "ops": [{ "op": "force", "links": "links", "source": "s", "target": "t", "weight": "w", "radius": "=d.size", "width": 400, "height": 300, "distance": 40, "charge": -80 }] },
        "ends": { "from": "links", "ops": [
            { "op": "lookup", "from": "laid", "key": "id", "field": "s", "values": ["x", "y"], "as": ["x1", "y1"] },
            { "op": "lookup", "from": "laid", "field": "t", "values": ["x", "y"], "as": ["x2", "y2"] }
        ] }
    });
    let nodes = texts(graph(), tables.clone(), "laid", "`${d.x}:${d.y}:${d.size}:${d.degree}`");
    assert_eq!(nodes.len(), 7);
    let pts: Vec<Vec<f64>> = nodes.iter().map(|s| nums(s)).collect();
    for p in &pts {
        let (x, y, r) = (p[0], p[1], p[2]);
        assert!(x - r >= -1e-6 && x + r <= 400.0 + 1e-6 && y - r >= -1e-6 && y + r <= 300.0 + 1e-6, "inside the box: {p:?}");
    }
    for i in 0..7 {
        for j in i + 1..7 {
            let d = ((pts[i][0] - pts[j][0]).powi(2) + (pts[i][1] - pts[j][1]).powi(2)).sqrt();
            assert!(d > pts[i][2] + pts[j][2] - 0.5, "nodes {i} and {j} overlap");
        }
    }
    // Degrees count the weights of known links only (a → zz is left out).
    assert_eq!(pts.iter().map(|p| p[3]).collect::<Vec<_>>(), vec![2.0, 2.0, 3.0, 5.0, 4.0, 4.0, 0.0]);
    // Deterministic: the same document, the same layout.
    assert_eq!(texts(graph(), tables.clone(), "laid", "`${d.x}:${d.y}:${d.size}:${d.degree}`"), nodes);
    // Each link row gets its ends' positions; an unknown end is null.
    let ends = texts(graph(), tables, "ends", "`${d.x1}:${d.y1}:${d.x2}:${d.y2}`");
    let first = nums(&ends[0]);
    assert_eq!((first[0], first[1], first[2], first[3]), (pts[0][0], pts[0][1], pts[1][0], pts[1][1]), "a → b");
    assert!(nums(&ends[7])[2].is_nan(), "a → zz has no target position: {}", ends[7]);
}

#[test]
fn communities_find_the_two_triangles() {
    let tables = serde_json::json!({ "c": { "from": "nodes", "ops": [{ "op": "communities", "links": "links", "source": "s", "target": "t" }] } });
    let rows = texts(graph(), tables, "c", "`${d.community}:${d.order}:${d.degree}`");
    let c: Vec<Vec<f64>> = rows.iter().map(|s| nums(s)).collect();
    assert_eq!(c[0][0], c[1][0]);
    assert_eq!(c[1][0], c[2][0]);
    assert_eq!(c[3][0], c[4][0]);
    assert_eq!(c[4][0], c[5][0]);
    assert_ne!(c[0][0], c[3][0]);
    assert_eq!(c[6][0], 2.0, "the loner is a community of its own, the smallest");
    let mut order: Vec<f64> = c.iter().map(|r| r[1]).collect();
    order.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(order, vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0], "order is a permutation");
}

fn org() -> serde_json::Value {
    serde_json::json!({
        "org": { "values": {
            "id": ["ceo", "ops", "eng", "web", "app", "hr", "data"],
            "boss": [null, "ceo", "ceo", "eng", "eng", "ops", "eng"],
            "people": [1, 2, 3, 8, 5, 4, 6]
        }, "key": ["id"] }
    })
}

#[test]
fn tree_places_parents_over_children_and_hands_them_their_parents_position() {
    let tables = serde_json::json!({ "t": { "from": "org", "ops": [{ "op": "tree", "parent": "boss", "width": 400, "height": 200 }] } });
    let rows = texts(org(), tables, "t", "`${d.x}:${d.y}:${d.px}:${d.py}:${d.depth}:${d.leaf}`");
    let r: Vec<Vec<f64>> = rows.iter().map(|s| nums(s)).collect();
    assert_eq!(r[0][1], 0.0, "the root on top");
    assert!(r[0][2].is_nan() && r[0][3].is_nan(), "the root has no parent position: {}", rows[0]);
    assert_eq!(r[3][1], 200.0, "the deepest level at the bottom");
    assert_eq!((r[3][2], r[3][3]), (r[2][0], r[2][1]), "web's parent is eng");
    assert!((r[2][0] - (r[3][0] + r[6][0]) / 2.0).abs() < 1e-9, "eng centred over its first and last child");
    assert!(r[3][0] < r[4][0] && r[4][0] < r[6][0], "siblings keep row order");
    assert!(rows[3].ends_with(":true") && rows[2].ends_with(":false"));
    let tables = serde_json::json!({ "t": { "from": "org", "ops": [{ "op": "tree", "parent": "boss", "width": 400, "height": 200 }] } });
    assert_eq!(texts(org(), tables, "t", "d.path")[3], "ceo / eng / web", "the path from the root");
    // A dendrogram puts every leaf on the last level.
    let tables = serde_json::json!({ "t": { "from": "org", "ops": [{ "op": "tree", "parent": "boss", "method": "cluster", "width": 400, "height": 200 }] } });
    let rows = texts(org(), tables, "t", "`${d.y}:${d.leaf}`");
    assert!(rows.iter().filter(|s| s.ends_with("true")).all(|s| s.starts_with("200:")), "{rows:?}");
}

#[test]
fn partition_spans_are_proportional_to_the_summed_values() {
    let tables = serde_json::json!({ "p": { "from": "org", "ops": [{ "op": "partition", "parent": "boss", "value": "people", "width": 300, "height": 90, "sort": true }] } });
    let rows = texts(org(), tables, "p", "`${d.x0}:${d.y0}:${d.x1}:${d.y1}:${d.sum}:${d.share}:${d.branch}`");
    let r: Vec<Vec<f64>> = rows.iter().map(|s| nums(s)).collect();
    // Leaves: web 8, app 5, data 6 (eng 19), hr 4 (ops 4): the root spans everything.
    assert_eq!((r[0][0], r[0][2], r[0][4]), (0.0, 300.0, 23.0));
    assert_eq!((r[0][1], r[0][3]), (0.0, 30.0), "a band per depth");
    for (i, total) in [(2, 19.0), (1, 4.0), (3, 8.0)] {
        assert!(((r[i][2] - r[i][0]) - 300.0 * total / 23.0).abs() < 1e-9, "row {i} ∝ its sum");
        assert_eq!(r[i][4], total);
    }
    assert!(r[2][0] < r[1][0], "sorted: eng (19) before ops (4)");
    assert!(r[3][0] < r[6][0] && r[6][0] < r[4][0], "web 8, data 6, app 5");
    assert!((r[2][5] - 19.0 / 23.0).abs() < 1e-12, "share of the root");
    assert!(rows[3].ends_with(":eng") && rows[5].ends_with(":ops") && rows[0].ends_with(":ceo"), "top-level branch");
}

#[test]
fn chord_groups_and_ribbons() {
    let data = serde_json::json!({
        "trade": { "values": { "from": ["North", "North", "South", "East"], "to": ["South", "East", "East", "North"], "v": [5, 3, 2, 4] }, "key": ["from", "to"] }
    });
    let tables = serde_json::json!({
        "g": { "from": "trade", "ops": [{ "op": "chord-groups", "source": "from", "target": "to", "value": "v", "pad": 0.1 }] },
        "r": { "from": "trade", "ops": [{ "op": "chord-ribbons", "source": "from", "target": "to", "value": "v", "pad": 0.1, "r": 100 }] }
    });
    let groups = texts(data.clone(), tables.clone(), "g", "`${d.name}:${d.value}:${d.a1 - d.a0}`");
    assert_eq!(groups.len(), 3);
    assert!(groups[0].starts_with("North:12:") && groups[1].starts_with("South:7:") && groups[2].starts_with("East:9:"), "{groups:?}");
    let k = (std::f64::consts::TAU - 0.3) / 28.0;
    let span: f64 = groups[0].rsplit(':').next().unwrap().parse().unwrap();
    assert!((span - 12.0 * k).abs() < 1e-9);
    let ribbons = texts(data, tables, "r", "`${d.sa1 - d.sa0}:${d.ta1 - d.ta0}:${d.path}`");
    assert_eq!(ribbons.len(), 4);
    let w: f64 = ribbons[0].split(':').next().unwrap().parse().unwrap();
    assert!((w - 5.0 * k).abs() < 1e-9, "as wide as its value");
    assert!(ribbons.iter().all(|r| r.contains(":M") && r.ends_with('Z')), "{ribbons:?}");
}
