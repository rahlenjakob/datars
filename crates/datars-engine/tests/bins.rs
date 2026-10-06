//! Big-data table ops run through documents: rows binned into hexagons (`hexbin`) and grid cells
//! (`bin2d`) — every row counted once, values aggregated, keys that name the lattice — density
//! contours (`contours`) whose levels nest and hold their shares, and rows joined into path data
//! per group (`paths`).

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
    s.split('|').map(|x| x.parse::<f64>().unwrap_or(f64::NAN)).collect()
}

/// `n` generated rows: `x` uniform over [0, 100), `y` over [0, 50), `v` = 3 everywhere, and `w`
/// the row number.
fn uniform(n: usize) -> serde_json::Value {
    serde_json::json!({ "pts": { "generate": { "rows": n, "columns": [
        { "as": "x", "expr": "rand(d.i, 1) * 100" }, { "as": "y", "expr": "rand(d.i, 2) * 50" },
        { "as": "v", "expr": "3" }, { "as": "w", "expr": "d.i" }
    ] } } })
}

#[test]
fn hexbin_counts_every_row_once_on_a_lattice_named_by_its_size() {
    let tables = serde_json::json!({
        "hex": { "from": "pts", "ops": [{ "op": "hexbin", "x": "x", "y": "y", "columns": 20, "aspect": 0.5, "extent": [0, 0, 100, 50], "value": "v", "fn": "mean" }] },
        "sums": { "from": "pts", "ops": [{ "op": "hexbin", "x": "x", "y": "y", "columns": 20, "aspect": 0.5, "extent": [0, 0, 100, 50], "value": "w", "fn": "sum" }] }
    });
    let rows = texts(uniform(20_000), tables.clone(), "hex", "`${d.hex}|${d.count}|${d.value}|${d.x}|${d.y}|${d.hw}|${d.hh}`");
    assert!(rows.len() > 100, "{} hexagons", rows.len());
    let mut keys = std::collections::BTreeSet::new();
    let mut total = 0.0;
    for r in &rows {
        let (key, rest) = r.split_once('|').unwrap();
        assert!(key.starts_with("20:"), "the key names the lattice: {key}");
        assert!(keys.insert(key.to_string()), "keys are unique: {key}");
        let v = nums(rest);
        total += v[0];
        assert_eq!(v[1], 3.0, "the mean of a constant");
        // Centres on the lattice, within a hexagon of the extent.
        assert!(v[2] >= -2.6 && v[2] <= 102.6 && v[3] >= -3.0 && v[3] <= 53.0, "{r}");
        // 20 hexagons across 100 units: half a hexagon 2.5 wide; the circumradius in y is
        // 1/√3 of a lattice unit, 50 / (20 × 0.5) units of y each.
        assert!((v[4] - 2.5).abs() < 1e-9);
        assert!((v[5] - 5.0 / 3f64.sqrt()).abs() < 1e-9);
    }
    assert_eq!(total, 20_000.0, "every row in exactly one hexagon");
    // Another aggregate over the same bins: the row numbers, summed, are every row number once.
    let sums: f64 = texts(uniform(20_000), tables.clone(), "sums", "`${d.value}`").iter().map(|s| s.parse::<f64>().unwrap()).sum();
    assert_eq!(sums, (0..20_000).map(|i| i as f64).sum::<f64>());
    // Deterministic.
    assert_eq!(texts(uniform(20_000), tables, "hex", "`${d.hex}|${d.count}|${d.value}|${d.x}|${d.y}|${d.hw}|${d.hh}`"), rows);
}

