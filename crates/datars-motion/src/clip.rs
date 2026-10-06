//! Authored animation: **clips** are pure functions of local time producing property patches over
//! keyed nodes (docs/05 §Clip algebra). Every composition has a known duration, so seeking is a
//! lookup and a film renders frame by frame with no state.
//!
//! Timing semantics are CSS `fill-mode: both`: a clip before its start holds its first frame, after
//! its end its last. In a `seq`, the active clip wins; between clips the past holds (a finished
//! clip's last frame beats a pending clip's first frame). In a `par`, later clips win.
//!
//! Patches compose onto the scene: opacity multiplies, `translate` / `transform` compose in the
//! parent's space, `scale` / `rotate` act about the node's own centre, the rest set values.

use crate::easing::Easing;
use crate::elements::content_bounds;
use crate::TextShaper;
use datars_color::Color;
use datars_math::{lerp, m, Affine, Rect, Vec2};
use datars_scene::{Key, KeyPath, Node, NodeKind, Paint, Scene};
use datars_theme::Ink;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn linear() -> Easing {
    Easing::Linear
}
fn is_linear(e: &Easing) -> bool {
    *e == Easing::Linear
}

/// An animatable property value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "prop", content = "value", rename_all = "kebab-case")]
pub enum PropValue {
    /// Multiplies the node's opacity.
    Opacity(f64),
    /// Offset in the parent's space.
    Translate(Vec2),
    /// Scale (x, y) about the node's centre.
    Scale(f64, f64),
    /// Rotation (radians, clockwise on screen) about the node's centre.
    Rotate(f64),
    /// Draw only [t0, t1] of the geometry's length.
    Trim(f64, f64),
    /// Fill ink (shapes, instances) or text ink. Literal colours mix in OKLab; tokens step at ½.
    Fill(Ink),
    StrokeWidth(f64),
    /// A text node's number (re-shaped by `apply_shaped`).
    Number(f64),
}

impl PropValue {
    /// Interpolate two values of the same property (mismatched properties step at ½).
    pub fn lerp(&self, o: &PropValue, t: f64) -> PropValue {
        use PropValue as P;
        if t <= 0.0 {
            return self.clone();
        }
        if t >= 1.0 {
            return o.clone();
        }
        match (self, o) {
            (P::Opacity(a), P::Opacity(b)) => P::Opacity(lerp(*a, *b, t)),
            (P::Translate(a), P::Translate(b)) => P::Translate(a.lerp(*b, t)),
            (P::Scale(a, b), P::Scale(c, d)) => P::Scale(lerp(*a, *c, t), lerp(*b, *d, t)),
            (P::Rotate(a), P::Rotate(b)) => P::Rotate(lerp(*a, *b, t)),
            (P::Trim(a, b), P::Trim(c, d)) => P::Trim(lerp(*a, *c, t), lerp(*b, *d, t)),
            (P::StrokeWidth(a), P::StrokeWidth(b)) => P::StrokeWidth(lerp(*a, *b, t)),
            (P::Number(a), P::Number(b)) => P::Number(lerp(*a, *b, t)),
            (P::Fill(Ink::Color(a)), P::Fill(Ink::Color(b))) => P::Fill(Ink::Color(a.lerp_oklab(*b, t))),
            _ => {
                if t < 0.5 {
                    self.clone()
                } else {
                    o.clone()
                }
            }
        }
    }

    fn write(&self, p: &mut PropPatch) {
        match self {
            PropValue::Opacity(v) => p.opacity = Some(*v),
            PropValue::Translate(v) => p.translate = Some(*v),
            PropValue::Scale(x, y) => p.scale = Some((*x, *y)),
            PropValue::Rotate(v) => p.rotate = Some(*v),
            PropValue::Trim(a, b) => p.trim = Some([*a, *b]),
            PropValue::Fill(i) => p.fill = Some(i.clone()),
            PropValue::StrokeWidth(v) => p.stroke_width = Some(*v),
            PropValue::Number(v) => p.number = Some(*v),
        }
    }
}

/// Property overrides for one node (or one instance: node path + instance key).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PropPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translate: Option<Vec2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<(f64, f64)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotate: Option<f64>,
    /// Composed after the node's transform, in the parent's space.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<Affine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trim: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Ink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

impl PropPatch {
    /// Overlay `o` (its set fields win).
    pub fn merge(&mut self, o: &PropPatch) {
        macro_rules! take {
            ($($f:ident),*) => { $( if o.$f.is_some() { self.$f = o.$f.clone(); } )* };
        }
        take!(opacity, translate, scale, rotate, transform, trim, fill, stroke_width, number, visible);
    }
}

