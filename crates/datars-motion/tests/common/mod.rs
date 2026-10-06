//! Shared helpers for datars-motion integration tests.
#![allow(dead_code)]

use datars_math::{m, PathData, Rect, Rng, Vec2};
use datars_motion::{PlanCx, TextShaper};
use datars_scene::text::TextStyle;
use datars_scene::*;
use datars_theme::{resolve, Ink, Mode, ResolvedTheme, ThemeSet};
use std::cell::Cell;
use std::sync::Arc;

pub fn theme() -> ResolvedTheme {
    let set = ThemeSet::with_builtins();
    resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]).0
}

pub fn cx() -> PlanCx {
    PlanCx::new(theme())
}

pub fn key(s: &str) -> Key {
    Key::name(s)
}

pub fn path(parts: &[&str]) -> KeyPath {
    KeyPath(parts.iter().map(|p| Key::name(p)).collect())
}

pub fn scene(children: Vec<Node>) -> Scene {
    Scene::new(400.0, 300.0, Node::group(key("root"), children))
}

pub fn group(k: &str, children: Vec<Node>) -> Node {
    Node::group(key(k), children)
}

pub fn rect(k: &str, x: f64, y: f64, w: f64, h: f64) -> Node {
    Node::shape(key(k), Geom::rect(x, y, w, h)).fill(Paint::Solid(Ink::token("accent")))
}

pub fn circle(k: &str, cx: f64, cy: f64, r: f64) -> Node {
    Node::shape(key(k), Geom::circle(cx, cy, r)).fill(Paint::Solid(Ink::token("accent")))
}

/// Walk a scene with key paths.
pub fn walk<'a>(s: &'a Scene, f: &mut dyn FnMut(&KeyPath, &'a Node)) {
    s.root.walk(&KeyPath::default(), f);
}

/// All nodes at `p` (a crossfade may have two).
pub fn find_all<'a>(s: &'a Scene, p: &KeyPath) -> Vec<&'a Node> {
    let mut v = Vec::new();
    walk(s, &mut |q, n| {
        if q == p {
            v.push(n);
        }
    });
    v
}

pub fn find<'a>(s: &'a Scene, p: &KeyPath) -> Option<&'a Node> {
    find_all(s, p).into_iter().last()
}

pub fn geom(n: &Node) -> &Geom {
    match &n.kind {
        NodeKind::Shape { geom, .. } => geom,
        k => panic!("not a shape: {k:?}"),
    }
}

/// Signed area of the first closed ring of a geometry (positive = clockwise on screen).
pub fn main_area(g: &Geom) -> f64 {
    let f = g.to_path().flatten(0.05);
    f.first().map_or(0.0, |(pts, _)| datars_math::path::signed_area(pts))
}

/// A text shaper for tests: formats `number` with 0 decimals and makes one glyph per char.
#[derive(Default)]
pub struct FakeShaper {
    pub calls: Cell<usize>,
}

impl TextShaper for FakeShaper {
    fn shape(&self, node: &mut TextNode) {
        self.calls.set(self.calls.get() + 1);
        if let Some(n) = &node.number {
            node.text = format!("{:.0}", n.value);
        }
        let glyphs: Vec<GlyphPos> = node.text.chars().enumerate().map(|(i, c)| GlyphPos { id: c as u16, x: i as f32 * 7.0, y: 0.0 }).collect();
        node.bounds = Rect::new(0.0, -node.style.size, glyphs.len() as f64 * 7.0, node.style.size);
        node.runs = vec![TextRun { font: Arc::from("Test-400"), size: node.style.size, ink: node.style.ink.clone(), glyphs: glyphs.into() }];
    }
}

pub fn number_text(k: &str, v: f64, at: Vec2) -> Node {
    let mut t = TextNode::new(format!("{v:.0}"), at, TextStyle::default());
    t.number = Some(NumberText { value: v, format: Arc::from(".0f"), locale: Arc::from("en") });
    FakeShaper::default().shape(&mut t);
    Node::text(key(k), t)
}

pub fn label(k: &str, s: &str, at: Vec2) -> Node {
    let mut t = TextNode::new(s, at, TextStyle::default());
    FakeShaper::default().shape(&mut t);
    t.number = None;
    Node::text(key(k), t)
}

pub fn instances(k: &str, keys: Vec<Key>, xs: Vec<f64>, ys: Vec<f64>, r: f64) -> Node {
    let n = keys.len();
    Node::new(
        key(k),
        NodeKind::Instances(std::sync::Arc::new(Instances {
            proto: Proto::Symbol { symbol: SymbolKind::Circle },
            keys,
            x: xs,
            y: ys,
            size: vec![r; n],
            w: None,
            h: None,
            fill: vec![Ink::palette("categorical", 0); n],
            opacity: vec![1.0; n],
            stroke: None,
            screen_size: false,
            labels: None,
            line_reach: None,
        })),
    )
}

// ---- finiteness ------------------------------------------------------------------------------

