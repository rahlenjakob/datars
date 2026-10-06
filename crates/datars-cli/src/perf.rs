//! `datars profile`'s transitions, stepped frame by frame the way a 60 Hz display shows them:
//! the input (the new state resolved, the transition planned), then every frame at exactly
//! 1/60 s — the engine's frame (interpolate, fill tiles, flatten) and the raster on a headless
//! GPU renderer, waited for — with what each frame cost the renderer (meshes tessellated, draw
//! calls). A slow transition says which frames are slow and why: a stall before it moves, a
//! steady per-frame cost, or a few hitches.
//!
//! It runs the runtime's own sequence (`goto`, then `frame(now)` until nothing moves) against one
//! renderer kept across the document, as a page keeps one, so the renderer's caches behave as
//! they do in the browser. Absolute times are this machine's (native; wasm is 1.5–3× slower);
//! what matters is where the time goes and the frames that stand out.

use datars_engine::{Engine, FrameOutput};
use std::time::Instant;

/// 60 Hz: the budget for everything a frame does.
pub const FRAME_MS: f64 = 1000.0 / 60.0;

#[derive(Clone, Debug)]
pub struct FrameCost {
    /// Engine time: interpolation, tiles, flattening.
    pub engine: f64,
    /// Raster: GPU encode + submit + execution (waited for), or the CPU reference; `submit` is
    /// its CPU part (tessellating, uploading, encoding), the rest waiting for the GPU.
    pub raster: f64,
    pub submit: f64,
    pub ops: usize,
    /// Meshes the renderer tessellated this frame (and of them fills, strokes, glyphs, halos), and
    /// its draw calls.
    pub tess: u64,
    pub kinds: [u64; 4],
    /// Render passes (1 + two per offscreen layer: group opacity, blend modes).
    pub passes: u32,
    pub draws: u32,
    /// Map tiles the engine decoded and styled this frame, tile bytes handed to it, and the
    /// features it styled.
    pub tiles: [u64; 4],
    /// Instances of big sets the engine flattened afresh (not kept from the frame before), and
    /// instances the renderer built and uploaded (all but those in buffers kept from an earlier
    /// frame). Work, not time: a desktop does 100,000 in a millisecond or two, a phone doesn't, so
    /// a transition that rebuilds a big set every frame is flagged whatever this machine's times.
    pub converted: u64,
    pub rebuilt: u32,
}

/// Instances rebuilt per frame (p50 over a transition) from which a profile flags it: around
/// where a phone starts to feel the per-frame build and upload (the galaxy missed frames at
/// ~30,000 a frame; a 20,000-point scatter moving is expected to reach it).
const REBUILT_FLAG: f64 = 20_000.0;

impl FrameCost {
    pub fn total(&self) -> f64 {
        self.engine + self.raster
    }
}

#[derive(Clone, Debug)]
pub struct TransitionProfile {
    pub from: String,
    pub to: String,
    /// Handling the step itself, before any frame: after the runtime's idle-time preparation
    /// (`Engine::prepare`), and without it (the new state resolved and the transition planned on
    /// input — a host that doesn't prepare, or a step taken before the page was idle).
    pub input: f64,
    pub unprepared: f64,
    pub frames: Vec<FrameCost>,
}

impl TransitionProfile {
    /// Input to the first frame on screen.
    pub fn stall(&self) -> f64 {
        self.input + self.frames.first().map_or(0.0, FrameCost::total)
    }
    /// Frames after the first (the first is the stall's).
    fn steady(&self) -> Vec<f64> {
        self.frames.iter().skip(1).map(FrameCost::total).collect()
    }
    /// 60 Hz frames missed: a frame costing 40 ms keeps two refreshes from showing anything new.
    pub fn dropped(&self) -> usize {
        self.frames.iter().skip(1).map(|f| (f.total() / FRAME_MS).ceil().max(1.0) as usize - 1).sum()
    }
}

fn pct(v: &[f64], q: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s[((q * s.len() as f64) as usize).min(s.len() - 1)]
}

/// Draws a display list and says how long it took.
pub enum Raster {
    Gpu(Box<datars_render_wgpu::Renderer>),
    Cpu(f64),
}

