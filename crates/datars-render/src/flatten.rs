use crate::{trim_path, DPaint, DisplayList, InstancesOp, Op, SharedPath, StrokeStyle};
use datars_color::Color;
use datars_math::{Affine, FillRule, PathData, Rect, Vec2};
use datars_scene::{Clip, Geom, Marker, Markers, Node, NodeKind, Paint, Scene, Stroke};
use datars_theme::ResolvedTheme;
use std::sync::Arc;

struct Cx<'a> {
    theme: &'a ResolvedTheme,
    ops: Vec<Op>,
    cache: Option<&'a mut FlattenCache>,
    /// Floating nodes met on the way ([`datars_scene::FLOAT_Z`]), with their parent's transform and
    /// opacity: drawn after everything else.
    floating: Vec<(Node, Affine, f64)>,
}

/// Flatten a scene into a display list, resolving inks against `theme`.
pub fn flatten(scene: &Scene, theme: &ResolvedTheme) -> DisplayList {
    let mut cx = Cx { theme, ops: Vec::new(), cache: None, floating: Vec::new() };
    node(&mut cx, &scene.root, Affine::IDENTITY, 1.0);
    draw_floating(&mut cx);
    DisplayList { width: scene.width, height: scene.height, background: scene.background.resolve(theme), ops: cx.ops }
}

/// The floating nodes, above everything: by `z`, then tree order (floating nodes inside them after).
fn draw_floating(cx: &mut Cx) {
    while !cx.floating.is_empty() {
        let mut all = std::mem::take(&mut cx.floating);
        all.sort_by_key(|(n, _, _)| n.common.z);
        for (n, xf, opacity) in &all {
            node(cx, n, *xf, *opacity);
        }
    }
}

/// Big instance sets flattened in earlier frames, by identity. A set that didn't change — the
/// same `Arc<Instances>`, whatever the opacity of the groups around it (a fade is `alpha`) —
/// reuses its converted columns instead of resolving
/// every ink and copying every column again: 400,000 dots on a map, every frame of a camera move,
/// were most of such a frame's flatten. Renderers key their uploaded instance buffers on these
/// columns' identity, so they keep them too. The inks were resolved in one theme: `clear` the
/// cache when it changes. Sets unused for a couple of flattens are dropped.
#[derive(Default)]
pub struct FlattenCache {
    sets: std::collections::BTreeMap<usize, CachedSet>,
    generation: u64,
    converted: u64,
}

struct CachedSet {
    /// Holds the set: its address can't be reused while it's a key.
    set: Arc<datars_scene::Instances>,
    x: Arc<[f64]>,
    y: Arc<[f64]>,
    size: Arc<[f64]>,
    w: Option<Arc<[f64]>>,
    h: Option<Arc<[f64]>>,
    fill: Arc<[Color]>,
    opacity: Arc<[f32]>,
    last: u64,
}

impl FlattenCache {
    /// Forget everything (the theme changed: the inks resolve differently).
    pub fn clear(&mut self) {
        self.sets.clear();
    }
    /// Instances of big sets converted since creation (not kept from an earlier frame): a set
    /// converted every frame of a transition is work a profile should show, whatever the machine.
    pub fn converted(&self) -> u64 {
        self.converted
    }
}

/// Instance sets at least this big are kept in a [`FlattenCache`] (a point pyramid's cells are a
/// few dozen rows and up; renderers keep sets from this size too).
const CACHE_MIN: usize = 32;

/// [`flatten`], reusing the columns of instance sets unchanged since an earlier call.
pub fn flatten_cached(scene: &Scene, theme: &ResolvedTheme, cache: &mut FlattenCache) -> DisplayList {
    cache.generation += 1;
    let generation = cache.generation;
    let mut cx = Cx { theme, ops: Vec::new(), cache: Some(cache), floating: Vec::new() };
    node(&mut cx, &scene.root, Affine::IDENTITY, 1.0);
    draw_floating(&mut cx);
    let ops = cx.ops;
    if let Some(c) = cx.cache {
        c.sets.retain(|_, s| s.last + 2 >= generation);
    }
    DisplayList { width: scene.width, height: scene.height, background: scene.background.resolve(theme), ops }
}

fn paint(cx: &Cx, p: &Paint) -> DPaint {
    match p {
        Paint::Solid(i) => DPaint::Solid(i.resolve(cx.theme)),
        Paint::Linear { linear, stops } => DPaint::Linear {
            p0: Vec2::new(linear[0], linear[1]),
            p1: Vec2::new(linear[2], linear[3]),
            stops: stops.iter().map(|s| (s.at as f32, s.ink.resolve(cx.theme))).collect(),
        },
        Paint::Radial { radial, stops } => DPaint::Radial {
            c: Vec2::new(radial[0], radial[1]),
            r: radial[2],
            stops: stops.iter().map(|s| (s.at as f32, s.ink.resolve(cx.theme))).collect(),
        },
    }
}

