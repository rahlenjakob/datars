//! The frame pass for point pyramids: which tiles a view shows, at which levels, and their nodes.
//!
//! Levels are drawn coarse to fine, each adding its sample of the rows, while the rows in view
//! fit the frame's budget: the next level multiplies what's drawn by its fraction's growth (×4
//! until every row is in), so it's drawn only when that prediction fits `points`. A dense view
//! stops early (a uniform sample of it), a sparse one goes on to every row — the same everywhere
//! in the view, so no seam shows. The last level drawn fades in as the prediction drops from the
//! budget to a fraction of it (as the reader zooms in, fewer rows are in view), so points arrive
//! smoothly rather than in steps. While a level's tiles are still on their way, deeper levels
//! wait: coarser levels keep showing — never a hole — and the frame, once everything is in, is
//! the one an archive in memory gives. All of it is a function of the frame's camera and the
//! data, like everything else here.

use super::pyramid::{Header, PointTile, Pyramid};
use super::{Binding, Indexed, Origin, Store};
use crate::resolve::{Cx, Resolver};
use crate::tiles::{BuildEnv, FillCx, FillReport, RangeFetch, TileState};
use datars_geo::TileId;
use datars_math::{m, Affine, Rect, Vec2};
use datars_scene::{Key, KeyPart, KeyPath, Node, NodeKind};
use std::rc::Rc;
use std::sync::Arc;

/// Seconds new detail (a tile below the root arriving, or first reached) takes to fade in.
const ARRIVE_S: f64 = 0.35;

/// Tiles one level may draw at once; a view asking for more (a camera zoomed far out of its
/// content's scale) stops at the level before.
const MAX_TILES_PER_LEVEL: usize = 256;
/// The smallest a tile gets on screen (px): no level deeper than that is fetched, so a sparse
/// view costs a bounded number of tiles.
const MIN_TILE_PX: f64 = 96.0;
/// Cells a built tile splits into at most (`2^6 × 2^6`), so a view inside a big tile draws its
/// part only; a cell is about this many px across on screen.
const MAX_SPLIT: u8 = 6;
const CELL_PX: f64 = 256.0;

/// A tile's data, wherever it lives.
pub(crate) enum Look {
    Ready(Arc<PointTile>),
    Pending,
    /// Here, but this frame's decode budget is spent: next frame.
    Later,
    Absent,
}

/// The pyramid a binding draws: in memory (built from its table), or an archive read by range.
enum Source {
    Memory(Rc<Pyramid>),
    Archive,
}

pub(crate) fn pyramid_of(env: &BuildEnv, st: &mut Store, b: &Binding, table: &Arc<datars_data::Table>) -> Rc<Pyramid> {
    let key = super::build_key(&b.spec, b.budget);
    if let Some(built) = st.from_tables.get_mut(&(b.from.clone(), key)) {
        if Arc::ptr_eq(&built.table, table) || *built.table == **table {
            built.table = table.clone();
            return built.pyramid.clone();
        }
    }
    // Index the table: positions evaluated once per row (they read the row only), the columns
    // the template reads stored with each row.
    let store = crate::tables::TableStore::new(env.sources.clone(), env.doc.tables.clone(), Default::default());
    let signals = std::collections::BTreeMap::new();
    let r = Resolver::new(env.doc, &signals, env.theme, env.fonts, None, env.size_class, env.geo, store);
    let cx = base_cx();
    let xs = r.column_nums(&b.spec.x, table, &cx);
    let ys = r.column_nums(&b.spec.y, table, &cx);
    let names = super::stored_columns(table);
    let cols: Vec<(String, &datars_data::Column)> = names.iter().filter_map(|n| table.column(n).map(|c| (n.clone(), c))).collect();
    let p = Rc::new(super::pyramid::build(&xs, &ys, &cols, &super::Options::with_budget(b.budget)));
    st.stats.indexed += 1;
    st.from_tables.insert((b.from.clone(), key), Indexed { table: table.clone(), pyramid: p.clone() });
    p
}

