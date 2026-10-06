//! Pixel-level tests of the reference rasterizer: exact coverage where geometry is exact, analytic
//! expectations (areas, 50 % edges, opacity) elsewhere, and pinned hashes for determinism.

use datars_color::Color;
use datars_math::{m, Affine, FillRule, PathData, Rect, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, InstancesOp, NoGlyphs, Op, SharedPath, StrokeStyle};
use datars_render_cpu::{render, Pixmap};
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto, SymbolKind};
use std::sync::Arc;

const RED: Color = Color::rgba(1.0, 0.0, 0.0, 1.0);
const BLUE: Color = Color::rgba(0.0, 0.0, 1.0, 1.0);

fn list(w: f64, h: f64, background: Color, ops: Vec<Op>) -> DisplayList {
    DisplayList { width: w, height: h, background, ops }
}

fn fill(path: PathData, color: Color) -> Op {
    fill_rule(path, color, FillRule::NonZero)
}

fn fill_rule(path: PathData, color: Color, rule: FillRule) -> Op {
    Op::Fill { path: SharedPath::new(path), xf: Affine::IDENTITY, paint: DPaint::Solid(color), rule, opacity: 1.0 }
}

fn style(width: f64, cap: Cap, join: Join) -> StrokeStyle {
    StrokeStyle { width_px: width, cap, join, miter_limit: 4.0, dash: None }
}

fn stroke(path: PathData, style: StrokeStyle, color: Color) -> Op {
    Op::Stroke { path: SharedPath::new(path), xf: Affine::IDENTITY, style, paint: DPaint::Solid(color), opacity: 1.0 }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> PathData {
    PathData::rect(Rect::new(x, y, w, h))
}

fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> PathData {
    PathData::polyline(&[Vec2::new(x0, y0), Vec2::new(x1, y1)])
}

fn draw(w: f64, h: f64, ops: Vec<Op>) -> Pixmap {
    render(&list(w, h, Color::TRANSPARENT, ops), &NoGlyphs, 1.0)
}

fn alpha(p: &Pixmap, x: u32, y: u32) -> u8 {
    p.pixel(x, y)[3]
}

/// Total coverage in pixels (sum of alpha / 255).
fn coverage(p: &Pixmap) -> f64 {
    p.data.iter().skip(3).step_by(4).map(|&a| a as f64).sum::<f64>() / 255.0
}

fn near(a: u8, b: u8, tol: u8) -> bool {
    a.abs_diff(b) <= tol
}

// ---------------------------------------------------------------------------------------------
// Fills

#[test]
fn integer_rect_fills_exactly_its_pixels() {
    let p = draw(20.0, 20.0, vec![fill(rect(5.0, 5.0, 10.0, 10.0), RED)]);
    assert_eq!((p.width, p.height), (20, 20));
    for y in 0..20 {
        for x in 0..20 {
            let inside = (5..15).contains(&x) && (5..15).contains(&y);
            assert_eq!(p.pixel(x, y), if inside { [255, 0, 0, 255] } else { [0, 0, 0, 0] }, "({x},{y})");
        }
    }
}

#[test]
fn half_pixel_edges_are_half_covered() {
    let p = draw(20.0, 20.0, vec![fill(rect(5.5, 5.5, 5.0, 5.0), RED)]);
    // Edges: 50 %; corners: 25 %; interior: full. Colour stays pure red in straight alpha.
    assert!(near(alpha(&p, 5, 8), 128, 1), "{}", alpha(&p, 5, 8));
    assert!(near(alpha(&p, 10, 8), 128, 1));
    assert!(near(alpha(&p, 8, 5), 128, 1));
    assert!(near(alpha(&p, 8, 10), 128, 1));
    assert!(near(alpha(&p, 5, 5), 64, 1), "{}", alpha(&p, 5, 5));
    assert_eq!(p.pixel(7, 7), [255, 0, 0, 255]);
    assert_eq!(p.pixel(5, 8)[..3], [255, 0, 0]);
    assert_eq!(alpha(&p, 4, 8), 0);
    assert_eq!(alpha(&p, 11, 8), 0);
    assert!((coverage(&p) - 25.0).abs() < 0.05, "{}", coverage(&p));
}

#[test]
fn circle_coverage_sums_to_its_area() {
    let r = 20.0;
    let p = draw(64.0, 64.0, vec![fill(PathData::circle(Vec2::new(32.3, 31.7), r), RED)]);
    let want = m::PI * r * r;
    let got = coverage(&p);
    // The 0.1 px flattening tolerance makes the polygon a hair smaller than the circle.
    assert!(got < want && got > want * 0.99, "{got} vs {want}");
    assert_eq!(p.pixel(32, 32), [255, 0, 0, 255]);
    assert_eq!(alpha(&p, 32, 5), 0);
}

#[test]
fn polygon_coverage_is_its_exact_area() {
    // An arbitrary triangle: the analytic rasterizer's coverage sum is the area up to 1/256 px
    // rounding of the vertices.
    let tri = PathData::polygon(&[Vec2::new(3.1, 2.7), Vec2::new(40.9, 11.3), Vec2::new(12.2, 37.6)]);
    let p = draw(48.0, 48.0, vec![fill(tri, RED)]);
    let area: f64 = 0.5 * ((40.9f64 - 3.1) * (37.6 - 2.7) - (12.2 - 3.1) * (11.3 - 2.7)).abs();
    assert!((coverage(&p) - area).abs() < 0.5, "{} vs {area}", coverage(&p));
}

fn star() -> PathData {
    // A pentagram drawn by connecting every second vertex: the centre has winding number 2.
    let c = Vec2::new(32.0, 32.0);
    let pts: Vec<Vec2> = (0..5).map(|i| Vec2::polar(c, 28.0, m::TAU * (i * 2) as f64 / 5.0)).collect();
    PathData::polygon(&pts)
}

#[test]
fn even_odd_star_has_a_hole_and_non_zero_does_not() {
    let nz = draw(64.0, 64.0, vec![fill_rule(star(), RED, FillRule::NonZero)]);
    let eo = draw(64.0, 64.0, vec![fill_rule(star(), RED, FillRule::EvenOdd)]);
    assert_eq!(alpha(&nz, 32, 32), 255);
    assert_eq!(alpha(&eo, 32, 32), 0);
    // A point: the tips are filled either way.
    assert_eq!(alpha(&nz, 32, 8), 255);
    assert_eq!(alpha(&eo, 32, 8), 255);
    assert!(coverage(&nz) > coverage(&eo) + 100.0);
}

#[test]
fn opposite_winding_subpaths_cut_holes_under_non_zero() {
    let mut p = rect(4.0, 4.0, 24.0, 24.0);
    p.extend(&rect(10.0, 10.0, 12.0, 12.0).reversed());
    let px = draw(32.0, 32.0, vec![fill(p, RED)]);
    assert_eq!(alpha(&px, 6, 16), 255);
    assert_eq!(alpha(&px, 16, 16), 0);
}

#[test]
fn shapes_clip_to_the_pixmap() {
    let p = draw(10.0, 10.0, vec![fill(rect(-5.0, -5.0, 10.0, 30.0), RED), fill(rect(8.0, 8.0, 100.0, 100.0), BLUE)]);
    assert_eq!(p.pixel(0, 0), [255, 0, 0, 255]);
    assert_eq!(p.pixel(4, 9), [255, 0, 0, 255]);
    assert_eq!(alpha(&p, 5, 0), 0);
    assert_eq!(p.pixel(9, 9), [0, 0, 255, 255]);
}

#[test]
fn transforms_apply_before_rasterizing() {
    let op = Op::Fill {
        path: SharedPath::new(rect(0.0, 0.0, 2.0, 2.0)),
        xf: Affine::translate(4.0, 6.0).mul(Affine::scale(3.0, 2.0)),
        paint: DPaint::Solid(RED),
        rule: FillRule::NonZero,
        opacity: 1.0,
    };
    let p = draw(16.0, 16.0, vec![op]);
    assert_eq!(alpha(&p, 4, 6), 255);
    assert_eq!(alpha(&p, 9, 9), 255);
    assert_eq!(alpha(&p, 10, 9), 0);
    assert_eq!(alpha(&p, 9, 10), 0);
    assert!((coverage(&p) - 24.0).abs() < 1e-9);
}

#[test]
fn op_opacity_scales_alpha() {
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 4.0, 4.0)), xf: Affine::IDENTITY, paint: DPaint::Solid(RED), rule: FillRule::NonZero, opacity: 0.5 };
    let p = render(&list(4.0, 4.0, Color::WHITE, vec![op]), &NoGlyphs, 1.0);
    assert_eq!(p.pixel(1, 1), [255, 127, 127, 255]);
}