fn fin(v: f64, what: &str) {
    assert!(v.is_finite(), "non-finite {what}: {v}");
}

pub fn assert_finite_geom(g: &Geom) {
    match g {
        Geom::Rect { x, y, w, h, r } => {
            for v in [*x, *y, *w, *h, r[0], r[1], r[2], r[3]] {
                fin(v, "rect");
            }
        }
        Geom::Ellipse { cx, cy, rx, ry } => [*cx, *cy, *rx, *ry].iter().for_each(|v| fin(*v, "ellipse")),
        Geom::Arc { cx, cy, r0, r1, a0, a1 } => [*cx, *cy, *r0, *r1, *a0, *a1].iter().for_each(|v| fin(*v, "arc")),
        Geom::Segment { x1, y1, x2, y2 } => [*x1, *y1, *x2, *y2].iter().for_each(|v| fin(*v, "segment")),
        Geom::Polyline { pts, .. } => pts.iter().for_each(|p| assert!(p.is_finite(), "polyline")),
        Geom::Area { top, base, .. } => top.iter().chain(base.iter()).for_each(|p| assert!(p.is_finite(), "area")),
        Geom::Path { path } => {
            for e in &path.els {
                match e {
                    datars_math::PathEl::Move { p } | datars_math::PathEl::Line { p } => assert!(p.is_finite(), "path"),
                    datars_math::PathEl::Quad { c, p } => assert!(c.is_finite() && p.is_finite(), "path"),
                    datars_math::PathEl::Cubic { c1, c2, p } => assert!(c1.is_finite() && c2.is_finite() && p.is_finite(), "path"),
                    datars_math::PathEl::Close => {}
                }
            }
        }
        Geom::Symbol { x, y, size, .. } => [*x, *y, *size].iter().for_each(|v| fin(*v, "symbol")),
    }
}

pub fn assert_finite(s: &Scene) {
    fin(s.width, "width");
    fin(s.height, "height");
    walk(s, &mut |p, n| {
        for v in n.common.transform.0 {
            assert!(v.is_finite(), "transform at {p}: {:?}", n.common.transform);
        }
        assert!(n.common.opacity.is_finite() && n.common.opacity >= 0.0 && n.common.opacity <= 1.0 + 1e-12, "opacity at {p}: {}", n.common.opacity);
        if let Some(t) = n.common.trim {
            fin(t[0], "trim");
            fin(t[1], "trim");
        }
        match &n.kind {
            NodeKind::Shape { geom, stroke, .. } => {
                assert_finite_geom(geom);
                if let Some(s) = stroke {
                    fin(s.width, "stroke width");
                }
            }
            NodeKind::Text(t) => {
                assert!(t.origin.is_finite(), "text origin at {p}");
                fin(t.style.size, "text size");
                fin(t.rotate, "rotate");
                if let Some(nb) = &t.number {
                    fin(nb.value, "number");
                }
            }
            NodeKind::Image { rect, .. } => [rect.x, rect.y, rect.w, rect.h].iter().for_each(|v| fin(*v, "image")),
            NodeKind::Instances(i) => {
                for v in i.x.iter().chain(&i.y).chain(&i.size).chain(&i.opacity) {
                    fin(*v, "instance column");
                }
                for v in i.w.iter().flatten().chain(i.h.iter().flatten()) {
                    fin(*v, "instance w/h");
                }
                assert!(i.opacity.iter().all(|o| (0.0..=1.0 + 1e-12).contains(o)), "instance opacity at {p}");
            }
            NodeKind::View { viewport, camera, .. } => {
                [viewport.x, viewport.y, viewport.w, viewport.h].iter().for_each(|v| fin(*v, "viewport"));
                if let Some(c) = camera {
                    [c.x, c.y, c.zoom, c.rotation].iter().for_each(|v| fin(*v, "camera"));
                }
            }
            NodeKind::Group { .. } => {}
        }
    });
}

// ---- seeded random scenes -------------------------------------------------------------------

fn random_geom(rng: &mut Rng, cx: f64, cy: f64) -> Geom {
    let s = rng.range(6.0, 40.0);
    match rng.below(8) {
        0 | 1 => Geom::rect(cx - s / 2.0, cy - s, s, rng.range(4.0, 80.0)),
        2 => Geom::circle(cx, cy, s / 2.0),
        3 => {
            let a0 = rng.range(0.0, 5.0);
            Geom::Arc { cx, cy, r0: rng.range(0.0, 10.0), r1: rng.range(12.0, 60.0), a0, a1: a0 + rng.range(0.05, 1.5) }
        }
        4 => Geom::Symbol { kind: SymbolKind::Diamond, x: cx, y: cy, size: s / 3.0 },
        5 => {
            let n = 3 + rng.below(6) as usize;
            let pts: Vec<Vec2> = (0..n).map(|i| Vec2::polar(Vec2::new(cx, cy), s * rng.range(0.5, 1.0), m::TAU * i as f64 / n as f64)).collect();
            // Sometimes wound anticlockwise.
            let pts: Vec<Vec2> = if rng.below(2) == 0 { pts.into_iter().rev().collect() } else { pts };
            Geom::path(PathData::polygon(&pts))
        }
        6 => {
            let n = 2 + rng.below(8) as usize;
            Geom::polyline((0..n).map(|i| Vec2::new(cx + i as f64 * 8.0, cy + rng.range(-20.0, 20.0))).collect())
        }
        _ => {
            let n = 2 + rng.below(6) as usize;
            let top: Vec<Vec2> = (0..n).map(|i| Vec2::new(cx + i as f64 * 10.0, cy - rng.range(5.0, 40.0))).collect();
            let base: Vec<Vec2> = (0..n).map(|i| Vec2::new(cx + i as f64 * 10.0, cy + 10.0)).collect();
            Geom::Area { top: top.into(), base: base.into(), curve: Curve::Linear }
        }
    }
}