impl Raster {
    /// The GPU at `dpr` for the engine's viewport, or the CPU reference when there's no GPU (or
    /// `cpu`: the browser's fallback path).
    pub fn new(engine: &Engine, dpr: f64, cpu: bool) -> Raster {
        let vp = engine.viewport();
        let (w, h) = ((vp.width * dpr).round().max(1.0) as u32, (vp.height * dpr).round().max(1.0) as u32);
        if !cpu {
            match datars_render_wgpu::Renderer::headless(w, h, dpr) {
                Ok(r) => return Raster::Gpu(Box::new(r)),
                Err(e) => eprintln!("profile: no GPU ({e}); timing the CPU reference instead"),
            }
        }
        Raster::Cpu(dpr)
    }
    /// Warm the renderer with a list (as the runtime does in idle time).
    fn warm(&mut self, list: &datars_render_wgpu::DisplayList, engine: &Engine) {
        if let Raster::Gpu(r) = self {
            r.warm(list, engine.fonts());
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Raster::Gpu(_) => "gpu",
            Raster::Cpu(_) => "cpu",
        }
    }
    /// Draw a frame: (ms, of which CPU-side, meshes tessellated, by kind, draw calls, passes,
    /// instances rebuilt).
    fn draw(&mut self, f: &FrameOutput, engine: &Engine) -> (f64, f64, u64, [u64; 4], u32, u32, u32) {
        let t = Instant::now();
        match self {
            Raster::Gpu(r) => {
                let (before, kinds_before) = (r.stats().tessellations, r.stats().tessellated_kinds);
                r.set_moving(f.animating);
                if let Err(e) = r.render(&f.display, engine.fonts()) {
                    eprintln!("profile: GPU frame failed: {e}");
                }
                let submit = t.elapsed().as_secs_f64() * 1000.0;
                if let Err(e) = r.finish() {
                    eprintln!("profile: GPU frame failed: {e}");
                }
                let s = r.stats();
                let kinds = [0, 1, 2, 3].map(|k| s.tessellated_kinds[k] - kinds_before[k]);
                (t.elapsed().as_secs_f64() * 1000.0, submit, s.tessellations - before, kinds, s.draws, s.passes, s.instances_rebuilt)
            }
            Raster::Cpu(dpr) => {
                std::hint::black_box(datars_render_cpu::render(&f.display, engine.fonts(), *dpr));
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                (ms, ms, 0, [0; 4], 0, 0, 0)
            }
        }
    }
}

/// Settle on `state` (frames until nothing moves, the last drawn twice so the renderer keeps its
/// meshes, as a page that has shown it for a while has).
fn settle(engine: &mut Engine, raster: &mut Raster, now: &mut f64, state: usize) {
    engine.set_clock(*now);
    engine.goto(state);
    for _ in 0..2000 {
        *now += 1.0;
        let f = engine.frame(*now);
        if !f.animating {
            raster.draw(&f, engine);
            *now += 1.0;
            let f = engine.frame(*now);
            raster.draw(&f, engine);
            return;
        }
    }
}

/// Step one transition, `from` → `to` state indices, at `fps`.
pub fn step(engine: &mut Engine, raster: &mut Raster, from: usize, to: usize, fps: f64, now: &mut f64) -> TransitionProfile {
    settle(engine, raster, now, from);
    // A state the reader hasn't seen yet: nothing of it is cached. Unprepared, the step resolves
    // it and plans on input…
    engine.clear_caches();
    let t = Instant::now();
    std::hint::black_box(engine.plan_states(from, to));
    let unprepared = t.elapsed().as_secs_f64() * 1000.0;
    // …but the runtime prepares the steps next to this one while the reader reads.
    engine.clear_caches();
    while engine.prepare() {}
    while let Some(list) = engine.prepared_to_warm() {
        raster.warm(&list, engine);
    }
    let names = engine.state_names();
    *now += 1.0;
    engine.set_clock(*now);
    let t = Instant::now();
    engine.goto(to);
    let input = t.elapsed().as_secs_f64() * 1000.0;
    let mut frames = Vec::new();
    // Up to a minute of frames (a clock that never stops would run forever).
    for _ in 0..(60.0 * fps) as usize {
        *now += 1.0 / fps;
        let (before, converted) = (engine.tile_stats(), engine.instances_converted());
        let t = Instant::now();
        let f = engine.frame(*now);
        let engine_ms = t.elapsed().as_secs_f64() * 1000.0;
        let after = engine.tile_stats();
        let tiles = [after.decoded - before.decoded, after.built - before.built, after.bytes - before.bytes, after.built_features - before.built_features];
        let converted = engine.instances_converted() - converted;
        let (raster_ms, submit, tess, kinds, draws, passes, rebuilt) = raster.draw(&f, engine);
        frames.push(FrameCost { engine: engine_ms, raster: raster_ms, submit, ops: f.display.ops.len(), tess, kinds, draws, passes, tiles, converted, rebuilt });
        if !f.animating {
            break;
        }
    }
    TransitionProfile { from: names.get(from).cloned().unwrap_or_default(), to: names.get(to).cloned().unwrap_or_default(), input, unprepared, frames }
}