/// One key of a keyframed clip: at `t` seconds the property is `value`; `ease` shapes the
/// segment to the next key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipKey {
    pub t: f64,
    pub value: PropValue,
    #[serde(default = "linear", skip_serializing_if = "is_linear")]
    pub ease: Easing,
}

/// A clip: authored animation as a pure function of local time (seconds).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Clip {
    /// One property of one node from `from` to `to`.
    Tween {
        target: KeyPath,
        from: PropValue,
        to: PropValue,
        duration: f64,
        #[serde(default = "linear", skip_serializing_if = "is_linear")]
        easing: Easing,
    },
    Keyframes { target: KeyPath, keys: Vec<ClipKey> },
    /// Nothing for `duration` seconds.
    Hold { duration: f64 },
    Seq { clips: Vec<Clip> },
    Par { clips: Vec<Clip> },
    Delay { by: f64, clip: Box<Clip> },
    /// Play `k` times slower (k > 1) or faster.
    Stretch { k: f64, clip: Box<Clip> },
    Reverse { clip: Box<Clip> },
    Loop { n: u32, clip: Box<Clip> },
    /// Re-time the clip through an easing curve.
    Ease { easing: Easing, clip: Box<Clip> },
    /// `clips[i]` starts `i · by` seconds after the first.
    Stagger { by: f64, clips: Vec<Clip> },
}

fn nn(v: f64) -> f64 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

impl Clip {
    /// Seconds.
    pub fn duration(&self) -> f64 {
        match self {
            Clip::Tween { duration, .. } => nn(*duration),
            Clip::Keyframes { keys, .. } => keys.iter().map(|k| nn(k.t)).fold(0.0, f64::max),
            Clip::Hold { duration } => nn(*duration),
            Clip::Seq { clips } => clips.iter().map(Clip::duration).sum(),
            Clip::Par { clips } => clips.iter().map(Clip::duration).fold(0.0, f64::max),
            Clip::Delay { by, clip } => nn(*by) + clip.duration(),
            Clip::Stretch { k, clip } => nn(*k) * clip.duration(),
            Clip::Reverse { clip } | Clip::Ease { clip, .. } => clip.duration(),
            Clip::Loop { n, clip } => *n as f64 * clip.duration(),
            Clip::Stagger { by, clips } => clips.iter().enumerate().map(|(i, c)| nn(*by) * i as f64 + c.duration()).fold(0.0, f64::max),
        }
    }

    /// The patches at local time `t` (seconds), sorted by key path.
    pub fn at(&self, t: f64) -> Vec<(KeyPath, PropPatch)> {
        let mut out = BTreeMap::new();
        self.eval(if t.is_nan() { 0.0 } else { t }, &mut out);
        out.into_iter().collect()
    }

    fn write(out: &mut BTreeMap<KeyPath, PropPatch>, target: &KeyPath, v: &PropValue) {
        v.write(out.entry(target.clone()).or_default());
    }

    fn eval(&self, t: f64, out: &mut BTreeMap<KeyPath, PropPatch>) {
        match self {
            Clip::Tween { target, from, to, duration, easing } => {
                let d = nn(*duration);
                let local = if d > 0.0 { (t / d).clamp(0.0, 1.0) } else if t >= 0.0 { 1.0 } else { 0.0 };
                Self::write(out, target, &from.lerp(to, easing.apply(local)));
            }
            Clip::Keyframes { target, keys } => {
                let Some(first) = keys.first() else { return };
                let mut v = first.value.clone();
                if t > first.t {
                    v = keys.last().map(|k| k.value.clone()).unwrap_or(v);
                    for w in keys.windows(2) {
                        let (a, b) = (&w[0], &w[1]);
                        if t < b.t {
                            let span = b.t - a.t;
                            let local = if span > 0.0 { ((t - a.t) / span).clamp(0.0, 1.0) } else { 1.0 };
                            v = a.value.lerp(&b.value, a.ease.apply(local));
                            break;
                        }
                    }
                }
                Self::write(out, target, &v);
            }
            Clip::Hold { .. } => {}
            Clip::Seq { clips } => {
                let mut starts = Vec::with_capacity(clips.len());
                let mut acc = 0.0;
                for c in clips {
                    starts.push(acc);
                    acc += c.duration();
                }
                let active = (0..clips.len()).find(|&i| t < starts[i] + clips[i].duration()).unwrap_or(clips.len().saturating_sub(1));
                // Pending clips hold their first frame (the nearest wins), finished ones their
                // last (the latest wins), the active one plays on top.
                for c in clips.iter().skip(active + 1).rev() {
                    c.eval(0.0, out);
                }
                for c in clips.iter().take(active) {
                    c.eval(c.duration(), out);
                }
                if let Some(c) = clips.get(active) {
                    c.eval(t - starts[active], out);
                }
            }
            Clip::Par { clips } => {
                for c in clips {
                    c.eval(t, out);
                }
            }
            Clip::Delay { by, clip } => clip.eval(t - nn(*by), out),
            Clip::Stretch { k, clip } => {
                let k = nn(*k);
                clip.eval(if k > 0.0 { t / k } else if t >= 0.0 { clip.duration() } else { 0.0 }, out)
            }
            Clip::Reverse { clip } => clip.eval(clip.duration() - t, out),
            Clip::Loop { n, clip } => {
                let d = clip.duration();
                let total = *n as f64 * d;
                let local = if t <= 0.0 || d <= 0.0 {
                    0.0
                } else if t >= total {
                    d
                } else {
                    m::fmod(t, d)
                };
                clip.eval(local, out)
            }
            Clip::Ease { easing, clip } => {
                let d = clip.duration();
                let local = if d > 0.0 { easing.apply(t / d) * d } else { t };
                clip.eval(local, out)
            }
            Clip::Stagger { by, clips } => {
                for (i, c) in clips.iter().enumerate() {
                    c.eval(t - nn(*by) * i as f64, out);
                }
            }
        }
    }
}