#[test]
fn degenerate_input_never_panics() {
    let nan = PathData::polygon(&[Vec2::new(f64::NAN, 1.0), Vec2::new(5.0, 5.0), Vec2::new(1.0, 9.0)]);
    let huge = PathData::polygon(&[Vec2::new(-1e30, -1e30), Vec2::new(1e30, 5.0), Vec2::new(3.0, 1e30)]);
    let ops = vec![
        fill(nan.clone(), RED),
        fill(huge.clone(), BLUE),
        stroke(nan, style(2.0, Cap::Round, Join::Round), RED),
        stroke(huge, style(2.0, Cap::Round, Join::Round), RED),
        stroke(line(1.0, 1.0, 1.0, 1.0), style(f64::NAN, Cap::Round, Join::Round), RED),
        fill(PathData::new(), RED),
        Op::PopClip,
        Op::PopLayer,
    ];
    let p = draw(10.0, 10.0, ops);
    assert_eq!(p.data.len(), 400);
    assert_eq!(draw(0.0, 0.0, vec![fill(rect(0.0, 0.0, 1.0, 1.0), RED)]).data.len(), 0);
    assert_eq!(render(&list(f64::NAN, 5.0, RED, vec![]), &NoGlyphs, 1.0).width, 0);
}

// ---------------------------------------------------------------------------------------------
// Strokes

