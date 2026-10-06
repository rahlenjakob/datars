//! GPU rendering tests: headless (Metal on macOS), read back, assert pixels. Every test skips
//! with a message when the machine has no adapter (e.g. CI without a GPU).

use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, Rect, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, InstancesOp, NoGlyphs, Op, SharedPath, StrokeStyle};
use datars_render_wgpu::{CachePolicy, GpuError, Renderer};
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto, SymbolKind};
use std::sync::Arc;

fn renderer(w: u32, h: u32, dpr: f64) -> Option<Renderer> {
    match Renderer::headless(w, h, dpr) {
        Ok(r) => Some(r),
        Err(GpuError::NoAdapter(e) | GpuError::NoDevice(e)) => {
            eprintln!("skipping GPU test: no GPU adapter available ({e})");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

struct Img {
    w: u32,
    data: Vec<u8>,
}

impl Img {
    fn px(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.data[i], self.data[i + 1], self.data[i + 2], self.data[i + 3]]
    }
}

fn render(r: &mut Renderer, list: &DisplayList) -> Img {
    render_with(r, list, &NoGlyphs)
}

fn render_with(r: &mut Renderer, list: &DisplayList, glyphs: &dyn GlyphSource) -> Img {
    r.render(list, glyphs).unwrap();
    let data = r.read_rgba();
    assert_eq!(data.len(), (r.size().0 * r.size().1 * 4) as usize);
    Img { w: r.size().0, data }
}

#[track_caller]
fn assert_px(img: &Img, x: u32, y: u32, want: [u8; 4], tol: u8) {
    let got = img.px(x, y);
    let ok = got.iter().zip(want.iter()).all(|(a, b)| a.abs_diff(*b) <= tol);
    assert!(ok, "pixel ({x}, {y}) = {got:?}, want {want:?} ±{tol}");
}

fn list(w: f64, h: f64, ops: Vec<Op>) -> DisplayList {
    DisplayList { width: w, height: h, background: Color::WHITE, ops }
}

fn fill(path: PathData, c: Color) -> Op {
    fill_paint(path, DPaint::Solid(c), 1.0)
}

fn fill_paint(path: PathData, paint: DPaint, opacity: f32) -> Op {
    Op::Fill { path: SharedPath::new(path), xf: Affine::IDENTITY, paint, rule: FillRule::NonZero, opacity }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> PathData {
    PathData::rect(Rect::new(x, y, w, h))
}

fn stroke(path: PathData, c: Color, width: f64, opacity: f32) -> Op {
    Op::Stroke {
        path: SharedPath::new(path),
        xf: Affine::IDENTITY,
        style: StrokeStyle { width_px: width, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None },
        paint: DPaint::Solid(c),
        opacity,
    }
}

fn symbols(kind: SymbolKind, pts: &[(f64, f64)], r: f64, c: Color) -> InstancesOp {
    let n = pts.len();
    InstancesOp {
        proto: Proto::Symbol { symbol: kind },
        xf: Affine::IDENTITY,
        x: pts.iter().map(|p| p.0).collect(),
        y: pts.iter().map(|p| p.1).collect(),
        size: vec![r; n].into(),
        w: None,
        h: None,
        fill: vec![c; n].into(),
        opacity: vec![1.0; n].into(),
        alpha: 1.0,
        stroke: None,
        size_scale: 1.0,
    }
}

const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn red() -> Color {
    Color::rgb8(255, 0, 0)
}
fn green() -> Color {
    Color::rgb8(0, 255, 0)
}
fn blue() -> Color {
    Color::rgb8(0, 0, 255)
}

#[test]
fn filled_rect_inside_outside_and_edges() {
    let Some(mut r) = renderer(100, 80, 1.0) else { return };
    assert_eq!(r.samples(), 4, "4x MSAA on this adapter");
    let img = render(&mut r, &list(100.0, 80.0, vec![fill(rect(10.0, 10.0, 40.0, 30.0), red()), fill(rect(60.5, 10.0, 20.0, 20.0), blue())]));
    assert_px(&img, 30, 25, RED, 0);
    assert_px(&img, 10, 10, RED, 0);
    assert_px(&img, 49, 39, RED, 0);
    assert_px(&img, 5, 5, WHITE, 0);
    assert_px(&img, 50, 25, WHITE, 0);
    assert_px(&img, 30, 40, WHITE, 0);
    // An edge through the middle of pixel 60: half covered (4x MSAA → 2 of 4 samples).
    let e = img.px(60, 15);
    assert!(e[0] > 100 && e[0] < 160 && e[2] == 255, "half-covered edge pixel {e:?}");
    assert_px(&img, 61, 15, BLUE, 0);
}

#[test]
fn dpr_scales_css_to_physical_pixels() {
    let Some(mut r) = renderer(80, 80, 2.0) else { return };
    let img = render(&mut r, &list(40.0, 40.0, vec![fill(rect(10.0, 10.0, 10.0, 10.0), red())]));
    assert_px(&img, 20, 20, RED, 0);
    assert_px(&img, 39, 39, RED, 0);
    assert_px(&img, 19, 30, WHITE, 0);
    assert_px(&img, 40, 30, WHITE, 0);
}

#[test]
fn stroked_line_dashes_and_hairlines() {
    let Some(mut r) = renderer(100, 100, 1.0) else { return };
    let line = |y: f64| PathData::polyline(&[Vec2::new(10.0, y), Vec2::new(90.0, y)]);
    let mut dashed = stroke(line(40.0), blue(), 4.0, 1.0);
    if let Op::Stroke { style, .. } = &mut dashed {
        style.dash = Some(vec![10.0, 10.0]);
    }
    let ops = vec![stroke(line(20.0), blue(), 4.0, 1.0), dashed, stroke(line(60.0), Color::BLACK, 0.5, 1.0)];
    let img = render(&mut r, &list(100.0, 100.0, ops));
    // Solid: 4 px wide around y = 20 → rows 18..21.
    assert_px(&img, 50, 18, BLUE, 0);
    assert_px(&img, 50, 21, BLUE, 0);
    assert_px(&img, 50, 17, WHITE, 0);
    assert_px(&img, 50, 22, WHITE, 0);
    assert_px(&img, 9, 20, WHITE, 0);
    // Butt caps end exactly at the endpoints.
    assert_px(&img, 10, 20, BLUE, 0);
    assert_px(&img, 89, 20, BLUE, 0);
    assert_px(&img, 90, 20, WHITE, 0);
    // Dashed [10, 10] from x = 10: dash 10..20, gap 20..30, dash 30..40.
    assert_px(&img, 15, 40, BLUE, 0);
    assert_px(&img, 25, 40, WHITE, 0);
    assert_px(&img, 35, 40, BLUE, 0);
    // A half-pixel hairline draws one pixel at half strength, not broken up.
    let h: Vec<u8> = (20..80).map(|x| img.px(x, 59)[0].min(img.px(x, 60)[0])).collect();
    assert!(h.iter().all(|&v| v < 200), "hairline visible along its length: {h:?}");
}

#[test]
fn strokes_stay_even_under_non_uniform_scale() {
    let Some(mut r) = renderer(100, 80, 1.0) else { return };
    // Data-space geometry under scale(10, 1): local x 2..8 → device 20..80, y 20..60.
    let grad = DPaint::Linear { p0: Vec2::new(2.0, 0.0), p1: Vec2::new(8.0, 0.0), stops: vec![(0.0, red()), (1.0, blue())] };
    let op = Op::Stroke {
        path: SharedPath::new(rect(2.0, 20.0, 6.0, 40.0)),
        xf: Affine::scale(10.0, 1.0),
        style: StrokeStyle { width_px: 4.0, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None },
        paint: grad,
        opacity: 1.0,
    };
    let img = render(&mut r, &list(100.0, 80.0, vec![op]));
    // 4 px everywhere: x 18..22 and 78..82 for the sides, y 18..22 and 58..62 for top/bottom.
    for (x, y) in [(18, 40), (21, 40), (78, 40), (81, 40), (50, 18), (50, 21), (50, 58), (50, 61)] {
        assert_ne!(img.px(x, y), WHITE, "({x}, {y}) is on the stroke");
    }
    for (x, y) in [(16, 40), (23, 40), (76, 40), (83, 40), (50, 16), (50, 23), (50, 56), (50, 63)] {
        assert_px(&img, x, y, WHITE, 0);
    }
    // The gradient runs in local x: red at the left side, blue at the right, half way between.
    assert_px(&img, 20, 40, RED, 3);
    assert_px(&img, 80, 40, BLUE, 3);
    let mid = img.px(50, 20);
    assert!(mid[0].abs_diff(mid[2]) <= 8 && mid[1] < 5, "{mid:?}");
    // Panning (translation only) reuses the device-space mesh.
    let n = r.stats().tessellations;
    let mut moved = list(100.0, 80.0, vec![]);
    moved.ops.push(Op::Stroke {
        path: SharedPath::new(rect(2.0, 20.0, 6.0, 40.0)),
        xf: Affine::translate(3.0, 4.0).mul(Affine::scale(10.0, 1.0)),
        style: StrokeStyle { width_px: 4.0, cap: Cap::Butt, join: Join::Miter, miter_limit: 4.0, dash: None },
        paint: DPaint::Solid(green()),
        opacity: 1.0,
    });
    let img = render(&mut r, &moved);
    assert_eq!(r.stats().tessellations, n);
    assert_px(&img, 23, 44, GREEN, 0);
}

#[test]
fn translucent_strokes_do_not_double_blend_at_joins() {
    let Some(mut r) = renderer(100, 100, 1.0) else { return };
    // A sharp V: lyon's triangles overlap around the join.
    let v = PathData::polyline(&[Vec2::new(10.0, 10.0), Vec2::new(50.0, 90.0), Vec2::new(90.0, 10.0)]);
    let img = render(&mut r, &list(100.0, 100.0, vec![stroke(v, Color::BLACK, 12.0, 0.5)]));
    let seg = img.px(30, 50); // on the first segment
    assert!(seg[0].abs_diff(128) <= 2, "50% black over white: {seg:?}");
    for (x, y) in [(50, 84), (50, 88), (48, 86), (52, 86)] {
        let j = img.px(x, y);
        assert!(j[0].abs_diff(seg[0]) <= 2, "join ({x},{y}) {j:?} vs segment {seg:?}");
    }
}

#[test]
fn circle_and_symbol_instances() {
    let Some(mut r) = renderer(160, 60, 1.0) else { return };
    let mut circles = symbols(SymbolKind::Circle, &[(30.0, 30.0)], 10.0, green());
    circles.stroke = Some((Color::BLACK, 2.0));
    let kinds = [SymbolKind::Square, SymbolKind::Diamond, SymbolKind::Triangle, SymbolKind::Star];
    let mut ops = vec![Op::Instances(circles)];
    for (i, k) in kinds.iter().enumerate() {
        ops.push(Op::Instances(symbols(*k, &[(60.0 + 25.0 * i as f64, 30.0)], 8.0, blue())));
    }
    let img = render(&mut r, &list(160.0, 60.0, ops));
    assert_px(&img, 30, 30, GREEN, 0);
    assert_px(&img, 30, 22, GREEN, 0);
    assert_px(&img, 30, 44, WHITE, 0);
    assert_px(&img, 45, 30, WHITE, 0);
    // The outline straddles the edge at r = 10: black around x = 40.
    let o = img.px(40, 30);
    assert!(o[0] < 60 && o[1] < 60, "outline {o:?}");
    // Anti-aliased: along the diagonal the outer edge crosses pixels partially.
    let edge: Vec<[u8; 4]> = (5..10).map(|k| img.px(30 + k, 30 + k)).collect();
    assert!(edge.iter().any(|p| p[0] > 20 && p[0] < 235), "soft edge {edge:?}");
    for i in 0..4u32 {
        assert_px(&img, 60 + 25 * i, 30, BLUE, 0);
    }
    // Square half-size 8: corner (+7, +7) inside; the diamond's corner region is outside.
    assert_px(&img, 67, 37, BLUE, 0);
    assert_px(&img, 85 + 6, 30 + 6, WHITE, 0);
    // Star: tip up at 1.25·8 = 10 px, notch between tips much closer.
    assert_px(&img, 135, 25, BLUE, 0);
    let tip = img.px(135, 21);
    assert!(tip[0] > 20 && tip[0] < 235, "the narrow tip is anti-aliased {tip:?}");
    assert_px(&img, 135, 36, WHITE, 0);
}

#[test]
fn rect_instances_follow_the_transform() {
    let Some(mut r) = renderer(100, 100, 1.0) else { return };
    let op = InstancesOp {
        proto: Proto::Rect,
        xf: Affine::translate(10.0, 10.0).mul(Affine::scale(2.0, 2.0)),
        x: vec![0.0, 20.0].into(),
        y: vec![0.0, 30.0].into(),
        size: vec![].into(),
        w: Some(vec![10.0, 10.0].into()),
        h: Some(vec![5.0, -10.0].into()),
        fill: vec![red(), blue()].into(),
        opacity: vec![1.0, 0.5].into(),
        alpha: 1.0,
        stroke: None,
        size_scale: 1.0,
    };
    let img = render(&mut r, &list(100.0, 100.0, vec![Op::Instances(op)]));
    // Rect 0: local (0,0)-(10,5) → device (10,10)-(30,20).
    assert_px(&img, 10, 10, RED, 0);
    assert_px(&img, 29, 19, RED, 0);
    assert_px(&img, 30, 15, WHITE, 0);
    assert_px(&img, 20, 20, WHITE, 0);
    // Rect 1: negative height → local (20,20)-(30,30) → device (50,50)-(70,70), half opacity.
    assert_px(&img, 60, 60, [127, 127, 255, 255], 1);
    assert_px(&img, 60, 71, WHITE, 0);
}

#[test]
fn linear_and_radial_gradients() {
    let Some(mut r) = renderer(100, 60, 1.0) else { return };
    let lin = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(100.0, 0.0), stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)] };
    let rad = DPaint::Radial { c: Vec2::new(50.0, 45.0), r: 10.0, stops: vec![(0.0, red()), (1.0, blue())] };
    let img = render(&mut r, &list(100.0, 60.0, vec![fill_paint(rect(0.0, 0.0, 100.0, 30.0), lin, 1.0), fill_paint(rect(0.0, 30.0, 100.0, 30.0), rad, 1.0)]));
    let row: Vec<u8> = (0..100).map(|x| img.px(x, 15)[0]).collect();
    assert!(row.windows(2).all(|w| w[1] >= w[0]), "monotonic ramp {row:?}");
    assert!(row[0] <= 3 && row[99] >= 252, "{} .. {}", row[0], row[99]);
    assert!(row[50].abs_diff(128) <= 3, "midpoint {}", row[50]);
    // Radial: red at the centre (the pixel centre is 0.71 px out: t = 0.07), blue beyond r
    // (pad), purple in between.
    assert_px(&img, 50, 45, [237, 0, 18, 255], 3);
    assert_px(&img, 70, 45, BLUE, 0);
    // Pixel centre (55.5, 45.5) is 5.52 px out: t = 0.552.
    assert_px(&img, 55, 45, [114, 0, 141, 255], 3);
}

