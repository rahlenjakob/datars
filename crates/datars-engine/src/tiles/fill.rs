//! The frame pass: fill every `tiles` placeholder of a scene from the camera it has in that
//! scene. Runs on settled scenes and on every frame of a transition, so a flight shows (and asks
//! for) the tiles of each intermediate zoom.

use super::decode::{DecodedTile, UNITS};
use super::layers::{build, BuildEnv, TileNodes};
use super::view::{self, Visible};
use super::{Binding, Lookup, RangeFetch, TileState};
use crate::bounds::{node_bounds, transform_rect};
use datars_geo::TileId;
use datars_math::{total_cmp, Affine, PathData, Rect, Vec2};
use datars_scene::{Clip, Key, KeyPart, KeyPath, Node, NodeKind, Scene};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;

/// Labels one view places at most (the rest are dropped, lowest priority first).
const MAX_LABELS: usize = 200;
/// Clear space around a placed label (px).
const LABEL_GAP: f64 = 3.0;
/// How far up the pyramid a missing tile looks for stand-in data.
const MAX_FALLBACK: u8 = 8;

pub(crate) struct FillCx<'a> {
    pub env: BuildEnv<'a>,
    pub state: &'a RefCell<TileState>,
    pub bindings: &'a BTreeMap<String, Rc<Binding>>,
    /// Point pyramid placeholders (`instances` with `lod`), by key path, and a factor on their
    /// budgets of points.
    pub points: &'a BTreeMap<String, Rc<crate::points::Binding>>,
    pub point_scale: f64,
    /// The host's share of the per-frame decode and styling budgets (`Engine::set_work_scale`).
    pub work: f64,
    pub fetch: Option<&'a RangeFetch>,
    pub labels: LabelMode<'a>,
    /// Rows of point tiles this fill may prepare (sort, run through the template) before finer
    /// levels wait for the next frame; `None` builds everything (renders, goldens).
    pub build_budget: Option<usize>,
    /// The frame's clock (seconds) for live frames — new detail fades in over time; `None` for still
    /// renders, which draw everything at once.
    pub now: Option<f64>,
    /// Look somewhere else than the camera does (prefetching around the view): every tile layer's
    /// camera zoomed by the factor about the middle of what it shows, and panned by the offset (a
    /// share of what it shows).
    pub look: Option<(f64, Vec2)>,
}

/// A label's identity across tiles and zoom levels: its text and where it stands (world units,
/// ~4 km cells) — the same place from a coarser or finer tile is the same label.
pub(crate) type LabelId = (String, i64, i64);

