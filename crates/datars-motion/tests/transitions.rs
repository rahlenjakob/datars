//! Behaviour of plans: matching, interpolation, choreography, ghosts, routes, cameras, text,
//! instances and retargeting.

mod common;

use common::*;
use datars_math::{m, Affine, PathData, Rect, Vec2};
use datars_motion::*;
use datars_scene::*;
use datars_theme::Ink;

fn linear() -> MotionRules {
    MotionRules::new(vec![Rule::new().easing(Easing::Linear)])
}

fn bars(specs: &[(&str, f64, f64, f64, f64)]) -> Scene {
    scene(vec![group("plot", specs.iter().map(|(k, x, y, w, h)| rect(k, *x, *y, *w, *h)).collect())])
}

#[test]
fn bar_to_bar_midpoint_is_exact() {
    let a = bars(&[("S", 0.0, 100.0, 20.0, 50.0)]);
    let b = bars(&[("S", 40.0, 50.0, 20.0, 100.0)]);
    let p = plan(&a, &b, &linear(), &cx());
    let mid = p.at(0.5, &NoShaper);
    let n = find(&mid, &path(&["root", "plot", "S"])).unwrap();
    assert_eq!(*geom(n), Geom::rect(20.0, 75.0, 20.0, 75.0));
    assert_eq!(p.stats().pairs, 1);
    assert_eq!(p.stats().flyers, 0, "same container: interpolates in place");
    assert!((p.duration() - DEFAULT_DURATION).abs() < 1e-12);
    // Cubic in-out by default: the midpoint is still exact (0.5 → 0.5).
    let q = plan(&a, &b, &MotionRules::default(), &cx());
    assert_eq!(*geom(find(&q.at(0.5, &NoShaper), &path(&["root", "plot", "S"])).unwrap()), Geom::rect(20.0, 75.0, 20.0, 75.0));
}

#[test]
fn pie_slice_angles_interpolate() {
    let arc = |a0: f64, a1: f64| Node::shape(key("A"), Geom::Arc { cx: 100.0, cy: 100.0, r0: 0.0, r1: 80.0, a0, a1 }).fill(Paint::token("accent"));
    let a = scene(vec![group("pie", vec![arc(0.0, 1.0)])]);
    let b = scene(vec![group("pie", vec![arc(1.0, 2.5)])]);
    let p = plan(&a, &b, &linear(), &cx());
    let g = geom(find(&p.at(0.5, &NoShaper), &path(&["root", "pie", "A"])).unwrap()).clone();
    assert_eq!(g, Geom::Arc { cx: 100.0, cy: 100.0, r0: 0.0, r1: 80.0, a0: 0.5, a1: 1.75 });
}

#[test]
fn rect_to_circle_disc_morph_never_folds() {
    let a = scene(vec![group("g", vec![rect("x", 200.0, 50.0, 40.0, 120.0)])]);
    let b = scene(vec![group("g", vec![circle("x", 60.0, 200.0, 30.0)])]);
    let p = plan(&a, &b, &linear(), &cx());
    assert_eq!(p.stats().morphs, 1);
    let (area_a, area_b) = (40.0 * 120.0, m::PI * 900.0);
    let mut min_area = f64::INFINITY;
    for i in 1..200 {
        let t = i as f64 / 200.0;
        let f = p.at(t, &NoShaper);
        let g = geom(find(&f, &path(&["root", "g", "x"])).unwrap());
        assert!(matches!(g, Geom::Path { .. }), "mid-morph geometry is an outline");
        let area = main_area(g);
        min_area = min_area.min(area / (area_a + (area_b - area_a) * t));
        assert!(area > 0.0, "t={t}: area {area}");
    }
    assert!(min_area > 0.5, "the outline stays substantial (area never collapses): {min_area}");
    // The centre travels on a straight line (no route): at ½ it is halfway.
    let f = p.at(0.5, &NoShaper);
    let c = geom(find(&f, &path(&["root", "g", "x"])).unwrap()).bounds().center();
    assert!((c.x - 140.0).abs() < 2.0 && (c.y - 155.0).abs() < 2.0, "{c:?}");
}

#[test]
fn anticlockwise_outlines_never_turn_inside_out() {
    let ccw = Geom::path(PathData::polygon(&[Vec2::new(0.0, 0.0), Vec2::new(0.0, 60.0), Vec2::new(30.0, 90.0), Vec2::new(80.0, 50.0), Vec2::new(60.0, 0.0)]));
    let cw = Geom::path(PathData::polygon(&[Vec2::new(0.0, 0.0), Vec2::new(60.0, 0.0), Vec2::new(80.0, 50.0), Vec2::new(30.0, 90.0), Vec2::new(0.0, 60.0)]));
    for from in [ccw, cw] {
        for strategy in [MorphStrategy::Disc, MorphStrategy::Resample] {
            let a = scene(vec![Node::shape(key("r"), from.clone()).fill(Paint::token("accent"))]);
            let b = scene(vec![rect("r", 200.0, 50.0, 40.0, 120.0)]);
            let p = plan(&a, &b, &MotionRules::new(vec![Rule::new().easing(Easing::Linear).morph(strategy)]), &cx());
            for t in [0.2, 0.5, 0.8] {
                let g = geom(find(&p.at(t, &NoShaper), &path(&["root", "r"])).unwrap()).clone();
                assert!(main_area(&g) > 500.0, "{strategy:?} t={t}: {}", main_area(&g));
            }
        }
    }
}

#[test]
fn multipolygon_largest_part_morphs_small_parts_shrink() {
    let mut p2 = PathData::rect(Rect::new(0.0, 0.0, 100.0, 100.0));
    p2.extend(&PathData::rect(Rect::new(150.0, 0.0, 10.0, 10.0)));
    let a = scene(vec![Node::shape(key("SE"), Geom::path(p2)).fill(Paint::token("accent"))]);
    let b = scene(vec![circle("SE", 300.0, 200.0, 40.0)]);
    let p = plan(&a, &b, &linear(), &cx());
    let f = p.at(0.5, &NoShaper);
    let g = geom(find(&f, &path(&["root", "SE"])).unwrap()).clone();
    let rings = g.to_path().flatten(0.1);
    assert_eq!(rings.len(), 2, "island still present mid-way");
    let island = datars_math::path::signed_area(&rings[1].0).abs();
    assert!((island - 25.0).abs() < 1.0, "island at half scale has a quarter of its area: {island}");
    let late = p.at(0.999, &NoShaper);
    let lr = geom(find(&late, &path(&["root", "SE"])).unwrap()).to_path().flatten(0.1);
    let island_late = lr.get(1).map_or(0.0, |r| datars_math::path::signed_area(&r.0).abs());
    assert!(island_late < 0.01, "island nearly gone: {island_late}");
}

