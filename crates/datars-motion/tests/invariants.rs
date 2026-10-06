//! Property tests over seeded random scenes: the hard invariants of every plan.
//!
//! - `at(0) == from`, `at(1) == to` exactly;
//! - no NaN / infinity at any t; opacities within [0, 1];
//! - entering elements' opacity never decreases, exiting elements' never increases;
//! - morphing outlines never turn inside out;
//! - frames are deterministic.

mod common;

use common::*;
use datars_motion::*;
use datars_scene::{KeyPath, NodeKind, Scene};

fn rule_sets() -> Vec<(&'static str, MotionRules)> {
    let r = |rules: Vec<Rule>| MotionRules::new(rules);
    vec![
        ("default", r(vec![])),
        ("by-key", r(vec![Rule::new().matcher(Matcher::ByKey)])),
        ("hierarchy", r(vec![Rule::new().matcher(Matcher::Hierarchy { partition: Partition::Grid })])),
        ("nearest", r(vec![Rule::new().matcher(Matcher::Nearest)])),
        ("none", r(vec![Rule::new().matcher(Matcher::None)])),
        (
            "stagger-arc-back",
            r(vec![Rule::new()
                .choreo(Choreography::Stagger { order: Order::Left, spread: 0.6 })
                .route(Route::Arc { height: 0.4 })
                .easing(Easing::parse("back-out").unwrap())
                .enter(Ghost::grow(Origin::Bottom))
                .exit(Ghost::rise(12.0))]),
        ),
        (
            "phased-spring-resample",
            r(vec![Rule::new().choreo(Choreography::Phased { exit: 0.3, update: 0.5, enter: 0.2 }).easing(Easing::spring(120.0, 8.0)).morph(MorphStrategy::Resample)]),
        ),
        (
            "ripple-elastic-drift",
            r(vec![
                Rule::new().choreo(Choreography::Ripple { origin: None, spread: 0.7 }).easing(Easing::parse("elastic-out").unwrap()).route(Route::Drift { seed: 3, amount: 30.0 }),
                Rule::new().select(Selector::kind("text")).delay(0.3).enter(Ghost::from_parent()),
            ]),
        ),
        (
            "wave-hop-steps-crossfade",
            r(vec![Rule::new().choreo(Choreography::Wave { spread: 0.5, angle: 0.3 }).route(Route::Hop { height: 20.0 }).easing(Easing::Steps(5, StepPosition::JumpBoth)).morph(MorphStrategy::Crossfade)]),
        ),
        (
            "value-order-spiral-bezier",
            r(vec![Rule::new()
                .matcher(Matcher::ByKey)
                .choreo(Choreography::Stagger { order: Order::Value, spread: 0.4 })
                .route(Route::Spiral { turns: 0.5 })
                .easing(Easing::CubicBezier(0.3, -0.4, 0.7, 1.4))
                .enter(Ghost::from_point(datars_math::Vec2::new(10.0, 10.0)))]),
        ),
    ]
}

const TS: [f64; 13] = [0.0, 1e-9, 0.01, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 0.99, 1.0 - 1e-9, 1.0];

/// Own opacity of the (last) node at `p`, or of the instance with `key` in an Instances node at
/// `p`'s parent; 0 when absent.
fn opacity_of(s: &Scene, p: &KeyPath) -> f64 {
    if let Some(n) = find(s, p) {
        return if n.common.visible { n.common.opacity } else { 0.0 };
    }
    0.0
}

#[test]
fn endpoints_are_exact_and_frames_finite() {
    let cxp = cx();
    for seed in 0..12u64 {
        let (a, b) = (random_scene(seed, 0), random_scene(seed, 1));
        for (name, rules) in rule_sets() {
            let p = plan(&a, &b, &rules, &cxp);
            let sh = FakeShaper::default();
            assert_eq!(p.at(0.0, &sh), a, "{name} seed {seed}: at(0) == from");
            assert_eq!(p.at(1.0, &sh), b, "{name} seed {seed}: at(1) == to");
            assert_eq!(p.at(-0.5, &sh), a);
            assert_eq!(p.at(1.5, &sh), b);
            assert_eq!(p.at(f64::NAN, &sh), a);
            assert!(p.duration() > 0.0 && p.duration().is_finite());
            for t in TS {
                assert_finite(&p.at(t, &sh));
            }
        }
    }
}

