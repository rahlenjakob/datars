//! Building a plan: flatten both scenes, resolve rules, match, merge structure, create tracks
//! (in place, in the flight layer, or column-wise), then choreograph windows.

use crate::choreo::{windows, Item, Phase};
use crate::columns::{ColItem, ColumnTrack, InstState, COLUMN_MAX};
use crate::easing::Easing;
use crate::elements::{content_bounds, flight_key, node_kind, Child, Flat, Src};
use crate::ghost::{Ghost, GhostFrom};
use crate::matching::{match_elements, Correspondence};
use crate::outline::partition_weighted;
use crate::plan::{path_hash, Follow, GeomPlan, NodeTrack, Plan, PlanStats, Slot, StructTrack};
use crate::route::RouteCx;
use crate::rules::{Choreography, Matcher, MotionRules, Resolved, Resolver, Target, DEFAULT_DURATION};
use crate::PlanCx;
use datars_math::{Affine, Vec2};
use datars_scene::{Common, Geom, Key, KeyPath, Node, NodeKind, Paint, Scene};
use datars_theme::Ink;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Home {
    /// Interpolates in local coordinates inside this merged structure node.
    InPlace(usize),
    /// Flies in root-content coordinates, mirrored under this destination parent path.
    Flight(KeyPath),
}

struct PendingNode {
    track: NodeTrack,
    home: Home,
    res: usize,
    center: Vec2,
    value: Option<f64>,
    /// Data order for staggers: the target's tree order, then exits in source tree order.
    order: usize,
}

#[derive(Default)]
struct ColBuilder {
    a: Option<usize>,
    b: Option<usize>,
    pairs: Vec<(usize, usize)>,
    enters: Vec<usize>,
    exits: Vec<usize>,
    /// Instances a shape split into, and the piece of it each starts as (a rect in the column's
    /// space): they stay in the column, one instanced draw, instead of flying one by one.
    pieces: Vec<(usize, InstState)>,
    flyers_a: BTreeSet<usize>,
    flyers_b: BTreeSet<usize>,
}

/// One scene's flattened elements plus per-element bookkeeping.
struct Side {
    f: Flat,
    elem_res: Vec<usize>,
    inst_res: Vec<usize>,
    struct_res: Vec<usize>,
    /// Flat struct index → merged struct index.
    home: Vec<Option<usize>>,
    /// Instances node index → column builder.
    col_of: Vec<Option<usize>>,
    /// Element index → node track.
    node_of: Vec<Option<usize>>,
    by_path: BTreeMap<KeyPath, usize>,
    by_key: BTreeMap<Key, usize>,
}

/// Keys of the data elements a scene draws as nodes (bars, regions): what instanced marks on the
/// other side may be the parts of.
fn data_keys(scene: &Scene) -> BTreeSet<Key> {
    let mut out = BTreeSet::new();
    scene.root.walk(&KeyPath::default(), &mut |_, n| {
        if !matches!(n.kind, datars_scene::NodeKind::Instances(_)) && n.semantics.as_ref().is_some_and(|s| matches!(s.role, datars_scene::Role::Datum | datars_scene::Role::Region)) {
            out.insert(n.key.clone());
        }
    });
    out
}

fn inst_res(f: &Flat, res: &mut Resolver) -> Vec<usize> {
    f.insts.iter().map(|n| res.resolve(&Target { path: &n.path, role: n.role, kind: "instance", is_shape: false })).collect()
}

/// Which instances nodes become one element per instance. By path, instances move as a column
/// (cheap for 10⁵ points); they become elements when a matcher needs them, or when the other scene
/// has the data elements they're parts of (seats `("S", i)` and the bar `("S",)`), so they can
/// split from / merge into them.
///
/// By key, a column still suffices when the only thing on the other side carrying these keys is
/// the instances node at the same path: pairing by key inside the node is what by-key matching
/// would do (same full path first). Materializing costs ~1 KB per instance (an element, its key
/// path, its track), which for a dot map of 400,000 dots is most of a gigabyte — per transition,
/// including the one when the data arrives.
fn materialized(fa: &Flat, fb: &Flat, ra: &[usize], rb: &[usize], table: &[Resolved], keys_from: &BTreeSet<Key>, keys_to: &BTreeSet<Key>) -> (Vec<bool>, Vec<bool>) {
    let decide = |f: &Flat, o: &Flat, r: &[usize], other_parents: &BTreeSet<Key>| -> Vec<bool> {
        (0..f.insts.len())
            .map(|k| {
                let node = &f.insts[k];
                if node.inst().len() > COLUMN_MAX {
                    // Crossfades whole whatever it would match (see `COLUMN_MAX`).
                    return false;
                }
                let parts_of_other = node.inst().keys.iter().any(|key| key.parent().is_some_and(|p| other_parents.contains(&p)));
                match table[r[k]].matcher {
                    _ if parts_of_other => true,
                    Matcher::ByPath => false,
                    Matcher::ByKey => !column_suffices(node, o),
                    _ => true,
                }
            })
            .collect()
    };
    let (mut ma, mut mb) = (decide(fa, fb, ra, keys_to), decide(fb, fa, rb, keys_from));
    // A column pairs two unmaterialized nodes at one path; if either side needs elements, both do
    // (elements can't pair with instances left in a column).
    for (kb, node) in fb.insts.iter().enumerate() {
        if let Some(&ka) = fa.inst_by_path.get(&node.path) {
            let huge = fa.insts[ka].inst().len().max(node.inst().len()) > COLUMN_MAX;
            let both = (ma[ka] || mb[kb]) && !huge;
            ma[ka] = both;
            mb[kb] = both;
        }
    }
    (ma, mb)
}

/// Can by-key matching of `node`'s instances be left to its column? Yes when its keys are unique
/// and none of them is carried on the other side by anything but the instances node at the same
/// path (whose keys must be unique too, so pairing inside the column is 1:1 like `by_key`).
fn column_suffices(node: &crate::elements::InstNode, other: &Flat) -> bool {
    let keys = &node.inst().keys;
    let mut own: BTreeSet<&Key> = BTreeSet::new();
    if !keys.iter().all(|k| own.insert(k)) {
        return false;
    }
    if other.elems.iter().any(|e| matches!(e.src, Src::Node(_)) && own.contains(&e.key)) {
        return false;
    }
    other.insts.iter().all(|o| {
        let ok = &o.inst().keys;
        if o.path == node.path {
            let mut seen: BTreeSet<&Key> = BTreeSet::new();
            ok.iter().all(|k| seen.insert(k))
        } else {
            !ok.iter().any(|k| own.contains(k))
        }
    })
}

fn side(mut f: Flat, inst_res: Vec<usize>, materialize: &[bool], res: &mut Resolver) -> Side {
    for (k, &m) in materialize.iter().enumerate() {
        if m {
            f.materialize(k);
        }
    }
    let elem_res: Vec<usize> = f
        .elems
        .iter()
        .map(|e| match e.src {
            Src::Node(_) => res.resolve(&Target { path: &e.path, role: e.role, kind: e.kind, is_shape: e.is_shape }),
            Src::Instance { inst, .. } => inst_res[inst],
        })
        .collect();
    let struct_res: Vec<usize> = f
        .structs
        .iter()
        .map(|s| res.resolve(&Target { path: &s.path, role: s.node.semantics.as_ref().map(|x| x.role), kind: node_kind(&s.node).0, is_shape: false }))
        .collect();
    let (ns, ni, ne) = (f.structs.len(), f.insts.len(), f.elems.len());
    Side { f, elem_res, inst_res, struct_res, home: vec![None; ns], col_of: vec![None; ni], node_of: vec![None; ne], by_path: BTreeMap::new(), by_key: BTreeMap::new() }
}

impl Side {
    fn fill_lookups(&mut self) {
        for (i, e) in self.f.elems.iter().enumerate() {
            if let Src::Node(_) = e.src {
                self.by_path.insert(e.path.clone(), i);
                self.by_key.entry(e.key.clone()).or_insert(i);
            }
        }
    }

    /// Root-content centre of the parent datum (see `GhostFrom::Parent`): the element at the
    /// parent key path, else the element keyed by this key minus its last part.
    fn parent_center(&self, parent_path: &KeyPath, key: &Key) -> Option<Vec2> {
        if let Some(&i) = self.by_path.get(parent_path) {
            return Some(self.f.elems[i].center);
        }
        let pk = key.parent()?;
        self.by_key.get(&pk).map(|&i| self.f.elems[i].center)
    }

    fn info(&self, i: usize) -> EInfo {
        let e = &self.f.elems[i];
        EInfo {
            home: match e.src {
                Src::Node(_) => e.parent_struct.and_then(|p| self.home[p]),
                Src::Instance { .. } => None,
            },
            content_xf: e.parent_struct.map(|p| self.f.structs[p].content_xf),
            parent: e.parent.clone(),
            path_hash: path_hash(&e.path),
            key: e.key.clone(),
            center: e.center,
            value: e.value,
            inst: match e.src {
                Src::Instance { inst, i } => Some((inst, i)),
                Src::Node(_) => None,
            },
            parent_xf: e.parent_xf,
        }
    }

