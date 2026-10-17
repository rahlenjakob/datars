//! Where the data is drawn, for text that should keep off it: the strokes and filled areas of the
//! marks under data semantics (`datum`, `series`, `region`), in canvas coordinates. A label placed
//! over a line is crossed out by it; over a filled mark it needs its own contrast. Texts with
//! alternative places (`TText::alternatives`) take the first clear one; legibility checks count
//! the texts that stay on data without a halo.

use datars_math::{Affine, Rect, Vec2};
use datars_scene::{Node, NodeKind, Paint, Role};
use datars_theme::Ink;

/// The most outline points collected (a map of counties is mostly not where labels go).
const MAX_POINTS: usize = 200_000;

#[derive(Clone, Debug, Default)]
pub struct Obstacles {
    /// Stroked segments, their half widths and when they're drawn (painter's order).
    segs: Vec<(Vec2, Vec2, f64, usize)>,
    /// Filled outlines (closed polygons), their bounds and when they're drawn.
    fills: Vec<(Vec<Vec2>, Rect, usize)>,
    /// Opaque boxes drawn over the data (a legend's or a card's backdrop): the data under them
    /// can't reach a text drawn on them.
    covers: Vec<(Rect, usize)>,
    order: usize,
    points: usize,
}

/// How much of a box the data covers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cover {
    /// A stroke crosses it.
    pub stroked: bool,
    /// The share of it under filled marks (0–1, sampled).
    pub filled: f64,
}

impl Cover {
    /// Text there would be hard to read: a line through its letters, or straddling a filled
    /// mark's edge (part on the mark, part off: one ink can't suit both). Text wholly inside a
    /// mark is that mark's own label, inked for it.
    pub fn hurts(&self) -> bool {
        self.stroked || (self.filled > 0.25 && self.filled < 0.75)
    }
}

fn shown(i: &Ink) -> bool {
    match i {
        Ink::Color(c) => c.a > 0.05,
        Ink::Token { alpha, .. } | Ink::Palette { alpha, .. } | Ink::Ramp { alpha, .. } | Ink::On { alpha, .. } => *alpha > 0.05,
    }
}

fn paint_shown(p: &Paint) -> bool {
    match p {
        Paint::Solid(i) => shown(i),
        _ => true,
    }
}

impl Obstacles {
    /// The data marks under `roots` (as resolved: transforms, views and cameras applied).
    pub fn collect(roots: &[Node]) -> Obstacles {
        let mut o = Obstacles::default();
        for n in roots {
            o.walk(n, Affine::IDENTITY, 1.0, false);
        }
        o
    }

