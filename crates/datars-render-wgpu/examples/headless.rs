//! Render a small scene on the GPU, headless, to `out/gpu.png` at the repository root.
//!
//! ```sh
//! cargo run -p datars-render-wgpu --example headless
//! ```
//!
//! The scene goes through the real pipeline — `datars_scene` nodes with theme inks, flattened
//! against the built-in neutral theme — and exercises fills, rounded rects, dashed and solid
//! strokes, a gradient area, a radial gradient, clipping, an isolated (layered) group, SDF symbol
//! instances and glyphs with halos. Text layout here is a naive left-to-right placement over the
//! bundled Inter font (the real shaping lives in `datars-text`).

use datars_math::{PathData, Rect, Vec2};
use datars_render::GlyphSource;
use datars_render_wgpu::Renderer;
use datars_scene::text::TextStyle;
use datars_scene::{Cap, Clip, Curve, GlyphPos, Instances, Join, Key, Node, NodeKind, Paint, Proto, Scene, Stop, Stroke, SymbolKind, TextNode, TextRun};
use datars_theme::{resolve, Ink, Mode, ThemeSet};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::sync::Arc;

static INTER: &[u8] = include_bytes!("../../../fonts/Inter-Regular.ttf");

/// Glyph outlines from the bundled font (y down, baseline origin, px).
struct Inter;

struct Pen(PathData);
impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(Vec2::new(x as f64, -y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(Vec2::new(x as f64, -y as f64));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.0.quad_to(Vec2::new(cx as f64, -cy as f64), Vec2::new(x as f64, -y as f64));
    }
    fn curve_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.0.cubic_to(Vec2::new(c1x as f64, -c1y as f64), Vec2::new(c2x as f64, -c2y as f64), Vec2::new(x as f64, -y as f64));
    }
    fn close(&mut self) {
        self.0.close();
    }
}

impl GlyphSource for Inter {
    fn outline(&self, _font: &str, glyph: u16, size: f64) -> Option<PathData> {
        let font = FontRef::new(INTER).ok()?;
        let g = font.outline_glyphs().get(GlyphId::new(glyph as u32))?;
        let mut pen = Pen(PathData::new());
        g.draw(DrawSettings::unhinted(Size::new(size as f32), LocationRef::default()), &mut pen).ok()?;
        Some(pen.0)
    }
}

/// A text node with naively placed glyphs (no kerning or shaping).
fn text(key: &str, s: &str, origin: Vec2, size: f64, ink: Ink, halo: Option<(Ink, f64)>) -> Node {
    let font = FontRef::new(INTER).expect("bundled font");
    let (cmap, metrics) = (font.charmap(), font.glyph_metrics(Size::new(size as f32), LocationRef::default()));
    let mut x = 0.0f32;
    let glyphs: Vec<GlyphPos> = s
        .chars()
        .map(|ch| {
            let id = cmap.map(ch).unwrap_or_default();
            let g = GlyphPos { id: id.to_u32() as u16, x, y: 0.0 };
            x += metrics.advance_width(id).unwrap_or(0.0);
            g
        })
        .collect();
    let style = TextStyle { size, ink: ink.clone(), ..TextStyle::default() };
    let mut t = TextNode::new(s, origin, style);
    t.runs = vec![TextRun { font: Arc::from("Inter-400"), size, ink, glyphs: glyphs.into() }];
    t.halo = halo;
    Node::text(Key::one(key), t)
}

fn shape(key: &str, geom: datars_scene::Geom) -> Node {
    Node::shape(Key::one(key), geom)
}

