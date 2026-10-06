//! Layout-algorithm table ops — thin adapters from tables to datars-algo (the algorithms live
//! there, behind public functions any package can use, P5).

use crate::resolve::{Cx, Resolver};
use crate::tables::{cell_string, eval_rows};
use datars_algo as algo;
use datars_data::transform as tf;
use datars_data::{Column, Table};
use datars_ir::Prop;
use datars_math::{m, Rect, Vec2};
use std::collections::BTreeMap;
use std::sync::Arc;

mod bins;

fn num(r: &Resolver, op: &serde_json::Value, k: &str, cx: &Cx, d: f64) -> f64 {
    match op.get(k) {
        None | Some(serde_json::Value::Null) => d,
        Some(v) => r.num(&Prop(v.clone()), cx, d),
    }
}

fn s<'a>(op: &'a serde_json::Value, k: &str) -> Result<&'a str, String> {
    op.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("needs `{k}`"))
}

fn nums(t: &Table, col: &str) -> Result<Vec<f64>, String> {
    Ok(t.column(col).ok_or_else(|| format!("no column `{col}`"))?.to_f64())
}

fn with(t: Table, cols: Vec<(&str, Vec<f64>)>) -> Result<Table, String> {
    let mut out = t;
    for (n, v) in cols {
        out = tf::derive(&out, n, Column::Num(v)).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn as_names<const N: usize>(op: &serde_json::Value, default: [&'static str; N]) -> Vec<String> {
    match op.get("as").and_then(|a| a.as_array()) {
        Some(a) if a.len() == N => a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
        _ => default.iter().map(|x| x.to_string()).collect(),
    }
}

/// Group row indices by a key column, in order of first appearance.
fn groups(t: &Table, col: &str) -> (Vec<String>, BTreeMap<String, Vec<usize>>) {
    let mut order = Vec::new();
    let mut map: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in 0..t.len() {
        let k = cell_string(t, col, i);
        if !map.contains_key(&k) {
            order.push(k.clone());
        }
        map.entry(k).or_default().push(i);
    }
    (order, map)
}

/// The first key column that isn't the unit index (the party of a seat).
fn parent_key(t: &Table) -> Option<String> {
    t.key.iter().find(|k| k.as_str() != "__unit").cloned()
}

pub(crate) fn apply(r: &Resolver, t: &Table, kind: &str, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    match kind {
        "stack" => {
            let (x, series, value) = (s(op, "x")?, s(op, "series")?, s(op, "value")?);
            let names = as_names(op, ["y0", "y1"]);
            let (xs, xmap) = groups(t, x);
            let (ss, _) = groups(t, series);
            let vals = nums(t, value)?;
            let mut m: Vec<Vec<f64>> = vec![vec![0.0; xs.len()]; ss.len()];
            let xi: BTreeMap<&str, usize> = xs.iter().enumerate().map(|(i, k)| (k.as_str(), i)).collect();
            let si: BTreeMap<String, usize> = ss.iter().enumerate().map(|(i, k)| (k.clone(), i)).collect();
            for (xk, rows) in &xmap {
                for &row in rows {
                    let s_i = si[&cell_string(t, series, row)];
                    m[s_i][xi[xk.as_str()]] += if vals[row].is_finite() { vals[row] } else { 0.0 };
                }
            }
            let offset = match op.get("offset").and_then(|o| o.as_str()) {
                Some("expand") => algo::StackOffset::Expand,
                Some("silhouette") => algo::StackOffset::Silhouette,
                Some("wiggle") => algo::StackOffset::Wiggle,
                _ => algo::StackOffset::Zero,
            };
            // `order`: which series sit at the baseline — `inside-out` is a streamgraph's (the
            // biggest layers in the middle, so the wiggle offset has least to move).
            let order = match op.get("order").and_then(|o| o.as_str()) {
                Some("reverse") => algo::StackOrder::Reverse,
                Some("ascending") => algo::StackOrder::Ascending,
                Some("descending") => algo::StackOrder::Descending,
                Some("inside-out") => algo::StackOrder::InsideOut,
                _ => algo::StackOrder::Input,
            };
            let st = algo::stack(&m, offset, order);
            let (mut y0, mut y1) = (vec![f64::NAN; t.len()], vec![f64::NAN; t.len()]);
            for i in 0..t.len() {
                let (a, b) = st[si[&cell_string(t, series, i)]][xi[cell_string(t, x, i).as_str()]];
                y0[i] = a;
                y1[i] = b;
            }
            with(t.clone(), vec![(&names[0], y0), (&names[1], y1)])
        }
        "pie" => {
            let v = nums(t, s(op, "value")?)?;
            let sort = match op.get("sort").and_then(|x| x.as_str()) {
                Some("asc") => algo::PieSort::Asc,
                Some("desc") => algo::PieSort::Desc,
                _ => algo::PieSort::None,
            };
            let a = algo::pie(&v, num(r, op, "start", cx, 0.0), num(r, op, "end", cx, m::TAU), num(r, op, "pad", cx, 0.0), sort);
            let names = as_names(op, ["a0", "a1"]);
            with(t.clone(), vec![(&names[0], a.iter().map(|x| x.0).collect()), (&names[1], a.iter().map(|x| x.1).collect())])
        }
        "treemap" => {
            let v = nums(t, s(op, "value")?)?;
            let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
            let rs = algo::treemap(&v, rect, algo::GOLDEN);
            let n = as_names(op, ["x0", "y0", "x1", "y1"]);
            with(t.clone(), vec![(&n[0], rs.iter().map(|q| q.x).collect()), (&n[1], rs.iter().map(|q| q.y).collect()), (&n[2], rs.iter().map(|q| q.x1()).collect()), (&n[3], rs.iter().map(|q| q.y1()).collect())])
        }
        "pack" => {
            let v = nums(t, s(op, "value")?)?;
            let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
            let cs = algo::pack_values(&v, rect, num(r, op, "padding", cx, 2.0));
            with(t.clone(), vec![("cx", cs.iter().map(|c| c.center.x).collect()), ("cy", cs.iter().map(|c| c.center.y).collect()), ("r", cs.iter().map(|c| c.r).collect())])
        }
        "beeswarm" => {
            let pos: Vec<f64> = eval_rows(r, t, &Prop(op.get("position").cloned().unwrap_or_default()), cx).iter().map(crate::resolve::value_num).collect();
            let off = algo::beeswarm(&pos, num(r, op, "radius", cx, 3.0), 0.0, algo::SwarmSide::Both);
            with(t.clone(), vec![(op.get("as").and_then(|a| a.as_str()).unwrap_or("offset"), off)])
        }
        "spread" => {
            // 1-D: positions (an expression, usually px through a scale) pushed `gap` apart.
            let pos: Vec<f64> = eval_rows(r, t, &Prop(op.get("position").cloned().unwrap_or_default()), cx).iter().map(crate::resolve::value_num).collect();
            let out = algo::spread(&pos, num(r, op, "gap", cx, 12.0), num(r, op, "min", cx, f64::NEG_INFINITY), num(r, op, "max", cx, f64::INFINITY));
            with(t.clone(), vec![(op.get("as").and_then(|a| a.as_str()).unwrap_or("spread"), out)])
        }
        "lanes" => {
            // 1-D intervals (expressions: a label's left and right edge in px) packed into lanes;
            // null past `max` lanes (what doesn't fit is left out, not piled on).
            let ends = |k: &str| -> Vec<f64> { eval_rows(r, t, &Prop(op.get(k).cloned().unwrap_or_default()), cx).iter().map(crate::resolve::value_num).collect() };
            let max = num(r, op, "max", cx, f64::INFINITY);
            let max = max.is_finite().then(|| max.max(0.0).floor() as usize);
            let out = algo::lanes(&ends("start"), &ends("end"), num(r, op, "gap", cx, 0.0), max);
            with(t.clone(), vec![(op.get("as").and_then(|a| a.as_str()).unwrap_or("lane"), out.iter().map(|l| l.map_or(f64::NAN, |l| l as f64)).collect())])
        }
        "units" => {
            let v = nums(t, s(op, "value")?)?;
            let mut rows = Vec::new();
            let mut unit = Vec::new();
            for (i, x) in v.iter().enumerate() {
                let n = if x.is_finite() { x.round().max(0.0) as usize } else { 0 };
                for u in 0..n.min(1_000_000) {
                    rows.push(i);
                    unit.push(u as f64);
                }
            }
            let mut out = t.take_rows(&rows);
            // The key's unit part is always the `__unit` column (a row's key reads it as a unit,
            // and `parliament` finds the parent key past it); `as` names the column expressions
            // read the unit number from, a copy of it.
            if let Some(name) = op.get("as").and_then(|a| a.as_str()).filter(|a| !a.is_empty() && *a != "__unit") {
                out = tf::derive(&out, name, Column::Num(unit.clone())).map_err(|e| e.to_string())?;
            }
            out = tf::derive(&out, "__unit", Column::Num(unit)).map_err(|e| e.to_string())?;
            let mut key: Vec<String> = t.key.clone();
            key.push("__unit".into());
            let kr: Vec<&str> = key.iter().map(|k| k.as_str()).collect();
            out.with_key(&kr).map_err(|e| e.to_string())
        }
        "parliament" => {
            let pk = parent_key(t).ok_or("parliament needs a units table (op units first)")?;
            let (order, map) = groups(t, &pk);
            let seats: Vec<usize> = order.iter().map(|k| map[k].len()).collect();
            let rows = op.get("rows").and_then(|v| v.as_u64()).map(|v| v as usize);
            let layout = algo::parliament(&seats, rows, Vec2::new(num(r, op, "cx", cx, cx.box_w / 2.0), num(r, op, "cy", cx, cx.box_h)), num(r, op, "r0", cx, cx.box_h * 0.4), num(r, op, "r1", cx, cx.box_h * 0.95));
            let (mut x, mut y) = (vec![f64::NAN; t.len()], vec![f64::NAN; t.len()]);
            let mut next: Vec<usize> = vec![0; order.len()];
            for (party, pos) in &layout.seats {
                if let Some(rs) = order.get(*party).and_then(|k| map.get(k)) {
                    if let Some(&row) = rs.get(next[*party]) {
                        x[row] = pos.x;
                        y[row] = pos.y;
                    }
                    next[*party] += 1;
                }
            }
            let rr = vec![layout.seat_radius; t.len()];
            with(t.clone(), vec![("x", x), ("y", y), ("r", rr)])
        }
        "waffle" => {
            let pk = parent_key(t).ok_or("waffle needs a units table (op units first)")?;
            let (order, map) = groups(t, &pk);
            let counts: Vec<usize> = order.iter().map(|k| map[k].len()).collect();
            let cols = op.get("columns").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let rws = op.get("rows").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
            let gap = num(r, op, "gap", cx, 2.0);
            let cells = algo::waffle(&counts, cols, rws, rect, algo::WaffleOrder::RowMajor);
            let (mut x, mut y, mut w, mut h) = (vec![f64::NAN; t.len()], vec![f64::NAN; t.len()], vec![0.0; t.len()], vec![0.0; t.len()]);
            let mut next: Vec<usize> = vec![0; order.len()];
            for (party, cell) in cells {
                if let Some(rs) = order.get(party).and_then(|k| map.get(k)) {
                    if let Some(&row) = rs.get(next[party]) {
                        x[row] = cell.x + gap / 2.0;
                        y[row] = cell.y + gap / 2.0;
                        w[row] = (cell.w - gap).max(0.5);
                        h[row] = (cell.h - gap).max(0.5);
                    }
                    next[party] += 1;
                }
            }
            with(t.clone(), vec![("x", x), ("y", y), ("w", w), ("h", h)])
        }
        "calendar" => {
            let days: Vec<i32> = nums(t, s(op, "date")?)?.iter().map(|d| *d as i32).collect();
            let cell = num(r, op, "cell", cx, 12.0);
            let week_start = if op.get("weekStart").and_then(|w| w.as_str()) == Some("sunday") { algo::WeekStart::Sunday } else { algo::WeekStart::Monday };
            let cells = algo::calendar(&days, cell, week_start);
            // `panel`: the day's year counted from the first year in the rows (0, 1, …), so several
            // years can stack instead of drawing on top of each other (cx/cy are panel-local).
            with(
                t.clone(),
                vec![
                    ("cx", cells.iter().map(|c| c.rect.x).collect()),
                    ("cy", cells.iter().map(|c| c.rect.y).collect()),
                    ("cell", vec![cell; t.len()]),
                    ("panel", cells.iter().map(|c| c.panel as f64).collect()),
                ],
            )
        }
        "sankey-nodes" | "sankey-links" => {
            let (src, dst, value) = (s(op, "source")?, s(op, "target")?, s(op, "value")?);
            let mut names: Vec<String> = Vec::new();
            let mut idx: BTreeMap<String, usize> = BTreeMap::new();
            let mut id = |n: String, names: &mut Vec<String>| -> usize {
                *idx.entry(n.clone()).or_insert_with(|| {
                    names.push(n);
                    names.len() - 1
                })
            };
            let vals = nums(t, value)?;
            let mut links = Vec::new();
            for i in 0..t.len() {
                let a = id(cell_string(t, src, i), &mut names);
                let b = id(cell_string(t, dst, i), &mut names);
                links.push((a, b, if vals[i].is_finite() { vals[i] } else { 0.0 }));
            }
            let rect = Rect::new(0.0, 0.0, num(r, op, "width", cx, cx.box_w), num(r, op, "height", cx, cx.box_h));
            let opts = algo::SankeyOptions { node_width: num(r, op, "nodeWidth", cx, 14.0), node_padding: num(r, op, "nodePadding", cx, 12.0), ..Default::default() };
            let lay = algo::sankey(names.len(), &links, rect, &opts);
            if kind == "sankey-nodes" {
                let cols = vec![
                    ("name".to_string(), Column::Str(names.iter().map(|n| Some(Arc::from(n.as_str()))).collect())),
                    ("x0".to_string(), Column::Num(lay.nodes.iter().map(|q| q.x).collect())),
                    ("y0".to_string(), Column::Num(lay.nodes.iter().map(|q| q.y).collect())),
                    ("x1".to_string(), Column::Num(lay.nodes.iter().map(|q| q.x1()).collect())),
                    ("y1".to_string(), Column::Num(lay.nodes.iter().map(|q| q.y1()).collect())),
                    ("value".to_string(), Column::Num(lay.node_values.clone())),
                ];
                Table::from_columns("sankey-nodes", cols).and_then(|t| t.with_key(&["name"])).map_err(|e| e.to_string())
            } else {
                let paths: Vec<Option<Arc<str>>> = lay
                    .links
                    .iter()
                    .map(|l| {
                        if !l.width.is_finite() || l.width <= 0.0 {
                            return Some(Arc::from(""));
                        }
                        let (h, xm) = (l.width / 2.0, (l.x0 + l.x1) / 2.0);
                        Some(Arc::from(
                            format!(
                                "M{:.2} {:.2}C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}L{:.2} {:.2}C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}Z",
                                l.x0, l.y0 - h, xm, l.y0 - h, xm, l.y1 - h, l.x1, l.y1 - h, l.x1, l.y1 + h, xm, l.y1 + h, xm, l.y0 + h, l.x0, l.y0 + h
                            )
                            .as_str(),
                        ))
                    })
                    .collect();
                tf::derive(t, "path", Column::Str(paths)).map_err(|e| e.to_string())
            }
        }
        "waterfall" => {
            let v = nums(t, s(op, "value")?)?;
            let totals: Vec<bool> = match op.get("total").and_then(|x| x.as_str()) {
                Some(c) => match t.column(c) {
                    Some(Column::Bool(b)) => b.clone(),
                    Some(col) => col.to_f64().iter().map(|x| *x != 0.0 && x.is_finite()).collect(),
                    None => vec![false; t.len()],
                },
                None => (0..t.len()).map(|i| t.key.first().is_some_and(|k| cell_string(t, k, i).starts_with('='))).collect(),
            };
            let w = algo::waterfall(&v, &totals);
            let out = with(t.clone(), vec![("start", w.iter().map(|x| x.0).collect()), ("end", w.iter().map(|x| x.1).collect())])?;
            tf::derive(&out, "is_total", Column::Bool(totals)).map_err(|e| e.to_string())
        }
        "scatter-in" => {
            let src = s(op, "geo")?;
            let g = r.geo.get(src).ok_or_else(|| format!("unknown geo source `{src}`"))?;
            let counts: Vec<f64> = eval_rows(r, t, &Prop(op.get("count").cloned().unwrap_or_default()), cx).iter().map(crate::resolve::value_num).collect();
            let seed = op.get("seed").and_then(|s| s.as_u64()).unwrap_or(1);
            // The column holding each row's feature id: `key` when given, else the table's key.
            let key_col = match op.get("key").and_then(|k| k.as_str()) {
                Some(k) => k.to_string(),
                None => t.key.first().cloned().unwrap_or_else(|| "id".into()),
            };
            if t.column(&key_col).is_none() {
                return Err(format!("no column `{key_col}` (the feature ids)"));
            }
            if ["dot", "lon", "lat"].contains(&key_col.as_str()) {
                return Err(format!("the feature id column can't be called `{key_col}` (a column of the dots)"));
            }
            let (mut lon, mut lat, mut keys, mut ids) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            for i in 0..t.len() {
                let k = cell_string(t, &key_col, i);
                let Some(f) = g.feature(&k) else { continue };
                let n = if counts[i].is_finite() { counts[i].max(0.0) as usize } else { 0 };
                // The largest polygon gets them all (islands stay empty at low counts).
                let polys = f.geometry.polygons();
                let Some(poly) = polys.iter().filter(|p| !p.is_empty()).max_by(|a, b| datars_math::total_cmp(crate::geo::ring_area(&a[0]), crate::geo::ring_area(&b[0]))) else { continue };
                for (j, p) in algo::scatter_in(poly, n, seed.wrapping_add(i as u64)).iter().enumerate() {
                    lon.push(p.x);
                    lat.push(p.y);
                    keys.push(Some(Arc::from(k.as_str())));
                    ids.push(j as f64);
                }
            }
            // Keyed (feature, dot index): a dot is one of its region's dots — it pairs only with
            // that region's dots in another state, and splits from (merges into) the region's key.
            let cols = vec![
                (key_col.clone(), Column::Str(keys)),
                ("dot".to_string(), Column::Num(ids)),
                ("lon".to_string(), Column::Num(lon)),
                ("lat".to_string(), Column::Num(lat)),
            ];
            Table::from_columns("dots", cols).and_then(|t| t.with_key(&[key_col.as_str(), "dot"])).map_err(|e| e.to_string())
        }
        "kde" => kde_table(r, t, op, cx),
        // Big data: rows into hexagons, grid cells and density contours; rows into paths.
        "hexbin" | "bin2d" | "contours" | "paths" => bins::apply(r, t, kind, op, cx),
        // Graphs and hierarchies (force, communities, lookup, tree, partition, chords).
        other => crate::graph_ops::apply(r, t, other, op, cx),
    }
}

/// `kde`: a 1-D Gaussian kernel density per group (`groupby`) of `field`, sampled at `steps` points
/// on one grid shared by every group — the field's extent over the whole table, or `extent` — so
/// ridgelines line up and violins compare. A new table: the group columns, `value` and `density`
/// (or `as`), and `__sample` (the grid index), keyed `(group…, __sample)`; groups in order of
/// first appearance. `bandwidth` (value units) defaults to Silverman's rule per group; `weight`
/// names a column of row weights. `trim`: each group's curve over its own values' extent instead
/// (violins end at their data); `extend`: the extent widened by this many bandwidths (tails taper).
fn kde_table(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let field = s(op, "field")?;
    let vals = nums(t, field)?;
    let groupby: Vec<String> = match op.get("groupby") {
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|g| g.as_str().map(String::from)).collect(),
        Some(serde_json::Value::String(g)) => vec![g.clone()],
        _ => Vec::new(),
    };
    for g in &groupby {
        if t.column(g).is_none() {
            return Err(format!("no column `{g}` to group by"));
        }
    }
    let weights = match op.get("weight").and_then(|w| w.as_str()) {
        Some(c) => Some(nums(t, c)?),
        None => None,
    };
    let steps = op.get("steps").and_then(|v| v.as_u64()).unwrap_or(64).clamp(2, 2048) as usize;
    let bandwidth = num(r, op, "bandwidth", cx, 0.0);
    let names = as_names(op, ["value", "density"]);
    // Rows by group, in order of first appearance.
    let mut order: Vec<String> = Vec::new();
    let mut members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for i in 0..t.len() {
        let k = groupby.iter().map(|g| cell_string(t, g, i)).collect::<Vec<_>>().join("\u{1f}");
        if !members.contains_key(&k) {
            order.push(k.clone());
        }
        members.entry(k).or_default().push(i);
    }
    let group_vals: Vec<Vec<f64>> = order.iter().map(|k| members[k].iter().map(|&i| vals[i]).collect()).collect();
    // Each group's bandwidth: the one given, else Silverman's rule over its values.
    let bw: Vec<f64> = group_vals.iter().map(|v| if bandwidth.is_finite() && bandwidth > 0.0 { bandwidth } else { algo::silverman_bandwidth(v) }).collect();
    let extent = |v: &[f64]| {
        let f = || v.iter().copied().filter(|x| x.is_finite());
        (f().fold(f64::INFINITY, f64::min), f().fold(f64::NEG_INFINITY, f64::max))
    };
    // The grid: `extent` when given; else the values' extent — every group's own with `trim`, the
    // whole table's otherwise (one grid, so curves line up) — widened by `extend` bandwidths (the
    // widest group's on a shared grid), and by three when all the values are one (a spike still
    // gets a curve around it).
    let extend = num(r, op, "extend", cx, 0.0).max(0.0);
    let trim = op.get("trim").and_then(|v| v.as_bool()).unwrap_or(false);
    let explicit = op.get("extent").and_then(|e| e.as_array()).filter(|e| e.len() == 2);
    let widen = |(lo, hi): (f64, f64), h: f64| {
        let k = if lo == hi { extend.max(3.0) } else { extend };
        let h = if h.is_finite() { h } else { 0.0 };
        (lo - k * h, hi + k * h)
    };
    let shared = match explicit {
        Some(e) => {
            let (lo, hi) = extent(&vals);
            (r.num(&Prop(e[0].clone()), cx, lo), r.num(&Prop(e[1].clone()), cx, hi))
        }
        None => widen(extent(&vals), bw.iter().copied().filter(|h| h.is_finite()).fold(0.0, f64::max)),
    };
    let grid_of = |(lo, hi): (f64, f64)| if lo.is_finite() && hi.is_finite() { algo::kde::grid(lo, hi, steps) } else { Vec::new() };
    let shared_grid = grid_of(shared);
    let (mut firsts, mut value, mut density, mut sample) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (g, k) in order.iter().enumerate() {
        let rows = &members[k];
        let w: Option<Vec<f64>> = weights.as_ref().map(|w| rows.iter().map(|&i| w[i]).collect());
        let own;
        let at = if trim && explicit.is_none() {
            own = grid_of(widen(extent(&group_vals[g]), bw[g]));
            &own
        } else {
            &shared_grid
        };
        let d = algo::kde(&group_vals[g], w.as_deref(), bw[g], at);
        for (j, (x, y)) in at.iter().zip(&d).enumerate() {
            firsts.push(rows[0]);
            value.push(*x);
            density.push(*y);
            sample.push(j as f64);
        }
    }
    let mut cols: Vec<(String, Column)> = groupby.iter().map(|g| (g.clone(), t.column(g).expect("checked").take(&firsts))).collect();
    cols.push((names[0].clone(), Column::Num(value)));
    cols.push((names[1].clone(), Column::Num(density)));
    cols.push(("__sample".to_string(), Column::Num(sample)));
    let mut key: Vec<&str> = groupby.iter().map(|g| g.as_str()).collect();
    key.push("__sample");
    Table::from_columns("kde", cols).and_then(|t| t.with_key(&key)).map_err(|e| e.to_string())
}
