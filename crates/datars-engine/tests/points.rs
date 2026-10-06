//! Point pyramids (`instances` with `lod`): a bounded, density-preserving sample per frame, every
//! row up close; pans and zooms that never re-index; hover on the rows drawn; a point archive that
//! draws exactly what the table draws, streamed by range; generated tables.

use datars_engine::{Engine, Pointer, Request};
use datars_expr::Value;
use datars_math::{Rect, Vec2};
use datars_scene::{Camera, Node, NodeKind, Scene};

const N: usize = 30_000;

/// Half the rows spread over a 100 × 100 square, half in a tight cluster at its centre.
fn rows() -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut rng = datars_math::Rng::new(11);
    let mut xs = Vec::with_capacity(N);
    let mut ys = Vec::with_capacity(N);
    for i in 0..N {
        if i % 2 == 0 {
            xs.push(rng.next_f64() * 100.0);
            ys.push(rng.next_f64() * 100.0);
        } else {
            xs.push(50.0 + (rng.next_f64() - 0.5) * 4.0);
            ys.push(50.0 + (rng.next_f64() - 0.5) * 4.0);
        }
    }
    let vs = (0..N).map(|i| (i % 997) as f64).collect();
    (xs, ys, vs)
}

fn doc(source: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 400 },
        "data": { "pts": source },
        "scene": { "kind": "view", "key": "sky", "camera": { "fit": { "bbox": [0, 0, 100, 100] }, "padding": 0, "explore": "cam", "max_zoom": 4096 },
            "children": [{ "kind": "instances", "key": "stars", "from": "pts", "x": "=d.x", "y": "=d.y", "r": 1.5, "screen_size": true,
                "fill": "#ffffff", "label": "=`star ${d.$row}: ${d.v}`", "semantics": { "role": "series", "label": "30,000 points" },
                "lod": { "budget": 256, "points": 4000 } }] }
    })
}

fn table_doc() -> serde_json::Value {
    let (xs, ys, vs) = rows();
    doc(serde_json::json!({ "values": { "x": xs, "y": ys, "v": vs } }))
}

fn engine(d: serde_json::Value) -> Engine {
    let mut e = Engine::new();
    let diags = e.load(datars_ir::Doc::from_json(&d.to_string()).unwrap());
    assert!(diags.is_empty(), "{diags:?}");
    e
}

/// Point the explorable camera and let the frame settle (a signal change animates).
fn look(e: &mut Engine, z: f64, x: f64, y: f64, t: f64) -> datars_engine::FrameOutput {
    e.set_clock(t);
    e.set_signal("cam.zoom", Value::Num(z));
    e.set_signal("cam.x", Value::Num(x));
    e.set_signal("cam.y", Value::Num(y));
    e.frame(t);
    // Then frames until nothing moves (levels are prepared a frame's budget at a time), as a host
    // draws them.
    let mut f = e.frame(t + 30.0);
    for k in 1..200 {
        if !f.animating {
            break;
        }
        f = e.frame(t + 30.0 + k as f64 / 60.0);
    }
    f
}

fn archive() -> Vec<u8> {
    let (xs, ys, vs) = rows();
    let t = datars_data::Table::from_columns("pts", vec![("x", datars_data::Column::Num(xs)), ("y", datars_data::Column::Num(ys)), ("v", datars_data::Column::Num(vs))]).unwrap();
    datars_engine::points::archive_of(&t, "x", "y", &datars_engine::points::Options::with_budget(256)).unwrap()
}

/// Every drawn instance: (row key, screen position).
fn drawn(scene: &Scene) -> Vec<(String, Vec2)> {
    fn walk(n: &Node, xf: datars_math::Affine, out: &mut Vec<(String, Vec2)>) {
        let xf = n.common.placed(xf);
        match &n.kind {
            NodeKind::View { viewport, camera, children, .. } => {
                let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(datars_math::Affine::IDENTITY);
                children.iter().for_each(|c| walk(c, xf.mul(cam), out));
            }
            NodeKind::Instances(i) => {
                for k in 0..i.len() {
                    out.push((i.keys[k].to_string(), xf.apply(Vec2::new(i.x[k], i.y[k]))));
                }
            }
            _ => n.children().iter().for_each(|c| walk(c, xf, out)),
        }
    }
    let mut out = Vec::new();
    walk(&scene.root, datars_math::Affine::IDENTITY, &mut out);
    out
}

