//! Local bounds of resolved nodes (for `auto` layout sizes, camera fitting and culling).

use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Node, NodeKind, TextNode};

pub fn node_bounds(n: &Node) -> Rect {
    bounds_xf(n, Affine::IDENTITY)
}

fn bounds_xf(n: &Node, parent: Affine) -> Rect {
    if !n.common.visible {
        return Rect::empty();
    }
    let xf = parent.mul(n.common.transform);
    if n.common.pin {
        // Screen-size content has no extent in content units: only its origin counts (a camera
        // fitting a map doesn't make room for a callout's text).
        let o = xf.apply(Vec2::ZERO);
        return Rect::new(o.x, o.y, 0.0, 0.0);
    }
    kind_bounds(n, xf)
}

/// Where a node is drawn under `parent` (its parent's transform to canvas px): [`node_bounds`],
/// except that pinned content counts at the screen size it's drawn at around its anchor (for
/// `explain` and editors; camera fits and layout measure pinned content by its anchor only).
pub fn drawn_bounds(n: &Node, parent: Affine) -> Rect {
    if !n.common.visible {
        return Rect::empty();
    }
    if n.common.pin {
        return kind_bounds(n, n.common.placed(parent));
    }
    bounds_xf(n, parent)
}

/// Bounds of a node's content drawn through `xf` (its own transform included).
fn kind_bounds(n: &Node, xf: Affine) -> Rect {
    let r = match &n.kind {
        NodeKind::Group { children } => children.iter().fold(Rect::empty(), |acc, c| acc.union(&bounds_xf(c, xf))),
        NodeKind::View { viewport, .. } => transform_rect(*viewport, &xf),
        NodeKind::Shape { geom, stroke, .. } => {
            let mut b = geom.bounds();
            if let Some(s) = stroke {
                b = b.inset(-s.width / 2.0);
            }
            transform_rect(b, &xf)
        }
        NodeKind::Text(t) => text_rect(t, &xf),
        NodeKind::Image { rect, .. } => transform_rect(*rect, &xf),
        NodeKind::Instances(i) => {
            let mut b = Rect::empty();
            for k in 0..i.len() {
                b = b.union(&transform_rect(i.geom(k).bounds(), &xf));
            }
            b
        }
    };
    r
}

/// Where a text is drawn through `xf` (every transform above it, its own included): the laid-out
/// box at the transformed origin plus its offset, turned by its rotation around that point — a
/// rotated axis label takes the room it covers, not the room it would take lying flat.
pub fn text_rect(t: &TextNode, xf: &Affine) -> Rect {
    let o = xf.apply(t.origin) + t.offset;
    let b = t.bounds;
    if t.rotate == 0.0 {
        return Rect::new(o.x + b.x, o.y + b.y, b.w, b.h);
    }
    text_quad(t, xf).iter().fold(Rect::empty(), |acc, p| acc.include(*p))
}

/// The four corners of a text as drawn (see [`text_rect`]), turned with it: two slanted labels
/// side by side overlap only if these do, whatever their boxes' extents.
pub fn text_quad(t: &TextNode, xf: &Affine) -> [Vec2; 4] {
    quad(t, xf, t.bounds)
}

/// [`text_quad`] without the line spacing above and below the glyphs: what two labels stacked a
/// line apart must not share (their line boxes may touch — that's spacing, not a collision).
pub fn text_core(t: &TextNode, xf: &Affine) -> [Vec2; 4] {
    let lead = (t.bounds.h * 0.12).max(0.0);
    quad(t, xf, Rect::new(t.bounds.x, t.bounds.y + lead, t.bounds.w, (t.bounds.h - 2.0 * lead).max(0.0)))
}

fn quad(t: &TextNode, xf: &Affine, b: Rect) -> [Vec2; 4] {
    let o = xf.apply(t.origin) + t.offset;
    let (s, c) = if t.rotate == 0.0 { (0.0, 1.0) } else { datars_math::m::sin_cos(t.rotate) };
    [(b.x, b.y), (b.x1(), b.y), (b.x1(), b.y1()), (b.x, b.y1())].map(|(x, y)| Vec2::new(o.x + x * c - y * s, o.y + x * s + y * c))
}

