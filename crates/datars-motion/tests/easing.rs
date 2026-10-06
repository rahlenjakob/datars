//! Easing curves: exact endpoints, CSS compatibility, closed-form springs, parsing and serde.

use datars_motion::*;

fn all_easings() -> Vec<Easing> {
    let mut v = vec![Easing::Linear];
    for f in Family::ALL {
        for d in [Dir::In, Dir::Out, Dir::InOut] {
            v.push(Easing::Named(f, d));
        }
    }
    v.extend([
        Easing::css_ease(),
        Easing::CubicBezier(0.42, 0.0, 0.58, 1.0),
        Easing::CubicBezier(0.2, 0.8, 0.2, 1.0),
        Easing::CubicBezier(0.3, -0.5, 0.7, 1.5),
        Easing::CubicBezier(0.0, 0.0, 1.0, 1.0),
        Easing::spring(170.0, 26.0),
        Easing::spring(100.0, 5.0),
        Easing::spring(100.0, 40.0),
        Easing::spring(100.0, 0.0),
        Easing::Spring { stiffness: 300.0, damping: 20.0, mass: 2.0, v0: 5.0 },
        Easing::Spring { stiffness: 170.0, damping: 26.0, mass: 1.0, v0: -3.0 },
    ]);
    for pos in [StepPosition::JumpStart, StepPosition::JumpEnd, StepPosition::JumpNone, StepPosition::JumpBoth] {
        v.push(Easing::Steps(4, pos));
        v.push(Easing::Steps(1, pos));
    }
    v.push(Easing::parse("keyframes(0 0; 0.3 0.8 cubic-out; 0.7 0.6; 1 1 back-out)").unwrap());
    v
}

#[test]
fn every_easing_hits_zero_and_one_exactly() {
    for e in all_easings() {
        assert_eq!(e.apply(0.0), 0.0, "{e}");
        assert_eq!(e.apply(1.0), 1.0, "{e}");
        assert_eq!(e.apply(-1.0), 0.0, "{e}");
        assert_eq!(e.apply(2.0), 1.0, "{e}");
        assert_eq!(e.apply(f64::NAN), 0.0, "{e}");
        for i in 0..=1000 {
            let v = e.apply(i as f64 / 1000.0);
            assert!(v.is_finite() && v.abs() < 3.0, "{e} at {i}: {v}");
        }
    }
}

#[test]
fn monotone_easings_never_decrease() {
    for e in all_easings().into_iter().filter(|e| e.is_monotone()) {
        let mut last = 0.0;
        for i in 0..=2000 {
            let v = e.apply(i as f64 / 2000.0);
            assert!(v >= last - 1e-12, "{e} decreases at {i}: {last} → {v}");
            assert!((-1e-12..=1.0 + 1e-12).contains(&v), "{e} leaves [0, 1] at {i}: {v}");
            last = v;
        }
    }
    assert!(!Easing::parse("back-out").unwrap().is_monotone());
    assert!(!Easing::parse("bounce").unwrap().is_monotone());
    assert!(!Easing::spring(100.0, 5.0).is_monotone());
    assert!(Easing::parse("expo-in-out").unwrap().is_monotone());
}