#[test]
fn enter_grows_from_the_baseline_and_exit_sinks() {
    let a = bars(&[("A", 0.0, 100.0, 20.0, 100.0)]);
    let b = bars(&[("A", 0.0, 100.0, 20.0, 100.0), ("B", 40.0, 50.0, 20.0, 150.0)]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::grow(Origin::Baseline(200.0))).exit(Ghost::rise(12.0))]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!(p.stats().enters, 1);
    let f = p.at(0.5, &NoShaper);
    let n = find(&f, &path(&["root", "plot", "B"])).unwrap();
    let xf = n.common.transform;
    assert!((xf.apply(Vec2::new(50.0, 200.0)).y - 200.0).abs() < 1e-9, "baseline stays put");
    assert!((xf.apply(Vec2::new(50.0, 50.0)).y - 125.0).abs() < 1e-9, "top is halfway up");
    assert_eq!(n.common.opacity, 1.0, "grow does not fade");
    // Exit: fades and sinks 12 px.
    let q = plan(&b, &a, &rules, &cx());
    let g = find(&q.at(0.5, &NoShaper), &path(&["root", "plot", "B"])).unwrap().clone();
    assert!((g.common.opacity - 0.5).abs() < 1e-12);
    assert!((g.common.transform.apply(Vec2::ZERO).y - 6.0).abs() < 1e-9);
}

#[test]
fn enter_from_point_and_from_parent() {
    let a = scene(vec![group("g", vec![rect("SE", 100.0, 100.0, 50.0, 50.0)])]);
    let b = scene(vec![group("g", vec![rect("SE", 100.0, 100.0, 50.0, 50.0), rect("NO", 300.0, 20.0, 20.0, 20.0)])]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::from_point(Vec2::new(0.0, 0.0)))]);
    let p = plan(&a, &b, &rules, &cx());
    let n = find(&p.at(1e-6, &NoShaper), &path(&["root", "g", "NO"])).unwrap().clone();
    let c = n.common.transform.apply(Vec2::new(310.0, 30.0));
    assert!(c.len() < 0.01, "starts at the point: {c:?}");
    // From parent: children ("SE", "AB") appear from ("SE",).
    let kid = |k: &str, x: f64| Node::shape(Key::new(vec!["SE".into(), k.into()]), Geom::rect(x, 0.0, 10.0, 10.0)).fill(Paint::token("accent"));
    let c2 = scene(vec![group("g", vec![kid("AB", 300.0), kid("C", 360.0)])]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::from_parent())]);
    let p = plan(&a, &c2, &rules, &cx());
    let path_ab = KeyPath(vec![key("root"), key("g"), Key::new(vec!["SE".into(), "AB".into()])]);
    let n = find(&p.at(1e-6, &NoShaper), &path_ab).unwrap().clone();
    let c = n.common.transform.apply(Vec2::new(305.0, 5.0));
    assert!((c - Vec2::new(125.0, 125.0)).len() < 0.01, "starts at the parent's centre: {c:?}");
}

fn seats(n: usize) -> Node {
    let keys: Vec<Key> = (0..n as u32).map(|i| Key::new(vec!["S".into(), KeyPart::Unit { unit: i }])).collect();
    let xs: Vec<f64> = (0..n).map(|i| 250.0 + (i % 6) as f64 * 12.0).collect();
    let ys: Vec<f64> = (0..n).map(|i| 100.0 + (i / 6) as f64 * 12.0).collect();
    instances("seats", keys, xs, ys, 4.0)
}

/// Every `Instances` node in a scene, depth first.
fn all_instances(n: &Node, out: &mut Vec<Instances>) {
    if let NodeKind::Instances(i) = &n.kind {
        out.push((**i).clone());
    }
    for c in n.children() {
        all_instances(c, out);
    }
}

#[test]
fn hierarchy_split_bar_into_units() {
    // A bar breaking into seats: the pieces ride in the seats' column (one instanced node per
    // prototype, not 30 flying shapes), squares of the bar crossfading into the seats' circles.
    let a = scene(vec![group("bars", vec![rect("S", 20.0, 40.0, 40.0, 120.0)])]);
    let b = scene(vec![group("parl", vec![seats(30)])]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::Hierarchy { partition: Partition::Slices })]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!(p.stats().splits, 1);
    assert_eq!(p.stats().flyers, 0, "no flying pieces");
    assert_eq!(p.stats().morphs, 0);
    assert_eq!(p.stats().instances, 30, "the pieces are the column's rows");
    let at = |t: f64| {
        let mut v = Vec::new();
        all_instances(&p.at(t, &NoShaper).root, &mut v);
        v
    };
    // At the start the pieces tile the bar, as rects.
    let f0 = at(1e-9);
    let rects = f0.iter().find(|i| i.proto == Proto::Rect).expect("the pieces, as rects");
    assert_eq!(rects.len(), 30);
    let mut area = 0.0;
    for k in 0..30 {
        let (x, y, w, h) = (rects.x[k], rects.y[k], rects.w.as_ref().unwrap()[k], rects.h.as_ref().unwrap()[k]);
        assert!(x >= 20.0 - 1e-6 && x + w <= 60.0 + 1e-6 && y >= 40.0 - 1e-6 && y + h <= 160.0 + 1e-6, "piece {k} inside the bar: {x} {y} {w} {h}");
        assert!(rects.opacity_at(k) > 0.99, "shown at the start");
        area += w * h;
    }
    assert!((area - 4800.0).abs() < 1.0, "pieces tile the bar: {area}");
    // At the end they are the seats (circles of radius 4 at the seat positions), the rects gone.
    let f1 = at(1.0 - 1e-9);
    let circles = f1.iter().find(|i| matches!(i.proto, Proto::Symbol { .. })).expect("the seats");
    for k in 0..30usize {
        let unit = circles.keys.iter().position(|key| *key == Key::new(vec!["S".into(), KeyPart::Unit { unit: k as u32 }])).expect("each seat");
        let (ex, ey) = (250.0 + (k % 6) as f64 * 12.0, 100.0 + (k / 6) as f64 * 12.0);
        assert!((circles.x[unit] - ex).abs() < 0.05 && (circles.y[unit] - ey).abs() < 0.05, "seat {k} in place");
        assert!((circles.size_at(unit) - 4.0).abs() < 0.05, "seat {k} radius");
        assert!(circles.opacity_at(unit) > 0.99);
    }
    if let Some(r) = f1.iter().find(|i| i.proto == Proto::Rect) {
        assert!((0..r.len()).all(|k| r.opacity_at(k) < 0.01), "the squares have faded");
    }
    assert_eq!(p.at(1.0, &NoShaper), b);
}

#[test]
fn hierarchy_merge_units_into_bar() {
    let a = scene(vec![group("parl", vec![seats(12)])]);
    let b = scene(vec![group("bars", vec![rect("S", 20.0, 40.0, 40.0, 120.0)])]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::Hierarchy { partition: Partition::Grid })]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!(p.stats().merges, 1);
    let f = p.at(1.0 - 1e-9, &NoShaper);
    let mut area = 0.0;
    walk(&f, &mut |q, n| {
        if q.0.get(1) == Some(&key(FLIGHT_KEY)) {
            if let NodeKind::Shape { geom, .. } = &n.kind {
                area += main_area(geom).abs();
            }
        }
    });
    assert!((area - 4800.0).abs() < 5.0, "units reassemble into the bar: {area}");
}