/// How labels are placed on this pass.
pub(crate) enum LabelMode<'a> {
    /// Placed and drawn at full opacity (settled scenes).
    Draw,
    /// A sample of the recent past during a transition: record what gets placed, draw nothing.
    Collect(&'a RefCell<BTreeSet<LabelId>>),
    /// During a transition: `history` counts, per label, in how many of `samples` recent moments
    /// it was placed. A label's opacity is the share of those moments plus now in which it shows,
    /// so labels fade in as they find room and fade out as they lose it — a pure function of the
    /// plan and the time, not of which frames a host happened to render (P1).
    /// `settle` (0…1) blends toward the plain placement over the last moments of the transition.
    Fade { history: &'a BTreeMap<LabelId, u32>, samples: u32, settle: f64 },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FillReport {
    pub wanted: u32,
    pub ready: u32,
    pub fallback: u32,
    pub pending: u32,
    pub labels: u32,
    /// Point levels waiting for the next frame's build budget: the host should draw another.
    pub building: u32,
}

/// Fill the scene's placeholders. With `draw` false, only look tiles up (requesting what's
/// missing) — prefetching along a flight.
pub(crate) fn fill_scene(cx: &FillCx, scene: &mut Scene, draw: bool) -> FillReport {
    let mut report = FillReport::default();
    if cx.bindings.is_empty() && cx.points.is_empty() {
        return report;
    }
    {
        let mut st = cx.state.borrow_mut();
        st.points.budget_left = cx.build_budget;
        // Frames decode new tiles within a budget; a look into the past (label history) and a
        // look-up that draws nothing (prefetching a flight) decode none, only ask for bytes; still
        // renders (no budget) decode everything.
        let past = matches!(cx.labels, LabelMode::Collect(_)) && cx.build_budget.is_some();
        let none = past || !draw;
        st.decode_left = if none { Some(0) } else { cx.build_budget.map(|_| ((super::DECODE_PER_FRAME as f64 * cx.work) as usize).max(4096)) };
        st.decoded_now = if none { u32::MAX } else { 0 };
        st.builds_left = if none { Some(0) } else { cx.build_budget.map(|_| ((super::BUILD_FEATURES_PER_FRAME as f64 * cx.work) as usize).max(500)) };
        // u32::MAX: this fill styles nothing new (it draws what's styled, or skips).
        st.built_now = if none { u32::MAX } else { 0 };
        if !none && cx.build_budget.is_some() {
            st.style_first = std::mem::take(&mut st.style_waits);
            st.decoded_heavy = false;
        }
    }
    let clip = Rect::new(0.0, 0.0, scene.width, scene.height);
    // Place names are decoration: they give way to every authored text on the canvas — titles,
    // legends, callouts, data labels, the attribution — so no city hides under a caption.
    let mut avoid = Vec::new();
    authored_text(&scene.root, Affine::IDENTITY, 1.0, &mut avoid);
    walk(cx, &mut scene.root, &KeyPath::default(), Affine::IDENTITY, clip, &avoid, draw, &mut report);
    if matches!(cx.labels, LabelMode::Collect(_)) {
        return report; // a look into the past: no stats
    }
    let mut st = cx.state.borrow_mut();
    let s = &mut st.stats;
    s.last_wanted = report.wanted;
    s.last_ready = report.ready;
    s.last_fallback = report.fallback;
    s.last_pending = report.pending;
    s.last_labels = report.labels;
    if draw {
        s.drawn += (report.ready + report.fallback) as u64;
    }
    report
}

/// Screen boxes of the texts a scene draws readably (before any tiles are filled in, so none of
/// them are place names).
fn authored_text(n: &Node, parent: Affine, opacity: f64, out: &mut Vec<Rect>) {
    let opacity = opacity * n.common.opacity;
    if !n.common.visible || opacity < 0.3 {
        return;
    }
    let xf = n.common.placed(parent);
    match &n.kind {
        NodeKind::Text(t) if !t.text.trim().is_empty() && t.bounds.w > 0.0 => {
            let r = if t.screen_size {
                let o = xf.apply(t.origin) + t.offset;
                Rect::new(o.x + t.bounds.x, o.y + t.bounds.y, t.bounds.w, t.bounds.h)
            } else {
                transform_rect(Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h), &xf)
            };
            out.push(r);
        }
        NodeKind::View { viewport, camera, children, .. } => {
            let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            children.iter().for_each(|c| authored_text(c, xf.mul(cam), opacity, out));
        }
        _ => n.children().iter().for_each(|c| authored_text(c, xf, opacity, out)),
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(cx: &FillCx, n: &mut Node, path: &KeyPath, parent: Affine, clip: Rect, avoid: &[Rect], draw: bool, report: &mut FillReport) {
    if !n.common.visible || n.common.opacity <= 0.0 {
        return;
    }
    let xf = parent.mul(n.common.transform);
    let here = path.push(&n.key);
    let clip = match &n.common.clip {
        Some(Clip::Rect { rect }) => transform_rect(*rect, &xf).intersect(&clip).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
        _ => clip,
    };
    match &mut n.kind {
        NodeKind::Group { children } if children.is_empty() => {
            let key = here.to_string();
            let xf = match cx.look {
                Some((k, pan)) => {
                    let c = clip.center();
                    Affine::translate(c.x + pan.x * clip.w, c.y + pan.y * clip.h).mul(Affine::scale(k, k)).mul(Affine::translate(-c.x, -c.y)).mul(xf)
                }
                None => xf,
            };
            if let Some(b) = cx.bindings.get(&key) {
                *children = fill_one(cx, b, xf, clip, avoid, draw, report);
            } else if let Some(b) = cx.points.get(&key) {
                // A look into the past only collects map labels: point layers have none.
                if !matches!(cx.labels, LabelMode::Collect(_)) {
                    *children = crate::points::fill::fill_one(cx, b, xf, clip, draw, report);
                }
            }
        }
        NodeKind::Group { children } => {
            for c in children {
                walk(cx, c, &here, xf, clip, avoid, draw, report);
            }
        }
        NodeKind::View { viewport, camera, clip: clips, children } => {
            let clip = if *clips { transform_rect(*viewport, &xf).intersect(&clip).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)) } else { clip };
            let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            for c in children {
                walk(cx, c, &here, xf.mul(cam), clip, avoid, draw, report);
            }
        }
        _ => {}
    }
}

