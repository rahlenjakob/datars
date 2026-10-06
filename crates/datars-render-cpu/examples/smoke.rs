//! Smoke test for the whole path scene → display list → pixels / SVG:
//! `cargo run -p datars-render-cpu --example smoke` writes `out/smoke.png` and `out/smoke.svg`.

use datars_math::{Affine, Rect, Vec2};
use datars_render::NoGlyphs;
use datars_scene::{Cap, Clip, Curve, Geom, Instances, Join, Key, Node, NodeKind, Paint, Proto, Scene, Stop, Stroke, SymbolKind};
use datars_theme::{resolve, Ink, Mode, ThemeSet};

fn scene() -> Scene {
    let mut nodes = Vec::new();

    // Rounded rects in palette colours, on a light grid of lines.
    for i in 0..=4 {
        let y = 40.0 + i as f64 * 50.0;
        nodes.push(Node::shape(Key::new(vec!["grid".into(), (i as i64).into()]), Geom::Segment { x1: 20.0, y1: y, x2: 460.0, y2: y }).stroke(Stroke::new(Paint::token("grid"), 1.0)));
    }
    for (i, h) in [120.0, 170.0, 90.0, 140.0].iter().enumerate() {
        let x = 40.0 + i as f64 * 50.0;
        let r = Geom::Rect { x, y: 240.0 - h, w: 36.0, h: *h, r: [4.0, 4.0, 0.0, 0.0] };
        nodes.push(Node::shape(Key::one(i as i64), r).fill(Paint::Solid(Ink::palette("categorical", i as u32))));
    }

    // A gradient-filled disc and a ring of annular sectors.
    let grad = Paint::Radial { radial: [300.0, 90.0, 45.0], stops: vec![Stop { at: 0.0, ink: Ink::token("paper") }, Stop { at: 1.0, ink: Ink::token("accent") }] };
    nodes.push(Node::shape(Key::one("disc"), Geom::circle(300.0, 90.0, 45.0)).fill(grad));
    let mut a0 = 0.0;
    for (i, share) in [0.4, 0.25, 0.2, 0.15].iter().enumerate() {
        let a1 = a0 + share * datars_math::m::TAU;
        let wedge = Geom::Arc { cx: 400.0, cy: 90.0, r0: 22.0, r1: 45.0, a0, a1 };
        let stroke = Stroke { join: Join::Round, ..Stroke::new(Paint::token("paper"), 1.5) };
        nodes.push(Node::shape(Key::one(format!("w{i}")), wedge).fill(Paint::Solid(Ink::palette("categorical", i as u32 + 4))).stroke(stroke));
        a0 = a1;
    }

    // An area with a vertical linear gradient, and a smooth line over it.
    let pts: Vec<Vec2> = (0..9).map(|i| Vec2::new(250.0 + i as f64 * 25.0, 200.0 - 40.0 * datars_math::m::sin(i as f64 * 0.8) - i as f64 * 4.0)).collect();
    let base: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.x, 250.0)).collect();
    let fade = Paint::Linear { linear: [0.0, 150.0, 0.0, 250.0], stops: vec![Stop { at: 0.0, ink: Ink::parse("$accent@0.5").unwrap() }, Stop { at: 1.0, ink: Ink::parse("$accent@0").unwrap() }] };
    nodes.push(Node::shape(Key::one("area"), Geom::Area { top: pts.clone().into(), base: base.into(), curve: Curve::MonotoneX }).fill(fade));
    let line = Stroke { cap: Cap::Round, join: Join::Round, ..Stroke::new(Paint::token("accent"), 2.5) };
    nodes.push(Node::shape(Key::one("line"), Geom::Polyline { pts: pts.clone().into(), closed: false, curve: Curve::MonotoneX }).stroke(line));

    // A dashed reference line, and dots inside a clipped view.
    let dashed = Stroke { dash: Some(vec![6.0, 4.0]), ..Stroke::new(Paint::token("negative"), 1.5) };
    nodes.push(Node::shape(Key::one("ref"), Geom::Segment { x1: 250.0, y1: 160.0, x2: 450.0, y2: 160.0 }).stroke(dashed));
    let n = 60;
    let dots = Instances {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        keys: (0..n).map(|i| Key::one(i as i64)).collect(),
        x: (0..n).map(|i| 20.0 + (i % 12) as f64 * 38.0).collect(),
        y: (0..n).map(|i| 262.0 + (i / 12) as f64 * 9.0).collect(),
        size: (0..n).map(|i| 1.5 + (i % 4) as f64).collect(),
        w: None,
        h: None,
        fill: (0..n).map(|i| Ink::palette("categorical", (i % 10) as u32)).collect(),
        opacity: (0..n).map(|i| if i % 3 == 0 { 0.5 } else { 1.0 }).collect(),
        stroke: Some(Stroke::new(Paint::token("ink"), 0.5)),
        screen_size: true,
        labels: None,
        line_reach: None,
    };
    let mut clipped = Node::new(Key::one("dots"), NodeKind::Instances(std::sync::Arc::new(dots)));
    clipped.common.clip = Some(Clip::Rect { rect: Rect::new(10.0, 255.0, 420.0, 40.0) });
    nodes.push(clipped);

    let root = Node::group(Key::name("root"), nodes).transform(Affine::translate(0.0, 0.0));
    Scene::new(480.0, 300.0, root)
}

fn main() {
    let set = ThemeSet::with_builtins();
    let (theme, _) = resolve(&set.chain("datars/neutral").expect("built-in theme"), Mode::Light, &[]);
    let list = datars_render::flatten(&scene(), &theme);
    let pixmap = datars_render_cpu::render(&list, &NoGlyphs, 2.0);
    let svg = datars_render_svg::to_svg(&list, &NoGlyphs);
    std::fs::create_dir_all("out").expect("create out/");
    std::fs::write("out/smoke.png", pixmap.to_png()).expect("write out/smoke.png");
    std::fs::write("out/smoke.svg", svg).expect("write out/smoke.svg");
    println!("{} ops → out/smoke.png ({}×{}, hash {:016x}) + out/smoke.svg", list.ops.len(), pixmap.width, pixmap.height, pixmap.hash());
}