/// Per-frame cost as a bar per frame (or per group of frames, to fit `width`): ▁ is nothing,
/// █ a whole 60 Hz frame or more.
fn sparkline(p: &TransitionProfile, width: usize) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let v: Vec<f64> = p.frames.iter().map(FrameCost::total).collect();
    let group = v.len().div_ceil(width.max(1)).max(1);
    v.chunks(group)
        .map(|c| {
            let m = c.iter().copied().fold(0.0, f64::max);
            BARS[((m / FRAME_MS) * 7.0).round().clamp(0.0, 7.0) as usize]
        })
        .collect()
}

pub fn print(p: &TransitionProfile, raster: &str, timeline: bool) {
    let steady = p.steady();
    let over = steady.iter().filter(|v| **v > FRAME_MS).count();
    let first = p.frames.first();
    // Frames after the first: what each rebuilt of its big instance sets (the renderer's count,
    // or the engine's on the CPU raster, which uploads nothing).
    let rebuilt: Vec<f64> = p.frames.iter().skip(1).map(|f| f.rebuilt.max(f.converted as u32) as f64).collect();
    let flag = if p.stall() > 100.0 || pct(&steady, 0.95) > FRAME_MS {
        "  ← slow".to_string()
    } else if pct(&rebuilt, 0.5) >= REBUILT_FLAG {
        format!("  ← rebuilds {:.0} instances a frame (cheap here; check on a phone)", pct(&rebuilt, 0.5))
    } else {
        String::new()
    };
    println!(
        "{} → {}   {} frames   stall {:.0} ms (input {:.0} + first frame {:.0}{}; unprepared input {:.0})   frames p50 {:.1} p95 {:.1} max {:.1} ms   {over} over 16.7, {} dropped{flag}",
        p.from,
        p.to,
        p.frames.len(),
        p.stall(),
        p.input,
        first.map_or(0.0, FrameCost::total),
        first.map_or(String::new(), |f| if f.tess > 0 { format!(": {} meshes tessellated", f.tess) } else { String::new() }),
        p.unprepared,
        pct(&steady, 0.5),
        pct(&steady, 0.95),
        steady.iter().copied().fold(0.0, f64::max),
        p.dropped(),
    );
    println!("    {}", sparkline(p, 72));
    let eng: Vec<f64> = p.frames.iter().skip(1).map(|f| f.engine).collect();
    let ras: Vec<f64> = p.frames.iter().skip(1).map(|f| f.raster).collect();
    let sub: Vec<f64> = p.frames.iter().skip(1).map(|f| f.submit).collect();
    let draws: Vec<f64> = p.frames.iter().skip(1).map(|f| f.draws as f64).collect();
    let tess: Vec<f64> = p.frames.iter().skip(1).map(|f| f.tess as f64).collect();
    println!(
        "    engine p50 {:.1} p95 {:.1} · {raster} p50 {:.1} p95 {:.1} ms (cpu side p50 {:.1}: tessellate, upload, encode) · {:.0} draws, {:.0} tessellated, {:.0} instances rebuilt per frame (p50; max {:.0})",
        pct(&eng, 0.5),
        pct(&eng, 0.95),
        pct(&ras, 0.5),
        pct(&ras, 0.95),
        pct(&sub, 0.5),
        pct(&draws, 0.5),
        pct(&tess, 0.5),
        pct(&rebuilt, 0.5),
        rebuilt.iter().copied().fold(0.0, f64::max)
    );
    // The worst frames after the first, and what they did.
    let mut worst: Vec<(usize, &FrameCost)> = p.frames.iter().enumerate().skip(1).filter(|(_, f)| f.total() > FRAME_MS).collect();
    worst.sort_by(|a, b| b.1.total().total_cmp(&a.1.total()));
    for (k, f) in worst.iter().take(3) {
        let [fi, st, gl, ha] = f.kinds;
        let [dec, styled, bytes, features] = f.tiles;
        let tiles = if dec + styled > 0 { format!("; tiles: {dec} decoded ({} KB in), {styled} styled ({features} features)", bytes / 1024) } else { String::new() };
        println!(
            "    frame {k}: {:.1} ms = engine {:.1}{tiles} + {raster} {:.1} (cpu side {:.1}; {} ops, {} draws, {} passes, {} tessellated: {fi} fills, {st} strokes, {gl} glyphs, {ha} halos; {} instances rebuilt)",
            f.total(),
            f.engine,
            f.raster,
            f.submit,
            f.ops,
            f.draws,
            f.passes,
            f.tess,
            f.rebuilt.max(f.converted as u32)
        );
    }
    if timeline {
        println!("    {:>5} {:>8} {:>8} {:>8} {:>8} {:>6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>9} {:>9}", "frame", "total", "engine", raster, "cpu side", "ops", "draws", "passes", "tess", "decoded", "styled", "converted", "rebuilt");
        for (k, f) in p.frames.iter().enumerate() {
            println!("    {k:>5} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>6} {:>6} {:>6} {:>6} {:>7} {:>7} {:>9} {:>9}", f.total(), f.engine, f.raster, f.submit, f.ops, f.draws, f.passes, f.tess, f.tiles[0], f.tiles[1], f.converted, f.rebuilt);
        }
    }
}

