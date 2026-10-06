//! Binning table ops for big data: rows into hexagons (`hexbin`), grid cells (`bin2d`) and density
//! contours (`contours`), and rows into path data (`paths`). Thin adapters over datars-algo
//! (`hexbin`, `grid_bins`, `bin_values`, `density`, `mass_thresholds`, `contour`).
//!
//! The binning ops work in the rows' own units and read nothing but rows and numbers, so their
//! tables are pure: computed once per data and kept across resolves — every state, layout pass and
//! hover reuses them — and a mark draws a bin per row of the result, thousands instead of millions.
//! A lattice's resolution is a number the recipe picks from its box when it expands (cells of a
//! few px on this screen). `paths` is the per-resolve half: a few thousand vertices through the
//! scales into one path per group.

use crate::resolve::{value_num, Cx, Resolver};
use crate::tables::{cell_string, eval_rows};
use datars_algo as algo;
use datars_data::{Column, Table};
use datars_ir::Prop;
use datars_math::{Rect, Vec2};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

/// Most cells a grid op makes along one side (a 2000 × 2000 density is 4M cells: far past a screen).
const MAX_SIDE: f64 = 2000.0;

fn num(r: &Resolver, op: &serde_json::Value, k: &str, cx: &Cx, d: f64) -> f64 {
    match op.get(k) {
        None | Some(serde_json::Value::Null) => d,
        Some(v) => r.num(&Prop(v.clone()), cx, d),
    }
}

fn field<'a>(op: &'a serde_json::Value, k: &str) -> Result<&'a str, String> {
    op.get(k).and_then(|v| v.as_str()).ok_or_else(|| format!("needs `{k}` (a column)"))
}

fn column(t: &Table, name: &str) -> Result<Vec<f64>, String> {
    Ok(t.column(name).ok_or_else(|| format!("no column `{name}`"))?.to_f64())
}

