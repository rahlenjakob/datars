//! Per-property interpolation paths: lines, areas, images, gradients, strokes, symbols, views,
//! instance ghosts, partitions.

mod common;

use common::*;
use datars_math::{Rect, Vec2};
use datars_motion::outline::{morph_geoms, partition};
use datars_motion::*;
use datars_scene::*;
use datars_theme::Ink;

fn linear() -> MotionRules {
    MotionRules::new(vec![Rule::new().easing(Easing::Linear)])
}

fn one(n: Node) -> Scene {
    scene(vec![n])
}

fn mid_geom(a: Node, b: Node, t: f64) -> Geom {
    let k = a.key.clone();
    let p = plan(&one(a), &one(b), &linear(), &cx());
    let f = p.at(t, &NoShaper);
    geom(find(&f, &KeyPath(vec![key("root"), k])).unwrap()).clone()
}

#[test]
fn polylines_of_different_lengths_resample_point_wise() {
    let a = Node::shape(key("l"), Geom::polyline(vec![Vec2::new(0.0, 0.0), Vec2::new(100.0, 0.0)]));
    let b = Node::shape(key("l"), Geom::polyline(vec![Vec2::new(0.0, 50.0), Vec2::new(50.0, 100.0), Vec2::new(100.0, 50.0)]));
    let Geom::Polyline { pts, closed, .. } = mid_geom(a, b, 0.5) else { panic!() };
    assert!(!closed);
    assert_eq!(pts.len(), 3);
    assert_eq!(pts[0], Vec2::new(0.0, 25.0));
    assert_eq!(pts[1], Vec2::new(50.0, 50.0));
    assert_eq!(pts[2], Vec2::new(100.0, 25.0));
    // Same length: exact point-wise lerp.
    let a = Node::shape(key("l"), Geom::polyline(vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)]));
    let b = Node::shape(key("l"), Geom::polyline(vec![Vec2::new(0.0, 20.0), Vec2::new(10.0, 30.0)]));
    let Geom::Polyline { pts, .. } = mid_geom(a, b, 0.5) else { panic!() };
    assert_eq!(&pts[..], &[Vec2::new(0.0, 10.0), Vec2::new(10.0, 20.0)]);
}

#[test]
fn areas_interpolate_top_and_base() {
    let area = |top: Vec<Vec2>, y: f64| {
        let base: Vec<Vec2> = top.iter().map(|p| Vec2::new(p.x, y)).collect();
        Node::shape(key("a"), Geom::Area { top: top.into(), base: base.into(), curve: Curve::MonotoneX }).fill(Paint::token("accent"))
    };
    let a = area(vec![Vec2::new(0.0, 50.0), Vec2::new(100.0, 50.0)], 100.0);
    let b = area(vec![Vec2::new(0.0, 0.0), Vec2::new(50.0, 20.0), Vec2::new(100.0, 0.0)], 100.0);
    let Geom::Area { top, base, curve } = mid_geom(a, b, 0.5) else { panic!() };
    // Smooth edges of different lengths are sampled along their curves and drawn straight between
    // the samples (an inserted vertex would bend a monotone curve); every vertex is kept.
    assert_eq!(curve, Curve::Linear);
    assert_eq!(top.len(), base.len());
    assert!(top.iter().any(|p| (p.x - 50.0).abs() < 1e-9 && (p.y - 35.0).abs() < 1e-9), "b's middle vertex, halfway from a's top: {top:?}");
    assert!(base.iter().all(|p| p.y == 100.0));
}