/// Reference: solve x(s) = t by 200 bisection steps.
fn bezier_ref(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    let bz = |p1: f64, p2: f64, s: f64| 3.0 * (1.0 - s) * (1.0 - s) * s * p1 + 3.0 * (1.0 - s) * s * s * p2 + s * s * s;
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if bz(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bz(y1, y2, (lo + hi) / 2.0)
}

#[test]
fn cubic_bezier_matches_css_ease() {
    let ease = Easing::parse("ease").unwrap();
    // CSS `ease` = cubic-bezier(0.25, 0.1, 0.25, 1): ease(0.5) = 0.8024034 (the textbook value);
    // x(s) = 0.25 at s ≈ 0.4094 gives y ≈ 0.4085; x(s) = 0.75 at s ≈ 0.8739 gives y ≈ 0.9605.
    assert!((ease.apply(0.5) - 0.802_403_4).abs() < 1e-6, "{}", ease.apply(0.5));
    assert!((ease.apply(0.25) - 0.4085).abs() < 1e-3, "{}", ease.apply(0.25));
    assert!((ease.apply(0.75) - 0.9605).abs() < 1e-3, "{}", ease.apply(0.75));
    for (x1, y1, x2, y2) in [(0.25, 0.1, 0.25, 1.0), (0.42, 0.0, 0.58, 1.0), (0.2, 0.8, 0.2, 1.0), (0.68, -0.55, 0.27, 1.55), (0.0, 0.0, 1.0, 1.0)] {
        let e = Easing::CubicBezier(x1, y1, x2, y2);
        for i in 1..100 {
            let t = i as f64 / 100.0;
            assert!((e.apply(t) - bezier_ref(x1, y1, x2, y2, t)).abs() < 1e-9, "cubic-bezier({x1},{y1},{x2},{y2}) at {t}");
        }
    }
    let io = Easing::parse("ease-in-out").unwrap();
    assert!((io.apply(0.5) - 0.5).abs() < 1e-9);
}

#[test]
fn spring_is_closed_form_monotone_ish_and_lands_on_one() {
    let s = Easing::parse("spring(170, 26)").unwrap();
    assert_eq!(s, Easing::spring(170.0, 26.0));
    let mut last = 0.0;
    let mut max = 0.0f64;
    for i in 0..=1000 {
        let v = s.apply(i as f64 / 1000.0);
        assert!(v >= last - 1e-3, "near-critically damped: no visible reversal at {i}: {last} → {v}");
        max = max.max(v);
        last = v;
    }
    assert!(max <= 1.01, "overshoot stays tiny: {max}");
    assert!(s.apply(0.3) > 0.7 && s.apply(0.5) > 0.9, "a spring leaves fast: {} {}", s.apply(0.3), s.apply(0.5));
    assert_eq!(s.apply(1.0), 1.0);
    // An underdamped spring rings past its target and settles.
    let bouncy = Easing::spring(100.0, 5.0);
    let peak = (0..1000).map(|i| bouncy.apply(i as f64 / 1000.0)).fold(0.0, f64::max);
    assert!(peak > 1.2, "rings: {peak}");
    assert!((bouncy.apply(0.999) - 1.0).abs() < 0.01);
    // Initial velocity: a spring already moving towards its target leads one at rest.
    let moving = Easing::Spring { stiffness: 170.0, damping: 26.0, mass: 1.0, v0: 20.0 };
    assert!(moving.apply(0.05) > s.apply(0.05));
    // Seekable: the same t gives the same value, in any order.
    let a: Vec<f64> = (0..50).map(|i| bouncy.apply(i as f64 / 50.0)).collect();
    let b: Vec<f64> = (0..50).rev().map(|i| bouncy.apply(i as f64 / 50.0)).collect();
    assert_eq!(a, b.into_iter().rev().collect::<Vec<_>>());
    assert!(spring_settle_time(170.0, 26.0, 1.0, 0.0) > 0.3 && spring_settle_time(170.0, 26.0, 1.0, 0.0) < 1.5);
}

#[test]
fn steps_follow_css() {
    let e = Easing::parse("steps(4)").unwrap();
    assert_eq!(e, Easing::Steps(4, StepPosition::JumpEnd));
    assert_eq!(e.apply(0.1), 0.0);
    assert_eq!(e.apply(0.3), 0.25);
    assert_eq!(e.apply(0.99), 0.75);
    let s = Easing::parse("steps(4, jump-start)").unwrap();
    assert_eq!(s.apply(0.1), 0.25);
    assert_eq!(s.apply(0.8), 1.0);
    let b = Easing::parse("steps(3, jump-both)").unwrap();
    assert_eq!(b.apply(0.1), 0.25);
    let n = Easing::parse("steps(3, jump-none)").unwrap();
    assert_eq!(n.apply(0.5), 0.5);
    assert_eq!(Easing::parse("step-end"), Some(Easing::Steps(1, StepPosition::JumpEnd)));
}

#[test]
fn keyframed_curves_pass_through_their_keys() {
    let k = Easing::parse("keyframes(0 0; 0.5 0.8 cubic-in-out; 1 1)").unwrap();
    assert!((k.apply(0.5) - 0.8).abs() < 1e-12);
    assert!((k.apply(0.25) - 0.4).abs() < 1e-12, "linear first segment");
    assert!((k.apply(0.75) - 0.9).abs() < 1e-12, "cubic in-out midpoint of the second segment");
}

#[test]
fn parse_display_and_serde_round_trip() {
    for s in [
        "linear",
        "cubic-in-out",
        "quad-in",
        "expo-out",
        "back-out",
        "bounce-in-out",
        "cubic-bezier(0.2,0.8,0.2,1)",
        "spring(170, 26)",
        "spring(300, 20, 2, 5)",
        "steps(4)",
        "steps(2, jump-none)",
        "keyframes(0 0; 0.5 0.8 cubic-out; 1 1)",
    ] {
        let e = Easing::parse(s).unwrap_or_else(|| panic!("parses {s}"));
        assert_eq!(e.to_string(), s, "canonical form");
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        let back: Easing = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
    }
    assert_eq!(Easing::parse("cubic"), Some(Easing::CUBIC_IN_OUT));
    assert_eq!(Easing::parse("elastic"), Some(Easing::Named(Family::Elastic, Dir::Out)));
    assert_eq!(Easing::parse(" Cubic-Bezier( .2 , .8 , .2 , 1 ) "), Some(Easing::CubicBezier(0.2, 0.8, 0.2, 1.0)));
    for bad in ["", "cubic-bezier(1,2,3)", "spring(1)", "steps(0)", "steps(2, sideways)", "wobble", "cubic-inout"] {
        assert_eq!(Easing::parse(bad), None, "rejects {bad:?}");
    }
    assert!(serde_json::from_str::<Easing>("\"wobble\"").is_err());
    assert_eq!(Easing::default(), Easing::CUBIC_IN_OUT);
}