#[test]
fn stroke_widths_have_the_expected_coverage() {
    // A horizontal line through pixel centres of row 20: widths 1 and 3 cover whole rows.
    let one = draw(64.0, 40.0, vec![stroke(line(10.0, 20.5, 50.0, 20.5), style(1.0, Cap::Butt, Join::Miter), RED)]);
    assert_eq!(alpha(&one, 30, 20), 255);
    assert_eq!(alpha(&one, 30, 19), 0);
    assert_eq!(alpha(&one, 30, 21), 0);
    assert!((coverage(&one) - 40.0).abs() < 1e-9);

    let three = draw(64.0, 40.0, vec![stroke(line(10.0, 20.5, 50.0, 20.5), style(3.0, Cap::Butt, Join::Miter), RED)]);
    for y in 19..=21 {
        assert_eq!(alpha(&three, 30, y), 255);
    }
    assert_eq!(alpha(&three, 30, 18), 0);
    assert_eq!(alpha(&three, 30, 22), 0);
    assert!((coverage(&three) - 120.0).abs() < 1e-9);

    // Thinner than a pixel: partial coverage, never nothing.
    let half = draw(64.0, 40.0, vec![stroke(line(10.0, 20.5, 50.0, 20.5), style(0.5, Cap::Butt, Join::Miter), RED)]);
    assert!(near(alpha(&half, 30, 20), 128, 1));
    assert!((coverage(&half) - 20.0).abs() < 0.2);
    let hair = draw(64.0, 40.0, vec![stroke(line(10.0, 20.5, 50.0, 20.5), style(0.1, Cap::Butt, Join::Miter), RED)]);
    assert!(alpha(&hair, 30, 20) > 20 && alpha(&hair, 30, 20) < 32, "{}", alpha(&hair, 30, 20));

    // A 1 px line on a pixel boundary straddles two rows at 50 % each.
    let edge = draw(64.0, 40.0, vec![stroke(line(10.0, 20.0, 50.0, 20.0), style(1.0, Cap::Butt, Join::Miter), RED)]);
    assert!(near(alpha(&edge, 30, 19), 128, 1) && near(alpha(&edge, 30, 20), 128, 1));
}

#[test]
fn diagonal_hairline_keeps_its_ink() {
    // A 0.5 px diagonal: coverage sum ≈ length × width, spread over many partial pixels.
    let p = draw(64.0, 64.0, vec![stroke(line(8.0, 8.0, 56.0, 56.0), style(0.5, Cap::Butt, Join::Miter), RED)]);
    let len = (48.0f64 * 48.0 * 2.0).sqrt();
    assert!((coverage(&p) - len * 0.5).abs() < 0.3, "{}", coverage(&p));
    assert!(alpha(&p, 32, 32) > 0);
}

#[test]
fn caps_differ_at_line_ends() {
    let mk = |cap| draw(48.0, 24.0, vec![stroke(line(10.0, 12.0, 30.0, 12.0), style(6.0, cap, Join::Miter), RED)]);
    let (butt, round, square) = (mk(Cap::Butt), mk(Cap::Round), mk(Cap::Square));
    // Pixel 8 lies 1.5 px beyond the start: only round and square caps reach it.
    assert_eq!(alpha(&butt, 8, 12), 0);
    assert_eq!(alpha(&round, 8, 12), 255);
    assert_eq!(alpha(&square, 8, 12), 255);
    // The corner beyond the end: square covers it, round only grazes it.
    assert_eq!(alpha(&square, 32, 9), 255);
    assert!(alpha(&round, 32, 9) < 40, "{}", alpha(&round, 32, 9));
    // Areas: butt = 20·6, round adds a disc of r = 3 (flattened within 0.1 px, so up to ~1 px²
    // less), square adds 2 · 3·6.
    assert!((coverage(&butt) - 120.0).abs() < 0.05);
    let disc = coverage(&round) - 120.0;
    assert!(disc < m::PI * 9.0 && disc > m::PI * 9.0 - 1.2, "{disc}");
    assert!((coverage(&square) - 156.0).abs() < 0.05);
}

#[test]
fn joins_differ_at_corners() {
    let path = PathData::polyline(&[Vec2::new(8.0, 30.0), Vec2::new(30.0, 30.0), Vec2::new(30.0, 8.0)]);
    let mk = |join| draw(40.0, 40.0, vec![stroke(path.clone(), style(8.0, Cap::Butt, join), RED)]);
    let (miter, round, bevel) = (mk(Join::Miter), mk(Join::Round), mk(Join::Bevel));
    // The outer corner of the right angle is at (34, 34).
    assert_eq!(alpha(&miter, 33, 33), 255);
    assert!(alpha(&round, 33, 33) < 100);
    assert_eq!(alpha(&bevel, 33, 33), 0);
    let (cm, cr, cb) = (coverage(&miter), coverage(&round), coverage(&bevel));
    assert!(cm > cr && cr > cb, "{cm} {cr} {cb}");
    // Miter adds the full 4×4 corner square, bevel half of it, round a quarter disc.
    assert!((cm - cb - 8.0).abs() < 0.05, "{}", cm - cb);
    // (The round join is a polygon within 0.1 px of the arc, so a little under the disc.)
    assert!((cr - cb - (m::PI * 4.0 - 8.0)).abs() < 0.4, "{}", cr - cb);
}

