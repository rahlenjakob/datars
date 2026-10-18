//! Text along a path (curved labels on arcs and circles: a gauge's bands, a radial chart's rings,
//! a circular axis). The text is shaped and measured as any text — its glyphs, kerning and size
//! are what a straight label of it would have — then each glyph is set on the path at its
//! advance's middle, turned to the path's direction there. One layout, done here, so every
//! renderer draws the same glyphs at the same places (P1).
//!
//! A text longer than its path isn't squeezed or run past the path's end: it's left out (its
//! placements empty), which `datars labels` reports. Recipes measure first (`measure()`) and give
//! way gracefully — a smaller size, a shorter name.

use datars_math::{m, PathData, Rect, Vec2};
use datars_scene::text::Align;
use datars_scene::TextNode;
use std::sync::Arc;

/// Flattening tolerance for the path, px.
const TOLERANCE: f64 = 0.05;

/// An arc `[cx, cy, r, a0, a1]` (radians clockwise from 12 o'clock) as a polyline from `a0` to
/// `a1`.
pub fn arc_points(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Vec<Vec2> {
    let span = a1 - a0;
    let n = ((span.abs() * r.abs().max(1.0) / 2.0).ceil() as usize).clamp(2, 4096);
    (0..=n)
        .map(|i| {
            let a = a0 + span * i as f64 / n as f64;
            let (s, c) = m::sin_cos(a);
            Vec2::new(cx + r * s, cy - r * c)
        })
        .collect()
}

/// The first subpath of `path` as a polyline.
pub fn path_points(path: &PathData) -> Vec<Vec2> {
    path.flatten(TOLERANCE).into_iter().next().map(|(p, _)| p).unwrap_or_default()
}

/// Set a shaped `node` along `pts`: where along the path its `align` point goes (`offset`, a
/// share of the length; `None`: by its alignment), reading left to right where the path runs
/// right to left when `upright`. The node's origin becomes the path's (0, 0); its bounds cover
/// its glyphs (places are relative to the origin, so nudging a label moves it whole).
pub fn set_along(node: &mut TextNode, pts: &[Vec2], offset: Option<f64>, upright: bool) {
    let mut pts: Vec<Vec2> = pts.iter().copied().filter(|p| p.is_finite()).collect();
    pts.dedup_by(|a, b| (*a - *b).len() < 1e-9);
    node.origin = Vec2::ZERO;
    if pts.len() < 2 || node.runs.is_empty() {
        node.along = Some(Arc::from(Vec::new()));
        node.runs.clear();
        return;
    }
    if upright && pts[pts.len() - 1].x < pts[0].x {
        pts.reverse();
    }
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1] - w[0]).len());
    }
    let length = *cum.last().unwrap_or(&0.0);
    // Glyph places along the text, in run order, and their advances.
    let xs: Vec<f64> = node.runs.iter().flat_map(|r| r.glyphs.iter().map(|g| g.x as f64)).collect();
    let (left, width) = (node.bounds.x, node.bounds.w);
    let k = match node.style.align {
        Align::Start => 0.0,
        Align::Middle => 0.5,
        Align::End => 1.0,
    };
    let at = offset.unwrap_or(k).clamp(0.0, 1.0) * length;
    let start = (at - k * width).clamp(0.0, (length - width).max(0.0));
    if width > length + 0.5 {
        node.along = Some(Arc::from(Vec::new()));
        node.runs.clear();
        return;
    }
    let point = |s: f64| -> (Vec2, Vec2) {
        let s = s.clamp(0.0, length);
        let i = cum.partition_point(|c| *c <= s).clamp(1, pts.len() - 1);
        let seg = cum[i] - cum[i - 1];
        let t = if seg > 0.0 { (s - cum[i - 1]) / seg } else { 0.0 };
        let d = pts[i] - pts[i - 1];
        (pts[i - 1] + d * t, d.normalize())
    };
    let mut places = Vec::with_capacity(xs.len());
    let mut bounds: Option<Rect> = None;
    let (top, h) = (node.bounds.y, node.bounds.h);
    for (j, x) in xs.iter().enumerate() {
        let adv = xs.get(j + 1).map_or(left + width - x, |n| n - x).max(0.0);
        let (p, dir) = point(start + (x - left) + adv / 2.0);
        let turn = m::atan2(dir.y, dir.x);
        let o = p - dir * (adv / 2.0);
        places.push([o.x as f32, o.y as f32, turn as f32, adv as f32]);
        // The glyph's line box, turned with it.
        let n = Vec2::new(-dir.y, dir.x);
        for c in [o + n * top, o + dir * adv + n * top, o + n * (top + h), o + dir * adv + n * (top + h)] {
            bounds = Some(bounds.map_or(Rect::new(c.x, c.y, 0.0, 0.0), |b| b.include(c)));
        }
    }
    node.bounds = bounds.unwrap_or_default();
    node.rotate = 0.0;
    node.along = Some(places.into());
}

