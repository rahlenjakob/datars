//! Point markers ride their line: dots sitting on a line's vertices stay on the line in every
//! frame of a transition — when values change, when points come and go, when x moves (a sliding
//! window), when the line and its dots have different rules, and while the line draws on.

mod common;

use common::*;
use datars_math::Vec2;
use datars_motion::*;
use datars_scene::*;

/// A series: an x → y table drawn as a line (with `curve`) plus a dot per row keyed by its x,
/// spread over 300 px horizontally from `x0` to `x1` (data units).
fn series(rows: &[(i64, f64)], x0: f64, x1: f64, curve: Curve) -> Node {
    let sx = |x: i64| (x as f64 - x0) / (x1 - x0) * 300.0;
    let sy = |y: f64| 200.0 - y * 15.0;
    let pts: Vec<Vec2> = rows.iter().map(|&(x, y)| Vec2::new(sx(x), sy(y))).collect();
    let line = Node::shape(key("line"), Geom::Polyline { pts: pts.clone().into(), closed: false, curve })
        .stroke(Stroke { paint: Paint::Solid(datars_theme::Ink::token("accent")), width: 2.0, dash: None, cap: Cap::Round, join: Join::Round, non_scaling: false });
    let dots = instances("points", rows.iter().map(|&(x, _)| Key::one(x)).collect(), pts.iter().map(|p| p.x).collect(), pts.iter().map(|p| p.y).collect(), 3.0);
    group("s", vec![line, dots])
}

fn chart(rows: &[(i64, f64)], x0: f64, x1: f64, curve: Curve) -> Scene {
    scene(vec![group("plot", vec![series(rows, x0, x1, curve)])])
}

fn rows(xs: std::ops::RangeInclusive<i64>, f: impl Fn(i64) -> f64) -> Vec<(i64, f64)> {
    xs.map(|x| (x, f(x))).collect()
}

fn dist_to_segment(q: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l2 = d.x * d.x + d.y * d.y;
    let t = if l2 > 0.0 { (((q - a).x * d.x + (q - a).y * d.y) / l2).clamp(0.0, 1.0) } else { 0.0 };
    q.dist(a.lerp(b, t))
}

/// The worst distance (px) from a visible dot to the drawn line in the frame at `t`, and how far
/// along the line (fraction of its length) the farthest visible dot is past the trim's end.
fn worst(p: &Plan, t: f64) -> (f64, f64) {
    let f = p.at(t, &NoShaper);
    let at = |k: &str| path(&["root", "plot", "s", k]);
    let line = find(&f, &at("line")).expect("the line");
    let flat = geom(line).to_path().flatten(0.005);
    let pts = &flat[0].0;
    let mut cum = vec![0.0];
    for w in pts.windows(2) {
        cum.push(cum.last().unwrap() + w[0].dist(w[1]));
    }
    let total = *cum.last().unwrap();
    let trim_end = line.common.trim.map_or(1.0, |t| t[1]);
    let (mut far, mut past) = (0.0f64, 0.0f64);
    for n in find_all(&f, &at("points")) {
        let NodeKind::Instances(ins) = &n.kind else { panic!("instances") };
        for i in 0..ins.len() {
            if ins.opacity[i] * n.common.opacity <= 1e-6 {
                continue;
            }
            let q = Vec2::new(ins.x[i], ins.y[i]);
            let (k, d) = pts.windows(2).enumerate().map(|(k, w)| (k, dist_to_segment(q, w[0], w[1]))).fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
            far = far.max(d);
            let along = (cum[k] + pts[k].dist(q).min(cum[k + 1] - cum[k])) / total;
            past = past.max(along - trim_end);
        }
    }
    (far, past)
}

fn assert_riding(p: &Plan, what: &str) {
    for k in 1..40 {
        let t = k as f64 / 40.0;
        let (d, past) = worst(p, t);
        assert!(d < 0.05, "{what}: a dot {d:.3} px off the line at t = {t}");
        assert!(past < 0.01, "{what}: a dot shown {past:.3} of the line ahead of its drawn end at t = {t}");
    }
    assert_eq!(p.stats().riders, 1, "{what}: the dots ride the line");
}