    /// An element's state in its placement space (flight = root content).
    fn state(&self, i: usize, flight: bool) -> Node {
        let e = &self.f.elems[i];
        match &e.src {
            Src::Node(n) => {
                let mut n = (**n).clone();
                if flight {
                    let eff = if n.common.visible { n.common.opacity } else { 0.0 };
                    n.common.transform = e.xf_root;
                    n.common.opacity = (e.acc_opacity * eff).clamp(0.0, 1.0);
                    n.common.visible = true;
                }
                n
            }
            Src::Instance { inst, i } => {
                let inode = &self.f.insts[*inst];
                let ins = inode.inst();
                let mut g = ins.geom(*i);
                if ins.screen_size {
                    let k = inode.xf_root.scale_factor();
                    if let Geom::Symbol { size, .. } = &mut g {
                        if k > 0.0 {
                            *size /= k;
                        }
                    }
                }
                Node {
                    key: ins.keys[*i].clone(),
                    kind: NodeKind::Shape { geom: g, fill: Some(Paint::Solid(ins.fill_at(*i))), stroke: ins.stroke.clone(), markers: None },
                    common: Common { transform: inode.xf_root, opacity: (inode.acc_opacity * inode.node_opacity() * ins.opacity_at(*i)).clamp(0.0, 1.0), ..Common::default() },
                    semantics: None,
                    pickable: inode.node.pickable,
                    anchors: Vec::new(),
                    prov: inode.node.prov,
                }
            }
        }
    }
}

struct EInfo {
    home: Option<usize>,
    content_xf: Option<Affine>,
    parent: KeyPath,
    path_hash: u64,
    key: Key,
    center: Vec2,
    value: Option<f64>,
    inst: Option<(usize, usize)>,
    parent_xf: Affine,
}

fn piece_state(base: &Node, geom: Geom) -> Node {
    let mut n = base.clone();
    let (fill, stroke) = match &base.kind {
        NodeKind::Shape { fill, stroke, .. } => (fill.clone(), stroke.clone()),
        NodeKind::Text(t) => (Some(Paint::Solid(t.style.ink.clone())), None),
        _ => (Some(Paint::Solid(Ink::token("muted"))), None),
    };
    n.kind = NodeKind::Shape { geom, fill, stroke, markers: None };
    n
}

fn geom_of(n: &Node) -> Geom {
    match &n.kind {
        NodeKind::Shape { geom, .. } => geom.clone(),
        _ => {
            let (b, _) = content_bounds(n);
            Geom::rect(b.x, b.y, b.w, b.h)
        }
    }
}

/// A split's pieces as column rows: when every piece becomes an instance of one column and the
/// parent is a rect with a solid fill and no stroke (a bar breaking into dots), each piece's
/// rect in the column's space. `None` otherwise (the pieces fly as shapes).
fn pieces_in_column(base: &Node, pieces: &[Geom], insts: &[Option<(usize, usize)>], col_of: &[Option<usize>], inst_xf: impl Fn(usize) -> Affine) -> Option<(usize, Vec<InstState>)> {
    let NodeKind::Shape { fill: Some(Paint::Solid(ink)), stroke: None, .. } = &base.kind else { return None };
    let mut col = None;
    for ii in insts {
        let (k, _) = (*ii)?;
        let c = col_of[k]?;
        if col.is_some_and(|x| x != c) {
            return None;
        }
        col = Some(c);
    }
    let (k, _) = insts.first().copied().flatten()?;
    let to_local = inst_xf(k).inverse()?;
    let xf = to_local.mul(base.common.transform);
    // Axis-aligned only: a rotated piece isn't a rect in the column's space.
    if xf.0[1].abs() > 1e-9 || xf.0[2].abs() > 1e-9 {
        return None;
    }
    let opacity = if base.common.visible { base.common.opacity } else { 0.0 };
    let states = pieces
        .iter()
        .map(|g| {
            let Geom::Rect { x, y, w, h, .. } = g else { return None };
            let (p, q) = (xf.apply(Vec2::new(*x, *y)), xf.apply(Vec2::new(x + w, y + h)));
            Some(InstState { x: p.x.min(q.x), y: p.y.min(q.y), size: 0.0, w: (q.x - p.x).abs(), h: (q.y - p.y).abs(), opacity, fill: ink.clone() })
        })
        .collect::<Option<Vec<_>>>()?;
    Some((col?, states))
}

/// The ghost of an element state (`target` in root-content coordinates).
fn ghost_node(state: &Node, g: &Ghost, target_root: Option<Vec2>, placement_to_root: Affine) -> Node {
    let (lb, lc) = content_bounds(state);
    let target = target_root.and_then(|p| placement_to_root.inverse().map(|inv| inv.apply(p)));
    let mut n = state.clone();
    n.common.transform = g.transform(state.common.transform, lb, lc, target);
    n.common.opacity *= g.opacity_factor();
    if matches!(state.kind, NodeKind::Shape { .. }) {
        n.common.trim = g.trimmed(state.common.trim);
    }
    // Screen-size text keeps its size under any scale, so a "grow from nothing" ghost would be a
    // full-size word parked at the origin: it fades by the scale instead.
    if let (NodeKind::Text(t), Some(k)) = (&state.kind, g.scale_factor()) {
        if t.screen_size {
            n.common.opacity *= k.clamp(0.0, 1.0);
        }
    }
    n
}

fn ghost_target(g: &Ghost, other: &Side, parent: &KeyPath, key: &Key, event: Option<Vec2>) -> Option<Vec2> {
    match g.from? {
        GhostFrom::Parent => other.parent_center(parent, key),
        GhostFrom::Point(p) => Some(p),
        GhostFrom::Event => event,
    }
}

fn mark_flyer(cols: &mut [ColBuilder], col_a: &[Option<usize>], col_b: &[Option<usize>], ia: Option<(usize, usize)>, ib: Option<(usize, usize)>) {
    if let Some((k, i)) = ia {
        if let Some(c) = col_a[k] {
            cols[c].flyers_a.insert(i);
        }
    }
    if let Some((k, j)) = ib {
        if let Some(c) = col_b[k] {
            cols[c].flyers_b.insert(j);
        }
    }
}

/// An instances column riding a line: every instance of its nodes sits on a vertex of an open
/// polyline in the same group (a line chart's dots). Interpolated on their own, dots and line part
/// ways — the line's vertices don't pair up with the dots' keys when points come and go, and each
/// has its own window and easing — so riders move with the line instead: their keys anchor its
/// vertices ([`crate::lines`]), they share its window and easing, entering ones start where their
/// vertex starts on the old line (exiting ones end on the new line), and when the line draws on or
/// off by a trim each appears or goes as the trim's end passes it.
struct Riders {
    /// The line's node track.
    track: usize,
    /// From-instance → vertex of the from-line; to-instance → vertex of the to-line.
    va: Vec<usize>,
    vb: Vec<usize>,
    /// To-line vertex → where it starts, in the column's space at the start (`None`: in place).
    enter_from: Option<Vec<Vec2>>,
    /// From-line vertex → where it ends, in the column's space at the end.
    exit_to: Option<Vec<Vec2>>,
    reveal: Option<Reveal>,
}

/// A line drawn on or off by a trim whose end moves from `from` to `to` (fractions of the drawn
/// length) with the line's eased progress.
struct Reveal {
    from: f64,
    to: f64,
    /// Each to-line vertex's place along the drawn line (entering riders appear there), and each
    /// from-line vertex's (exiting riders go there).
    fb: Option<Vec<f64>>,
    fa: Option<Vec<f64>>,
}

/// Share of the line's window a revealed rider takes to fade in or out.
const REVEAL_FADE: f64 = 0.1;

/// A node track carried along a line: (the track, the line's track, where the line's pen reveals
/// or retracts past it — its place along the line's extra part, and whether it's arriving).
type Follower = (usize, usize, Option<(f64, bool)>);
/// A follower's window, easing, and its line's window.
type FollowTiming = ((f64, f64), Easing, (f64, f64));

/// A line track re-planned on its riders' keys: the anchors used, and — when it grows along its
/// path — where (at its end, growing, the junction vertex of the longer line; see `run_on`).
type Anchored = (usize, Vec<(usize, usize)>, Option<(bool, bool, usize)>);

/// Does one line run on from the other at one end — every vertex of the shorter one anchored, in
/// order, to one run of the longer one's at its start or its end? Then `(at_end, growing, the
/// junction vertex of the longer line)`; `growing`: the longer is the new one.
fn run_on(anchors: &[(usize, usize)], na: usize, nb: usize) -> Option<(bool, bool, usize)> {
    if na == nb || anchors.is_empty() {
        return None;
    }
    let growing = nb > na;
    let (short, long) = if growing { (na, nb) } else { (nb, na) };
    let (on_short, on_long): (Vec<usize>, Vec<usize>) = anchors.iter().map(|&(i, j)| if growing { (i, j) } else { (j, i) }).unzip();
    if on_short != (0..short).collect::<Vec<_>>() {
        return None;
    }
    if on_long == (0..short).collect::<Vec<_>>() {
        Some((true, growing, short - 1))
    } else if on_long == (long - short..long).collect::<Vec<_>>() {
        Some((false, growing, long - short))
    } else {
        None
    }
}

impl Reveal {
    /// The rider's window within the line window `w`: opening when the trim's end reaches `s`
    /// (entering) or closing when it passes back over it (exiting). `ease[k]` is the eased
    /// progress at k / (len − 1).
    fn window(&self, w: (f64, f64), ease: &[f64], s: f64, entering: bool) -> (f64, f64) {
        let end = |e: f64| (self.from + (self.to - self.from) * e).clamp(0.0, 1.0);
        let reached = |v: f64| if entering { v >= s - 1e-9 } else { v <= s + 1e-9 };
        let n = ease.len().saturating_sub(1).max(1);
        let u = match ease.iter().position(|&e| reached(end(e))) {
            None => 1.0,
            Some(0) => 0.0,
            Some(k) => {
                // Between samples k − 1 and k, where the end crosses s.
                let (v0, v1) = (end(ease[k - 1]), end(ease[k]));
                let f = if (v1 - v0).abs() > 1e-15 { ((s - v0) / (v1 - v0)).clamp(0.0, 1.0) } else { 1.0 };
                ((k - 1) as f64 + f) / n as f64
            }
        };
        let l = w.1 - w.0;
        let at = w.0 + u * l;
        if entering {
            (at, (at + REVEAL_FADE * l).min(w.1))
        } else {
            ((at - REVEAL_FADE * l).max(w.0), at)
        }
    }
}