#[test]
fn by_key_matches_across_parents() {
    let region = Node::shape(key("SE"), Geom::path(PathData::polygon(&[Vec2::new(10.0, 10.0), Vec2::new(90.0, 20.0), Vec2::new(60.0, 90.0)]))).fill(Paint::token("accent"));
    let mut map = Node::new(key("map"), NodeKind::View { viewport: Rect::new(0.0, 0.0, 200.0, 200.0), camera: Some(Camera { x: 50.0, y: 50.0, zoom: 2.0, rotation: 0.0 }), clip: true, children: vec![region] });
    map.common.opacity = 1.0;
    let a = scene(vec![map]);
    let b = scene(vec![group("bars", vec![rect("SE", 250.0, 50.0, 30.0, 200.0)])]);
    let by_path = plan(&a, &b, &MotionRules::default(), &cx());
    assert_eq!((by_path.stats().pairs, by_path.stats().enters, by_path.stats().exits), (0, 1, 1));
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::ByKey)]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!((p.stats().pairs, p.stats().enters, p.stats().exits, p.stats().flyers), (1, 0, 0, 1));
    assert_eq!(p.correspondence().pairs, vec![(path(&["root", "map", "SE"]), path(&["root", "bars", "SE"]))]);
    // In flight, in root coordinates: starts where the region was drawn (through the camera).
    let f = p.at(1e-9, &NoShaper);
    let n = find(&f, &path(&["root", FLIGHT_KEY, "bars", "SE"])).unwrap();
    let b0 = n.common.transform.apply(geom(n).bounds().center());
    // Region centre ≈ (53, 40) in map units → camera: (x − 50)·2 + 100.
    let rc = Geom::path(PathData::polygon(&[Vec2::new(10.0, 10.0), Vec2::new(90.0, 20.0), Vec2::new(60.0, 90.0)])).bounds().center();
    let expect = Vec2::new((rc.x - 50.0) * 2.0 + 100.0, (rc.y - 50.0) * 2.0 + 100.0);
    assert!((b0 - expect).len() < 1.0, "{b0:?} vs {expect:?}");
    assert!(find(&f, &path(&["root", "map", "SE"])).is_none(), "not drawn twice");
}

#[test]
fn stagger_left_starts_the_leftmost_first() {
    let xs = [300.0, 20.0, 180.0, 90.0, 240.0];
    let names = ["a", "b", "c", "d", "e"];
    let specs: Vec<(&str, f64, f64, f64, f64)> = names.iter().zip(xs).map(|(k, x)| (*k, x, 100.0, 20.0, 50.0)).collect();
    let a = bars(&specs);
    let b = bars(&specs.iter().map(|(k, x, y, w, h)| (*k, *x, *y - 50.0, *w, *h + 50.0)).collect::<Vec<_>>());
    let rules = MotionRules::new(vec![Rule::new().choreo(Choreography::Stagger { order: Order::Left, spread: 0.5 })]);
    let p = plan(&a, &b, &rules, &cx());
    let mut wins: Vec<(f64, f64)> = p.windows().into_iter().map(|(k, w)| (xs[names.iter().position(|n| Key::name(n) == k).unwrap()], w.0)).collect();
    wins.sort_by(|a, b| a.0.total_cmp(&b.0));
    for w in wins.windows(2) {
        assert!(w[0].1 < w[1].1, "leftmost starts first: {wins:?}");
    }
    assert_eq!(wins[0].1, 0.0);
    assert!((wins[4].1 - 0.5).abs() < 1e-12, "the last starts after the spread");
    // Frames agree: early on, the leftmost bar has moved more than the rightmost.
    let f = p.at(0.2, &NoShaper);
    let h = |k: &str| match geom(find(&f, &path(&["root", "plot", k])).unwrap()) {
        Geom::Rect { h, .. } => *h,
        _ => 0.0,
    };
    assert!(h("b") > h("a"), "b (x=20) ahead of a (x=300)");
    assert_eq!(h("a"), 50.0, "a has not started");
}

