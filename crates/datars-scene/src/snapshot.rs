//! The text snapshot format (docs/13-testing.md): stable, sorted by tree order, rounded to 1/100 px,
//! readable in a code review and by an agent.

use crate::node::{Node, NodeKind, Proto};
use crate::{Geom, Paint, Scene};
use std::fmt::Write;

fn f(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0 + 0.0;
    format!("{r:.2}")
}

fn paint(p: &Paint) -> String {
    match p {
        Paint::Solid(i) => i.to_string(),
        Paint::Linear { stops, .. } => format!("linear[{}]", stops.iter().map(|s| s.ink.to_string()).collect::<Vec<_>>().join(",")),
        Paint::Radial { stops, .. } => format!("radial[{}]", stops.iter().map(|s| s.ink.to_string()).collect::<Vec<_>>().join(",")),
    }
}

fn geom(g: &Geom) -> String {
    match g {
        Geom::Rect { x, y, w, h, r } => {
            let mut s = format!("rect x={} y={} w={} h={}", f(*x), f(*y), f(*w), f(*h));
            if r.iter().any(|v| *v != 0.0) {
                s += &format!(" r={}", f(r[0]));
            }
            s
        }
        Geom::Ellipse { cx, cy, rx, ry } => format!("ellipse c=({}, {}) r=({}, {})", f(*cx), f(*cy), f(*rx), f(*ry)),
        Geom::Arc { cx, cy, r0, r1, a0, a1 } => format!("arc c=({}, {}) r={}..{} a={}..{}", f(*cx), f(*cy), f(*r0), f(*r1), f(*a0), f(*a1)),
        Geom::Segment { x1, y1, x2, y2 } => format!("segment ({}, {})→({}, {})", f(*x1), f(*y1), f(*x2), f(*y2)),
        Geom::Polyline { pts, closed, .. } => format!("polyline n={}{} first=({}, {})", pts.len(), if *closed { " closed" } else { "" }, f(pts.first().map_or(0.0, |p| p.x)), f(pts.first().map_or(0.0, |p| p.y))),
        Geom::Area { top, .. } => format!("area n={}", top.len()),
        Geom::Path { path } => {
            let b = path.bounds();
            format!("path els={} bounds=[{} {} {} {}]", path.els.len(), f(b.x), f(b.y), f(b.w), f(b.h))
        }
        Geom::Symbol { kind, x, y, size } => format!("symbol {kind:?} ({}, {}) size={}", f(*x), f(*y), f(*size)).to_lowercase(),
    }
}

impl Scene {
    pub fn snapshot(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "scene {}×{} background={}", f(self.width), f(self.height), self.background);
        node(&self.root, 1, &mut s);
        s
    }
}

impl Node {
    /// This node's line in [`Scene::snapshot`] (without indentation or children).
    pub fn snapshot_line(&self) -> String {
        line(self)
    }
}

fn node(n: &Node, depth: usize, s: &mut String) {
    let pad = "  ".repeat(depth);
    let _ = writeln!(s, "{pad}{}", line(n));
    if let NodeKind::Instances(i) = &n.kind {
        for k in 0..i.len().min(3) {
            let _ = writeln!(s, "{pad}  · {} ({}, {}) fill={}", i.keys[k], f(i.x[k]), f(i.y[k]), i.fill_at(k));
        }
        if i.len() > 3 {
            let _ = writeln!(s, "{pad}  · … {} more", i.len() - 3);
        }
    }
    for ch in n.children() {
        node(ch, depth + 1, s);
    }
}

fn line(n: &Node) -> String {
    let mut line = String::new();
    match &n.kind {
        NodeKind::Group { .. } => line += &format!("group {}", n.key),
        NodeKind::View { viewport, camera, .. } => {
            line += &format!("view {} [{} {} {} {}]", n.key, f(viewport.x), f(viewport.y), f(viewport.w), f(viewport.h));
            if let Some(c) = camera {
                line += &format!(" camera=({}, {}) zoom={}", f(c.x), f(c.y), f(c.zoom));
            }
        }
        NodeKind::Shape { geom: g, fill, stroke, .. } => {
            line += &format!("{} {}", n.key, geom(g));
            if let Some(p) = fill {
                line += &format!(" fill={}", paint(p));
            }
            if let Some(st) = stroke {
                line += &format!(" stroke={}/{}", paint(&st.paint), f(st.width));
            }
        }
        NodeKind::Text(t) => {
            line += &format!("{} text {:?} at ({}, {}) size={} ink={}", n.key, t.text, f(t.origin.x), f(t.origin.y), f(t.style.size), t.style.ink);
            if t.offset != datars_math::Vec2::ZERO {
                line += &format!(" offset=({}, {})", f(t.offset.x), f(t.offset.y));
            }
        }
        NodeKind::Image { asset, rect } => line += &format!("{} image {asset} [{} {} {} {}]", n.key, f(rect.x), f(rect.y), f(rect.w), f(rect.h)),
        NodeKind::Instances(i) => {
            let proto = match &i.proto {
                Proto::Symbol { symbol } => format!("{symbol:?}").to_lowercase(),
                Proto::Rect => "rect".into(),
            };
            line += &format!("{} instances {proto} ×{}", n.key, i.len());
        }
    }
    let c = &n.common;
    if c.opacity != 1.0 {
        line += &format!(" opacity={}", f(c.opacity));
    }
    if !c.visible {
        line += " hidden";
    }
    if !c.transform.is_identity() {
        let t = c.transform.0;
        line += &format!(" transform=[{} {} {} {} {} {}]", f(t[0]), f(t[1]), f(t[2]), f(t[3]), f(t[4]), f(t[5]));
    }
    if let Some(t) = c.trim {
        line += &format!(" trim={}..{}", f(t[0]), f(t[1]));
    }
    if let Some(sem) = &n.semantics {
        line += &format!(" role={}", serde_json::to_string(&sem.role).unwrap_or_default().trim_matches('"'));
        if !sem.label.is_empty() {
            line += &format!(" {:?}", sem.label);
        }
    }
    line
}