#[test]
fn symbol_kind_change_morphs_and_same_kind_lerps() {
    let a = Node::shape(key("s"), Geom::Symbol { kind: SymbolKind::Circle, x: 0.0, y: 0.0, size: 10.0 }).fill(Paint::token("accent"));
    let b = Node::shape(key("s"), Geom::Symbol { kind: SymbolKind::Circle, x: 100.0, y: 0.0, size: 20.0 }).fill(Paint::token("accent"));
    assert_eq!(mid_geom(a.clone(), b, 0.5), Geom::Symbol { kind: SymbolKind::Circle, x: 50.0, y: 0.0, size: 15.0 });
    let c = Node::shape(key("s"), Geom::Symbol { kind: SymbolKind::Star, x: 100.0, y: 0.0, size: 20.0 }).fill(Paint::token("accent"));
    let g = mid_geom(a, c, 0.5);
    assert!(matches!(g, Geom::Path { .. }));
    assert!(main_area(&g) > 0.0);
}

#[test]
fn same_structure_paths_interpolate_control_points() {
    let tri = |dx: f64| Geom::path(datars_math::PathData::polygon(&[Vec2::new(dx, 0.0), Vec2::new(dx + 10.0, 0.0), Vec2::new(dx + 5.0, 10.0)]));
    let g = morph_geoms(&tri(0.0), &tri(20.0), 0.5, MorphStrategy::Disc);
    assert_eq!(g, tri(10.0));
}

#[test]
fn images_move_and_swap_assets_by_crossfade() {
    let img = |asset: &str, x: f64| Node::new(key("i"), NodeKind::Image { asset: std::sync::Arc::from(asset), rect: Rect::new(x, 0.0, 10.0, 10.0) });
    let p = plan(&one(img("a.png", 0.0)), &one(img("a.png", 20.0)), &linear(), &cx());
    let f = p.at(0.5, &NoShaper);
    let NodeKind::Image { rect, .. } = &find(&f, &path(&["root", "i"])).unwrap().kind else { panic!() };
    assert_eq!(*rect, Rect::new(10.0, 0.0, 10.0, 10.0));
    let q = plan(&one(img("a.png", 0.0)), &one(img("b.png", 0.0)), &linear(), &cx());
    assert_eq!(find_all(&q.at(0.5, &NoShaper), &path(&["root", "i"])).len(), 2);
    assert_eq!(q.stats().crossfades, 1);
}

#[test]
fn gradients_and_strokes_interpolate() {
    let grad = |c: &str| Paint::Linear { linear: [0.0, 0.0, 100.0, 0.0], stops: vec![Stop { at: 0.0, ink: Ink::token("accent") }, Stop { at: 1.0, ink: Ink::parse(c).unwrap() }] };
    let a = rect("r", 0.0, 0.0, 10.0, 10.0).fill(Paint::Solid(Ink::parse("#000000").unwrap()));
    let b = rect("r", 0.0, 0.0, 10.0, 10.0).fill(grad("#ffffff")).stroke(Stroke::new(Ink::token("ink"), 4.0));
    let p = plan(&one(a), &one(b), &linear(), &cx());
    let n = find(&p.at(0.5, &NoShaper), &path(&["root", "r"])).unwrap().clone();
    let NodeKind::Shape { fill: Some(Paint::Linear { stops, .. }), stroke: Some(s), .. } = &n.kind else { panic!("{:?}", n.kind) };
    assert_eq!(stops.len(), 2);
    assert!(matches!(stops[1].ink, Ink::Color(_)));
    assert_eq!(s.width, 2.0, "an appearing stroke grows");
    assert!(matches!(&s.paint, Paint::Solid(Ink::Token { alpha, .. }) if (*alpha - 0.5).abs() < 1e-6), "and fades in as a token: {:?}", s.paint);
}

#[test]
fn views_interpolate_viewports() {
    let view = |vp: Rect| {
        let mut v = Node::new(key("v"), NodeKind::View { viewport: vp, camera: None, clip: true, children: vec![circle("c", 10.0, 10.0, 5.0)] });
        v.common.opacity = 1.0;
        one(v)
    };
    let p = plan(&view(Rect::new(0.0, 0.0, 100.0, 100.0)), &view(Rect::new(100.0, 0.0, 200.0, 100.0)), &linear(), &cx());
    let f = p.at(0.5, &NoShaper);
    let NodeKind::View { viewport, camera, .. } = &find(&f, &path(&["root", "v"])).unwrap().kind else { panic!() };
    assert_eq!(*viewport, Rect::new(50.0, 0.0, 150.0, 100.0));
    assert_eq!(*camera, None);
}

