//! Graph and hierarchy table ops — thin adapters from node/link and parent-child tables to
//! datars-algo (force layouts, communities, chords, tidy trees, partitions), plus `lookup`, which
//! copies another table's columns onto rows by key (a link's endpoint positions from the laid-out
//! nodes). Nodes and links are matched by their id text, so numeric and text ids both work.

use crate::resolve::{Cx, Resolver};
use crate::tables::{cell_string, eval_rows, get_original};
use datars_algo as algo;
use datars_data::transform as tf;
use datars_data::{Column, Table};
use datars_ir::Prop;
use datars_math::{Hash64, Rect, Rng, Vec2};
use std::collections::BTreeMap;
use std::sync::Arc;

fn num(r: &Resolver, op: &serde_json::Value, k: &str, cx: &Cx, d: f64) -> f64 {
    match op.get(k) {
        None | Some(serde_json::Value::Null) => d,
        Some(v) => r.num(&Prop(v.clone()), cx, d),
    }
}

fn opt_str<'a>(op: &'a serde_json::Value, k: &str) -> Option<&'a str> {
    op.get(k).and_then(|v| v.as_str()).filter(|v| !v.is_empty())
}

fn need(t: &Table, col: &str, what: &str) -> Result<(), String> {
    t.column(col).map(|_| ()).ok_or_else(|| format!("no column `{col}` ({what})"))
}