#[test]
fn equal_count_values_change() {
    let a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let b = chart(&rows(1..=8, |x| ((x * 5) % 7) as f64), 1.0, 8.0, Curve::MonotoneX);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    assert_riding(&p, "equal count");
    assert_eq!(p.at(1.0, &NoShaper), b);
}

#[test]
fn dots_keep_to_their_lines_timing() {
    // The document staggers dots and gives lines another easing and length: dots still follow.
    let rules = MotionRules::new(vec![
        Rule::new().select(Selector::kind("instance")).choreo(Choreography::Stagger { order: Order::Data, spread: 0.6 }).easing(Easing::parse("spring(170, 26)").unwrap()),
        Rule::new().select(Selector::kind("polyline")).easing(Easing::Linear).duration(1.5),
    ]);
    let a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let b = chart(&rows(1..=8, |x| ((x * 5) % 7) as f64), 1.0, 8.0, Curve::MonotoneX);
    assert_riding(&plan(&a, &b, &rules, &cx()), "own rules");
}

#[test]
fn points_come_and_go() {
    // 8 points → 12 (the x domain grows with them: every x moves), and back; linear and smooth.
    for curve in [Curve::Linear, Curve::MonotoneX, Curve::CatmullRom, Curve::StepAfter] {
        let a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, curve);
        let b = chart(&rows(1..=12, |x| ((x * 5) % 7) as f64), 1.0, 12.0, curve);
        let p = plan(&a, &b, &MotionRules::default(), &cx());
        assert_riding(&p, &format!("more points, {curve:?}"));
        assert_eq!(p.at(1.0, &NoShaper), b);
        assert_riding(&plan(&b, &a, &MotionRules::default(), &cx()), &format!("fewer points, {curve:?}"));
        // Every other point (the x domain unchanged).
        let odd = chart(&rows(1..=12, |x| ((x * 5) % 7) as f64).into_iter().filter(|r| r.0 % 2 == 1).collect::<Vec<_>>(), 1.0, 12.0, curve);
        assert_riding(&plan(&b, &odd, &MotionRules::default(), &cx()), &format!("every other point, {curve:?}"));
    }
}

#[test]
fn a_sliding_window_slides() {
    // The same number of points, x 1..8 → 5..12: the shared points keep their keys and slide left
    // (a vertex-by-vertex lerp would morph the line in place while the dots slide).
    let data = |x: i64| ((x * 5) % 7) as f64 + 1.0;
    let a = chart(&rows(1..=8, data), 1.0, 8.0, Curve::MonotoneX);
    let b = chart(&rows(5..=12, data), 5.0, 12.0, Curve::MonotoneX);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    assert_riding(&p, "sliding window");
    // Mid-flight, the dot for x = 8 is on the line and halfway between its two places.
    let f = p.at(0.5, &NoShaper);
    let NodeKind::Instances(ins) = &find(&f, &path(&["root", "plot", "s", "points"])).unwrap().kind else { panic!() };
    let i = ins.keys.iter().position(|k| *k == Key::one(8)).unwrap();
    assert!((ins.x[i] - (300.0 + 300.0 * 3.0 / 7.0) / 2.0).abs() < 1e-9, "x = 8 halfway from the right edge to 3/7 across: {}", ins.x[i]);
}

#[test]
fn dots_appear_as_their_line_draws_on() {
    let empty = scene(vec![group("plot", vec![])]);
    let with = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let rules = MotionRules::new(vec![]).with_defaults(vec![Rule::new().select(Selector::kind("polyline")).enter(Ghost::draw_on()).exit(Ghost::draw_on())]);
    let p = plan(&empty, &with, &rules, &cx());
    assert_riding(&p, "draw on");
    // Halfway through, the dots ahead of the drawn end are hidden and some behind it are shown.
    let f = p.at(0.5, &NoShaper);
    let NodeKind::Instances(ins) = &find(&f, &path(&["root", "plot", "s", "points"])).unwrap().kind else { panic!() };
    assert!(ins.opacity[0] > 0.99 && ins.opacity[7] == 0.0, "{:?}", ins.opacity);
    assert_eq!(p.at(1.0, &NoShaper), with);
    assert_riding(&plan(&with, &empty, &rules, &cx()), "draw off");
}

