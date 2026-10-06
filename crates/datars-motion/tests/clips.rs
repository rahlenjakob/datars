//! The clip algebra: durations, seeking, fill semantics, and applying patches to scenes.

mod common;

use common::*;
use datars_math::{Affine, Vec2};
use datars_motion::clip::*;
use datars_motion::Easing;
use datars_scene::{Key, KeyPath, NodeKind, Paint};
use datars_theme::Ink;

fn target(k: &str) -> KeyPath {
    path(&["root", k])
}

fn fade(k: &str, d: f64) -> Clip {
    tween(target(k), PropValue::Opacity(0.0), PropValue::Opacity(1.0), d)
}

fn opacity(c: &Clip, t: f64, k: &str) -> Option<f64> {
    c.at(t).into_iter().find(|(p, _)| *p == target(k)).and_then(|(_, p)| p.opacity)
}

#[test]
fn durations_compose() {
    let (a, b) = (fade("a", 1.0), fade("b", 2.0));
    assert_eq!(seq(vec![a.clone(), b.clone()]).duration(), 3.0);
    assert_eq!(par(vec![a.clone(), b.clone()]).duration(), 2.0);
    assert_eq!(delay(0.5, a.clone()).duration(), 1.5);
    assert_eq!(stretch(3.0, a.clone()).duration(), 3.0);
    assert_eq!(reverse(b.clone()).duration(), 2.0);
    assert_eq!(loop_n(4, a.clone()).duration(), 4.0);
    assert_eq!(ease(Easing::CUBIC_IN_OUT, b.clone()).duration(), 2.0);
    assert_eq!(stagger(0.25, vec![a.clone(), a.clone(), a.clone()]).duration(), 1.5);
    assert_eq!(hold(2.5).duration(), 2.5);
    assert_eq!(keyframes(target("a"), vec![(0.0, PropValue::Opacity(0.0), Easing::Linear), (1.5, PropValue::Opacity(1.0), Easing::Linear)]).duration(), 1.5);
}

#[test]
fn tweens_seek_and_hold_their_ends() {
    let a = fade("a", 2.0);
    assert_eq!(opacity(&a, 1.0, "a"), Some(0.5));
    assert_eq!(opacity(&a, -1.0, "a"), Some(0.0), "before: first frame");
    assert_eq!(opacity(&a, 5.0, "a"), Some(1.0), "after: last frame");
    let e = Clip::Tween { target: target("a"), from: PropValue::Opacity(0.0), to: PropValue::Opacity(1.0), duration: 1.0, easing: Easing::parse("quad-in").unwrap() };
    assert_eq!(opacity(&e, 0.5, "a"), Some(0.25));
}

#[test]
fn seq_plays_in_order_and_the_past_holds() {
    let out = tween(target("a"), PropValue::Opacity(1.0), PropValue::Opacity(0.0), 1.0);
    let c = seq(vec![fade("a", 1.0), hold(1.0), out, fade("b", 1.0)]);
    assert_eq!(opacity(&c, 0.5, "a"), Some(0.5));
    assert_eq!(opacity(&c, 1.5, "a"), Some(1.0), "held after the fade-in, before the fade-out");
    assert_eq!(opacity(&c, 2.5, "a"), Some(0.5));
    assert_eq!(opacity(&c, 0.5, "b"), Some(0.0), "b waits at its first frame");
    assert_eq!(opacity(&c, 3.5, "b"), Some(0.5));
    assert_eq!(opacity(&c, 3.5, "a"), Some(0.0), "a stays faded out");
}

#[test]
fn par_later_wins_and_delay_stretch_reverse_loop_ease() {
    let half = tween(target("a"), PropValue::Opacity(0.5), PropValue::Opacity(0.5), 1.0);
    assert_eq!(opacity(&par(vec![fade("a", 1.0), half.clone()]), 0.2, "a"), Some(0.5));
    let d = delay(1.0, fade("a", 1.0));
    assert_eq!(opacity(&d, 0.5, "a"), Some(0.0));
    assert_eq!(opacity(&d, 1.5, "a"), Some(0.5));
    let s = stretch(4.0, fade("a", 1.0));
    assert_eq!(opacity(&s, 1.0, "a"), Some(0.25));
    let r = reverse(fade("a", 1.0));
    assert_eq!(opacity(&r, 0.25, "a"), Some(0.75));
    let l = loop_n(3, fade("a", 1.0));
    assert_eq!(opacity(&l, 1.25, "a"), Some(0.25));
    assert_eq!(opacity(&l, 2.5, "a"), Some(0.5));
    assert_eq!(opacity(&l, 3.0, "a"), Some(1.0), "ends on the last frame");
    let e = ease(Easing::parse("quad-in").unwrap(), fade("a", 2.0));
    assert_eq!(opacity(&e, 1.0, "a"), Some(0.25));
}

