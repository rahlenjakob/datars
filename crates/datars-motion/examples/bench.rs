//! Plan and frame timings. Run with
//! `cargo run --release -p datars-motion --example bench`.
//!
//! Budgets (docs/05, P13): planning 10,000 elements < 50 ms; `at(t)` for 10,000 shape elements
//! < 10 ms; `at(t)` for 100,000 instances < 10 ms.

use datars_motion::*;
use datars_scene::*;
use datars_theme::{resolve, Ink, Mode, ThemeSet};
use std::time::Instant;

fn cx() -> PlanCx {
    let set = ThemeSet::with_builtins();
    PlanCx::new(resolve(&set.chain("datars/neutral").unwrap(), Mode::Light, &[]).0)
}

fn shapes(n: usize, shift: f64, round: bool) -> Scene {
    let kids = (0..n)
        .map(|i| {
            let (x, y) = ((i % 100) as f64 * 8.0 + shift, (i / 100) as f64 * 6.0);
            let h = 3.0 + ((i * 7919) % 13) as f64 * 0.2 + shift * 0.01;
            let g = if round { Geom::circle(x + 3.0, y + 3.0, 3.0) } else { Geom::rect(x, y, 6.0, h) };
            Node::shape(Key::one(i as i64), g).fill(Paint::Solid(Ink::palette("categorical", (i % 10) as u32))).semantics(Semantics::new(Role::Datum, format!("datum {i}")))
        })
        .collect();
    Scene::new(800.0, 600.0, Node::group(Key::name("root"), vec![Node::group(Key::name("plot"), kids)]))
}

fn dots(n: usize, shift: f64, recolor: bool) -> Scene {
    let keys: Vec<Key> = (0..n).map(|i| Key::one(i as i64)).collect();
    let x: Vec<f64> = (0..n).map(|i| ((i * 37) % 800) as f64 + shift).collect();
    let y: Vec<f64> = (0..n).map(|i| ((i * 91) % 600) as f64).collect();
    let fill: Vec<Ink> = (0..n).map(|i| Ink::palette("categorical", ((i + recolor as usize) % 10) as u32)).collect();
    let ins = Instances { proto: Proto::Symbol { symbol: SymbolKind::Circle }, keys, x, y, size: vec![2.0; n], w: None, h: None, fill, opacity: vec![1.0; n], stroke: None, screen_size: false, labels: None, line_reach: None };
    Scene::new(800.0, 600.0, Node::group(Key::name("root"), vec![Node::new(Key::name("dots"), NodeKind::Instances(std::sync::Arc::new(ins)))]))
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn time_plan(a: &Scene, b: &Scene, rules: &MotionRules, cx: &PlanCx, runs: usize) -> (f64, Plan) {
    let mut ts = Vec::new();
    let mut last = None;
    for _ in 0..runs {
        let t0 = Instant::now();
        let p = plan(a, b, rules, cx);
        ts.push(t0.elapsed().as_secs_f64() * 1e3);
        last = Some(p);
    }
    (median(ts), last.unwrap())
}

fn time_frames(p: &Plan, runs: usize) -> f64 {
    let mut ts = Vec::new();
    for i in 0..runs {
        let t = 0.05 + 0.9 * (i as f64 / runs as f64);
        let t0 = Instant::now();
        let s = p.at(t, &NoShaper);
        ts.push(t0.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(s);
    }
    median(ts)
}

fn row(name: &str, plan_ms: f64, frame_ms: f64, p: &Plan) {
    let s = p.stats();
    println!("{name:<52} plan {plan_ms:>8.2} ms   at(t) {frame_ms:>7.2} ms   (pairs {}, morphs {}, instances {})", s.pairs, s.morphs, s.instances);
}

fn main() {
    let cx = cx();
    let default = MotionRules::default();
    let stagger = MotionRules::new(vec![Rule::new().choreo(Choreography::Stagger { order: Order::Left, spread: 0.5 }).route(Route::Arc { height: 0.3 })]);

    let (a, b) = (shapes(10_000, 0.0, false), shapes(10_000, 40.0, false));
    let (pm, p) = time_plan(&a, &b, &default, &cx, 7);
    row("10,000 rects → rects (parametric)", pm, time_frames(&p, 21), &p);
    let (pm, p) = time_plan(&a, &b, &stagger, &cx, 7);
    row("10,000 rects → rects (stagger left + arc route)", pm, time_frames(&p, 21), &p);
    let c = shapes(10_000, 40.0, true);
    let (pm, p) = time_plan(&a, &c, &default, &cx, 5);
    row("10,000 rects → circles (disc morph, 32-pt outlines)", pm, time_frames(&p, 11), &p);
    let by_key = MotionRules::new(vec![Rule::new().matcher(Matcher::ByKey)]);
    let (pm, p) = time_plan(&a, &b, &by_key, &cx, 7);
    row("10,000 rects → rects (by key)", pm, time_frames(&p, 21), &p);

    let (da, db) = (dots(100_000, 0.0, false), dots(100_000, 25.0, true));
    let (pm, p) = time_plan(&da, &db, &default, &cx, 5);
    row("100,000 instances (move + recolour, together)", pm, time_frames(&p, 21), &p);
    let (pm, p) = time_plan(&da, &db, &stagger, &cx, 5);
    row("100,000 instances (stagger left + arc route)", pm, time_frames(&p, 21), &p);
    let dm = dots(100_000, 25.0, false);
    let (pm, p) = time_plan(&da, &dm, &default, &cx, 5);
    row("100,000 instances (move only)", pm, time_frames(&p, 21), &p);

    let near = MotionRules::new(vec![Rule::new().matcher(Matcher::Nearest)]);
    let (na, nb) = (shapes(2_000, 0.0, false), shapes(2_000, 3.0, false));
    let (pm, p) = time_plan(&na, &nb, &near, &cx, 3);
    row("2,000 shapes nearest (auction)", pm, time_frames(&p, 11), &p);
    let (na, nb) = (shapes(10_000, 0.0, false), shapes(10_000, 3.0, false));
    let (pm, p) = time_plan(&na, &nb, &near, &cx, 3);
    row("10,000 shapes nearest (greedy grid)", pm, time_frames(&p, 11), &p);
}