#[test]
fn a_line_running_on_reveals_its_new_dots() {
    // The same x scale, two more points: the line grows along itself (a trim); its new dots
    // appear as it reaches them.
    let a = chart(&rows(1..=6, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let b = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    assert!(find(&p.at(0.5, &NoShaper), &path(&["root", "plot", "s", "line"])).unwrap().common.trim.is_some(), "grows along itself");
    assert_riding(&p, "run on");
}

#[test]
fn a_retarget_mid_flight_keeps_them_together() {
    // Interrupted halfway (the frame's line is sampled along its curve, the dots on some of its
    // vertices), then sent elsewhere — with spring velocity carried over.
    let spring = MotionRules::new(vec![Rule::new().easing(Easing::parse("spring(170, 20)").unwrap())]);
    let a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::MonotoneX);
    let b = chart(&rows(1..=12, |x| ((x * 5) % 7) as f64), 1.0, 12.0, Curve::MonotoneX);
    let c = chart(&rows(4..=9, |x| ((x * 3) % 5) as f64 + 1.0), 4.0, 9.0, Curve::MonotoneX);
    let p = plan(&a, &b, &spring, &cx());
    let q = p.retarget(0.4, &c, &spring, &cx(), &NoShaper);
    assert!(q.stats().riders == 1);
    assert_riding(&q, "retargeted with velocity");
    assert_riding(&retarget(&p.at(0.4, &NoShaper), &a, &MotionRules::default(), &cx()), "retargeted back");
}

#[test]
fn unrelated_dots_move_on_their_own() {
    // Dots that aren't on the line's vertices (a scatter beside a trend line) keep their own rules.
    let mut a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, Curve::Linear);
    if let Some(s) = a.root.children_mut().and_then(|c| c[0].children_mut()).and_then(|c| c[0].children_mut()) {
        if let NodeKind::Instances(ins) = &mut s[1].kind {
            std::sync::Arc::make_mut(ins).y[3] += 5.0;
        }
    }
    let b = chart(&rows(1..=12, |x| ((x * 5) % 7) as f64), 1.0, 12.0, Curve::Linear);
    assert_eq!(plan(&a, &b, &MotionRules::default(), &cx()).stats().riders, 0);
}

/// The drawn line in the frame at `t`, flattened.
fn drawn(p: &Plan, t: f64) -> Vec<Vec2> {
    let f = p.at(t, &NoShaper);
    let line = find(&f, &path(&["root", "plot", "s", "line"])).expect("the line");
    geom(line).to_path().flatten(0.005)[0].0.clone()
}

#[test]
fn a_line_running_on_grows_from_its_edge() {
    // 8 points → 12 while the x domain grows (every shared point moves left): the new part keeps
    // its final shape and moves with the junction, drawn on like a pen — so the line's end stays
    // on the plot's right edge while the new data scrolls in there, never bending out of the old
    // end. Cut back (12 → 8), the part cut off slides out through the edge the same way.
    for curve in [Curve::Linear, Curve::MonotoneX] {
        let a = chart(&rows(1..=8, |x| (x % 3) as f64 + 2.0), 1.0, 8.0, curve);
        let b = chart(&rows(1..=12, |x| ((x * 5) % 7) as f64), 1.0, 12.0, curve);
        let (grow, shrink) = (plan(&a, &b, &MotionRules::default(), &cx()), plan(&b, &a, &MotionRules::default(), &cx()));
        for k in 1..10 {
            let t = k as f64 / 10.0;
            for (p, what) in [(&grow, "growing"), (&shrink, "cut back")] {
                let line = drawn(p, t);
                let end = *line.last().unwrap();
                assert!((end.x - 300.0).abs() < 0.5, "{curve:?} {what} t = {t}: the line ends on the edge, at {end:?}");
                assert!(line.iter().all(|q| q.x <= 300.5), "{curve:?} {what} t = {t}: nothing drawn past the edge");
            }
        }
    }
}

