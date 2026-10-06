//! Greedy label placement with priorities, candidate positions and collision avoidance.

use crate::util::rect_ok;
use datars_math::{total_cmp, Rect, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A label to place.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LabelBox {
    /// The label's box relative to its placement point (e.g. `Rect::new(-w / 2, -h, w, h)` for a
    /// label centred above the point). Add padding here to keep labels apart.
    pub rect: Rect,
    /// Higher is placed first (and so wins conflicts).
    pub priority: f64,
    /// The point being labelled.
    pub anchor: Vec2,
    /// Offsets from `anchor` to try, in order of preference. Empty means the single offset (0, 0).
    pub candidates: Vec<Vec2>,
}

/// Chooses a position for each label, or hides it: returns, per label (indexed like the input),
/// the index of the chosen candidate offset, or `None` when no candidate fits.
///
/// Greedy by priority (descending; ties in input order; NaN last): each label takes its first candidate
/// whose box lies inside `bounds` and overlaps neither an already placed label nor any of the
/// `obstacles` (e.g. marks or boxes around other anchors). Boxes that merely touch don't overlap.
/// The result never has two chosen boxes overlapping, and a label is only hidden when every one of
/// its candidates collides with something placed before it (i.e. with higher priority) or leaves
/// the bounds.
///
/// A uniform grid of placed boxes keeps each test local, so thousands of labels place in
/// milliseconds. `bounds` with a non-finite or negative size disables the bounds check; labels
/// with a non-finite anchor or box are hidden. Deterministic.
pub fn label_layout(boxes: &[LabelBox], bounds: Rect, obstacles: &[Rect]) -> Vec<Option<usize>> {
    let mut out = vec![None; boxes.len()];
    let bounded = rect_ok(&bounds);
    let finite_rect = rect_ok;
    // Grid cell size: the median label extent (at least something positive).
    let mut sizes: Vec<f64> = boxes.iter().filter(|b| finite_rect(&b.rect)).map(|b| b.rect.w.max(b.rect.h)).collect();
    sizes.sort_by(|a, b| total_cmp(*a, *b));
    let cell = sizes.get(sizes.len() / 2).copied().filter(|&s| s > 0.0).unwrap_or(1.0);
    let mut index = GridIndex { cell, cells: BTreeMap::new(), rects: Vec::new() };
    for o in obstacles.iter().filter(|o| finite_rect(o)) {
        index.insert(*o);
    }
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    // NaN priorities go last (total_cmp alone would rank them highest).
    let prio = |i: usize| if boxes[i].priority.is_nan() { f64::NEG_INFINITY } else { boxes[i].priority };
    order.sort_by(|&a, &b| total_cmp(prio(b), prio(a)));
    let zero = [Vec2::ZERO];
    for &i in &order {
        let b = &boxes[i];
        if !b.anchor.is_finite() || !finite_rect(&b.rect) {
            continue;
        }
        let cands: &[Vec2] = if b.candidates.is_empty() { &zero } else { &b.candidates };
        for (k, off) in cands.iter().enumerate() {
            if !off.is_finite() {
                continue;
            }
            let r = Rect::new(b.anchor.x + off.x + b.rect.x, b.anchor.y + off.y + b.rect.y, b.rect.w, b.rect.h);
            let inside = !bounded || (r.x >= bounds.x && r.y >= bounds.y && r.x1() <= bounds.x1() && r.y1() <= bounds.y1());
            if inside && !index.hits(&r) {
                index.insert(r);
                out[i] = Some(k);
                break;
            }
        }
    }
    out
}

/// Strict overlap (touching edges don't count).
fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x1() && b.x < a.x1() && a.y < b.y1() && b.y < a.y1()
}

struct GridIndex {
    cell: f64,
    cells: BTreeMap<(i64, i64), Vec<u32>>,
    rects: Vec<Rect>,
}