#[test]
fn phased_windows_split_exit_update_enter() {
    let a = bars(&[("stay", 0.0, 0.0, 10.0, 10.0), ("go", 20.0, 0.0, 10.0, 10.0)]);
    let b = bars(&[("stay", 50.0, 0.0, 10.0, 10.0), ("new", 80.0, 0.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![Rule::new().choreo(Choreography::Phased { exit: 0.25, update: 0.5, enter: 0.25 })]);
    let p = plan(&a, &b, &rules, &cx());
    let w: std::collections::BTreeMap<String, (f64, f64)> = p.windows().into_iter().map(|(k, w)| (k.to_string(), w)).collect();
    assert_eq!(w["(\"go\",)"], (0.0, 0.25));
    assert_eq!(w["(\"stay\",)"], (0.25, 0.75));
    assert_eq!(w["(\"new\",)"], (0.75, 1.0));
}

#[test]
fn delays_and_durations_set_the_plan_length() {
    let a = bars(&[("x", 0.0, 0.0, 10.0, 10.0), ("y", 0.0, 20.0, 10.0, 10.0)]);
    let b = bars(&[("x", 50.0, 0.0, 10.0, 10.0), ("y", 50.0, 20.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![
        Rule::new().duration(1.0),
        Rule::new().select(Selector::prefix(path(&["root", "plot", "y"]))).delay(0.5),
    ]);
    let p = plan(&a, &b, &rules, &cx());
    assert!((p.duration() - 1.5).abs() < 1e-12);
    let w: std::collections::BTreeMap<String, (f64, f64)> = p.windows().into_iter().map(|(k, w)| (k.to_string(), w)).collect();
    assert_eq!(w["(\"x\",)"], (0.0, 1.0 / 1.5));
    assert_eq!(w["(\"y\",)"], (0.5 / 1.5, 1.0));
}

#[test]
fn number_text_counts_through_the_shaper() {
    let a = scene(vec![number_text("v", 10.0, Vec2::new(10.0, 10.0))]);
    let b = scene(vec![number_text("v", 20.0, Vec2::new(50.0, 10.0))]);
    let p = plan(&a, &b, &linear(), &cx());
    let sh = FakeShaper::default();
    let f = p.at(0.5, &sh);
    let NodeKind::Text(t) = &find(&f, &path(&["root", "v"])).unwrap().kind else { panic!() };
    assert_eq!(t.text, "15");
    assert_eq!(t.number.as_ref().unwrap().value, 15.0);
    assert_eq!(t.origin, Vec2::new(30.0, 10.0));
    assert_eq!(t.runs[0].glyphs.len(), 2);
    assert_eq!(sh.calls.get(), 1, "re-shaped once per frame");
    // Plain labels with different text crossfade in sequence (two copies, the outgoing first):
    // the old text is gone before the new one shows, so they never overlap.
    let a = scene(vec![label("l", "alpha", Vec2::new(0.0, 0.0))]);
    let b = scene(vec![label("l", "beta", Vec2::new(0.0, 0.0))]);
    let p = plan(&a, &b, &linear(), &cx());
    let f = p.at(0.25, &sh);
    let both = find_all(&f, &path(&["root", "l"]));
    assert_eq!(both.len(), 2);
    assert!((both[0].common.opacity - 0.5).abs() < 1e-12 && both[1].common.opacity.abs() < 1e-12);
    let f = p.at(0.75, &sh);
    let both = find_all(&f, &path(&["root", "l"]));
    assert!(both[0].common.opacity.abs() < 1e-12 && (both[1].common.opacity - 0.5).abs() < 1e-12);
    // Same text, different ink: interpolates without re-shaping.
    let mut lb = label("l", "alpha", Vec2::new(10.0, 0.0));
    if let NodeKind::Text(t) = &mut lb.kind {
        t.style.ink = Ink::token("accent");
        t.runs[0].ink = Ink::token("accent");
    }
    let p = plan(&scene(vec![label("l", "alpha", Vec2::new(0.0, 0.0))]), &scene(vec![lb]), &linear(), &cx());
    let sh2 = FakeShaper::default();
    let f = p.at(0.5, &sh2);
    let NodeKind::Text(t) = &find(&f, &path(&["root", "l"])).unwrap().kind else { panic!() };
    assert!(matches!(t.style.ink, Ink::Color(_)), "mid-way colour is literal");
    assert_eq!(t.origin, Vec2::new(5.0, 0.0));
    assert_eq!(sh2.calls.get(), 0);
}

#[test]
fn inks_stay_tokens_when_equal_and_mix_in_oklab_when_not() {
    let a = bars(&[("s", 0.0, 0.0, 10.0, 10.0)]);
    let mut b = bars(&[("s", 50.0, 0.0, 10.0, 10.0)]);
    let p = plan(&a, &b, &linear(), &cx());
    let n = find(&p.at(0.5, &NoShaper), &path(&["root", "plot", "s"])).unwrap().clone();
    let NodeKind::Shape { fill, .. } = &n.kind else { panic!() };
    assert_eq!(*fill, Some(Paint::token("accent")), "equal inks stay tokens");
    if let NodeKind::Group { children } = &mut b.root.kind {
        if let NodeKind::Group { children } = &mut children[0].kind {
            children[0] = children[0].clone().fill(Paint::Solid(Ink::token("negative")));
        }
    }
    let p = plan(&a, &b, &linear(), &cx());
    let n = find(&p.at(0.5, &NoShaper), &path(&["root", "plot", "s"])).unwrap().clone();
    let NodeKind::Shape { fill: Some(Paint::Solid(Ink::Color(c))), .. } = &n.kind else { panic!("{:?}", n.kind) };
    let th = theme();
    let expect = Ink::token("accent").resolve(&th).lerp_oklab(Ink::token("negative").resolve(&th), 0.5);
    assert_eq!(*c, expect);
}

#[test]
fn camera_zooms_out_on_a_long_pan() {
    let view = |x: f64, zoom: f64| {
        let mut v = Node::new(key("map"), NodeKind::View { viewport: Rect::new(0.0, 0.0, 400.0, 300.0), camera: Some(Camera { x, y: 150.0, zoom, rotation: 0.0 }), clip: true, children: vec![circle("c", 500.0, 150.0, 10.0)] });
        v.common.opacity = 1.0;
        scene(vec![v])
    };
    let cam = |s: &Scene| match &find(s, &path(&["root", "map"])).unwrap().kind {
        NodeKind::View { camera: Some(c), .. } => *c,
        _ => panic!(),
    };
    // Visible width 100 at zoom 4; a pan of 800 is 8 widths: the path zooms out.
    let p = plan(&view(100.0, 4.0), &view(900.0, 4.0), &linear(), &cx());
    let mid = cam(&p.at(0.5, &NoShaper));
    assert!(mid.zoom < 2.0, "zooms out mid-flight: {}", mid.zoom);
    assert!((mid.x - 500.0).abs() < 1e-6, "symmetric path: halfway at ½: {}", mid.x);
    // A short pan stays at its zoom (straight interpolation).
    let q = plan(&view(100.0, 4.0), &view(130.0, 4.0), &linear(), &cx());
    let c = cam(&q.at(0.5, &NoShaper));
    assert!((c.zoom - 4.0).abs() < 1e-9 && (c.x - 115.0).abs() < 1e-9);
    // Content inside the view stays glued to the camera (in-place, unchanged).
    let f = p.at(0.5, &NoShaper);
    assert_eq!(*geom(find(&f, &path(&["root", "map", "c"])).unwrap()), Geom::circle(500.0, 150.0, 10.0));
}

#[test]
fn routes_detour_mid_flight_only() {
    let a = bars(&[("x", 0.0, 100.0, 10.0, 10.0)]);
    let b = bars(&[("x", 200.0, 100.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).route(Route::Arc { height: 0.5 })]);
    let p = plan(&a, &b, &rules, &cx());
    let f = p.at(0.5, &NoShaper);
    let n = find(&f, &path(&["root", "plot", "x"])).unwrap();
    let c = n.common.transform.apply(geom(n).bounds().center());
    assert!((c.x - 105.0).abs() < 1e-9 && (c.y - 5.0).abs() < 1e-9, "arcs up by half the distance: {c:?}");
    let g = find(&p.at(1.0 - 1e-12, &NoShaper), &path(&["root", "plot", "x"])).unwrap().clone();
    assert!(g.common.transform.apply(Vec2::ZERO).len() < 1e-6);
}

#[test]
fn route_offsets_vanish_at_both_ends() {
    let routes = [
        Route::Straight,
        Route::Arc { height: 0.45 },
        Route::Elbow,
        Route::Spiral { turns: 0.45 },
        Route::Explode,
        Route::Hop { height: 20.0 },
        Route::Drift { seed: 7, amount: 40.0 },
        Route::Drop { bounce: 1.0 },
    ];
    let rcx = RouteCx { mid: Vec2::new(100.0, 100.0), span: 300.0 };
    let (a, b) = (Vec2::new(10.0, 20.0), Vec2::new(200.0, 80.0));
    for r in &routes {
        for k in [0.0, 1.0, -0.5, 1.5, f64::NAN] {
            assert_eq!(r.offset(a, b, k, 0xdead_beef, &rcx), Vec2::ZERO, "{r:?} at {k}");
        }
        if !r.is_straight() {
            assert!(r.offset(a, b, 0.5, 0xdead_beef, &rcx).len() > 1.0, "{r:?} detours mid-way");
        }
        let o = r.offset(a, b, 1e-6, 1, &rcx);
        assert!(o.len() < 0.01 || matches!(r, Route::Elbow), "{r:?} starts continuously: {o:?}");
    }
    assert!(Route::Hop { height: 20.0 }.offset(a, b, 0.5, 0, &rcx).y < -19.9, "a hop goes up");
}

#[test]
fn route_offsets_are_continuous_through_the_centre() {
    // A trip straight through the scene centre: no route may jump where the path crosses it.
    let rcx = RouteCx { mid: Vec2::new(100.0, 100.0), span: 300.0 };
    let (a, b) = (Vec2::new(20.0, 60.0), Vec2::new(180.0, 140.0));
    for r in [Route::Arc { height: 0.45 }, Route::Spiral { turns: 0.45 }, Route::Explode, Route::Hop { height: 20.0 }, Route::Drift { seed: 7, amount: 40.0 }] {
        let n = 400;
        let pts: Vec<Vec2> = (0..=n).map(|i| r.offset(a, b, i as f64 / n as f64, 0xdead_beef, &rcx)).collect();
        let worst = pts.windows(2).map(|w| w[0].dist(w[1])).fold(0.0, f64::max);
        assert!(worst < 5.0, "{r:?} jumps {worst:.1} px between steps of 1/{n}");
    }
}

#[test]
fn instances_interpolate_column_wise() {
    let n = 100;
    let keys: Vec<Key> = (0..n).map(|i| Key::one(i as i64)).collect();
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let a = scene(vec![instances("dots", keys.clone(), xs.clone(), vec![0.0; n], 2.0)]);
    let mut keys_b = keys[10..].to_vec();
    keys_b.push(Key::one(1000));
    let xb: Vec<f64> = keys_b.iter().map(|k| match k.parts()[0] { KeyPart::Int(v) => v as f64 + 10.0, _ => 0.0 }).collect();
    let b = scene(vec![instances("dots", keys_b.clone(), xb, vec![0.0; keys_b.len()], 2.0)]);
    let p = plan(&a, &b, &linear(), &cx());
    assert_eq!(p.stats().instances, 101);
    let f = p.at(0.5, &NoShaper);
    let NodeKind::Instances(i) = &find(&f, &path(&["root", "dots"])).unwrap().kind else { panic!() };
    assert_eq!(i.len(), 91 + 10, "targets + exits");
    assert_eq!(i.keys[0], Key::one(10));
    assert_eq!(i.x[0], 15.0);
    let new = i.keys.iter().position(|k| *k == Key::one(1000)).unwrap();
    assert_eq!(i.opacity[new], 0.5, "entering instance half faded in");
    let gone = i.keys.iter().position(|k| *k == Key::one(3)).unwrap();
    assert_eq!(i.opacity[gone], 0.5, "exiting instance half faded out");
    assert_eq!(p.at(1.0, &NoShaper), b);
}

fn line(k: &str, pts: &[(f64, f64)]) -> Node {
    let pts: Vec<Vec2> = pts.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
    Node::shape(key(k), Geom::Polyline { pts: pts.into(), closed: false, curve: Default::default() })
        .stroke(Stroke { paint: Paint::Solid(Ink::token("accent")), width: 2.0, dash: None, cap: Cap::Round, join: Join::Round, non_scaling: false })
}

#[test]
fn a_line_that_runs_on_grows_along_its_path() {
    // A track drawn further up the clock: the new points extend the old ones. Mid-flight the line
    // is the longer one, trimmed — every point it shows lies on its path (no bending across).
    let short = [(0.0, 0.0), (100.0, 0.0)];
    let long = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
    let a = scene(vec![line("path", &short)]);
    let b = scene(vec![line("path", &long)]);
    let p = plan(&a, &b, &linear(), &cx());
    let mid = p.at(0.5, &NoShaper);
    let n = find(&mid, &path(&["root", "path"])).unwrap();
    assert_eq!(geom(n), geom(find(&b, &path(&["root", "path"])).unwrap()), "draws the longer line");
    let [t0, t1] = n.common.trim.expect("trimmed");
    assert_eq!(t0, 0.0);
    assert!((t1 - (1.0 / 3.0 + (2.0 / 3.0) * 0.5)).abs() < 1e-3, "halfway from a third of the length to all of it: {t1}");
    assert_eq!(p.at(0.0, &NoShaper), a);
    assert_eq!(p.at(1.0, &NoShaper), b);
    // Backwards it shrinks along itself.
    let back = plan(&b, &a, &linear(), &cx());
    let n = find(&back.at(0.5, &NoShaper), &path(&["root", "path"])).map(|n| (geom(n).clone(), n.common.trim)).unwrap();
    assert_eq!(n.0, *geom(find(&b, &path(&["root", "path"])).unwrap()));
    assert!((n.1.unwrap()[1] - 2.0 / 3.0).abs() < 1e-3);
    // A line that changes shape (not a run-on) still bends into its new shape.
    let c = scene(vec![line("path", &[(0.0, 50.0), (100.0, 50.0), (100.0, 150.0)])]);
    let n = find(&plan(&a, &c, &linear(), &cx()).at(0.5, &NoShaper), &path(&["root", "path"])).unwrap().common.trim;
    assert_eq!(n, None);
}

#[test]
fn by_key_instances_stay_a_column_unless_something_else_has_their_keys() {
    // A dot map's dots keyed by row, matched by key: with only the same node's instances on the
    // other side, pairing inside the column is what by-key matching does — no element per dot.
    let by_key = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::ByKey)]);
    let keys: Vec<Key> = (0..50).map(|i| Key::one(i as i64)).collect();
    let a = scene(vec![instances("dots", keys.clone(), (0..50).map(|i| i as f64).collect(), vec![0.0; 50], 2.0)]);
    let b = scene(vec![instances("dots", keys[5..].to_vec(), (5..50).map(|i| i as f64 + 10.0).collect(), vec![0.0; 45], 2.0)]);
    let p = plan(&a, &b, &by_key, &cx());
    assert_eq!(p.stats().pairs, 0, "no per-instance elements");
    assert_eq!(p.stats().instances, 50, "45 pairs + 5 exits in the column");
    let byp = plan(&a, &b, &linear(), &cx());
    assert_eq!(p.at(0.5, &NoShaper), byp.at(0.5, &NoShaper), "the same frames as pairing by path");
    // A bar on the other side keyed like one of the dots: it becomes that dot (elements needed).
    let with_bar = scene(vec![instances("dots", keys[5..].to_vec(), (5..50).map(|i| i as f64).collect(), vec![0.0; 45], 2.0), Node::shape(Key::one(2), Geom::rect(0.0, 0.0, 10.0, 10.0)).fill(Paint::Solid(Ink::token("accent")))]);
    let p = plan(&a, &with_bar, &by_key, &cx());
    assert!(p.correspondence().pairs.iter().any(|(x, y)| x.0.last() == Some(&Key::one(2)) && y.0.last() == Some(&Key::one(2))), "dot 2 → bar 2");
}

#[test]
fn huge_instance_sets_crossfade_whole() {
    // Past COLUMN_MAX a column isn't planned per instance: 400,000 dots arriving fade in as one.
    let n = COLUMN_MAX + 1;
    let keys: Vec<Key> = (0..n).map(|i| Key::one(i as i64)).collect();
    let a = scene(vec![instances("dots", vec![], vec![], vec![], 2.0)]);
    let b = scene(vec![instances("dots", keys, (0..n).map(|i| (i % 400) as f64).collect(), vec![0.0; n], 2.0)]);
    let p = plan(&a, &b, &MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::ByKey)]), &cx());
    assert_eq!(p.stats().instances, n);
    assert_eq!(p.at(0.0, &NoShaper), a);
    assert_eq!(p.at(1.0, &NoShaper), b);
    let mid = p.at(0.5, &NoShaper);
    let dots = find_all(&mid, &path(&["root", "dots"]));
    let last = dots.last().unwrap();
    assert!((last.common.opacity - 0.5).abs() < 1e-12, "the arriving node half faded in");
    let NodeKind::Instances(i) = &last.kind else { panic!() };
    assert_eq!(i.len(), n, "every dot where it lands");
    // Unchanged, it just stays.
    let same = plan(&b, &b, &linear(), &cx());
    assert_eq!(same.at(0.5, &NoShaper), b);
}