/// How long a square's finer data fades in over what it drew before (an ancestor's coarser
/// detail, overzoomed), seconds. Swapped in one frame, arriving detail pops; fading, it doesn't —
/// and the renderer can spread the new geometry's first tessellation over the first frames of the
/// fade, while it's barely visible.
pub(crate) const FADE_S: f64 = 0.3;

/// What a square drew: the tile whose data it was, and its styled nodes.
#[derive(Clone)]
pub(crate) struct Shown {
    data: TileId,
    tile: Arc<DecodedTile>,
    nodes: Rc<TileNodes>,
}

/// A square's new data replacing, since `start`, what was drawn there before: `from`, each with
/// the square it was drawn for — the square itself or an ancestor (coarser data, which the new
/// data fades in over), or, with `out`, the finer squares it replaces after a zoom-out (which
/// fade out over it).
#[derive(Clone)]
pub(crate) struct Fade {
    start: f64,
    from: Vec<(TileId, Shown)>,
    out: bool,
}

/// One visible square and the tile whose data draws it (itself, or an ancestor standing in).
struct Draw {
    square: TileId,
    data: TileId,
    tile: Arc<DecodedTile>,
}

/// A stand-in already in memory for a pending tile (no new requests).
fn cached_ancestor(st: &mut TileState, source: &str, t: TileId) -> Option<(TileId, Arc<DecodedTile>)> {
    (1..=t.z.min(MAX_FALLBACK)).find_map(|up| {
        let a = t.ancestor(up);
        st.cached(source, a).map(|d| (a, d))
    })
}