    fn walk(&mut self, n: &Node, parent: Affine, opacity: f64, in_data: bool) {
        let opacity = opacity * n.common.opacity;
        if !n.common.visible || opacity < 0.2 || self.points > MAX_POINTS {
            return;
        }
        let in_data = match &n.semantics {
            Some(s) if s.role == Role::Decoration || s.role == Role::Annotation || s.role == Role::Control => false,
            Some(s) => in_data || matches!(s.role, Role::Datum | Role::Series | Role::Region),
            None => in_data,
        };
        let xf = n.common.placed(parent);
        self.order += 1;
        let order = self.order;
        match &n.kind {
            // An opaque box over what's under it (a backdrop): not data, but it hides data.
            NodeKind::Shape { geom: datars_scene::Geom::Rect { x, y, w, h, .. }, fill: Some(f), .. } if !in_data && opacity >= 0.9 && match f {
                Paint::Solid(Ink::Token { name, alpha }) => *alpha >= 0.9 && matches!(name.as_ref(), "paper" | "surface" | "card"),
                Paint::Solid(Ink::Color(c)) => c.a >= 0.9,
                _ => false,
            } => {
                let r = crate::bounds::transform_rect(Rect::new(*x, *y, *w, *h), &xf);
                self.covers.push((r, order));
            }
            NodeKind::Shape { geom, fill, stroke, .. } if in_data => {
                let stroked = stroke.as_ref().filter(|s| s.width > 0.0 && paint_shown(&s.paint));
                let filled = fill.as_ref().is_some_and(paint_shown) && geom.is_closed();
                if stroked.is_none() && !filled {
                    return;
                }
                let scale = (xf.apply_vec(Vec2::new(1.0, 0.0)).len() + xf.apply_vec(Vec2::new(0.0, 1.0)).len()) / 2.0;
                for (pts, closed) in geom.flatten(0.75) {
                    let pts: Vec<Vec2> = pts.iter().map(|p| xf.apply(*p)).collect();
                    self.points += pts.len();
                    if let Some(s) = stroked {
                        let half = if s.non_scaling { s.width / 2.0 } else { s.width * scale / 2.0 };
                        for w in pts.windows(2) {
                            self.segs.push((w[0], w[1], half, order));
                        }
                        if closed && pts.len() > 2 {
                            self.segs.push((pts[pts.len() - 1], pts[0], half, order));
                        }
                    }
                    if filled && pts.len() > 2 {
                        let b = pts.iter().fold(Rect::empty(), |acc, p| acc.include(*p));
                        self.fills.push((pts, b, order));
                    }
                }
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let inner = xf.mul(camera.map(|c| c.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y)));
                children.iter().for_each(|c| self.walk(c, inner, opacity, in_data));
            }
            _ => n.children().iter().for_each(|c| self.walk(c, xf, opacity, in_data)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.segs.is_empty() && self.fills.is_empty()
    }

    /// How much of `r` (canvas px) the data covers.
    pub fn cover(&self, r: Rect) -> Cover {
        self.cover_quad(&[Vec2::new(r.x, r.y), Vec2::new(r.x1(), r.y), Vec2::new(r.x1(), r.y1()), Vec2::new(r.x, r.y1())])
    }

    /// How much of a text's outline (four corners in order, turned with it) the data covers.
    pub fn cover_quad(&self, q: &[Vec2; 4]) -> Cover {
        let bounds = q.iter().fold(Rect::empty(), |acc, p| acc.include(*p));
        let poly = q.to_vec();
        // Data drawn before an opaque box that holds the whole text doesn't reach it.
        let hidden = |at: usize| self.covers.iter().any(|(c, o)| *o > at && c.inset(-0.5).contains(Vec2::new(bounds.x, bounds.y)) && c.inset(-0.5).contains(Vec2::new(bounds.x1(), bounds.y1())));
        let stroked = self.segs.iter().any(|(a, b, half, at)| {
            if hidden(*at) {
                return false;
            }
            // (Quick reject by bounds, then: an end inside, or crossing an edge — of the outline
            // grown by the stroke's half width, approximated by its bounds when turned.)
            if !segment_hits(*a, *b, &bounds.inset(-half)) {
                return false;
            }
            let turned = (q[0].y - q[1].y).abs() > 1e-6;
            !turned || point_in(&poly, *a) || point_in(&poly, *b) || (0..4).any(|i| crosses(*a, *b, q[i], q[(i + 1) % 4]))
        });
        let mut inside = 0;
        let (nx, ny) = (6, 3);
        let near: Vec<&(Vec<Vec2>, Rect, usize)> = self.fills.iter().filter(|(_, b, at)| b.intersects(&bounds) && !hidden(*at)).collect();
        if !near.is_empty() {
            for i in 0..nx {
                for j in 0..ny {
                    let (u, v) = ((i as f64 + 0.5) / nx as f64, (j as f64 + 0.5) / ny as f64);
                    // A point of the outline: along its top and bottom edges, then between them.
                    let top = q[0] + (q[1] - q[0]) * u;
                    let bottom = q[3] + (q[2] - q[3]) * u;
                    let p = top + (bottom - top) * v;
                    if near.iter().any(|(poly, b, _)| b.contains(p) && point_in(poly, p)) {
                        inside += 1;
                    }
                }
            }
        }
        Cover { stroked, filled: inside as f64 / (nx * ny) as f64 }
    }
}

/// Does the segment a–b pass through `r`?
fn segment_hits(a: Vec2, b: Vec2, r: &Rect) -> bool {
    if r.contains(a) || r.contains(b) {
        return true;
    }
    let lo = Vec2::new(a.x.min(b.x), a.y.min(b.y));
    let hi = Vec2::new(a.x.max(b.x), a.y.max(b.y));
    if hi.x < r.x || lo.x > r.x1() || hi.y < r.y || lo.y > r.y1() {
        return false;
    }
    let corners = [Vec2::new(r.x, r.y), Vec2::new(r.x1(), r.y), Vec2::new(r.x1(), r.y1()), Vec2::new(r.x, r.y1())];
    (0..4).any(|i| crosses(a, b, corners[i], corners[(i + 1) % 4]))
}

fn crosses(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let side = |p: Vec2, q: Vec2, r: Vec2| (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
    let (d1, d2, d3, d4) = (side(c, d, a), side(c, d, b), side(a, b, c), side(a, b, d));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

fn point_in(poly: &[Vec2], p: Vec2) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_through_a_box_and_a_bar_under_it_are_found() {
        let a = Vec2::new(0.0, 50.0);
        let b = Vec2::new(100.0, 50.0);
        assert!(segment_hits(a, b, &Rect::new(40.0, 40.0, 20.0, 20.0)));
        assert!(!segment_hits(a, b, &Rect::new(40.0, 60.0, 20.0, 20.0)));
        assert!(segment_hits(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0), &Rect::new(40.0, 40.0, 5.0, 5.0)), "diagonally through, no end inside");
        let square = vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 10.0), Vec2::new(0.0, 10.0)];
        assert!(point_in(&square, Vec2::new(5.0, 5.0)) && !point_in(&square, Vec2::new(15.0, 5.0)));
    }
}