fn stroke_style(s: &Stroke, xf: &Affine) -> StrokeStyle {
    let k = if s.non_scaling { 1.0 } else { xf.scale_factor() };
    StrokeStyle {
        width_px: s.width * k,
        cap: s.cap,
        join: s.join,
        miter_limit: 4.0,
        dash: s.dash.as_ref().map(|d| d.iter().map(|v| v * k).collect()),
    }
}

fn node(cx: &mut Cx, n: &Node, parent: Affine, parent_opacity: f64) {
    let c = &n.common;
    if !c.visible || c.opacity <= 0.0 {
        return;
    }
    let xf = c.placed(parent);
    let (opacity, layered) = if c.isolate && c.opacity < 1.0 {
        cx.ops.push(Op::PushLayer { opacity: c.opacity as f32, blend: c.blend });
        (parent_opacity, true)
    } else {
        (parent_opacity * c.opacity, false)
    };
    let mut clips = 0;
    if let Some(clip) = &c.clip {
        let path = match clip {
            Clip::Rect { rect } => PathData::rect(*rect),
            Clip::Path { path } => (**path).clone(),
        };
        cx.ops.push(Op::PushClip { path: SharedPath::new(path), xf, rule: FillRule::NonZero });
        clips += 1;
    }
    match &n.kind {
        NodeKind::Group { children } => children_in_z_order(cx, children, xf, opacity),
        NodeKind::View { viewport, camera, clip, children } => {
            if *clip {
                cx.ops.push(Op::PushClip { path: SharedPath::new(PathData::rect(*viewport)), xf, rule: FillRule::NonZero });
                clips += 1;
            }
            let cam = camera.map(|cm| cm.transform(*viewport)).unwrap_or(Affine::translate(viewport.x, viewport.y));
            children_in_z_order(cx, children, xf.mul(cam), opacity);
        }
        NodeKind::Shape { geom, fill, stroke, markers } => shape(cx, geom, fill.as_ref(), stroke.as_ref(), markers.as_ref(), c.trim, xf, opacity),
        NodeKind::Text(t) => {
            if !t.runs.is_empty() {
                let scale = if t.screen_size { 1.0 } else { xf.scale_factor() };
                let origin = xf.apply(t.origin) + t.offset * scale;
                let halo = t.halo.as_ref().map(|(i, w)| (i.resolve(cx.theme).fade(opacity as f32), *w));
                for (i, r) in t.runs.iter().enumerate() {
                    let color = r.ink.resolve(cx.theme).fade(opacity as f32);
                    let text = (i == 0 && !t.text.is_empty()).then(|| Arc::<str>::from(t.text.as_ref()));
                    cx.ops.push(Op::Glyphs { font: r.font.clone(), size: r.size, glyphs: r.glyphs.clone(), origin, scale, rotate: t.rotate, color, halo, text });
                }
            }
        }
        NodeKind::Image { asset, rect } => cx.ops.push(Op::Image { asset: asset.clone(), rect: *rect, xf, opacity: opacity as f32 }),
        NodeKind::Instances(i) => {
            if !i.is_empty() {
                let theme = cx.theme;
                let convert = || CachedSet {
                    set: i.clone(),
                    x: i.x.clone().into(),
                    y: i.y.clone().into(),
                    size: (0..i.len()).map(|k| i.size_at(k)).collect(),
                    w: i.w.clone().map(Into::into),
                    h: i.h.clone().map(Into::into),
                    fill: (0..i.len()).map(|k| i.fill_at(k).resolve(theme)).collect(),
                    opacity: (0..i.len()).map(|k| i.opacity_at(k) as f32).collect(),
                    last: 0,
                };
                let key = Arc::as_ptr(i) as usize;
                let cols = match cx.cache.as_deref_mut() {
                    Some(cache) if i.len() >= CACHE_MIN => {
                        let generation = cache.generation;
                        let mut converted = 0;
                        let e = cache.sets.entry(key).or_insert_with(|| {
                            converted = i.len();
                            convert()
                        });
                        if !Arc::ptr_eq(&e.set, i) {
                            *e = convert();
                            converted = i.len();
                        }
                        e.last = generation;
                        let cols = (e.x.clone(), e.y.clone(), e.size.clone(), e.w.clone(), e.h.clone(), e.fill.clone(), e.opacity.clone());
                        cache.converted += converted as u64;
                        cols
                    }
                    _ => {
                        let c = convert();
                        (c.x, c.y, c.size, c.w, c.h, c.fill, c.opacity)
                    }
                };
                let (x, y, size, w, h, fill, op) = cols;
                cx.ops.push(Op::Instances(InstancesOp {
                    proto: i.proto.clone(),
                    xf,
                    x,
                    y,
                    size,
                    w,
                    h,
                    fill,
                    opacity: op,
                    alpha: opacity as f32,
                    stroke: i.stroke.as_ref().map(|s| (paint(cx, &s.paint).representative(), s.width)),
                    size_scale: if i.screen_size { 1.0 } else { xf.scale_factor() },
                }));
            }
        }
    }
    for _ in 0..clips {
        cx.ops.push(Op::PopClip);
    }
    if layered {
        cx.ops.push(Op::PopLayer);
    }
}

