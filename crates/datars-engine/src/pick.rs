//! Hit testing and anchors on a frame's scene (exact at any t: geometry, not a cached index).

use crate::AnchorOut;
use datars_math::{Affine, Vec2};
use datars_scene::{Key, KeyPath, Node, NodeKind, Scene};

#[derive(Clone, Debug)]
pub struct Hit {
    pub path: KeyPath,
    pub label: Option<String>,
    /// Inside a control (role `control`): it shows its own value, so it gets no tooltip.
    pub control: bool,
}

impl Hit {
    pub fn key_string(&self) -> String {
        self.path.last().map(|k| k.to_string()).unwrap_or_default()
    }
}

/// An element this faint — its opacity times its ancestors' — isn't there for the reader: the
/// pointer passes through it (to what is visible under it) and screen readers don't read it out.
/// A layer faded out over another (a county map over a street map) is the common case.
pub(crate) const INVISIBLE: f64 = 0.01;

/// The topmost pickable element under `p` (CSS px); else the instance nearest to it within a few
/// px (small marks are hard to hit exactly). Decoration and invisible elements aren't picked.
pub fn pick(scene: &Scene, p: Vec2) -> Option<Hit> {
    pick_within(scene, p, 1.0)
}

/// [`pick`] with every reach around thin and small marks (open lines, dots, lines of dots) scaled
/// by `reach`: a finger covers far more than a mouse pointer's tip ([`TOUCH_REACH`]).
pub fn pick_within(scene: &Scene, p: Vec2, reach: f64) -> Option<Hit> {
    let mut best: Option<(Hit, f64)> = None;
    let mut floating = Vec::new();
    walk(&scene.root, &KeyPath::default(), Affine::IDENTITY, 1.0, p, &mut best, (None, false), reach, &mut floating);
    // Floating nodes are drawn above everything, so they're offered last (and win what's under).
    while !floating.is_empty() {
        let mut all = std::mem::take(&mut floating);
        all.sort_by_key(|f: &Floater| f.0.common.z);
        for (n, path, xf, acc, inherited) in all {
            walk(n, &path, xf, acc, p, &mut best, inherited, reach, &mut floating);
        }
    }
    best.map(|(h, _)| h)
}

/// What a node takes from its ancestors when picked: the nearest label, and whether it's inside a
/// control.
type Inherited<'a> = (Option<&'a str>, bool);

/// A floating node met on the way ([`datars_scene::FLOAT_Z`]): the node, its parent's path,
/// transform, opacity and what it inherits — picked after everything else.
type Floater<'a> = (&'a Node, KeyPath, Affine, f64, Inherited<'a>);

/// How much further a tap reaches than a mouse pointer: a fingertip is ~10 px across, and lands
/// a few px off what the reader aimed at.
pub const TOUCH_REACH: f64 = 2.5;

/// A node's opacity under ancestors of opacity `acc` (0 when hidden).
pub(crate) fn seen(n: &Node, acc: f64) -> f64 {
    if n.common.visible {
        acc * n.common.opacity
    } else {
        0.0
    }
}

/// Is every node on `path` visible, their opacities together above [`INVISIBLE`]?
pub fn shown_at(scene: &Scene, path: &KeyPath) -> bool {
    let mut n = &scene.root;
    if path.0.first() != Some(&n.key) {
        return false;
    }
    let mut acc = seen(n, 1.0);
    for k in &path.0[1..] {
        match n.children().iter().find(|c| &c.key == k) {
            Some(c) => {
                acc = seen(c, acc);
                n = c;
            }
            None => return false,
        }
    }
    acc > INVISIBLE
}

/// Keep `hit` if it beats `best`: anything under the pointer beats a near miss (and a later one —
/// drawn on top — beats an earlier one); among near misses the nearest wins.
fn offer(best: &mut Option<(Hit, f64)>, hit: Hit, gap: f64) {
    if gap <= 0.0 || best.as_ref().is_none_or(|(_, g)| *g > gap) {
        *best = Some((hit, gap.max(0.0)));
    }
}