#[test]
fn even_odd_fill_rule() {
    let Some(mut r) = renderer(60, 60, 1.0) else { return };
    let mut ring = rect(10.0, 10.0, 40.0, 40.0);
    ring.extend(&rect(20.0, 20.0, 20.0, 20.0));
    let op = Op::Fill { path: SharedPath::new(ring), xf: Affine::IDENTITY, paint: DPaint::Solid(red()), rule: FillRule::EvenOdd, opacity: 1.0 };
    let img = render(&mut r, &list(60.0, 60.0, vec![op]));
    assert_px(&img, 15, 30, RED, 0);
    assert_px(&img, 30, 30, WHITE, 0);
}

#[test]
fn nested_clips_intersect_and_pop() {
    let Some(mut r) = renderer(100, 100, 1.0) else { return };
    let clip = |p: PathData| Op::PushClip { path: SharedPath::new(p), xf: Affine::IDENTITY, rule: FillRule::NonZero };
    let ops = vec![
        clip(rect(0.0, 0.0, 50.0, 100.0)),
        fill(rect(0.0, 0.0, 100.0, 10.0), blue()), // clipped to x < 50
        clip(PathData::circle(Vec2::new(50.0, 50.0), 30.0)),
        fill(rect(0.0, 0.0, 100.0, 100.0), red()), // clipped to the left half of the circle
        Op::PopClip,
        fill(rect(0.0, 90.0, 100.0, 10.0), green()), // back to x < 50 only
        Op::PopClip,
        fill(rect(90.0, 0.0, 10.0, 10.0), Color::BLACK), // unclipped
    ];
    let img = render(&mut r, &list(100.0, 100.0, ops));
    assert_px(&img, 25, 5, BLUE, 0);
    assert_px(&img, 75, 5, WHITE, 0);
    assert_px(&img, 40, 50, RED, 0);
    assert_px(&img, 60, 50, WHITE, 0);
    assert_px(&img, 25, 25, WHITE, 0);
    assert_px(&img, 25, 95, GREEN, 0);
    assert_px(&img, 75, 95, WHITE, 0);
    assert_px(&img, 95, 5, [0, 0, 0, 255], 0);
}