#[test]
fn entering_opacity_never_decreases_and_exiting_never_increases() {
    let cxp = cx();
    for seed in 0..12u64 {
        let (a, b) = (random_scene(seed, 0), random_scene(seed, 1));
        for (name, rules) in rule_sets() {
            let p = plan(&a, &b, &rules, &cxp);
            let sh = FakeShaper::default();
            let frames: Vec<Scene> = (0..=60).map(|i| p.at(i as f64 / 60.0, &sh)).collect();
            let corr = p.correspondence();
            // Paths reused by another element on the other side (possible with by-position
            // matching) are not about one element; skip them.
            for e in corr.enters.iter().filter(|e| find(&a, e).is_none()) {
                let mut last = -1.0;
                for (i, f) in frames.iter().enumerate() {
                    let o = opacity_of(f, e);
                    assert!(o >= last - 1e-12, "{name} seed {seed}: entering {e} dims at frame {i}: {last} → {o}");
                    last = o;
                }
            }
            for e in corr.exits.iter().filter(|e| find(&b, e).is_none()) {
                let mut last = f64::INFINITY;
                for (i, f) in frames.iter().enumerate() {
                    let o = opacity_of(f, e);
                    assert!(o <= last + 1e-12, "{name} seed {seed}: exiting {e} brightens at frame {i}: {last} → {o}");
                    last = o;
                }
            }
            // Column instances: entering keys only brighten, exiting ones only fade.
            let inst_opacity = |f: &Scene, k: &datars_scene::Key, exiting: bool| -> Option<f64> {
                let dots = find_all(f, &path(&["root", "dots"]));
                // Exits ride in the node too (after the target's instances).
                let mut best = None;
                for n in dots {
                    if let NodeKind::Instances(i) = &n.kind {
                        let idx = if exiting { i.keys.iter().rposition(|q| q == k) } else { i.keys.iter().position(|q| q == k) };
                        if let Some(ix) = idx {
                            best = Some(i.opacity_at(ix));
                        }
                    }
                }
                best
            };
            let keys_of = |s: &Scene| -> Vec<datars_scene::Key> {
                match find(s, &path(&["root", "dots"])).map(|n| &n.kind) {
                    Some(NodeKind::Instances(i)) => i.keys.clone(),
                    _ => Vec::new(),
                }
            };
            if !matches!(name, "default" | "none") {
                continue; // other matchers may fly instances out of the column
            }
            let (ka, kb) = (keys_of(&a), keys_of(&b));
            for k in kb.iter().filter(|k| !ka.contains(k)) {
                let mut last = -1.0;
                for f in &frames[1..frames.len() - 1] {
                    let o = inst_opacity(f, k, false).unwrap_or(0.0);
                    assert!(o >= last - 1e-12, "{name} seed {seed}: entering instance {k} dims");
                    last = o;
                }
            }
            for k in ka.iter().filter(|k| !kb.contains(k)) {
                let mut last = f64::INFINITY;
                for f in &frames[1..frames.len() - 1] {
                    let o = inst_opacity(f, k, true).unwrap_or(0.0);
                    assert!(o <= last + 1e-12, "{name} seed {seed}: exiting instance {k} brightens");
                    last = o;
                }
            }
        }
    }
}