fn scene(w: f64, h: f64) -> Scene {
    let plot = Rect::new(48.0, 56.0, 404.0, 196.0);
    let y_of = |v: f64| plot.y + plot.h - v / 100.0 * plot.h;
    let mut marks = Vec::new();

    // Dashed guides.
    for (i, v) in [25.0, 50.0, 75.0, 100.0].iter().enumerate() {
        let y = y_of(*v);
        let mut s = Stroke::new(Paint::token("grid"), 1.0);
        s.dash = Some(vec![4.0, 3.0]);
        marks.push(shape(&format!("guide{i}"), datars_scene::Geom::Segment { x1: plot.x, y1: y, x2: plot.x + plot.w, y2: y }).stroke(s));
    }
    // Columns with rounded tops, palette colours.
    let values = [42.0, 67.0, 55.0, 88.0, 73.0, 36.0, 61.0, 80.0];
    let bw = plot.w / values.len() as f64;
    for (i, v) in values.iter().enumerate() {
        let x = plot.x + bw * i as f64 + bw * 0.18;
        let top = y_of(*v);
        let r = datars_scene::Geom::Rect { x, y: top, w: bw * 0.64, h: plot.y + plot.h - top, r: [4.0, 4.0, 0.0, 0.0] };
        marks.push(shape(&format!("col{i}"), r).fill(Paint::Solid(Ink::palette("categorical", i as u32))).opacity(0.85));
    }
    // A gradient area under a smooth curve that overshoots the plot (clipped).
    let pts: Vec<Vec2> = (0..=20)
        .map(|i| {
            let t = i as f64 / 20.0;
            let v = 30.0 + 60.0 * datars_math::m::sin(t * 7.0).abs() * (1.0 - t * 0.4) + t * 25.0;
            Vec2::new(plot.x - 20.0 + (plot.w + 40.0) * t, y_of(v))
        })
        .collect();
    let base: Vec<Vec2> = pts.iter().map(|p| Vec2::new(p.x, plot.y + plot.h)).collect();
    let area = datars_scene::Geom::Area { top: pts.clone().into(), base: base.into(), curve: Curve::MonotoneX };
    let fade = Paint::Linear {
        linear: [0.0, plot.y, 0.0, plot.y + plot.h],
        stops: vec![Stop { at: 0.0, ink: Ink::token("accent").fade(0.45) }, Stop { at: 1.0, ink: Ink::token("accent").fade(0.0) }],
    };
    marks.push(shape("area", area).fill(fade));
    let mut line = Stroke::new(Paint::token("accent"), 2.5);
    line.join = Join::Round;
    line.cap = Cap::Round;
    marks.push(shape("line", datars_scene::Geom::Polyline { pts: pts.clone().into(), closed: false, curve: Curve::MonotoneX }).stroke(line));
    // Scatter: SDF circles with an outline.
    let n = 60;
    let (xs, ys): (Vec<f64>, Vec<f64>) = (0..n)
        .map(|i| {
            let t = i as f64 / n as f64;
            (plot.x + 10.0 + (plot.w - 20.0) * t, y_of(20.0 + 30.0 * (1.0 + datars_math::m::sin(i as f64 * 1.7)) * 0.5 + 25.0 * t))
        })
        .unzip();
    let scatter = Instances {
        proto: Proto::Symbol { symbol: SymbolKind::Circle },
        keys: (0..n).map(|i| Key::one(i as i64)).collect(),
        x: xs,
        y: ys,
        size: (0..n).map(|i| 2.5 + (i % 4) as f64).collect(),
        w: None,
        h: None,
        fill: vec![Ink::palette("categorical", 2)],
        opacity: vec![0.85; n],
        stroke: Some(Stroke::new(Paint::token("paper"), 1.0)),
        screen_size: true,
        labels: None,
        line_reach: None,
    };
    marks.push(Node::new(Key::name("scatter"), NodeKind::Instances(std::sync::Arc::new(scatter))));
    let mut plot_group = Node::group(Key::name("plot"), marks);
    plot_group.common.clip = Some(Clip::Rect { rect: plot });

    // Axis rule and a label with a halo sitting on the curve.
    let mut rule = Stroke::new(Paint::token("rule"), 1.0);
    rule.cap = Cap::Square;
    let axis = shape("rule", datars_scene::Geom::Segment { x1: plot.x, y1: plot.y + plot.h, x2: plot.x + plot.w, y2: plot.y + plot.h }).stroke(rule);
    let peak = pts[4];
    let label = text("peak", "peak", Vec2::new(peak.x - 12.0, peak.y + 4.0), 12.0, Ink::token("ink"), Some((Ink::token("paper"), 2.5)));

    // Symbols of every kind, and an isolated group (exact group opacity via a layer).
    let kinds = [SymbolKind::Circle, SymbolKind::Square, SymbolKind::Diamond, SymbolKind::Triangle, SymbolKind::Cross, SymbolKind::Star];
    let sym = Instances {
        proto: Proto::Symbol { symbol: SymbolKind::Star },
        keys: vec![Key::one(0)],
        x: vec![0.0],
        y: vec![0.0],
        size: vec![6.0],
        w: None,
        h: None,
        fill: vec![Ink::token("highlight")],
        opacity: vec![1.0],
        stroke: Some(Stroke::new(Paint::token("ink"), 1.0)),
        screen_size: true,
        labels: None,
        line_reach: None,
    };
    let symbols: Vec<Node> = kinds
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let mut s = sym.clone();
            s.proto = Proto::Symbol { symbol: *k };
            s.x = vec![300.0 + 24.0 * i as f64];
            s.y = vec![28.0];
            s.fill = vec![Ink::palette("categorical", i as u32)];
            Node::new(Key::one(format!("sym{i}")), NodeKind::Instances(std::sync::Arc::new(s)))
        })
        .collect();
    let blob = |k: &str, cx: f64, i: u32| shape(k, datars_scene::Geom::circle(cx, 278.0, 12.0)).fill(Paint::Solid(Ink::palette("categorical", i)));
    let mut layered = Node::group(Key::name("layered"), vec![blob("b0", 400.0, 0), blob("b1", 414.0, 2), blob("b2", 428.0, 4)]).opacity(0.6);
    layered.common.isolate = true;
    let glow = shape("glow", datars_scene::Geom::circle(360.0, 278.0, 14.0)).fill(Paint::Radial {
        radial: [356.0, 274.0, 16.0],
        stops: vec![Stop { at: 0.0, ink: Ink::token("highlight") }, Stop { at: 1.0, ink: Ink::token("negative") }],
    });
    let title = text("title", "GPU backend: wgpu, headless", Vec2::new(20.0, 34.0), 17.0, Ink::token("ink"), None);
    let caption = text("caption", "fills, strokes, gradients, clips, layers, instances, glyphs", Vec2::new(20.0, 283.0), 11.0, Ink::token("muted"), None);
    let mut rotated = text("rotated", "rotated", Vec2::new(30.0, 200.0), 11.0, Ink::token("ink-2"), None);
    if let NodeKind::Text(t) = &mut rotated.kind {
        t.rotate = -datars_math::m::PI / 2.0;
    }

    let mut children = vec![plot_group, axis, label, title, caption, rotated, glow, layered];
    children.extend(symbols);
    let mut s = Scene::new(w, h, Node::group(Key::name("root"), children));
    s.background = Ink::token("paper");
    s
}

fn main() {
    let (w, h, dpr) = (480.0, 300.0, 2.0);
    let set = ThemeSet::with_builtins();
    let (theme, _) = resolve(&set.chain("datars/neutral").expect("neutral theme"), Mode::Light, &[]);
    let list = datars_render::flatten(&scene(w, h), &theme);
    let mut r = match Renderer::headless((w * dpr) as u32, (h * dpr) as u32, dpr) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("cannot render on the GPU here: {e}");
            std::process::exit(1);
        }
    };
    r.render(&list, &Inter).expect("render");
    let rgba = r.read_rgba();
    let (pw, ph) = r.size();
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../out/gpu.png");
    std::fs::create_dir_all(out.parent().expect("parent")).expect("create out/");
    let file = std::fs::File::create(&out).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), pw, ph);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().and_then(|mut wr| wr.write_image_data(&rgba)).expect("write png");
    let out = out.canonicalize().unwrap_or(out);
    let s = r.stats();
    println!(
        "{}: {}x{} px, {} ops → {} draws in {} passes, {} meshes tessellated, {} samples/px → {}",
        r.backend(),
        pw,
        ph,
        list.ops.len(),
        s.draws,
        s.passes,
        s.tessellations,
        r.samples(),
        out.display()
    );
}