#[test]
fn layer_opacity_is_exact_group_opacity() {
    let Some(mut r) = renderer(100, 60, 1.0) else { return };
    let ops = vec![
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal },
        fill(rect(10.0, 10.0, 50.0, 40.0), red()),
        fill(rect(40.0, 10.0, 50.0, 40.0), blue()),
        Op::PopLayer,
    ];
    let img = render(&mut r, &list(100.0, 60.0, ops));
    assert_px(&img, 20, 30, [255, 127, 127, 255], 1);
    assert_px(&img, 50, 30, [127, 127, 255, 255], 1); // blue over red inside the layer, then 50%
    assert_px(&img, 80, 30, [127, 127, 255, 255], 1);
    assert_px(&img, 95, 30, WHITE, 0);
}

#[test]
fn multiply_and_screen_layers() {
    let Some(mut r) = renderer(100, 40, 1.0) else { return };
    let gray = Color::rgba(0.5, 0.5, 0.5, 1.0);
    let ops = vec![
        fill(rect(0.0, 0.0, 100.0, 40.0), red()),
        Op::PushLayer { opacity: 1.0, blend: Blend::Multiply },
        fill(rect(0.0, 0.0, 50.0, 40.0), gray),
        Op::PopLayer,
        Op::PushLayer { opacity: 1.0, blend: Blend::Screen },
        fill(rect(50.0, 0.0, 50.0, 40.0), gray),
        Op::PopLayer,
    ];
    let img = render(&mut r, &list(100.0, 40.0, ops));
    assert_px(&img, 25, 20, [128, 0, 0, 255], 1);
    assert_px(&img, 75, 20, [255, 128, 128, 255], 1);
}

