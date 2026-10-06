//! Point pyramids: instances at any scale (docs/04-primitives.md, "Instances"; docs/12-delivery.md,
//! "Big data stays interactive"). An `instances` node with `lod` draws the rows of a table — or of a
//! point archive the host serves by byte range — however many there are: the rows are indexed
//! once into a pyramid of tiles ([`pyramid`]), and each frame draws only the tiles its camera
//! shows, down to the level where the rows in view fill the frame's budget of points (`points`).
//! Zoom in and deeper levels add their rows until every row in view is there; pan and nothing is
//! re-resolved — the frame pass picks tiles, and a tile's instances are built once (its rows
//! through the node's template) and reused.
//!
//! Like `tiles`, an `lod` node resolves to an empty placeholder plus a [`Binding`], filled per
//! frame — settled or mid-flight — from the camera it has in that frame ([`fill::fill_one`]). Rows
//! sit at their `x`/`y` in the node's own coordinates; a transform or a view's camera above maps
//! them to the screen. Nothing here knows what the rows are: the template says how they look.

pub mod codec;
pub(crate) mod fill;
pub mod pyramid;

pub use codec::{archive, decode, encode};
pub use pyramid::{build, Extent, Header, Options, PointTile, Pyramid};

use crate::resolve::{Cx, Resolver};
use crate::tiles::Lru;
use datars_data::Table;
use datars_expr::Value;
use datars_geo::TileId;
use datars_ir::{SourceKind, TInstances};
use datars_math::Hash64;
use datars_scene::{KeyPath, NodeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

/// Decoded archive tiles kept in memory (LRU).
const MAX_DECODED: usize = 512;
/// Tiles kept in Morton order (LRU).
const MAX_BUILT: usize = 512;
/// Cells of built tiles kept in memory (LRU).
const MAX_CELLS: usize = 4096;

/// Where an `lod` node's rows come from.
#[derive(Clone, Debug)]
pub(crate) enum Origin {
    /// A table: indexed in memory the first time a frame needs it.
    Table(Arc<Table>),
    /// A point archive (a `tiles` source): tiles read by range as views need them.
    Archive,
}

/// What an `lod` instances node resolved to: everything the frame pass needs to fill it.
#[derive(Clone, Debug)]
pub(crate) struct Binding {
    /// The table or tiles source.
    pub from: String,
    pub origin: Origin,
    pub spec: Rc<TInstances>,
    /// Signal values when the node resolved (the template evaluates against them).
    pub signals: Rc<BTreeMap<String, Value>>,
    /// Identity of what a tile's nodes look like — the template and the values of the signals it
    /// reads — so built tiles are reused across frames, pans and states.
    pub fingerprint: u64,
    /// Rows per tile area at average density (for pyramids built from tables).
    pub budget: u32,
    /// The most points a frame draws.
    pub max_points: f64,
}

/// A pyramid built from a table, and what it was built from.
struct Indexed {
    table: Arc<Table>,
    pyramid: Rc<Pyramid>,
}

/// Counters for inspection, tests and the performance report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PointStats {
    /// Rows in the pyramids drawn by the last fill.
    pub rows: u64,
    /// Points the last fill drew, and in how many tiles; the deepest level it drew.
    pub drawn: u64,
    pub tiles: u32,
    pub level: u8,
    /// Canonical cells built (the template evaluated over their rows), and pyramids indexed.
    pub built: u64,
    pub indexed: u64,
}

/// Point pyramids and caches for the loaded document (inside the engine's tile state: filling a
/// frame reads caches and records requests from `&self` paths).
pub(crate) struct Store {
    from_tables: BTreeMap<(String, u64), Indexed>,
    decoded: Lru<(String, TileId), Arc<PointTile>>,
    absent: BTreeSet<(String, TileId)>,
    /// A tile's rows in Morton order (for any binding).
    ordered: Lru<(String, TileId), Rc<fill::Ordered>>,
    /// A tile's canonical cells (its rows in each of an 8 × 8 split) through a binding's template,
    /// built as views reach them.
    canon: Lru<(u64, String, TileId, u32), Arc<datars_scene::Instances>>,
    /// Cells drawn so far (tile, split, column, row): slices or unions of canonical cells.
    cells: Lru<(u64, String, TileId, u8, u32, u32), datars_scene::Node>,
    pub stats: PointStats,
    /// Rows the current fill may still prepare (`None`: unlimited).
    pub budget_left: Option<usize>,
    /// When each tile below the root was first drawn (frame clock, seconds): new detail fades in
    /// from then instead of appearing at once.
    pub first_drawn: BTreeMap<(String, TileId), f64>,
}

impl Store {
    pub fn new() -> Store {
        Store { from_tables: BTreeMap::new(), decoded: Lru::new(MAX_DECODED), absent: BTreeSet::new(), ordered: Lru::new(MAX_BUILT), canon: Lru::new(MAX_CELLS), cells: Lru::new(MAX_CELLS), stats: PointStats::default(), budget_left: None, first_drawn: BTreeMap::new() }
    }

    /// Drop built nodes (styles changed: theme, mode). Pyramids and decoded tiles stay.
    pub fn clear_built(&mut self) {
        self.canon.clear();
        self.cells.clear();
    }
}