fn fill_one(cx: &FillCx, b: &Binding, xf: Affine, clip: Rect, avoid: &[Rect], draw: bool, report: &mut FillReport) -> Vec<Node> {
    let mut st = cx.state.borrow_mut();
    let src = b.source.as_str();
    if st.zoom_range(src).is_none() {
        // Opening the archive: the header (+ root directory) comes first.
        st.lookup(src, TileId::new(0, 0, 0), cx.fetch);
    }
    let Some(zooms) = st.zoom_range(src) else {
        report.pending += 1;
        return Vec::new();
    };
    let tile_size = b.spec.tile_size.unwrap_or(512.0);
    let Some(vis) = view::visible(&b.proj, &xf, clip, tile_size, zooms) else { return Vec::new() };
    report.wanted += vis.tiles.len() as u32;
    let mut draws: Vec<Draw> = Vec::new();
    for &t in &vis.tiles {
        match st.lookup(src, t, cx.fetch) {
            Lookup::Ready(tile) => {
                report.ready += 1;
                draws.push(Draw { square: t, data: t, tile });
            }
            l @ (Lookup::Pending | Lookup::Later) => {
                // Downloading, or here but waiting for the next frame's decode budget (the host
                // draws another frame for it).
                if matches!(l, Lookup::Later) {
                    report.building += 1;
                } else {
                    report.pending += 1;
                }
                if let Some((a, tile)) = cached_ancestor(&mut st, src, t) {
                    report.fallback += 1;
                    draws.push(Draw { square: t, data: a, tile });
                }
            }
            Lookup::Absent => {
                // Outside the archive's coverage at this zoom (a regional extract): the nearest
                // ancestor with data, overzoomed.
                for up in 1..=t.z.min(MAX_FALLBACK) {
                    let a = t.ancestor(up);
                    match st.lookup(src, a, cx.fetch) {
                        Lookup::Ready(tile) => {
                            report.fallback += 1;
                            draws.push(Draw { square: t, data: a, tile });
                            break;
                        }
                        l @ (Lookup::Pending | Lookup::Later) => {
                            if matches!(l, Lookup::Later) {
                                report.building += 1;
                            } else {
                                report.pending += 1;
                            }
                            if let Some((a2, tile)) = cached_ancestor(&mut st, src, a) {
                                report.fallback += 1;
                                draws.push(Draw { square: t, data: a2, tile });
                            }
                            break;
                        }
                        Lookup::Absent => continue,
                    }
                }
            }
        }
    }
    if !draw {
        return Vec::new();
    }
    let reprojected = view::world_to_content(&b.proj).is_none();
    let collect = matches!(cx.labels, LabelMode::Collect(_));
    let mut out = Vec::with_capacity(draws.len() + 1);
    let mut candidates: Vec<Candidate> = Vec::new();
    // Live frames remember what each square drew, so finer data can fade in over it.
    let live = cx.now.filter(|_| !collect);
    let fp = b.fingerprint;
    let mut shown_now: BTreeMap<TileId, Shown> = BTreeMap::new();
    // What finer squares fade in over, collected to draw first: (the fading square, the square
    // its old data was drawn for — itself or an ancestor —, that data).
    let mut unders: Vec<(TileId, TileId, Shown)> = Vec::new();
    for d in &draws {
        // Past the styling budget, a tile never styled before shows an ancestor's styled nodes
        // (overzoomed, clipped to its square) until a frame has budget for it: a zoom crossing a
        // data level brings dozens of new tiles at once, and styling them all froze that frame.
        // With no ancestor styled either, a frame styles it anyway (nothing to show otherwise); a
        // look into the past (label history) styles nothing new and skips it.
        let (nodes, stale, stand_in) = match built(cx, &mut st, b, &d.tile, vis.zoom, reprojected, true) {
            Some((nodes, stale)) => (nodes, stale, None),
            None => match built_ancestor(&mut st, b, d.data, vis.zoom) {
                Some((a, nodes)) => (nodes, true, Some(Draw { square: d.square, data: a, tile: d.tile.clone() })),
                None if st.built_now == u32::MAX => continue,
                None => match built(cx, &mut st, b, &d.tile, vis.zoom, reprojected, false) {
                    Some((nodes, stale)) => (nodes, stale, None),
                    None => continue,
                },
            },
        };
        let d = stand_in.as_ref().unwrap_or(d);
        if stale {
            report.building += 1;
        }
        let tile_xf = if reprojected { Affine::IDENTITY } else { b.proj.tile_transform(d.data, UNITS).unwrap_or(Affine::IDENTITY) };
        let (sq_origin, sq_size) = square_in(d.square, d.data);
        let mut opacity = 1.0;
        // What was drawn here last frame, fading out over this square's new data (see below).
        let mut over: Vec<Node> = Vec::new();
        if let Some(now) = live {
            // New data for a square doesn't swap in within one frame. Finer data fades in over
            // the coarser data drawn there before — the square's own, or, for a square new in view
            // after a zoom-in (a flight crosses a level every second or so), its nearest ancestor
            // square's, which covers it whole — and the renderer spreads the new geometry's first
            // tessellation over the faint first frames. After a zoom-out, the new, coarser square
            // draws at once and what its finer squares drew last frame fades out on top of it:
            // they needn't cover it all (the view grew), and nothing may show through. Detail
            // neither pops in nor vanishes.
            if !st.fades.contains_key(&(fp, d.square)) {
                let fade = st.shown.get(&fp).and_then(|m| {
                    match (0..=d.square.z.min(MAX_FALLBACK)).find_map(|up| m.get_key_value(&d.square.ancestor(up))) {
                        Some((above, p)) if p.data.z < d.data.z => Some(Fade { start: now, from: vec![(*above, p.clone())], out: false }),
                        Some(_) => None,
                        None => {
                            let from: Vec<(TileId, Shown)> = m.iter().filter(|(sq, p)| sq.z > d.square.z && sq.ancestor(sq.z - d.square.z) == d.square && p.data != d.data).map(|(sq, p)| (*sq, p.clone())).collect();
                            (!from.is_empty()).then_some(Fade { start: now, from, out: true })
                        }
                    }
                });
                if let Some(f) = fade {
                    st.fades.insert((fp, d.square), f);
                }
            }
            shown_now.insert(d.square, Shown { data: d.data, tile: d.tile.clone(), nodes: nodes.clone() });
            if let Some(f) = st.fades.get(&(fp, d.square)).cloned() {
                let k = ((now - f.start) / FADE_S).clamp(0.0, 1.0);
                // Eased (smoothstep); done when it reaches 1 (k just short of 1 rounds there).
                let eased = k * k * (3.0 - 2.0 * k);
                if eased < 1.0 {
                    report.building += 1;
                    if f.out {
                        for (sq, from) in &f.from {
                            let mut g = before_group(&vis, b, *sq, from, xf, reprojected, over_key(*sq));
                            g.common.opacity = 1.0 - eased;
                            over.push(g);
                        }
                    } else {
                        unders.extend(f.from.iter().map(|(above, from)| (d.square, *above, from.clone())));
                    }
                    if !f.out {
                        opacity = eased;
                    }
                } else {
                    st.fades.remove(&(fp, d.square));
                }
            }
        }
        if !collect {
            let mut g = Node::group(tile_key(d.square), nodes.layers.clone());
            g.common.transform = tile_xf;
            g.common.clip = square_clip(&vis, b, d, xf, tile_xf, reprojected);
            g.common.opacity = opacity;
            out.push(g);
            out.append(&mut over);
        }
        let screen = xf.mul(tile_xf);
        for l in &nodes.labels {
            let inside = l.unit.x >= sq_origin.x && l.unit.x < sq_origin.x + sq_size && l.unit.y >= sq_origin.y && l.unit.y < sq_origin.y + sq_size;
            if inside {
                candidates.push(Candidate { priority: l.priority, order: candidates.len(), tile: d.data, m: screen, node: l.node.clone(), id: label_id(&l.node, d.data, l.unit) });
            }
        }
    }
    if !unders.is_empty() {
        // Under everything else. Where every square drawn inside an ancestor fades in over that
        // ancestor's data (a level crossed: all its children at once), the data is drawn once,
        // clipped to the ancestor — not once per child, each clipped to it: draws cost (on GL
        // every one does), and four children drew the whole parent tile four times. Elsewhere,
        // per square: where an ancestor has squares drawn that aren't fading over it, its data
        // mustn't show under them (its coastline isn't theirs).
        let mut first: Vec<Node> = Vec::new();
        let mut done: BTreeSet<TileId> = BTreeSet::new();
        for (sq, above, from) in &unders {
            let whole = draws.iter().filter(|o| o.square.z >= above.z && o.square.ancestor(o.square.z - above.z) == *above).all(|o| unders.iter().any(|(s2, a2, f2)| *s2 == o.square && a2 == above && f2.data == from.data));
            if !whole {
                first.push(before_group(&vis, b, *sq, from, xf, reprojected, under_key(*sq)));
            } else if done.insert(*above) {
                first.push(before_group(&vis, b, *above, from, xf, reprojected, under_key(*above)));
            }
        }
        first.append(&mut out);
        out = first;
    }
    if live.is_some() {
        // Squares out of view forget what they showed (and stop fading).
        st.fades.retain(|(f, sq), _| *f != fp || shown_now.contains_key(sq));
        st.shown.insert(fp, shown_now);
    }
    let order = place_labels(&candidates, clip, avoid);
    // Opacity per shown candidate: placed now, or fading out after recently being placed.
    let mut shown: Vec<(usize, f64)> = match &cx.labels {
        LabelMode::Collect(set) => {
            let mut set = set.borrow_mut();
            for &i in &order {
                set.insert(candidates[i].id.clone());
            }
            return out;
        }
        LabelMode::Draw => order.iter().map(|&i| (i, 1.0)).collect(),
        LabelMode::Fade { history, samples, settle } => {
            let k = (*samples + 1) as f64;
            let past = |i: usize| history.get(&candidates[i].id).copied().unwrap_or(0) as f64;
            let now: BTreeSet<usize> = order.iter().copied().collect();
            let mut v: Vec<(usize, f64)> = order.iter().map(|&i| (i, (1.0 + past(i)) / k * (1.0 - settle) + settle)).collect();
            let mut gone: BTreeSet<&LabelId> = order.iter().map(|&i| &candidates[i].id).collect();
            for i in 0..candidates.len() {
                if !now.contains(&i) && past(i) > 0.0 && gone.insert(&candidates[i].id) && label_box(&candidates[i].node, &candidates[i].m).is_some_and(|bx| clip.intersects(&bx)) {
                    v.push((i, past(i) / k * (1.0 - settle)));
                }
            }
            v
        }
    };
    report.labels += shown.len() as u32;
    let mut placed: BTreeMap<TileId, Vec<Node>> = BTreeMap::new();
    for (i, w) in shown.drain(..).filter(|(_, w)| *w > 0.0) {
        let c = &candidates[i];
        let mut n = c.node.clone();
        if w < 1.0 {
            n.common.opacity *= w;
        }
        placed.entry(c.tile).or_default().push(n);
    }
    if !placed.is_empty() {
        let groups = placed
            .into_iter()
            .map(|(t, nodes)| {
                let mut g = Node::group(tile_key(t), nodes);
                g.common.transform = if reprojected { Affine::IDENTITY } else { b.proj.tile_transform(t, UNITS).unwrap_or(Affine::IDENTITY) };
                g
            })
            .collect();
        out.push(Node::group(Key::name("labels"), groups));
    }
    out
}

