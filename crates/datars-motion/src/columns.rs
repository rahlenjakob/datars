//! Instances interpolate column-wise: one output `Instances` node per matched node path, its
//! x / y / size / w / h / opacity / fill columns lerped per instance with per-instance windows —
//! plain slice arithmetic, the CPU reference of the GPU path (docs/05 §GPU interpolation).
//! Entering and exiting instances ride along in the same node, interpolating from / to ghost
//! states. When the two nodes' prototypes differ, two nodes crossfade (positions still move).

use crate::choreo::{progress, Phase};
use crate::easing::Easing;
use crate::ghost::Ghost;
use crate::interp::{lerp_common, oklab_to_color_fast, to_lab};
use crate::route::{Route, RouteCx};
use datars_color::Oklab;
use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Instances, Key, Node, NodeKind, Proto};
use datars_theme::{Ink, ResolvedTheme};

/// Above this many instances on either side, a column crossfades its two nodes whole instead of
/// planning each instance: per-instance tracks cost a few hundred bytes each (states, windows,
/// keys), and a dot map of 400,000 dots would spend a gigabyte on a transition nobody can follow
/// dot by dot. Unchanged nodes stay put at any size.
pub const COLUMN_MAX: usize = 100_000;

/// One instance's state in some prototype's native terms.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InstState {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub w: f64,
    pub h: f64,
    pub opacity: f64,
    pub fill: Ink,
}

impl InstState {
    pub fn of(ins: &Instances, i: usize) -> InstState {
        InstState {
            x: ins.x[i],
            y: ins.y[i],
            size: ins.size_at(i),
            w: ins.w.as_ref().and_then(|v| v.get(i)).copied().unwrap_or(1.0),
            h: ins.h.as_ref().and_then(|v| v.get(i)).copied().unwrap_or(1.0),
            opacity: ins.opacity_at(i),
            fill: ins.fill_at(i),
        }
    }

    pub fn center(&self, proto: &Proto) -> Vec2 {
        match proto {
            Proto::Rect => Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0),
            Proto::Symbol { .. } => Vec2::new(self.x, self.y),
        }
    }

    pub fn bounds(&self, proto: &Proto) -> Rect {
        match proto {
            Proto::Rect => Rect::new(self.x, self.y, self.w, self.h),
            Proto::Symbol { .. } => Rect::new(self.x - self.size, self.y - self.size, 2.0 * self.size, 2.0 * self.size),
        }
    }

    /// The same instance with its centre moved to `c`.
    pub fn centred(mut self, proto: &Proto, c: Vec2) -> InstState {
        if !c.is_finite() {
            return self;
        }
        match proto {
            Proto::Rect => {
                self.x = c.x - self.w / 2.0;
                self.y = c.y - self.h / 2.0;
            }
            Proto::Symbol { .. } => {
                self.x = c.x;
                self.y = c.y;
            }
        }
        self
    }

    /// The same instance expressed in another prototype's terms (centre and extent preserved).
    pub fn convert(&self, from: &Proto, to: &Proto) -> InstState {
        match (from, to) {
            (Proto::Rect, Proto::Symbol { .. }) => {
                let c = self.center(from);
                InstState { x: c.x, y: c.y, size: self.w.abs().min(self.h.abs()) / 2.0, ..self.clone() }
            }
            (Proto::Symbol { .. }, Proto::Rect) => {
                InstState { x: self.x - self.size, y: self.y - self.size, w: 2.0 * self.size, h: 2.0 * self.size, ..self.clone() }
            }
            _ => self.clone(),
        }
    }

    /// Apply a ghost (`target` in node-local coordinates).
    pub fn ghost(&self, g: &Ghost, proto: &Proto, target: Option<Vec2>) -> InstState {
        let mut s = self.clone();
        s.opacity *= g.opacity_factor();
        if let Some((o, sx, sy)) = g.scaling(self.bounds(proto), self.center(proto)) {
            match proto {
                Proto::Symbol { .. } => {
                    s.x = o.x + (s.x - o.x) * sx;
                    s.y = o.y + (s.y - o.y) * sy;
                    s.size *= sx.min(sy).max(0.0);
                }
                Proto::Rect => {
                    s.x = o.x + (s.x - o.x) * sx;
                    s.y = o.y + (s.y - o.y) * sy;
                    s.w *= sx;
                    s.h *= sy;
                }
            }
        }
        let mut d = Vec2::new(g.dx, g.dy);
        if let Some(p) = target {
            d += p - s.center(proto);
        }
        if d.is_finite() {
            s.x += d.x;
            s.y += d.y;
        }
        s
    }
}