#[test]
fn bin2d_counts_rows_into_cells_and_folds_a_value() {
    let data = serde_json::json!({ "pts": { "values": {
        "x": [0.0, 0.5, 9.9, 10.0, 5.0, 5.0, 20.0],
        "y": [0.0, 0.5, 9.9, 10.0, 5.0, 5.0, 1.0],
        "v": [1.0, 3.0, 10.0, 20.0, 7.0, 9.0, 100.0]
    } } });
    let tables = serde_json::json!({
        "cells": { "from": "pts", "ops": [{ "op": "bin2d", "x": "x", "y": "y", "columns": 2, "rows": 2, "extent": [0, 0, 10, 10], "value": "v", "fn": "max" }] },
        "all": { "from": "pts", "ops": [{ "op": "bin2d", "x": "x", "y": "y", "columns": 2, "rows": 2, "extent": [0, 0, 10, 10], "empty": true }] }
    });
    let rows = texts(data.clone(), tables.clone(), "cells", "`${d.cell}|${d.count}|${d.value}|${d.x0}|${d.x1}|${d.y0}|${d.y1}`");
    // (0,0) and (0.5,0.5) in the first cell; (9.9,9.9), (10,10) — the far edge is the last cell's —
    // and (5,5) twice in the last; (20, 1) outside the extent.
    assert_eq!(rows, vec!["2x2:0:0|2|3|0|5|0|5", "2x2:1:1|4|20|5|10|5|10"]);
    let all = texts(data, tables, "all", "`${d.cell}|${d.count}|${d.value}`");
    assert_eq!(all, vec!["2x2:0:0|2|2", "2x2:0:1|0|0", "2x2:1:0|0|0", "2x2:1:1|4|4"], "every cell with `empty`; the count without a value");
}

#[test]
fn contours_nest_and_hold_their_shares() {
    let data = serde_json::json!({ "pts": { "generate": { "rows": 30_000, "columns": [
        { "as": "x", "expr": "50 + randn(d.i, 1) * 8" }, { "as": "y", "expr": "20 + randn(d.i, 2) * 4" }
    ] } } });
    let tables = serde_json::json!({
        "rings": { "from": "pts", "ops": [{ "op": "contours", "x": "x", "y": "y", "columns": 80, "rows": 60, "bandwidth": 2, "levels": 3 }] }
    });
    let rows: Vec<Vec<f64>> = texts(data.clone(), tables, "rings", "`${d.level}|${d.ring}|${d.x}|${d.y}|${d.share}|${d.density}`").iter().map(|s| nums(s)).collect();
    let levels: std::collections::BTreeSet<i64> = rows.iter().map(|r| r[0] as i64).collect();
    assert_eq!(levels.into_iter().collect::<Vec<_>>(), vec![0, 1, 2]);
    // Each level holds its share — 3/4, 1/2, 1/4 of the rows — as far as a grid can cut them.
    for (level, want) in [(0, 0.75), (1, 0.5), (2, 0.25)] {
        let r = rows.iter().find(|r| r[0] as i64 == level).unwrap();
        assert!((r[4] - want).abs() < 0.03, "level {level}: share {} vs {want}", r[4]);
    }
    // Higher levels are denser…
    let density = |l: i64| rows.iter().find(|r| r[0] as i64 == l).unwrap()[5];
    assert!(density(0) < density(1) && density(1) < density(2));
    // …and nest: every vertex of level 2 lies inside a ring of level 0 (one blob: one ring each).
    let ring = |l: i64| -> Vec<(f64, f64)> { rows.iter().filter(|r| r[0] as i64 == l).map(|r| (r[2], r[3])).collect() };
    let outer = ring(0);
    let inside = |p: (f64, f64)| {
        let mut c = false;
        for i in 0..outer.len() {
            let (a, b) = (outer[i], outer[(i + 1) % outer.len()]);
            if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                c = !c;
            }
        }
        c
    };
    assert!(ring(2).iter().all(|p| inside(*p)), "level 2 inside level 0");
    // Around the cluster's centre, in the rows' own units.
    assert!(ring(2).iter().all(|p| (p.0 - 50.0).abs() < 12.0 && (p.1 - 20.0).abs() < 6.0));
}