#[allow(clippy::too_many_arguments)]
fn walk<'a>(n: &'a Node, path: &KeyPath, parent: Affine, acc: f64, p: Vec2, best: &mut Option<(Hit, f64)>, inherited: Inherited<'a>, k: f64, floating: &mut Vec<Floater<'a>>) {
    // Decoration (gridlines, a glow under data, a basemap) is never what the pointer means, and
    // what can't be seen isn't there.
    let acc = seen(n, acc);
    if acc <= INVISIBLE || n.semantics.as_ref().is_some_and(|s| s.role == datars_scene::Role::Decoration) {
        return;
    }
    let xf = n.common.placed(parent);
    let here = path.push(&n.key);
    let label = n.semantics.as_ref().map(|s| s.label.as_str()).filter(|l| !l.is_empty()).or(inherited.0);
    let control = inherited.1 || n.semantics.as_ref().is_some_and(|s| s.role == datars_scene::Role::Control);
    let inherited = (label, control);
    match &n.kind {
        NodeKind::Group { children } => {
            for c in children {
                if c.common.floats() {
                    floating.push((c, here.clone(), xf, acc, inherited));
                } else {
                    walk(c, &here, xf, acc, p, best, inherited, k, floating);
                }
            }
        }
        NodeKind::View { viewport, camera, children, .. } => {
            let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            for c in children {
                if c.common.floats() {
                    floating.push((c, here.clone(), xf.mul(cam), acc, inherited));
                } else {
                    walk(c, &here, xf.mul(cam), acc, p, best, inherited, k, floating);
                }
            }
        }
        NodeKind::Shape { geom, fill, .. } if n.pickable => {
            if let Some(inv) = xf.inverse() {
                let lp = inv.apply(p);
                let closed = hit_inside(geom, fill.is_some());
                let line = REACH * k;
                // Outside the box (with an open line's reach): no need to flatten it (hover runs
                // this over every shape on each pointer move).
                let inside = geom.bounds().inset(if closed { -0.5 } else { -line }).contains(lp)
                    && if closed {
                        geom.flatten(0.5).iter().any(|(pts, _)| point_in_poly(lp, pts))
                    } else {
                        geom.flatten(0.5).iter().any(|(pts, _)| pts.windows(2).any(|w| dist_seg(lp, w[0], w[1]) < line))
                    };
                if inside {
                    offer(best, Hit { path: here, label: label.map(String::from), control }, 0.0);
                }
            }
        }
        // A pickable text is hit by its box (a checkbox's or a switch's label).
        NodeKind::Text(t) if n.pickable && !t.text.trim().is_empty() => {
            let b = datars_math::Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h);
            if crate::bounds::transform_rect(b, &xf).inset(-2.0).contains(p) {
                offer(best, Hit { path: here, label: label.map(String::from), control }, 0.0);
            }
        }
        NodeKind::Instances(inst) => {
            let found = match inst.line_reach {
                Some(reach) => along_line(inst, &xf, acc, p, reach * k),
                None => instance_within(inst, &xf, acc, p, REACH * k),
            };
            if let Some((i, gap)) = found {
                let l = inst.labels.as_ref().and_then(|ls| ls.get(i).cloned()).or(label.map(String::from));
                offer(best, Hit { path: here.push(&inst.keys[i]), label: l, control }, gap);
            }
        }
        _ => {}
    }
}