#[test]
fn instance_prototype_change_crossfades_two_nodes() {
    let keys: Vec<Key> = (0..5).map(|i| Key::one(i as i64)).collect();
    let a = scene(vec![instances("d", keys.clone(), vec![0.0; 5], vec![0.0; 5], 3.0)]);
    let mut nb = instances("d", keys, vec![10.0; 5], vec![0.0; 5], 3.0);
    if let NodeKind::Instances(i) = &mut nb.kind {
        let i = std::sync::Arc::make_mut(i);
        i.proto = Proto::Rect;
        i.w = Some(vec![6.0; 5]);
        i.h = Some(vec![6.0; 5]);
        i.x = vec![7.0; 5];
        i.y = vec![-3.0; 5];
    }
    let b = scene(vec![nb]);
    let p = plan(&a, &b, &linear(), &cx());
    let f = p.at(0.5, &NoShaper);
    let both = find_all(&f, &path(&["root", "d"]));
    assert_eq!(both.len(), 2);
    let (NodeKind::Instances(x), NodeKind::Instances(y)) = (&both[0].kind, &both[1].kind) else { panic!() };
    assert!(matches!(x.proto, Proto::Symbol { .. }) && matches!(y.proto, Proto::Rect));
    assert!((x.x[0] - 5.0).abs() < 1e-9, "circle centre halfway: {}", x.x[0]);
    assert!((x.opacity[0] - 0.5).abs() < 1e-12 && (y.opacity[0] - 0.5).abs() < 1e-12);
}