#[test]
fn multiply_over_a_transparent_backdrop_is_exact() {
    let Some(mut r) = renderer(40, 40, 1.0) else { return };
    let mut l = list(40.0, 40.0, vec![Op::PushLayer { opacity: 1.0, blend: Blend::Multiply }, fill(rect(0.0, 0.0, 40.0, 40.0), Color::rgba(0.0, 0.0, 1.0, 0.5)), Op::PopLayer]);
    l.background = Color::TRANSPARENT;
    let img = render(&mut r, &l);
    // Over nothing, multiply is plain source-over: 50% blue.
    assert_px(&img, 20, 20, [0, 0, 255, 128], 1);
}

#[test]
fn mixed_scene_clip_layer_gradient_instances() {
    let Some(mut r) = renderer(120, 120, 1.0) else { return };
    let grad = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(0.0, 120.0), stops: vec![(0.0, red()), (1.0, blue())] };
    let ops = vec![
        fill_paint(rect(0.0, 0.0, 120.0, 120.0), grad, 1.0),
        Op::PushClip { path: SharedPath::new(rect(20.0, 20.0, 80.0, 80.0)), xf: Affine::IDENTITY, rule: FillRule::NonZero },
        Op::PushLayer { opacity: 0.5, blend: Blend::Normal },
        fill(rect(0.0, 0.0, 120.0, 120.0), Color::WHITE),
        Op::Instances(symbols(SymbolKind::Circle, &[(60.0, 60.0), (10.0, 10.0)], 15.0, green())),
        Op::PopLayer,
        Op::PopClip,
    ];
    let img = render(&mut r, &list(120.0, 120.0, ops));
    let top = img.px(60, 5);
    assert!(top[0] > 240 && top[2] < 15, "gradient top red {top:?}");
    // Inside the clip: white at 50% over the gradient.
    let w = img.px(30, 60);
    assert!(w[0] > 180 && w[2] > 180 && w[1].abs_diff(128) <= 2, "{w:?}");
    // The circle at the centre: green at 50% over the gradient.
    let g = img.px(60, 60);
    assert!(g[1] >= 126 && g[0] < 130 && g[2] < 130, "{g:?}");
    // The circle at (10, 10) is outside the clip.
    let c = img.px(10, 10);
    assert!(c[1] < 5, "clipped instance {c:?}");
    assert_eq!(r.stats().passes, 3, "root, layer, root again");
}

#[test]
fn straight_fills_keep_their_mesh_across_zoom_curves_retessellate() {
    // A camera flight into a city: building outlines are all straight edges, so their triangles
    // don't depend on the zoom; a circle's do (finer curves as it grows).
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    let block = PathData::polygon(&[Vec2::new(2.0, 2.0), Vec2::new(12.0, 2.0), Vec2::new(12.0, 9.0), Vec2::new(2.0, 9.0)]);
    let ring = PathData::circle(Vec2::new(8.0, 8.0), 5.0);
    let at = |zoom: f64| {
        let ops = vec![fill(block.clone(), red()), fill(ring.clone(), blue())];
        let mut l = list(64.0, 64.0, ops);
        for op in &mut l.ops {
            if let Op::Fill { xf, .. } = op {
                *xf = Affine::scale(zoom, zoom);
            }
        }
        l
    };
    r.render(&at(1.0), &NoGlyphs).unwrap();
    assert_eq!(r.stats().tessellations, 2);
    r.render(&at(4.0), &NoGlyphs).unwrap();
    assert_eq!(r.stats().tessellations, 3, "only the circle again, two octaves in");
    let img = render(&mut r, &at(4.0));
    assert_px(&img, 20, 12, RED, 0);
}