fn tile_key(t: TileId) -> Key {
    Key::new(vec![KeyPart::Int(t.z as i64), KeyPart::Int(t.x as i64), KeyPart::Int(t.y as i64)])
}

/// The key of what was drawn for square `t` before, under data fading in over it.
fn under_key(t: TileId) -> Key {
    Key::new(vec![KeyPart::Str("under".into()), KeyPart::Int(t.z as i64), KeyPart::Int(t.x as i64), KeyPart::Int(t.y as i64)])
}

/// The key of what was drawn for square `t` before, fading out over a coarser square's data.
fn over_key(t: TileId) -> Key {
    Key::new(vec![KeyPart::Str("over".into()), KeyPart::Int(t.z as i64), KeyPart::Int(t.x as i64), KeyPart::Int(t.y as i64)])
}

/// What was drawn before (`from`), clipped to square `sq`.
fn before_group(vis: &Visible, b: &Binding, sq: TileId, from: &Shown, xf: Affine, reprojected: bool, key: Key) -> Node {
    let before = Draw { square: sq, data: from.data, tile: from.tile.clone() };
    let before_xf = if reprojected { Affine::IDENTITY } else { b.proj.tile_transform(before.data, UNITS).unwrap_or(Affine::IDENTITY) };
    let mut g = Node::group(key, from.nodes.layers.clone());
    g.common.transform = before_xf;
    g.common.clip = square_clip(vis, b, &before, xf, before_xf, reprojected);
    g
}