fn camera(scene: &Scene) -> (Rect, Camera) {
    fn find(n: &Node) -> Option<(Rect, Camera)> {
        match &n.kind {
            NodeKind::View { viewport, camera: Some(c), .. } => Some((*viewport, *c)),
            _ => n.children().iter().find_map(find),
        }
    }
    find(&scene.root).expect("a view with a camera")
}

#[test]
fn a_frame_draws_a_bounded_sample_and_every_row_up_close() {
    let mut e = engine(table_doc());
    let f = e.frame(0.0);
    let s = e.point_stats();
    assert_eq!(s.rows, N as u64);
    let all = drawn(&f.scene);
    assert_eq!(all.len() as u64, s.drawn);
    // The whole square: a uniform sample within the frame's budget of 4,000 points (levels stop
    // before one would take it past that).
    assert!((1000..=4000).contains(&all.len()), "{} drawn", all.len());
    assert_eq!(s.indexed, 1);

    // Zoom far into the cluster: every row in view is drawn, and not many more.
    let f = look(&mut e, 400.0, 50.0, 50.0, 1.0);
    let (vp, cam) = camera(&f.scene);
    let to_content = cam.transform(vp).inverse().unwrap();
    let (a, b) = (to_content.apply(Vec2::new(0.0, 0.0)), to_content.apply(Vec2::new(400.0, 400.0)));
    let (xs, ys, _) = rows();
    let cell = 100.0 / 65536.0 / 1024.0; // well under a pyramid grid cell at this depth
    let inside: Vec<usize> = (0..N).filter(|&i| xs[i] > a.x + cell && xs[i] < b.x - cell && ys[i] > a.y + cell && ys[i] < b.y - cell).collect();
    assert!(!inside.is_empty());
    let keys: std::collections::BTreeSet<String> = drawn(&f.scene).into_iter().map(|(k, _)| k).collect();
    for i in &inside {
        assert!(keys.contains(&format!("({i},)")), "row {i} is in view but not drawn");
    }
    // Rows of the cells touching the view, within the budget however deep.
    assert!(keys.len() <= 4000, "{} drawn for {} in view", keys.len(), inside.len());
    assert_eq!(e.point_stats().indexed, 1, "zooming never re-indexes");
}

#[test]
fn pans_and_zooms_reuse_built_tiles() {
    let mut e = engine(table_doc());
    e.frame(0.0);
    e.pointer(Pointer::Wheel { x: 200.0, y: 200.0, delta: -900.0 });
    e.frame(0.1);
    let built = e.point_stats().built;
    // Drag back and forth: the same tiles come back from the cache.
    for k in 0..6 {
        let dx = if k % 2 == 0 { 60.0 } else { -60.0 };
        e.pointer(Pointer::Down { x: 200.0, y: 200.0 });
        e.pointer(Pointer::Move { x: 200.0 + dx, y: 200.0 });
        e.pointer(Pointer::Up { x: 200.0 + dx, y: 200.0 });
        e.frame(0.2 + k as f64 * 0.1);
    }
    let s = e.point_stats();
    assert_eq!(s.indexed, 1);
    assert!(s.built < built * 3, "{} tiles built after panning (first view: {built})", s.built);
}