#[test]
fn meshes_are_cached_across_frames_and_evicted_when_unseen() {
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    let a = list(64.0, 64.0, vec![fill(PathData::circle(Vec2::new(32.0, 32.0), 20.0), red()), stroke(rect(5.0, 5.0, 50.0, 50.0), blue(), 2.0, 1.0)]);
    r.render(&a, &NoGlyphs).unwrap();
    let s1 = r.stats();
    assert_eq!(s1.tessellations, 2);
    assert_eq!(s1.meshes, 0, "first frame: drawn from the frame's arena");
    r.render(&a, &NoGlyphs).unwrap();
    let s2 = r.stats();
    assert_eq!(s2.tessellations, 2, "second frame: no new tessellation");
    assert_eq!(s2.meshes, 2, "second frame: retained");
    assert_eq!(s2.cache_hits, s1.cache_hits + 2);
    // Moving and recolouring reuses the mesh (the transform is a uniform).
    let mut moved = a.clone();
    if let Op::Fill { xf, paint, .. } = &mut moved.ops[0] {
        *xf = Affine::translate(3.0, 1.5);
        *paint = DPaint::Solid(green());
    }
    let img = render(&mut r, &moved);
    assert_eq!(r.stats().tessellations, 2);
    assert_px(&img, 35, 34, GREEN, 0);
    // A different stroke width in another bucket is a new mesh; zooming 2x a new tolerance.
    let mut wider = a.clone();
    if let Op::Stroke { style, .. } = &mut wider.ops[1] {
        style.width_px = 6.0;
    }
    r.render(&wider, &NoGlyphs).unwrap();
    assert_eq!(r.stats().tessellations, 3);
    // Unseen meshes go.
    r.set_cache_policy(CachePolicy { max_unseen_frames: 2, max_bytes: u64::MAX });
    let b = list(64.0, 64.0, vec![fill(rect(1.0, 1.0, 5.0, 5.0), red())]);
    // (The cache looks for stale meshes every few dozen frames, not every frame.)
    for _ in 0..40 {
        r.render(&b, &NoGlyphs).unwrap();
    }
    let s = r.stats();
    assert_eq!(s.meshes, 1, "only the rect is resident");
    // The circle and the first stroke; the wider stroke, drawn one frame, was never retained.
    assert!(s.evictions >= 2);
    r.clear_caches();
    assert_eq!(r.stats().meshes, 0);
}

#[test]
fn retained_meshes_share_pool_pages_that_are_recycled() {
    // Hundreds of retained meshes live in a page or two (not two buffers each); once they're all
    // evicted, the page takes new meshes, which draw their own shapes, not what was there.
    let Some(mut r) = renderer(200, 200, 1.0) else { return };
    let grid = |c: Color, dx: f64| list(200.0, 200.0, (0..400).map(|i| fill(rect((i % 20) as f64 * 10.0 + dx, (i / 20) as f64 * 10.0, 4.0 + (i % 3) as f64 * 0.01, 4.0), c)).collect());
    let a = grid(red(), 0.0);
    for _ in 0..8 {
        r.render(&a, &NoGlyphs).unwrap();
    }
    let s = r.stats();
    assert!(s.meshes >= 3, "{} meshes", s.meshes);
    assert_eq!(s.pool_pages, 2, "a page of vertices and one of indices for every fill");
    r.set_cache_policy(CachePolicy { max_unseen_frames: 2, max_bytes: u64::MAX });
    let b = grid(blue(), 5.0);
    for _ in 0..40 {
        r.render(&b, &NoGlyphs).unwrap();
    }
    let img = render(&mut r, &b);
    assert_px(&img, 7, 2, BLUE, 0);
    assert_px(&img, 2, 2, WHITE, 0);
    assert!(r.stats().evictions >= 3);
    assert!(r.stats().pool_pages <= 4, "pages recycled, not piled up: {}", r.stats().pool_pages);
}

#[test]
fn huge_transforms_keep_f32_precision() {
    let Some(mut r) = renderer(100, 100, 1.0) else { return };
    // A half-unit square 10 million units from the origin, zoomed 100x back onto the screen.
    let far = 1.0e7;
    let op = Op::Fill {
        path: SharedPath::new(rect(far, far, 0.5, 0.5)),
        xf: Affine::translate(25.0 - 100.0 * far, 25.0 - 100.0 * far).mul(Affine::scale(100.0, 100.0)),
        paint: DPaint::Solid(red()),
        rule: FillRule::NonZero,
        opacity: 1.0,
    };
    let img = render(&mut r, &list(100.0, 100.0, vec![op]));
    for (x, want) in [(24, WHITE), (25, RED), (74, RED), (75, WHITE)] {
        assert_px(&img, x, 50, want, 0);
        assert_px(&img, 50, x, want, 0);
    }
}