/// Which struct holds each instances node as a direct child.
fn inst_parents(f: &Flat) -> Vec<Option<usize>> {
    let mut out = vec![None; f.insts.len()];
    for (s, st) in f.structs.iter().enumerate() {
        for c in &st.children {
            if let Child::Inst(k) = c {
                out[*k] = Some(s);
            }
        }
    }
    out
}

/// `from`'s local space → `to`'s (both given as local → root content).
fn space_map(from: Affine, to: Affine) -> Option<Affine> {
    Some(to.inverse()?.mul(from))
}

/// The vertex of line element `li` each instance of instances node `k` sits on — every instance
/// on a vertex of its own, else `None` — and the map from the line's space to the node's.
fn riding(f: &Flat, k: usize, li: usize) -> Option<(Vec<usize>, Affine)> {
    let e = &f.elems[li];
    let Src::Node(n) = &e.src else { return None };
    let NodeKind::Shape { geom: Geom::Polyline { pts, closed: false, .. }, .. } = &n.kind else { return None };
    let node = &f.insts[k];
    let len = node.inst().len();
    if pts.len() < 2 || len == 0 || len > pts.len() {
        return None;
    }
    let xf = space_map(e.xf_root, node.xf_root)?;
    let verts: Vec<Vec2> = pts.iter().map(|p| xf.apply(*p)).collect();
    let tol = 1e-6 * verts.iter().fold(1.0f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
    let on = |a: Vec2, b: Vec2| (a.x - b.x).abs() <= tol && (a.y - b.y).abs() <= tol;
    // Usually instance i sits on vertex i (both drawn from the same rows).
    if len == verts.len() && (0..len).all(|i| on(node.local_center(i), verts[i])) {
        return Some(((0..len).collect(), xf));
    }
    let mut order: Vec<usize> = (0..verts.len()).collect();
    order.sort_by(|&a, &b| datars_math::total_cmp(verts[a].x, verts[b].x).then(a.cmp(&b)));
    let xs: Vec<f64> = order.iter().map(|&v| verts[v].x).collect();
    let mut used = vec![false; verts.len()];
    let mut map = Vec::with_capacity(len);
    for i in 0..len {
        let c = node.local_center(i);
        let lo = xs.partition_point(|&x| x < c.x - tol);
        let v = order[lo..].iter().copied().take_while(|&v| verts[v].x <= c.x + tol).find(|&v| !used[v] && on(c, verts[v]))?;
        used[v] = true;
        map.push(v);
    }
    Some((map, xf))
}

fn polyline_of(n: &Node) -> Option<&Geom> {
    match &n.kind {
        NodeKind::Shape { geom: g @ Geom::Polyline { closed: false, .. }, .. } => Some(g),
        _ => None,
    }
}

fn trim_end(n: &Node) -> f64 {
    n.common.trim.map_or(1.0, |t| t[1])
}

struct Builder<'a> {
    cx: &'a PlanCx,
    a: Side,
    b: Side,
    table: Vec<Resolved>,
    structs: Vec<StructTrack>,
    seq: Vec<(Vec<Child>, Vec<Child>)>,
    struct_res: Vec<usize>,
    pending: Vec<PendingNode>,
    cols: Vec<ColBuilder>,
    stats: PlanStats,
    corr: Correspondence,
    root_mid: Vec2,
    root_span: f64,
    /// Node tracks carried along a line (follower, line), which take the line's timing — and for
    /// a callout arriving on a growing line's new part (or leaving on a part cut off), its place
    /// along that part, where the pen reveals (or retracts past) it.
    followers: Vec<Follower>,
}

/// Build a plan from `from` to `to` under `rules`: the expensive work (matching, choreography,
/// outline resampling, ink resolution) happens here, once.
pub fn plan(from: &Scene, to: &Scene, rules: &MotionRules, cx: &PlanCx) -> Plan {
    let mut res = Resolver::new(rules, cx.from_state.as_deref(), cx.to_state.as_deref());
    let (keys_from, keys_to) = (data_keys(from), data_keys(to));
    let (fa, fb) = (Flat::new(from), Flat::new(to));
    let (ra, rb) = (inst_res(&fa, &mut res), inst_res(&fb, &mut res));
    let (ma, mb) = materialized(&fa, &fb, &ra, &rb, &res.table, &keys_from, &keys_to);
    let mut a = side(fa, ra, &ma, &mut res);
    let mut b = side(fb, rb, &mb, &mut res);
    let table = res.table;
    let needs_parent = table.iter().any(|r| r.enter.from == Some(GhostFrom::Parent) || r.exit.from == Some(GhostFrom::Parent));
    if needs_parent {
        a.fill_lookups();
        b.fill_lookups();
    }
    let (w, h) = (to.width, to.height);
    let mut bld = Builder {
        cx,
        a,
        b,
        table,
        structs: Vec::new(),
        seq: Vec::new(),
        struct_res: Vec::new(),
        pending: Vec::new(),
        cols: Vec::new(),
        stats: PlanStats::default(),
        corr: Correspondence::default(),
        root_mid: Vec2::new(w / 2.0, h / 2.0),
        root_span: w.min(h).max(1.0),
        followers: Vec::new(),
    };
    let root = bld.merge_struct(Some(0), Some(0));
    bld.build_tracks();
    let riders = bld.bind_riders();
    bld.finish(from, to, root, riders)
}

/// Plan from a mid-flight scene (a frame of another plan, or any scene) to `to`. In-flight
/// elements keep their logical key paths (the flight layer is transparent to matching), so they
/// continue from where they are instead of crossfading; `plan.at(0)` is exactly `current`.
pub fn retarget(current: &Scene, to: &Scene, rules: &MotionRules, cx: &PlanCx) -> Plan {
    plan(current, to, rules, cx)
}