#[test]
fn miter_limit_bevels_sharp_corners() {
    // Miter ratio ≈ 6.1 at the tip (40, 20): beyond the default limit of 4.
    let sharp = PathData::polyline(&[Vec2::new(4.0, 26.0), Vec2::new(40.0, 20.0), Vec2::new(4.0, 14.0)]);
    let mut s = style(4.0, Cap::Butt, Join::Miter);
    let limited = draw(64.0, 40.0, vec![stroke(sharp.clone(), s.clone(), RED)]);
    s.miter_limit = 20.0;
    let long = draw(64.0, 40.0, vec![stroke(sharp, s, RED)]);
    assert_eq!(alpha(&limited, 46, 20), 0);
    assert!(alpha(&long, 46, 20) > 0);
}

#[test]
fn closed_outline_has_no_seam() {
    let p = draw(32.0, 32.0, vec![stroke(rect(8.0, 8.0, 16.0, 16.0), style(2.0, Cap::Butt, Join::Miter), RED)]);
    // All four corners are fully covered (joins close the start corner too) and the inside is empty.
    for (x, y) in [(7, 7), (24, 7), (24, 24), (7, 24), (16, 7)] {
        assert_eq!(alpha(&p, x, y), 255, "({x},{y})");
    }
    assert_eq!(alpha(&p, 16, 16), 0);
    assert!((coverage(&p) - (18.0 * 18.0 - 14.0 * 14.0)).abs() < 1e-9);
}

#[test]
fn dashes_produce_gaps() {
    let mut s = style(2.0, Cap::Butt, Join::Miter);
    s.dash = Some(vec![4.0, 4.0]);
    let p = draw(48.0, 20.0, vec![stroke(line(0.0, 10.0, 40.0, 10.0), s, RED)]);
    for x in [0, 1, 2, 3, 8, 11, 16, 19] {
        assert_eq!(alpha(&p, x, 10), 255, "on at {x}");
    }
    for x in [4, 5, 6, 7, 12, 15, 20] {
        assert_eq!(alpha(&p, x, 10), 0, "off at {x}");
    }
    assert!((coverage(&p) - 40.0).abs() < 1e-9, "half of 40 × 2");
}

#[test]
fn round_capped_zero_length_dashes_are_dots() {
    let mut s = style(4.0, Cap::Round, Join::Round);
    s.dash = Some(vec![0.0, 10.0]);
    let p = draw(48.0, 20.0, vec![stroke(line(5.0, 10.0, 47.0, 10.0), s, RED)]);
    for x in [5, 15, 25, 35, 45] {
        assert!(alpha(&p, x, 10) > 200, "dot at {x}: {}", alpha(&p, x, 10));
    }
    assert_eq!(alpha(&p, 10, 10), 0);
}

#[test]
fn curved_strokes_are_uniformly_thick() {
    // A stroked circle's coverage ≈ the annulus area, and all of it is opaque inside the band.
    let c = Vec2::new(32.0, 32.0);
    let p = draw(64.0, 64.0, vec![stroke(PathData::circle(c, 20.0), style(4.0, Cap::Butt, Join::Round), RED)]);
    let want = m::PI * (22.0 * 22.0 - 18.0 * 18.0);
    assert!((coverage(&p) - want).abs() < want * 0.01, "{} vs {want}", coverage(&p));
    assert_eq!(alpha(&p, 32, 12), 255);
    assert_eq!(alpha(&p, 32, 32), 0);
    // Semi-transparent strokes stay uniform: overlapping pieces don't double up.
    let op = Op::Stroke {
        path: SharedPath::new(PathData::circle(c, 20.0)),
        xf: Affine::IDENTITY,
        style: style(4.0, Cap::Butt, Join::Miter),
        paint: DPaint::Solid(RED),
        opacity: 0.5,
    };
    let half = draw(64.0, 64.0, vec![op]);
    let band: Vec<u8> = [(32, 12), (12, 32), (52, 32), (32, 52)].iter().map(|&(x, y)| alpha(&half, x, y)).collect();
    assert!(band.iter().all(|&a| near(a, 128, 1)), "{band:?}");
}

#[test]
fn strokes_ignore_non_uniform_scale_for_width() {
    // The display list carries the final width in px; strokes are built in device space.
    let op = Op::Stroke {
        path: SharedPath::new(line(0.0, 5.0, 10.0, 5.0)),
        xf: Affine::scale(4.0, 1.0),
        style: style(2.0, Cap::Butt, Join::Miter),
        paint: DPaint::Solid(RED),
        opacity: 1.0,
    };
    let p = draw(48.0, 10.0, vec![op]);
    assert!((coverage(&p) - 80.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------------
// Paints

#[test]
fn linear_gradient_endpoints() {
    let paint = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(100.0, 0.0), stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)] };
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 100.0, 4.0)), xf: Affine::IDENTITY, paint, rule: FillRule::NonZero, opacity: 1.0 };
    let p = draw(110.0, 4.0, vec![op]);
    assert!(p.pixel(0, 1)[0] <= 2, "{:?}", p.pixel(0, 1));
    assert!(p.pixel(99, 1)[0] >= 253, "{:?}", p.pixel(99, 1));
    assert!(near(p.pixel(50, 1)[0], 129, 1), "{:?}", p.pixel(50, 1));
    // Monotone along the gradient, constant across it.
    for x in 1..100 {
        assert!(p.pixel(x, 1)[0] >= p.pixel(x - 1, 1)[0]);
    }
    assert_eq!(p.pixel(37, 0), p.pixel(37, 3));
}