#[test]
fn many_instances_render() {
    let Some(mut r) = renderer(512, 512, 1.0) else { return };
    let n = 100_000usize;
    // A deterministic grid-ish scatter with per-instance colour and opacity.
    let (mut xs, mut ys, mut fills, mut ops) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for i in 0..n {
        xs.push((i % 400) as f64 * 1.25 + 6.0);
        ys.push((i / 400) as f64 * 2.0 + 6.0);
        fills.push(if i % 2 == 0 { red() } else { blue() });
        ops.push(if i % 3 == 0 { 0.5 } else { 1.0 });
    }
    let inst = InstancesOp {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        xf: Affine::IDENTITY,
        x: xs.into(),
        y: ys.into(),
        size: vec![1.5; n].into(),
        w: None,
        h: None,
        fill: fills.into(),
        opacity: ops.into(),
        alpha: 1.0,
        stroke: None,
        size_scale: 1.0,
    };
    let l = list(512.0, 512.0, vec![Op::Instances(inst)]);
    let img = render(&mut r, &l);
    assert_eq!(r.stats().instances, n as u32);
    assert_eq!(r.stats().draws, 1, "one instanced draw");
    assert_px(&img, 510, 510, WHITE, 0);
    let covered = (0..500u32).filter(|&y| img.px(100, y) != WHITE).count();
    assert!(covered > 400, "the scatter covers the field: {covered}");
    let frames = 30;
    let t = std::time::Instant::now();
    for _ in 0..frames {
        r.render(&l, &NoGlyphs).unwrap();
    }
    let _ = r.read_rgba(); // wait for the GPU
    let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    eprintln!("{n} instances: {ms:.2} ms/frame (upload + draw, headless, incl. CPU packing)");
}

#[test]
fn cached_paths_make_frames_cheap() {
    let Some(mut r) = renderer(512, 512, 1.0) else { return };
    // 4000 distinct paths (circles and outlines): tessellated once, then only uniforms + draws.
    let mut ops = Vec::new();
    for i in 0..2000 {
        let (x, y) = ((i % 50) as f64 * 10.0 + 6.0, (i / 50) as f64 * 12.0 + 6.0);
        ops.push(fill(PathData::circle(Vec2::new(x, y), 3.0 + (i % 3) as f64), if i % 2 == 0 { red() } else { blue() }));
        ops.push(stroke(rect(x - 4.0, y - 4.0, 8.0, 8.0), Color::rgba(0.0, 0.0, 0.0, 0.5), 1.0, 1.0));
    }
    let l = list(512.0, 512.0, ops);
    let t = std::time::Instant::now();
    r.render(&l, &NoGlyphs).unwrap();
    let _ = r.read_rgba();
    let first = t.elapsed().as_secs_f64() * 1000.0;
    let tess = r.stats().tessellations;
    assert_eq!(tess, 4000);
    let frames = 20;
    let t = std::time::Instant::now();
    for _ in 0..frames {
        r.render(&l, &NoGlyphs).unwrap();
    }
    let _ = r.read_rgba();
    let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
    assert_eq!(r.stats().tessellations, tess, "no tessellation after the first frame");
    eprintln!("4000 paths: first frame {first:.1} ms (tessellate + upload), then {ms:.2} ms/frame ({} draws)", r.stats().draws);
}

/// Glyph "font": every glyph is a 6×8 box on the baseline.
struct Boxes;
impl GlyphSource for Boxes {
    fn outline(&self, _font: &str, _glyph: u16, size: f64) -> Option<PathData> {
        let k = size / 10.0;
        Some(rect(0.0, -8.0 * k, 6.0 * k, 8.0 * k))
    }
}

#[test]
fn glyphs_fill_with_halos_underneath() {
    let Some(mut r) = renderer(100, 40, 1.0) else { return };
    let glyphs: Arc<[GlyphPos]> = vec![GlyphPos { id: 7, x: 0.0, y: 0.0 }, GlyphPos { id: 7, x: 20.0, y: 0.0 }].into();
    let op = Op::Glyphs { font: Arc::from("Box"), size: 20.0, glyphs, origin: Vec2::new(10.0, 30.0), scale: 1.0, rotate: 0.0, color: Color::BLACK, halo: Some((red(), 4.0)), text: None };
    let img = render_with(&mut r, &list(100.0, 40.0, vec![op]), &Boxes);
    // Glyph 1: 12×16 box at (10, 14)-(22, 30); a halo of width 4 is a 4 px stroke of the outline,
    // reaching 2 px beyond it (as the CPU reference and SVG draw it).
    assert_px(&img, 16, 22, [0, 0, 0, 255], 0);
    assert_px(&img, 36, 22, [0, 0, 0, 255], 0);
    assert_px(&img, 23, 22, RED, 0);
    assert_px(&img, 16, 12, RED, 0);
    assert_px(&img, 25, 22, WHITE, 0);
    assert_px(&img, 16, 10, WHITE, 0);
    assert_eq!(r.stats().tessellations, 2, "one outline mesh and one halo mesh, shared by both glyphs");
    r.render(&list(100.0, 40.0, vec![]), &Boxes).unwrap();
    assert_eq!(r.stats().tessellations, 2);
}

#[test]
fn a_run_drawn_again_is_one_mesh_with_the_same_pixels() {
    // Its first frame draws glyph by glyph (a run seen once may be a number counting up); from its
    // second frame running, the run is one mesh for its fill and one for its halo.
    let Some(mut r) = renderer(160, 40, 1.0) else { return };
    let glyphs: Arc<[GlyphPos]> = (0..6).map(|i| GlyphPos { id: 7, x: i as f32 * 20.0, y: 0.0 }).collect::<Vec<_>>().into();
    let op = Op::Glyphs { font: Arc::from("Box"), size: 20.0, glyphs, origin: Vec2::new(10.0, 30.0), scale: 1.0, rotate: 0.0, color: Color::BLACK, halo: Some((red(), 4.0)), text: None };
    let l = list(160.0, 40.0, vec![op]);
    let first = render_with(&mut r, &l, &Boxes);
    assert_eq!(r.stats().draws, 12, "six glyphs and six halos");
    let second = render_with(&mut r, &l, &Boxes);
    assert_eq!(r.stats().draws, 2, "the run's halo and its fill");
    assert_eq!(first.data, second.data);
    render_with(&mut r, &l, &Boxes);
    assert_eq!(r.stats().draws, 2);
}