fn base_cx() -> Cx {
    Cx {
        box_w: 0.0,
        box_h: 0.0,
        scope: Rc::new(crate::scales::Scope::default()),
        row: None,
        fields: None,
        group: None,
        path: KeyPath::default(),
        depth: 0,
        proj: None,
        default_key: None,
        tile: None,
        recipes: None,
        interactive: None,
    }
}

/// Look a tile up in an archive: decoded if its bytes are in, requested otherwise.
pub(crate) fn archive_tile(ts: &mut TileState, source: &str, t: TileId, fetch: Option<&RangeFetch>) -> Look {
    let key = (source.to_string(), t);
    if let Some(d) = ts.points.decoded.get(&key) {
        return Look::Ready(d);
    }
    if ts.points.absent.contains(&key) {
        return Look::Absent;
    }
    match ts.raw_or_waiting(source, t, fetch) {
        // This frame's decode budget spent (shared with map tiles): next frame; coarser levels
        // keep showing meanwhile, and the bytes wait.
        crate::tiles::Raw::Ready(bytes) if ts.over_budget(bytes.len()) => {
            ts.wait(source, t, bytes);
            Look::Later
        }
        crate::tiles::Raw::Ready(bytes) => match super::codec::decode(t, &bytes) {
            Ok(d) => {
                ts.decode_left = ts.decode_left.map(|left| left.saturating_sub(bytes.len()));
                ts.decoded_now += 1;
                let d = Arc::new(d);
                ts.points.decoded.insert(key, d.clone());
                Look::Ready(d)
            }
            Err(e) => {
                ts.diags.push(format!("points `{source}` {}/{}/{}: {e}", t.z, t.x, t.y));
                ts.points.absent.insert(key);
                Look::Absent
            }
        },
        crate::tiles::Raw::Pending => Look::Pending,
        crate::tiles::Raw::Absent => {
            ts.points.absent.insert(key);
            Look::Absent
        }
    }
}

fn look(ts: &mut TileState, src: &Source, from: &str, t: TileId, fetch: Option<&RangeFetch>) -> Look {
    match src {
        Source::Memory(p) => p.tiles.get(&t).map_or(Look::Absent, |tile| Look::Ready(tile.clone())),
        Source::Archive => archive_tile(ts, from, t, fetch),
    }
}

/// The tiles of level `z` covering `r` (content units), in row-major order.
fn covering(h: &Header, z: u8, r: Rect) -> Vec<TileId> {
    let n = (1u64 << z) as f64;
    let ts = h.extent.size / n;
    let cell = |v: f64, o: f64| ((v - o) / ts).floor().clamp(0.0, n - 1.0) as u32;
    let last = |v: f64, o: f64| (((v - o) / ts).ceil() - 1.0).clamp(0.0, n - 1.0) as u32;
    let (x0, y0) = (cell(r.x, h.extent.x0), cell(r.y, h.extent.y0));
    let (x1, y1) = (last(r.x1(), h.extent.x0).max(x0), last(r.y1(), h.extent.y0).max(y0));
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push(TileId::new(z, x, y));
        }
    }
    out
}