#[test]
fn instance_ghosts_grow_from_a_baseline() {
    let keys = vec![Key::one(1)];
    let a = scene(vec![instances("d", vec![], vec![], vec![], 3.0)]);
    let b = scene(vec![instances("d", keys, vec![50.0], vec![20.0], 4.0)]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::grow(Origin::Baseline(100.0)))]);
    let p = plan(&a, &b, &rules, &cx());
    let f = p.at(0.5, &NoShaper);
    let NodeKind::Instances(i) = &find(&f, &path(&["root", "d"])).unwrap().kind else { panic!() };
    assert_eq!(i.y[0], 60.0, "rises from the baseline");
    assert_eq!(i.size[0], 2.0, "and grows");
    assert_eq!(i.opacity[0], 1.0, "without fading");
}

#[test]
fn arcs_partition_by_angle_or_radius() {
    let slice = Geom::Arc { cx: 0.0, cy: 0.0, r0: 0.0, r1: 100.0, a0: 0.0, a1: 2.0 };
    let ps = partition(&slice, 4, Partition::Slices);
    assert_eq!(ps[1], Geom::Arc { cx: 0.0, cy: 0.0, r0: 0.0, r1: 100.0, a0: 0.5, a1: 1.0 });
    let ring = Geom::Arc { cx: 0.0, cy: 0.0, r0: 50.0, r1: 100.0, a0: 0.0, a1: 0.2 };
    let ps = partition(&ring, 5, Partition::Slices);
    assert_eq!(ps[0], Geom::Arc { cx: 0.0, cy: 0.0, r0: 50.0, r1: 60.0, a0: 0.0, a1: 0.2 }, "thick short arc slices radially");
    let blob = Geom::circle(0.0, 0.0, 10.0);
    let ps = partition(&blob, 9, Partition::Grid);
    let total: f64 = ps.iter().map(|g| main_area(g).abs()).sum();
    let whole = main_area(&blob).abs();
    assert!((total - whole).abs() < 0.005 * whole, "grid cells of a disc tile it: {total} vs {whole}");
}

#[test]
fn opacity_and_visibility_changes_interpolate() {
    let mut a = rect("r", 0.0, 0.0, 10.0, 10.0);
    a.common.visible = false;
    let b = rect("r", 0.0, 0.0, 10.0, 10.0).opacity(0.8);
    let p = plan(&one(a), &one(b), &linear(), &cx());
    let n = find(&p.at(0.5, &NoShaper), &path(&["root", "r"])).unwrap().clone();
    assert!(n.common.visible);
    assert!((n.common.opacity - 0.4).abs() < 1e-12, "hidden counts as transparent: {}", n.common.opacity);
}

#[test]
fn a_moved_path_keeps_its_outline_under_a_moving_transform() {
    // A region re-fitted a few px lower (a map in a slightly taller box): every frame draws its own
    // outline, moved — the frames interpolating every point would make, with geometry that stays
    // the same (renderers keep its mesh instead of re-tessellating thousands of regions a frame).
    let mut p = datars_math::PathData::new();
    p.move_to(Vec2::new(10.0, 10.0)).line_to(Vec2::new(40.0, 12.0)).line_to(Vec2::new(25.0, 30.0)).close();
    let moved = p.transform(&datars_math::Affine::translate(0.0, 8.12));
    let (a, b) = (Node::shape(key("r"), Geom::path(p.clone())), Node::shape(key("r"), Geom::path(moved.clone())));
    let pl = plan(&one(a), &one(b), &linear(), &cx());
    for t in [0.25, 0.5, 0.75] {
        let f = pl.at(t, &NoShaper);
        let n = find(&f, &KeyPath(vec![key("root"), key("r")])).unwrap();
        assert_eq!(geom(n), &Geom::path(p.clone()), "the outline itself doesn't change");
        let xf = n.common.transform.0;
        assert!(xf[..5] == [1.0, 0.0, 0.0, 1.0, 0.0] && (xf[5] - 8.12 * t).abs() < 1e-9, "moved {xf:?} at {t}");
    }
    let end = pl.at(1.0, &NoShaper);
    assert_eq!(geom(find(&end, &KeyPath(vec![key("root"), key("r")])).unwrap()), &Geom::path(moved), "and lands on the target's");
}

