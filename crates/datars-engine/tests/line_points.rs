//! A line chart's dots (`std/line` with `points: true`) stay on the line through every
//! transition: values changing, points coming and going with the x domain, a sliding window,
//! every other point, and the line drawing on.

use datars_engine::Engine;
use datars_math::Vec2;
use datars_scene::{Geom, Node, NodeKind, Scene};

fn engine() -> Engine {
    let j = serde_json::json!({
        "datars": 1, "size": { "width": 480, "height": 300 },
        "signals": {
            "show": { "type": "bool", "default": true }, "mode": { "type": "num", "default": 0 },
            "lo": { "type": "num", "default": 1 }, "hi": { "type": "num", "default": 8 }, "odd": { "type": "bool", "default": false }
        },
        "data": { "raw": { "values": {
            "x": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            "a": [3, 5, 4, 8, 6, 9, 7, 10, 8, 12, 9, 11],
            "b": [9, 4, 10, 3, 8, 2, 7, 5, 11, 4, 9, 6]
        }, "key": ["x"] } },
        "tables": { "t": { "from": "raw", "ops": [
            { "op": "derive", "as": "y", "expr": { "expr": "mode == 0 ? d.a : d.b" } },
            { "op": "filter", "expr": { "expr": "d.x >= lo && d.x <= hi && (!odd || d.x % 2 == 1)" } }
        ] } },
        "scene": { "kind": "use", "recipe": "@datars/std/plot", "params": { "data": "t", "x": "x", "y": "y", "xType": "linear", "zero": false, "yDomain": [0, 13],
            "children": [{ "kind": "use", "recipe": "@datars/std/line", "params": { "points": true }, "when": "=show" }] } },
        "program": { "states": [
            { "name": "none", "set": { "show": false } },
            { "name": "a", "set": { "show": true, "mode": 0, "lo": 1, "hi": 8, "odd": false } },
            { "name": "b", "set": { "mode": 1 } },
            { "name": "wider", "set": { "hi": 12 } },
            { "name": "window", "set": { "lo": 5 } },
            { "name": "odd", "set": { "odd": true } }
        ] }
    });
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&j.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

fn dist_to_segment(q: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l2 = d.x * d.x + d.y * d.y;
    let t = if l2 > 0.0 { (((q - a).x * d.x + (q - a).y * d.y) / l2).clamp(0.0, 1.0) } else { 0.0 };
    q.dist(a.lerp(b, t))
}

/// Over every group holding a polyline and instances: the worst distance (px) from a visible dot
/// to the line, and how far past the line's drawn end (fraction of its length) a visible dot is.
fn worst(n: &Node, out: &mut (f64, f64, usize)) {
    let kids = n.children();
    let line = kids.iter().find(|c| matches!(&c.kind, NodeKind::Shape { geom: Geom::Polyline { .. }, .. }));
    if let Some(line) = line {
        let NodeKind::Shape { geom, .. } = &line.kind else { unreachable!() };
        let flat = geom.to_path().flatten(0.005);
        let pts: Vec<Vec2> = flat[0].0.iter().map(|p| line.common.transform.apply(*p)).collect();
        let mut cum = vec![0.0];
        for w in pts.windows(2) {
            cum.push(cum.last().unwrap() + w[0].dist(w[1]));
        }
        let total = cum.last().copied().unwrap_or(0.0).max(1e-9);
        let trim_end = line.common.trim.map_or(1.0, |t| t[1]);
        for dots in kids.iter() {
            let NodeKind::Instances(ins) = &dots.kind else { continue };
            out.2 += 1;
            for i in 0..ins.len() {
                if ins.opacity[i] * dots.common.opacity <= 1e-6 {
                    continue;
                }
                let q = dots.common.transform.apply(Vec2::new(ins.x[i], ins.y[i]));
                let (k, d) = pts.windows(2).enumerate().map(|(k, w)| (k, dist_to_segment(q, w[0], w[1]))).fold((0, f64::INFINITY), |a, b| if b.1 < a.1 { b } else { a });
                out.0 = out.0.max(d);
                out.1 = out.1.max((cum[k] + pts[k].dist(q).min(cum[k + 1] - cum[k])) / total - trim_end);
            }
        }
    }
    for c in kids {
        worst(c, out);
    }
}

fn assert_on_line(e: &mut Engine, from: usize, to: usize, what: &str) {
    let (_, _, plan) = e.plan_states(from, to);
    assert!(plan.stats().riders >= 1, "{what}: the dots ride the line ({:?})", plan.stats());
    for k in 1..32 {
        let t = k as f64 / 32.0;
        let f: Scene = e.planned_at(&plan, t);
        let mut w = (0.0, 0.0, 0);
        worst(&f.root, &mut w);
        assert!(w.2 >= 1, "{what}: dots beside a line at t = {t}");
        assert!(w.0 < 0.05, "{what}: a dot {:.3} px off the line at t = {t}", w.0);
        assert!(w.1 < 0.01, "{what}: a dot {:.3} of the line ahead of its drawn end at t = {t}", w.1);
    }
}

#[test]
fn dots_stay_on_their_line() {
    let mut e = engine();
    assert_on_line(&mut e, 0, 1, "drawing on");
    assert_on_line(&mut e, 1, 2, "values change");
    assert_on_line(&mut e, 2, 3, "more points, x domain grows");
    assert_on_line(&mut e, 3, 4, "a window: fewer points, x moves");
    assert_on_line(&mut e, 4, 5, "every other point");
    assert_on_line(&mut e, 5, 2, "back");
}