/// Instances hit as the line through them in order (`hit: line`): if `p` (screen px) is within
/// `reach` of that line, the instance nearest to where it meets the line — the endpoint of the
/// nearest segment on `p`'s side of its middle — and how far off the line `p` is beyond
/// [`REACH`] (0: on it). A line chart's value wherever it's hovered, its points however far apart;
/// among several lines the nearest wins.
fn along_line(inst: &datars_scene::Instances, xf: &Affine, acc: f64, p: Vec2, reach: f64) -> Option<(usize, f64)> {
    let pts: Vec<(usize, Vec2)> = (0..inst.len())
        .filter(|&i| acc * inst.opacity_at(i) > INVISIBLE)
        .map(|i| (i, xf.apply(Vec2::new(inst.x[i], inst.y[i]))))
        .filter(|(_, q)| q.is_finite())
        .collect();
    let mut best: Option<(f64, usize)> = None;
    let mut consider = |d: f64, i: usize| {
        if d <= reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, i));
        }
    };
    match pts.as_slice() {
        [] => return None,
        [(i, q)] => consider(p.dist(*q), *i),
        _ => {
            for w in pts.windows(2) {
                let ((i, a), (j, b)) = (w[0], w[1]);
                let ab = b - a;
                let t = if ab.len2() > 0.0 { ((p - a).dot(ab) / ab.len2()).clamp(0.0, 1.0) } else { 0.0 };
                consider(p.dist(a + ab * t), if t < 0.5 { i } else { j });
            }
        }
    }
    best.map(|(d, i)| (i, (d - REACH).max(0.0)))
}

/// Screen px around a small mark that still count as on it: a star of radius 1 in a field of
/// thousands is hard to hit exactly.
const REACH: f64 = 4.0;

/// Is a shape hit by its inside (else along its outline, within reach of it)? Closed geometry is,
/// except a path drawn with no fill: a series' line or a route is only its stroke, and hit by its
/// inside it would claim the whole hull between its ends (the last of a thousand lines would win
/// every hover).
fn hit_inside(geom: &datars_scene::Geom, filled: bool) -> bool {
    geom.is_closed() && (filled || !matches!(geom, datars_scene::Geom::Path { .. }))
}

/// The instance under `p` (screen px) of instances drawn through `xf`, and how far outside its
/// mark `p` is (0: on it): the topmost whose mark contains `p`, else the nearest within
/// [`REACH`] px of its edge. Measured on screen, so marks that keep their screen size under a
/// zooming camera are hit where they're drawn. `acc` is the node's opacity with its ancestors':
/// instances that come out [`INVISIBLE`] aren't hit.
/// Where instance `i` is drawn, in root px: a screen-sized symbol around its projected centre,
/// else its geometry through `xf` (what a hit reports and `explain` outlines).
pub(crate) fn instance_bounds(inst: &datars_scene::Instances, xf: &Affine, i: usize) -> datars_math::Rect {
    if inst.screen_size {
        let c = xf.apply(Vec2::new(inst.x[i], inst.y[i]));
        let r = inst.size_at(i);
        datars_math::Rect::new(c.x - r, c.y - r, 2.0 * r, 2.0 * r)
    } else {
        crate::bounds::transform_rect(inst.geom(i).bounds(), xf)
    }
}

pub(crate) fn instance_at(inst: &datars_scene::Instances, xf: &Affine, acc: f64, p: Vec2) -> Option<(usize, f64)> {
    instance_within(inst, xf, acc, p, REACH)
}

/// [`instance_at`] within `reach` px of a mark's edge.
fn instance_within(inst: &datars_scene::Instances, xf: &Affine, acc: f64, p: Vec2, reach: f64) -> Option<(usize, f64)> {
    let shown = |i: usize| acc * inst.opacity_at(i) > INVISIBLE;
    if let datars_scene::Proto::Rect = inst.proto {
        let inv = xf.inverse()?;
        let lp = inv.apply(p);
        let tol = reach / xf.scale_factor().max(1e-12);
        return (0..inst.len()).rev().find(|&i| shown(i) && inst.geom(i).bounds().inset(-tol).contains(lp)).map(|i| (i, 0.0));
    }
    let scale = if inst.screen_size { 1.0 } else { xf.scale_factor() };
    let mut near: Option<(f64, usize)> = None;
    for i in (0..inst.len()).rev() {
        if !shown(i) {
            continue;
        }
        let c = xf.apply(Vec2::new(inst.x[i], inst.y[i]));
        let (dx, dy) = (c.x - p.x, c.y - p.y);
        if dx.abs() > 64.0 + reach || dy.abs() > 64.0 + reach {
            continue; // far away: skip the square root
        }
        let r = inst.size_at(i).abs() * scale;
        let d = (dx * dx + dy * dy).sqrt();
        if d <= r {
            return Some((i, 0.0));
        }
        if d - r <= reach && near.is_none_or(|(g, _)| d - r < g) {
            near = Some((d - r, i));
        }
    }
    near.map(|(g, i)| (i, g))
}