#[test]
fn a_huge_set_moved_whole_is_drawn_once_moving() {
    // More dots than are planned one by one (a dot-density map re-fitted a few px lower): not two
    // copies crossfading, the one set moving — what each dot's own interpolation would draw.
    let n = COLUMN_MAX + 1;
    let keys: Vec<Key> = (0..n).map(|i| Key::new(vec![KeyPart::Int(i as i64)])).collect();
    let xs: Vec<f64> = (0..n).map(|i| (i % 1000) as f64).collect();
    let ys: Vec<f64> = (0..n).map(|i| (i / 1000) as f64).collect();
    let a = instances("dots", keys.clone(), xs.clone(), ys.clone(), 1.0);
    let b = instances("dots", keys, xs, ys.iter().map(|y| y + 8.0).collect(), 1.0);
    let pl = plan(&one(a), &one(b), &linear(), &cx());
    let f = pl.at(0.5, &NoShaper);
    let drawn = find_all(&f, &KeyPath(vec![key("root"), key("dots")]));
    assert_eq!(drawn.len(), 1, "one set, not a crossfade");
    let xf = drawn[0].common.transform.0;
    assert!(xf[..5] == [1.0, 0.0, 0.0, 1.0, 0.0] && (xf[5] - 4.0).abs() < 1e-9, "halfway down: {xf:?}");
    assert_eq!(drawn[0].common.opacity, 1.0);
}

/// An overshooting easing overshoots a moved group as it does a moved shape: the transform
/// extrapolates past its end value (it once jumped straight there once the eased value passed 1,
/// clipping back, elastic and spring on every `transform`), and still lands exactly.
#[test]
fn overshooting_easings_carry_transforms_past_the_end_like_geometry() {
    let back = MotionRules::new(vec![Rule::new().easing(Easing::parse("back-out").unwrap())]);
    let moved = |x: f64| group("g", vec![rect("dot", 0.0, 0.0, 10.0, 10.0)]).transform(datars_math::Affine::translate(x, 0.0));
    let shape = |x: f64| rect("s", x, 20.0, 10.0, 10.0);
    let (a, b) = (scene(vec![moved(0.0), shape(0.0)]), scene(vec![moved(100.0), shape(100.0)]));
    let p = plan(&a, &b, &back, &cx());
    let (gp, sp) = (path(&["root", "g"]), path(&["root", "s"]));
    let mut peak: f64 = 0.0;
    for i in 1..40 {
        let f = p.at(i as f64 / 40.0, &NoShaper);
        let gx = find(&f, &gp).unwrap().common.transform.0[4];
        let Geom::Rect { x: sx, .. } = geom(find(&f, &sp).unwrap()).clone() else { panic!() };
        let sx = sx + find(&f, &sp).unwrap().common.transform.0[4];
        assert!((gx - sx).abs() < 1e-9, "the group moves as the shape does at {i}/40: {gx} vs {sx}");
        peak = peak.max(gx);
    }
    assert!(peak > 105.0, "back-out overshoots the end (by about 10 %): peak {peak}");
    assert_eq!(p.at(0.0, &NoShaper), a, "exact at 0");
    assert_eq!(p.at(1.0, &NoShaper), b, "exact at 1");
    // Rotation and scale extrapolate too, but a scale never passes through zero into a mirror.
    let r0 = datars_math::Affine::rotate(0.0);
    let r1 = datars_math::Affine::rotate(1.0).then(datars_math::Affine::scale(2.0, 2.0));
    let over = interp::lerp_affine(&r0, &r1, 1.1);
    assert!((over.scale_factor() - 2.1).abs() < 1e-9 && (m_atan2(over.0[1], over.0[0]) - 1.1).abs() < 1e-9, "{over:?}");
    let grow = interp::lerp_affine(&datars_math::Affine::scale(0.0, 0.0), &datars_math::Affine::IDENTITY, -0.1);
    assert!(grow.0[..4].iter().all(|v| *v == 0.0), "backing up from nothing stays nothing: {grow:?}");
}