fn names(op: &serde_json::Value, k: &str) -> Vec<String> {
    match op.get(k) {
        Some(serde_json::Value::Array(a)) => a.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// The finite extent of values, or `None` when there are none.
fn span(v: &[f64]) -> Option<(f64, f64)> {
    let (lo, hi) = v.iter().filter(|x| x.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    (lo <= hi).then_some((lo, hi))
}

/// The extent to bin over, in the rows' units: `extent: [x0, y0, x1, y1]` (numbers or expressions)
/// when given, else the rows' own, widened by `pad` of its size on each side. A degenerate side
/// (every row at one value) gets a unit of room, so there's always a grid to put it in.
fn extent(r: &Resolver, op: &serde_json::Value, cx: &Cx, xs: &[f64], ys: &[f64], pad: f64) -> Result<Rect, String> {
    let given: Option<[f64; 4]> = op.get("extent").and_then(|e| e.as_array()).filter(|a| a.len() == 4).map(|a| {
        let mut out = [f64::NAN; 4];
        for (o, v) in out.iter_mut().zip(a) {
            *o = r.num(&Prop(v.clone()), cx, f64::NAN);
        }
        out
    });
    let (x0, x1, y0, y1) = match given.filter(|g| g.iter().all(|v| v.is_finite())) {
        Some([a, b, c, d]) => (a.min(c), a.max(c), b.min(d), b.max(d)),
        None => {
            let (Some((x0, x1)), Some((y0, y1))) = (span(xs), span(ys)) else { return Err("no rows with finite x and y to bin".into()) };
            let (px, py) = ((x1 - x0) * pad, (y1 - y0) * pad);
            (x0 - px, x1 + px, y0 - py, y1 + py)
        }
    };
    let widen = |lo: f64, hi: f64| if hi > lo { (lo, hi) } else { (lo - 0.5, hi + 0.5) };
    let ((x0, x1), (y0, y1)) = (widen(x0, x1), widen(y0, y1));
    Ok(Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// The value column a bin reports: `fn` of `value` over its rows, or its row count.
fn agg_of(op: &serde_json::Value, has_value: bool) -> Result<algo::BinAgg, String> {
    match op.get("fn").and_then(|f| f.as_str()) {
        Some(f) => algo::BinAgg::parse(f).ok_or_else(|| format!("unknown fn `{f}` (count, sum, mean, min, max)")),
        None => Ok(if has_value { algo::BinAgg::Mean } else { algo::BinAgg::Count }),
    }
}

fn table(name: &str, cols: Vec<(&str, Column)>, key: &[&str]) -> Result<Table, String> {
    let t = Table::from_columns(name, cols).map_err(|e| e.to_string())?;
    if key.is_empty() {
        Ok(t)
    } else {
        t.with_key(key).map_err(|e| e.to_string())
    }
}

fn strings(v: Vec<String>) -> Column {
    Column::Str(v.into_iter().map(|s| Some(Arc::from(s.as_str()))).collect())
}

pub(super) fn apply(r: &Resolver, t: &Table, kind: &str, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    match kind {
        "hexbin" => hexbin(r, t, op, cx),
        "bin2d" => bin2d(r, t, op, cx),
        "contours" => contours(r, t, op, cx),
        "paths" => paths(r, t, op, cx),
        other => Err(format!("unknown table op `{other}`")),
    }
}

/// One row per non-empty hexagon of a lattice `columns` hexagons across the extent, pointy-top,
/// as regular on screen as `aspect` (the extent's height ÷ width where it's drawn) says. Columns:
/// `hex` (the key: lattice size, row, column — a different lattice is different hexagons),
/// `x`, `y` (centre), `count`, `value`, `hw`, `hh` (half the width, and the circumradius up and
/// down — a hexagon's corners are (x, y ± hh) and (x ± hw, y ± hh/2)), `col`, `row`.
fn hexbin(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let (xs, ys) = (column(t, field(op, "x")?)?, column(t, field(op, "y")?)?);
    let ext = extent(r, op, cx, &xs, &ys, 0.0)?;
    let cols = num(r, op, "columns", cx, 30.0);
    let cols = if cols.is_finite() { cols.round().clamp(1.0, MAX_SIDE) } else { 30.0 };
    let aspect = num(r, op, "aspect", cx, 1.0);
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect.clamp(0.01, 100.0) } else { 1.0 };
    // Lattice space: x across `cols` units (one hexagon per unit), y up `cols · aspect` of them —
    // square on screen when the extent is drawn `aspect` times as tall as it is wide.
    let (sx, sy) = (cols / ext.w, cols * aspect / ext.h);
    let pts: Vec<Vec2> = xs.iter().zip(&ys).map(|(x, y)| Vec2::new((x - ext.x) * sx, (y - ext.y) * sy)).collect();
    let radius = 1.0 / 3f64.sqrt();
    let bins = algo::hexbin(&pts, radius, Rect::new(0.0, 0.0, cols, cols * aspect));
    let value = op.get("value").and_then(|v| v.as_str()).map(|f| column(t, f)).transpose()?;
    let agg = agg_of(op, value.is_some())?;
    // Per row, its bin: then one fold for every bin's value.
    let mut cell = vec![algo::GridBins::OUTSIDE; pts.len()];
    for (b, h) in bins.iter().enumerate() {
        for &i in &h.indices {
            cell[i] = b as u32;
        }
    }
    let values = algo::bin_values(&cell, value.as_deref().unwrap_or(&[]), bins.len(), agg);
    let n = bins.len();
    let (mut key, mut x, mut y, mut count, mut col, mut row) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
    for h in &bins {
        let j = (h.center.y / (1.5 * radius)).round();
        let i = (h.center.x - datars_math::m::rem_euclid(j, 2.0) / 2.0).round();
        key.push(format!("{}:{}:{}", cols as i64, j as i64, i as i64));
        x.push(ext.x + h.center.x / sx);
        y.push(ext.y + h.center.y / sy);
        count.push(h.indices.len() as f64);
        col.push(i);
        row.push(j);
    }
    let hw = vec![0.5 / sx; n];
    let hh = vec![radius / sy; n];
    table(
        "hexbin",
        vec![("hex", strings(key)), ("x", Column::Num(x)), ("y", Column::Num(y)), ("count", Column::Num(count)), ("value", Column::Num(values)), ("hw", Column::Num(hw)), ("hh", Column::Num(hh)), ("col", Column::Num(col)), ("row", Column::Num(row))],
        &["hex"],
    )
}

/// One row per non-empty cell (every cell with `empty: true`) of a `columns × rows` grid over the
/// extent, counted in one pass. Columns: `cell` (the key: grid size, row, column), `x0`, `x1`,
/// `y0`, `y1` (the cell's extent), `x`, `y` (its centre), `count`, `value`, `col`, `row` (row 0
/// at the low end of y).
fn bin2d(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let (xs, ys) = (column(t, field(op, "x")?)?, column(t, field(op, "y")?)?);
    let ext = extent(r, op, cx, &xs, &ys, 0.0)?;
    let side = |v: f64, d: f64| if v.is_finite() { v.round().clamp(1.0, MAX_SIDE) as usize } else { d as usize };
    let nx = side(num(r, op, "columns", cx, 50.0), 50.0);
    let ny = side(num(r, op, "rows", cx, nx as f64), nx as f64);
    let g = algo::grid_bins(&xs, &ys, ext, nx, ny);
    let value = op.get("value").and_then(|v| v.as_str()).map(|f| column(t, f)).transpose()?;
    let agg = agg_of(op, value.is_some())?;
    let values = algo::bin_values(&g.cells, value.as_deref().unwrap_or(&[]), g.counts.len(), agg);
    let empty = op.get("empty").and_then(|e| e.as_bool()).unwrap_or(false);
    let mut cols: [Vec<f64>; 10] = Default::default();
    let mut key = Vec::new();
    for (c, (&n, &v)) in g.counts.iter().zip(&values).enumerate() {
        if n == 0 && !empty {
            continue;
        }
        let (i, j) = (c % nx, c / nx);
        let q = g.cell_rect(ext, i, j);
        key.push(format!("{nx}x{ny}:{j}:{i}"));
        for (v, x) in cols.iter_mut().zip([q.x, q.x1(), q.y, q.y1(), q.x + q.w / 2.0, q.y + q.h / 2.0, n as f64, v, i as f64, j as f64]) {
            v.push(x);
        }
    }
    let [x0, x1, y0, y1, x, y, count, value, col, row] = cols;
    let named = ["x0", "x1", "y0", "y1", "x", "y", "count", "value", "col", "row"];
    let mut out: Vec<(&str, Column)> = vec![("cell", strings(key))];
    out.extend(named.into_iter().zip([x0, x1, y0, y1, x, y, count, value, col, row].map(Column::Num)));
    table("bin2d", out, &["cell"])
}

/// Density contours of the rows: a Gaussian density of their points (or `weight`) on a
/// `columns × rows` grid over the extent (the rows' own, widened by `pad`, 0.05 by default),
/// `bandwidth` cells wide, cut at `levels` thresholds — by default where the region above holds
/// 1/(levels+1), 2/(levels+1), … of the rows (`by: "density"`: evenly spaced up to the peak;
/// `shares`: exactly those shares). One row per ring vertex, in ring order: `level` (0 the lowest
/// density, the largest region), `ring` (numbered across levels), `x`, `y`, `share` (of the rows
/// inside the level) and `density` (the threshold, in rows per unit² of x × y). Outer rings run
/// clockwise on screen (y up in data, down on screen) and holes the other way, so each level
/// drawn as one path (`paths` by level) fills with its holes.
fn contours(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let (xs, ys) = (column(t, field(op, "x")?)?, column(t, field(op, "y")?)?);
    let pad = num(r, op, "pad", cx, 0.05);
    let ext = extent(r, op, cx, &xs, &ys, if pad.is_finite() { pad.clamp(0.0, 1.0) } else { 0.05 })?;
    let side = |v: f64, d: f64| if v.is_finite() { v.round().clamp(2.0, 1000.0) as usize } else { d as usize };
    let nx = side(num(r, op, "columns", cx, 100.0), 100.0);
    let ny = side(num(r, op, "rows", cx, nx as f64), nx as f64);
    let bw = num(r, op, "bandwidth", cx, 2.0);
    let weights = op.get("weight").and_then(|v| v.as_str()).map(|f| column(t, f)).transpose()?;
    // Grid space: sample (i, j) at the centre of cell i across, j down from the extent's top.
    let (sx, sy) = (nx as f64 / ext.w, ny as f64 / ext.h);
    let pts: Vec<Vec2> = xs.iter().zip(&ys).map(|(x, y)| Vec2::new((x - ext.x) * sx, (ext.y1() - y) * sy)).collect();
    let grid = algo::density(&pts, weights.as_deref(), bw, nx, ny, Rect::new(0.0, 0.0, nx as f64, ny as f64));
    let total: f64 = grid.iter().filter(|v| v.is_finite() && **v > 0.0).sum();
    let peak = grid.iter().copied().filter(|v| v.is_finite()).fold(0.0, f64::max);
    let levels = num(r, op, "levels", cx, 5.0);
    let levels = if levels.is_finite() { levels.round().clamp(1.0, 50.0) as usize } else { 5 };
    let given: Vec<f64> = op.get("shares").and_then(|s| s.as_array()).map(|a| a.iter().filter_map(|v| v.as_f64()).filter(|v| *v > 0.0 && *v <= 1.0).collect()).unwrap_or_default();
    let mut thresholds: Vec<f64> = if !given.is_empty() {
        algo::mass_thresholds(&grid, &given)
    } else if op.get("by").and_then(|b| b.as_str()) == Some("density") {
        (1..=levels).map(|k| peak * k as f64 / (levels + 1) as f64).collect()
    } else {
        algo::mass_thresholds(&grid, &(1..=levels).rev().map(|k| k as f64 / (levels + 1) as f64).collect::<Vec<_>>())
    };
    thresholds.retain(|v| v.is_finite() && *v > 0.0);
    thresholds.sort_by(|a, b| datars_math::total_cmp(*a, *b));
    thresholds.dedup();
    let cell_area = (ext.w / nx as f64) * (ext.h / ny as f64);
    let mut cols: [Vec<f64>; 6] = Default::default();
    let mut ring_no = 0.0;
    for (level, (thr, rings)) in algo::contour(&grid, nx, ny, &thresholds).into_iter().enumerate() {
        let inside: f64 = grid.iter().filter(|v| **v >= thr).sum();
        let share = if total > 0.0 { inside / total } else { f64::NAN };
        for ring in rings {
            for p in ring {
                for (v, x) in cols.iter_mut().zip([level as f64, ring_no, ext.x + p.x / sx, ext.y1() - p.y / sy, share, thr / cell_area]) {
                    v.push(x);
                }
            }
            ring_no += 1.0;
        }
    }
    let [level, ring, x, y, share, density] = cols;
    table("contours", vec![("level", Column::Num(level)), ("ring", Column::Num(ring)), ("x", Column::Num(x)), ("y", Column::Num(y)), ("share", Column::Num(share)), ("density", Column::Num(density))], &[])
}

/// One row per group of rows (`by`: columns; none — one group of every row), its rows' points
/// (`x`, `y`: expressions, usually through the scales) joined in row order into SVG path data in
/// column `as` (default `path`) — a new subpath where `ring` changes and after a missing point,
/// each closed with `closed`. Keeps the group's first row's other columns, adds `end_x`, `end_y`
/// (its last point: where a label goes) and is keyed by `by`. Thousands of series drawn as a path
/// each, contour levels with their holes.
fn paths(r: &Resolver, t: &Table, op: &serde_json::Value, cx: &Cx) -> Result<Table, String> {
    let by = names(op, "by");
    for b in &by {
        if t.column(b).is_none() {
            return Err(format!("no column `{b}`"));
        }
    }
    let at = |k: &str| -> Vec<f64> { eval_rows(r, t, &Prop(op.get(k).cloned().unwrap_or_default()), cx).iter().map(value_num).collect() };
    let (xs, ys) = (at("x"), at("y"));
    let ring = op.get("ring").and_then(|v| v.as_str());
    let rings: Option<Vec<f64>> = match ring {
        Some(c) => Some(column(t, c)?),
        None => None,
    };
    let closed = op.get("closed").and_then(|c| c.as_bool()).unwrap_or(false);
    // Groups in order of first appearance. Rows usually come a series at a time: a row whose
    // group columns equal the previous row's joins its group without building a key.
    let cols: Vec<&Column> = by.iter().filter_map(|b| t.column(b)).collect();
    let mut order: Vec<Vec<usize>> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut last = 0;
    for i in 0..t.len() {
        let g = if i > 0 && cols.iter().all(|c| c.get(i) == c.get(i - 1)) {
            last
        } else {
            let k = by.iter().map(|b| cell_string(t, b, i)).collect::<Vec<_>>().join("\u{1f}");
            *index.entry(k).or_insert_with(|| {
                order.push(Vec::new());
                order.len() - 1
            })
        };
        order[g].push(i);
        last = g;
    }
    let (mut data, mut end_x, mut end_y) = (Vec::with_capacity(order.len()), Vec::with_capacity(order.len()), Vec::with_capacity(order.len()));
    for rows in &order {
        let mut d = String::with_capacity(rows.len() * 14);
        let (mut down, mut last_ring, mut end) = (false, f64::NAN, (f64::NAN, f64::NAN));
        for &i in rows {
            let (x, y) = (xs[i], ys[i]);
            if let Some(rs) = &rings {
                // `!=` on the raw bits: two NaN ring ids are the same ring.
                if rs[i].to_bits() != last_ring.to_bits() {
                    if closed && down {
                        d.push('Z');
                    }
                    down = false;
                    last_ring = rs[i];
                }
            }
            if !(x.is_finite() && y.is_finite()) {
                if closed && down {
                    d.push('Z');
                }
                down = false;
                continue;
            }
            let _ = write!(d, "{}{:.1} {:.1}", if down { 'L' } else { 'M' }, x, y);
            down = true;
            end = (x, y);
        }
        if closed && down {
            d.push('Z');
        }
        data.push(d);
        end_x.push(end.0);
        end_y.push(end.1);
    }
    let firsts: Vec<usize> = order.iter().map(|rows| rows[0]).collect();
    let mut out = t.take_rows(&firsts);
    let as_ = op.get("as").and_then(|a| a.as_str()).unwrap_or("path");
    for (name, col) in [(as_, strings(data)), ("end_x", Column::Num(end_x)), ("end_y", Column::Num(end_y))] {
        out = datars_data::transform::derive(&out, name, col).map_err(|e| e.to_string())?;
    }
    let key: Vec<&str> = by.iter().map(|b| b.as_str()).collect();
    if key.is_empty() {
        out.key.clear();
        Ok(out)
    } else {
        out.with_key(&key).map_err(|e| e.to_string())
    }
}