fn random_ink(rng: &mut Rng) -> Ink {
    match rng.below(3) {
        0 => Ink::token("accent"),
        1 => Ink::palette("categorical", rng.below(10) as u32),
        _ => Ink::Color(datars_color::Color::rgb8(rng.below(256) as u8, rng.below(256) as u8, rng.below(256) as u8)),
    }
}

/// A random scene drawn from a shared universe of keys: `variant` picks which elements exist,
/// their kinds, positions, containers and styles.
pub fn random_scene(seed: u64, variant: u64) -> Scene {
    let mut rng = Rng::new(seed.wrapping_mul(1000).wrapping_add(variant));
    let groups = ["g0", "g1", "g2"];
    let mut kids: Vec<Vec<Node>> = vec![Vec::new(); groups.len()];
    for i in 0..24 {
        if rng.next_f64() < 0.3 {
            continue;
        }
        let k = format!("e{i}");
        let (cx, cy) = (rng.range(20.0, 380.0), rng.range(20.0, 280.0));
        let mut n = match rng.below(10) {
            0 => number_text(&k, rng.range(0.0, 1000.0).round(), Vec2::new(cx, cy)),
            1 => label(&k, if rng.below(2) == 0 { "alpha" } else { "beta" }, Vec2::new(cx, cy)),
            _ => {
                let mut n = Node::shape(key(&k), random_geom(&mut rng, cx, cy)).fill(Paint::Solid(random_ink(&mut rng)));
                if rng.below(4) == 0 {
                    n = n.stroke(Stroke::new(random_ink(&mut rng), rng.range(0.5, 3.0)));
                }
                n
            }
        };
        n.common.opacity = if rng.below(4) == 0 { rng.range(0.3, 1.0) } else { 1.0 };
        if rng.below(6) == 0 {
            n.common.transform = datars_math::Affine::rotate(rng.range(-0.5, 0.5));
        }
        n.semantics = Some(Semantics { role: if rng.below(3) == 0 { Role::Label } else { Role::Datum }, value: Some(rng.range(0.0, 100.0)), ..Default::default() });
        kids[rng.below(groups.len() as u64) as usize].push(n);
    }
    let mut children: Vec<Node> = Vec::new();
    for (g, ks) in groups.iter().zip(kids) {
        let mut gn = group(g, ks);
        if rng.below(3) == 0 {
            gn.common.transform = datars_math::Affine::translate(rng.range(-30.0, 30.0), rng.range(-30.0, 30.0));
        }
        if rng.below(4) == 0 {
            gn.common.opacity = rng.range(0.5, 1.0);
        }
        children.push(gn);
    }
    // Instances: a subset of a shared key universe.
    let mut keys = Vec::new();
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for i in 0..30 {
        if rng.next_f64() < 0.7 {
            keys.push(Key::one(i as i64));
            xs.push(rng.range(0.0, 400.0));
            ys.push(rng.range(0.0, 300.0));
        }
    }
    let mut dots = instances("dots", keys, xs, ys, rng.range(2.0, 5.0));
    if rng.below(3) == 0 {
        if let NodeKind::Instances(i) = &mut dots.kind {
            let i = std::sync::Arc::make_mut(i);
            i.proto = Proto::Rect;
            i.w = Some(vec![4.0; i.len()]);
            i.h = Some(vec![6.0; i.len()]);
        }
    }
    children.push(dots);
    // A view with a camera, sometimes.
    if rng.below(2) == 0 {
        let cam = Camera { x: rng.range(0.0, 400.0), y: rng.range(0.0, 300.0), zoom: m::exp2(rng.range(-1.0, 3.0)), rotation: 0.0 };
        let mut v = Node::new(
            key("map"),
            NodeKind::View { viewport: Rect::new(0.0, 0.0, 200.0, 150.0), camera: Some(cam), clip: true, children: vec![circle("m0", 100.0, 100.0, rng.range(5.0, 30.0))] },
        );
        v.common.opacity = 1.0;
        children.push(v);
    }
    Scene::new(400.0, 300.0, Node::group(key("root"), children))
}