/// Interpolation columns in one prototype's terms.
#[derive(Clone, Debug, Default)]
struct Cols {
    x0: Vec<f64>,
    x1: Vec<f64>,
    y0: Vec<f64>,
    y1: Vec<f64>,
    s0: Vec<f64>,
    s1: Vec<f64>,
    w0: Vec<f64>,
    w1: Vec<f64>,
    h0: Vec<f64>,
    h1: Vec<f64>,
    o0: Vec<f64>,
    o1: Vec<f64>,
}

impl Cols {
    fn push(&mut self, a: &InstState, b: &InstState) {
        self.x0.push(a.x);
        self.x1.push(b.x);
        self.y0.push(a.y);
        self.y1.push(b.y);
        self.s0.push(a.size);
        self.s1.push(b.size);
        self.w0.push(a.w);
        self.w1.push(b.w);
        self.h0.push(a.h);
        self.h1.push(b.h);
        self.o0.push(a.opacity);
        self.o1.push(b.opacity);
    }
}

/// `a` at 0 and `b` at 1 exactly.
#[inline]
fn mix(a: f64, b: f64, e: f64) -> f64 {
    if e == 1.0 {
        b
    } else {
        a + (b - a) * e
    }
}

#[inline]
fn mix_size(a: f64, b: f64, e: f64) -> f64 {
    let v = mix(a, b, e);
    if v < 0.0 && a >= 0.0 && b >= 0.0 {
        0.0
    } else {
        v
    }
}

#[derive(Clone, Debug)]
enum Windows {
    Shared((f64, f64)),
    Per(Vec<(f64, f64)>),
}

/// One instance to interpolate: its from / to states and phase.
pub(crate) struct ColItem {
    pub key: Key,
    pub phase: Phase,
    pub a: InstState,
    pub b: InstState,
    /// The prototype `a` is in when it isn't the from-node's (a piece of a bar that splits into
    /// dots: a rect, becoming a symbol).
    pub from: Option<Proto>,
}

/// `b` is `a` moved: every instance the same, at a position shifted by one vector (not zero).
fn instances_shift(a: &Instances, b: &Instances) -> Option<Vec2> {
    let n = a.x.len();
    if n == 0 || n != b.x.len() || a.y.len() != n || b.y.len() != n {
        return None;
    }
    if a.proto != b.proto || a.size != b.size || a.w != b.w || a.h != b.h || a.fill != b.fill || a.opacity != b.opacity || a.stroke != b.stroke || a.screen_size != b.screen_size || a.labels != b.labels || a.keys != b.keys {
        return None;
    }
    let d = Vec2::new(b.x[0] - a.x[0], b.y[0] - a.y[0]);
    if d == Vec2::ZERO || !d.x.is_finite() || !d.y.is_finite() {
        return None;
    }
    let tol = |v: f64| 1e-9 * (1.0 + v.abs());
    let moved = (0..n).all(|i| (b.x[i] - (a.x[i] + d.x)).abs() <= tol(b.x[i]) && (b.y[i] - (a.y[i] + d.y)).abs() <= tol(b.y[i]));
    moved.then_some(d)
}

#[derive(Clone, Debug)]
pub(crate) struct ColumnTrack {
    key: Key,
    node_a: Option<Node>,
    node_b: Option<Node>,
    node_win: (f64, f64),
    easing: Easing,
    mono: bool,
    proto: Proto,
    /// The from-node prototype when it differs from `proto` (crossfade mode).
    dual: Option<Proto>,
    keys: Vec<Key>,
    phase: Vec<Phase>,
    /// Per row: shown in both prototypes, crossfading (in a column with a second prototype).
    crossfade: Vec<bool>,
    cols: Cols,
    dual_cols: Option<Cols>,
    fill_a: Vec<Ink>,
    fill_b: Vec<Ink>,
    lab_a: Vec<[f64; 4]>,
    lab_b: Vec<[f64; 4]>,
    fill_same: Vec<bool>,
    win: Windows,
    route: Route,
    rcx: RouteCx,
    hashes: Vec<u64>,
    /// The whole node is unchanged.
    same: bool,
    /// Too many instances to plan one by one (`COLUMN_MAX`): the nodes crossfade whole.
    bulk: bool,
    /// A bulk node whose target is itself moved by this vector (a dot map re-fitted to a slightly
    /// different box): drawn once, under a moving transform, instead of two copies crossfading.
    shift: Option<Vec2>,
    /// The node track of the line these instances ride (they share its easing, even when a
    /// retarget gives it a velocity).
    pub(crate) line: Option<usize>,
    /// Per row, the window its position follows when that differs from the one its opacity does:
    /// a rider revealed by its line's pen fades in on its own window but moves with the line.
    pub(crate) pos_win: Option<Vec<(f64, f64)>>,
}