/// One state's costs, each the median of five runs: resolved cold (caches cleared) and warm,
/// flattened to a display list, rastered by the CPU reference.
#[derive(Clone, Debug)]
pub struct StateCost {
    pub name: String,
    pub cold: f64,
    pub warm: f64,
    pub flatten: f64,
    pub cpu_raster: f64,
    pub nodes: usize,
    pub ops: usize,
}

/// What `datars profile` measures — every state, then every transition stepped at `fps` (forward
/// through the program, then back to the start in one jump) — and what `datars dev` shows.
pub struct Profile {
    pub raster: &'static str,
    pub fps: f64,
    pub dpr: f64,
    pub states: Vec<StateCost>,
    pub transitions: Vec<TransitionProfile>,
}

/// Profile a loaded document (`cpu`: time the CPU reference instead of a headless GPU).
pub fn profile(engine: &mut Engine, dpr: f64, fps: f64, cpu: bool) -> Profile {
    let time = |f: &mut dyn FnMut()| {
        let mut v: Vec<f64> = (0..5)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        v.sort_by(|a, b| a.total_cmp(b));
        v[2]
    };
    let n = engine.state_names().len();
    let mut states = Vec::new();
    for i in 0..n {
        let cold = time(&mut || {
            engine.clear_caches();
            let _ = engine.scene_for_state(i);
        });
        let warm = time(&mut || {
            let _ = engine.scene_for_state(i);
        });
        let scene = engine.scene_for_state(i);
        let flatten = time(&mut || {
            let _ = engine.display_list(&scene);
        });
        let list = engine.display_list(&scene);
        let cpu_raster = time(&mut || {
            let _ = datars_render_cpu::render(&list, engine.fonts(), dpr);
        });
        states.push(StateCost { name: engine.state_names()[i].clone(), cold, warm, flatten, cpu_raster, nodes: scene.snapshot().lines().count(), ops: list.ops.len() });
    }
    let mut raster = Raster::new(engine, dpr, cpu);
    let mut pairs: Vec<(usize, usize)> = (1..n).map(|i| (i - 1, i)).collect();
    if n > 2 {
        pairs.push((n - 1, 0));
    }
    let mut now = 10_000.0;
    let transitions = pairs.into_iter().map(|(from, to)| step(engine, &mut raster, from, to, fps, &mut now)).collect();
    Profile { raster: raster.name(), fps, dpr, states, transitions }
}

impl Profile {
    pub fn to_json(&self) -> serde_json::Value {
        let states: Vec<serde_json::Value> = self.states.iter().map(|s| serde_json::json!({ "name": s.name, "cold_ms": s.cold, "warm_ms": s.warm, "flatten_ms": s.flatten, "cpu_raster_ms": s.cpu_raster, "nodes": s.nodes, "ops": s.ops })).collect();
        serde_json::json!({ "raster": self.raster, "fps": self.fps, "dpr": self.dpr, "states": states, "transitions": self.transitions.iter().map(to_json).collect::<Vec<_>>() })
    }
}

pub fn to_json(p: &TransitionProfile) -> serde_json::Value {
    serde_json::json!({
        "from": p.from, "to": p.to, "input_ms": p.input, "unprepared_input_ms": p.unprepared, "stall_ms": p.stall(), "dropped": p.dropped(),
        "frames": p.frames.iter().map(|f| serde_json::json!({ "engine_ms": f.engine, "raster_ms": f.raster, "raster_cpu_ms": f.submit, "ops": f.ops, "tessellated": f.tess, "draws": f.draws, "tiles_decoded": f.tiles[0], "tiles_styled": f.tiles[1], "instances_converted": f.converted, "instances_rebuilt": f.rebuilt })).collect::<Vec<_>>(),
    })
}