/// Do two convex quads come within `gap` of each other? (Separating axes: the quads' edge normals.)
pub fn quads_overlap(a: &[Vec2; 4], b: &[Vec2; 4], gap: f64) -> bool {
    let span = |q: &[Vec2; 4], n: Vec2| q.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| (lo.min(p.dot(n)), hi.max(p.dot(n))));
    for q in [a, b] {
        for i in 0..2 {
            let e = q[i + 1] - q[i];
            let len = e.len();
            if len < 1e-9 {
                continue;
            }
            let n = Vec2::new(-e.y / len, e.x / len);
            let ((alo, ahi), (blo, bhi)) = (span(a, n), span(b, n));
            if ahi + gap <= blo || bhi + gap <= alo {
                return false;
            }
        }
    }
    true
}

pub fn transform_rect(r: Rect, xf: &Affine) -> Rect {
    if r.is_empty() {
        return r;
    }
    let pts = [Vec2::new(r.x, r.y), Vec2::new(r.x1(), r.y), Vec2::new(r.x, r.y1()), Vec2::new(r.x1(), r.y1())];
    pts.iter().fold(Rect::empty(), |acc, p| acc.include(xf.apply(*p)))
}

/// The extent to frame when a camera fits this node: for multi-part paths (a country with
/// overseas territories), the largest part plus the parts near it — France fits as metropolitan
/// France, not France-to-French-Guiana; Norway keeps Svalbard. "Near" means within one diagonal
/// of the largest part's box. Everything else fits by its ordinary bounds.
pub fn fit_bounds(n: &Node) -> Rect {
    let NodeKind::Shape { geom: datars_scene::Geom::Path { path }, .. } = &n.kind else {
        return node_bounds(n);
    };
    let parts = part_bounds(path);
    if parts.len() < 2 {
        return node_bounds(n);
    }
    let main = parts.iter().copied().fold(parts[0], |best, r| if r.w * r.h > best.w * best.h { r } else { best });
    let reach = (main.w * main.w + main.h * main.h).sqrt();
    let near = parts.iter().filter(|r| gap(&main, r) <= reach).fold(Rect::empty(), |acc, r| acc.union(r));
    transform_rect(near, &n.common.transform)
}

fn part_bounds(p: &datars_math::PathData) -> Vec<Rect> {
    use datars_math::PathEl;
    let mut out = Vec::new();
    let mut cur = Rect::empty();
    for e in &p.els {
        match *e {
            PathEl::Move { p } => {
                if !cur.is_empty() {
                    out.push(cur);
                }
                cur = Rect::empty().include(p);
            }
            PathEl::Line { p } => cur = cur.include(p),
            PathEl::Quad { c, p } => cur = cur.include(c).include(p),
            PathEl::Cubic { c1, c2, p } => cur = cur.include(c1).include(c2).include(p),
            _ => {}
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Distance between two boxes (0 when they overlap).
fn gap(a: &Rect, b: &Rect) -> f64 {
    let dx = (b.x - a.x1()).max(a.x - b.x1()).max(0.0);
    let dy = (b.y - a.y1()).max(a.y - b.y1()).max(0.0);
    (dx * dx + dy * dy).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::PathData;
    use datars_scene::{Geom, Key};

    fn square(p: &mut PathData, x: f64, y: f64, s: f64) {
        p.move_to(Vec2::new(x, y));
        p.line_to(Vec2::new(x + s, y));
        p.line_to(Vec2::new(x + s, y + s));
        p.line_to(Vec2::new(x, y + s));
        p.close();
    }

    #[test]
    fn far_territories_do_not_stretch_the_fit() {
        let mut p = PathData::new();
        square(&mut p, 100.0, 100.0, 50.0); // mainland
        square(&mut p, 160.0, 90.0, 10.0); // a nearby island: kept
        square(&mut p, -400.0, 400.0, 20.0); // an overseas territory: dropped
        let n = Node::shape(Key::name("FRA"), Geom::Path { path: std::sync::Arc::new(p) });
        let r = fit_bounds(&n);
        assert_eq!((r.x, r.y, r.x1(), r.y1()), (100.0, 90.0, 170.0, 150.0));
        assert!(node_bounds(&n).x < -300.0);
    }
}