#[test]
fn gradients_follow_the_op_transform() {
    let paint = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(10.0, 0.0), stops: vec![(0.0, RED), (1.0, BLUE)] };
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 10.0, 1.0)), xf: Affine::translate(20.0, 0.0).mul(Affine::scale(4.0, 8.0)), paint, rule: FillRule::NonZero, opacity: 1.0 };
    let p = draw(64.0, 8.0, vec![op]);
    // Local x = (device x + 0.5 − 20) / 4: t = 0.0125 at the first pixel, 0.9875 at the last.
    assert_eq!(p.pixel(20, 4), [252, 0, 3, 255]);
    assert_eq!(p.pixel(59, 4), [3, 0, 252, 255]);
    assert_eq!(alpha(&p, 60, 4), 0);
    assert_eq!(alpha(&p, 19, 4), 0);
}

#[test]
fn radial_gradient_centre_and_rim() {
    let paint = DPaint::Radial { c: Vec2::new(16.0, 16.0), r: 12.0, stops: vec![(0.0, Color::WHITE), (1.0, BLUE)] };
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 32.0, 32.0)), xf: Affine::IDENTITY, paint, rule: FillRule::NonZero, opacity: 1.0 };
    let p = draw(32.0, 32.0, vec![op]);
    let c = p.pixel(16, 16);
    assert!(c[0] >= 235 && c[2] == 255, "{c:?}");
    assert_eq!(p.pixel(0, 0), [0, 0, 255, 255], "clamped beyond the radius");
    assert_eq!(p.pixel(16, 3), p.pixel(3, 16), "radially symmetric");
}

#[test]
fn transparent_stops_fade_without_darkening() {
    let paint = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(64.0, 0.0), stops: vec![(0.0, RED), (1.0, RED.with_alpha(0.0))] };
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 64.0, 2.0)), xf: Affine::IDENTITY, paint, rule: FillRule::NonZero, opacity: 1.0 };
    let p = draw(64.0, 2.0, vec![op]);
    let mid = p.pixel(32, 0);
    assert_eq!(mid[..3], [255, 0, 0][..], "premultiplied interpolation keeps the hue: {mid:?}");
    assert!(near(mid[3], 126, 2), "{mid:?}");
}

// ---------------------------------------------------------------------------------------------
// Clips and layers

#[test]
fn nested_clips_intersect() {
    let clip = |r: Rect| Op::PushClip { path: SharedPath::new(PathData::rect(r)), xf: Affine::IDENTITY, rule: FillRule::NonZero };
    let ops = vec![
        clip(Rect::new(0.0, 0.0, 20.0, 40.0)),
        clip(Rect::new(10.0, 5.0, 30.0, 30.0)),
        fill(rect(0.0, 0.0, 40.0, 40.0), RED),
        Op::PopClip,
        fill(rect(0.0, 36.0, 40.0, 4.0), BLUE), // only the outer clip applies
        Op::PopClip,
        fill(rect(36.0, 0.0, 4.0, 4.0), BLUE), // no clip
    ];
    let p = draw(40.0, 40.0, ops);
    assert_eq!(p.pixel(15, 20), [255, 0, 0, 255]);
    assert_eq!(alpha(&p, 5, 20), 0, "outside the inner clip");
    assert_eq!(alpha(&p, 25, 20), 0, "outside the outer clip");
    assert_eq!(alpha(&p, 15, 2), 0, "above the inner clip");
    assert_eq!(p.pixel(5, 38), [0, 0, 255, 255]);
    assert_eq!(alpha(&p, 25, 38), 0);
    assert_eq!(p.pixel(38, 1), [0, 0, 255, 255]);
    assert!((coverage(&p) - (10.0 * 30.0 + 20.0 * 4.0 + 16.0)).abs() < 1e-9);
}