fn children_in_z_order(cx: &mut Cx, children: &[Node], xf: Affine, opacity: f64) {
    if children.iter().any(|c| c.common.z != 0) {
        let mut order: Vec<&Node> = children.iter().collect();
        order.sort_by_key(|c| c.common.z); // stable: tree order within a z level
        for c in order {
            if c.common.floats() {
                cx.floating.push((c.clone(), xf, opacity));
            } else {
                node(cx, c, xf, opacity);
            }
        }
    } else {
        for c in children {
            node(cx, c, xf, opacity);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn shape(cx: &mut Cx, geom: &Geom, fill: Option<&Paint>, stroke: Option<&Stroke>, markers: Option<&Markers>, trim: Option<[f64; 2]>, xf: Affine, opacity: f64) {
    let (shared, trimmed) = match (geom, trim) {
        (_, Some([a, b])) if a > 0.0 || b < 1.0 => (SharedPath::new(trim_path(&geom.to_path(), a, b, 0.25 / xf.scale_factor().max(1e-6))), true),
        // A shared path stays shared: no copy, and its hash is remembered across frames.
        (Geom::Path { path }, _) => (SharedPath::from_shared(path), false),
        _ => (SharedPath::new(geom.to_path()), false),
    };
    if let Some(f) = fill {
        if geom.is_closed() && !trimmed {
            cx.ops.push(Op::Fill { path: shared.clone(), xf, paint: paint(cx, f), rule: FillRule::NonZero, opacity: opacity as f32 });
        }
    }
    if let Some(s) = stroke {
        if s.width > 0.0 {
            cx.ops.push(Op::Stroke { path: shared.clone(), xf, style: stroke_style(s, &xf), paint: paint(cx, &s.paint), opacity: opacity as f32 });
            if let Some(m) = markers {
                markers_ops(cx, &shared.path, m, s, xf, opacity);
            }
        }
    }
}

/// Arrowheads and dots at the ends of a stroked path, in device space.
fn markers_ops(cx: &mut Cx, path: &PathData, m: &Markers, s: &Stroke, xf: Affine, opacity: f64) {
    let mut polys = path.transform(&xf).flatten(0.2);
    if polys.is_empty() {
        // A path of a single point (flattening drops it): its dots still have a place.
        if let Some(datars_math::path::PathEl::Move { p }) = path.els.first() {
            polys.push((vec![xf.apply(*p)], false));
        }
    }
    let color = paint(cx, &s.paint);
    let Some((first, _)) = polys.first() else { return };
    let Some((last, _)) = polys.last() else { return };
    let mut emit = |tip: Vec2, from: Vec2, mk: &Marker| {
        let d = (tip - from).normalize();
        let p = match *mk {
            Marker::Arrow { size } => {
                let side = d.perp() * (size * 0.5);
                PathData::polygon(&[tip, tip - d * size + side, tip - d * size - side])
            }
            Marker::Dot { r } => PathData::circle(tip, r),
        };
        cx.ops.push(Op::Fill { path: SharedPath::new(p), xf: Affine::IDENTITY, paint: color.clone(), rule: FillRule::NonZero, opacity: opacity as f32 });
    };
    // An arrow needs a direction (two points); a dot doesn't — a track's head shows before the
    // track has moved (a path of one point).
    let needs = |mk: &Marker| if matches!(mk, Marker::Dot { .. }) { 1 } else { 2 };
    if let Some(mk) = &m.start {
        if first.len() >= needs(mk) {
            emit(first[0], first.get(1).copied().unwrap_or(first[0]), mk);
        }
    }
    if let Some(mk) = &m.end {
        if last.len() >= needs(mk) {
            emit(last[last.len() - 1], last[last.len().saturating_sub(2)], mk);
        }
    }
}

/// Device-space bounds of a display list's drawn geometry (for tests and culling).
pub fn bounds(list: &DisplayList) -> Rect {
    let mut r = Rect::empty();
    for op in &list.ops {
        if let Op::Fill { path, xf, .. } | Op::Stroke { path, xf, .. } = op {
            r = r.union(&path.path.transform(xf).bounds());
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_scene::{Key, Node};
    use datars_theme::{resolve, Mode, ThemeSet};

    #[test]
    fn inks_resolve_and_transforms_accumulate() {
        let set = ThemeSet::with_builtins();
        let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]);
        let bar = Node::shape(Key::one("a"), Geom::rect(0.0, 0.0, 10.0, 10.0)).fill(Paint::token("accent"));
        let root = Node::group(Key::name("root"), vec![bar]).transform(Affine::translate(5.0, 5.0)).opacity(0.5);
        let list = flatten(&Scene::new(100.0, 100.0, root), &theme);
        assert_eq!(list.ops.len(), 1);
        match &list.ops[0] {
            Op::Fill { xf, paint: DPaint::Solid(c), opacity, .. } => {
                assert_eq!(*xf, Affine::translate(5.0, 5.0));
                assert_eq!(c.to_hex(), "#4269d0");
                assert_eq!(*opacity, 0.5);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(list.background.to_hex(), "#ffffff");
        let (dark, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Dark, &[]);
        let dl = flatten(&Scene::new(100.0, 100.0, Node::group(Key::name("r"), vec![])), &dark);
        assert_eq!(dl.background.to_hex(), "#111318", "same scene, dark theme: no re-resolve");
        assert_ne!(list.hash(), dl.hash());
    }

    #[test]
    fn unchanged_instance_sets_keep_their_flattened_columns() {
        // A dot map during a camera move: the same set every frame keeps its converted columns
        // (the same allocations: renderers key their uploads on them) and draws the same, and so
        // does a set fading in (the groups' opacity is the op's `alpha`); another set or a cleared
        // cache (a new theme) converts again.
        let set = ThemeSet::with_builtins();
        let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]);
        let n = 1000;
        let dots = std::sync::Arc::new(datars_scene::Instances {
            proto: datars_scene::Proto::Symbol { symbol: datars_scene::SymbolKind::Circle },
            keys: (0..n).map(|i| Key::new(vec![datars_scene::KeyPart::Int(i as i64)])).collect(),
            x: (0..n).map(|i| i as f64).collect(),
            y: vec![0.0; n],
            size: vec![1.0; n],
            w: None,
            h: None,
            fill: vec![datars_theme::Ink::token("accent"); n],
            opacity: vec![1.0; n],
            stroke: None,
            screen_size: true,
            labels: None,
            line_reach: None,
        });
        let scene = |opacity: f64, d: &std::sync::Arc<datars_scene::Instances>| {
            let n = Node::new(Key::name("dots"), NodeKind::Instances(d.clone()));
            Scene::new(100.0, 100.0, Node::group(Key::name("root"), vec![n]).opacity(opacity))
        };
        let cols = |l: &DisplayList| match &l.ops[0] {
            Op::Instances(i) => (i.x.as_ptr() as usize, i.fill.as_ptr() as usize, i.opacity_at(0), i.fill[0]),
            other => panic!("{other:?}"),
        };
        let mut cache = FlattenCache::default();
        let a = flatten_cached(&scene(1.0, &dots), &theme, &mut cache);
        let b = flatten_cached(&scene(1.0, &dots), &theme, &mut cache);
        assert_eq!(cols(&a), cols(&b), "kept");
        assert_eq!(a, flatten(&scene(1.0, &dots), &theme), "and the same as flattening afresh");
        assert_eq!(cache.converted(), n as u64, "converted once");
        let faded = flatten_cached(&scene(0.5, &dots), &theme, &mut cache);
        assert_eq!(cache.converted(), n as u64, "a fade converts nothing");
        assert_eq!(cols(&faded).2, 0.5, "drawn at the groups' opacity");
        assert_eq!(cols(&faded).0, cols(&a).0, "from the same columns: a fade converts nothing");
        let other = std::sync::Arc::new((*dots).clone());
        assert_ne!(cols(&flatten_cached(&scene(1.0, &other), &theme, &mut cache)).0, cols(&a).0, "another set");
        cache.clear();
        let (dark, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Dark, &[]);
        let d = flatten_cached(&scene(1.0, &dots), &dark, &mut cache);
        assert_eq!(cols(&d).3, flatten(&scene(1.0, &dots), &dark).ops.iter().find_map(|o| if let Op::Instances(i) = o { Some(i.fill[0]) } else { None }).unwrap(), "a new theme's inks after a clear");
    }

    #[test]
    fn a_dot_marker_rides_the_end_of_a_trimmed_line_and_marks_a_lone_point() {
        let set = ThemeSet::with_builtins();
        let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]);
        let stroke = Stroke { paint: Paint::token("accent"), width: 2.0, dash: None, cap: datars_scene::Cap::Round, join: datars_scene::Join::Round, non_scaling: false };
        let head = Markers { start: None, end: Some(Marker::Dot { r: 5.0 }) };
        let dot_at = |pts: Vec<Vec2>, trim: Option<[f64; 2]>| -> Option<Vec2> {
            let mut n = Node::new(Key::name("track"), NodeKind::Shape { geom: Geom::Polyline { pts: pts.into(), closed: false, curve: Default::default() }, fill: None, stroke: Some(stroke.clone()), markers: Some(head) });
            n.common.trim = trim;
            let list = flatten(&Scene::new(200.0, 200.0, n), &theme);
            list.ops.iter().find_map(|op| match op { Op::Fill { path, .. } => Some(path.path.bounds().center()), _ => None })
        };
        let line = vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0), Vec2::new(100.0, 100.0)];
        let c = dot_at(line.clone(), None).expect("a dot at the end");
        assert!(c.dist(Vec2::new(100.0, 100.0)) < 1e-6);
        let c = dot_at(line, Some([0.0, 0.25])).expect("a dot at the trimmed end");
        assert!(c.dist(Vec2::new(50.0, 0.0)) < 1e-6, "{c:?}");
        let c = dot_at(vec![Vec2::new(30.0, 40.0)], None).expect("a lone point still gets its dot");
        assert!(c.dist(Vec2::new(30.0, 40.0)) < 1e-6);
    }

    #[test]
    fn text_offsets_stay_in_screen_px_under_zoom() {
        let set = ThemeSet::with_builtins();
        let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]);
        let mut t = datars_scene::TextNode::new("a", Vec2::new(10.0, 10.0), datars_scene::text::TextStyle::default());
        t.runs.push(datars_scene::TextRun { font: std::sync::Arc::from("Inter-400"), size: 12.0, ink: datars_theme::Ink::token("ink"), glyphs: std::sync::Arc::from(vec![datars_scene::GlyphPos { id: 1, x: 0.0, y: 0.0 }]) });
        t.offset = Vec2::new(0.0, -8.0);
        let root = Node::text(Key::one("label"), t).transform(Affine::scale(100.0, 100.0));
        let list = flatten(&Scene::new(100.0, 100.0, root), &theme);
        let Op::Glyphs { origin, .. } = &list.ops[0] else { panic!("{:?}", list.ops) };
        assert_eq!(*origin, Vec2::new(1000.0, 992.0), "the origin zooms, the offset doesn't");
    }

    #[test]
    fn a_pinned_node_follows_the_camera_but_keeps_screen_size() {
        let set = ThemeSet::with_builtins();
        let (theme, _) = resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]);
        let dot = |pin: bool| {
            let mut g = Node::group(Key::name("callout"), vec![Node::shape(Key::one("dot"), Geom::circle(0.0, 0.0, 3.0)).fill(Paint::token("ink"))]).transform(Affine::translate(20.0, 10.0));
            g.common.pin = pin;
            g
        };
        // A view zoomed 10× on content point (20, 10).
        let cam = datars_scene::Camera { x: 20.0, y: 10.0, zoom: 10.0, rotation: 0.0 };
        let view = |pin: bool| Node::new(Key::name("v"), NodeKind::View { viewport: Rect::new(0.0, 0.0, 200.0, 100.0), camera: Some(cam), clip: false, children: vec![dot(pin)] });
        let xf_of = |pin: bool| match &flatten(&Scene::new(200.0, 100.0, view(pin)), &theme).ops[0] {
            Op::Fill { xf, .. } => *xf,
            other => panic!("{other:?}"),
        };
        let scaled = xf_of(false);
        assert!((scaled.scale_factor() - 10.0).abs() < 1e-9, "unpinned content zooms with the camera");
        let pinned = xf_of(true);
        assert_eq!(pinned, Affine::translate(100.0, 50.0), "pinned: at the camera-placed origin (the view centre), unscaled");
    }
}