/// Fill one `lod` placeholder whose content goes to the screen through `xf`, clipped to `clip`.
/// With `draw` false, only look the tiles up (requesting what's missing): prefetching.
pub(crate) fn fill_one(cx: &FillCx, b: &Binding, xf: Affine, clip: Rect, draw: bool, report: &mut FillReport) -> Vec<Node> {
    let mut guard = cx.state.borrow_mut();
    let ts = &mut *guard;
    let src = match &b.origin {
        Origin::Table(t) => Source::Memory(pyramid_of(&cx.env, &mut ts.points, b, t)),
        Origin::Archive => Source::Archive,
    };
    // Every tile says what the pyramid is; the root is always drawn, so it comes first.
    let root = TileId::new(0, 0, 0);
    let header = match look(ts, &src, &b.from, root, cx.fetch) {
        Look::Ready(t) => t.header.clone(),
        Look::Pending => {
            report.pending += 1;
            return Vec::new();
        }
        Look::Later => {
            report.building += 1;
            return Vec::new();
        }
        Look::Absent => return Vec::new(),
    };
    let Some(inv) = xf.inverse() else { return Vec::new() };
    if clip.is_empty() || clip.w <= 0.0 || clip.h <= 0.0 {
        return Vec::new();
    }
    let corners = [Vec2::new(clip.x, clip.y), Vec2::new(clip.x1(), clip.y), Vec2::new(clip.x, clip.y1()), Vec2::new(clip.x1(), clip.y1())];
    let seen = corners.iter().fold(Rect::empty(), |r, c| r.include(inv.apply(*c)));
    let e = header.extent;
    let Some(view) = seen.intersect(&Rect::new(e.x0, e.y0, e.size, e.size)) else { return Vec::new() };
    let scale = xf.scale_factor();
    let px = e.size * scale;
    // How deep tiles may go at this zoom (fractional: a level allowed by it fades in as the zoom
    // brings its tiles up to MIN_TILE_PX, like one allowed by the budget).
    let depth = if px > 0.0 && px.is_finite() { m::log2(px / MIN_TILE_PX) } else { 0.0 };
    let deepest = (depth.ceil().max(0.0) as u8).min(header.levels);
    let full = datars_algo::pyramid::full_level(header.budget, header.rows);
    let budget = b.max_points * cx.point_scale;
    // Levels coarse to fine, each with its fade: the cells of it in view.
    let mut levels: Vec<(Vec<Node>, f64)> = Vec::new();
    // Rows of the levels so far inside the view (exactly: the decisions follow the camera
    // smoothly), and rows drawn (whole cells: a cell straddling the view's edge draws in full).
    let mut in_view = 0usize;
    let mut points = 0usize;
    let mut tiles_drawn = 0u32;
    let mut waiting = false;
    for z in 0..=deepest {
        let mut fade = 1.0;
        if z > 0 {
            // Deeper only once every tile so far is in (the decision reads their rows).
            if waiting {
                break;
            }
            let f = |l: u8| datars_algo::pyramid::fraction(header.budget, header.rows, l);
            // Beyond the full level only capped regions have rows left: at most as many again.
            let growth = if z > full { 2.0 } else { f(z) / f(z - 1) };
            let predicted = in_view as f64 * growth;
            if predicted > budget {
                break;
            }
            let by_budget = if growth > 1.0 && predicted > 0.0 { (m::ln(budget / predicted) / m::ln(growth)).clamp(0.0, 1.0) } else { 1.0 };
            let by_zoom = (depth - (z as f64 - 1.0)).clamp(0.0, 1.0);
            // Never stronger than the level above it.
            fade = by_budget.min(by_zoom).min(levels.last().map_or(1.0, |l: &(Vec<Node>, f64)| l.1));
        }
        let want = covering(&header, z, view);
        if want.len() > MAX_TILES_PER_LEVEL {
            break;
        }
        report.wanted += want.len() as u32;
        // Split each tile into cells about CELL_PX on screen; only the cells in view are drawn.
        let tile_px = e.size / (1u64 << z) as f64 * scale;
        let split = if tile_px > CELL_PX { m::log2(tile_px / CELL_PX).ceil().clamp(0.0, MAX_SPLIT as f64) as u8 } else { 0 };
        // A level past the first draws once all of it is prepared: what it lacks is built within
        // the frame's budget, and if that runs out it waits — the levels above keep showing,
        // never a seam — and the host draws another frame to go on.
        if z > 0 && draw && !prepare_level(&cx.env, ts, &src, b, &want, split, view, &header, cx.fetch) {
            report.building += 1;
            break;
        }
        let mut nodes = Vec::new();
        for t in want {
            match look(ts, &src, &b.from, t, cx.fetch) {
                Look::Ready(tile) => {
                    report.ready += 1;
                    tiles_drawn += 1;
                    // New detail eases in from the first frame that draws it — data arriving
                    // mid-zoom would otherwise flash in. Keyed by tile (a zoom re-splits cells).
                    let arrive = match cx.now {
                        Some(now) if z > 0 && draw => {
                            let first = *ts.points.first_drawn.entry((b.from.clone(), t)).or_insert(now);
                            let k = ((now - first) / ARRIVE_S).clamp(0.0, 1.0);
                            if k < 1.0 {
                                report.building += 1;
                            }
                            k * k * (3.0 - 2.0 * k)
                        }
                        _ => 1.0,
                    };
                    let first_node = nodes.len();
                    let ord = ordered(ts, &b.from, &tile);
                    let n = 1u32 << split;
                    let side = e.size / (1u64 << z) as f64 / n as f64;
                    let (ox, oy) = (e.x0 + t.x as f64 * side * n as f64, e.y0 + t.y as f64 * side * n as f64);
                    let span = |lo: f64, hi: f64, o: f64| (((lo - o) / side).floor().clamp(0.0, (n - 1) as f64) as u32, ((hi - o) / side).floor().clamp(0.0, (n - 1) as f64) as u32);
                    let (cx0, cx1) = span(view.x, view.x1(), ox);
                    let (cy0, cy1) = span(view.y, view.y1(), oy);
                    for cy in cy0..=cy1 {
                        for cxi in cx0..=cx1 {
                            let range = ord.cell(split, cxi, cy);
                            if range.is_empty() {
                                continue;
                            }
                            let r = Rect::new(ox + cxi as f64 * side, oy + cy as f64 * side, side, side);
                            points += range.len();
                            in_view += if r.x >= view.x && r.y >= view.y && r.x1() <= view.x1() && r.y1() <= view.y1() {
                                range.len()
                            } else {
                                range.clone().filter(|&k| view.contains(Vec2::new(ord.xs[k], ord.ys[k]))).count()
                            };
                            if draw {
                                nodes.push(cell_node(&cx.env, ts, b, &ord, t, split, cxi, cy, range));
                            }
                        }
                    }
                    if arrive < 1.0 {
                        for n in &mut nodes[first_node..] {
                            n.common.opacity *= arrive;
                        }
                    }
                }
                Look::Pending => {
                    report.pending += 1;
                    waiting = true;
                }
                Look::Later => {
                    report.building += 1;
                    waiting = true;
                }
                Look::Absent => {}
            }
        }
        levels.push((nodes, fade));
    }
    ts.points.stats.rows = header.rows;
    ts.points.stats.drawn = points as u64;
    ts.points.stats.tiles = tiles_drawn;
    ts.points.stats.level = levels.len().saturating_sub(1) as u8;
    let mut out = Vec::with_capacity(levels.iter().map(|l| l.0.len()).sum());
    for (nodes, fade) in levels {
        for mut n in nodes {
            if fade < 1.0 {
                n.common.opacity *= fade;
            }
            out.push(n);
        }
    }
    out
}