#[test]
fn clips_anti_alias_and_multiply_coverage() {
    let ops = vec![
        Op::PushClip { path: SharedPath::new(rect(0.0, 0.0, 10.5, 20.0)), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        Op::PushClip { path: SharedPath::new(rect(0.0, 0.0, 20.0, 10.5)), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        fill(rect(0.0, 0.0, 20.0, 20.0), RED),
        Op::PopClip,
        Op::PopClip,
    ];
    let p = draw(20.0, 20.0, ops);
    assert!(near(alpha(&p, 10, 5), 128, 1));
    assert!(near(alpha(&p, 10, 10), 64, 1), "{}", alpha(&p, 10, 10));
}

#[test]
fn empty_clip_hides_everything_until_popped() {
    let ops = vec![
        Op::PushClip { path: SharedPath::new(rect(50.0, 50.0, 5.0, 5.0)), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        fill(rect(0.0, 0.0, 10.0, 10.0), RED),
        Op::PopClip,
        fill(rect(0.0, 0.0, 2.0, 2.0), BLUE),
    ];
    let p = draw(10.0, 10.0, ops);
    assert_eq!(p.pixel(1, 1), [0, 0, 255, 255]);
    assert_eq!(alpha(&p, 5, 5), 0);
}

#[test]
fn layer_opacity_composites_the_group_once() {
    let ops = vec![
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal },
        fill(rect(0.0, 0.0, 6.0, 4.0), RED),
        fill(rect(4.0, 0.0, 6.0, 4.0), RED),
        Op::PopLayer,
    ];
    let p = render(&list(10.0, 4.0, Color::WHITE, ops), &NoGlyphs, 1.0);
    // 50 % red over white, the same in the overlap (x = 4, 5) as outside it.
    assert_eq!(p.pixel(1, 1), [255, 127, 127, 255]);
    assert_eq!(p.pixel(5, 1), [255, 127, 127, 255]);
    assert_eq!(p.pixel(8, 1), [255, 127, 127, 255]);
    // Without the layer, per-op opacity darkens the overlap.
    let op = |x| Op::Fill { path: SharedPath::new(rect(x, 0.0, 6.0, 4.0)), xf: Affine::IDENTITY, paint: DPaint::Solid(RED), rule: FillRule::NonZero, opacity: 0.5 };
    let q = render(&list(10.0, 4.0, Color::WHITE, vec![op(0.0), op(4.0)]), &NoGlyphs, 1.0);
    assert_eq!(q.pixel(1, 1), [255, 127, 127, 255]);
    assert_eq!(q.pixel(5, 1), [255, 63, 63, 255]);
}

#[test]
fn sequential_layers_and_clips_start_clean() {
    // Popped layer and clip buffers are recycled; each new one must start empty.
    let clip = |r: Rect| Op::PushClip { path: SharedPath::new(PathData::rect(r)), xf: Affine::IDENTITY, rule: FillRule::NonZero };
    let ops = vec![
        clip(Rect::new(0.0, 0.0, 5.0, 10.0)),
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal },
        fill(rect(0.0, 0.0, 10.0, 10.0), RED),
        Op::PopLayer,
        Op::PopClip,
        clip(Rect::new(5.0, 0.0, 5.0, 10.0)),
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal },
        fill(rect(0.0, 5.0, 10.0, 5.0), BLUE),
        Op::PopLayer,
        Op::PopClip,
    ];
    let p = draw(10.0, 10.0, ops);
    assert_eq!(p.pixel(2, 2), [255, 0, 0, 128]);
    assert_eq!(p.pixel(2, 7), [255, 0, 0, 128], "the second clip doesn't reach x < 5");
    assert_eq!(p.pixel(7, 2), [0, 0, 0, 0], "the second layer holds nothing of the first");
    assert_eq!(p.pixel(7, 7), [0, 0, 255, 128]);
}

#[test]
fn layer_blend_modes() {
    let bg = Color::rgb8(200, 100, 50);
    let layer = |blend| {
        let ops = vec![Op::PushLayer { opacity: 1.0, blend }, fill(rect(0.0, 0.0, 4.0, 4.0), Color::rgb8(128, 255, 0)), Op::PopLayer];
        render(&list(4.0, 4.0, bg, ops), &NoGlyphs, 1.0).pixel(1, 1)
    };
    assert_eq!(layer(Blend::Normal), [128, 255, 0, 255]);
    assert_eq!(layer(Blend::Multiply), [100, 100, 0, 255]);
    assert_eq!(layer(Blend::Screen), [228, 255, 50, 255]);
}

#[test]
fn transparent_background_gives_straight_alpha() {
    let op = Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 2.0, 2.0)), xf: Affine::IDENTITY, paint: DPaint::Solid(Color::rgba(0.2, 0.4, 0.6, 1.0)), rule: FillRule::NonZero, opacity: 0.25 };
    let p = draw(2.0, 2.0, vec![op]);
    let px = p.pixel(0, 0);
    assert_eq!(px[3], 64);
    assert!(near(px[0], 51, 2) && near(px[1], 102, 2) && near(px[2], 153, 2), "{px:?}");
}

// ---------------------------------------------------------------------------------------------
// Instances and glyphs

fn instances_op() -> InstancesOp {
    let n = 40;
    let x: Vec<f64> = (0..n).map(|i| 4.0 + (i % 8) as f64 * 7.3).collect();
    let y: Vec<f64> = (0..n).map(|i| 5.0 + (i / 8) as f64 * 7.9).collect();
    let size: Vec<f64> = (0..n).map(|i| 1.5 + (i % 5) as f64 * 0.7).collect();
    let fill: Vec<Color> = (0..n).map(|i| Color::rgb8((i * 37 % 256) as u8, (i * 91 % 256) as u8, 180)).collect();
    let opacity: Vec<f32> = (0..n).map(|i| 0.4 + (i % 3) as f32 * 0.3).collect();
    InstancesOp {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        xf: Affine::translate(1.25, 0.5),
        x: x.into(),
        y: y.into(),
        size: size.into(),
        w: None,
        h: None,
        fill: fill.into(),
        opacity: opacity.into(),
        alpha: 1.0,
        stroke: Some((Color::BLACK, 0.75)),
        size_scale: 1.0,
    }
}