/// A chart with a callout on the line: a dot on the point at `x` and a label above it, in a group
/// keyed `note` (the same note moving between points) or keyed by its point (a new note per step).
fn chart_with_note(rows: &[(i64, f64)], x0: f64, x1: f64, x: i64, kept: bool) -> Scene {
    let sx = |v: i64| (v as f64 - x0) / (x1 - x0) * 300.0;
    let y = rows.iter().find(|r| r.0 == x).unwrap().1;
    let at = Vec2::new(sx(x), 200.0 - y * 15.0);
    let note = Node::group(key(&if kept { "note".to_string() } else { format!("note-{x}") }), vec![
        circle("dot", at.x, at.y, 3.0),
        label("text", &format!("point {x}"), Vec2::new(at.x, at.y - 12.0)),
    ])
    .semantics(Semantics::new(Role::Annotation, format!("point {x}")));
    scene(vec![group("plot", vec![series(rows, x0, x1, Curve::Linear), note])])
}

/// The dot's centre and the label's origin in the frame at `t`, for the note keyed `k` (if shown).
fn note_at(p: &Plan, t: f64, k: &str) -> Option<(Vec2, Vec2, f64)> {
    let f = p.at(t, &NoShaper);
    let dot = find(&f, &path(&["root", "plot", k, "dot"]))?;
    let text = find(&f, &path(&["root", "plot", k, "text"]))?;
    let Geom::Ellipse { cx, cy, .. } = geom(dot) else { panic!("a dot") };
    let place = |n: &Node, q: Vec2| n.common.transform.apply(q);
    let NodeKind::Text(tx) = &text.kind else { panic!("a label") };
    Some((place(dot, Vec2::new(*cx, *cy)), place(text, tx.origin), dot.common.opacity))
}

#[test]
fn a_callout_on_a_line_moves_along_it() {
    let data = |x: i64| ((x * 5) % 7) as f64 + 1.0;
    let on = |q: Vec2, line: &[Vec2]| line.windows(2).map(|w| dist_to_segment(q, w[0], w[1])).fold(f64::INFINITY, f64::min);
    // The same note, from point 3 to point 9, while the x domain grows (the line runs on): its dot
    // travels along the line as drawn, and its label keeps its place above the dot.
    let a = chart_with_note(&rows(1..=8, data), 1.0, 8.0, 3, true);
    let b = chart_with_note(&rows(1..=12, data), 1.0, 12.0, 9, true);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    let mut xs = Vec::new();
    for k in 1..20 {
        let t = k as f64 / 20.0;
        let (dot, text, _) = note_at(&p, t, "note").expect("the note");
        assert!(on(dot, &drawn(&p, t)) < 0.05, "t = {t}: the dot {dot:?} is on the line");
        assert!((text - dot - Vec2::new(0.0, -12.0)).len() < 1e-6, "t = {t}: the label rides above the dot: off by {:?}", text - dot - Vec2::new(0.0, -12.0));
        xs.push(dot.x);
    }
    assert!(xs.windows(2).all(|w| w[1] >= w[0] - 1e-9), "it travels one way: {xs:?}");
    assert_eq!(p.at(1.0, &NoShaper), b);
    // A note per step: the leaving one rides its point while it fades, the arriving one rides in
    // on its own — each on the line whenever it shows.
    let a = chart_with_note(&rows(1..=8, data), 1.0, 8.0, 3, false);
    let b = chart_with_note(&rows(1..=12, data), 1.0, 12.0, 9, false);
    let p = plan(&a, &b, &MotionRules::default(), &cx());
    for k in 1..20 {
        let t = k as f64 / 20.0;
        for n in ["note-3", "note-9"] {
            if let Some((dot, _, opacity)) = note_at(&p, t, n) {
                if opacity > 1e-3 {
                    assert!(on(dot, &drawn(&p, t)) < 0.05, "t = {t}: {n}'s dot {dot:?} is on the line");
                }
            }
        }
    }
}