/// A tile of a binding's pyramid, if it's in memory (the frame that drew it had it).
fn tile_of(cx: &BuildEnv, ts: &mut TileState, b: &Binding, t: TileId) -> Option<Arc<PointTile>> {
    let src = match &b.origin {
        Origin::Table(table) => Source::Memory(pyramid_of(cx, &mut ts.points, b, table)),
        Origin::Archive => Source::Archive,
    };
    match look(ts, &src, &b.from, t, None) {
        Look::Ready(tile) => Some(tile),
        _ => None,
    }
}

/// The label of a drawn row: tile `t`'s row `row` (its number in the source) through the
/// template's `label`, built for that one row (see `Resolver::lod_instances`).
pub(crate) fn label(env: &BuildEnv, ts: &mut TileState, b: &Binding, t: TileId, row: u64) -> Option<String> {
    let tile = tile_of(env, ts, b, t)?;
    let k = tile.row.binary_search(&row).ok()?;
    let table = Arc::new(tile.table(&b.from));
    let store = crate::tables::TableStore::new(env.sources.clone(), env.doc.tables.clone(), Default::default());
    let r = Resolver::new(env.doc, &b.signals, env.theme, env.fonts, None, env.size_class, env.geo, store).with_exprs(&ts.exprs());
    r.lod_label(&b.spec, &table, k, &base_cx())
}