#[test]
fn levels_arrive_faded_in_never_popped() {
    // A budget of 600 points: the views below hold more than that at first, then fewer.
    let mut d = table_doc();
    d["scene"]["children"][0]["lod"]["points"] = serde_json::json!(600);
    let mut e = engine(d);
    // Zoom in 5% at a time (and back out) over the evenly spread rows, clear of the cluster: no
    // level's opacity jumps between two steps — a level joins nearly transparent — and a deeper
    // level is never stronger than a coarser. (A view whose edge slides across a cluster hundreds
    // of times denser than the rest changes what it holds, and so its levels, faster.)
    let mut prev: std::collections::BTreeMap<i64, f64> = Default::default();
    let (mut joined, mut seen_full) = (0, false);
    let zooms: Vec<f64> = (0..60).map(|k| 1.6 * 1.05f64.powi(k)).collect();
    for (step, z) in zooms.iter().chain(zooms.iter().rev()).enumerate() {
        let f = look(&mut e, *z, 10.0, 90.0, step as f64 * 100.0);
        let mut now: std::collections::BTreeMap<i64, f64> = Default::default();
        f.scene.root.walk(&Default::default(), &mut |_, n| {
            if let (NodeKind::Instances(_), Some(datars_scene::KeyPart::Int(level))) = (&n.kind, n.key.parts().first()) {
                now.insert(*level, n.common.opacity);
            }
        });
        let fades: Vec<f64> = now.values().copied().collect();
        assert!(fades.windows(2).all(|w| w[0] >= w[1]), "zoom {z}: {now:?}");
        if step > 0 {
            // (A coarse level can have no rows in a small view: only levels drawn in both count.)
            for (l, b) in &now {
                match prev.get(l) {
                    Some(a) => assert!((a - b).abs() <= 0.35, "zoom {z}: level {l} jumped {a} → {b}"),
                    None if prev.keys().all(|p| p < l) => {
                        assert!(*b <= 0.35, "zoom {z}: level {l} popped in at {b}");
                        joined += 1;
                    }
                    None => {}
                }
            }
        }
        seen_full |= now.len() > 2 && now.values().all(|&f| f > 0.95);
        prev = now;
    }
    assert!(joined >= 2 && seen_full, "levels joined: {joined}");
}

#[test]
fn hovering_finds_the_row_under_the_pointer() {
    let mut e = engine(table_doc());
    let f = look(&mut e, 10.0, 20.0, 20.0, 0.0);
    let (key, at) = drawn(&f.scene).into_iter().find(|(_, p)| p.x > 100.0 && p.x < 300.0 && p.y > 100.0 && p.y < 300.0).expect("a row in view");
    let row: usize = key.trim_matches(|c| c == '(' || c == ')' || c == ',').parse().unwrap();
    let label = e.pointer(Pointer::Move { x: at.x + 0.5, y: at.y }).expect("a label");
    assert_eq!(label, format!("star {row}: {}", row % 997));
    // Between rows, more than a few px from any: nothing.
    let f = look(&mut e, 400.0, 20.0, 20.0, 100.0);
    let near: Vec<Vec2> = drawn(&f.scene).into_iter().map(|(_, p)| p).collect();
    let empty = (0..40).flat_map(|i| (0..40).map(move |j| Vec2::new(5.0 + i as f64 * 10.0, 5.0 + j as f64 * 10.0))).find(|q| near.iter().all(|p| ((p.x - q.x).powi(2) + (p.y - q.y).powi(2)).sqrt() > 12.0)).expect("an empty spot");
    assert_eq!(e.pointer(Pointer::Move { x: empty.x, y: empty.y }), None);
}

#[test]
fn an_archive_draws_exactly_what_the_table_draws() {
    let mut a = engine(table_doc());
    let mut b = engine(doc(serde_json::json!({ "tiles": "pts.pmtiles" })));
    b.provide("pts", &archive()).unwrap();
    for (k, (z, x, y)) in [(1.0, 50.0, 50.0), (3.3, 30.0, 70.0), (60.0, 50.2, 49.9), (30.0, 12.0, 80.0), (900.0, 50.05, 50.02)].into_iter().enumerate() {
        let (fa, fb) = (look(&mut a, z, x, y, k as f64 * 100.0), look(&mut b, z, x, y, k as f64 * 100.0));
        assert!(!drawn(&fa.scene).is_empty());
        assert_eq!(fa.scene.hash(), fb.scene.hash(), "zoom {z}: the archive draws what the table draws");
        assert_eq!(fa.hash(), fb.hash());
        assert_eq!(fb.pending_tiles, 0);
    }
    assert!(b.diagnostics().is_empty(), "{:?}", b.diagnostics());
}

