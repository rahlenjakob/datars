//! A decoded tile → scene nodes, through the `tiles` node's layer templates. Done once per tile
//! (and zoom, and binding) and cached: each feature's template resolves like any repeat row, then
//! features whose resolved style is identical are batched into one path, so a tile draws a
//! handful of nodes however many features it holds.

use super::decode::{DecodedTile, UNITS};
use super::Binding;
use crate::resolve::{Cx, Resolver};
use datars_data::{Column, Table};
use datars_expr::Value;
use datars_geo::tile::world_to_lonlat;
use datars_geo::{Projector, TileId};
use datars_math::{PathData, PathEl, Vec2};
use datars_scene::{Geom, Key, KeyPath, Node, NodeKind};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

/// What a feature template can see of its layer beyond the row: the geometries.
pub(crate) struct LayerCx {
    geoms: Arc<Vec<Geom>>,
    /// Non-affine projections: geometry is reprojected from this tile's units into content.
    reproject: Option<(TileId, Rc<Projector>)>,
}

/// The engine state building needs (borrowed from the engine for one fill).
pub(crate) struct BuildEnv<'a> {
    pub doc: &'a datars_ir::Doc,
    pub theme: &'a datars_theme::ResolvedTheme,
    pub fonts: &'a datars_text::FontDb,
    pub geo: &'a crate::geo::GeoStore,
    pub sources: &'a BTreeMap<String, Arc<Table>>,
    pub size_class: &'a str,
}

/// A label candidate: placed (or not) per frame in screen space.
#[derive(Clone)]
pub(crate) struct Label {
    pub node: Node,
    /// Where the feature is, in tile units (which tile square it belongs to: buffered copies of
    /// a feature in neighbouring tiles fall outside their squares and are dropped).
    pub unit: Vec2,
    pub priority: f64,
}

/// A tile's nodes (tile units; content units when reprojected): one group per drawn layer, and
/// label candidates.
pub(crate) struct TileNodes {
    pub layers: Vec<Node>,
    pub labels: Vec<Label>,
}

/// The current tile feature's geometry (`{"type": "feature"}` inside a tiles layer template).
pub(crate) fn feature_geom(r: &Resolver, cx: &Cx) -> Option<Geom> {
    let Some(l) = &cx.tile else {
        r.diag("a `feature` geometry needs a `source` outside tiles layers");
        return None;
    };
    let g = l.geoms.get(cx.row.as_ref()?.row)?;
    match &l.reproject {
        None => Some(g.clone()),
        Some((t, p)) => reproject(g, *t, p),
    }
}

fn tile_to_lonlat(t: TileId, q: Vec2) -> Vec2 {
    let n = (1u64 << t.z) as f64;
    world_to_lonlat(Vec2::new((t.x as f64 + q.x / UNITS) / n, (t.y as f64 + q.y / UNITS) / n))
}

/// Tile units → content through a projection. Vertices it can't show (the far side of a globe)
/// break the path there.
fn reproject(g: &Geom, t: TileId, p: &Projector) -> Option<Geom> {
    let Geom::Path { path } = g else { return Some(g.clone()) };
    let mut out = PathData::new();
    let mut open = false;
    let mut whole = true;
    for e in &path.els {
        match e {
            PathEl::Move { p: q } => {
                whole = true;
                open = match p.forward(tile_to_lonlat(t, *q)) {
                    Some(v) => {
                        out.move_to(v);
                        true
                    }
                    None => false,
                };
                whole &= open;
            }
            PathEl::Line { p: q } | PathEl::Quad { p: q, .. } | PathEl::Cubic { p: q, .. } => match p.forward(tile_to_lonlat(t, *q)) {
                Some(v) if open => {
                    out.line_to(v);
                }
                Some(v) => {
                    out.move_to(v);
                    open = true;
                }
                None => {
                    open = false;
                    whole = false;
                }
            },
            PathEl::Close => {
                if open && whole {
                    out.close();
                }
                open = false;
            }
        }
    }
    (!out.is_empty()).then(|| Geom::path(out))
}

fn truthy(v: f64) -> bool {
    v != 0.0 && !v.is_nan()
}

/// Features batched by resolved style.
struct Batches {
    protos: Vec<(Node, PathData)>,
}

impl Batches {
    /// Take `n` into a batch, or hand it back if it can't be batched (it has identity: semantics,
    /// picking, anchors; or isn't a plain path).
    fn add(&mut self, n: Node) -> Option<Node> {
        let batchable = n.semantics.is_none() && !n.pickable && n.anchors.is_empty() && n.common.trim.is_none() && n.common.clip.is_none();
        let (path, fill, stroke) = match &n.kind {
            NodeKind::Shape { geom: Geom::Path { path }, fill, stroke, markers: None } if batchable => (path, fill, stroke),
            _ => return Some(n),
        };
        for (proto, acc) in &mut self.protos {
            if let NodeKind::Shape { fill: f, stroke: s, .. } = &proto.kind {
                if f == fill && s == stroke && proto.common == n.common {
                    acc.els.extend_from_slice(&path.els);
                    return None;
                }
            }
        }
        let acc = (**path).clone();
        self.protos.push((n, acc));
        None
    }