/// A tile's rows reordered along a Morton (Z-order) curve of their positions in the tile: every
/// cell of every split is then one contiguous run of rows, found by binary search — no per-split
/// index — and only the cells a view shows are ever run through a template.
pub(crate) struct Ordered {
    /// The rows as a table (`x`, `y`, `$row`, stored columns), in Morton order.
    table: Arc<datars_data::Table>,
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    /// Each row's Morton code (sorted), over `bits` bits per axis.
    morton: Vec<u128>,
    bits: u32,
}

impl Ordered {
    /// The rows of cell (`cx`, `cy`) of a `2^split × 2^split` split.
    pub fn cell(&self, split: u8, cx: u32, cy: u32) -> std::ops::Range<usize> {
        if split == 0 {
            return 0..self.morton.len();
        }
        let s = split as u32;
        if s > self.bits {
            return 0..0;
        }
        let shift = 2 * (self.bits - s);
        let code = interleave(cx as u64, cy as u64);
        let (lo, hi) = (code << shift, (code + 1) << shift);
        self.morton.partition_point(|&m| m < lo)..self.morton.partition_point(|&m| m < hi)
    }
}

/// Bits of `x` and `y` interleaved (x in the even bits): a Morton code.
fn interleave(x: u64, y: u64) -> u128 {
    /// The 32 bits of `v` spread to the even bits of a u64 (the usual magic-mask steps).
    fn spread32(v: u64) -> u64 {
        let mut v = v & 0xFFFF_FFFF;
        v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
        v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
        v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
        v = (v | (v << 2)) & 0x3333_3333_3333_3333;
        (v | (v << 1)) & 0x5555_5555_5555_5555
    }
    let spread = |v: u64| -> u128 { spread32(v) as u128 | ((spread32(v >> 32) as u128) << 64) };
    spread(x) | (spread(y) << 1)
}

/// A tile's rows in Morton order, cached.
fn ordered(ts: &mut TileState, from: &str, tile: &Arc<PointTile>) -> Rc<Ordered> {
    let key = (from.to_string(), tile.id);
    if let Some(o) = ts.points.ordered.get(&key) {
        return o;
    }
    spend(ts, tile.len());
    let t = tile.id;
    let cells = tile.header.tile_cells(t.z);
    let (ox, oy) = (t.x as u64 * cells, t.y as u64 * cells);
    let codes: Vec<u128> = (0..tile.len()).map(|k| interleave(tile.gx[k] - ox, tile.gy[k] - oy)).collect();
    let mut order: Vec<usize> = (0..tile.len()).collect();
    order.sort_by_key(|&k| (codes[k], k));
    let table = crate::tables::take_rows(&tile.table(from), &order);
    let (xs, ys) = (table.num("x").map(<[f64]>::to_vec).unwrap_or_default(), table.num("y").map(<[f64]>::to_vec).unwrap_or_default());
    let o = Rc::new(Ordered { table: Arc::new(table), xs, ys, morton: order.iter().map(|&k| codes[k]).collect(), bits: cells.trailing_zeros() });
    ts.points.ordered.insert(key, o.clone());
    o
}

/// The split a tile's rows go through the template in: every cell drawn at any split is a slice of
/// one of these (finer splits) or a union of some (coarser), so zooming re-cuts instances but
/// never re-evaluates the template.
const CANON: u8 = 3;

/// Canonical cell `c` (its Morton code at split [`CANON`]) of a tile: its rows through the
/// binding's template, built once and cached.
fn canon(env: &BuildEnv, ts: &mut TileState, b: &Binding, ord: &Ordered, t: TileId, c: u32) -> Arc<datars_scene::Instances> {
    let key = (b.fingerprint, b.from.clone(), t, c);
    if let Some(i) = ts.points.canon.get(&key) {
        return i;
    }
    let (cx, cy) = deinterleave(c as u64);
    let range = ord.cell(CANON, cx, cy);
    spend(ts, range.len());
    let table = Arc::new(crate::tables::take_rows(&ord.table, &range.collect::<Vec<_>>()));
    let store = crate::tables::TableStore::new(env.sources.clone(), env.doc.tables.clone(), Default::default());
    let r = Resolver::new(env.doc, &b.signals, env.theme, env.fonts, None, env.size_class, env.geo, store).with_exprs(&ts.exprs());
    let inst = Arc::new(r.lod_instances(&b.spec, &table, &base_cx()));
    for d in r.diags.take() {
        if !ts.diags.contains(&d.message) {
            ts.diags.push(d.message);
        }
    }
    ts.points.stats.built += 1;
    ts.points.canon.insert(key, inst.clone());
    inst
}