#[test]
fn nearest_matches_unkeyed_sets_without_crossing() {
    let a = scene(vec![group("g", (0..8).map(|i| circle(&format!("a{i}"), i as f64 * 40.0, 0.0, 5.0)).collect())]);
    let b = scene(vec![group("g", (0..6).map(|i| circle(&format!("b{i}"), 300.0 - i as f64 * 45.0, 100.0, 5.0)).collect())]);
    let rules = MotionRules::new(vec![Rule::new().matcher(Matcher::Nearest)]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!((p.stats().pairs, p.stats().exits, p.stats().enters), (6, 2, 0));
    let x_of = |s: &Scene, q: &KeyPath| geom(find(s, q).unwrap()).bounds().center().x;
    let mut pairs: Vec<(f64, f64)> = p.correspondence().pairs.iter().map(|(pa, pb)| (x_of(&a, pa), x_of(&b, pb))).collect();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    for w in pairs.windows(2) {
        assert!(w[0].1 <= w[1].1, "monotone assignment on a line (no crossings): {pairs:?}");
    }
    // Large sets use the greedy path and still pair everything.
    let big = |n: usize, dy: f64| scene(vec![group("g", (0..n).map(|i| circle(&format!("p{i}"), (i % 60) as f64 * 5.0, (i / 60) as f64 * 5.0 + dy, 1.0)).collect())]);
    let (ba, bb) = (big(2500, 0.0), big(2500, 1.0));
    let rules = MotionRules::new(vec![Rule::new().matcher(Matcher::Nearest)]);
    let q = plan(&ba, &bb, &rules, &cx());
    assert_eq!(q.stats().pairs, 2500);
}

#[test]
fn none_matcher_crossfades_everything() {
    let a = bars(&[("x", 0.0, 0.0, 10.0, 10.0)]);
    let b = bars(&[("x", 50.0, 0.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::None)]);
    let p = plan(&a, &b, &rules, &cx());
    assert_eq!((p.stats().pairs, p.stats().enters, p.stats().exits), (0, 1, 1));
    let f = p.at(0.5, &NoShaper);
    let both = find_all(&f, &path(&["root", "plot", "x"]));
    assert_eq!(both.len(), 2);
    assert_eq!(both.iter().map(|n| n.common.opacity).sum::<f64>(), 1.0);
}

#[test]
fn rules_cascade_by_specificity_and_state() {
    let a = bars(&[("x", 0.0, 0.0, 10.0, 10.0)]);
    let b = bars(&[("x", 50.0, 0.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![
        Rule::new().select(Selector::kind("rect")).duration(2.0),
        Rule::new().duration(1.0), // later but less specific: loses on duration
        Rule::new().when(None, Some("rank*")).duration(3.0).select(Selector::kind("rect")),
    ]);
    assert!((plan(&a, &b, &rules, &cx()).duration() - 2.0).abs() < 1e-12);
    let ranked = cx().states(Some("start"), Some("ranked"));
    assert!((plan(&a, &b, &rules, &ranked).duration() - 3.0).abs() < 1e-12);
    // JSON round trip (rules are part of the document IR).
    let json = serde_json::to_string(&rules).unwrap();
    let back: MotionRules = serde_json::from_str(&json).unwrap();
    assert_eq!(back, rules);
    let authored = r#"{"rules":[
        {"when":{"from":"*","to":"*"},"duration":1.2,"easing":"cubic-in-out"},
        {"when":{"to":"ranked"},"select":{"role":"datum"},"choreo":{"type":"stagger","order":"value","spread":0.5},"route":{"type":"arc","height":0.4}},
        {"select":{"kind":"text","role":"datum"},"enter":{"opacity":0},"delay":0.7},
        {"matcher":{"hierarchy":{"partition":"grid"}},"morph":"resample","exit":{"scale":0,"origin":{"baseline":200}}},
        {"choreo":{"type":"stagger","order":{"random":7}},"route":{"type":"drift","seed":3,"amount":20},"easing":"spring(170, 26)"}
    ]}"#;
    let r: MotionRules = serde_json::from_str(authored).unwrap();
    assert_eq!(r.rules.len(), 5);
    assert_eq!(r.rules[1].choreo, Some(Choreography::Stagger { order: Order::Value, spread: 0.5 }));
    assert_eq!(r.rules[3].exit, Some(Ghost::grow(Origin::Baseline(200.0))));
    assert_eq!(r.rules[4].easing, Some(Easing::spring(170.0, 26.0)));
    let again: MotionRules = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(again, r);
}

#[test]
fn groups_interpolate_and_children_stay_glued() {
    let mut ga = group("plot", vec![rect("x", 0.0, 0.0, 10.0, 10.0)]);
    ga.common.transform = Affine::translate(50.0, 50.0);
    let mut gb = group("plot", vec![rect("x", 0.0, 0.0, 10.0, 10.0)]);
    gb.common.transform = Affine::translate(150.0, 50.0);
    let p = plan(&scene(vec![ga]), &scene(vec![gb]), &linear(), &cx());
    let f = p.at(0.5, &NoShaper);
    assert_eq!(find(&f, &path(&["root", "plot"])).unwrap().common.transform, Affine::translate(100.0, 50.0));
    assert_eq!(*geom(find(&f, &path(&["root", "plot", "x"])).unwrap()), Geom::rect(0.0, 0.0, 10.0, 10.0));
}

#[test]
fn retarget_continues_from_the_current_frame() {
    let a = scene(vec![group("map", vec![circle("SE", 50.0, 50.0, 20.0)])]);
    let b = scene(vec![group("bars", vec![rect("SE", 250.0, 50.0, 30.0, 200.0)])]);
    let c = scene(vec![group("bars", vec![rect("SE", 300.0, 150.0, 30.0, 100.0)])]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).matcher(Matcher::ByKey)]);
    let p1 = plan(&a, &b, &rules, &cx());
    let mid = p1.at(0.4, &NoShaper);
    let p2 = retarget(&mid, &c, &rules, &cx());
    assert_eq!(p2.at(0.0, &NoShaper), p1.at(0.4, &NoShaper), "retarget at 0.4: at(0) == old at(0.4)");
    // The in-flight element continues (matched by its logical path), it does not crossfade.
    assert_eq!((p2.stats().pairs, p2.stats().enters, p2.stats().exits, p2.stats().crossfades), (1, 0, 0, 0));
    // Just after, it is still where it was.
    let bounds_of = |s: &Scene| {
        let mut out = None;
        walk(s, &mut |_, n| {
            if n.key == key("SE") {
                if let NodeKind::Shape { geom, .. } = &n.kind {
                    let b = geom.bounds();
                    let xf = n.common.transform;
                    out = Some(Rect::from_points(xf.apply(Vec2::new(b.x, b.y)), xf.apply(Vec2::new(b.x1(), b.y1()))));
                }
            }
        });
        out.unwrap()
    };
    let (b0, b1) = (bounds_of(&mid), bounds_of(&p2.at(1e-6, &NoShaper)));
    assert!((b0.center() - b1.center()).len() < 0.01 && (b0.w - b1.w).abs() < 0.05, "{b0:?} vs {b1:?}");
    assert_eq!(p2.at(1.0, &NoShaper), c);
}