    fn finish(self) -> Vec<Node> {
        self.protos
            .into_iter()
            .enumerate()
            .map(|(i, (mut n, acc))| {
                n.key = Key::one(i as i64);
                if let NodeKind::Shape { geom, .. } = &mut n.kind {
                    *geom = Geom::path(acc);
                }
                n
            })
            .collect()
    }
}

/// Resolve every drawn layer of `tile` at `zoom`, compiling into the tiles' shared `exprs`.
/// Resolver diagnostics go to `diags`.
pub(crate) fn build(env: &BuildEnv, b: &Binding, tile: &DecodedTile, zoom: u8, reprojected: bool, exprs: &crate::resolve::SharedExprs, diags: &mut Vec<String>) -> TileNodes {
    let mut signals = (*b.signals).clone();
    signals.insert("tile.zoom".into(), Value::Num(zoom as f64));
    let store = crate::tables::TableStore::new(env.sources.clone(), env.doc.tables.clone(), Default::default());
    let r = Resolver::new(env.doc, &signals, env.theme, env.fonts, None, env.size_class, env.geo, store).with_exprs(exprs);
    let projector = reprojected.then(|| Rc::new(b.proj.projector()));
    let mut out = TileNodes { layers: Vec::new(), labels: Vec::new() };
    let z = zoom as f64;
    for spec in &b.spec.layers {
        if spec.minzoom.is_some_and(|m| z < m) || spec.maxzoom.is_some_and(|m| z >= m) {
            continue;
        }
        let id = spec.id.clone().unwrap_or_else(|| spec.layer.clone());
        let mut batches = Batches { protos: Vec::new() };
        let mut singles: Vec<Node> = Vec::new();
        for layer in tile.layers(&spec.layer) {
            let (table, anchors) = match &projector {
                None => (layer.table.clone(), layer.anchors.clone()),
                Some(p) => reprojected_anchors(layer.table.clone(), &layer.anchors, tile.id, p),
            };
            let lcx = Rc::new(LayerCx { geoms: layer.geoms.clone(), reproject: projector.clone().map(|p| (tile.id, p)) });
            let base = Cx {
                box_w: UNITS,
                box_h: UNITS,
                scope: Rc::new(crate::scales::Scope::default()),
                row: None,
                fields: None,
                group: None,
                path: KeyPath::default(),
                depth: 0,
                proj: projector.is_some().then(|| b.proj.clone()),
                default_key: None,
                tile: Some(lcx),
                recipes: None,
                interactive: None,
            };
            let n = table.len();
            let keep: Vec<bool> = if spec.filter.is_null() { vec![true; n] } else { r.column_values(&spec.filter, &table, &base).into_iter().map(truthy).collect() };
            let priority = if spec.labels && !spec.priority.is_null() { r.column_values(&spec.priority, &table, &base) } else { vec![0.0; n] };
            for i in (0..n).filter(|&i| keep[i]) {
                // Keyed by feature index (a number: no `#i` text formatted for each of thousands of
                // features, most batched and re-keyed anyway).
                let cx = Cx { row: Some(Rc::new(crate::env::Row { table: table.clone(), row: i })), default_key: Some(Key::one(i as i64)), ..base.clone() };
                for node in r.resolve(&spec.template, &cx, i) {
                    if spec.labels {
                        if anchors[i].is_finite() {
                            out.labels.push(Label { node, unit: layer.anchors[i], priority: if priority[i].is_nan() { f64::NEG_INFINITY } else { priority[i] } });
                        }
                    } else if !spec.merge {
                        singles.push(node);
                    } else if let Some(n) = batches.add(node) {
                        singles.push(n);
                    }
                }
            }
        }
        let mut kids = batches.finish();
        kids.extend(singles);
        if !kids.is_empty() {
            out.layers.push(Node::group(Key::one(id.as_str()), kids));
        }
    }
    for d in r.diags.take() {
        if !diags.contains(&d.message) {
            diags.push(d.message);
        }
    }
    out
}

/// A layer's table with `$x`/`$y` in content units, and the anchors to match.
fn reprojected_anchors(table: Arc<Table>, anchors: &[Vec2], t: TileId, p: &Projector) -> (Arc<Table>, Arc<Vec<Vec2>>) {
    let pts: Vec<Vec2> = anchors.iter().map(|a| p.forward(tile_to_lonlat(t, *a)).unwrap_or(Vec2::new(f64::NAN, f64::NAN))).collect();
    let mut tb = (*table).clone();
    if let Some(Column::Num(xs)) = tb.column_mut("$x") {
        *xs = pts.iter().map(|q| q.x).collect();
    }
    if let Some(Column::Num(ys)) = tb.column_mut("$y") {
        *ys = pts.iter().map(|q| q.y).collect();
    }
    (Arc::new(tb), Arc::new(pts))
}