#[test]
fn flattened_theme_scene_renders() {
    use datars_scene::{Geom, Key, Node, Paint, Scene, Stroke};
    use datars_theme::{resolve, Mode, ThemeSet};
    let Some(mut r) = renderer(120, 80, 1.0) else { return };
    let set = ThemeSet::with_builtins();
    let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Dark, &[]);
    let bar = Node::shape(Key::one("bar"), Geom::rect(10.0, 10.0, 40.0, 60.0)).fill(Paint::token("accent"));
    let line = Node::shape(Key::one("line"), Geom::polyline(vec![Vec2::new(60.0, 70.0), Vec2::new(110.0, 20.0)])).stroke(Stroke::new(Paint::token("ink"), 3.0));
    let scene = Scene::new(120.0, 80.0, Node::group(Key::name("root"), vec![bar, line]));
    let dl = datars_render::flatten(&scene, &theme);
    let img = render(&mut r, &dl);
    let bg = dl.background.to_rgba8();
    assert_px(&img, 2, 2, bg, 0);
    let accent = match &dl.ops[0] {
        Op::Fill { paint: DPaint::Solid(c), .. } => c.to_rgba8(),
        other => panic!("{other:?}"),
    };
    assert_px(&img, 30, 40, accent, 0);
    assert_px(&img, 85, 45, match &dl.ops[1] {
        Op::Stroke { paint: DPaint::Solid(c), .. } => c.to_rgba8(),
        other => panic!("{other:?}"),
    }, 2);
}

#[test]
fn resize_and_errors() {
    let Some(mut r) = renderer(10, 10, 1.0) else { return };
    r.resize(33, 17, 1.5);
    assert_eq!(r.size(), (33, 17));
    let img = render(&mut r, &list(22.0, 11.0, vec![fill(rect(0.0, 0.0, 100.0, 100.0), red())]));
    assert_px(&img, 32, 16, RED, 0);
    assert_eq!(img.data.len(), 33 * 17 * 4);
    // Empty and degenerate ops draw nothing and don't fail.
    let junk = vec![
        fill(PathData::new(), red()),
        fill(rect(f64::NAN, 0.0, 1.0, 1.0), red()),
        Op::Fill { path: SharedPath::new(rect(0.0, 0.0, 5.0, 5.0)), xf: Affine::scale(0.0, 1.0), paint: DPaint::Solid(red()), rule: FillRule::NonZero, opacity: 1.0 },
        Op::PopClip,
        Op::PopLayer,
        Op::PushLayer { opacity: 0.0, blend: Blend::Normal },
    ];
    let img = render(&mut r, &list(22.0, 11.0, junk));
    assert_px(&img, 5, 5, WHITE, 0);
}

#[test]
fn outlines_new_every_frame_are_drawn_without_being_retained() {
    // A morph: every frame a new outline for each of many shapes. They're drawn (from the frame's
    // arena) and never retained; the cache stays as it was.
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    for f in 0..6 {
        let k = f as f64 * 0.5;
        let ops: Vec<Op> = (0..40)
            .map(|i| {
                let (x, y) = ((i % 8) as f64 * 8.0 + 1.0, (i / 8) as f64 * 8.0 + 1.0);
                fill(PathData::polygon(&[Vec2::new(x, y), Vec2::new(x + 5.0 + k, y), Vec2::new(x + 5.0, y + 5.0 + k), Vec2::new(x, y + 5.0)]), red())
            })
            .collect();
        let img = render(&mut r, &list(64.0, 64.0, ops));
        assert_px(&img, 3, 3, RED, 0);
        assert_px(&img, 11, 11, RED, 0);
        assert_px(&img, 7, 7, WHITE, 0);
    }
    let s = r.stats();
    assert_eq!(s.meshes, 0, "nothing retained");
    assert_eq!(s.tessellations, 240);
}

#[test]
fn concave_and_self_crossing_outlines_fill_like_before() {
    // The convex fan must not take these: an L (concave) and a bow-tie (crossing, even-odd).
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    let l = PathData::polygon(&[Vec2::new(4.0, 4.0), Vec2::new(30.0, 4.0), Vec2::new(30.0, 12.0), Vec2::new(12.0, 12.0), Vec2::new(12.0, 30.0), Vec2::new(4.0, 30.0)]);
    let img = render(&mut r, &list(64.0, 64.0, vec![fill(l, red())]));
    assert_px(&img, 8, 8, RED, 0);
    assert_px(&img, 8, 26, RED, 0);
    assert_px(&img, 24, 24, WHITE, 0);
    let star = PathData::polygon(&[
        Vec2::new(32.0, 4.0), Vec2::new(44.0, 60.0), Vec2::new(4.0, 22.0), Vec2::new(60.0, 22.0), Vec2::new(20.0, 60.0),
    ]);
    let op = Op::Fill { path: SharedPath::new(star), xf: Affine::IDENTITY, paint: DPaint::Solid(red()), rule: FillRule::EvenOdd, opacity: 1.0 };
    let img = render(&mut r, &list(64.0, 64.0, vec![op]));
    assert_px(&img, 32, 34, WHITE, 0);
    assert_px(&img, 32, 10, RED, 0);
}