impl Builder<'_> {
    /// Merge structure recursively: struct children pair by key (first unpaired occurrence).
    fn merge_struct(&mut self, sa: Option<usize>, sb: Option<usize>) -> usize {
        let idx = self.structs.len();
        let na = sa.map(|i| self.a.f.structs[i].node.clone());
        let nb = sb.map(|j| self.b.f.structs[j].node.clone());
        let same = na.is_some() && na == nb;
        let virtual_root = idx == 0 && self.a.f.virtual_root && self.b.f.virtual_root;
        self.structs.push(StructTrack { a: na, b: nb, win: (0.0, 1.0), easing: Easing::default(), children: Vec::new(), same, virtual_root });
        let r = sb.map(|j| self.b.struct_res[j]).or(sa.map(|i| self.a.struct_res[i])).unwrap_or(0);
        self.struct_res.push(r);
        self.seq.push((Vec::new(), Vec::new()));
        if let Some(i) = sa {
            self.a.home[i] = Some(idx);
        }
        if let Some(j) = sb {
            self.b.home[j] = Some(idx);
        }
        let ca: Vec<Child> = sa.map(|i| self.a.f.structs[i].children.clone()).unwrap_or_default();
        let cb: Vec<Child> = sb.map(|j| self.b.f.structs[j].children.clone()).unwrap_or_default();
        // Structs pair by key and structural kind: a view (camera, clip) never pairs with a plain
        // group — the view would lose its camera halfway through; it exits whole instead.
        let is_view = |n: &Node| matches!(n.kind, NodeKind::View { .. });
        let mut by_key_a: BTreeMap<(Key, bool), Vec<usize>> = BTreeMap::new();
        for c in &ca {
            if let Child::Struct(i) = c {
                let n = &self.a.f.structs[*i].node;
                by_key_a.entry((n.key.clone(), is_view(n))).or_default().push(*i);
            }
        }
        for v in by_key_a.values_mut() {
            v.reverse();
        }
        let mut paired_a: BTreeSet<usize> = BTreeSet::new();
        for c in &cb {
            if let Child::Struct(j) = c {
                let n = &self.b.f.structs[*j].node;
                let k = (n.key.clone(), is_view(n));
                let ia = by_key_a.get_mut(&k).and_then(|v| v.pop());
                if let Some(i) = ia {
                    paired_a.insert(i);
                }
                self.merge_struct(ia, Some(*j));
            }
        }
        for c in &ca {
            if let Child::Struct(i) = c {
                if !paired_a.contains(i) {
                    self.merge_struct(Some(*i), None);
                }
            }
        }
        self.seq[idx] = (ca, cb);
        idx
    }

    /// Route context and approximate screen scale of a placement space (`None` = root content).
    fn rcx_for(&self, content_xf: Option<Affine>) -> (RouteCx, f64) {
        match content_xf {
            None => (RouteCx { mid: self.root_mid, span: self.root_span }, 1.0),
            Some(xf) => {
                let s = xf.scale_factor();
                let s = if s > 0.0 && s.is_finite() { s } else { 1.0 };
                let mid = xf.inverse().map_or(self.root_mid, |inv| inv.apply(self.root_mid));
                (RouteCx { mid, span: self.root_span / s }, s)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_node(&mut self, a: Node, b: Node, phase: Phase, res: usize, home: Home, content_xf: Option<Affine>, hash: u64, center: Vec2, value: Option<f64>, order: usize) -> usize {
        let (rcx, scale) = self.rcx_for(content_xf);
        let track = NodeTrack::new(a, b, phase, &self.table[res], rcx, scale, hash, &self.cx.theme);
        if track.crossfade {
            self.stats.crossfades += 1;
        }
        if matches!(track.geom, GeomPlan::Morph(_)) {
            self.stats.morphs += 1;
        }
        if matches!(home, Home::Flight(_)) {
            self.stats.flyers += 1;
        }
        self.pending.push(PendingNode { track, home, res, center, value, order });
        self.pending.len() - 1
    }

    fn col_builder(&mut self, ka: Option<usize>, kb: Option<usize>) {
        let c = self.cols.len();
        self.cols.push(ColBuilder { a: ka, b: kb, ..Default::default() });
        if let Some(k) = ka {
            self.a.col_of[k] = Some(c);
        }
        if let Some(k) = kb {
            self.b.col_of[k] = Some(c);
        }
    }

    fn build_tracks(&mut self) {
        // Instances nodes pair by path.
        for kb in 0..self.b.f.insts.len() {
            let path = &self.b.f.insts[kb].path;
            let ka = self.a.f.inst_by_path.get(path).copied().filter(|&ka| self.a.col_of[ka].is_none());
            self.col_builder(ka, Some(kb));
        }
        for ka in 0..self.a.f.insts.len() {
            if self.a.col_of[ka].is_none() {
                self.col_builder(Some(ka), None);
            }
        }

        let all_a: Vec<usize> = (0..self.a.f.elems.len()).collect();
        let all_b: Vec<usize> = (0..self.b.f.elems.len()).collect();
        let ma: Vec<Matcher> = self.a.elem_res.iter().map(|&r| self.table[r].matcher.clone()).collect();
        let mb: Vec<Matcher> = self.b.elem_res.iter().map(|&r| self.table[r].matcher.clone()).collect();
        let corr = match_elements(&self.a.f, &self.b.f, &all_a, &all_b, &ma, &mb);
        let mut used_a = vec![false; self.a.f.elems.len()];
        let mut used_b = vec![false; self.b.f.elems.len()];
        let event = self.cx.event_point;

        for &(i, j) in &corr.pairs {
            used_a[i] = true;
            used_b[j] = true;
            self.stats.pairs += 1;
            self.corr.pairs.push((self.a.f.elems[i].path.clone(), self.b.f.elems[j].path.clone()));
            let (ia, ib) = (self.a.info(i), self.b.info(j));
            if let (Some((ka, ii)), Some((kb, jj))) = (ia.inst, ib.inst) {
                let (ca, cb) = (self.a.col_of[ka], self.b.col_of[kb]);
                if let (Some(ca), Some(cb)) = (ca, cb) {
                    if ca == cb {
                        self.cols[ca].pairs.push((ii, jj));
                        continue;
                    }
                }
            }
            let in_place = ia.home.is_some() && ia.home == ib.home;
            let (sa, sb) = (self.a.state(i, !in_place), self.b.state(j, !in_place));
            let (home, cxf) = match (in_place, ib.home) {
                (true, Some(m)) => (Home::InPlace(m), ib.content_xf),
                _ => (Home::Flight(ib.parent.clone()), None),
            };
            mark_flyer(&mut self.cols, &self.a.col_of, &self.b.col_of, ia.inst, ib.inst);
            let t = self.push_node(sa, sb, Phase::Update, self.b.elem_res[j], home, cxf, ib.path_hash, ib.center, ib.value, j);
            self.a.node_of[i] = Some(t);
            self.b.node_of[j] = Some(t);
        }

        for (i, js, part) in &corr.splits {
            used_a[*i] = true;
            self.stats.splits += 1;
            self.corr.splits.push((self.a.f.elems[*i].path.clone(), js.iter().map(|&j| self.b.f.elems[j].path.clone()).collect()));
            let ia = self.a.info(*i);
            let base = self.a.state(*i, true);
            let weights: Vec<f64> = js.iter().map(|&j| self.b.f.elems[j].value.unwrap_or(f64::NAN)).collect();
            let pieces = partition_weighted(&geom_of(&base), &weights, *part);
            let insts: Vec<Option<(usize, usize)>> = js.iter().map(|&j| self.b.info(j).inst).collect();
            if let Some((c, states)) = pieces_in_column(&base, &pieces, &insts, &self.b.col_of, |k| self.b.f.insts[k].xf_root) {
                for ((&j, ii), st) in js.iter().zip(&insts).zip(states) {
                    used_b[j] = true;
                    if let Some((_, jj)) = ii {
                        self.cols[c].pieces.push((*jj, st));
                    }
                }
                continue;
            }
            for (&j, piece) in js.iter().zip(pieces) {
                used_b[j] = true;
                let ib = self.b.info(j);
                mark_flyer(&mut self.cols, &self.a.col_of, &self.b.col_of, ia.inst, ib.inst);
                let sb = self.b.state(j, true);
                let mut pa = piece_state(&base, piece);
                pa.key = sb.key.clone();
                self.push_node(pa, sb, Phase::Update, self.b.elem_res[j], Home::Flight(ib.parent.clone()), None, ib.path_hash, ib.center, ib.value, j);
            }
        }

        for (is, j, part) in &corr.merges {
            used_b[*j] = true;
            self.stats.merges += 1;
            self.corr.merges.push((is.iter().map(|&i| self.a.f.elems[i].path.clone()).collect(), self.b.f.elems[*j].path.clone()));
            let ib = self.b.info(*j);
            let base = self.b.state(*j, true);
            let weights: Vec<f64> = is.iter().map(|&i| self.a.f.elems[i].value.unwrap_or(f64::NAN)).collect();
            let pieces = partition_weighted(&geom_of(&base), &weights, *part);
            for (&i, piece) in is.iter().zip(pieces) {
                used_a[i] = true;
                let ia = self.a.info(i);
                mark_flyer(&mut self.cols, &self.a.col_of, &self.b.col_of, ia.inst, ib.inst);
                let sa = self.a.state(i, true);
                let mut pb = piece_state(&base, piece);
                pb.key = sa.key.clone();
                let nb = self.b.f.elems.len();
                self.push_node(sa, pb, Phase::Update, self.b.elem_res[*j], Home::Flight(ib.parent.clone()), None, ia.path_hash, ia.center, ib.value, nb + i);
            }
        }

        // Exits: to their ghost, in their source container.
        for (i, used) in used_a.iter().enumerate() {
            if *used {
                continue;
            }
            let ia = self.a.info(i);
            if let Some((k, ii)) = ia.inst {
                if let Some(c) = self.a.col_of[k] {
                    self.cols[c].exits.push(ii);
                }
                continue;
            }
            self.stats.exits += 1;
            self.corr.exits.push(self.a.f.elems[i].path.clone());
            let res = self.a.elem_res[i];
            let flight = ia.home.is_none();
            let st = self.a.state(i, flight);
            let g = &self.table[res].exit;
            let target = if g.needs_target() { ghost_target(g, &self.b, &ia.parent, &ia.key, event) } else { None };
            let gs = ghost_node(&st, g, target, if flight { Affine::IDENTITY } else { ia.parent_xf });
            let (home, cxf) = match ia.home {
                Some(m) => (Home::InPlace(m), ia.content_xf),
                None => (Home::Flight(ia.parent.clone()), None),
            };
            let nb = self.b.f.elems.len();
            let t = self.push_node(st, gs, Phase::Exit, res, home, cxf, ia.path_hash, ia.center, ia.value, nb + i);
            self.a.node_of[i] = Some(t);
        }
        // Enters: from their ghost, in their target container.
        for (j, used) in used_b.iter().enumerate() {
            if *used {
                continue;
            }
            let ib = self.b.info(j);
            if let Some((k, jj)) = ib.inst {
                if let Some(c) = self.b.col_of[k] {
                    self.cols[c].enters.push(jj);
                }
                continue;
            }
            self.stats.enters += 1;
            self.corr.enters.push(self.b.f.elems[j].path.clone());
            let res = self.b.elem_res[j];
            let flight = ib.home.is_none();
            let st = self.b.state(j, flight);
            let g = &self.table[res].enter;
            let target = if g.needs_target() { ghost_target(g, &self.a, &ib.parent, &ib.key, event) } else { None };
            let gs = ghost_node(&st, g, target, if flight { Affine::IDENTITY } else { ib.parent_xf });
            let (home, cxf) = match ib.home {
                Some(m) => (Home::InPlace(m), ib.content_xf),
                None => (Home::Flight(ib.parent.clone()), None),
            };
            let t = self.push_node(gs, st, Phase::Enter, res, home, cxf, ib.path_hash, ib.center, ib.value, j);
            self.b.node_of[j] = Some(t);
        }
    }

    /// Columns riding a line (see [`Riders`]), by column; re-plans the lines they anchor.
    fn bind_riders(&mut self) -> Vec<Option<Riders>> {
        let (pa, pb) = (inst_parents(&self.a.f), inst_parents(&self.b.f));
        let track_elems = |side: &Side| -> BTreeMap<usize, usize> { side.node_of.iter().enumerate().filter_map(|(i, t)| t.map(|t| (t, i))).collect() };
        let (ea, eb) = (track_elems(&self.a), track_elems(&self.b));
        let mut anchored: Vec<Anchored> = Vec::new();
        let out: Vec<Option<Riders>> = (0..self.cols.len()).map(|c| self.riders(c, &pa, &pb, &ea, &eb, &mut anchored)).collect();
        self.stats.riders = out.iter().filter(|r| r.is_some()).count();
        self.follow_lines(&anchored, &ea, &eb);
        self.bind_followers(&anchored, &ea, &eb);
        out
    }

    /// Callouts on a line move with it. An annotation whose marker (a small dot) sits on a line's
    /// vertex follows that vertex as the line moves — leaving and arriving ones too, riding their
    /// data point while they fade — and one kept across the step travels along the line from its
    /// old point to its new one. The callout's other parts are carried with its marker, all on the
    /// line's window and easing. (Otherwise a leaving note stays where its point was while the line
    /// moves away under it, and an arriving one waits where the line hasn't reached yet.)
    fn bind_followers(&mut self, anchored: &[Anchored], ea: &BTreeMap<usize, usize>, eb: &BTreeMap<usize, usize>) {
        struct LineInfo {
            t: usize,
            /// Merged points (root content), and each vertex's merged index, per side.
            a: Vec<Vec2>,
            b: Vec<Vec2>,
            va: Vec<usize>,
            vb: Vec<usize>,
            /// The vertices as drawn (root content), per side.
            pa: Vec<Vec2>,
            pb: Vec<Vec2>,
            grow: Option<(usize, bool, bool)>,
        }
        let mut lines: Vec<LineInfo> = Vec::new();
        for t in 0..self.pending.len() {
            let tr = &self.pending[t].track;
            if tr.phase != Phase::Update || tr.crossfade {
                continue;
            }
            let (Some(ga), Some(gb)) = (polyline_of(&tr.a), polyline_of(&tr.b)) else { continue };
            let (Some(&ia), Some(&ib)) = (ea.get(&t), eb.get(&t)) else { continue };
            let (xa, xb) = (self.a.f.elems[ia].xf_root, self.b.f.elems[ib].xf_root);
            let anchors = anchored.iter().find(|x| x.0 == t).map_or(&[][..], |x| &x.1[..]);
            let Some(lp) = crate::plan::line_plan(ga, gb, anchors, 0.1 / tr.px.max(1e-9)) else { continue };
            let verts = |g: &Geom, xf: Affine| -> Vec<Vec2> {
                match g {
                    Geom::Polyline { pts, .. } => pts.iter().map(|p| xf.apply(*p)).collect(),
                    _ => Vec::new(),
                }
            };
            let grow = match tr.geom {
                GeomPlan::Grow { junction, at_end, growing, .. } => Some((junction, at_end, growing)),
                _ => None,
            };
            lines.push(LineInfo { t, a: lp.a.iter().map(|p| xa.apply(*p)).collect(), b: lp.b.iter().map(|p| xb.apply(*p)).collect(), va: lp.va, vb: lp.vb, pa: verts(ga, xa), pb: verts(gb, xb), grow });
        }
        if lines.is_empty() {
            return;
        }
        // A small round mark in an annotation: its centre in root content, and its parent struct.
        let marker = |side: &Side, i: usize| -> Option<(Vec2, usize)> {
            let e = &side.f.elems[i];
            let p = e.parent_struct?;
            let annotation = side.f.structs[p].node.semantics.as_ref().is_some_and(|s| s.role == datars_scene::Role::Annotation);
            let Src::Node(n) = &e.src else { return None };
            match n.kind {
                NodeKind::Shape { geom: Geom::Ellipse { cx, cy, rx, ry }, .. } if annotation && (rx - ry).abs() < 1e-9 && rx <= 8.0 => Some((e.xf_root.apply(Vec2::new(cx, cy)), p)),
                _ => None,
            }
        };
        let tol = 0.01;
        let vertex = |pts: &[Vec2], c: Vec2| pts.iter().position(|q| (q.x - c.x).abs() <= tol && (q.y - c.y).abs() <= tol);
        let mut follows: Vec<(usize, Follow)> = Vec::new();
        for t in 0..self.pending.len() {
            let phase = self.pending[t].track.phase;
            let ma = ea.get(&t).and_then(|&i| marker(&self.a, i));
            let mb = eb.get(&t).and_then(|&j| marker(&self.b, j));
            // The line it's on, the merged indices it goes from and to, and where it would be.
            let found = lines.iter().find_map(|l| {
                // A merged index on the growing line's extra part: its place along that part.
                let revealed = |m: usize| -> Option<f64> {
                    let (junction, at_end, growing) = l.grow?;
                    let src = if growing { &l.b } else { &l.a };
                    let order: Vec<usize> = if at_end { (junction..src.len()).collect() } else { (0..=junction).rev().collect() };
                    let pos = order.iter().position(|&k| k == m)?;
                    let params = crate::plan::extra_params(src, &order);
                    (pos > 0).then(|| params[pos] / params.last().copied().unwrap_or(0.0).max(1e-12))
                };
                let (from, to, lin) = match phase {
                    Phase::Update => {
                        let ((ca, _), (cb, _)) = (ma?, mb?);
                        (l.va[vertex(&l.pa, ca)?], l.vb[vertex(&l.pb, cb)?], (ca, cb))
                    }
                    Phase::Exit => {
                        let (ca, _) = ma?;
                        let m = l.va[vertex(&l.pa, ca)?];
                        (m, m, (ca, ca))
                    }
                    Phase::Enter => {
                        let (cb, _) = mb?;
                        let m = l.vb[vertex(&l.pb, cb)?];
                        (m, m, (cb, cb))
                    }
                };
                let reveal = match (phase, l.grow) {
                    (Phase::Enter, Some((_, _, true))) => revealed(to).map(|s| (s, true)),
                    (Phase::Exit, Some((_, _, false))) => revealed(from).map(|s| (s, false)),
                    _ => None,
                };
                Some((l.t, reveal, Follow { a: l.a.clone(), b: l.b.clone(), from, to, lin, grow: l.grow, to_parent: Affine::IDENTITY }))
            });
            let Some((line, reveal, f)) = found else { continue };
            // The callout's parts: every track under the marker's parent, on either side.
            let (pa, pb) = (ma.map(|m| m.1), mb.map(|m| m.1));
            for k in 0..self.pending.len() {
                let on_a = ea.get(&k).filter(|&&i| pa.is_some() && self.a.f.elems[i].parent_struct == pa).map(|&i| self.a.f.elems[i].parent_xf);
                let on_b = eb.get(&k).filter(|&&j| pb.is_some() && self.b.f.elems[j].parent_struct == pb).map(|&j| self.b.f.elems[j].parent_xf);
                let Some(parent) = on_b.or(on_a) else { continue };
                let Some(to_parent) = parent.inverse() else { continue };
                follows.push((k, Follow { to_parent, ..f.clone() }));
                self.followers.push((k, line, reveal));
            }
        }
        for (k, f) in follows {
            self.pending[k].track.follow = Some(std::sync::Arc::new(f));
        }
    }

    /// An area whose top edge is a line re-planned by its riders (an area under a line with dots)
    /// takes the line's anchors, so the two keep drawing the same edge.
    fn follow_lines(&mut self, anchored: &[Anchored], elem_a: &BTreeMap<usize, usize>, elem_b: &BTreeMap<usize, usize>) {
        if anchored.is_empty() {
            return;
        }
        // Points of a track's geometry on each side, in root content.
        let root_pts = |t: usize, top: bool| -> Option<(Vec<Vec2>, Vec<Vec2>)> {
            let tr = &self.pending[t].track;
            let pts = |n: &Node| -> Option<Vec<Vec2>> {
                match &n.kind {
                    NodeKind::Shape { geom: Geom::Area { top: p, .. }, .. } if top => Some(p.to_vec()),
                    NodeKind::Shape { geom: Geom::Polyline { pts: p, closed: false, .. }, .. } if !top => Some(p.to_vec()),
                    _ => None,
                }
            };
            let (xa, xb) = (self.a.f.elems[*elem_a.get(&t)?].xf_root, self.b.f.elems[*elem_b.get(&t)?].xf_root);
            Some((pts(&tr.a)?.iter().map(|p| xa.apply(*p)).collect(), pts(&tr.b)?.iter().map(|p| xb.apply(*p)).collect()))
        };
        let same = |x: &[Vec2], y: &[Vec2]| {
            let tol = 1e-6 * x.iter().fold(1.0f64, |m, p| m.max(p.x.abs()).max(p.y.abs()));
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| (p.x - q.x).abs() <= tol && (p.y - q.y).abs() <= tol)
        };
        let lines: Vec<_> = anchored.iter().filter_map(|(t, an, grow)| root_pts(*t, false).map(|(a, b)| (a, b, an, *grow))).collect();
        let mut replanned = Vec::new();
        for t in 0..self.pending.len() {
            let tr = &self.pending[t].track;
            if tr.phase != Phase::Update || tr.same || tr.crossfade || !matches!(tr.geom, GeomPlan::Param | GeomPlan::Points { .. }) {
                continue;
            }
            let Some((ta, tb)) = root_pts(t, true) else { continue };
            let Some((_, _, an, grow)) = lines.iter().find(|(la, lb, _, _)| same(la, &ta) && same(lb, &tb)) else { continue };
            let (NodeKind::Shape { geom: ga, .. }, NodeKind::Shape { geom: gb, .. }) = (&tr.a.kind, &tr.b.kind) else { continue };
            let tol = 0.1 / tr.px.max(1e-9);
            // Under a line that grows along its path, the area grows with it.
            let plan = match grow {
                Some((at_end, growing, vertex)) => crate::plan::area_grow_plan(ga, gb, an, tol, *vertex, *at_end, *growing),
                None => crate::plan::area_plan(ga, gb, an, tol),
            };
            if let Some(p) = plan {
                replanned.push((t, p));
            }
        }
        for (t, p) in replanned {
            self.pending[t].track.geom = p;
        }
    }

    /// `anchored`: gets the lines re-planned on their riders' keys, with the anchors used.
    #[allow(clippy::too_many_arguments)]
    fn riders(&mut self, c: usize, pa: &[Option<usize>], pb: &[Option<usize>], elem_a: &BTreeMap<usize, usize>, elem_b: &BTreeMap<usize, usize>, anchored: &mut Vec<Anchored>) -> Option<Riders> {
        let cb = &self.cols[c];
        if !cb.pieces.is_empty() {
            return None;
        }
        let (ka, kb) = (cb.a, cb.b);
        let column = |f: &Flat, k: usize| f.insts[k].elems.is_none() && f.insts[k].inst().len() <= COLUMN_MAX;
        if ka.is_some_and(|k| !column(&self.a.f, k)) || kb.is_some_and(|k| !column(&self.b.f, k)) {
            return None;
        }
        // The line a node rides: an open polyline among its siblings, animating in place there.
        let pending = &self.pending;
        let find = |side: &Side, parents: &[Option<usize>], k: usize| -> Option<(usize, Vec<usize>, Affine)> {
            let p = parents[k]?;
            let m = side.home[p]?;
            side.f.structs[p].children.iter().find_map(|ch| {
                let Child::Elem(i) = ch else { return None };
                let t = side.node_of[*i]?;
                if pending[t].home != Home::InPlace(m) {
                    return None;
                }
                let (map, xf) = riding(&side.f, k, *i)?;
                Some((t, map, xf))
            })
        };
        let ra = match ka {
            Some(k) => Some(find(&self.a, pa, k)?),
            None => None,
        };
        let rb = match kb {
            Some(k) => Some(find(&self.b, pb, k)?),
            None => None,
        };
        let t = match (&ra, &rb) {
            (Some(x), Some(y)) if x.0 == y.0 => x.0,
            (Some(x), None) => x.0,
            (None, Some(y)) => y.0,
            _ => return None,
        };
        let track = &self.pending[t].track;
        let (va, xa) = ra.map_or((Vec::new(), None), |(_, v, x)| (v, Some(x)));
        let (vb, xb) = rb.map_or((Vec::new(), None), |(_, v, x)| (v, Some(x)));
        let tol = 0.1 / track.px.max(1e-9);
        let fractions = |n: &Node| polyline_of(n).and_then(|g| match g {
            Geom::Polyline { pts, curve, .. } => Some(crate::lines::Line::new(pts, *curve).vertex_fractions(tol)),
            _ => None,
        });
        let mut riders = Riders { track: t, va, vb, enter_from: None, exit_to: None, reveal: None };
        match track.phase {
            Phase::Update => {
                let (Some(&ia), Some(&ib)) = (elem_a.get(&t), elem_b.get(&t)) else { return None };
                if track.same {
                    return None;
                }
                if let GeomPlan::Extend { from, to, long_b } = track.geom {
                    // The longer line is drawn, trimmed: riders on its new part appear as the trim
                    // reaches them (or go as it retracts past them).
                    let (fa, fb) = if long_b { (None, fractions(&track.b)) } else { (fractions(&track.a), None) };
                    riders.reveal = Some(Reveal { from, to, fa, fb });
                    return Some(riders);
                }
                if track.crossfade || !matches!(track.geom, GeomPlan::Param | GeomPlan::Points { .. }) {
                    return Some(riders);
                }
                // Anchors: the vertices under the same key on both sides.
                let anchors: Vec<(usize, usize)> = match (ka, kb) {
                    (Some(ka), Some(kb)) => {
                        let (keys_a, keys_b) = (&self.a.f.insts[ka].inst().keys, &self.b.f.insts[kb].inst().keys);
                        let mut at: BTreeMap<&Key, usize> = BTreeMap::new();
                        for (i, k) in keys_a.iter().enumerate() {
                            if at.insert(k, i).is_some() {
                                return None;
                            }
                        }
                        let mut seen: BTreeSet<&Key> = BTreeSet::new();
                        let mut out = Vec::new();
                        for (j, k) in keys_b.iter().enumerate() {
                            if !seen.insert(k) {
                                return None;
                            }
                            if let Some(&i) = at.get(k) {
                                out.push((riders.va[i], riders.vb[j]));
                            }
                        }
                        out
                    }
                    _ => Vec::new(),
                };
                let (Some(ga), Some(gb)) = (polyline_of(&track.a), polyline_of(&track.b)) else { return Some(riders) };
                let lp = crate::plan::line_plan(ga, gb, &anchors, tol)?;
                // Where vertices start and end, in the column's space at the start (the from-node's,
                // else the to-node's) and at the end (the to-node's, else the from-node's).
                let (line_a, line_b) = (self.a.f.elems[ia].xf_root, self.b.f.elems[ib].xf_root);
                let start = match (xa, kb) {
                    (Some(x), _) => Some(x),
                    (None, Some(kb)) => space_map(line_a, self.b.f.insts[kb].xf_root),
                    _ => None,
                };
                let end = match (xb, ka) {
                    (Some(x), _) => Some(x),
                    (None, Some(ka)) => space_map(line_b, self.a.f.insts[ka].xf_root),
                    _ => None,
                };
                // A line running on at one end (or cut back) while the part both share moves: the
                // new part is drawn on along its path, its riders appearing as the pen reaches
                // them (a cut part retracts, riders going as it passes them). See `GeomPlan::Grow`.
                let (na, nb) = (lp.va.len(), lp.vb.len());
                if let (Some((at_end, growing, vertex)), true) = (run_on(&anchors, na, nb), riders.va.len() == na && riders.vb.len() == nb) {
                    let junction = if growing { lp.vb[vertex] } else { lp.va[vertex] };
                    let (pts, verts) = if growing { (&lp.b, &lp.vb) } else { (&lp.a, &lp.va) };
                    // Each vertex's place along the extra part, as the pen measures it (0: on the
                    // shared part).
                    let order: Vec<usize> = if at_end { (junction..pts.len()).collect() } else { (0..=junction).rev().collect() };
                    let params = crate::plan::extra_params(pts, &order);
                    let total = params.last().copied().unwrap_or(0.0).max(1e-12);
                    let mut along = vec![0.0; pts.len()];
                    for (w, &k) in order.iter().enumerate() {
                        along[k] = params[w];
                    }
                    let fr: Vec<f64> = verts.iter().map(|&m| along[m] / total).collect();
                    riders.reveal = Some(if growing { Reveal { from: 0.0, to: 1.0, fa: None, fb: Some(fr) } } else { Reveal { from: 1.0, to: 0.0, fa: Some(fr), fb: None } });
                    // Riders on the extra part move with it: offset by the junction's move.
                    let (ja, jb) = (lp.a[junction], lp.b[junction]);
                    if growing {
                        riders.enter_from = start.map(|x| lp.vb.iter().map(|&m| x.apply(lp.b[m] + (ja - jb))).collect());
                    } else {
                        riders.exit_to = end.map(|x| lp.va.iter().map(|&m| x.apply(lp.a[m] + (jb - ja))).collect());
                    }
                    self.pending[t].track.geom = GeomPlan::Grow { a: vec![lp.a], b: vec![lp.b], area: false, curve: lp.curve, junction, at_end, growing };
                    anchored.push((t, anchors, Some((at_end, growing, vertex))));
                    return Some(riders);
                }
                riders.enter_from = start.map(|x| lp.enter_from.iter().map(|p| x.apply(*p)).collect());
                riders.exit_to = end.map(|x| lp.exit_to.iter().map(|p| x.apply(*p)).collect());
                if !lp.identity {
                    self.pending[t].track.geom = GeomPlan::Points { a: vec![lp.a], b: vec![lp.b], area: false, curve: lp.curve };
                    anchored.push((t, anchors, None));
                }
            }
            Phase::Enter | Phase::Exit => {
                // A line coming or going whole, with its riders.
                let entering = track.phase == Phase::Enter;
                let whole = if entering { ka.is_none() && kb.is_some() } else { ka.is_some() && kb.is_none() };
                if !whole {
                    return None;
                }
                let (from, to) = (trim_end(&track.a), trim_end(&track.b));
                if (from - to).abs() > 1e-9 {
                    let (fa, fb) = if entering { (None, fractions(&track.b)) } else { (fractions(&track.a), None) };
                    riders.reveal = Some(Reveal { from, to, fa, fb });
                }
            }
        }
        Some(riders)
    }

    fn finish(mut self, from: &Scene, to: &Scene, root: usize, riders: Vec<Option<Riders>>) -> Plan {
        struct ColPending {
            items: Vec<ColItem>,
            choreo: Vec<Item>,
            hashes: Vec<u64>,
            /// Per item: its phase and instance index (in the to-node; exits in the from-node).
            src: Vec<(Phase, usize)>,
            res: usize,
            rcx: RouteCx,
            /// Crossfades whole (`COLUMN_MAX`).
            bulk: bool,
        }
        let cols = std::mem::take(&mut self.cols);
        let mut col_pending: Vec<ColPending> = Vec::with_capacity(cols.len());
        let mut n_instances = 0usize;
        let event = self.cx.event_point;
        for (ci, cb) in cols.iter().enumerate() {
            let rider = riders[ci].as_ref();
            let na = cb.a.map(|k| &self.a.f.insts[k]);
            let nb = cb.b.map(|k| &self.b.f.insts[k]);
            let res = cb.b.map(|k| self.b.inst_res[k]).or(cb.a.map(|k| self.a.inst_res[k])).unwrap_or(0);
            let r = &self.table[res];
            let mat_a = na.is_some_and(|n| n.elems.is_some());
            let mat_b = nb.is_some_and(|n| n.elems.is_some());
            let len = |n: Option<&crate::elements::InstNode>| n.map_or(0, |n| n.inst().len());
            if !mat_a && !mat_b && len(na).max(len(nb)) > COLUMN_MAX {
                n_instances += len(na) + len(nb);
                let node = nb.or(na);
                let (rcx, _) = self.rcx_for(Some(node.map_or(Affine::IDENTITY, |n| n.xf_root)));
                col_pending.push(ColPending { items: Vec::new(), choreo: Vec::new(), hashes: Vec::new(), src: Vec::new(), res, rcx, bulk: true });
                continue;
            }
            let mut pairs = cb.pairs.clone();
            let mut enters = cb.enters.clone();
            let mut exits = cb.exits.clone();
            match (na, nb) {
                (Some(x), Some(y)) if !mat_a && !mat_b => {
                    // By-path instances pair by instance key inside their node.
                    let (ia, ib) = (x.inst(), y.inst());
                    let mut kb: BTreeMap<&Key, usize> = BTreeMap::new();
                    for (j, k) in ib.keys.iter().enumerate() {
                        kb.insert(k, j);
                    }
                    let mut used = vec![false; ib.len()];
                    for (i, k) in ia.keys.iter().enumerate() {
                        match kb.get(k) {
                            Some(&j) if !used[j] => {
                                used[j] = true;
                                pairs.push((i, j));
                            }
                            _ => exits.push(i),
                        }
                    }
                    enters.extend((0..ib.len()).filter(|&j| !used[j]));
                }
                _ => {
                    if let Some(x) = na {
                        if !mat_a {
                            exits.extend(0..x.inst().len());
                        }
                    }
                    if let Some(y) = nb {
                        if !mat_b {
                            enters.extend(0..y.inst().len());
                        }
                    }
                }
            }
            pairs.retain(|(i, j)| !cb.flyers_a.contains(i) && !cb.flyers_b.contains(j));
            exits.retain(|i| !cb.flyers_a.contains(i));
            enters.retain(|j| !cb.flyers_b.contains(j));
            exits.sort_unstable();
            exits.dedup();
            n_instances += pairs.len() + enters.len() + exits.len();
            let nbl = nb.map_or(0, |n| n.inst().len());
            // Output order: the target's instance order (pairs and enters), then exits.
            #[derive(Clone, Copy)]
            enum Fate {
                Pair(usize),
                Enter,
                Piece(usize),
            }
            let mut fate_b: Vec<Option<Fate>> = vec![None; nbl];
            for &(i, j) in &pairs {
                fate_b[j] = Some(Fate::Pair(i));
            }
            for (p, (j, _)) in cb.pieces.iter().enumerate() {
                if *j < nbl && fate_b[*j].is_none() {
                    fate_b[*j] = Some(Fate::Piece(p));
                }
            }
            for &j in &enters {
                if fate_b[j].is_none() {
                    fate_b[j] = Some(Fate::Enter);
                }
            }
            n_instances += cb.pieces.len();
            let node = nb.or(na);
            let local_to_root = node.map_or(Affine::IDENTITY, |n| n.xf_root);
            let (rcx, _) = self.rcx_for(Some(local_to_root));
            let base_hash = node.map_or(0, |n| path_hash(&n.path));
            let key_hash = |k: &Key| path_hash(&KeyPath(vec![k.clone()])) ^ base_hash.rotate_left(7);
            let order_base = self.a.f.elems.len() + self.b.f.elems.len() + col_pending.iter().map(|c| c.items.len()).sum::<usize>();
            let (mut items, mut choreo, mut hashes, mut src) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            if let Some(y_node) = nb {
                let y = y_node.inst();
                for (j, fate) in fate_b.iter().enumerate() {
                    let Some(fate) = fate else { continue };
                    let bs = InstState::of(y, j);
                    let key = y.keys[j].clone();
                    let mut from = None;
                    let (phase, as_) = match (fate, na) {
                        (Fate::Pair(i), Some(x)) => (Phase::Update, InstState::of(x.inst(), *i)),
                        (Fate::Piece(p), _) => {
                            from = Some(datars_scene::Proto::Rect);
                            (Phase::Update, cb.pieces[*p].1.clone())
                        }
                        _ => {
                            let g = &r.enter;
                            let target = if g.needs_target() {
                                ghost_target(g, &self.a, &y_node.path, &key, event).and_then(|p| local_to_root.inverse().map(|inv| inv.apply(p)))
                            } else {
                                None
                            };
                            let mut gs = bs.ghost(g, &y.proto, target);
                            // A rider starts where its vertex starts, on the old line.
                            if let Some(p) = rider.and_then(|r| r.enter_from.as_ref()?.get(*r.vb.get(j)?).copied()) {
                                gs = gs.centred(&y.proto, p);
                            }
                            (Phase::Enter, gs)
                        }
                    };
                    let h = key_hash(&key);
                    choreo.push(Item { group: r.choreo_src, phase, center: local_to_root.apply(bs.center(&y.proto)), order: order_base + j, value: None, hash: h, ws: 0.0, we: 1.0 });
                    hashes.push(h);
                    src.push((phase, j));
                    items.push(ColItem { key, phase, a: as_, b: bs, from });
                }
            }
            if let Some(x_node) = na {
                let x = x_node.inst();
                let xf_a = x_node.xf_root;
                for &i in &exits {
                    let s = InstState::of(x, i);
                    let key = x.keys[i].clone();
                    let g = &r.exit;
                    let target = if g.needs_target() {
                        ghost_target(g, &self.b, &x_node.path, &key, event).and_then(|p| xf_a.inverse().map(|inv| inv.apply(p)))
                    } else {
                        None
                    };
                    let mut gs = s.ghost(g, &x.proto, target);
                    // A rider ends where its vertex ends, on the new line.
                    if let Some(p) = rider.and_then(|r| r.exit_to.as_ref()?.get(*r.va.get(i)?).copied()) {
                        gs = gs.centred(&x.proto, p);
                    }
                    let h = key_hash(&key);
                    choreo.push(Item { group: r.choreo_src, phase: Phase::Exit, center: xf_a.apply(s.center(&x.proto)), order: order_base + nbl + i, value: None, hash: h, ws: 0.0, we: 1.0 });
                    hashes.push(h);
                    src.push((Phase::Exit, i));
                    items.push(ColItem { key, phase: Phase::Exit, a: s, b: gs, from: None });
                }
            }
            col_pending.push(ColPending { items, choreo, hashes, src, res, rcx, bulk: false });
        }
        self.stats.instances = n_instances;

        // Duration: the longest delay + duration of anything that moves.
        let mut dur = 0.0f64;
        {
            let mut consider = |r: &Resolved| dur = dur.max(r.delay + r.duration);
            for p in &self.pending {
                if !p.track.same {
                    consider(&self.table[p.res]);
                }
            }
            for (c, cb) in col_pending.iter().zip(&cols) {
                let same_nodes = match (cb.a, cb.b) {
                    (Some(ka), Some(kb)) => self.a.f.insts[ka].node == self.b.f.insts[kb].node,
                    _ => false,
                };
                if (!c.items.is_empty() || c.bulk) && !same_nodes {
                    consider(&self.table[c.res]);
                }
            }
            for (s, st) in self.structs.iter().enumerate() {
                if !st.same && st.a.is_some() && st.b.is_some() {
                    consider(&self.table[self.struct_res[s]]);
                }
            }
        }
        if dur <= 0.0 {
            // Nothing moves: the root's rule duration (a nominal length for timelines).
            let r = self.table.get(self.struct_res.first().copied().unwrap_or(0));
            dur = r.map_or(DEFAULT_DURATION, |r| r.delay + r.duration);
        }
        let rule_win = |r: &Resolved| -> (f64, f64) {
            if dur > 0.0 {
                ((r.delay / dur).clamp(0.0, 1.0), ((r.delay + r.duration) / dur).clamp(0.0, 1.0))
            } else {
                (0.0, 0.0)
            }
        };

        // Choreography over node tracks and column instances together (a rule's stagger spans
        // shapes and instances alike).
        let mut items: Vec<Item> = Vec::with_capacity(self.pending.len());
        for p in &self.pending {
            let r = &self.table[p.res];
            let (ws, we) = rule_win(r);
            items.push(Item { group: r.choreo_src, phase: p.track.phase, center: p.center, order: p.order, value: p.value, hash: p.track.hash, ws, we });
        }
        let mut col_ranges = Vec::with_capacity(col_pending.len());
        for c in &mut col_pending {
            let (ws, we) = rule_win(&self.table[c.res]);
            let start = items.len();
            for mut it in std::mem::take(&mut c.choreo) {
                it.ws = ws;
                it.we = we;
                items.push(it);
            }
            col_ranges.push(start..items.len());
        }
        let mut choreos: BTreeMap<usize, Choreography> = BTreeMap::new();
        for r in &self.table {
            if r.choreo_src != usize::MAX {
                choreos.entry(r.choreo_src).or_insert_with(|| r.choreo.clone());
            }
        }
        let choreo_of = |g: usize| choreos.get(&g).cloned().unwrap_or_default();
        let mut wins = windows(&items, &choreo_of, event, self.root_mid);
        // Words never overlap mid-transition: a text leaving and another arriving in the same place
        // (a title that changes with the scene) take turns — out in the first half of its window,
        // in during the second half of its own.
        let text_at = |p: &PendingNode, n: &Node| match &n.kind {
            NodeKind::Text(t) => Some((p.center, t.bounds.w, t.bounds.h)),
            _ => None,
        };
        let leaving: Vec<(usize, (Vec2, f64, f64))> = self.pending.iter().enumerate().filter(|(_, p)| p.track.phase == Phase::Exit).filter_map(|(k, p)| text_at(p, &p.track.a).map(|b| (k, b))).collect();
        let arriving: Vec<(usize, (Vec2, f64, f64))> = self.pending.iter().enumerate().filter(|(_, p)| p.track.phase == Phase::Enter).filter_map(|(k, p)| text_at(p, &p.track.b).map(|b| (k, b))).collect();
        let overlap = |(ca, wa, ha): (Vec2, f64, f64), (cb, wb, hb): (Vec2, f64, f64)| (ca.x - cb.x).abs() < (wa + wb) / 2.0 && (ca.y - cb.y).abs() < (ha + hb) / 2.0;
        // Over the pair's common span (their own windows may be staggered differently).
        let before = wins.clone();
        for &(ke, be) in &leaving {
            for &(kn, bn) in &arriving {
                if overlap(be, bn) {
                    let (s, e) = (before[ke].0.min(before[kn].0), before[ke].1.max(before[kn].1));
                    let mid = s + (e - s) / 2.0;
                    wins[ke] = (s, mid);
                    wins[kn] = (mid, e);
                }
            }
        }

        // Riders keep to their line's window (and easing, below); revealed ones open or close
        // their windows as its trim passes them.
        let mut rider_style: Vec<Option<(Easing, bool, usize)>> = Vec::with_capacity(cols.len());
        // Revealed riders' position windows (see `ColumnTrack::pos_win`), by item.
        let mut pos_wins: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
        for ((r, c), range) in riders.iter().zip(&col_pending).zip(&col_ranges) {
            let Some(r) = r else {
                rider_style.push(None);
                continue;
            };
            let line = &self.pending[r.track].track;
            let lw = wins[r.track];
            let ease: Vec<f64> = match &r.reveal {
                Some(_) => (0..=256).map(|k| line.easing.apply(k as f64 / 256.0)).collect(),
                None => Vec::new(),
            };
            for (k, &(phase, idx)) in c.src.iter().enumerate() {
                let revealed = r.reveal.as_ref().and_then(|rv| {
                    let (f, v, entering) = match phase {
                        Phase::Enter => (rv.fb.as_ref()?, &r.vb, true),
                        Phase::Exit => (rv.fa.as_ref()?, &r.va, false),
                        Phase::Update => return None,
                    };
                    Some(rv.window(lw, &ease, *f.get(*v.get(idx)?)?, entering))
                });
                wins[range.start + k] = revealed.unwrap_or(lw);
                // Revealed, it fades in (or out) on its own window but moves with the line.
                if revealed.is_some() {
                    pos_wins.insert(range.start + k, lw);
                }
            }
            rider_style.push(Some((line.easing.clone(), line.route.is_straight(), r.track)));
        }

        // Callouts carried along a line take its timing (see `bind_followers`).
        // (Its window, the line's easing, and the line's window.)
        let mut timing: BTreeMap<usize, FollowTiming> = BTreeMap::new();
        for &(k, line, reveal) in &self.followers {
            let (lw, easing) = (wins[line], self.pending[line].track.easing.clone());
            let win = match reveal {
                // Opening (or closing) as the pen passes its point, as a revealed rider does.
                Some((s, entering)) => {
                    let ease: Vec<f64> = (0..=256).map(|j| easing.apply(j as f64 / 256.0)).collect();
                    let r = Reveal { from: if entering { 0.0 } else { 1.0 }, to: if entering { 1.0 } else { 0.0 }, fa: None, fb: None };
                    r.window(lw, &ease, s, entering)
                }
                None => lw,
            };
            timing.insert(k, (win, easing, lw));
        }
        let mut nodes: Vec<NodeTrack> = Vec::with_capacity(self.pending.len());
        let mut homes: Vec<Home> = Vec::with_capacity(self.pending.len());
        for (k, p) in std::mem::take(&mut self.pending).into_iter().enumerate() {
            let mut t = p.track;
            t.win = wins[k];
            if let Some((win, easing, lw)) = timing.remove(&k) {
                t.win = win;
                t.follow_timing = Some((lw, easing.clone()));
                t.easing = easing;
            }
            nodes.push(t);
            homes.push(p.home);
        }
        let mut columns: Vec<ColumnTrack> = Vec::with_capacity(cols.len());
        let mut col_pos: Option<Vec<(f64, f64)>> = None;
        for (((cb, c), range), style) in cols.iter().zip(col_pending).zip(col_ranges).zip(rider_style) {
            let r = &self.table[c.res];
            let na = cb.a.map(|k| self.a.f.insts[k].node.clone());
            let nb = cb.b.map(|k| self.b.f.insts[k].node.clone());
            if c.bulk {
                columns.push(ColumnTrack::bulk(na, nb, rule_win(r), r.easing.clone()));
                continue;
            }
            let (easing, route, line) = match style {
                Some((e, true, t)) => (e, crate::route::Route::Straight, Some(t)),
                Some((e, false, t)) => (e, r.route.clone(), Some(t)),
                None => (r.easing.clone(), r.route.clone(), None),
            };
            if range.clone().any(|k| pos_wins.contains_key(&k)) {
                col_pos = Some(range.clone().map(|k| pos_wins.get(&k).copied().unwrap_or(wins[k])).collect());
            }
            let mut col = ColumnTrack::new(na, nb, c.items, wins[range].to_vec(), rule_win(r), easing, route, c.rcx, c.hashes, &self.cx.theme);
            col.line = line;
            col.pos_win = col_pos.take();
            columns.push(col);
        }

        for s in 0..self.structs.len() {
            let r = &self.table[self.struct_res[s]];
            self.structs[s].win = rule_win(r);
            self.structs[s].easing = r.easing.clone();
        }

        // Children of every merged structure node.
        let mut placed = vec![false; nodes.len()];
        let mut orphans: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (t, h) in homes.iter().enumerate() {
            if let Home::InPlace(m) = h {
                orphans.entry(*m).or_default().push(t);
            }
        }
        for m in 0..self.structs.len() {
            let (sa, sb) = std::mem::take(&mut self.seq[m]);
            let map = |side: &Side, c: &Child| -> Option<Slot> {
                match c {
                    Child::Struct(i) => side.home[*i].map(Slot::Struct),
                    Child::Elem(i) => side.node_of[*i].filter(|&t| homes[t] == Home::InPlace(m)).map(Slot::Node),
                    Child::Inst(k) => side.col_of[*k].map(Slot::Column),
                }
            };
            let seq_a: Vec<Slot> = sa.iter().filter_map(|c| map(&self.a, c)).collect();
            let seq_b: Vec<Slot> = sb.iter().filter_map(|c| map(&self.b, c)).collect();
            let mut slots = merge_seqs(&seq_a, &seq_b);
            for s in &slots {
                if let Slot::Node(t) = s {
                    placed[*t] = true;
                }
            }
            // In-place tracks whose element was not a direct child (continuing ex-flyers).
            for &t in orphans.get(&m).map(|v| v.as_slice()).unwrap_or(&[]) {
                if !placed[t] {
                    placed[t] = true;
                    slots.push(Slot::Node(t));
                }
            }
            self.structs[m].children = slots;
        }

        // The flight layer: groups mirroring destination paths below the root.
        let mut mirror: BTreeMap<Vec<Key>, usize> = BTreeMap::new();
        let mut flight_root: Option<usize> = None;
        for (t, h) in homes.iter().enumerate() {
            let Home::Flight(parent) = h else { continue };
            let fr = match flight_root {
                Some(f) => f,
                None => {
                    self.structs.push(StructTrack::constant(Node::group(flight_key(), Vec::new())));
                    let f = self.structs.len() - 1;
                    flight_root = Some(f);
                    f
                }
            };
            let skip = if self.a.f.virtual_root && self.b.f.virtual_root { 0 } else { 1 };
            let rel: Vec<Key> = parent.0.iter().skip(skip).cloned().collect();
            let mut cur = fr;
            for d in 1..=rel.len() {
                let pre = rel[..d].to_vec();
                cur = match mirror.get(&pre) {
                    Some(&g) => g,
                    None => {
                        self.structs.push(StructTrack::constant(Node::group(rel[d - 1].clone(), Vec::new())));
                        let g = self.structs.len() - 1;
                        self.structs[cur].children.push(Slot::Struct(g));
                        mirror.insert(pre, g);
                        g
                    }
                };
            }
            self.structs[cur].children.push(Slot::Node(t));
        }
        if let Some(fr) = flight_root {
            self.structs[root].children.push(Slot::Struct(fr));
            if self.structs[root].virtual_root {
                // A leaf root plus a flight layer needs a real group to hold both.
                self.structs[root].virtual_root = false;
            }
        }

        Plan { from: from.clone(), to: to.clone(), duration: dur, structs: self.structs, nodes, columns, root, stats: self.stats, corr: self.corr }
    }
}

/// Merge two child sequences: the target's order, with the source's leftovers inserted after
/// their nearest preceding shared item.
fn merge_seqs(a: &[Slot], b: &[Slot]) -> Vec<Slot> {
    let in_b: BTreeSet<Slot> = b.iter().copied().collect();
    let mut anchored: BTreeMap<Option<Slot>, Vec<Slot>> = BTreeMap::new();
    let mut last: Option<Slot> = None;
    for s in a {
        if in_b.contains(s) {
            last = Some(*s);
        } else {
            anchored.entry(last).or_default().push(*s);
        }
    }
    let mut out = Vec::with_capacity(a.len() + b.len());
    let mut seen: BTreeSet<Slot> = BTreeSet::new();
    let mut push = |s: Slot, out: &mut Vec<Slot>| {
        if seen.insert(s) {
            out.push(s);
        }
    };
    if let Some(v) = anchored.get(&None) {
        for s in v {
            push(*s, &mut out);
        }
    }
    for s in b {
        push(*s, &mut out);
        if let Some(v) = anchored.get(&Some(*s)) {
            for x in v {
                push(*x, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_target_order_and_anchors_leftovers() {
        let (a, b, c, d) = (Slot::Node(0), Slot::Node(1), Slot::Node(2), Slot::Node(3));
        assert_eq!(merge_seqs(&[a, b, c], &[c, a, d]), vec![c, a, b, d]);
        assert_eq!(merge_seqs(&[b, a], &[a]), vec![b, a]);
        assert_eq!(merge_seqs(&[a, b], &[a, b]), vec![a, b]);
    }
}