/// The key a pyramid built from a table depends on besides the table: positions and budget.
/// Every column of the table is stored with its rows, so any number of `lod` nodes over the
/// same rows (dots and a glow, say) share one pyramid — and one archive when published.
pub(crate) fn build_key(spec: &TInstances, budget: u32) -> u64 {
    let mut h = Hash64::new();
    h.str(&serde_json::to_string(&spec.x).unwrap_or_default());
    h.str(&serde_json::to_string(&spec.y).unwrap_or_default());
    h.u32(budget);
    h.finish()
}

/// The columns a pyramid stores per row: all of the table's but the tile's own (`x`, `y`,
/// `$row`: its rows' positions and numbers).
pub(crate) fn stored_columns(table: &Table) -> Vec<String> {
    table.column_names().into_iter().filter(|c| !matches!(*c, "x" | "y" | "$row")).map(String::from).collect()
}

/// Expression sources in a JSON template (`"=…"` strings and `{"expr": …}` objects).
fn expressions(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) if s.starts_with('=') => out.push(s[1..].to_string()),
        serde_json::Value::Object(o) => match o.get("expr").and_then(|e| e.as_str()) {
            Some(e) if o.len() == 1 => out.push(e.to_string()),
            _ => o.values().for_each(|x| expressions(x, out)),
        },
        serde_json::Value::Array(a) => a.iter().for_each(|x| expressions(x, out)),
        _ => {}
    }
}

/// Is `p` the plain column `name` (`"=d.x"`, `"=x"`, `"x"`)?
fn is_column(p: &datars_ir::Prop, name: &str) -> bool {
    match p.as_expr() {
        Some(src) => datars_expr::parse(src).ok().and_then(|e| datars_expr::compile(&e).ok()).is_some_and(|c| c.fields() == [name] && c.calls().is_empty() && c.signals().iter().all(|s| s == name)),
        None => p.as_str() == Some(name),
    }
}

/// Resolve an `lod` instances node: an empty group now, filled per frame (see the module docs).
pub(crate) fn resolve(r: &Resolver, ti: &TInstances, cx: &Cx, path: &KeyPath) -> Option<NodeKind> {
    let lod = ti.lod.as_ref()?;
    let origin = match r.doc.data.get(&ti.from).map(|s| &s.from) {
        Some(SourceKind::Tiles(_)) => {
            if !is_column(&ti.x, "x") || !is_column(&ti.y, "y") {
                r.diag(format!("instances from the point archive `{}`: rows sit at the archive's `x`/`y` (write `x: \"=d.x\", y: \"=d.y\"`)", ti.from));
            }
            Origin::Archive
        }
        _ => {
            let table = r.table_for(&ti.from, cx)?;
            // Positions are indexed once, so they can't depend on the layout box, scales or
            // signals: place the node under a transform or a view to map them to the screen.
            for (axis, p) in [("x", &ti.x), ("y", &ti.y)] {
                if let Some(c) = p.as_expr().and_then(|s| r.compiled(s)) {
                    if !c.calls().is_empty() || c.signals().iter().any(|s| table.column(s).is_none()) {
                        r.diag(format!("instances with lod: `{axis}` must depend on the row only (it is indexed once); place the node under a transform or view instead"));
                    }
                }
            }
            Origin::Table(table)
        }
    };
    let mut h = Hash64::new();
    let mut spec = serde_json::to_value(ti).unwrap_or_default();
    if let Some(o) = spec.as_object_mut() {
        o.remove("lod");
    }
    h.str(&spec.to_string());
    // The signals the template reads (not the others: hovering elsewhere mustn't rebuild).
    let mut srcs = Vec::new();
    expressions(&spec, &mut srcs);
    let mut read = BTreeSet::new();
    for s in &srcs {
        if let Some(c) = r.compiled(s) {
            read.extend(c.signals());
        }
    }
    for name in &read {
        h.str(name);
        h.str(&format!("{:?}", r.signals.get(name)));
    }
    let budget = lod.budget.unwrap_or(Options::default().budget as f64).clamp(1.0, 1.0e7) as u32;
    let max_points = lod.points.unwrap_or(150_000.0).clamp(100.0, 5_000_000.0);
    let b = Binding { from: ti.from.clone(), origin, spec: Rc::new(ti.clone()), signals: Rc::new(r.signals.clone()), fingerprint: h.finish(), budget, max_points };
    r.points.borrow_mut().push((path.to_string(), Rc::new(b)));
    Some(NodeKind::Group { children: Vec::new() })
}

/// A point archive of `table`'s rows at (`x`, `y`), every column kept — what the publish compiler
/// ships for an `lod` node over a table, and `datars` writes for data too big for a document.
pub fn archive_of(table: &Table, x: &str, y: &str, opts: &Options) -> Result<Vec<u8>, String> {
    let xs = table.num(x).ok_or_else(|| format!("no numeric column `{x}`"))?;
    let ys = table.num(y).ok_or_else(|| format!("no numeric column `{y}`"))?;
    let names = stored_columns(table);
    let cols: Vec<(String, &datars_data::Column)> = names.iter().filter_map(|c| table.column(c).map(|col| (c.clone(), col))).collect();
    archive(&build(xs, ys, &cols, opts))
}
