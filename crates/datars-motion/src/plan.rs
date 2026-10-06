//! Transition plans: `plan(from, to, rules, cx)` does the expensive work once — flattening,
//! matching, choreography windows, outline resampling, ink resolution — and `Plan::at(t)` is a
//! cheap, pure function of normalized time returning a `Scene`.
//!
//! **Output structure.** A frame is the union of both scenes' structure: groups and views are
//! merged by key (children ordered by the target's order, the source's leftovers after their
//! nearest shared predecessor), their transforms, opacity, viewports and cameras interpolated.
//! Elements whose two states live in the same container interpolate *in place*, in local
//! coordinates, so they stay glued to an animating parent (a panning camera, a moving plot).
//! Elements that change container (a by-key morph from a map into a bar chart, the pieces of a
//! hierarchy split, an instance turning into a shape) fly in the **flight layer**: the last child
//! of the root, keyed `~flight`, nested in groups mirroring each element's destination path, with
//! states in root-content coordinates. Instances of the same node interpolate column-wise.
//!
//! **Exactness.** `at(t ≤ 0)` returns `from` and `at(t ≥ 1)` returns `to` (clones, so `==`
//! holds bit for bit). In between, every element whose window has not opened renders exactly its
//! source state, and every element whose window has closed exactly its target state.