/// The same instances drawn as individual fill + stroke ops of their `instance_path`.
fn as_paths(i: &InstancesOp) -> Vec<Op> {
    let mut ops = Vec::new();
    for k in 0..i.len() {
        let path = SharedPath::new(i.instance_path(k));
        let opacity = i.opacity_at(k);
        ops.push(Op::Fill { path: path.clone(), xf: Affine::IDENTITY, paint: DPaint::Solid(i.fill[k]), rule: FillRule::NonZero, opacity });
        if let Some((c, w)) = i.stroke {
            ops.push(Op::Stroke { path, xf: Affine::IDENTITY, style: style(w, Cap::Butt, Join::Miter), paint: DPaint::Solid(c), opacity });
        }
    }
    ops
}

#[test]
fn instances_render_exactly_like_their_paths() {
    let mut protos = vec![instances_op()];
    for symbol in [SymbolKind::Square, SymbolKind::Diamond, SymbolKind::Triangle, SymbolKind::Cross, SymbolKind::Star] {
        let mut i = instances_op();
        i.proto = Proto::Symbol { symbol };
        protos.push(i);
    }
    let mut rects = instances_op();
    rects.proto = Proto::Rect;
    rects.w = Some((0..40).map(|k| 2.0 + (k % 4) as f64).collect::<Vec<_>>().into());
    rects.h = Some((0..40).map(|k| 3.0 - (k % 3) as f64 * 0.8).collect::<Vec<_>>().into());
    rects.xf = Affine::translate(2.0, 1.0).mul(Affine::scale(1.1, 0.9));
    protos.push(rects);
    for i in protos {
        let a = render(&list(64.0, 48.0, Color::WHITE, vec![Op::Instances(i.clone())]), &NoGlyphs, 2.0);
        let b = render(&list(64.0, 48.0, Color::WHITE, as_paths(&i)), &NoGlyphs, 2.0);
        assert_eq!(a.hash(), b.hash(), "{:?}", i.proto);
        assert!(a.data.iter().any(|&v| v < 200), "something was drawn");
    }
}

/// A fake font: every glyph is a square of side `size / 2` sitting on the baseline, 0.1·size in.
struct Squares;
impl GlyphSource for Squares {
    fn outline(&self, _font: &str, glyph: u16, size: f64) -> Option<PathData> {
        (glyph != 0).then(|| rect(0.1 * size, -0.5 * size, 0.5 * size, 0.5 * size))
    }
}

fn glyph_op(halo: Option<(Color, f64)>, scale: f64, rotate: f64) -> Op {
    let glyphs: Arc<[GlyphPos]> = vec![GlyphPos { id: 1, x: 0.0, y: 0.0 }, GlyphPos { id: 0, x: 10.0, y: 0.0 }, GlyphPos { id: 2, x: 20.0, y: 0.0 }].into();
    Op::Glyphs { font: Arc::from("Test-400"), size: 20.0, glyphs, origin: Vec2::new(8.0, 30.0), scale, rotate, color: BLUE, halo, text: None }
}

#[test]
fn glyphs_draw_outlines_from_the_glyph_source() {
    let l = list(64.0, 40.0, Color::TRANSPARENT, vec![glyph_op(None, 1.0, 0.0)]);
    let p = render(&l, &Squares, 1.0);
    // Glyph 1: x 10..20, y 20..30. Glyph 0 has no outline. Glyph 2: x 30..40.
    assert_eq!(p.pixel(15, 25), [0, 0, 255, 255]);
    assert_eq!(p.pixel(35, 25), [0, 0, 255, 255]);
    assert_eq!(alpha(&p, 25, 25), 0);
    assert_eq!(alpha(&p, 15, 31), 0);
    assert!((coverage(&p) - 200.0).abs() < 1e-9);
    // Nothing without a glyph source.
    assert_eq!(coverage(&render(&l, &NoGlyphs, 1.0)), 0.0);
}

#[test]
fn glyph_scale_rotation_and_halo() {
    let p = render(&list(64.0, 40.0, Color::TRANSPARENT, vec![glyph_op(None, 0.5, 0.0)]), &Squares, 1.0);
    assert!((coverage(&p) - 50.0).abs() < 1e-9, "offsets and outlines both scale");
    assert_eq!(alpha(&p, 11, 27), 255);
    let r = render(&list(64.0, 64.0, Color::TRANSPARENT, vec![glyph_op(None, 1.0, m::PI / 2.0)]), &Squares, 1.0);
    // Rotated a quarter turn clockwise about the origin (8, 30): glyph 1 spans x 8..-2 → 8+0..8+10,
    // y 30+2..30+12.
    assert!((coverage(&r) - 200.0).abs() < 0.01);
    assert_eq!(alpha(&r, 13, 35), 255);
    let h = render(&list(64.0, 40.0, Color::TRANSPARENT, vec![glyph_op(Some((Color::WHITE, 4.0)), 1.0, 0.0)]), &Squares, 1.0);
    assert_eq!(h.pixel(15, 25), [0, 0, 255, 255], "fill on top of the halo");
    assert_eq!(h.pixel(15, 31), [255, 255, 255, 255], "halo extends 2 px outside");
    assert_eq!(alpha(&h, 15, 33), 0);
}

// ---------------------------------------------------------------------------------------------
// Device pixel ratio, encoding, determinism