/// Where `square` lies in `data`'s tile units (an ancestor covers its descendants).
fn square_in(square: TileId, data: TileId) -> (Vec2, f64) {
    let dz = square.z - data.z;
    let size = UNITS / (1u64 << dz) as f64;
    let ox = (square.x - (data.x << dz)) as f64 * size;
    let oy = (square.y - (data.y << dz)) as f64 * size;
    (Vec2::new(ox, oy), size)
}

/// The clip for one drawn square, in the drawing tile's units. Mercator squares snap to whole
/// screen pixels — computed from the shared grid lines, so neighbours meet exactly and no seam of
/// background shows through antialiased edges. Reprojected squares clip to their projected outline.
fn square_clip(vis: &Visible, b: &Binding, d: &Draw, xf: Affine, tile_xf: Affine, reprojected: bool) -> Option<Clip> {
    if reprojected {
        let p = b.proj.projector();
        let mut path = PathData::new();
        let n = 16;
        let t = d.square;
        let edge = |i: usize| -> Vec2 {
            let s = i as f64 / n as f64;
            match i / n {
                0 => Vec2::new(s, 0.0),
                1 => Vec2::new(1.0, s - 1.0),
                2 => Vec2::new(3.0 - s, 1.0),
                _ => Vec2::new(0.0, 4.0 - s),
            }
        };
        for i in 0..4 * n {
            let q = edge(i) * UNITS;
            let zz = (1u64 << t.z) as f64;
            let ll = datars_geo::tile::world_to_lonlat(Vec2::new((t.x as f64 + q.x / UNITS) / zz, (t.y as f64 + q.y / UNITS) / zz));
            let v = p.forward(ll)?;
            if i == 0 {
                path.move_to(v);
            } else {
                path.line_to(v);
            }
        }
        path.close();
        return Some(Clip::Path { path: Arc::new(path) });
    }
    let (o, size) = square_in(d.square, d.data);
    let exact = Rect::new(o.x, o.y, size, size);
    let (Some(w2s), Some(inv)) = (vis.world_to_screen, xf.mul(tile_xf).inverse()) else { return Some(Clip::Rect { rect: exact }) };
    if w2s.0[1] != 0.0 || w2s.0[2] != 0.0 {
        return Some(Clip::Rect { rect: exact }); // a rotated camera: no pixel grid to snap to
    }
    let t = d.square;
    let n = (1u64 << t.z) as f64;
    let gx = |i: u32| w2s.apply(Vec2::new(i as f64 / n, 0.0)).x.round();
    let gy = |j: u32| w2s.apply(Vec2::new(0.0, j as f64 / n)).y.round();
    let a = inv.apply(Vec2::new(gx(t.x), gy(t.y)));
    let c = inv.apply(Vec2::new(gx(t.x + 1), gy(t.y + 1)));
    Some(Clip::Rect { rect: Rect::from_points(a, c) })
}