fn inst_of(n: &Node) -> &Instances {
    match &n.kind {
        NodeKind::Instances(i) => i,
        _ => unreachable!("column nodes are Instances"),
    }
}

impl ColumnTrack {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_a: Option<Node>,
        node_b: Option<Node>,
        items: Vec<ColItem>,
        windows: Vec<(f64, f64)>,
        node_win: (f64, f64),
        easing: Easing,
        route: Route,
        rcx: RouteCx,
        hashes: Vec<u64>,
        theme: &ResolvedTheme,
    ) -> ColumnTrack {
        let key = node_b.as_ref().or(node_a.as_ref()).map(|n| n.key.clone()).unwrap_or_default();
        let proto = node_b.as_ref().or(node_a.as_ref()).map(|n| inst_of(n).proto.clone()).unwrap_or(Proto::Rect);
        let pa = node_a.as_ref().map(|n| inst_of(n).proto.clone());
        let node_dual = pa.filter(|p| *p != proto);
        // Rows can come from another prototype than their node (split pieces): they crossfade too.
        let dual = node_dual.clone().or_else(|| items.iter().find_map(|it| it.from.clone().filter(|p| *p != proto)));
        let same = node_a.is_some() && node_a == node_b;
        let n = items.len();
        let mut cols = Cols::default();
        let mut dual_cols = dual.as_ref().map(|_| Cols::default());
        let (mut keys, mut phase) = (Vec::with_capacity(n), Vec::with_capacity(n));
        let (mut fill_a, mut fill_b, mut lab_a, mut lab_b, mut fill_same) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        let mut crossfade = Vec::with_capacity(n);
        for it in &items {
            // Item states: updates go from-prototype → to-prototype; enters live in the
            // to-prototype's terms, exits in the from-prototype's.
            let from_proto = it.from.as_ref().or(node_dual.as_ref()).unwrap_or(&proto);
            crossfade.push(*from_proto != proto);
            let (a_main, b_main) = match it.phase {
                Phase::Update => (it.a.convert(from_proto, &proto), it.b.clone()),
                Phase::Exit => (it.a.convert(from_proto, &proto), it.b.convert(from_proto, &proto)),
                Phase::Enter => (it.a.clone(), it.b.clone()),
            };
            cols.push(&a_main, &b_main);
            if let (Some(dc), Some(fp)) = (dual_cols.as_mut(), dual.as_ref()) {
                let (a_d, b_d) = match it.phase {
                    Phase::Update => (it.a.convert(from_proto, fp), it.b.convert(&proto, fp)),
                    Phase::Exit => (it.a.convert(from_proto, fp), it.b.convert(from_proto, fp)),
                    Phase::Enter => (it.a.convert(&proto, fp), it.b.convert(&proto, fp)),
                };
                dc.push(&a_d, &b_d);
            }
            keys.push(it.key.clone());
            phase.push(it.phase);
            let same_fill = it.a.fill == it.b.fill;
            fill_same.push(same_fill);
            if same_fill {
                lab_a.push([0.0; 4]);
                lab_b.push([0.0; 4]);
            } else {
                lab_a.push(to_lab(it.a.fill.resolve(theme)));
                lab_b.push(to_lab(it.b.fill.resolve(theme)));
            }
            fill_a.push(it.a.fill.clone());
            fill_b.push(it.b.fill.clone());
        }
        let win = match windows.first() {
            Some(w0) if windows.iter().all(|w| w == w0) => Windows::Shared(*w0),
            Some(_) => Windows::Per(windows),
            None => Windows::Shared(node_win),
        };
        ColumnTrack {
            key,
            node_a,
            node_b,
            node_win,
            mono: easing.is_monotone(),
            easing,
            proto,
            dual,
            keys,
            phase,
            crossfade,
            cols,
            dual_cols,
            fill_a,
            fill_b,
            lab_a,
            lab_b,
            fill_same,
            win,
            route,
            rcx,
            hashes,
            same,
            bulk: false,
            shift: None,
            line: None,
            pos_win: None,
        }
    }

    /// A column too large to plan per instance (`COLUMN_MAX`): the from-node fades out as the
    /// to-node fades in over the node's window — or stays put when the two are the same.
    pub fn bulk(node_a: Option<Node>, node_b: Option<Node>, node_win: (f64, f64), easing: Easing) -> ColumnTrack {
        let key = node_b.as_ref().or(node_a.as_ref()).map(|n| n.key.clone()).unwrap_or_default();
        let proto = node_b.as_ref().or(node_a.as_ref()).map(|n| inst_of(n).proto.clone()).unwrap_or(Proto::Rect);
        let same = node_a.is_some() && node_a == node_b;
        let shift = match (&node_a, &node_b) {
            (Some(a), Some(b)) if !same => instances_shift(inst_of(a), inst_of(b)),
            _ => None,
        };
        ColumnTrack {
            key,
            node_a,
            node_b,
            node_win,
            mono: easing.is_monotone(),
            easing,
            proto,
            dual: None,
            keys: Vec::new(),
            phase: Vec::new(),
            crossfade: Vec::new(),
            cols: Cols::default(),
            dual_cols: None,
            fill_a: Vec::new(),
            fill_b: Vec::new(),
            lab_a: Vec::new(),
            lab_b: Vec::new(),
            fill_same: Vec::new(),
            win: Windows::Shared(node_win),
            route: Route::Straight,
            rcx: RouteCx { mid: Vec2::ZERO, span: 1.0 },
            hashes: Vec::new(),
            same,
            bulk: true,
            shift,
            line: None,
            pos_win: None,
        }
    }

    pub(crate) fn set_easing(&mut self, e: Easing) {
        self.mono = e.is_monotone();
        self.easing = e;
    }

    /// Emit the node(s) for plan time `t`.
    pub fn frame(&self, t: f64, out: &mut Vec<Node>) {
        if self.same {
            if let Some(n) = &self.node_b {
                out.push(n.clone());
            }
            return;
        }
        if self.bulk {
            // Exactly the from-node at 0 and the to-node at 1; a crossfade between — or, when the
            // target is the same set moved, the one set moving (what every instance's own
            // interpolation would draw, at a fraction of the cost).
            let u = progress(t, self.node_win);
            if let (Some(d), Some(a), Some(b)) = (self.shift, &self.node_a, &self.node_b) {
                if u >= 1.0 {
                    out.push(b.clone());
                    return;
                }
                let ev = self.easing.apply(u);
                let mut n = a.clone();
                n.common = lerp_common(&a.common, &b.common, ev, ev.clamp(0.0, 1.0), u);
                n.common.transform = Affine::translate(d.x * ev, d.y * ev).then(n.common.transform);
                out.push(n);
                return;
            }
            let e = self.easing.apply(u).clamp(0.0, 1.0);
            let faded = |n: &Node, f: f64| {
                let mut n = n.clone();
                n.common.opacity *= f;
                n
            };
            match (&self.node_a, &self.node_b) {
                (Some(a), _) if e <= 0.0 => out.push(a.clone()),
                (_, Some(b)) if e >= 1.0 => out.push(b.clone()),
                (Some(a), Some(b)) => {
                    out.push(faded(a, 1.0 - e));
                    out.push(faded(b, e));
                }
                (Some(a), None) => out.push(faded(a, 1.0 - e)),
                (None, Some(b)) => out.push(faded(b, e)),
                (None, None) => {}
            }
            return;
        }
        let n = self.keys.len();
        let (mut e, mut op, mut uu) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        let op_of = |u: f64, ev: f64, ph: Phase| -> f64 {
            if ph == Phase::Update || self.mono {
                ev.clamp(0.0, 1.0)
            } else {
                u
            }
        };
        match &self.win {
            Windows::Shared(w) => {
                let u = progress(t, *w);
                let ev = self.easing.apply(u);
                for &ph in &self.phase {
                    e.push(ev);
                    op.push(op_of(u, ev, ph));
                    uu.push(u);
                }
            }
            Windows::Per(ws) => {
                for (w, &ph) in ws.iter().zip(&self.phase) {
                    let u = progress(t, *w);
                    let ev = self.easing.apply(u);
                    e.push(ev);
                    op.push(op_of(u, ev, ph));
                    uu.push(u);
                }
            }
        }
        if let Some(pw) = &self.pos_win {
            for (k, w) in pw.iter().enumerate().take(n) {
                e[k] = self.easing.apply(progress(t, *w));
            }
        }
        let u_node = progress(t, self.node_win);
        let e_node = self.easing.apply(u_node);
        let base = match (&self.node_a, &self.node_b) {
            (_, Some(b)) => b,
            (Some(a), None) => a,
            (None, None) => return,
        };
        let common = match (&self.node_a, &self.node_b) {
            (Some(a), Some(b)) => lerp_common(&a.common, &b.common, e_node, e_node.clamp(0.0, 1.0), u_node),
            _ => base.common.clone(),
        };
        let late = u_node >= 0.5;
        let meta = if late { self.node_b.as_ref().unwrap_or(base) } else { self.node_a.as_ref().unwrap_or(base) };
        let meta_ins = inst_of(meta);
        let fill: Vec<Ink> = (0..n)
            .map(|k| {
                if self.fill_same[k] || uu[k] <= 0.0 {
                    self.fill_a[k].clone()
                } else if uu[k] >= 1.0 {
                    self.fill_b[k].clone()
                } else {
                    let t = e[k].clamp(0.0, 1.0);
                    // Premultiplied, as solid fills mix (Oklab::mix).
                    let lab = |c: &[f64; 4]| Oklab { l: c[0], a: c[1], b: c[2], alpha: c[3] };
                    let m = lab(&self.lab_a[k]).mix(lab(&self.lab_b[k]), t);
                    Ink::Color(oklab_to_color_fast(m.l, m.a, m.b, m.alpha))
                }
            })
            .collect();
        let offsets: Option<Vec<Vec2>> = (!self.route.is_straight()).then(|| {
            (0..n)
                .map(|k| {
                    let c = &self.cols;
                    let (ca, cb) = match self.proto {
                        Proto::Rect => (Vec2::new(c.x0[k] + c.w0[k] / 2.0, c.y0[k] + c.h0[k] / 2.0), Vec2::new(c.x1[k] + c.w1[k] / 2.0, c.y1[k] + c.h1[k] / 2.0)),
                        Proto::Symbol { .. } => (Vec2::new(c.x0[k], c.y0[k]), Vec2::new(c.x1[k], c.y1[k])),
                    };
                    self.route.offset(ca, cb, e[k].clamp(0.0, 1.0), self.hashes.get(k).copied().unwrap_or(0), &self.rcx)
                })
                .collect()
        });
        let build = |proto: &Proto, c: &Cols, factor: &dyn Fn(usize) -> f64| -> Instances {
            let mut x = Vec::with_capacity(n);
            let mut y = Vec::with_capacity(n);
            let mut o = Vec::with_capacity(n);
            for k in 0..n {
                let (ek, ok) = (e[k], op[k]);
                let (mut xv, mut yv) = (mix(c.x0[k], c.x1[k], ek), mix(c.y0[k], c.y1[k], ek));
                if let Some(off) = &offsets {
                    xv += off[k].x;
                    yv += off[k].y;
                }
                x.push(xv);
                y.push(yv);
                o.push((mix(c.o0[k], c.o1[k], ok) * factor(k)).clamp(0.0, 1.0));
            }
            let (size, w, h) = match proto {
                Proto::Rect => (
                    Vec::new(),
                    Some((0..n).map(|k| mix_size(c.w0[k], c.w1[k], e[k])).collect()),
                    Some((0..n).map(|k| mix_size(c.h0[k], c.h1[k], e[k])).collect()),
                ),
                Proto::Symbol { .. } => ((0..n).map(|k| mix_size(c.s0[k], c.s1[k], e[k])).collect(), None, None),
            };
            Instances {
                proto: proto.clone(),
                keys: self.keys.clone(),
                x,
                y,
                size,
                w,
                h,
                fill: fill.clone(),
                opacity: o,
                stroke: meta_ins.stroke.clone(),
                screen_size: meta_ins.screen_size,
                labels: None,
                line_reach: meta_ins.line_reach,
            }
        };
        let mk = |ins: Instances| Node {
            key: self.key.clone(),
            kind: NodeKind::Instances(std::sync::Arc::new(ins)),
            common: common.clone(),
            semantics: meta.semantics.clone(),
            pickable: meta.pickable,
            anchors: meta.anchors.clone(),
            prov: meta.prov,
        };
        match (&self.dual, &self.dual_cols) {
            (Some(fp), Some(dc)) => {
                // Crossfade between prototypes: the from-shape fades as the to-shape appears.
                let m = |k: usize| match self.phase[k] {
                    _ if !self.crossfade[k] => 1.0,
                    Phase::Update => op[k],
                    Phase::Enter => 1.0,
                    Phase::Exit => 0.0,
                };
                out.push(mk(build(fp, dc, &|k| 1.0 - m(k))));
                out.push(mk(build(&self.proto, &self.cols, &m)));
            }
            _ => out.push(mk(build(&self.proto, &self.cols, &|_| 1.0))),
        }
    }
}