#[test]
fn morphing_outlines_never_turn_inside_out() {
    let cxp = cx();
    let mut checked = 0;
    for seed in 0..16u64 {
        let (a, b) = (random_scene(seed, 0), random_scene(seed, 1));
        for (name, rules) in rule_sets() {
            let p = plan(&a, &b, &rules, &cxp);
            let sh = FakeShaper::default();
            for (pa, pb) in &p.correspondence().pairs {
                let (Some(na), Some(nb)) = (find(&a, pa), find(&b, pb)) else { continue };
                let (NodeKind::Shape { geom: ga, .. }, NodeKind::Shape { geom: gb, .. }) = (&na.kind, &nb.kind) else { continue };
                if std::mem::discriminant(ga) == std::mem::discriminant(gb) || !ga.is_closed() || !gb.is_closed() {
                    continue;
                }
                if main_area(ga).abs() < 1.0 || main_area(gb).abs() < 1.0 {
                    continue;
                }
                for i in 1..40 {
                    let f = p.at(i as f64 / 40.0, &sh);
                    for n in find_all(&f, pb) {
                        if let NodeKind::Shape { geom: g @ datars_scene::Geom::Path { .. }, .. } = &n.kind {
                            if g == ga || g == gb {
                                continue; // window not open yet / already closed: the source shapes
                            }
                            let area = main_area(g);
                            assert!(area > 0.0, "{name} seed {seed}: {pb} inside out at {i}/40 (area {area})");
                            checked += 1;
                        }
                    }
                    // Flyers live under the flight layer.
                    let mut fp = vec![datars_scene::Key::name("root"), datars_scene::Key::name(FLIGHT_KEY)];
                    fp.extend(pb.0.iter().skip(1).cloned());
                    for n in find_all(&f, &KeyPath(fp)) {
                        if let NodeKind::Shape { geom: g @ datars_scene::Geom::Path { .. }, .. } = &n.kind {
                            if g == ga || g == gb {
                                continue;
                            }
                            let area = main_area(g);
                            assert!(area > 0.0, "{name} seed {seed}: flying {pb} inside out at {i}/40 (area {area})");
                            checked += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 100, "enough morph frames checked: {checked}");
}

#[test]
fn frames_are_deterministic() {
    let cxp = cx();
    for seed in 0..4u64 {
        let (a, b) = (random_scene(seed, 0), random_scene(seed, 1));
        for (_, rules) in rule_sets() {
            let p1 = plan(&a, &b, &rules, &cxp);
            let p2 = plan(&a, &b, &rules, &cxp);
            let sh = FakeShaper::default();
            for t in [0.2, 0.5, 0.8] {
                let (f1, f2) = (p1.at(t, &sh), p2.at(t, &sh));
                assert_eq!(f1, f2);
                assert_eq!(f1.hash(), f2.hash());
            }
        }
    }
}

#[test]
fn unchanged_scene_stays_put() {
    let cxp = cx();
    for seed in 0..6u64 {
        let a = random_scene(seed, 0);
        let p = plan(&a, &a, &MotionRules::default(), &cxp);
        let sh = FakeShaper::default();
        assert_eq!(p.at(0.5, &sh), a, "seed {seed}: a no-op plan reproduces the scene mid-way");
        assert_eq!(p.stats().enters + p.stats().exits, 0);
    }
}

#[test]
fn frames_retarget_from_anywhere() {
    // A frame of any plan is itself a valid `from`: planning onward is exact at 0 and finite.
    let cxp = cx();
    for seed in 0..8u64 {
        let (a, b, c) = (random_scene(seed, 0), random_scene(seed, 1), random_scene(seed, 2));
        for (name, rules) in rule_sets() {
            let p = plan(&a, &b, &rules, &cxp);
            let sh = FakeShaper::default();
            let mid = p.at(0.4, &sh);
            let q = retarget(&mid, &c, &rules, &cxp);
            assert_eq!(q.at(0.0, &sh), mid, "{name} seed {seed}");
            assert_eq!(q.at(1.0, &sh), c);
            for t in [0.01, 0.3, 0.7, 0.99] {
                assert_finite(&q.at(t, &sh));
            }
        }
    }
}