#[test]
fn dpr_scales_the_pixmap_and_geometry() {
    let l = list(30.0, 20.5, Color::WHITE, vec![fill(rect(5.0, 5.0, 10.0, 10.0), RED)]);
    let p = render(&l, &NoGlyphs, 2.0);
    assert_eq!((p.width, p.height), (60, 41));
    assert_eq!(p.pixel(10, 10), [255, 0, 0, 255]);
    assert_eq!(p.pixel(29, 29), [255, 0, 0, 255]);
    assert_eq!(p.pixel(30, 29), [255, 255, 255, 255]);
    assert_eq!(p.pixel(9, 10), [255, 255, 255, 255]);
    assert_eq!(p.pixel(59, 40), [255, 255, 255, 255], "background covers the partial pixel");
    let s = render(&list(64.0, 40.0, Color::TRANSPARENT, vec![stroke(line(10.0, 20.5, 50.0, 20.5), style(1.0, Cap::Butt, Join::Miter), RED)]), &NoGlyphs, 2.0);
    assert!((coverage(&s) - 160.0).abs() < 1e-9, "stroke widths scale with dpr");
}

#[test]
fn png_round_trips() {
    let p = render(&list(7.0, 5.0, Color::WHITE, vec![fill(rect(1.5, 1.0, 3.0, 2.0), RED)]), &NoGlyphs, 1.0);
    let bytes = p.to_png();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let dec = png::Decoder::new(bytes.as_slice());
    let mut reader = dec.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!((info.width, info.height), (7, 5));
    assert_eq!(&buf[..info.buffer_size()], p.data.as_slice());
    assert!(!Pixmap { width: 0, height: 0, data: vec![] }.to_png().is_empty());
}

fn fixed_scene() -> DisplayList {
    let wedge = PathData::annular_sector(Vec2::new(60.0, 50.0), 12.0, 34.0, 0.3, 2.4);
    let curve = {
        let mut p = PathData::new();
        p.move_to(Vec2::new(10.0, 90.0)).cubic_to(Vec2::new(40.0, 10.0), Vec2::new(80.0, 130.0), Vec2::new(150.0, 40.0));
        p
    };
    let mut dashed = style(2.5, Cap::Round, Join::Round);
    dashed.dash = Some(vec![6.0, 3.0, 1.0, 3.0]);
    let grad = DPaint::Linear { p0: Vec2::new(100.0, 10.0), p1: Vec2::new(150.0, 60.0), stops: vec![(0.0, Color::rgb8(66, 105, 208)), (0.6, Color::rgb8(239, 177, 24)), (1.0, Color::rgb8(255, 114, 92).with_alpha(0.5))] };
    let radial = DPaint::Radial { c: Vec2::new(30.0, 30.0), r: 25.0, stops: vec![(0.0, Color::WHITE), (1.0, Color::rgb8(60, 169, 81))] };
    let ops = vec![
        Op::Fill { path: SharedPath::new(rect(95.0, 5.0, 60.0, 60.0)), xf: Affine::IDENTITY, paint: grad, rule: FillRule::NonZero, opacity: 1.0 },
        Op::Fill { path: SharedPath::new(PathData::circle(Vec2::new(30.0, 30.0), 25.0)), xf: Affine::IDENTITY, paint: radial, rule: FillRule::NonZero, opacity: 0.9 },
        Op::PushClip { path: SharedPath::new(PathData::circle(Vec2::new(60.0, 50.0), 30.0)), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        Op::Fill { path: SharedPath::new(wedge), xf: Affine::rotate(0.1), paint: DPaint::Solid(Color::rgb8(164, 99, 242)), rule: FillRule::EvenOdd, opacity: 1.0 },
        Op::PopClip,
        Op::PushLayer { opacity: 0.7, blend: Blend::Multiply },
        stroke(curve, style(5.0, Cap::Square, Join::Miter), Color::rgb8(255, 138, 183)),
        Op::PopLayer,
        stroke(PathData::polyline(&[Vec2::new(5.0, 105.0), Vec2::new(60.0, 70.0), Vec2::new(90.0, 110.0), Vec2::new(155.0, 75.0)]), dashed, Color::rgb8(29, 31, 36)),
        Op::Instances(instances_op()),
        glyph_op(Some((Color::WHITE, 2.0)), 1.0, 0.2),
    ];
    list(160.0, 120.0, Color::rgb8(251, 248, 241), ops)
}

#[test]
fn rendering_is_deterministic() {
    let a = render(&fixed_scene(), &Squares, 1.5);
    let b = render(&fixed_scene(), &Squares, 1.5);
    assert_eq!(a, b);
    assert_eq!(a.hash(), b.hash());
    assert_eq!(a.to_png(), b.to_png());
}

/// Pinned so that any change to rasterization, stroking, paints or compositing is caught. If a
/// change is intended, look at the new frame (render `fixed_scene` to PNG) and update the value.
#[test]
fn pinned_frame_hash() {
    let p = render(&fixed_scene(), &Squares, 1.5);
    assert_eq!((p.width, p.height), (240, 180));
    assert_eq!(p.hash(), 0x39a7_1296_444a_349f, "frame hash changed: {:#018x}", p.hash());
}