/// A tile's nodes styled for `zoom`, and whether they're a nearby zoom's standing in (this frame's
/// styling budget is spent: see [`super::BUILD_FEATURES_PER_FRAME`]). `None` when the budget is spent, the
/// tile was never styled and `defer` lets it wait (its caller draws an ancestor's nodes instead).
fn built(cx: &FillCx, st: &mut TileState, b: &Binding, tile: &Arc<DecodedTile>, zoom: u8, reprojected: bool, defer: bool) -> Option<(Rc<TileNodes>, bool)> {
    let key = (b.fingerprint, b.source.clone(), tile.id, zoom);
    if let Some(n) = st.built.get(&key) {
        return Some((n, false));
    }
    // What styling it costs, and drawing it the first time: its features, or half its path
    // elements where its geometry is heavier — a street tile's buildings batch into one path of
    // 38,000 elements, 12 ms to tessellate natively, 30 in a browser (styling a feature is cheap).
    let features: usize = tile.layers.iter().map(|l| l.geoms.len()).sum();
    let elements: usize = tile.layers.iter().flat_map(|l| l.geoms.iter()).map(|g| match g {
        datars_scene::Geom::Path { path } => path.els.len(),
        _ => 1,
    }).sum();
    let cost = features.max(elements / 2);
    // Over budget: none left, or not enough for this tile and it isn't the frame's first. (A heavy
    // tile doesn't wait for a flight's landing: what stands in for it — its ancestor — would have
    // to be styled at the new zoom, the root tile at 28,800 features, heavier still.)
    // (Nor, the frame a big tile was decoded: they take turns, see `TileState::decoded_heavy`.)
    if st.builds_left.is_some_and(|left| left == 0 || (cost > left && (st.built_now > 0 || st.decoded_heavy))) {
        let turn = st.built_now == 0 && st.decoded_heavy;
        if let Some(n) = built_near(st, b, tile.id, zoom, &[-1, 1, -2, 2]) {
            st.style_waits |= turn;
            return Some((n, true));
        }
        if defer {
            st.style_waits |= turn;
            return None;
        }
    }
    st.builds_left = st.builds_left.map(|n| n.saturating_sub(cost.max(1)));
    st.built_now = st.built_now.saturating_add(1);
    st.stats.built_features += features as u64;
    let mut diags = Vec::new();
    let n = Rc::new(build(&cx.env, b, tile, zoom, reprojected, &st.exprs(), &mut diags));
    st.stats.built += 1;
    for d in diags {
        if !st.diags.contains(&d) {
            st.diags.push(d);
        }
    }
    st.built.insert(key, n.clone());
    Some((n, false))
}