fn m_atan2(y: f64, x: f64) -> f64 {
    datars_math::m::atan2(y, x)
}

/// A fill turning from an opaque grey into a faint blue mixes premultiplied by alpha: what it lays
/// down stays between the two ends all the way (straight-alpha mixing gave the faint end its full
/// colour strength mid-way — bluer and darker over the paper than either end, a flash).
#[test]
fn a_fill_fading_into_a_translucent_colour_never_looks_stronger_than_its_ends() {
    use datars_color::Color;
    let grey = Color { r: 0.5, g: 0.5, b: 0.5, a: 1.0 };
    let faint = Color { r: 0.0, g: 0.0, b: 1.0, a: 0.3 };
    let fill = |c: Color| Node::shape(key("r"), Geom::rect(0.0, 0.0, 10.0, 10.0)).fill(Paint::Solid(Ink::Color(c)));
    let p = plan(&one(fill(grey)), &one(fill(faint)), &linear(), &cx());
    // The contribution: OKLab × alpha (what the mix interpolates), and the result over white paper.
    let contribution = |c: Color| {
        let o = c.to_oklab();
        [o.l, o.a, o.b].map(|v| v * c.a as f64)
    };
    let over_paper = |c: Color| [c.r, c.g, c.b].map(|v| (c.a * v + 1.0 - c.a) as f64);
    let chroma = |c: Color| {
        let o = c.to_oklab();
        c.a as f64 * datars_math::m::hypot(o.a, o.b)
    };
    let (ca, cb) = (contribution(grey), contribution(faint));
    let (pa, pb) = (over_paper(grey), over_paper(faint));
    for i in 1..20 {
        let f = p.at(i as f64 / 20.0, &NoShaper);
        let n = find(&f, &path(&["root", "r"])).unwrap();
        let NodeKind::Shape { fill: Some(Paint::Solid(Ink::Color(c))), .. } = &n.kind else { panic!("{:?}", n.kind) };
        let (cm, pm) = (contribution(*c), over_paper(*c));
        for ch in 0..3 {
            // (Colours are stored as f32: a little slack.)
            let (lo, hi) = (ca[ch].min(cb[ch]), ca[ch].max(cb[ch]));
            assert!(cm[ch] >= lo - 1e-4 && cm[ch] <= hi + 1e-4, "OKLab·α {ch} at {i}/20: {} outside [{lo}, {hi}]", cm[ch]);
            // Never darker over the paper than the darker end (straight mixing: red 0.477 < 0.5).
            assert!(pm[ch] >= pa[ch].min(pb[ch]) - 1e-3, "channel {ch} at {i}/20 darker than both ends: {}", pm[ch]);
        }
        assert!(chroma(*c) <= chroma(faint) + 1e-6, "no more colour than the blue end at {i}/20: {} > {}", chroma(*c), chroma(faint));
    }
    // Equal alphas (every opaque pair) mix exactly as before: straight OKLab.
    let (a, b) = (Color::parse("#1f77b4").unwrap(), Color::parse("#ff7f0e").unwrap());
    let (la, lb) = (a.to_oklab(), b.to_oklab());
    let s = datars_color::Oklab { l: la.l + (lb.l - la.l) * 0.3, a: la.a + (lb.a - la.a) * 0.3, b: la.b + (lb.b - la.b) * 0.3, alpha: 1.0 };
    assert_eq!(a.lerp_oklab(b, 0.3), s.to_color());
}