// ---- combinators -------------------------------------------------------------------------------

pub fn tween(target: KeyPath, from: PropValue, to: PropValue, duration: f64) -> Clip {
    Clip::Tween { target, from, to, duration, easing: Easing::Linear }
}
pub fn seq(clips: Vec<Clip>) -> Clip {
    Clip::Seq { clips }
}
pub fn par(clips: Vec<Clip>) -> Clip {
    Clip::Par { clips }
}
pub fn delay(by: f64, clip: Clip) -> Clip {
    Clip::Delay { by, clip: Box::new(clip) }
}
pub fn stretch(k: f64, clip: Clip) -> Clip {
    Clip::Stretch { k, clip: Box::new(clip) }
}
pub fn reverse(clip: Clip) -> Clip {
    Clip::Reverse { clip: Box::new(clip) }
}
pub fn loop_n(n: u32, clip: Clip) -> Clip {
    Clip::Loop { n, clip: Box::new(clip) }
}
pub fn ease(easing: Easing, clip: Clip) -> Clip {
    Clip::Ease { easing, clip: Box::new(clip) }
}
pub fn stagger(by: f64, clips: Vec<Clip>) -> Clip {
    Clip::Stagger { by, clips }
}
pub fn keyframes(target: KeyPath, keys: Vec<(f64, PropValue, Easing)>) -> Clip {
    Clip::Keyframes { target, keys: keys.into_iter().map(|(t, value, ease)| ClipKey { t, value, ease }).collect() }
}
pub fn hold(duration: f64) -> Clip {
    Clip::Hold { duration }
}

// ---- applying patches ----------------------------------------------------------------------------

/// Local bounds of a node's content (groups: the union of their children).
fn node_bounds(n: &Node) -> Rect {
    match &n.kind {
        NodeKind::Group { children } | NodeKind::View { children, .. } => children.iter().fold(Rect::empty(), |acc, c| {
            let b = node_bounds(c);
            if b.is_empty() {
                return acc;
            }
            let xf = c.common.transform;
            let pts = [Vec2::new(b.x, b.y), Vec2::new(b.x1(), b.y), Vec2::new(b.x1(), b.y1()), Vec2::new(b.x, b.y1())];
            pts.iter().fold(acc, |r, p| r.include(xf.apply(*p)))
        }),
        NodeKind::Instances(ins) => (0..ins.len()).fold(Rect::empty(), |r, i| r.union(&ins.geom(i).bounds())),
        _ => content_bounds(n).0,
    }
}

fn apply_node(n: &mut Node, p: &PropPatch, shaper: Option<&dyn TextShaper>) {
    if let Some(o) = p.opacity {
        n.common.opacity = (n.common.opacity * o).max(0.0);
    }
    if let Some(v) = p.visible {
        n.common.visible = v;
    }
    if p.scale.is_some() || p.rotate.is_some() {
        let c = {
            let b = node_bounds(n);
            if b.is_empty() {
                Vec2::ZERO
            } else {
                b.center()
            }
        };
        let (sx, sy) = p.scale.unwrap_or((1.0, 1.0));
        let local = Affine::translate(-c.x, -c.y).then(Affine::scale(sx, sy)).then(Affine::rotate(p.rotate.unwrap_or(0.0))).then(Affine::translate(c.x, c.y));
        n.common.transform = n.common.transform.mul(local);
    }
    if let Some(d) = p.translate {
        n.common.transform = n.common.transform.then(Affine::translate(d.x, d.y));
    }
    if let Some(x) = p.transform {
        n.common.transform = n.common.transform.then(x);
    }
    if let Some(t) = p.trim {
        n.common.trim = Some([t[0].clamp(0.0, 1.0), t[1].clamp(0.0, 1.0)]);
    }
    match &mut n.kind {
        NodeKind::Shape { fill, stroke, .. } => {
            if let Some(i) = &p.fill {
                *fill = Some(Paint::Solid(i.clone()));
            }
            if let (Some(w), Some(s)) = (p.stroke_width, stroke.as_mut()) {
                s.width = w.max(0.0);
            }
        }
        NodeKind::Text(t) => {
            if let Some(i) = &p.fill {
                t.style.ink = i.clone();
                for r in &mut t.runs {
                    r.ink = i.clone();
                }
            }
            if let (Some(v), Some(num)) = (p.number, t.number.as_mut()) {
                num.value = v;
                if let Some(s) = shaper {
                    s.shape(t);
                }
            }
        }
        NodeKind::Instances(ins) => {
            if let Some(i) = &p.fill {
                let n = ins.len();
                std::sync::Arc::make_mut(ins).fill = vec![i.clone(); n];
            }
        }
        _ => {}
    }
}