impl GridIndex {
    fn span(&self, r: &Rect) -> (i64, i64, i64, i64) {
        let f = |v: f64| (v / self.cell).floor().clamp(-1e15, 1e15) as i64;
        (f(r.x), f(r.y), f(r.x1()), f(r.y1()))
    }
    fn insert(&mut self, r: Rect) {
        let id = self.rects.len() as u32;
        self.rects.push(r);
        let (x0, y0, x1, y1) = self.span(&r);
        // Very large boxes (relative to the cell) would touch too many cells: cap the loop by
        // storing them in a coarse overflow cell checked by everyone.
        if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > 4096 {
            self.cells.entry((i64::MIN, i64::MIN)).or_default().push(id);
            return;
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.cells.entry((x, y)).or_default().push(id);
            }
        }
    }
    fn hits(&self, r: &Rect) -> bool {
        if let Some(big) = self.cells.get(&(i64::MIN, i64::MIN)) {
            if big.iter().any(|&id| overlaps(&self.rects[id as usize], r)) {
                return true;
            }
        }
        let (x0, y0, x1, y1) = self.span(r);
        if (x1 - x0 + 1).saturating_mul(y1 - y0 + 1) > 4096 {
            return self.rects.iter().any(|o| overlaps(o, r));
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                if let Some(ids) = self.cells.get(&(x, y)) {
                    if ids.iter().any(|&id| overlaps(&self.rects[id as usize], r)) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_math::Rng;

    fn around(anchor: Vec2, priority: f64) -> LabelBox {
        LabelBox {
            rect: Rect::new(0.0, 0.0, 40.0, 12.0),
            priority,
            anchor,
            candidates: vec![Vec2::new(4.0, -14.0), Vec2::new(4.0, 2.0), Vec2::new(-44.0, -14.0), Vec2::new(-44.0, 2.0)],
        }
    }

    fn placed(boxes: &[LabelBox], chosen: &[Option<usize>]) -> Vec<(usize, Rect)> {
        chosen
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                c.map(|k| {
                    let b = &boxes[i];
                    let o = b.candidates.get(k).copied().unwrap_or(Vec2::ZERO);
                    (i, Rect::new(b.anchor.x + o.x + b.rect.x, b.anchor.y + o.y + b.rect.y, b.rect.w, b.rect.h))
                })
            })
            .collect()
    }

    #[test]
    fn never_overlaps_and_respects_bounds() {
        let mut rng = Rng::new(5);
        let boxes: Vec<LabelBox> =
            (0..500).map(|i| around(Vec2::new(rng.range(0.0, 600.0), rng.range(0.0, 400.0)), (i % 7) as f64)).collect();
        let bounds = Rect::new(0.0, 0.0, 600.0, 400.0);
        let chosen = label_layout(&boxes, bounds, &[]);
        let p = placed(&boxes, &chosen);
        assert!(p.len() > 50 && p.len() < 500, "some placed, some hidden: {}", p.len());
        for (a, (i, ra)) in p.iter().enumerate() {
            assert!(ra.x >= 0.0 && ra.y >= 0.0 && ra.x1() <= 600.0 && ra.y1() <= 400.0);
            for (j, rb) in &p[a + 1..] {
                assert!(!overlaps(ra, rb), "{i} and {j} overlap");
            }
        }
        assert_eq!(label_layout(&boxes, bounds, &[]), chosen, "deterministic");
    }

    #[test]
    fn higher_priority_wins() {
        // Two labels competing for the same single spot.
        let mk = |p: f64| LabelBox { rect: Rect::new(0.0, 0.0, 10.0, 10.0), priority: p, anchor: Vec2::new(5.0, 5.0), candidates: vec![] };
        let chosen = label_layout(&[mk(1.0), mk(5.0), mk(3.0)], Rect::new(0.0, 0.0, 100.0, 100.0), &[]);
        assert_eq!(chosen, vec![None, Some(0), None]);
        let nan_last = label_layout(&[mk(f64::NAN), mk(-1e9)], Rect::new(0.0, 0.0, 100.0, 100.0), &[]);
        assert_eq!(nan_last, vec![None, Some(0)], "a NaN priority ranks below everything");
        // A hidden label had every candidate blocked by something placed earlier.
        let mut rng = Rng::new(9);
        let boxes: Vec<LabelBox> = (0..200).map(|_| around(Vec2::new(rng.range(0.0, 200.0), rng.range(0.0, 200.0)), rng.next_f64())).collect();
        let chosen = label_layout(&boxes, Rect::new(-1e9, -1e9, 2e9, 2e9), &[]);
        let p = placed(&boxes, &chosen);
        for (i, c) in chosen.iter().enumerate() {
            if c.is_none() {
                for off in &boxes[i].candidates {
                    let b = &boxes[i];
                    let r = Rect::new(b.anchor.x + off.x, b.anchor.y + off.y, 40.0, 12.0);
                    let blocked = p.iter().any(|(j, q)| overlaps(q, &r) && boxes[*j].priority >= b.priority);
                    assert!(blocked, "label {i} is only hidden by labels of at least its priority");
                }
            }
        }
    }

    #[test]
    fn candidates_in_order_and_obstacles() {
        let b = around(Vec2::new(100.0, 100.0), 1.0);
        let first = label_layout(std::slice::from_ref(&b), Rect::new(0.0, 0.0, 200.0, 200.0), &[]);
        assert_eq!(first, vec![Some(0)]);
        // Block the first two candidates with an obstacle to the right.
        let obstacle = Rect::new(100.0, 80.0, 60.0, 40.0);
        let moved = label_layout(std::slice::from_ref(&b), Rect::new(0.0, 0.0, 200.0, 200.0), &[obstacle]);
        assert_eq!(moved, vec![Some(2)]);
        // Out of bounds everywhere.
        let none = label_layout(std::slice::from_ref(&b), Rect::new(0.0, 0.0, 50.0, 50.0), &[]);
        assert_eq!(none, vec![None]);
        let unbounded = label_layout(std::slice::from_ref(&b), Rect::new(0.0, 0.0, f64::INFINITY, 1.0), &[]);
        assert_eq!(unbounded, vec![Some(0)]);
        let bad = LabelBox { anchor: Vec2::new(f64::NAN, 0.0), ..b.clone() };
        assert_eq!(label_layout(&[bad], Rect::new(0.0, 0.0, 200.0, 200.0), &[]), vec![None]);
        assert!(label_layout(&[], Rect::new(0.0, 0.0, 1.0, 1.0), &[]).is_empty());
    }
}
