//! `datars-test` — visual and motion tests that coding agents can trust (docs/13-testing.md).
//!
//! For every example document: each state's scene snapshot + scene hash + CPU pixel hash; each
//! transition sampled densely (default 64 samples) and hashed; motion invariants checked on every
//! sample. Goldens are hashes and text (kilobytes, in git); images are produced only for failures
//! and review (filmstrips, motion trails). No browser, no GPU, no network. Exact comparisons: the
//! engine is deterministic, so any difference is a real change.

#[cfg(feature = "gpu")]
pub mod gpu;

use datars_engine::Engine;
use datars_math::Vec2;
use datars_scene::{Node, NodeKind, Scene};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Golden {
    pub states: Vec<StateGolden>,
    pub transitions: Vec<TransitionGolden>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct StateGolden {
    pub name: String,
    pub scene: String,
    pub pixels: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TransitionGolden {
    pub from: String,
    pub to: String,
    /// One hash over all sampled frames (the per-sample list lives in the report on mismatch).
    pub sweep: String,
    pub samples: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Failure {
    pub example: String,
    pub what: String,
    pub detail: String,
    pub artifacts: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub examples: usize,
    pub states: usize,
    pub transitions: usize,
    pub frames_checked: usize,
    pub failures: Vec<Failure>,
    pub updated: Vec<String>,
    pub millis: u128,
}

pub struct Options {
    pub root: PathBuf,
    pub filter: String,
    pub update: bool,
    pub samples: usize,
    pub pixels: bool,
}

fn hex(v: u64) -> String {
    format!("{v:016x}")
}

fn hash_all(hs: &[u64]) -> String {
    let mut h = datars_math::Hash64::new();
    for v in hs {
        h.u64(*v);
    }
    hex(h.finish())
}

/// One drawable element of a frame: accumulated opacity, centre on screen, and centre relative to
/// the cameras above it (so a camera flight isn't mistaken for elements jumping).
#[derive(Clone, Copy, Debug)]
struct El {
    op: f64,
    at: Vec2,
    content: Vec2,
    /// Under a `decoration` subtree (a basemap, gridlines): scenery that may come and go as a
    /// camera passes (map labels fading in and out), not data that must not flash.
    decor: bool,
}

/// Two drawables under one key path are a crossfade pair (motion emits the outgoing and incoming
/// copies together): their combined presence is the over-composite of their opacities, placed at
/// the more opaque copy.
fn add(out: &mut BTreeMap<String, El>, pairs: &mut std::collections::BTreeSet<String>, k: String, v: El) {
    match out.get_mut(&k) {
        Some(prev) => {
            let top = if v.op > prev.op { v } else { *prev };
            *prev = El { op: 1.0 - (1.0 - prev.op) * (1.0 - v.op), ..top };
            pairs.insert(k);
        }
        None => {
            out.insert(k, v);
        }
    }
}

/// Accumulated opacity and centre of every drawable element, by key path.
pub fn elements(scene: &Scene) -> BTreeMap<String, (f64, Vec2)> {
    elements_and_pairs(scene).0.into_iter().map(|(k, e)| (k, (e.op, e.at))).collect()
}

/// Every element, and the key paths drawn as crossfade pairs (two copies) in this frame.
fn elements_and_pairs(scene: &Scene) -> (BTreeMap<String, El>, std::collections::BTreeSet<String>) {
    use datars_math::Affine;
    let mut out = BTreeMap::new();
    let mut pairs = std::collections::BTreeSet::new();
    // `xf` places on screen; `cf` is the same without the cameras of enclosing views.
    #[allow(clippy::too_many_arguments)]
    fn go(n: &Node, path: &str, xf: Affine, cf: Affine, op: f64, decor: bool, out: &mut BTreeMap<String, El>, pairs: &mut std::collections::BTreeSet<String>) {
        let xf = n.common.placed(xf);
        let cf = n.common.placed(cf);
        let op = op * n.common.opacity * if n.common.visible { 1.0 } else { 0.0 };
        let decor = decor || n.semantics.as_ref().is_some_and(|s| s.role == datars_scene::Role::Decoration);
        let here = format!("{path}/{}", n.key);
        let el = |op: f64, p: Vec2| El { op, at: xf.apply(p), content: cf.apply(p), decor };
        match &n.kind {
            NodeKind::Shape { geom, .. } => {
                add(out, pairs, here, el(op, geom.center()));
            }
            NodeKind::Text(t) => {
                add(out, pairs, here, el(op, t.origin));
            }
            NodeKind::Instances(i) => {
                for k in 0..i.len().min(20_000) {
                    add(out, pairs, format!("{here}/{}", i.keys[k]), el(op * i.opacity_at(k), Vec2::new(i.x[k], i.y[k])));
                }
            }
            NodeKind::View { viewport, camera, children, .. } => {
                let origin = Affine::translate(viewport.x, viewport.y);
                let cam = camera.map(|c| c.transform(*viewport)).unwrap_or(origin);
                for c in children {
                    go(c, &here, xf.mul(cam), cf.mul(origin), op, decor, out, pairs);
                }
            }
            _ => {
                for c in n.children() {
                    go(c, &here, xf, cf, op, decor, out, pairs);
                }
            }
        }
    }
    go(&scene.root, "", Affine::IDENTITY, Affine::IDENTITY, 1.0, false, &mut out, &mut pairs);
    (out, pairs)
}

fn has_nan(scene: &Scene) -> bool {
    // NaN serializes as null in numeric positions; any null where a number belongs is suspect.
    let j = scene.to_json();
    j.contains(":null,") && (j.contains("\"x\":null") || j.contains("\"y\":null") || j.contains("\"w\":null") || j.contains("\"h\":null") || j.contains("\"cx\":null") || j.contains("\"opacity\":null"))
}

/// Motion invariants over a sequence of sampled frames. Returns human-readable violations.
pub fn check_motion(frames: &[Scene], diag: f64) -> Vec<String> {
    let mut v = Vec::new();
    let (els, pairs): (Vec<BTreeMap<String, El>>, Vec<std::collections::BTreeSet<String>>) = frames.iter().map(elements_and_pairs).unzip();
    let n = frames.len();
    if n < 3 {
        return v;
    }
    // Only what can be seen can pop: an element outside the view (a region far off-screen while
    // the camera flies) moves fast without anyone noticing.
    let (w, h) = (frames[0].width, frames[0].height);
    let margin = diag * 0.05;
    let visible = |p: Vec2| p.x >= -margin && p.x <= w + margin && p.y >= -margin && p.y <= h + margin;
    for (i, f) in frames.iter().enumerate() {
        if has_nan(f) {
            v.push(format!("NaN in frame {i}/{}", n - 1));
            break;
        }
    }
    let first = &els[0];
    let last = &els[n - 1];
    let mut keys: Vec<&String> = els.iter().flat_map(|m| m.keys()).collect();
    keys.sort();
    keys.dedup();
    let step_bound = diag * 0.35; // a jump larger than a third of the view between samples is a pop
    // Under a zooming camera a big element's centre (the sea behind a map) can sit thousands of px
    // off screen and move fast while what shows of it doesn't jump: centres more than a diagonal
    // outside the view on both samples aren't pops.
    let far =|p: Vec2| p.x < -diag || p.y < -diag || p.x > w + diag || p.y > h + diag;
    for k in keys {
        let series: Vec<Option<El>> = els.iter().map(|m| m.get(k).copied()).collect();
        let entering = !first.contains_key(k) && last.contains_key(k);
        let exiting = first.contains_key(k) && !last.contains_key(k);
        let ops: Vec<f64> = series.iter().map(|s| s.map(|x| x.op).unwrap_or(0.0)).collect();
        // Scenery (a basemap's labels) may come and go as the camera passes; data must not.
        let decor = series.iter().flatten().all(|e| e.decor);
        if !decor && entering && ops.windows(2).any(|w| w[1] + 1e-6 < w[0] && w[0] > 0.05) {
            v.push(format!("flash: entering {k} fades back out mid-transition"));
        }
        if !decor && exiting && ops.windows(2).any(|w| w[1] > w[0] + 1e-6 && w[1] > 0.05) {
            v.push(format!("flash: exiting {k} comes back mid-transition"));
        }
        // A crossfade between two versions of an element (a title whose text changes) dims on
        // purpose: the old text is out before the new one comes in.
        let swapped = pairs.iter().any(|p| p.contains(k));
        if !decor && !entering && !exiting && !swapped && first.contains_key(k) {
            let lo = ops[0].min(ops[n - 1]);
            if lo > 0.2 && ops.iter().any(|o| *o < lo * 0.25) {
                v.push(format!("blink: {k} dips to {:.2} opacity mid-transition", ops.iter().copied().fold(1.0, f64::min)));
            }
        }
        for (i, s) in series.windows(2).enumerate() {
            // A crossfade pair's presence moves from the outgoing copy to the incoming one: that's
            // a fade, not a jump.
            let crossfade = pairs[i].contains(k) || pairs[i + 1].contains(k);
            // Distances are measured under the cameras: a camera flight moves everything at once
            // (smoothly, by its own interpolator), which is not an element popping.
            if let (Some(a), Some(b)) = (s[0], s[1]) {
                let d = a.content.dist(b.content);
                if d > step_bound && a.op > 0.05 && b.op > 0.05 && !crossfade && (visible(a.at) || visible(b.at)) && !(far(a.at) && far(b.at)) {
                    v.push(format!("pop: {k} jumps {d:.0} px between samples"));
                    break;
                }
            }
        }
        if v.len() > 50 {
            v.push("… more".into());
            break;
        }
    }
    v
}

fn examples(root: &Path, filter: &str) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root.join("examples")) {
        for e in rd.flatten() {
            let p = e.path().join("doc.json");
            let name = e.file_name().to_string_lossy().into_owned();
            if p.exists() && name.contains(filter) {
                out.push((name, p));
            }
        }
    }
    out.sort();
    out
}

/// Run the suite.
pub fn run(opts: &Options) -> Report {
    let t0 = std::time::Instant::now();
    let mut report = Report::default();
    let out_dir = opts.root.join("out/test");
    let _ = std::fs::create_dir_all(&out_dir);
    let list = examples(&opts.root, &opts.filter);
    // One thread per example (each builds its own engine; results are deterministic regardless).
    let results: Vec<(String, Golden, Vec<Failure>, usize, BTreeMap<String, String>)> = std::thread::scope(|s| {
        let handles: Vec<_> = list
            .iter()
            .map(|(name, path)| {
                let name = name.clone();
                let path = path.clone();
                let out_dir = out_dir.clone();
                let samples = opts.samples;
                let pixels = opts.pixels;
                s.spawn(move || run_example(&name, &path, &out_dir, samples, pixels))
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("example thread")).collect()
    });
    for (name, golden, mut failures, frames, snaps) in results {
        report.examples += 1;
        report.states += golden.states.len();
        report.transitions += golden.transitions.len();
        report.frames_checked += frames;
        let gdir = opts.root.join("tests/golden").join(&name);
        let gfile = gdir.join("golden.json");
        let old: Option<Golden> = std::fs::read_to_string(&gfile).ok().and_then(|s| serde_json::from_str(&s).ok());
        if opts.update || old.is_none() {
            let _ = std::fs::create_dir_all(&gdir);
            let _ = std::fs::write(&gfile, serde_json::to_string_pretty(&golden).unwrap_or_default());
            for (state, snap) in &snaps {
                let _ = std::fs::write(gdir.join(format!("{state}.txt")), snap);
            }
            report.updated.push(name.clone());
        } else if let Some(old) = old {
            for (i, st) in golden.states.iter().enumerate() {
                let prev = old.states.iter().find(|s| s.name == st.name);
                match prev {
                    None => failures.push(Failure { example: name.clone(), what: format!("state {} is new", st.name), detail: "run with --update to accept".into(), artifacts: vec![] }),
                    Some(p) if p.scene != st.scene => {
                        let old_snap = std::fs::read_to_string(gdir.join(format!("{}.txt", st.name))).unwrap_or_default();
                        let cur = snaps.get(&st.name).cloned().unwrap_or_default();
                        let cur_path = out_dir.join(format!("{name}-{}.txt", st.name));
                        let _ = std::fs::write(&cur_path, &cur);
                        let diff = text_diff(&old_snap, &cur);
                        let detail = if diff.trim().is_empty() {
                            // The snapshot is a readable summary; the hash covers everything.
                            "the snapshot text is identical: what changed is below its precision (glyph ids, e.g. after a font change; paint or stroke details; float bits). Compare the renders; accept with --update".to_string()
                        } else {
                            diff
                        };
                        failures.push(Failure { example: name.clone(), what: format!("state {} ({i}) scene changed", st.name), detail, artifacts: vec![cur_path.display().to_string()] });
                    }
                    Some(p) if opts.pixels && !p.pixels.is_empty() && p.pixels != st.pixels => {
                        failures.push(Failure { example: name.clone(), what: format!("state {} pixels changed (scene identical)", st.name), detail: "the rasterizer changed".into(), artifacts: vec![] });
                    }
                    _ => {}
                }
            }
            for tr in &golden.transitions {
                if let Some(p) = old.transitions.iter().find(|t| t.from == tr.from && t.to == tr.to) {
                    if p.sweep != tr.sweep {
                        failures.push(Failure { example: name.clone(), what: format!("transition {} → {} changed", tr.from, tr.to), detail: format!("{} sampled frames differ from the golden sweep", tr.samples), artifacts: vec![out_dir.join(format!("{name}-{}-{}-strip.png", tr.from, tr.to)).display().to_string()] });
                    }
                }
            }
        }
        report.failures.extend(failures);
    }
    report.millis = t0.elapsed().as_millis();
    let _ = std::fs::write(out_dir.join("report.json"), serde_json::to_string_pretty(&report).unwrap_or_default());
    report
}

fn run_example(name: &str, path: &Path, out_dir: &Path, samples: usize, pixels: bool) -> (String, Golden, Vec<Failure>, usize, BTreeMap<String, String>) {
    let mut failures = Vec::new();
    let mut golden = Golden::default();
    let mut snaps = BTreeMap::new();
    let mut frames_checked = 0;
    let json = match std::fs::read_to_string(path) {
        Ok(j) => j,
        Err(e) => {
            failures.push(Failure { example: name.into(), what: "read".into(), detail: e.to_string(), artifacts: vec![] });
            return (name.into(), golden, failures, 0, snaps);
        }
    };
    let mut engine: Engine = match datars_headless::load_at(&json, path.parent()) {
        Ok(e) => e,
        Err(e) => {
            failures.push(Failure { example: name.into(), what: "load".into(), detail: e, artifacts: vec![] });
            return (name.into(), golden, failures, 0, snaps);
        }
    };
    let names = engine.state_names();
    for (i, st) in names.iter().enumerate() {
        let scene = engine.scene_for_state(i);
        let px = if pixels { hex(datars_headless::render_scene(&engine, &scene, 1.0).hash()) } else { String::new() };
        // Labels a reader can't read are failures, like a wrong golden: cut off at the canvas edge,
        // or drawn over one another.
        let labels = datars_devtools::label_issues(&scene);
        if !labels.cut.is_empty() || !labels.overlapping.is_empty() {
            let mut detail: Vec<String> = labels.cut.iter().map(|t| format!("cut off at the edge: \"{t}\"")).collect();
            detail.extend(labels.overlapping.iter().map(|(a, b)| format!("overlapping: \"{a}\" / \"{b}\"")));
            failures.push(Failure { example: name.into(), what: format!("state {st}: unreadable labels"), detail: detail.join("\n"), artifacts: vec![] });
        }
        snaps.insert(st.clone(), scene.snapshot());
        golden.states.push(StateGolden { name: st.clone(), scene: scene.hash_hex(), pixels: px });
    }
    for d in engine.diagnostics() {
        failures.push(Failure { example: name.into(), what: "diagnostic".into(), detail: d.message.clone(), artifacts: vec![] });
    }
    let diag = (engine.viewport().width.powi(2) + engine.viewport().height.powi(2)).sqrt();
    for i in 0..names.len().saturating_sub(1) {
        let (sa, sb, plan) = engine.plan_states(i, i + 1);
        let frames: Vec<Scene> = (0..samples).map(|k| engine.plan_at(&plan, k as f64 / (samples - 1) as f64)).collect();
        frames_checked += frames.len();
        let hashes: Vec<u64> = frames.iter().map(|f| f.hash()).collect();
        // Motion invariants hold for the planned motion; tile content (filled per frame from the
        // camera) is in the hashes above but not a motion to check.
        let planned: Vec<Scene> = (0..samples).map(|k| engine.planned_at(&plan, k as f64 / (samples - 1) as f64)).collect();
        if frames[0].hash() != sa.hash() || frames[samples - 1].hash() != sb.hash() {
            failures.push(Failure { example: name.into(), what: format!("transition {} → {}: endpoints are not exact", names[i], names[i + 1]), detail: "plan.at(0) must equal the from-scene and plan.at(1) the to-scene".into(), artifacts: vec![] });
        }
        let violations = check_motion(&planned, diag);
        if !violations.is_empty() {
            let strip = datars_headless::filmstrip(&mut engine, i, i + 1, 8, 0.5);
            let tr = datars_headless::trails(&mut engine, i, i + 1, 24, 1.0);
            let sp = out_dir.join(format!("{name}-{}-{}-strip.png", names[i], names[i + 1]));
            let tp = out_dir.join(format!("{name}-{}-{}-trails.png", names[i], names[i + 1]));
            let _ = std::fs::write(&sp, strip.to_png());
            let _ = std::fs::write(&tp, tr.to_png());
            let artifacts = vec![sp.display().to_string(), tp.display().to_string()];
            failures.push(Failure { example: name.into(), what: format!("motion {} → {}", names[i], names[i + 1]), detail: violations.join("\n"), artifacts });
        }
        golden.transitions.push(TransitionGolden { from: names[i].clone(), to: names[i + 1].clone(), sweep: hash_all(&hashes), samples });
    }
    (name.into(), golden, failures, frames_checked, snaps)
}

/// A minimal line diff (changed lines with +/-), capped for readability.
pub fn text_diff(old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let mut out = Vec::new();
    let n = a.len().max(b.len());
    for i in 0..n {
        match (a.get(i), b.get(i)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => {
                out.push(format!("- {x}"));
                out.push(format!("+ {y}"));
            }
            (Some(x), None) => out.push(format!("- {x}")),
            (None, Some(y)) => out.push(format!("+ {y}")),
            _ => {}
        }
        if out.len() > 40 {
            out.push("…".into());
            break;
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use datars_scene::{Geom, Key};

    fn frame(copies: &[f64]) -> Scene {
        let kids = copies
            .iter()
            .map(|op| {
                let mut n = Node::shape(Key::name("title"), Geom::rect(10.0, 10.0, 50.0, 20.0));
                n.common.opacity = *op;
                n
            })
            .collect();
        Scene::new(200.0, 100.0, Node::group(Key::name("root"), kids))
    }

    #[test]
    fn a_crossfade_pair_is_not_a_blink() {
        // Outgoing copy fades out while the incoming one fades in, under one key path.
        let frames: Vec<Scene> = [0.0, 0.25, 0.5, 0.75, 1.0].iter().map(|k| frame(&[1.0 - k, *k])).collect();
        assert_eq!(check_motion(&frames, 220.0), Vec::<String>::new());
    }

    fn at(x: f64, op: f64) -> Node {
        let mut n = Node::shape(Key::name("dot"), Geom::rect(x, 40.0, 10.0, 10.0));
        n.common.opacity = op;
        n
    }

    #[test]
    fn a_jump_in_view_is_a_pop_but_not_off_screen() {
        let scene = |x: f64| Scene::new(200.0, 100.0, Node::group(Key::name("root"), vec![at(x, 1.0)]));
        let on: Vec<Scene> = [10.0, 20.0, 180.0, 190.0].iter().map(|x| scene(*x)).collect();
        assert!(check_motion(&on, 220.0).iter().any(|m| m.starts_with("pop:")));
        // Far outside the 200 × 100 view (a region while the camera flies elsewhere): unseen.
        let off: Vec<Scene> = [900.0, 1400.0, 2400.0, 3000.0].iter().map(|x| scene(*x)).collect();
        assert_eq!(check_motion(&off, 220.0), Vec::<String>::new());
    }

    #[test]
    fn a_camera_flight_is_not_a_pop() {
        // A fast pan: the content stays put under a camera that sweeps across it.
        let frames: Vec<Scene> = [0.0, 150.0, 300.0, 450.0]
            .iter()
            .map(|x| {
                let cam = datars_scene::Camera { x: 100.0 + x, y: 50.0, zoom: 1.0, rotation: 0.0 };
                let dots = (0..6).map(|i| Node::shape(Key::one(i as i64), Geom::rect(i as f64 * 100.0, 40.0, 10.0, 10.0))).collect();
                let view = Node::new(Key::name("view"), NodeKind::View { viewport: datars_math::Rect::new(0.0, 0.0, 200.0, 100.0), camera: Some(cam), clip: true, children: dots });
                Scene::new(200.0, 100.0, Node::group(Key::name("root"), vec![view]))
            })
            .collect();
        assert_eq!(check_motion(&frames, 220.0), Vec::<String>::new());
    }

    #[test]
    fn a_crossfade_between_places_is_not_a_pop() {
        // The outgoing copy fades out on the left while the incoming one fades in on the right.
        let frames: Vec<Scene> = [0.0, 0.3, 0.7, 1.0]
            .iter()
            .map(|k| Scene::new(200.0, 100.0, Node::group(Key::name("root"), vec![at(10.0, 1.0 - k), at(180.0, *k)])))
            .collect();
        assert_eq!(check_motion(&frames, 220.0), Vec::<String>::new());
    }

    #[test]
    fn far_off_screen_centres_are_not_pops() {
        let at = |x: f64| {
            let n = Node::shape(Key::name("sea"), Geom::rect(x - 5.0, 40.0, 10.0, 10.0));
            Scene::new(200.0, 100.0, Node::group(Key::name("root"), vec![n]))
        };
        // Moving 300 px per sample far to the left of a 200×100 view: invisible.
        let far: Vec<Scene> = [-5000.0, -4700.0, -4400.0].iter().map(|x| at(*x)).collect();
        assert_eq!(check_motion(&far, 220.0), Vec::<String>::new());
        let near: Vec<Scene> = [0.0, 100.0, 300.0].iter().map(|x| at(*x)).collect();
        assert!(check_motion(&near, 220.0).iter().any(|m| m.starts_with("pop:")));
    }

    #[test]
    fn a_single_element_dipping_to_zero_is_a_blink() {
        let frames: Vec<Scene> = [1.0, 0.5, 0.0, 0.5, 1.0].iter().map(|op| frame(&[*op])).collect();
        let v = check_motion(&frames, 220.0);
        assert!(v.iter().any(|m| m.starts_with("blink:")), "{v:?}");
    }
}

#[cfg(test)]
mod decor_tests {
    use super::*;
    use datars_scene::{Geom, Key, Node, NodeKind, Role, Scene, Semantics};

    fn frame(op: f64, decor: bool) -> Scene {
        let mut dot = Node::new(Key::name("dot"), NodeKind::Shape { geom: Geom::Ellipse { cx: 10.0, cy: 10.0, rx: 3.0, ry: 3.0 }, fill: None, stroke: None, markers: None });
        dot.common.opacity = op;
        let mut g = Node::group(Key::name("layer"), vec![dot]);
        if decor {
            g.semantics = Some(Semantics { role: Role::Decoration, label: String::new(), datum: None, order: None, value: None, link: None });
        }
        Scene::new(100.0, 100.0, g)
    }

    #[test]
    fn scenery_may_flash_data_may_not() {
        // Absent at the start, visible mid-way, fading out again, present (faint) at the end.
        let ops = [0.0, 0.6, 1.0, 0.3, 0.1];
        let run = |decor: bool| {
            let frames: Vec<Scene> = ops.iter().enumerate().map(|(i, &o)| if i == 0 { Scene::new(100.0, 100.0, Node::group(Key::name("layer"), vec![])) } else { frame(o, decor) }).collect();
            check_motion(&frames, 141.0)
        };
        assert!(run(false).iter().any(|v| v.starts_with("flash")), "{:?}", run(false));
        assert!(run(true).is_empty(), "{:?}", run(true));
    }
}