#[test]
fn thin_straight_strokes_keep_their_mesh_while_moving() {
    // A map zooming in: its borders are 1 px wide on screen, so their width in the path's units
    // changes every frame, crossing a width bucket every quarter octave. Moving, each keeps the
    // mesh it was last built with (the shader draws the exact width); settled, the exact one is
    // built.
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    let border = rect(1.0, 1.0, 6.0, 6.0);
    let at = |zoom: f64| {
        let mut l = list(64.0, 64.0, vec![stroke(border.clone(), blue(), 1.0, 1.0)]);
        if let Op::Stroke { xf, .. } = &mut l.ops[0] {
            *xf = Affine::scale(zoom, zoom);
        }
        l
    };
    r.set_moving(true);
    for i in 0..40 {
        r.render(&at(1.0 + 7.0 * i as f64 / 39.0), &NoGlyphs).unwrap();
    }
    assert_eq!(r.stats().tessellations, 1, "three octaves of zoom on the first mesh");
    // Drawn where it belongs, at its exact width: a 1 px line on x = 8, half in each pixel.
    let img = render(&mut r, &at(8.0));
    for x in [7, 8] {
        let c = img.px(x, 30);
        assert!(c[2] == 255 && (100..160).contains(&c[0]), "half of the line in pixel {x}: {c:?}");
    }
    assert_eq!(img.px(20, 30)[0], 255, "nothing beside it");
    r.set_moving(false);
    r.render(&at(8.0), &NoGlyphs).unwrap();
    assert_eq!(r.stats().tessellations, 2, "settled: the exact mesh");
}

#[test]
fn big_unchanged_instance_sets_keep_their_buffer() {
    // A dot map of thousands of dots, the camera moving over it: the same columns every frame
    // (the engine's flatten keeps them), so from the second frame on the set draws from a buffer
    // of its own instead of being rebuilt and uploaded; new columns start over.
    let Some(mut r) = renderer(64, 64, 1.0) else { return };
    let n = 5000;
    let dots = |x0: f64| InstancesOp {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        xf: Affine::IDENTITY,
        x: (0..n).map(|i| x0 + (i % 50) as f64).collect(),
        y: (0..n).map(|i| (i / 50) as f64 * 0.5).collect(),
        size: vec![1.0; n].into(),
        w: None,
        h: None,
        fill: vec![blue(); n].into(),
        opacity: vec![1.0; n].into(),
        alpha: 1.0,
        stroke: None,
        size_scale: 1.0,
    };
    let set = dots(5.0);
    let at = |dx: f64, set: &InstancesOp| {
        let mut op = set.clone();
        op.xf = Affine::translate(dx, 0.0);
        list(64.0, 64.0, vec![Op::Instances(op)])
    };
    r.render(&at(0.0, &set), &NoGlyphs).unwrap();
    assert_eq!(r.stats().instance_sets_kept, 0, "first frame: this frame's buffer");
    let img0 = render(&mut r, &at(0.0, &set));
    for dx in 1..4 {
        r.render(&at(dx as f64, &set), &NoGlyphs).unwrap();
        assert_eq!(r.stats().instance_sets_kept, 1, "frame {dx}: its own buffer");
    }
    assert_eq!(r.stats().instances_rebuilt, 0, "nothing rebuilt");
    // Moved back: the same picture as when it was built.
    let img = render(&mut r, &at(0.0, &set));
    assert_eq!((img.px(10, 10), img.px(40, 20)), (img0.px(10, 10), img0.px(40, 20)));
    // Fading (a tile easing in, a level crossfading): the groups' alpha, from the same buffer.
    let mut faded = set.clone();
    faded.alpha = 0.5;
    let img = render(&mut r, &at(0.0, &faded));
    assert_eq!((r.stats().instance_sets_kept, r.stats().instances_rebuilt), (1, 0), "a fade rebuilds nothing");
    // …drawn as the same opacity baked into the columns would be.
    let mut baked = set.clone();
    baked.opacity = vec![0.5; n].into();
    let want = render(&mut r, &at(0.0, &baked));
    for (x, y) in [(5, 0), (10, 10), (54, 24), (30, 30)] {
        assert_eq!(img.px(x, y), want.px(x, y), "({x}, {y})");
    }
    assert_ne!(img.px(5, 0), img0.px(5, 0), "and fainter");
    // New columns (another set): built again.
    r.render(&at(0.0, &dots(6.0)), &NoGlyphs).unwrap();
    assert_eq!(r.stats().instance_sets_kept, 0, "new columns: rebuilt");
    assert_eq!(r.stats().instances_rebuilt, n as u32);
}

#[test]
fn a_path_drawn_in_chunks_shows_no_seams_between_them() {
    // 1,600 squares that share their edges, in one opaque path far over the chunk size: drawn as
    // several meshes, the shared edges must not show the background (MSAA covers each sample
    // with exactly one of the adjacent triangles).
    let Some(mut r) = renderer(200, 200, 1.0) else { return };
    let mut p = datars_math::PathData::new();
    for i in 0..1600 {
        let (x, y) = ((i % 40) as f64 * 5.0, (i / 40) as f64 * 5.0);
        p.move_to(Vec2::new(x, y));
        p.line_to(Vec2::new(x + 5.0, y));
        p.line_to(Vec2::new(x + 5.0, y + 5.0));
        p.line_to(Vec2::new(x, y + 5.0));
        p.close();
    }
    assert!(p.els.len() > 4096);
    let op = Op::Fill { path: SharedPath::new(p), xf: Affine::translate(0.3, 0.3), paint: DPaint::Solid(blue()), rule: FillRule::NonZero, opacity: 1.0 };
    let img = render(&mut r, &list(200.0, 200.0, vec![op]));
    for (x, y) in [(5, 5), (10, 52), (100, 100), (150, 35), (185, 180)] {
        assert_eq!(img.px(x, y), [0, 0, 255, 255], "({x}, {y}): an edge between squares");
    }
}