#[test]
fn paths_join_each_groups_rows_with_subpaths_and_gaps() {
    let data = serde_json::json!({ "r": { "values": {
        "g": ["a", "a", "a", "b", "b", "b", "b", "a", "a", "a"],
        "ring": [0, 0, 0, 1, 1, 2, 2, 3, 3, 3],
        "x": [0, 1, 1, 5, 6, 7, 8, 2, 3, 3],
        "y": [0, 0, 1, 5, 5, null, 9, 2, 2, 3]
    } } });
    let tables = serde_json::json!({
        "closed": { "from": "r", "ops": [{ "op": "paths", "by": "g", "ring": "ring", "closed": true, "x": "=d.x * 10", "y": "=d.y" }] },
        "open": { "from": "r", "ops": [{ "op": "paths", "x": "=d.x", "y": "=d.y", "as": "d" }] }
    });
    let rows = texts(data.clone(), tables.clone(), "closed", "`${d.g}|${d.path}|${d.end_x}|${d.end_y}`");
    assert_eq!(rows, vec![
        // Two rings of `a` (its rows need not be adjacent), each closed.
        "a|M0.0 0.0L10.0 0.0L10.0 1.0ZM20.0 2.0L30.0 2.0L30.0 3.0Z|30|3",
        // A missing point ends a subpath (ring 2 starts again after it).
        "b|M50.0 5.0L60.0 5.0ZM80.0 9.0Z|80|9",
    ]);
    // No `by`: one path of every row, open, a gap where a point is missing.
    let one = texts(data, tables, "open", "`${d.d}`");
    assert_eq!(one, vec!["M0.0 0.0L1.0 0.0L1.0 1.0L5.0 5.0L6.0 5.0M8.0 9.0L2.0 2.0L3.0 2.0L3.0 3.0"]);
}

#[test]
fn a_bin_op_without_its_columns_says_so() {
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 100 },
        "data": { "pts": { "values": { "x": [1, 2] } } },
        "tables": { "h": { "from": "pts", "ops": [{ "op": "hexbin", "x": "x", "y": "nope" }] } },
        "scene": { "kind": "repeat", "key": "rows", "from": "h", "template": { "kind": "text", "text": "=d.hex", "at": [0, 0] } }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let _ = e.scene();
    assert!(e.diagnostics().iter().any(|d| d.message.contains("hexbin") && d.message.contains("nope")), "{:?}", e.diagnostics());
}

#[test]
fn a_stroked_path_is_hovered_along_its_line_not_across_its_hull() {
    // Two open paths (series lines) as `op.paths` makes them, the second drawn on top. Its hull —
    // the chord from its last point back to its first — covers the pointer, its stroke doesn't:
    // the pointer finds the line it's actually on.
    let doc = serde_json::json!({
        "datars": 1, "size": { "width": 200, "height": 200 },
        "scene": { "kind": "group", "key": "root", "children": [
            { "kind": "shape", "key": "low", "geom": { "type": "path", "d": "M10 150L190 150" }, "stroke": { "paint": "#888888", "width": 1 }, "pickable": true, "semantics": { "role": "series", "label": "low" } },
            { "kind": "shape", "key": "arch", "geom": { "type": "path", "d": "M10 190L100 20L190 190" }, "stroke": { "paint": "#888888", "width": 1 }, "pickable": true, "semantics": { "role": "series", "label": "arch" } }
        ] }
    });
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&doc.to_string()).unwrap());
    let _ = e.frame(0.0);
    assert_eq!(e.pointer(datars_engine::Pointer::Move { x: 100.0, y: 151.0 }).as_deref(), Some("low"), "on the low line, inside the arch's hull");
    assert_eq!(e.pointer(datars_engine::Pointer::Move { x: 100.0, y: 100.0 }), None, "inside the hull, near neither stroke");
    assert_eq!(e.pointer(datars_engine::Pointer::Move { x: 55.0, y: 105.5 }).as_deref(), Some("arch"), "on the arch");
}