fn with(t: Table, cols: Vec<(&str, Column)>) -> Result<Table, String> {
    let mut out = t;
    for (n, c) in cols {
        out = tf::derive(&out, n, c).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

/// The column holding each row's id: `id` when given, else the table's first key column.
fn id_column(t: &Table, op: &serde_json::Value) -> Result<String, String> {
    let id = opt_str(op, "id").map(String::from).or_else(|| t.key.first().cloned()).ok_or("needs `id` (the column naming each row), or a keyed table")?;
    need(t, &id, "the ids")?;
    Ok(id)
}

/// Row index of each id (the first row with it).
fn index_of(t: &Table, col: &str) -> BTreeMap<String, usize> {
    let mut m = BTreeMap::new();
    for i in 0..t.len() {
        m.entry(cell_string(t, col, i)).or_insert(i);
    }
    m
}

/// A links table's rows as `(source node, target node, weight)` over the nodes' ids; links naming a
/// node that isn't there are left out. Weights: the `weight` column, else 1.
fn graph_links(r: &Resolver, nodes: &Table, id: &str, op: &serde_json::Value, cx: &Cx) -> Result<Vec<(usize, usize, f64)>, String> {
    let name = opt_str(op, "links").ok_or("needs `links` (the table of links)")?;
    let links = get_original(r, name, cx).ok_or_else(|| format!("unknown links table `{name}`"))?;
    let (src, dst) = (opt_str(op, "source").unwrap_or("source"), opt_str(op, "target").unwrap_or("target"));
    need(&links, src, "the links' sources")?;
    need(&links, dst, "the links' targets")?;
    let weight = match opt_str(op, "weight") {
        Some(w) => {
            need(&links, w, "the links' weights")?;
            links.column(w).map(|c| c.to_f64())
        }
        None => None,
    };
    let idx = index_of(nodes, id);
    Ok((0..links.len())
        .filter_map(|i| {
            let a = *idx.get(&cell_string(&links, src, i))?;
            let b = *idx.get(&cell_string(&links, dst, i))?;
            let w = weight.as_ref().map_or(1.0, |w| w[i]);
            Some((a, b, if w.is_finite() { w.max(0.0) } else { 0.0 }))
        })
        .collect())
}

/// Parent index per row from an id column and a parent column (a null, empty or unknown parent
/// makes a root).
fn parents(t: &Table, op: &serde_json::Value) -> Result<(String, Vec<Option<usize>>), String> {
    let id = id_column(t, op)?;
    let parent = opt_str(op, "parent").unwrap_or("parent");
    need(t, parent, "each row's parent id")?;
    let idx = index_of(t, &id);
    let pcol = t.column(parent).ok_or("parent")?;
    let ps = (0..t.len()).map(|i| if pcol.is_null(i) { None } else { idx.get(&cell_string(t, parent, i)).copied().filter(|&p| p != i) }).collect();
    Ok((id, ps))
}

/// Each node's path from its root, as names joined by " / " (the `label` column's text, else the
/// ids): what a tooltip says to place a node in its hierarchy.
fn paths(t: &Table, h: &algo::Hierarchy, id: &str, op: &serde_json::Value) -> Column {
    let name_col = opt_str(op, "label").filter(|l| t.column(l).is_some()).unwrap_or(id);
    let mut out: Vec<String> = vec![String::new(); t.len()];
    for &v in &h.preorder {
        let name = cell_string(t, name_col, v);
        out[v] = match h.parent[v] {
            Some(p) => format!("{} / {name}", out[p]),
            None => name,
        };
    }
    Column::Str(out.into_iter().map(|s| Some(Arc::from(s.as_str()))).collect())
}

pub(crate) fn apply(r: &Resolver, t: &Table, kind: &str, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    match kind {
        "force" => force(r, t, op, cx),
        "communities" => {
            let id = id_column(t, op)?;
            let links = graph_links(r, t, &id, op, cx)?;
            let c = algo::communities(t.len(), &links, num(r, op, "resolution", cx, 1.0));
            let d = algo::degrees(t.len(), &links);
            let order = algo::community_order(&c, &d);
            with(t.clone(), vec![
                ("community", Column::Num(c.iter().map(|&x| x as f64).collect())),
                ("degree", Column::Num(d)),
                ("order", Column::Num(order.iter().map(|&x| x as f64).collect())),
            ])
        }
        "lookup" => {
            let name = opt_str(op, "from").ok_or("needs `from` (the table to look values up in)")?;
            let from = get_original(r, name, cx).ok_or_else(|| format!("unknown table `{name}`"))?;
            let key = opt_str(op, "key").map(String::from).or_else(|| from.key.first().cloned()).ok_or("needs `key` (the column of `from` to match), or a keyed `from` table")?;
            need(&from, &key, "the looked-up ids")?;
            let field = opt_str(op, "field").ok_or("needs `field` (the column holding each row's id in `from`)")?;
            need(t, field, "the ids to look up")?;
            let values: Vec<String> = op.get("values").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            if values.is_empty() {
                return Err("needs `values` (the columns to copy)".into());
            }
            let names: Vec<String> = match op.get("as").and_then(|v| v.as_array()) {
                Some(a) if a.len() == values.len() => a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
                _ => values.clone(),
            };
            let idx = index_of(&from, &key);
            let rows: Vec<Option<usize>> = (0..t.len()).map(|i| idx.get(&cell_string(t, field, i)).copied()).collect();
            let mut cols = Vec::new();
            for (v, n) in values.iter().zip(&names) {
                let c = from.column(v).ok_or_else(|| format!("no column `{v}` in `{name}`"))?;
                cols.push((n.as_str(), c.take_opt(&rows)));
            }
            with(t.clone(), cols)
        }
        "tree" => {
            let (id, ps) = parents(t, op)?;
            let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
            let opts = algo::TreeOptions { sibling_separation: num(r, op, "separation", cx, 1.0), cousin_separation: 2.0 * num(r, op, "separation", cx, 1.0), node_size: None };
            let pos = if op.get("method").and_then(|m| m.as_str()) == Some("cluster") { algo::cluster(&ps, rect, &opts) } else { algo::tree(&ps, rect, &opts) };
            let h = algo::Hierarchy::new(&ps);
            let n = t.len();
            let parent_of = |i: usize| h.parent[i];
            with(t.clone(), vec![
                ("x", Column::Num(pos.iter().map(|p| p.x).collect())),
                ("y", Column::Num(pos.iter().map(|p| p.y).collect())),
                ("px", Column::Num((0..n).map(|i| parent_of(i).map_or(f64::NAN, |p| pos[p].x)).collect())),
                ("py", Column::Num((0..n).map(|i| parent_of(i).map_or(f64::NAN, |p| pos[p].y)).collect())),
                ("depth", Column::Num(h.depth.iter().map(|&d| d as f64).collect())),
                ("leaf", Column::Bool((0..n).map(|i| h.is_leaf(i)).collect())),
                ("path", paths(t, &h, &id, op)),
            ])
        }
        "partition" => partition(r, t, op, cx),
        "chord-groups" | "chord-ribbons" => chord(r, t, kind, op, cx),
        other => Err(format!("unknown table op `{other}`")),
    }
}

fn force(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let id = id_column(t, op)?;
    let raw = graph_links(r, t, &id, op, cx)?;
    let mut links = raw.clone();
    // Heavier links pull harder: strength ∝ √(weight / heaviest), so light ones still hold.
    if opt_str(op, "weight").is_some() {
        let max = links.iter().map(|l| l.2).fold(0.0, f64::max);
        for l in &mut links {
            l.2 = if max > 0.0 { (l.2 / max).sqrt() } else { 1.0 };
        }
    }
    let n = t.len();
    let (w, h) = (num(r, op, "width", cx, cx.box_w).max(0.0), num(r, op, "height", cx, cx.box_h).max(0.0));
    let radii: Vec<f64> = match op.get("radius") {
        None | Some(serde_json::Value::Null) => vec![5.0; n],
        Some(p) => eval_rows(r, t, &Prop(p.clone()), cx).iter().map(crate::resolve::value_num).collect(),
    };
    let seed = op.get("seed").and_then(|s| s.as_u64()).unwrap_or(1);
    let center = Vec2::new(w / 2.0, h / 2.0);
    // Each node starts at a place drawn from its own id (and the seed), not its row number, so
    // adding or removing a node leaves the others' starts — and mostly their layout — where they were.
    let spread = w.min(h) * 0.3;
    let init: Vec<Vec2> = (0..n)
        .map(|i| {
            let mut hs = Hash64::new();
            hs.str(&cell_string(t, &id, i));
            let mut rng = Rng::new(hs.finish() ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let a = rng.range(0.0, datars_math::m::TAU);
            let rr = spread * rng.next_f64().sqrt();
            Vec2::polar(center, rr, a)
        })
        .collect();
    let opts = algo::ForceOptions {
        iterations: op.get("iterations").and_then(|s| s.as_u64()).unwrap_or(300).min(10_000) as usize,
        seed,
        charge: num(r, op, "charge", cx, -30.0),
        link_distance: num(r, op, "distance", cx, 30.0),
        center,
        collide_radius: num(r, op, "collide", cx, 1.0),
        theta: 0.9,
        gravity: num(r, op, "gravity", cx, 0.05),
        velocity_decay: 0.4,
    };
    let p = algo::force_within(&init, &links, &radii, Some(Rect::new(0.0, 0.0, w, h)), &opts);
    let names: Vec<String> = match op.get("as").and_then(|a| a.as_array()) {
        Some(a) if a.len() == 2 => a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
        _ => vec!["x".into(), "y".into()],
    };
    with(t.clone(), vec![
        (names[0].as_str(), Column::Num(p.iter().map(|q| q.x).collect())),
        (names[1].as_str(), Column::Num(p.iter().map(|q| q.y).collect())),
        ("degree", Column::Num(algo::degrees(n, &raw))),
    ])
}

fn partition(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let (id, ps) = parents(t, op)?;
    let n = t.len();
    let h = algo::Hierarchy::new(&ps);
    // Leaves' values (a missing value field counts each leaf once).
    let values: Vec<f64> = match opt_str(op, "value") {
        Some(v) => {
            need(t, v, "the values")?;
            t.column(v).map(|c| c.to_f64()).unwrap_or_default()
        }
        None => vec![1.0; n],
    };
    let sums = h.sums(&values);
    let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
    let pad = num(r, op, "padding", cx, 0.0);
    let rects = if op.get("sort").and_then(|s| s.as_bool()).unwrap_or(false) {
        let (order, np) = algo::sort_by_key_desc(&ps, &sums);
        let nv: Vec<f64> = order.iter().map(|&i| values.get(i).copied().unwrap_or(0.0)).collect();
        let laid = algo::partition(&np, &nv, rect, pad);
        let mut out = vec![Rect::new(0.0, 0.0, 0.0, 0.0); n];
        for (k, &i) in order.iter().enumerate() {
            out[i] = laid[k];
        }
        out
    } else {
        algo::partition(&ps, &values, rect, pad)
    };
    // Each node's top-level branch (its ancestor just below a root; a root is its own) and share
    // of its root's total.
    let mut branch = vec![0usize; n];
    let mut root = vec![0usize; n];
    for &v in &h.preorder {
        match h.parent[v] {
            None => {
                branch[v] = v;
                root[v] = v;
            }
            Some(p) => {
                branch[v] = if h.parent[p].is_none() { v } else { branch[p] };
                root[v] = root[p];
            }
        }
    }
    let ids: Vec<Option<Arc<str>>> = (0..n).map(|i| Some(Arc::from(cell_string(t, &id, branch[i]).as_str()))).collect();
    with(t.clone(), vec![
        ("x0", Column::Num(rects.iter().map(|q| q.x).collect())),
        ("y0", Column::Num(rects.iter().map(|q| q.y).collect())),
        ("x1", Column::Num(rects.iter().map(|q| q.x1()).collect())),
        ("y1", Column::Num(rects.iter().map(|q| q.y1()).collect())),
        ("depth", Column::Num(h.depth.iter().map(|&d| d as f64).collect())),
        ("leaf", Column::Bool((0..n).map(|i| h.is_leaf(i)).collect())),
        ("sum", Column::Num(sums.clone())),
        ("share", Column::Num((0..n).map(|i| if sums[root[i]] > 0.0 { sums[i] / sums[root[i]] } else { 0.0 }).collect())),
        ("branch", Column::Str(ids)),
        ("path", paths(t, &h, &id, op)),
    ])
}

fn chord(r: &Resolver, t: &Table, kind: &str, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let (src, dst) = (opt_str(op, "source").unwrap_or("source"), opt_str(op, "target").unwrap_or("target"));
    need(t, src, "the links' sources")?;
    need(t, dst, "the links' targets")?;
    let vals: Vec<f64> = match opt_str(op, "value") {
        Some(v) => {
            need(t, v, "the links' values")?;
            t.column(v).map(|c| c.to_f64()).unwrap_or_default()
        }
        None => vec![1.0; t.len()],
    };
    // Groups in order of first appearance.
    let mut names: Vec<String> = Vec::new();
    let mut idx: BTreeMap<String, usize> = BTreeMap::new();
    let mut id = |s: String, names: &mut Vec<String>| {
        *idx.entry(s.clone()).or_insert_with(|| {
            names.push(s);
            names.len() - 1
        })
    };
    let links: Vec<(usize, usize, f64)> = (0..t.len())
        .map(|i| {
            let a = id(cell_string(t, src, i), &mut names);
            let b = id(cell_string(t, dst, i), &mut names);
            (a, b, vals[i])
        })
        .collect();
    let lay = algo::chord(names.len(), &links, num(r, op, "pad", cx, 0.04));
    if kind == "chord-groups" {
        let cols = vec![
            ("name".to_string(), Column::Str(names.iter().map(|n| Some(Arc::from(n.as_str()))).collect())),
            ("a0".to_string(), Column::Num(lay.groups.iter().map(|g| g.a0).collect())),
            ("a1".to_string(), Column::Num(lay.groups.iter().map(|g| g.a1).collect())),
            ("value".to_string(), Column::Num(lay.groups.iter().map(|g| g.value).collect())),
            ("index".to_string(), Column::Num((0..names.len()).map(|i| i as f64).collect())),
        ];
        return Table::from_columns("chord-groups", cols).and_then(|t| t.with_key(&["name"])).map_err(|e| e.to_string());
    }
    let mut cols = vec![
        ("sa0", Column::Num(lay.ribbons.iter().map(|x| x.source.0).collect())),
        ("sa1", Column::Num(lay.ribbons.iter().map(|x| x.source.1).collect())),
        ("ta0", Column::Num(lay.ribbons.iter().map(|x| x.target.0).collect())),
        ("ta1", Column::Num(lay.ribbons.iter().map(|x| x.target.1).collect())),
    ];
    // The ribbon's outline, when told where the circle is.
    if op.get("r").is_some_and(|v| !v.is_null()) {
        let c = Vec2::new(num(r, op, "cx", cx, cx.box_w / 2.0), num(r, op, "cy", cx, cx.box_h / 2.0));
        let radius = num(r, op, "r", cx, 0.0);
        let paths = lay.ribbons.iter().map(|rb| Some(Arc::from(crate::resolve::svg_path_string(&algo::chord_ribbon(c, radius, rb)).as_str()))).collect();
        cols.push(("path", Column::Str(paths)));
    }
    with(t.clone(), cols)
}