/// Each glyph's line box as drawn on a curved text (empty for a straight one): what two curved
/// labels must not share.
pub fn glyph_quads(t: &TextNode, xf: &datars_math::Affine) -> Vec<[Vec2; 4]> {
    let Some(along) = &t.along else { return Vec::new() };
    let (top, h) = {
        // The line box of the text as shaped, from its style: ascent above, descent below.
        let size = t.style.size;
        (-size * 0.8, size * 1.05)
    };
    along
        .iter()
        .map(|[x, y, turn, adv]| {
            let o = xf.apply(t.origin + Vec2::new(*x as f64, *y as f64));
            let (s, c) = m::sin_cos(*turn as f64);
            let (dir, n) = (Vec2::new(c, s), Vec2::new(-s, c));
            let a = *adv as f64;
            [o + n * top, o + dir * a + n * top, o + dir * a + n * (top + h), o + n * (top + h)]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_scene::text::{Baseline, TextStyle};

    fn shaped(s: &str, align: Align) -> TextNode {
        let db = datars_text::FontDb::builtin();
        let mut t = TextNode::new(s, Vec2::ZERO, TextStyle { size: 12.0, align, baseline: Baseline::Middle, ..Default::default() });
        datars_text::layout(&db, &mut t);
        t
    }

    #[test]
    fn glyphs_sit_on_the_arc_turned_with_it() {
        let mut t = shaped("Greed", Align::Middle);
        let w = t.bounds.w;
        // A half circle of radius 100 over (100, 100), left to right through the top.
        let pts = arc_points(100.0, 100.0, 100.0, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        set_along(&mut t, &pts, None, true);
        let along = t.along.clone().unwrap();
        assert_eq!(along.len(), 5);
        // Centred at the top: the middle glyph's middle is near (100, 0), turned flat.
        let [x, y, turn, adv] = along[2];
        let (s, c) = m::sin_cos(turn as f64);
        let mid = Vec2::new(x as f64 + c * adv as f64 / 2.0, y as f64 + s * adv as f64 / 2.0);
        assert!((mid - Vec2::new(100.0, 0.0)).len() < 1.5, "{mid:?}");
        assert!(turn.abs() < 0.1);
        // The first glyph turns up-left (negative angle), the last down-right.
        assert!(along[0][2] < 0.0 && along[4][2] > 0.0, "{along:?}");
        // Every glyph is about its radius from the centre.
        for g in along.iter() {
            assert!(((Vec2::new(g[0] as f64, g[1] as f64) - Vec2::new(100.0, 100.0)).len() - 100.0).abs() < 2.0);
        }
        assert!(t.bounds.w >= w && t.bounds.w < w * 1.6 && t.bounds.contains(Vec2::new(100.0, 0.0)), "the glyphs' boxes: {:?} for {w}", t.bounds);
    }

    #[test]
    fn upright_text_on_a_lower_arc_reads_left_to_right() {
        let mut t = shaped("Fear", Align::Middle);
        // The bottom half, drawn right to left (clockwise from 3 o'clock to 9 o'clock).
        let pts = arc_points(0.0, 0.0, 80.0, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
        set_along(&mut t, &pts, None, true);
        let a = t.along.clone().unwrap();
        assert!(a[0][0] < a[3][0], "first letter on the left: {a:?}");
    }

    #[test]
    fn a_text_longer_than_its_path_is_left_out() {
        let mut t = shaped("Extreme greed and more", Align::Start);
        set_along(&mut t, &[Vec2::new(0.0, 0.0), Vec2::new(20.0, 0.0)], None, true);
        assert!(t.along.as_ref().unwrap().is_empty() && t.runs.is_empty());
    }
}