/// Any drawn element under a point, for editors: what the pointer is on, pickable or not (text,
/// axes, gridlines, backdrops, marks), topmost first in drawing order.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct AnyHit {
    /// Key path, as `inspect`, `explain` and semantics spell it.
    pub path: String,
    /// `rect`, `text`, `path`, `instance`, …
    pub kind: String,
    /// Semantic role, when it has one (`datum`, `title`, `axis`, `annotation`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// A text element's text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Canvas-px bounds `[x, y, w, h]`.
    pub bounds: [f64; 4],
}

/// Every element under `p`, topmost first (the last drawn wins, as on screen).
pub fn pick_all(scene: &Scene, p: Vec2) -> Vec<AnyHit> {
    pick_many(scene, &[p]).pop().unwrap_or_default()
}

/// [`pick_all`] for several points in one walk of the scene: each shape's box and outline are
/// worked out once for all of them. (A card's placement samples sixty points; walked once per
/// point, a contour chart's paths had their bounds taken sixty times — 25 ms of its first frame.)
pub fn pick_many(scene: &Scene, pts: &[Vec2]) -> Vec<Vec<AnyHit>> {
    /// A floating node, its parent's path, transform, opacity and role: gone through last.
    type Later<'a> = (&'a Node, KeyPath, Affine, f64, Option<&'a str>);
    #[allow(clippy::too_many_arguments)]
    fn go<'a>(n: &'a Node, path: &KeyPath, parent: Affine, acc: f64, pts: &[Vec2], role: Option<&'a str>, out: &mut [Vec<AnyHit>], later: &mut Vec<Later<'a>>) {
        let acc = seen(n, acc);
        if acc <= INVISIBLE {
            return;
        }
        let xf = n.common.placed(parent);
        // The path, role and label are built for groups (their children inherit them) and for hits:
        // a county map's thousands of shapes miss almost every point, and building them for each
        // shape and point made placing a narration card cost 45 ms.
        let own_role = || n.semantics.as_ref().map(|s| format!("{:?}", s.role).to_lowercase());
        let label = || n.semantics.as_ref().map(|s| s.label.clone()).filter(|l| !l.is_empty());
        let rect = |b: datars_math::Rect| -> [f64; 4] {
            let r = crate::bounds::transform_rect(b, &xf);
            [r.x, r.y, r.w, r.h]
        };
        let role_of = |own: Option<String>| own.or(role.map(String::from));
        match &n.kind {
            NodeKind::Group { children } => {
                let here = path.push(&n.key);
                let role = n.semantics.as_ref().map(|s| role_name(s.role)).or(role);
                for c in children {
                    if c.common.floats() {
                        later.push((c, here.clone(), xf, acc, role));
                    } else {
                        go(c, &here, xf, acc, pts, role, out, later);
                    }
                }
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let here = path.push(&n.key);
                let role = n.semantics.as_ref().map(|s| role_name(s.role)).or(role);
                let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
                for c in children {
                    if c.common.floats() {
                        later.push((c, here.clone(), xf.mul(cam), acc, role));
                    } else {
                        go(c, &here, xf.mul(cam), acc, pts, role, out, later);
                    }
                }
            }
            NodeKind::Text(t) if !t.text.trim().is_empty() => {
                let b = datars_math::Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h);
                let r = crate::bounds::transform_rect(b, &xf);
                for (k, p) in pts.iter().enumerate() {
                    if r.inset(-2.0).contains(*p) {
                        out[k].push(AnyHit { path: path.push(&n.key).to_string(), kind: "text".into(), role: role_of(own_role()), label: label(), text: Some(t.text.clone()), bounds: [r.x, r.y, r.w, r.h] });
                    }
                }
            }
            NodeKind::Shape { geom, fill, .. } => {
                if let Some(inv) = xf.inverse() {
                    let closed = hit_inside(geom, fill.is_some());
                    // Outside the shape's box (with the reach of an open line), it can't be a hit.
                    let bounds = geom.bounds();
                    let reach = bounds.inset(if closed { -0.5 } else { -4.0 });
                    let mut polys = None;
                    for (k, p) in pts.iter().enumerate() {
                        let lp = inv.apply(*p);
                        if !reach.contains(lp) {
                            continue;
                        }
                        let polys = polys.get_or_insert_with(|| geom.flatten(0.5));
                        let inside = if closed { polys.iter().any(|(pts, _)| point_in_poly(lp, pts)) } else { polys.iter().any(|(pts, _)| pts.windows(2).any(|w| dist_seg(lp, w[0], w[1]) < 4.0)) };
                        if inside {
                            out[k].push(AnyHit { path: path.push(&n.key).to_string(), kind: geom.kind_name().into(), role: role_of(own_role()), label: label(), text: None, bounds: rect(bounds) });
                        }
                    }
                }
            }
            NodeKind::Instances(inst) => {
                for (k, p) in pts.iter().enumerate() {
                    if let Some((i, _)) = instance_at(inst, &xf, acc, *p) {
                        let l = inst.labels.as_ref().and_then(|ls| ls.get(i).cloned()).or_else(label);
                        let r = instance_bounds(inst, &xf, i);
                        let b = [r.x, r.y, r.w, r.h];
                        out[k].push(AnyHit { path: path.push(&n.key).push(&inst.keys[i]).to_string(), kind: "instance".into(), role: role_of(own_role()), label: l, text: None, bounds: b });
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = vec![Vec::new(); pts.len()];
    let mut later = Vec::new();
    go(&scene.root, &KeyPath::default(), Affine::IDENTITY, 1.0, pts, None, &mut out, &mut later);
    while !later.is_empty() {
        let mut all = std::mem::take(&mut later);
        all.sort_by_key(|l| l.0.common.z);
        for (n, path, xf, acc, role) in all {
            go(n, &path, xf, acc, pts, role, &mut out, &mut later);
        }
    }
    for hits in &mut out {
        hits.reverse();
    }
    out
}

/// A role as `pick_all` reports it — its name in lower case, `legenditem` included — static, so
/// children can borrow it.
fn role_name(r: datars_scene::Role) -> &'static str {
    use datars_scene::Role as R;
    match r {
        R::Group => "group",
        R::Datum => "datum",
        R::Series => "series",
        R::Region => "region",
        R::Axis => "axis",
        R::Tick => "tick",
        R::Grid => "grid",
        R::Legend => "legend",
        R::LegendItem => "legenditem",
        R::Annotation => "annotation",
        R::Title => "title",
        R::Label => "label",
        R::Tooltip => "tooltip",
        R::Control => "control",
        R::Decoration => "decoration",
    }
}

fn point_in_poly(p: Vec2, pts: &[Vec2]) -> bool {
    let mut inside = false;
    let n = pts.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_seg(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let ab = b - a;
    let t = if ab.len2() > 0.0 { ((p - a).dot(ab) / ab.len2()).clamp(0.0, 1.0) } else { 0.0 };
    p.dist(a + ab * t)
}

/// Screen positions of every declared anchor.
pub fn anchors(scene: &Scene) -> Vec<AnchorOut> {
    let mut out = Vec::new();
    fn go(n: &Node, path: &KeyPath, xf: Affine, out: &mut Vec<AnchorOut>) {
        let xf = n.common.placed(xf);
        let here = path.push(&n.key);
        for a in &n.anchors {
            let q = xf.apply(a.at);
            out.push(AnchorOut { path: here.to_string(), name: a.name.to_string(), x: q.x, y: q.y });
        }
        let inner = match &n.kind {
            NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
            _ => xf,
        };
        for c in n.children() {
            go(c, &here, inner, out);
        }
    }
    go(&scene.root, &KeyPath::default(), Affine::IDENTITY, &mut out);
    let _ = Key::default();
    out
}

/// [`node_at`] for a key path as text (`("chart",)/("SE",)`, as origins and semantics spell it).
pub fn node_at_str<'a>(scene: &'a Scene, path: &str) -> Option<(&'a Node, Affine)> {
    fn go<'a>(n: &'a Node, here: KeyPath, want: &str, parent: Affine) -> Option<(&'a Node, Affine)> {
        let here = here.push(&n.key);
        let s = here.to_string();
        if !(want == s || want.starts_with(&format!("{s}/"))) {
            return None;
        }
        let xf = parent.mul(n.common.transform);
        if want == s {
            return Some((n, xf));
        }
        let inner = match &n.kind {
            NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
            _ => xf,
        };
        n.children().iter().find_map(|c| go(c, here.clone(), want, inner))
    }
    go(&scene.root, KeyPath::default(), path, Affine::IDENTITY)
}

/// A node by key path in the current scene, with the transform from its local space to root
/// space (its own transform included; for views, the local space is the viewport's, before the
/// camera).
pub fn node_at<'a>(scene: &'a Scene, path: &KeyPath) -> Option<(&'a Node, Affine)> {
    fn go<'a>(n: &'a Node, here: KeyPath, want: &KeyPath, parent: Affine) -> Option<(&'a Node, Affine)> {
        let here = here.push(&n.key);
        if !want.0.starts_with(&here.0) {
            return None;
        }
        let xf = parent.mul(n.common.transform);
        if here.0.len() == want.0.len() {
            return Some((n, xf));
        }
        let inner = match &n.kind {
            NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
            _ => xf,
        };
        n.children().iter().find_map(|c| go(c, here.clone(), want, inner))
    }
    go(&scene.root, KeyPath::default(), path, Affine::IDENTITY)
}