fn apply_instance(ins: &mut datars_scene::Instances, i: usize, p: &PropPatch) {
    if let Some(o) = p.opacity {
        let n = ins.len();
        if ins.opacity.len() < n {
            ins.opacity.resize(n, 1.0);
        }
        ins.opacity[i] = (ins.opacity[i] * o).max(0.0);
    }
    if let Some(d) = p.translate {
        ins.x[i] += d.x;
        ins.y[i] += d.y;
    }
    if let Some((sx, sy)) = p.scale {
        let n = ins.len();
        if ins.size.len() < n {
            ins.size.resize(n, 3.0);
        }
        ins.size[i] *= sx.min(sy);
        if let Some(w) = ins.w.as_mut() {
            w[i] *= sx;
        }
        if let Some(h) = ins.h.as_mut() {
            h[i] *= sy;
        }
    }
    if let Some(f) = &p.fill {
        let n = ins.len();
        if ins.fill.len() < n {
            let fills: Vec<Ink> = (0..n).map(|k| ins.fill_at(k)).collect();
            ins.fill = fills;
        }
        ins.fill[i] = f.clone();
    }
}

fn walk(n: &mut Node, path: &KeyPath, patches: &BTreeMap<&KeyPath, &PropPatch>, shaper: Option<&dyn TextShaper>) {
    let p = path.push(&n.key);
    if let NodeKind::Instances(ins) = &mut n.kind {
        // Instance patches: node path + instance key.
        let keys: BTreeMap<&Key, usize> = ins.keys.iter().enumerate().map(|(i, k)| (k, i)).collect();
        let hits: Vec<(usize, &PropPatch)> = patches
            .range::<&KeyPath, _>(&p..)
            .take_while(|(k, _)| k.0.len() >= p.0.len() && k.0[..p.0.len()] == p.0[..])
            .filter(|(k, _)| k.0.len() == p.0.len() + 1)
            .filter_map(|(k, v)| k.0.last().and_then(|key| keys.get(key)).map(|&i| (i, *v)))
            .collect();
        if !hits.is_empty() {
            let ins = std::sync::Arc::make_mut(ins);
            for (i, patch) in hits {
                apply_instance(ins, i, patch);
            }
        }
    }
    if let Some(patch) = patches.get(&p) {
        apply_node(n, patch, shaper);
    }
    if let Some(cs) = n.children_mut() {
        for c in cs {
            walk(c, &p, patches, shaper);
        }
    }
}

/// Apply patches to a scene (numbers set by patches are not re-shaped; see [`apply_shaped`]).
pub fn apply(scene: &Scene, patches: &[(KeyPath, PropPatch)]) -> Scene {
    apply_impl(scene, patches, None)
}

/// Apply patches, re-shaping text whose number changed.
pub fn apply_shaped(scene: &Scene, patches: &[(KeyPath, PropPatch)], shaper: &dyn TextShaper) -> Scene {
    apply_impl(scene, patches, Some(shaper))
}

fn apply_impl(scene: &Scene, patches: &[(KeyPath, PropPatch)], shaper: Option<&dyn TextShaper>) -> Scene {
    let mut merged: BTreeMap<&KeyPath, PropPatch> = BTreeMap::new();
    for (k, p) in patches {
        merged.entry(k).or_default().merge(p);
    }
    let refs: BTreeMap<&KeyPath, &PropPatch> = merged.iter().map(|(k, v)| (*k, v)).collect();
    let mut out = scene.clone();
    walk(&mut out.root, &KeyPath::default(), &refs, shaper);
    out
}

/// A colour tween helper for literal colours.
pub fn fill_tween(target: KeyPath, from: Color, to: Color, duration: f64) -> Clip {
    tween(target, PropValue::Fill(Ink::Color(from)), PropValue::Fill(Ink::Color(to)), duration)
}