#[test]
fn archives_stream_by_range_and_draw_coarse_levels_meanwhile() {
    let bytes = archive();
    let mut whole = engine(doc(serde_json::json!({ "tiles": "pts.pmtiles" })));
    whole.provide("pts", &bytes).unwrap();
    let mut e = engine(doc(serde_json::json!({ "tiles": "pts.pmtiles" })));
    let want = look(&mut whole, 8.0, 50.0, 50.0, 0.0).hash();
    e.set_signal("cam.zoom", Value::Num(8.0));
    e.set_signal("cam.x", Value::Num(50.0));
    e.set_signal("cam.y", Value::Num(50.0));
    let mut served = 0usize;
    let mut first_partial = None;
    for round in 0..16 {
        let mut f = e.frame(30.0 + round as f64);
        if f.pending_tiles == 0 && round > 0 {
            // New detail eases in: once settled, the frame is the whole archive's.
            for k in 1..100 {
                if !f.animating {
                    break;
                }
                f = e.frame(30.0 + round as f64 + k as f64 / 60.0);
            }
            assert_eq!(f.hash(), want, "all tiles in: the same frame as the whole archive");
            assert!(first_partial.is_some_and(|n: usize| n > 0), "coarse levels drew while finer ones loaded");
            assert!(served < bytes.len(), "only the ranges in view were read ({served} of {} bytes)", bytes.len());
            return;
        }
        if round > 1 && first_partial.is_none() {
            first_partial = Some(drawn(&f.scene).len());
        }
        for r in e.requests() {
            if let Request::Range { name, offset, length, .. } = r {
                let end = ((offset + length) as usize).min(bytes.len());
                served += end - offset as usize;
                e.provide_range(&name, offset, &bytes[offset as usize..end]).unwrap();
            }
        }
    }
    panic!("tiles never finished arriving");
}

#[test]
fn generated_tables_are_expressions_over_row_numbers() {
    let d = serde_json::json!({
        "datars": 1, "size": { "width": 100, "height": 100 },
        "data": { "g": { "generate": { "rows": 70_000, "columns": [
            { "as": "u", "expr": "rand(d.i, 1)" },
            { "as": "x", "expr": "d.i * 2 + d.u" },
            { "as": "kind", "expr": "d.u < 0.5 ? \"low\" : \"high\"" },
        ], "keep": ["x", "kind"] } } },
        "scene": { "kind": "instances", "key": "dots", "from": "g", "x": "=d.x", "y": "=d.kind == \"low\" ? 1 : 2", "r": 1 }
    });
    let mut e = engine(d.clone());
    let s = e.scene();
    let NodeKind::Instances(i) = &s.root.kind else { panic!("{:?}", s.root.kind_name()) };
    assert_eq!(i.len(), 70_000);
    // Row 65,537 (the second chunk) sees its own row number, and the same random draw every time.
    let k = 65_537;
    assert!((i.x[k] - (k as f64 * 2.0)).abs() < 1.0);
    let again = engine(d).scene();
    assert_eq!(s.hash(), again.hash());
    assert!(i.y.contains(&1.0) && i.y.contains(&2.0));

    // Mistakes are diagnostics: the row index of a lambda, a column that isn't generated.
    for (cols, keep, want) in [
        (serde_json::json!([{ "as": "x", "expr": "(d, i) => i" }]), serde_json::json!([]), "d.i"),
        (serde_json::json!([{ "as": "x", "expr": "d.i" }]), serde_json::json!(["y"]), "keeps `y`"),
    ] {
        let bad = serde_json::json!({ "datars": 1, "data": { "g": { "generate": { "rows": 10, "columns": cols, "keep": keep } } }, "scene": { "kind": "group" } });
        let mut e = Engine::new();
        let diags = e.load(datars_ir::Doc::from_json(&bad.to_string()).unwrap());
        assert!(diags.iter().any(|d| d.message.contains(want)), "{diags:?}");
    }
}

#[test]
fn lod_positions_must_depend_on_the_row_only() {
    let mut d = table_doc();
    d["scene"]["children"][0]["x"] = serde_json::json!("=d.x * cam.zoom");
    let mut e = Engine::new();
    e.load(datars_ir::Doc::from_json(&d.to_string()).unwrap());
    e.frame(0.0);
    assert!(e.diagnostics().iter().any(|d| d.message.contains("indexed once")), "{:?}", e.diagnostics());
}