#[test]
fn stagger_offsets_each_clip() {
    let c = stagger(0.5, vec![fade("a", 1.0), fade("b", 1.0), fade("c", 1.0)]);
    assert_eq!(opacity(&c, 0.5, "a"), Some(0.5));
    assert_eq!(opacity(&c, 0.5, "b"), Some(0.0));
    assert_eq!(opacity(&c, 1.25, "c"), Some(0.25));
}

#[test]
fn keyframes_interpolate_segments() {
    let k = keyframes(
        target("a"),
        vec![(0.0, PropValue::Translate(Vec2::new(0.0, 20.0)), Easing::Linear), (1.0, PropValue::Translate(Vec2::ZERO), Easing::parse("quad-in").unwrap()), (2.0, PropValue::Translate(Vec2::new(10.0, 0.0)), Easing::Linear)],
    );
    let tr = |t: f64| k.at(t)[0].1.translate.unwrap();
    assert_eq!(tr(0.5), Vec2::new(0.0, 10.0));
    assert_eq!(tr(1.5), Vec2::new(2.5, 0.0));
    assert_eq!(tr(3.0), Vec2::new(10.0, 0.0));
    assert_eq!(tr(-1.0), Vec2::new(0.0, 20.0));
}

#[test]
fn apply_composes_patches_onto_a_scene() {
    let mut half = rect("a", 0.0, 0.0, 10.0, 10.0);
    half.common.opacity = 0.5;
    let s = scene(vec![half, rect("b", 20.0, 0.0, 10.0, 20.0), number_text("n", 3.0, Vec2::ZERO)]);
    let clip = par(vec![
        fade("a", 1.0),
        tween(target("b"), PropValue::Translate(Vec2::new(0.0, 20.0)), PropValue::Translate(Vec2::ZERO), 1.0),
        tween(target("b"), PropValue::Scale(0.0, 0.0), PropValue::Scale(1.0, 1.0), 1.0),
        fill_tween(target("a"), datars_color::Color::BLACK, datars_color::Color::WHITE, 1.0),
        tween(target("n"), PropValue::Number(0.0), PropValue::Number(100.0), 1.0),
    ]);
    let out = apply(&s, &clip.at(0.5));
    let a = find(&out, &target("a")).unwrap();
    assert_eq!(a.common.opacity, 0.25, "opacity multiplies");
    let NodeKind::Shape { fill: Some(Paint::Solid(Ink::Color(c))), .. } = &a.kind else { panic!() };
    assert_eq!(*c, datars_color::Color::BLACK.lerp_oklab(datars_color::Color::WHITE, 0.5));
    let b = find(&out, &target("b")).unwrap();
    // Scale ½ about the centre (25, 10), then translate down 10.
    let xf = b.common.transform;
    assert_eq!(xf.apply(Vec2::new(25.0, 10.0)), Vec2::new(25.0, 20.0));
    assert_eq!(xf.apply(Vec2::new(30.0, 20.0)), Vec2::new(27.5, 25.0));
    let shaped = apply_shaped(&s, &clip.at(0.5), &FakeShaper::default());
    let NodeKind::Text(t) = &find(&shaped, &target("n")).unwrap().kind else { panic!() };
    assert_eq!(t.text, "50");
    // Patches at t = 0 of an identity-ending clip leave the scene at its end state at t = end.
    let end = apply(&s, &par(vec![tween(target("b"), PropValue::Translate(Vec2::new(5.0, 5.0)), PropValue::Translate(Vec2::ZERO), 1.0)]).at(1.0));
    assert_eq!(find(&end, &target("b")).unwrap().common.transform, Affine::IDENTITY);
}

#[test]
fn instance_patches_target_one_instance() {
    let keys: Vec<Key> = (0..3).map(|i| Key::one(i as i64)).collect();
    let s = scene(vec![instances("dots", keys, vec![0.0, 10.0, 20.0], vec![0.0; 3], 2.0)]);
    let mut p = path(&["root", "dots"]);
    p.0.push(Key::one(1));
    let clip = tween(p, PropValue::Translate(Vec2::new(0.0, 10.0)), PropValue::Translate(Vec2::ZERO), 1.0);
    let out = apply(&s, &clip.at(0.0));
    let NodeKind::Instances(i) = &find(&out, &path(&["root", "dots"])).unwrap().kind else { panic!() };
    assert_eq!(i.y, vec![0.0, 10.0, 0.0]);
}

#[test]
fn clips_serialize_for_the_ir() {
    let c = seq(vec![fade("a", 1.0), delay(0.5, loop_n(2, reverse(fade("b", 0.5)))), stagger(0.1, vec![fade("c", 1.0)])]);
    let json = serde_json::to_string(&c).unwrap();
    let back: Clip = serde_json::from_str(&json).unwrap();
    assert_eq!(back, c);
    assert_eq!(back.at(1.7), c.at(1.7));
}