/// A tile's nodes already styled at `zoom` shifted by one of `dz`, if any.
fn built_near(st: &mut TileState, b: &Binding, t: TileId, zoom: u8, dz: &[i16]) -> Option<Rc<TileNodes>> {
    dz.iter().find_map(|dz| {
        let z = zoom as i16 + dz;
        (0..=u8::MAX as i16).contains(&z).then(|| st.built.get(&(b.fingerprint, b.source.clone(), t, z as u8))).flatten()
    })
}

/// The nearest ancestor of `t` already styled (at `zoom` or near it): the stand-in for a tile
/// whose styling waits for a frame with budget.
fn built_ancestor(st: &mut TileState, b: &Binding, t: TileId, zoom: u8) -> Option<(TileId, Rc<TileNodes>)> {
    (1..=t.z.min(MAX_FALLBACK)).find_map(|up| {
        let a = t.ancestor(up);
        built_near(st, b, a, zoom, &[0, -1, 1, -2, 2]).map(|n| (a, n))
    })
}

/// The label's box on screen.
fn label_box(n: &Node, m: &Affine) -> Option<Rect> {
    let xf = m.mul(n.common.transform);
    let r = match &n.kind {
        NodeKind::Text(t) if t.screen_size => {
            let o = xf.apply(t.origin) + t.offset;
            Rect::new(o.x + t.bounds.x, o.y + t.bounds.y, t.bounds.w, t.bounds.h)
        }
        NodeKind::Instances(i) if i.screen_size => (0..i.len()).fold(Rect::empty(), |r, k| {
            let c = xf.apply(Vec2::new(i.x[k], i.y[k]));
            let s = i.size_at(k);
            r.union(&Rect::new(c.x - s, c.y - s, 2.0 * s, 2.0 * s))
        }),
        NodeKind::Group { children } => children.iter().filter_map(|c| label_box(c, &xf)).fold(Rect::empty(), |a, r| a.union(&r)),
        _ => transform_rect(node_bounds(n), m),
    };
    (!r.is_empty()).then_some(r)
}

/// A label that could be placed in this view.
struct Candidate {
    priority: f64,
    order: usize,
    tile: TileId,
    /// Tile units → screen.
    m: Affine,
    node: Node,
    id: LabelId,
}

/// A label's text (its first text node) and its anchor in world units, in ~4 km cells.
fn label_id(n: &Node, tile: TileId, unit: Vec2) -> LabelId {
    fn text(n: &Node) -> Option<String> {
        match &n.kind {
            NodeKind::Text(t) => Some(t.text.to_string()),
            NodeKind::Group { children } => children.iter().find_map(text),
            _ => None,
        }
    }
    let z = (1u64 << tile.z) as f64;
    let (wx, wy) = ((tile.x as f64 + unit.x / UNITS) / z, (tile.y as f64 + unit.y / UNITS) / z);
    (text(n).unwrap_or_default(), (wx * 10_000.0).round() as i64, (wy * 10_000.0).round() as i64)
}

/// Greedy placement: by priority (highest first, then the order found), keeping labels that fit
/// inside the view and clear of every label already placed. Returns the placed candidates'
/// indices in placement order.
fn place_labels(cands: &[Candidate], clip: Rect, avoid: &[Rect]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..cands.len()).collect();
    idx.sort_by(|&a, &b| total_cmp(cands[b].priority, cands[a].priority).then(cands[a].order.cmp(&cands[b].order)));
    let mut boxes: Vec<Rect> = Vec::new();
    let mut out = Vec::new();
    for i in idx {
        if boxes.len() >= MAX_LABELS {
            break;
        }
        let c = &cands[i];
        let Some(bx) = label_box(&c.node, &c.m) else { continue };
        if bx.x < clip.x || bx.y < clip.y || bx.x1() > clip.x1() || bx.y1() > clip.y1() {
            continue;
        }
        let padded = bx.inset(-LABEL_GAP);
        if boxes.iter().chain(avoid).any(|a| a.intersects(&padded)) {
            continue;
        }
        boxes.push(bx);
        out.push(i);
    }
    out
}