#[test]
fn plans_are_send_and_sync() {
    fn is<T: Send + Sync + Clone>() {}
    is::<Plan>();
}

#[test]
fn stagger_data_order_is_tree_order() {
    // Keys sort as a < m < z, but the tree says z, a, m.
    let a = bars(&[("z", 0.0, 0.0, 10.0, 10.0), ("a", 20.0, 0.0, 10.0, 10.0), ("m", 40.0, 0.0, 10.0, 10.0)]);
    let b = bars(&[("z", 0.0, 50.0, 10.0, 10.0), ("a", 20.0, 50.0, 10.0, 10.0), ("m", 40.0, 50.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![Rule::new().choreo(Choreography::Stagger { order: Order::Data, spread: 0.5 })]);
    let w: std::collections::BTreeMap<String, f64> = plan(&a, &b, &rules, &cx()).windows().into_iter().map(|(k, w)| (k.to_string(), w.0)).collect();
    assert!(w["(\"z\",)"] < w["(\"a\",)"] && w["(\"a\",)"] < w["(\"m\",)"], "{w:?}");
}

#[test]
fn retarget_carries_velocity_into_springs() {
    let a = bars(&[("x", 0.0, 0.0, 10.0, 10.0)]);
    let b = bars(&[("x", 300.0, 0.0, 10.0, 10.0)]);
    let rules = MotionRules::new(vec![Rule::new().duration(1.0).easing(Easing::spring(170.0, 26.0))]);
    let x_of = |s: &Scene| {
        let n = find(s, &path(&["root", "plot", "x"])).unwrap();
        let x = match geom(n) {
            Geom::Rect { x, .. } => *x,
            _ => f64::NAN,
        };
        n.common.transform.apply(Vec2::new(x, 0.0)).x
    };
    let p1 = plan(&a, &b, &rules, &cx());
    let (t, dt) = (0.1, 1e-4);
    let v_before = (x_of(&p1.at(t + dt, &NoShaper)) - x_of(&p1.at(t - dt, &NoShaper))) / (2.0 * dt * p1.duration());
    assert!(v_before > 100.0, "moving fast at the interruption: {v_before} px/s");
    // Same destination, velocity carried: the motion continues at the same speed.
    let p2 = p1.retarget(t, &b, &rules, &cx(), &NoShaper);
    assert_eq!(p2.at(0.0, &NoShaper), p1.at(t, &NoShaper));
    let v_after = (x_of(&p2.at(dt, &NoShaper)) - x_of(&p2.at(0.0, &NoShaper))) / (dt * p2.duration());
    assert!((v_after - v_before).abs() < 0.1 * v_before, "velocity continues: {v_before} → {v_after}");
    // Plain retarget starts the spring from rest.
    let p3 = retarget(&p1.at(t, &NoShaper), &b, &rules, &cx());
    let v_rest = (x_of(&p3.at(dt, &NoShaper)) - x_of(&p3.at(0.0, &NoShaper))) / (dt * p3.duration());
    assert!(v_rest.abs() < 0.2 * v_before, "from rest: {v_rest}");
}

#[test]
fn a_bar_splits_into_its_seats_without_a_rule() {
    // ("S",) as one bar on one side; ("S", 0…5) as dots on the other: the default matcher splits
    // the bar into near-square cells (the seats are compact) and each flies to its seat.
    let mut bar = Node::shape(Key::one("S"), Geom::rect(0.0, 0.0, 30.0, 60.0));
    bar.semantics = Some(Semantics { role: Role::Datum, label: "S".into(), ..Default::default() });
    let a = scene(vec![bar]);
    let seats: Vec<Node> = (0..6)
        .map(|i| {
            let mut n = Node::shape(Key::one("S").child(KeyPart::Unit { unit: i }), Geom::circle(100.0 + 12.0 * i as f64, 20.0, 5.0));
            n.semantics = Some(Semantics { role: Role::Datum, label: "S".into(), ..Default::default() });
            n
        })
        .collect();
    let b = scene(seats);
    let p = plan(&a, &b, &linear(), &cx());
    let c = p.correspondence();
    assert_eq!(c.splits.len(), 1, "{c:?}");
    assert_eq!(c.splits[0].1.len(), 6);
    // Halfway, six pieces are in flight between the bar and the seats.
    let f = p.at(0.5, &FakeShaper::default());
    let mut keys = Vec::new();
    walk(&f, &mut |p, n| {
        if matches!(n.kind, NodeKind::Shape { .. }) {
            keys.push(p.to_string());
        }
    });
    assert_eq!(keys.iter().filter(|k| k.contains("Unit(")).count(), 6, "{keys:?}");
}

#[test]
fn a_title_that_changes_structure_takes_turns() {
    // One scene's title is a text; the next scene's is a group with a text (another recipe): the
    // old title leaves in the first half, the new one arrives in the second — never both at once.
    let a = scene(vec![label("title", "Seats per bloc", Vec2::new(0.0, 0.0))]);
    let b = scene(vec![group("title", vec![label("#0", "The new parliament", Vec2::new(0.0, 0.0))])]);
    let p = plan(&a, &b, &linear(), &cx());
    let w: Vec<(String, (f64, f64))> = p.windows().into_iter().map(|(k, w)| (k.to_string(), w)).collect();
    let old = w.iter().find(|(k, _)| k.contains("title")).map(|x| x.1);
    let new = w.iter().find(|(k, _)| k.contains("#0")).map(|x| x.1);
    assert_eq!((old, new), (Some((0.0, 0.5)), Some((0.5, 1.0))), "{w:?}");
}

#[test]
fn segments_merge_into_their_bar_in_proportion_to_their_values() {
    // ("Food", "Apr") 10 and ("Food", "May") 30 → the bar ("Food",): pieces as wide as their values.
    let seg = |m: &str, v: f64, y: f64| {
        let mut n = Node::shape(Key::new(vec!["Food".into(), m.into()]), Geom::rect(0.0, y, 20.0, 10.0));
        n.semantics = Some(Semantics { role: Role::Datum, value: Some(v), ..Default::default() });
        n
    };
    let a = scene(vec![seg("Apr", 10.0, 0.0), seg("May", 30.0, 20.0)]);
    let mut bar = Node::shape(Key::one("Food"), Geom::rect(100.0, 50.0, 200.0, 20.0));
    bar.semantics = Some(Semantics { role: Role::Datum, value: Some(40.0), ..Default::default() });
    let b = scene(vec![bar]);
    let p = plan(&a, &b, &linear(), &cx());
    assert_eq!(p.correspondence().merges.len(), 1);
    // At the end of their flight, the pieces tile the bar: 50 px for 10, 150 px for 30.
    let f = p.at(0.999, &FakeShaper::default());
    let mut widths = Vec::new();
    walk(&f, &mut |_, n| {
        if let NodeKind::Shape { geom: Geom::Rect { w, .. }, .. } = &n.kind {
            widths.push((*w * 10.0).round() / 10.0);
        }
    });
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert!(widths.len() >= 2, "{widths:?}");
    assert!((widths[0] - 50.0).abs() < 2.0 && (widths[1] - 150.0).abs() < 2.0, "{widths:?}");
}

fn zigzag(k: &str) -> Node {
    line(k, &[(40.0, 200.0), (100.0, 170.0), (160.0, 190.0), (220.0, 140.0), (280.0, 150.0)])
}

fn trim_at(p: &Plan, t: f64, at: &KeyPath) -> (Option<[f64; 2]>, f64) {
    let f = p.at(t, &NoShaper);
    let n = find(&f, at).unwrap_or_else(|| panic!("{at} at {t}"));
    (n.common.trim, n.common.opacity)
}

#[test]
fn a_trim_ghost_draws_a_line_on_and_off() {
    let (empty, with) = (scene(vec![group("plot", vec![])]), scene(vec![group("plot", vec![zigzag("SE")])]));
    let at = path(&["root", "plot", "SE"]);
    let rules = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::draw_on()).exit(Ghost::draw_on())]);
    let enter = plan(&empty, &with, &rules, &cx());
    assert_eq!(trim_at(&enter, 0.25, &at), (Some([0.0, 0.25]), 1.0), "a quarter drawn, at full opacity");
    assert_eq!(enter.at(1.0, &NoShaper), with, "exactly the line at the end (untrimmed)");
    let exit = plan(&with, &empty, &rules, &cx());
    assert_eq!(trim_at(&exit, 0.25, &at), (Some([0.0, 0.75]), 1.0), "draws off from its end");
    // A line already trimmed draws on within its own trim.
    let mut part = with.clone();
    if let Some(c) = part.root.children_mut().and_then(|c| c[0].children_mut()) {
        c[0].common.trim = Some([0.2, 0.6]);
    }
    let p = plan(&empty, &part, &rules, &cx());
    let (tr, _) = trim_at(&p, 0.5, &at);
    let tr = tr.unwrap();
    assert!((tr[0] - 0.2).abs() < 1e-12 && (tr[1] - 0.4).abs() < 1e-12, "{tr:?}");
    // The ghost survives serde (the SDK writes `enter: { trim: 0 }`).
    let g: Ghost = serde_json::from_str(r#"{ "trim": 0 }"#).unwrap();
    assert_eq!(g, Ghost::draw_on());
}