use crate::choreo::{progress, Phase};
use crate::columns::ColumnTrack;
use crate::easing::Easing;
use crate::elements::content_bounds;
use crate::lines::LinePlan;
use crate::interp::{lerp_common, lerp_geom, lerp_rect, lerp_size, lerp_v, paint_at, plan_paint, plan_stroke, same_path_structure, stroke_at, translation_between, PaintPlan, StrokePlan};
use crate::matching::Correspondence;
use crate::outline::{lod_points, perimeter, rings, Morph};
use crate::route::{Route, RouteCx};
use crate::rules::{MorphStrategy, MotionRules, Resolved};
use crate::camera::interpolate_camera;
use crate::{PlanCx, TextShaper};
use datars_color::Color;
use datars_math::{lerp, Affine, Hash64, PathData, Vec2};
use datars_scene::{Camera, Clip, Common, Curve, Geom, Key, KeyPart, KeyPath, Node, NodeKind, Scene, Stroke, TextNode};
use datars_theme::{Ink, ResolvedTheme};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Counts for inspection and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanStats {
    pub pairs: usize,
    pub enters: usize,
    pub exits: usize,
    pub splits: usize,
    pub merges: usize,
    /// Node tracks in the flight layer.
    pub flyers: usize,
    /// Node tracks morphing outlines.
    pub morphs: usize,
    pub crossfades: usize,
    /// Instances interpolated column-wise.
    pub instances: usize,
    /// Instances nodes riding a line (its point markers, moving with it).
    pub riders: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Slot {
    Struct(usize),
    Node(usize),
    Column(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct StructTrack {
    pub a: Option<Node>,
    pub b: Option<Node>,
    pub win: (f64, f64),
    pub easing: Easing,
    pub children: Vec<Slot>,
    pub same: bool,
    pub virtual_root: bool,
}

impl StructTrack {
    pub fn constant(n: Node) -> StructTrack {
        StructTrack { a: Some(n.clone()), b: Some(n), win: (0.0, 0.0), easing: Easing::Linear, children: Vec::new(), same: true, virtual_root: false }
    }

    /// The node (without children) at plan time `t`.
    fn state(&self, t: f64) -> Node {
        match (&self.a, &self.b) {
            (Some(a), Some(b)) if !self.same => {
                let u = progress(t, self.win);
                if u <= 0.0 {
                    return a.clone();
                }
                if u >= 1.0 {
                    return b.clone();
                }
                let e = self.easing.apply(u);
                let late = u >= 0.5;
                let common = lerp_common(&a.common, &b.common, e, e.clamp(0.0, 1.0), u);
                let kind = match (&a.kind, &b.kind) {
                    (NodeKind::View { viewport: va, camera: ca, clip: cla, .. }, NodeKind::View { viewport: vb, camera: cb, clip: clb, .. }) => {
                        let vp = lerp_rect(va, vb, e);
                        let camera = match (ca, cb) {
                            (None, None) => None,
                            _ => {
                                let ca = ca.unwrap_or(Camera::identity_for(*va));
                                let cb = cb.unwrap_or(Camera::identity_for(*vb));
                                // One viewport width for the whole move keeps the path fixed
                                // even when the viewport itself animates.
                                let vw = (va.w + vb.w) / 2.0;
                                Some(if ca == cb { ca } else { interpolate_camera(&ca, &cb, e.clamp(0.0, 1.0), vw) })
                            }
                        };
                        NodeKind::View { viewport: vp, camera, clip: if late { *clb } else { *cla }, children: Vec::new() }
                    }
                    _ => {
                        if late {
                            b.kind.clone()
                        } else {
                            a.kind.clone()
                        }
                    }
                };
                let meta = if late { b } else { a };
                Node { key: b.key.clone(), kind, common, semantics: meta.semantics.clone(), pickable: meta.pickable, anchors: meta.anchors.clone(), prov: meta.prov }
            }
            (Some(a), _) => a.clone(),
            (None, Some(b)) => b.clone(),
            (None, None) => Node::group(Key::default(), Vec::new()),
        }
    }
}

/// How a shape's geometry interpolates.
#[derive(Clone, Debug)]
pub(crate) enum GeomPlan {
    NotShape,
    Same,
    Param,
    /// Open lines / areas merged to a common point count, every vertex of both kept
    /// ([`crate::lines`]). `curve`: the curve of every frame (`None`: the source's until halfway,
    /// then the target's).
    Points { a: Vec<Vec<Vec2>>, b: Vec<Vec<Vec2>>, area: bool, curve: Option<Curve> },
    /// One open line runs on from the other (its points start with the other's: a track drawn
    /// further up the clock, a series with new data). The longer line is drawn, trimmed from the
    /// shorter one's length to its own — growing *along* its path instead of bending into it.
    /// `from` / `to` are fractions of the longer line's length; `long_b`: the longer is `b`.
    Extend { from: f64, to: f64, long_b: bool },
    /// One open line (or area) runs on at one end — or is cut back — while the part both share
    /// moves (a series with new data under a new scale). The shared vertices interpolate as
    /// `Points` do; the new part is drawn along its final path from the junction, like a pen (a
    /// part cut off retracts along its old path), instead of bulging out of the old line's end.
    /// The extra part keeps its shape and moves with the junction, so where the line ends at an
    /// edge (a plot's right edge, as a series does) the pen stays on that edge while the shared
    /// part rescales: new data scrolls in there, a part cut off slides out through it.
    /// `a` / `b` as for `Points`; `junction`: the merged index where the shared part meets the
    /// other; `at_end`: the extra part runs after it (else before); `growing`: it's `b`'s.
    Grow { a: Vec<Vec<Vec2>>, b: Vec<Vec<Vec2>>, area: bool, curve: Option<Curve>, junction: usize, at_end: bool, growing: bool },
    Morph(Box<MorphGeom>),
    /// `b` is `a` moved by this vector ([`crate::interp::translation_between`]): drawn as `a`
    /// under a transform that moves with the easing — the frames the point-by-point interpolation
    /// would make, with `a`'s geometry every frame, so renderers keep its meshes.
    Shift(Vec2),
}

#[derive(Clone, Debug)]
pub(crate) struct MorphGeom {
    morph: Morph,
    /// Both states' transforms were baked into the outlines (their linear parts differ).
    baked: bool,
    stroke_a: Option<Stroke>,
    stroke_b: Option<Stroke>,
}

#[derive(Clone, Debug)]
enum TextPlan {
    NotText,
    /// Same text and shaping: origin, rotation, halo and inks interpolate.
    Static { ink: Option<(Color, Color)>, runs: Vec<Option<(Color, Color)>>, halo: Option<(Color, Color)> },
    /// Numbers count or sizes change: value / size interpolate and the shaper re-shapes.
    Shape { ink: Option<(Color, Color)>, halo: Option<(Color, Color)> },
}

#[derive(Clone, Debug)]
pub(crate) struct NodeTrack {
    pub a: Node,
    pub b: Node,
    pub phase: Phase,
    pub win: (f64, f64),
    pub(crate) easing: Easing,
    mono: bool,
    pub geom: GeomPlan,
    text: TextPlan,
    fill: PaintPlan,
    stroke: StrokePlan,
    pub(crate) route: Route,
    pub ra: Vec2,
    pub rb: Vec2,
    pub hash: u64,
    rcx: RouteCx,
    pub same: bool,
    pub crossfade: bool,
    /// Placement units → screen px (approximately), for flattening tolerances.
    pub(crate) px: f64,
    /// Carried along a line (a callout riding its data point; see [`Follow`]), on the line's
    /// window and easing — which differ from the track's own when the line's pen reveals it.
    pub(crate) follow: Option<Arc<Follow>>,
    pub(crate) follow_timing: Option<((f64, f64), Easing)>,
}

/// How a callout on a line moves with it: its marker sits on the line's vertex `from` (merged
/// index; the from-state) and ends on vertex `to` (the to-state), and every frame the callout is
/// moved from where it would be (`lin` interpolated) to that place on the line as drawn — along
/// the line from one to the other when they differ. The line's merged points `a` → `b` are in root
/// content; `grow`: the line's [`GeomPlan::Grow`] (its extra part drawn where it ends up).
#[derive(Clone, Debug)]
pub(crate) struct Follow {
    pub a: Vec<Vec2>,
    pub b: Vec<Vec2>,
    pub from: usize,
    pub to: usize,
    pub lin: (Vec2, Vec2),
    pub grow: Option<(usize, bool, bool)>,
    /// Root content → the node's parent content (for the displacement's direction and length).
    pub to_parent: Affine,
}

impl Follow {
    /// Merged point `k` of the line at eased progress `e`.
    fn point(&self, k: usize, e: f64) -> Vec2 {
        if let Some((junction, at_end, growing)) = self.grow {
            let extra = if at_end { k > junction } else { k < junction };
            if extra {
                // Moved with the junction, as the line draws it (`grow_rings`).
                let src = if growing { &self.b } else { &self.a };
                return src[k] + (self.a[junction].lerp(self.b[junction], e) - src[junction]);
            }
        }
        self.a[k].lerp(self.b[k], e)
    }

    /// Where the marker is on the line at `e`: `e` of the way along it from `from` to `to`.
    fn on_line(&self, e: f64) -> Vec2 {
        if self.from == self.to {
            return self.point(self.from, e);
        }
        let (lo, hi) = (self.from.min(self.to), self.from.max(self.to));
        let pts: Vec<Vec2> = (lo..=hi).map(|k| self.point(k, e)).collect();
        let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
        let mut want = e.clamp(0.0, 1.0) * total;
        let seq: Box<dyn Iterator<Item = (Vec2, Vec2)>> = if self.from < self.to { Box::new(pts.windows(2).map(|w| (w[0], w[1]))) } else { Box::new(pts.windows(2).rev().map(|w| (w[1], w[0]))) };
        let mut last = if self.from < self.to { pts[0] } else { pts[pts.len() - 1] };
        for (p, q) in seq {
            let d = p.dist(q);
            if want <= d {
                return if d > 0.0 { p.lerp(q, want / d) } else { p };
            }
            want -= d;
            last = q;
        }
        last
    }

    /// The displacement (in the node's parent content) from a node placed for its marker at `base`
    /// (root content) to where the marker's place on the line at `e` puts it.
    pub(crate) fn delta_from(&self, e: f64, base: Vec2) -> Vec2 {
        let d = self.on_line(e) - base;
        self.to_parent.apply(d) - self.to_parent.apply(Vec2::ZERO)
    }
}

fn ink_pair(a: &Ink, b: &Ink, theme: &ResolvedTheme) -> Option<(Color, Color)> {
    (a != b).then(|| (a.resolve(theme), b.resolve(theme)))
}

fn ink_at(a: &Ink, b: &Ink, pair: &Option<(Color, Color)>, e: f64) -> Ink {
    match pair {
        None => a.clone(),
        Some((ca, cb)) => {
            if e <= 0.0 {
                a.clone()
            } else if e >= 1.0 {
                b.clone()
            } else {
                Ink::Color(ca.lerp_oklab(*cb, e))
            }
        }
    }
}

fn runs_same_shaping(a: &TextNode, b: &TextNode) -> bool {
    a.runs.len() == b.runs.len() && a.runs.iter().zip(&b.runs).all(|(x, y)| x.font == y.font && x.size == y.size && x.glyphs == y.glyphs)
}

fn plan_text(a: &TextNode, b: &TextNode, theme: &ResolvedTheme) -> Option<TextPlan> {
    let (sa, sb) = (&a.style, &b.style);
    let same_face = sa.family == sb.family && sa.weight == sb.weight;
    let ink = ink_pair(&sa.ink, &sb.ink, theme);
    let halo = match (&a.halo, &b.halo) {
        (Some((ia, _)), Some((ib, _))) => ink_pair(ia, ib, theme),
        _ => None,
    };
    if a.number.is_some() && b.number.is_some() && same_face {
        return Some(TextPlan::Shape { ink, halo });
    }
    let same_layout = a.text == b.text && same_face && sa.align == sb.align && sa.baseline == sb.baseline && sa.max_width == sb.max_width && sa.line_height == sb.line_height;
    if !same_layout {
        return None;
    }
    if sa.size == sb.size && runs_same_shaping(a, b) {
        let runs = a.runs.iter().zip(&b.runs).map(|(x, y)| ink_pair(&x.ink, &y.ink, theme)).collect();
        Some(TextPlan::Static { ink, runs, halo })
    } else {
        Some(TextPlan::Shape { ink, halo })
    }
}

fn text_at(plan: &TextPlan, a: &TextNode, b: &TextNode, e: f64, u: f64, shaper: &dyn TextShaper) -> TextNode {
    let late = u >= 0.5;
    let base = if late { b } else { a };
    let halo_at = |hp: &Option<(Color, Color)>| match (&a.halo, &b.halo) {
        (Some((ia, wa)), Some((ib, wb))) => Some((ink_at(ia, ib, hp, e.clamp(0.0, 1.0)), lerp_size(*wa, *wb, e))),
        _ => base.halo.clone(),
    };
    match plan {
        TextPlan::Static { ink, runs, halo } => {
            let mut t = base.clone();
            t.origin = lerp_v(a.origin, b.origin, e);
            t.offset = lerp_v(a.offset, b.offset, e);
            t.rotate = lerp(a.rotate, b.rotate, e);
            t.style.ink = ink_at(&a.style.ink, &b.style.ink, ink, e.clamp(0.0, 1.0));
            for (i, r) in t.runs.iter_mut().enumerate() {
                if let (Some(pair), Some(ra), Some(rb)) = (runs.get(i), a.runs.get(i), b.runs.get(i)) {
                    r.ink = ink_at(&ra.ink, &rb.ink, pair, e.clamp(0.0, 1.0));
                }
            }
            t.halo = halo_at(halo);
            t
        }
        TextPlan::Shape { ink, halo } => {
            let mut t = base.clone();
            t.origin = lerp_v(a.origin, b.origin, e);
            t.offset = lerp_v(a.offset, b.offset, e);
            t.rotate = lerp(a.rotate, b.rotate, e);
            t.style.size = lerp_size(a.style.size, b.style.size, e);
            t.style.ink = ink_at(&a.style.ink, &b.style.ink, ink, e.clamp(0.0, 1.0));
            if let (Some(na), Some(nb), Some(nt)) = (&a.number, &b.number, t.number.as_mut()) {
                nt.value = lerp(na.value, nb.value, e);
            }
            t.halo = halo_at(halo);
            shaper.shape(&mut t);
            t
        }
        TextPlan::NotText => base.clone(),
    }
}

fn line_pts(g: &Geom) -> Option<Vec<Vec2>> {
    match g {
        Geom::Segment { x1, y1, x2, y2 } => Some(vec![Vec2::new(*x1, *y1), Vec2::new(*x2, *y2)]),
        Geom::Polyline { pts, closed: false, .. } => Some(pts.to_vec()),
        _ => None,
    }
}

fn line_curve(g: &Geom) -> Curve {
    match g {
        Geom::Polyline { curve, .. } => *curve,
        _ => Curve::Linear,
    }
}

/// Flattening tolerance in a shape's own units (a tenth of a pixel).
fn line_tol(a: &Node, b: &Node, scale: f64) -> f64 {
    let k = scale * a.common.transform.scale_factor().max(b.common.transform.scale_factor());
    0.1 / if k > 0.0 && k.is_finite() { k } else { 1.0 }
}

/// Two open lines (segments count as two-point lines) planned by [`crate::lines`].
pub(crate) fn line_plan(ga: &Geom, gb: &Geom, anchors: &[(usize, usize)], tol: f64) -> Option<LinePlan> {
    let (pa, pb) = (line_pts(ga)?, line_pts(gb)?);
    crate::lines::plan_lines(&pa, line_curve(ga), &pb, line_curve(gb), anchors, tol)
}

/// Two areas planned by [`crate::lines`] (`anchors` on their tops' vertices).
pub(crate) fn area_plan(ga: &Geom, gb: &Geom, anchors: &[(usize, usize)], tol: f64) -> Option<GeomPlan> {
    let (Geom::Area { top: ta, base: ba, curve: ca }, Geom::Area { top: tb, base: bb, curve: cb }) = (ga, gb) else { return None };
    let (a, b, curve, _, _) = crate::lines::plan_areas(ta, ba, *ca, tb, bb, *cb, anchors, tol)?;
    Some(GeomPlan::Points { a: a.to_vec(), b: b.to_vec(), area: true, curve })
}

/// Two areas planned as [`area_plan`], growing (or cut back) at one end as their line does: the
/// junction at top vertex `vertex` (of `b` when growing, else of `a`).
pub(crate) fn area_grow_plan(ga: &Geom, gb: &Geom, anchors: &[(usize, usize)], tol: f64, vertex: usize, at_end: bool, growing: bool) -> Option<GeomPlan> {
    let (Geom::Area { top: ta, base: ba, curve: ca }, Geom::Area { top: tb, base: bb, curve: cb }) = (ga, gb) else { return None };
    let (a, b, curve, va, vb) = crate::lines::plan_areas(ta, ba, *ca, tb, bb, *cb, anchors, tol)?;
    let junction = *if growing { vb.get(vertex)? } else { va.get(vertex)? };
    Some(GeomPlan::Grow { a: a.to_vec(), b: b.to_vec(), area: true, curve, junction, at_end, growing })
}

/// How far along a line's extra part (points `pts` in `order`, from the junction outward) each of
/// them is, from 0 at the junction: by x when the part runs one way in x (a series — so a pen
/// moving at a steady x keeps the line's end on the edge it meets), else by length.
pub(crate) fn extra_params(pts: &[Vec2], order: &[usize]) -> Vec<f64> {
    let xs: Vec<f64> = order.iter().map(|&k| pts[k].x).collect();
    let monotone = xs.windows(2).all(|w| w[1] >= w[0]) || xs.windows(2).all(|w| w[1] <= w[0]);
    let mut out = vec![0.0; order.len()];
    for w in 1..order.len() {
        let (p, q) = (pts[order[w - 1]], pts[order[w]]);
        out[w] = out[w - 1] + if monotone { (q.x - p.x).abs() } else { p.dist(q) };
    }
    out
}

/// A [`GeomPlan::Grow`]'s rings at eased progress `e`: the shared vertices interpolated, then the
/// extra part up to the pen. The pen travels the extra part's own path (the new line's, or the old
/// line's for a part cut off), measured along the first ring (a line, an area's top) as the
/// riders' reveal places are; the other rings are cut at the same place. The drawn part starts at
/// the junction where it is now: the whole extra part moves with it, keeping its shape.
pub(crate) fn grow_rings(a: &[Vec<Vec2>], b: &[Vec<Vec2>], e: f64, junction: usize, at_end: bool, growing: bool) -> Vec<Vec<Vec2>> {
    let n = a.first().map_or(0, |r| r.len());
    if n == 0 || junction >= n {
        return a.to_vec();
    }
    // The extra part, from the junction outward.
    let order: Vec<usize> = if at_end { (junction..n).collect() } else { (0..=junction).rev().collect() };
    let src = if growing { b } else { a };
    let segs = order.len().saturating_sub(1);
    let along = extra_params(&src[0], &order);
    let total = along.last().copied().unwrap_or(0.0);
    let reach = if growing { e } else { 1.0 - e } * total;
    // The segment the pen is on, and how far along it; the whole part once it's reached the end.
    let mut cut = (segs, 0.0);
    for w in 0..segs {
        if along[w + 1] > reach {
            let d = along[w + 1] - along[w];
            cut = (w, if d > 0.0 { ((reach - along[w]) / d).clamp(0.0, 1.0) } else { 0.0 });
            break;
        }
    }
    (0..a.len())
        .map(|r| {
            let lerp = |k: usize| a[r][k].lerp(b[r][k], e);
            // The extra part, moved with the junction.
            let d = lerp(junction) - src[r][junction];
            let mut extra = vec![lerp(junction)];
            extra.extend(order[1..=cut.0.min(segs)].iter().map(|&k| src[r][k] + d));
            if cut.0 < segs && cut.1 > 0.0 {
                extra.push(src[r][order[cut.0]].lerp(src[r][order[cut.0 + 1]], cut.1) + d);
            }
            if at_end {
                (0..junction).map(lerp).chain(extra).collect()
            } else {
                extra.into_iter().rev().chain((junction + 1..n).map(lerp)).collect()
            }
        })
        .collect()
}

/// Is one open polyline the other run on (the same curve, its points a prefix of the other's)?
/// Then the longer one grows or shrinks along itself ([`GeomPlan::Extend`]). `untrimmed`: neither
/// node is already trimmed (the plan owns the trim).
fn extend_plan(ga: &Geom, gb: &Geom, untrimmed: bool) -> Option<GeomPlan> {
    let (Geom::Polyline { pts: pa, closed: false, curve: ca }, Geom::Polyline { pts: pb, closed: false, curve: cb }) = (ga, gb) else { return None };
    if !untrimmed || ca != cb || pa.len() == pb.len() || pa.is_empty() || pb.is_empty() {
        return None;
    }
    let long_b = pb.len() > pa.len();
    let (short, long) = if long_b { (pa, pb) } else { (pb, pa) };
    if long[..short.len()] != short[..] {
        return None;
    }
    // Lengths as the renderer measures them for a trim: the flattened curve.
    let len = |g: &Geom| -> f64 { g.to_path().flatten(0.1).iter().map(|(p, _)| p.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>()).sum() };
    let (ls, ll) = if long_b { (len(ga), len(gb)) } else { (len(gb), len(ga)) };
    if ll <= 1e-9 {
        return None;
    }
    let part = (ls / ll).clamp(0.0, 1.0);
    Some(if long_b { GeomPlan::Extend { from: part, to: 1.0, long_b } } else { GeomPlan::Extend { from: 1.0, to: part, long_b } })
}

fn scaled_stroke(s: Option<&Stroke>, k: f64) -> Option<Stroke> {
    s.map(|s| if s.non_scaling { s.clone() } else { Stroke { width: s.width * k, dash: s.dash.as_ref().map(|d| d.iter().map(|v| v * k).collect()), ..s.clone() } })
}

fn same_kind(a: &Node, b: &Node) -> bool {
    std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
}

impl NodeTrack {
    /// `scale` maps placement units to (approximately) screen px, for outline level of detail.
    #[allow(clippy::too_many_arguments)]
    pub fn new(a: Node, b: Node, phase: Phase, r: &Resolved, rcx: RouteCx, scale: f64, hash: u64, theme: &ResolvedTheme) -> NodeTrack {
        let same = a == b;
        let mut crossfade = !same_kind(&a, &b) || (r.morph == MorphStrategy::Crossfade && phase == Phase::Update && !same);
        let mut geom = GeomPlan::NotShape;
        let mut text = TextPlan::NotText;
        let (mut fill, mut stroke) = (PaintPlan::Same, StrokePlan { paint: PaintPlan::Same });
        if !crossfade && !same {
            match (&a.kind, &b.kind) {
                (NodeKind::Shape { geom: ga, fill: fa, stroke: sa, .. }, NodeKind::Shape { geom: gb, fill: fb, stroke: sb, .. }) => {
                    fill = plan_paint(fa.as_ref(), fb.as_ref(), theme);
                    stroke = plan_stroke(sa.as_ref(), sb.as_ref(), theme);
                    geom = if ga == gb {
                        GeomPlan::Same
                    } else if let Some(d) = translation_between(ga, gb) {
                        GeomPlan::Shift(d)
                    } else if let Some(p) = extend_plan(ga, gb, a.common.trim.is_none() && b.common.trim.is_none()) {
                        p
                    } else if lerp_geom(ga, gb, 0.5, 0.5).is_some() {
                        GeomPlan::Param
                    } else if let Some(p) = line_plan(ga, gb, &[], line_tol(&a, &b, scale)) {
                        GeomPlan::Points { a: vec![p.a], b: vec![p.b], area: false, curve: p.curve }
                    } else if let Some(p) = area_plan(ga, gb, &[], line_tol(&a, &b, scale)) {
                        p
                    } else if matches!((ga, gb), (Geom::Path { path: pa }, Geom::Path { path: pb }) if same_path_structure(pa, pb)) {
                        GeomPlan::Param
                    } else {
                        let (xa, xb) = (a.common.transform, b.common.transform);
                        let baked = xa.0[..4] != xb.0[..4];
                        let (ba, bb) = if baked { (xa, xb) } else { (Affine::IDENTITY, Affine::IDENTITY) };
                        let (sa_, sb_) = if baked { (scale, scale) } else { (scale * xa.scale_factor(), scale * xb.scale_factor()) };
                        let tol = |s: f64| 0.25 / s.max(1e-9);
                        let ringsa = rings(ga, &ba, tol(sa_));
                        let ringsb = rings(gb, &bb, tol(sb_));
                        let per = ringsa.first().map_or(0.0, |r| perimeter(r) * sa_).max(ringsb.first().map_or(0.0, |r| perimeter(r) * sb_));
                        let n = lod_points(per);
                        let morph = Morph::new(ringsa, ringsb, n, ba.apply(ga.center()), bb.apply(gb.center()), r.morph != MorphStrategy::Resample);
                        GeomPlan::Morph(Box::new(MorphGeom {
                            morph,
                            baked,
                            stroke_a: if baked { scaled_stroke(sa.as_ref(), xa.scale_factor()) } else { None },
                            stroke_b: if baked { scaled_stroke(sb.as_ref(), xb.scale_factor()) } else { None },
                        }))
                    };
                }
                (NodeKind::Text(ta), NodeKind::Text(tb)) => match plan_text(ta, tb, theme) {
                    Some(p) => text = p,
                    None => crossfade = true,
                },
                (NodeKind::Image { asset: aa, .. }, NodeKind::Image { asset: ab, .. }) if aa != ab => crossfade = true,
                _ => {}
            }
        }
        let center = |n: &Node| n.common.transform.apply(content_bounds(n).1);
        let ra = center(&a);
        NodeTrack {
            ra,
            rb: if same { ra } else { center(&b) },
            a,
            b,
            phase,
            win: (0.0, 1.0),
            mono: r.easing.is_monotone(),
            easing: r.easing.clone(),
            geom,
            text,
            fill,
            stroke,
            route: r.route.clone(),
            hash,
            rcx,
            same,
            crossfade,
            px: scale,
            follow: None,
            follow_timing: None,
        }
    }

    fn frame(&self, t: f64, shaper: &dyn TextShaper, out: &mut Vec<Node>) {
        let u = progress(t, self.win);
        // Carried along a line: where the line is, on its own timing.
        let e_line = self.follow_timing.as_ref().map(|(w, ez)| ez.apply(progress(t, *w)));
        let carried = |n: &Node, base: fn(&Follow) -> Vec2| -> Node {
            let (Some(f), Some(el)) = (&self.follow, e_line) else { return n.clone() };
            let d = f.delta_from(el, base(f));
            let mut n = n.clone();
            if d != Vec2::ZERO {
                n.common.transform = n.common.transform.then(Affine::translate(d.x, d.y));
            }
            n
        };
        if self.same || u <= 0.0 {
            out.push(carried(&self.a, |f| f.lin.0));
            return;
        }
        if u >= 1.0 {
            out.push(carried(&self.b, |f| f.lin.1));
            return;
        }
        let e = self.easing.apply(u);
        let mono_p = if self.mono { e.clamp(0.0, 1.0) } else { u };
        let op = if self.phase == Phase::Update { e.clamp(0.0, 1.0) } else { mono_p };
        let (a, b) = (&self.a, &self.b);
        let late = u >= 0.5;
        let meta = if late { b } else { a };
        let offset = if self.route.is_straight() { Vec2::ZERO } else { self.route.offset(self.ra, self.rb, e.clamp(0.0, 1.0), self.hash, &self.rcx) };
        let route = offset;
        let el = e_line.unwrap_or(e);
        let offset = offset + self.follow.as_ref().map_or(Vec2::ZERO, |f| f.delta_from(el, f.lin.0.lerp(f.lin.1, e)));
        let shift = |mut c: Common, d: Vec2| -> Common {
            if d != Vec2::ZERO {
                c.transform = c.transform.then(Affine::translate(d.x, d.y));
            }
            c
        };
        let place = |c: Common| -> Common { shift(c, offset) };
        if self.crossfade {
            // Both copies travel together; the outgoing one fades out as the incoming fades in.
            let k = mono_p;
            let mut ca = lerp_common(&a.common, &b.common, e, op, u);
            let mut cb = ca.clone();
            let eff = |c: &Common| if c.visible { c.opacity } else { 0.0 };
            // Two texts never show at once (overlapping words are unreadable): the old one fades
            // out in the first half, the new one in during the second. Shapes blend throughout.
            let (ka, kb) = if matches!((&a.kind, &b.kind), (NodeKind::Text(_), NodeKind::Text(_))) {
                ((1.0 - 2.0 * k).max(0.0), (2.0 * k - 1.0).max(0.0))
            } else {
                (1.0 - k, k)
            };
            ca.opacity = eff(&a.common) * ka;
            cb.opacity = eff(&b.common) * kb;
            // Carried along a line, each copy moves from its own marker's place (they don't lerp).
            let (da, db) = match &self.follow {
                Some(f) => (route + f.delta_from(el, f.lin.0), route + f.delta_from(el, f.lin.1)),
                None => (offset, offset),
            };
            let mut na = a.clone();
            na.common = shift(ca, da);
            let mut nb = b.clone();
            nb.common = shift(cb, db);
            out.push(na);
            out.push(nb);
            return;
        }
        let mut common = lerp_common(&a.common, &b.common, e, op, u);
        let kind = match (&a.kind, &b.kind) {
            (NodeKind::Shape { geom: ga, fill: fa, stroke: sa, markers: ma }, NodeKind::Shape { geom: gb, fill: fb, stroke: sb, markers: mb }) => {
                let fill = paint_at(&self.fill, fa.as_ref(), fb.as_ref(), e, u);
                let mut stroke = stroke_at(&self.stroke, sa.as_ref(), sb.as_ref(), e, u);
                let geom = match &self.geom {
                    GeomPlan::Same | GeomPlan::NotShape => ga.clone(),
                    GeomPlan::Param => lerp_geom(ga, gb, e, u).unwrap_or_else(|| if late { gb.clone() } else { ga.clone() }),
                    GeomPlan::Shift(d) => {
                        common.transform = Affine::translate(d.x * e, d.y * e).then(common.transform);
                        ga.clone()
                    }
                    GeomPlan::Points { a: pa, b: pb, area, curve: forced } => {
                        let lp = |x: &[Vec2], y: &[Vec2]| -> Arc<[Vec2]> { x.iter().zip(y).map(|(p, q)| p.lerp(*q, e)).collect() };
                        let curve = |g: &Geom| match g {
                            Geom::Polyline { curve, .. } | Geom::Area { curve, .. } => *curve,
                            _ => Default::default(),
                        };
                        let cv = forced.unwrap_or(if late { curve(gb) } else { curve(ga) });
                        if *area {
                            Geom::Area { top: lp(&pa[0], &pb[0]), base: lp(&pa[1], &pb[1]), curve: cv }
                        } else {
                            Geom::Polyline { pts: lp(&pa[0], &pb[0]), closed: false, curve: cv }
                        }
                    }
                    GeomPlan::Grow { a: pa, b: pb, area, curve: forced, junction, at_end, growing } => {
                        let curve = |g: &Geom| match g {
                            Geom::Polyline { curve, .. } | Geom::Area { curve, .. } => *curve,
                            _ => Default::default(),
                        };
                        let cv = forced.unwrap_or(if late { curve(gb) } else { curve(ga) });
                        let rings = grow_rings(pa, pb, e, *junction, *at_end, *growing);
                        if *area && rings.len() == 2 {
                            Geom::Area { top: rings[0].clone().into(), base: rings[1].clone().into(), curve: cv }
                        } else {
                            Geom::Polyline { pts: rings[0].clone().into(), closed: false, curve: cv }
                        }
                    }
                    GeomPlan::Extend { from, to, long_b } => {
                        let f = from + (to - from) * e;
                        common.trim = Some([0.0, f.clamp(0.0, 1.0)]);
                        if *long_b { gb.clone() } else { ga.clone() }
                    }
                    GeomPlan::Morph(mg) => {
                        if mg.baked {
                            common.transform = Affine::IDENTITY;
                            common.clip = (if late { &b.common } else { &a.common }).clip.as_ref().map(|c| {
                                let xf = if late { b.common.transform } else { a.common.transform };
                                let p = match c {
                                    Clip::Rect { rect } => PathData::rect(*rect),
                                    Clip::Path { path } => (**path).clone(),
                                };
                                Clip::Path { path: Arc::new(p.transform(&xf)) }
                            });
                            stroke = stroke_at(&self.stroke, mg.stroke_a.as_ref(), mg.stroke_b.as_ref(), e, u);
                        }
                        Geom::path(mg.morph.at(e))
                    }
                };
                NodeKind::Shape { geom, fill, stroke, markers: if late { *mb } else { *ma } }
            }
            (NodeKind::Text(ta), NodeKind::Text(tb)) => NodeKind::Text(text_at(&self.text, ta, tb, e, u, shaper)),
            (NodeKind::Image { rect: ra, .. }, NodeKind::Image { asset, rect: rb }) => NodeKind::Image { asset: asset.clone(), rect: lerp_rect(ra, rb, e) },
            _ => meta.kind.clone(),
        };
        let anchors = if a.anchors.len() == b.anchors.len() && a.anchors.iter().zip(&b.anchors).all(|(x, y)| x.name == y.name) {
            a.anchors.iter().zip(&b.anchors).map(|(x, y)| datars_scene::Anchor { name: x.name.clone(), at: x.at.lerp(y.at, e) }).collect()
        } else {
            meta.anchors.clone()
        };
        // The destination's key throughout: an element's identity mid-flight is the datum it is
        // becoming (exits keep theirs — their ghost shares it).
        out.push(Node { key: b.key.clone(), kind, common: place(common), semantics: meta.semantics.clone(), pickable: meta.pickable, anchors, prov: meta.prov });
    }
}

/// A seekable transition between two scenes.
#[derive(Clone, Debug)]
pub struct Plan {
    pub(crate) from: Scene,
    pub(crate) to: Scene,
    pub(crate) duration: f64,
    pub(crate) structs: Vec<StructTrack>,
    pub(crate) nodes: Vec<NodeTrack>,
    pub(crate) columns: Vec<ColumnTrack>,
    pub(crate) root: usize,
    pub(crate) stats: PlanStats,
    pub(crate) corr: Correspondence,
}

impl Plan {
    /// Total length in seconds (the longest delay + duration of any element).
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// The scene at normalized time `t` ∈ [0, 1]: exactly `from` at 0 (and before), exactly `to`
    /// at 1 (and after). NaN counts as 0.
    pub fn at(&self, t: f64, shaper: &dyn TextShaper) -> Scene {
        if t.is_nan() || t <= 0.0 {
            return self.from.clone();
        }
        if t >= 1.0 {
            return self.to.clone();
        }
        let late = t >= 0.5;
        let root = self.frame_struct(self.root, t, shaper);
        let root = if self.structs[self.root].virtual_root {
            match root.kind {
                NodeKind::Group { mut children } if children.len() == 1 => children.pop().unwrap_or(root_placeholder()),
                kind => Node { kind, ..root },
            }
        } else {
            root
        };
        Scene {
            width: lerp(self.from.width, self.to.width, t),
            height: lerp(self.from.height, self.to.height, t),
            background: if late { self.to.background.clone() } else { self.from.background.clone() },
            root,
        }
    }

    /// The frame at `t` as a new "from" for retargeting (the same as [`Plan::at`]).
    pub fn snapshot(&self, t: f64, shaper: &dyn TextShaper) -> Scene {
        self.at(t, shaper)
    }

    pub fn from(&self) -> &Scene {
        &self.from
    }
    pub fn to(&self) -> &Scene {
        &self.to
    }
    pub fn stats(&self) -> &PlanStats {
        &self.stats
    }
    /// Who became whom, as key paths.
    pub fn correspondence(&self) -> &Correspondence {
        &self.corr
    }

    /// Retarget at plan time `t` towards `to`, carrying velocity: plans from `self.at(t)` (as
    /// [`crate::retarget`]) and gives every spring-eased element that was already moving the
    /// initial velocity it had, so an interruption does not stall it. Elements are identified by
    /// their key (the first track with a key wins); non-spring easings start from rest.
    pub fn retarget(&self, t: f64, to: &Scene, rules: &MotionRules, cx: &PlanCx, shaper: &dyn TextShaper) -> Plan {
        let current = self.at(t, shaper);
        let mut next = crate::build::plan(&current, to, rules, cx);
        if !(t > 0.0 && t < 1.0) || self.duration <= 0.0 {
            return next;
        }
        // Old progress rate (per second) and progress of every moving update track.
        let mut rates: BTreeMap<Key, (f64, f64)> = BTreeMap::new();
        for n in &self.nodes {
            if n.phase != Phase::Update || n.same {
                continue;
            }
            let u = progress(t, n.win);
            let span = n.win.1 - n.win.0;
            if !(u > 0.0 && u < 1.0) || span <= 0.0 {
                continue;
            }
            let h = 1e-4f64.min(u).min(1.0 - u);
            let de_du = (n.easing.apply(u + h) - n.easing.apply(u - h)) / (2.0 * h);
            let rate = de_du / (span * self.duration);
            if rate.is_finite() {
                rates.entry(n.b.key.clone()).or_insert((rate, n.easing.apply(u)));
            }
        }
        let dur = next.duration;
        for n in &mut next.nodes {
            let Easing::Spring { stiffness, damping, mass, .. } = n.easing else { continue };
            if n.phase != Phase::Update {
                continue;
            }
            let Some(&(rate, e)) = rates.get(&n.a.key) else { continue };
            // New progress p covers the old remaining distance: dp/ds = (de/ds) / (1 − e).
            let rem = (1.0 - e).abs().max(1e-6);
            let settle = crate::easing::spring_settle_time(stiffness, damping, mass, 0.0);
            let span = n.win.1 - n.win.0;
            if settle > 0.0 && span > 0.0 {
                let v0 = rate / rem * span * dur / settle;
                if v0.is_finite() {
                    n.easing = Easing::Spring { stiffness, damping, mass, v0 };
                    n.mono = n.easing.is_monotone();
                }
            }
        }
        // Riders keep their line's easing, velocity included.
        for c in &mut next.columns {
            if let Some(e) = c.line.and_then(|t| next.nodes.get(t)).map(|n| n.easing.clone()) {
                c.set_easing(e);
            }
        }
        next
    }

    /// The window [start, end] (normalized) of every node track, in build order — for timeline
    /// inspection.
    pub fn windows(&self) -> Vec<(Key, (f64, f64))> {
        self.nodes.iter().map(|n| (n.b.key.clone(), n.win)).collect()
    }

    fn frame_struct(&self, s: usize, t: f64, shaper: &dyn TextShaper) -> Node {
        let st = &self.structs[s];
        let mut children = Vec::with_capacity(st.children.len());
        for slot in &st.children {
            match slot {
                Slot::Struct(i) => children.push(self.frame_struct(*i, t, shaper)),
                Slot::Node(i) => self.nodes[*i].frame(t, shaper, &mut children),
                Slot::Column(i) => self.columns[*i].frame(t, &mut children),
            }
        }
        let mut n = st.state(t);
        if let Some(c) = n.children_mut() {
            *c = children;
        }
        n
    }
}

fn root_placeholder() -> Node {
    Node::group(Key::default(), Vec::new())
}

/// A stable hash of a key path (route jitter, random orders).
pub(crate) fn path_hash(p: &KeyPath) -> u64 {
    let mut h = Hash64::new();
    for k in &p.0 {
        h.u8(0xff);
        for part in k.parts() {
            match part {
                KeyPart::Int(v) => {
                    h.u8(0);
                    h.u64(*v as u64);
                }
                KeyPart::Str(s) => {
                    h.u8(1);
                    h.str(s);
                }
                KeyPart::Unit { unit } => {
                    h.u8(2);
                    h.u32(*unit);
                }
            }
        }
    }
    h.finish()
}