#[test]
fn the_accessible_description_summarizes_instead_of_listing_every_row() {
    let mut e = engine(table_doc());
    e.frame(0.0);
    let items = e.semantic_items();
    assert!(items.iter().any(|s| s.label == "30,000 points"));
    assert!(items.len() <= 201, "{} items", items.len());
}



#[test]
fn nodes_over_the_same_rows_share_one_pyramid() {
    // Dots and a glow under them (another budget of points, other columns read): one index.
    let mut d = table_doc();
    let mut glow = d["scene"]["children"][0].clone();
    glow["key"] = serde_json::json!("glow");
    glow["r"] = serde_json::json!("=6 + d.v / 997");
    glow["lod"]["points"] = serde_json::json!(300);
    d["scene"]["children"].as_array_mut().unwrap().insert(0, glow);
    let mut e = engine(d.clone());
    e.frame(0.0);
    look(&mut e, 5.0, 40.0, 60.0, 1.0);
    assert_eq!(e.point_stats().indexed, 1);
    // Published: one archive serves both.
    let archives = e.point_archives();
    assert_eq!(archives.len(), 1);
    let bytes = archives[0].1.clone().unwrap();
    let mut a = engine(d.clone());
    d["data"]["pts"] = serde_json::json!({ "tiles": "pts.pmtiles" });
    let mut b = engine(d);
    b.provide("pts", &bytes).unwrap();
    assert_eq!(look(&mut a, 5.0, 40.0, 60.0, 1.0).hash(), look(&mut b, 5.0, 40.0, 60.0, 1.0).hash());
}

#[test]
fn a_point_scale_thins_the_sample() {
    let mut e = engine(table_doc());
    let full = drawn(&e.frame(0.0).scene).len();
    e.set_point_scale(0.1);
    let thin = drawn(&e.frame(1.0).scene).len();
    assert!(thin * 3 < full && thin > 0, "{thin} vs {full}");
    e.set_point_scale(1.0);
    assert_eq!(drawn(&e.frame(2.0).scene).len(), full);
}

/// A zoom step into new detail spreads its preparation over frames: finer levels wait (the
/// coarser ones keep showing) while their tiles are sorted and run through the template a budget
/// of rows per frame, frames keep coming until they're in, and the last one is the settled view.
#[test]
fn a_zoom_step_spreads_its_work_over_frames() {
    let d = serde_json::json!({
        "datars": 1, "size": { "width": 400, "height": 400 },
        "data": { "pts": { "generate": { "rows": 400_000, "columns": [
            { "as": "x", "expr": "rand(d.i, 1) * 100" }, { "as": "y", "expr": "rand(d.i, 2) * 100" } ] } } },
        "scene": { "kind": "view", "key": "sky", "camera": { "fit": { "bbox": [0, 0, 100, 100] }, "padding": 0, "explore": "cam", "max_zoom": 4096 },
            "children": [{ "kind": "instances", "key": "stars", "from": "pts", "x": "=d.x", "y": "=d.y", "r": 1, "screen_size": true, "fill": "#ffffff",
                "lod": { "budget": 4096, "points": 150_000 } }] }
    });
    let mut e = engine(d);
    e.frame(0.0);
    e.pointer(Pointer::Wheel { x: 200.0, y: 200.0, delta: -300.0 });
    let mut frames = Vec::new();
    for k in 1..200 {
        let f = e.frame(k as f64 / 60.0);
        let n = drawn(&f.scene).len();
        let moving = f.animating;
        frames.push(n);
        if !moving {
            break;
        }
    }
    assert!(frames.len() > 2, "the step was spread over frames: {frames:?}");
    assert!(frames.windows(2).all(|w| w[1] >= w[0]), "points only arrive, never vanish: {frames:?}");
    let settled = drawn(&e.scene());
    let last = drawn(&e.frame(10.0).scene);
    assert_eq!(last.len(), settled.len(), "{frames:?}");
}