#[test]
fn defaults_sit_under_the_documents_rules() {
    let (empty, with) = (scene(vec![group("plot", vec![zigzag("SE")]), group("other", vec![])]), scene(vec![group("plot", vec![zigzag("SE"), zigzag("NO")]), group("other", vec![zigzag("DK")])]));
    // A recipe's default, scoped to its key path (`root/plot`): lines there draw on.
    let default = Rule::new().select(Selector { kind: Some("polyline".into()), key_prefix: Some(path(&["root", "plot"])), ..Default::default() }).enter(Ghost::draw_on());
    let no = path(&["root", "plot", "NO"]);
    let dk = path(&["root", "other", "DK"]);
    // A document rule that only sets timing keeps the default's ghost.
    let timing = MotionRules::new(vec![Rule::new().easing(Easing::Linear)]).with_defaults(vec![default.clone()]);
    let p = plan(&empty, &with, &timing, &cx());
    assert_eq!(trim_at(&p, 0.5, &no), (Some([0.0, 0.5]), 1.0), "draws on");
    assert_eq!(trim_at(&p, 0.5, &dk), (None, 0.5), "outside the recipe: the default fade");
    // A document rule that sets the ghost wins, however much less specific it is.
    let fade = MotionRules::new(vec![Rule::new().easing(Easing::Linear).enter(Ghost::fade())]).with_defaults(vec![default]);
    let p = plan(&empty, &with, &fade, &cx());
    assert_eq!(trim_at(&p, 0.5, &no), (None, 0.5), "the document's fade");
}