/// An element that links somewhere, where it is on screen.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct LinkOut {
    pub href: String,
    pub label: String,
    /// Canvas-px bounds `[x, y, w, h]`.
    pub bounds: [f64; 4],
}

/// Every visible element with a link (`semantics.link`), in drawing order.
pub fn links(scene: &Scene) -> Vec<LinkOut> {
    fn go(n: &Node, parent: Affine, opacity: f64, out: &mut Vec<LinkOut>) {
        let opacity = opacity * n.common.opacity;
        if !n.common.visible || opacity < 0.2 {
            return;
        }
        let xf = n.common.placed(parent);
        if let Some(href) = n.semantics.as_ref().and_then(|s| s.link.as_ref()) {
            let local = match &n.kind {
                NodeKind::Text(t) => datars_math::Rect::new(t.origin.x + t.bounds.x, t.origin.y + t.bounds.y, t.bounds.w, t.bounds.h),
                _ => crate::bounds::node_bounds(&Node { common: Default::default(), ..n.clone() }),
            };
            let r = crate::bounds::transform_rect(local, &xf);
            if r.w > 0.0 && r.h > 0.0 {
                out.push(LinkOut { href: href.to_string(), label: n.semantics.as_ref().map(|s| s.label.clone()).unwrap_or_default(), bounds: [r.x, r.y, r.w, r.h] });
            }
        }
        let inner = match &n.kind {
            NodeKind::View { viewport, camera, .. } => xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y))),
            _ => xf,
        };
        n.children().iter().for_each(|c| go(c, inner, opacity, out));
    }
    let mut out = Vec::new();
    go(&scene.root, Affine::IDENTITY, 1.0, &mut out);
    out
}
