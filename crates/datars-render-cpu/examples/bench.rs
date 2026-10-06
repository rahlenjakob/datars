//! Rough timings for the reference rasterizer (run with `--release`):
//! `cargo run -p datars-render-cpu --release --example bench`

use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, Rect, Rng, Vec2};
use datars_render::{DPaint, DisplayList, InstancesOp, NoGlyphs, Op, SharedPath, StrokeStyle};
use datars_render_cpu::render;
use datars_scene::{Cap, Join, Proto, SymbolKind};
use std::time::Instant;

fn time(name: &str, list: &DisplayList) {
    let _ = render(list, &NoGlyphs, 1.0); // warm up
    let runs = 5;
    let t = Instant::now();
    let mut h = 0;
    for _ in 0..runs {
        h ^= render(list, &NoGlyphs, 1.0).hash();
    }
    let ms = t.elapsed().as_secs_f64() * 1000.0 / runs as f64;
    println!("{name:<44} {ms:8.1} ms   (hash {h:016x})");
}

fn main() {
    let mut rng = Rng::new(7);
    let colour = |rng: &mut Rng| Color::rgba(rng.next_f64() as f32, rng.next_f64() as f32, rng.next_f64() as f32, 0.85);

    // 5,000 filled paths of mixed kinds and sizes on a 1000×1000 frame.
    let mut ops = Vec::new();
    for i in 0..5000 {
        let (x, y) = (rng.next_f64() * 1000.0, rng.next_f64() * 1000.0);
        let s = 5.0 + rng.next_f64() * 45.0;
        let path = match i % 4 {
            0 => PathData::rect(Rect::new(x, y, s, s * 0.7)),
            1 => PathData::circle(Vec2::new(x, y), s * 0.5),
            2 => PathData::annular_sector(Vec2::new(x, y), s * 0.3, s, 0.2, 2.0),
            _ => PathData::polygon(&[Vec2::new(x, y), Vec2::new(x + s, y + s * 0.3), Vec2::new(x + s * 0.2, y + s)]),
        };
        ops.push(Op::Fill { path: SharedPath::new(path), xf: Affine::IDENTITY, paint: DPaint::Solid(colour(&mut rng)), rule: FillRule::NonZero, opacity: 1.0 });
    }
    let fills = DisplayList { width: 1000.0, height: 1000.0, background: Color::WHITE, ops };
    time("5,000 filled paths, 1000×1000", &fills);

    // 1,000 stroked polylines of 200 points each (spaghetti lines).
    let mut ops = Vec::new();
    for _ in 0..1000 {
        let mut pts = Vec::new();
        let mut y = rng.next_f64() * 1000.0;
        for k in 0..200 {
            y = (y + (rng.next_f64() - 0.5) * 20.0).clamp(0.0, 1000.0);
            pts.push(Vec2::new(k as f64 * 5.0, y));
        }
        let style = StrokeStyle { width_px: 1.5, cap: Cap::Round, join: Join::Round, miter_limit: 4.0, dash: None };
        ops.push(Op::Stroke { path: SharedPath::new(PathData::polyline(&pts)), xf: Affine::IDENTITY, style, paint: DPaint::Solid(colour(&mut rng)), opacity: 1.0 });
    }
    let lines = DisplayList { width: 1000.0, height: 1000.0, background: Color::WHITE, ops };
    time("1,000 stroked 200-point lines, 1000×1000", &lines);

    // 100,000 circle instances.
    let n = 100_000;
    let x: Vec<f64> = (0..n).map(|_| rng.next_f64() * 1000.0).collect();
    let y: Vec<f64> = (0..n).map(|_| rng.next_f64() * 1000.0).collect();
    let fill: Vec<Color> = (0..n).map(|_| colour(&mut rng)).collect();
    let inst = |stroke| InstancesOp {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        xf: Affine::IDENTITY,
        x: x.clone().into(),
        y: y.clone().into(),
        size: vec![3.0; n].into(),
        w: None,
        h: None,
        fill: fill.clone().into(),
        opacity: vec![1.0; n].into(),
        alpha: 1.0,
        stroke,
        size_scale: 1.0,
    };
    let dots = DisplayList { width: 1000.0, height: 1000.0, background: Color::WHITE, ops: vec![Op::Instances(inst(None))] };
    time("100,000 circle instances (r = 3)", &dots);
    let ringed = DisplayList { width: 1000.0, height: 1000.0, background: Color::WHITE, ops: vec![Op::Instances(inst(Some((Color::BLACK, 0.5))))] };
    if std::env::args().any(|a| a == "--profile") {
        // A long run of one case, for a sampling profiler.
        for _ in 0..40 {
            let _ = render(&ringed, &NoGlyphs, 1.0);
        }
        return;
    }
    time("100,000 stroked circle instances", &ringed);

    // One full-frame gradient fill at 2× (a big single path).
    let paint = DPaint::Linear { p0: Vec2::new(0.0, 0.0), p1: Vec2::new(1000.0, 1000.0), stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)] };
    let big = DisplayList {
        width: 1000.0,
        height: 1000.0,
        background: Color::WHITE,
        ops: vec![Op::Fill { path: SharedPath::new(PathData::circle(Vec2::new(500.0, 500.0), 490.0)), xf: Affine::IDENTITY, paint, rule: FillRule::NonZero, opacity: 1.0 }],
    };
    time("full-frame gradient disc, 1000×1000", &big);
}