/// The column and row of a Morton code (the inverse of [`interleave`], 32 bits a side).
fn deinterleave(code: u64) -> (u32, u32) {
    fn squash(v: u64) -> u32 {
        let mut v = v & 0x5555_5555_5555_5555;
        v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
        v = (v | (v >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
        v = (v | (v >> 4)) & 0x00FF_00FF_00FF_00FF;
        v = (v | (v >> 8)) & 0x0000_FFFF_0000_FFFF;
        ((v | (v >> 16)) & 0xFFFF_FFFF) as u32
    }
    (squash(code), squash(code >> 1))
}

/// The node drawing one cell of a tile (the rows `range` in Morton order), cut from its canonical
/// cells and cached.
#[allow(clippy::too_many_arguments)]
fn cell_node(env: &BuildEnv, ts: &mut TileState, b: &Binding, ord: &Ordered, t: TileId, split: u8, cx: u32, cy: u32, range: std::ops::Range<usize>) -> Node {
    let whole = range.len() == ord.morton.len();
    let (split, cx, cy) = if whole { (0, 0, 0) } else { (split, cx, cy) };
    let key = (b.fingerprint, b.from.clone(), t, split, cx, cy);
    if let Some(n) = ts.points.cells.get(&key) {
        return n;
    }
    let inst = if split >= CANON {
        // Inside one canonical cell: a slice of it.
        let c = canon_cells(ord, split, cx, cy)[0];
        let whole_c = canon(env, ts, b, ord, t, c);
        let (ccx, ccy) = deinterleave(c as u64);
        let start = ord.cell(CANON, ccx, ccy).start;
        if range.len() == whole_c.len() {
            whole_c
        } else {
            Arc::new(whole_c.take(&(range.start - start..range.end - start).collect::<Vec<_>>()))
        }
    } else {
        // Several canonical cells: their union, in Morton order.
        let parts: Vec<Arc<datars_scene::Instances>> = canon_cells(ord, split, cx, cy).into_iter().map(|c| canon(env, ts, b, ord, t, c)).collect();
        match parts.len() {
            1 => parts[0].clone(),
            _ => Arc::new(datars_scene::Instances::concat(&parts.iter().map(|p| &**p).collect::<Vec<_>>())),
        }
    };
    let n = Node::new(tile_key(t, split, cx, cy), NodeKind::Instances(inst));
    ts.points.cells.insert(key, n.clone());
    n
}

/// The canonical cells (Morton codes at split [`CANON`]) a cell at `split` is cut from: the one it
/// lies in, or the non-empty ones it covers.
fn canon_cells(ord: &Ordered, split: u8, cx: u32, cy: u32) -> Vec<u32> {
    let canon_bits = CANON as u32;
    if split >= CANON {
        return vec![(interleave(cx as u64, cy as u64) >> (2 * (split as u32 - canon_bits))) as u32];
    }
    let shift = canon_bits - split as u32;
    let first = (interleave(cx as u64, cy as u64) << (2 * shift)) as u32;
    (first..first + (1u32 << (2 * shift)))
        .filter(|&c| {
            let (ccx, ccy) = deinterleave(c as u64);
            !ord.cell(CANON, ccx, ccy).is_empty()
        })
        .collect()
}

/// Rows prepared (sorted, or run through a template) count against the fill's budget.
fn spend(ts: &mut TileState, rows: usize) {
    if let Some(left) = &mut ts.points.budget_left {
        *left = left.saturating_sub(rows);
    }
}

/// Whether the fill may prepare more now: always without a budget, and while any is left (so a
/// frame always makes progress, however big the next piece).
fn may_build(ts: &TileState) -> bool {
    ts.points.budget_left.is_none_or(|left| left > 0)
}

/// Prepare what a level would draw in `view` — its tiles' Morton order, the canonical cells of
/// the cells in view — while the budget lasts. True when all of it is ready.
#[allow(clippy::too_many_arguments)]
fn prepare_level(env: &BuildEnv, ts: &mut TileState, src: &Source, b: &Binding, want: &[TileId], split: u8, view: Rect, header: &Header, fetch: Option<&RangeFetch>) -> bool {
    let mut complete = true;
    let e = header.extent;
    for &t in want {
        let Look::Ready(tile) = look(ts, src, &b.from, t, fetch) else { continue };
        if !ts.points.ordered.contains(&(b.from.clone(), t)) && !may_build(ts) {
            complete = false;
            continue;
        }
        let ord = ordered(ts, &b.from, &tile);
        let n = 1u32 << split;
        let side = e.size / (1u64 << t.z) as f64 / n as f64;
        let (ox, oy) = (e.x0 + t.x as f64 * side * n as f64, e.y0 + t.y as f64 * side * n as f64);
        let span = |lo: f64, hi: f64, o: f64| (((lo - o) / side).floor().clamp(0.0, (n - 1) as f64) as u32, ((hi - o) / side).floor().clamp(0.0, (n - 1) as f64) as u32);
        let (cx0, cx1) = span(view.x, view.x1(), ox);
        let (cy0, cy1) = span(view.y, view.y1(), oy);
        for cy in cy0..=cy1 {
            for cxi in cx0..=cx1 {
                let range = ord.cell(split, cxi, cy);
                if range.is_empty() {
                    continue;
                }
                let (split, cxi, cy) = if range.len() == ord.morton.len() { (0, 0, 0) } else { (split, cxi, cy) };
                // A cell already cut (a level drawn before) needs nothing: checking its canonical
                // cells instead would rebuild whatever the cache let go of, round after round.
                if ts.points.cells.contains(&(b.fingerprint, b.from.clone(), t, split, cxi, cy)) {
                    continue;
                }
                for c in canon_cells(&ord, split, cxi, cy) {
                    if ts.points.canon.contains(&(b.fingerprint, b.from.clone(), t, c)) {
                        continue;
                    }
                    if !may_build(ts) {
                        complete = false;
                        continue;
                    }
                    canon(env, ts, b, &ord, t, c);
                }
            }
        }
    }
    complete
}

fn tile_key(t: TileId, split: u8, cx: u32, cy: u32) -> Key {
    let mut parts = vec![KeyPart::Int(t.z as i64), KeyPart::Int(t.x as i64), KeyPart::Int(t.y as i64)];
    if split > 0 {
        parts.extend([KeyPart::Int(split as i64), KeyPart::Int(cx as i64), KeyPart::Int(cy as i64)]);
    }
    Key::new(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::points::pyramid::Extent;

    #[test]
    fn morton_codes_interleave_and_invert() {
        assert_eq!(interleave(0b11, 0b01), 0b0111);
        assert_eq!(interleave(1 << 39, 0), 1u128 << 78);
        assert_eq!(interleave(0, (1 << 39) | 5), (1u128 << 79) | 0b100010);
        for (x, y) in [(0u32, 0u32), (7, 3), (123_456, 654_321), (u32::MAX, 1)] {
            assert_eq!(deinterleave(interleave(x as u64, y as u64) as u64), (x, y));
        }
    }

    #[test]
    fn covering_clamps_to_the_pyramid() {
        let h = Header { extent: Extent { x0: 0.0, y0: 0.0, size: 100.0 }, levels: 4, rows: 1, budget: 1 };
        assert_eq!(covering(&h, 0, Rect::new(-50.0, -50.0, 500.0, 500.0)), vec![TileId::new(0, 0, 0)]);
        // A view on the right half at level 2: two columns of four rows.
        let t = covering(&h, 2, Rect::new(50.0, 0.0, 50.0, 100.0));
        assert_eq!(t.len(), 8);
        assert!(t.iter().all(|t| t.x >= 2));
        // Edges are exclusive at the max side: a view ending on a tile boundary doesn't take the next.
        assert_eq!(covering(&h, 2, Rect::new(0.0, 0.0, 25.0, 25.0)), vec![TileId::new(2, 0, 0)]);
    }
}
