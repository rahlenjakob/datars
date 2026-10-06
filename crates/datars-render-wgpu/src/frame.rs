//! One frame: display-list ops → a flat plan of passes and draws (tessellating cache misses on the
//! way), then uniforms and instances are uploaded in one write each and the plan is encoded.
//!
//! Planning first keeps the encoder free of borrows into the caches, and lets each pass know
//! whether its multisampled contents must survive (a layer interrupts it) or can be discarded.

use crate::cache::MeshKey;
use crate::gpu::{DrawU, GpuMesh, InstanceV, Pipe};
use crate::ramp::RAMP_W;
use crate::tess::{Mesh, StrokeSpec, Tessellators};
use crate::{GpuError, Renderer};
use datars_color::Color;
use datars_math::{m, Affine, FillRule, Vec2};
use datars_render::{DPaint, DisplayList, GlyphSource, InstancesOp, Op, SharedPath, StrokeStyle};
use datars_scene::{Blend, Cap, GlyphPos, Join, Proto};

/// Flattening tolerance in device pixels.
const TOL_PX: f64 = 0.1;
/// The tolerance bucket of fills with no curves: any tolerance tessellates them the same.
const STRAIGHT: i32 = i32::MIN;
/// Strokes at most this wide (device px) on straight edges: past the frame's tessellation share,
/// the path's last mesh at any width stands in (see `Renderer::stand_in`).
const THIN_PX: f64 = 2.5;
/// Meshes given buffers of their own per frame (see `Renderer::mesh`).
const PROMOTE_PER_FRAME: u32 = 64;
/// Meshes tessellated per frame before a cached neighbour stands in for a miss (see
/// `Renderer::stand_in`).
const TESS_PER_FRAME: u32 = 24;

/// Path elements a frame tessellates before new meshes that are barely visible yet — detail
/// fading in, below [`DEFER_BELOW`] opacity — wait for a later frame (times the host's share). A
/// street tile arriving mid-flight brings tens of thousands (12 ms natively for 38,000, three times
/// that in a browser); fading in over its stand-in, it gets them over a few frames instead.
const TESS_ELEMENTS_PER_FRAME: usize = 16_000;
/// Opacity under which a new mesh may wait (see [`TESS_ELEMENTS_PER_FRAME`]).
const DEFER_BELOW: f32 = 0.4;

/// Paths longer than this (elements) in an opaque paint draw as chunks of whole polygons, each its
/// own mesh: a street tile's buildings come as one path of 38,000 elements, and in one piece its
/// tessellation can't be spread over frames. Under 4×MSAA, meshes that share an edge cover each
/// sample once, so the chunks draw exactly what the whole did. (The CPU reference, whose coverage
/// antialiasing would seam them, draws paths whole.)
const CHUNK_ELEMENTS: usize = 4096;
/// Text runs joined into one mesh a frame (times the host's work share; see `glyphs_op`): a new
/// label's outline and halo tessellated whole, ~0.1–0.3 ms each.
const RUNS_PER_FRAME: usize = 24;

/// `path` split into runs of at most about [`CHUNK_ELEMENTS`], or `None` if it's short or one
/// piece. A fill splits only where a polygon's outer ring starts (its holes, the other way round,
/// stay with it), a stroke at any subpath.
fn split_path(path: &datars_math::PathData, fill: bool) -> Option<Vec<SharedPath>> {
    use datars_math::PathEl;
    if path.els.len() <= CHUNK_ELEMENTS {
        return None;
    }
    // Subpaths: (start, end) element ranges and their signed areas.
    let mut subs: Vec<(usize, usize, f64)> = Vec::new();
    let (mut start, mut area, mut first, mut last) = (0usize, 0.0f64, Vec2::ZERO, Vec2::ZERO);
    for (i, e) in path.els.iter().enumerate() {
        let p = match *e {
            PathEl::Move { p } => {
                if i > start {
                    subs.push((start, i, area + (last.x * first.y - first.x * last.y)));
                }
                start = i;
                area = 0.0;
                first = p;
                last = p;
                continue;
            }
            PathEl::Line { p } | PathEl::Quad { p, .. } | PathEl::Cubic { p, .. } => p,
            PathEl::Close => continue,
        };
        area += last.x * p.y - p.x * last.y;
        last = p;
    }
    subs.push((start, path.els.len(), area + (last.x * first.y - first.x * last.y)));
    let outer = subs.first().map_or(1.0, |s| s.2.signum());
    let mut out = Vec::new();
    let mut from = 0usize;
    for &(s, _, a) in subs.iter().skip(1) {
        let new_polygon = !fill || (a.signum() == outer && a != 0.0);
        if s - from >= CHUNK_ELEMENTS && new_polygon {
            out.push(SharedPath::new(datars_math::PathData { els: path.els[from..s].to_vec() }));
            from = s;
        }
    }
    if out.is_empty() {
        return None;
    }
    out.push(SharedPath::new(datars_math::PathData { els: path.els[from..].to_vec() }));
    Some(out)
}
/// Meshes at least this big (bytes) get buffers of their own the first time they're drawn.
const RETAIN_AT_ONCE: u64 = 16 * 1024;
/// Deepest clip nesting the 8-bit stencil counts (one level is kept for translucent strokes).
const MAX_CLIP_DEPTH: u32 = 254;

pub(crate) enum Item {
    /// Start a pass on layer `level` (0 = the target), clearing to `clear` or loading.
    Begin { level: usize, clear: Option<wgpu::Color> },
    End,
    Mesh { pipe: Pipe, slot: u32, mesh: GpuMesh, stencil: u32 },
    /// Instances from this frame's buffer at `offset`, or from a buffer of their own (`buf`).
    /// `kept`: a set drawn from the kept-instances slab (`offset` into its page), else from this
    /// frame's instance buffer at `offset`.
    Inst { pipe: Pipe, slot: u32, offset: u64, count: u32, stencil: u32, kept: Option<crate::gpu::SlabSpan> },
    Composite { pipe: Pipe, slot: u32, level: usize },
}

pub(crate) struct Frame {
    items: Vec<Item>,
    uniforms: Vec<DrawU>,
    instances: Vec<InstanceV>,
    level: usize,
    max_level: usize,
    depth: u32,
    /// Active clips: the draw that pushed each (to pop it), or `None` past the stencil's range.
    clips: Vec<Option<(u32, GpuMesh)>>,
    layers: Vec<(f32, Blend)>,
    dpr: f64,
    vp: [f32; 4],
    srgb: f32,
    skipped: u32,
    /// Big instance sets drawn from buffers of their own this frame, and those given one.
    kept_sets: u32,
    owned: u32,
    /// Instances built on the CPU and uploaded this frame (every one not in a kept set).
    rebuilt: u32,
}

impl Frame {
    fn push_uniform(&mut self, u: DrawU) -> u32 {
        self.uniforms.push(u);
        (self.uniforms.len() - 1) as u32
    }

    /// The common part of a draw's uniforms: the local → device transform (mesh centre folded
    /// into the translation in f64) and the target.
    fn base(&self, dev: &Affine, centre: Vec2) -> DrawU {
        let [a, b, c, d, ..] = dev.0;
        let t = dev.apply(centre);
        DrawU {
            m: [a as f32, b as f32, c as f32, d as f32],
            t: [t.x as f32, t.y as f32, 0.0, 0.0],
            gm: [1.0, 0.0, 0.0, 1.0],
            misc: [1.0, 0.0, 0.0, self.srgb],
            vp: self.vp,
            ..DrawU::default()
        }
    }

    fn draw_mesh(&mut self, pipe: Pipe, u: DrawU, mesh: GpuMesh) {
        if mesh.count == 0 {
            return;
        }
        let slot = self.push_uniform(u);
        self.items.push(Item::Mesh { pipe, slot, mesh, stencil: self.depth });
    }

    /// A stroke; translucent ones blend each sample once (see `Pipe::StrokeOnce`).
    fn draw_stroke(&mut self, u: DrawU, mesh: GpuMesh, translucent: bool) {
        if mesh.count == 0 {
            return;
        }
        let slot = self.push_uniform(u);
        let d = self.depth;
        if translucent && d < 255 {
            self.items.push(Item::Mesh { pipe: Pipe::StrokeOnce, slot, mesh: mesh.clone(), stencil: d });
            self.items.push(Item::Mesh { pipe: Pipe::StrokeRestore, slot, mesh, stencil: d + 1 });
        } else {
            self.items.push(Item::Mesh { pipe: Pipe::Stroke, slot, mesh, stencil: d });
        }
    }

    fn begin(&mut self, level: usize, clear: Option<wgpu::Color>) {
        self.items.push(Item::Begin { level, clear });
    }
}

/// Local → device transform: the op's transform, then the pixel ratio.
fn device(dpr: f64, xf: &Affine) -> Affine {
    Affine::scale(dpr, dpr).mul(*xf)
}

/// Largest singular value of the linear part (the most a unit vector can stretch).
fn max_stretch(xf: &Affine) -> f64 {
    let [a, b, c, d, ..] = xf.0;
    let s = a * a + b * b + c * c + d * d;
    let det = a * d - b * c;
    ((s + (s * s - 4.0 * det * det).max(0.0).sqrt()) / 2.0).sqrt()
}

/// Tessellation-tolerance bucket: log2 of the device scale, rounded up, so a mesh is fine enough
/// for every scale in its bucket and zooming re-tessellates once per octave.
fn tol_bucket(dev: &Affine) -> Option<i32> {
    let s = max_stretch(dev);
    let finite = dev.0.iter().all(|v| v.is_finite());
    (finite && s > 0.0 && s.is_finite() && dev.determinant() != 0.0).then(|| m::log2(s).ceil().clamp(-64.0, 64.0) as i32)
}

/// Whether the linear part is a uniform scale times a rotation (or reflection): strokes built in
/// local units stay even under it.
fn is_similarity(xf: &Affine) -> bool {
    let [a, b, c, d, ..] = xf.0;
    let eps = 1e-9 * (a.abs() + b.abs() + c.abs() + d.abs());
    ((a - d).abs() <= eps && (b + c).abs() <= eps) || ((a + d).abs() <= eps && (b - c).abs() <= eps)
}

fn tol_local(bucket: i32) -> f64 {
    TOL_PX / m::exp2(bucket as f64)
}

/// Quarter-octave bucket of a stroke width in local units; meshes are built at the bucket's width
/// and the shader extrudes to the exact one.
fn width_bucket(w: f64) -> i32 {
    (m::log2(w) * 4.0).round().clamp(-400.0, 400.0) as i32
}

fn bucket_width(b: i32) -> f64 {
    m::exp2(b as f64 / 4.0)
}

/// Eighth-octave bucket of a glyph size: outlines are fetched at the bucket size and scaled.
fn size_bucket(size: f64) -> i32 {
    (m::log2(size) * 8.0).round().clamp(-400.0, 400.0) as i32
}

fn bucket_size(b: i32) -> f64 {
    m::exp2(b as f64 / 8.0)
}

/// Width in local units for a stroke `width_css` px wide under local scale `s`, and an alpha
/// factor: strokes thinner than a device pixel draw one pixel wide at proportionally lower alpha
/// (what coverage rasterization produces; MSAA alone would break them up).
fn stroke_width_local(width_css: f64, s: f64, dpr: f64) -> (f64, f32) {
    let dev = width_css * dpr;
    if dev < 1.0 {
        (1.0 / (dpr * s), dev as f32)
    } else {
        (width_css / s, 1.0)
    }
}

/// `v > 0`, false for NaN.
fn positive(v: f64) -> bool {
    v > 0.0
}

fn opaque(p: &DPaint) -> bool {
    match p {
        DPaint::Solid(c) => c.a >= 1.0,
        DPaint::Linear { stops, .. } | DPaint::Radial { stops, .. } => stops.iter().all(|s| s.1.a >= 1.0),
    }
}

fn cap_u8(c: Cap) -> u8 {
    c as u8
}
fn join_u8(j: Join) -> u8 {
    j as u8
}

/// A thin stroke's identity without its width and tolerance buckets: path, cap, join, miter (thin
/// strokes are undashed, in local space).
fn thin_id(key: &MeshKey) -> u64 {
    match key {
        MeshKey::Stroke { path, cap, join, miter, .. } => path ^ ((*cap as u64) << 56) ^ ((*join as u64) << 48) ^ miter.rotate_left(17),
        _ => 0,
    }
}

/// Instance sets at least this big get a buffer of their own when drawn unchanged. A point
/// pyramid draws its tiles as screen-sized cells, a few dozen to a few thousand rows each and
/// hundreds of them: kept only from 512, the galaxy rebuilt and re-uploaded ~30,000 stars every
/// frame of a zoom — nothing on a desktop, missed frames on a phone.
const INST_OWN: usize = 32;

/// Buffers of their own a frame may create (times the host's share, `set_work_scale`): a new
/// level of cells arriving at once waits a frame or two in the shared buffer rather than
/// allocating hundreds of buffers in one frame.
const OWN_PER_FRAME: u32 = 48;

/// A big instance set's uploaded instances (`kept`, in the kept-instances slab, from its second
/// frame with the same columns).
pub(crate) struct InstBuf {
    pub kept: Option<crate::gpu::SlabSpan>,
    pub count: u32,
    pub centre: Vec2,
    pub last: u64,
    /// The columns themselves: while they're held, no other set can land at their addresses (the key).
    #[allow(dead_code)]
    keep: InstCols,
}

type InstCols = (std::sync::Arc<[f64]>, std::sync::Arc<[f64]>, std::sync::Arc<[f64]>, std::sync::Arc<[Color]>, std::sync::Arc<[f32]>, Option<std::sync::Arc<[f64]>>, Option<std::sync::Arc<[f64]>>);

fn inst_cols(op: &InstancesOp) -> InstCols {
    (op.x.clone(), op.y.clone(), op.size.clone(), op.fill.clone(), op.opacity.clone(), op.w.clone(), op.h.clone())
}

/// An instance set's columns, by identity (the engine hands out the same ones while it's unchanged).
/// A glyph's place in its run's outline: in outline units, to 1/256 — the same at any size in a
/// size bucket (shaped places scale with the size), so a run's mesh serves it as it grows.
fn run_place(v: f32, k: f64) -> f64 {
    (v as f64 / k * 256.0).round() / 256.0
}

/// A run's glyphs and their places (see [`run_place`]).
fn run_hash(glyphs: &[GlyphPos], k: f64) -> u64 {
    let mut h = datars_math::Hash64::new();
    for g in glyphs {
        h.u32(g.id as u32);
        h.f64(run_place(g.x, k));
        h.f64(run_place(g.y, k));
    }
    h.finish()
}

fn inst_key(op: &InstancesOp) -> [usize; 8] {
    let p = |a: &Option<std::sync::Arc<[f64]>>| a.as_ref().map_or(0, |a| a.as_ptr() as usize);
    [op.x.as_ptr() as usize, op.y.as_ptr() as usize, op.size.as_ptr() as usize, op.fill.as_ptr() as usize, op.opacity.as_ptr() as usize, p(&op.w), p(&op.h), op.len()]
}

impl Renderer {
    /// Meshes a frame may tessellate before stand-ins take over: [`TESS_PER_FRAME`] times the
    /// host's share ([`Renderer::set_work_scale`]).
    fn tess_budget(&self) -> u32 {
        ((TESS_PER_FRAME as f64 * self.work) as u32).max(4)
    }

    /// A big path's chunks (see [`CHUNK_ELEMENTS`]), split once and kept, when its paint is opaque
    /// (chunks drawn one after another blend overlaps twice in a translucent one).
    fn chunks_of(&mut self, path: &SharedPath, paint: &DPaint, fill: bool) -> Option<std::rc::Rc<[SharedPath]>> {
        if path.path.els.len() <= CHUNK_ELEMENTS || !opaque(paint) {
            return None;
        }
        if let Some(c) = self.chunks.get(&path.hash) {
            return Some(c.clone());
        }
        let c: std::rc::Rc<[SharedPath]> = split_path(&path.path, fill)?.into();
        if self.chunks.len() > 4096 {
            self.chunks.clear();
        }
        self.chunks.insert(path.hash, c.clone());
        Some(c)
    }

    /// Whether a new mesh for `key` (`elements` long, drawn at `opacity`) waits for a later frame:
    /// barely visible yet (fading in over its stand-in) and this frame has tessellated its share.
    /// The draw is skipped and counted as a stand-in, so frames keep coming until it's built.
    fn defer(&self, key: &MeshKey, opacity: f32, elements: usize) -> bool {
        opacity < DEFER_BELOW
            && self.tessellated_now > 0
            && self.tess_elements_now + elements > ((TESS_ELEMENTS_PER_FRAME as f64 * self.work) as usize).max(2_000)
            && !self.meshes.contains(key)
            && !self.recent.contains_key(key)
    }

    /// Remember the mesh a thin stroke's path was drawn with (its stand-in at other widths).
    fn note_thin(&mut self, key: &MeshKey) {
        if self.thin_last.len() > 200_000 {
            self.thin_last.clear();
        }
        self.thin_last.insert(thin_id(key), key.clone());
    }

    /// When this frame has tessellated its share and `key` isn't cached: the same path's mesh at a
    /// neighbouring tolerance or width bucket, if that one is. A zoom crosses a stroke-width bucket
    /// every quarter octave and a tolerance bucket every octave, and each crossing would
    /// re-tessellate every road on screen in one frame; the neighbour draws the same outline (the
    /// shader extrudes strokes to their exact width) with joins and curves a bucket off, until the
    /// exact mesh is built on a later frame. Dashed strokes don't stand in (their dashes would
    /// jump).
    ///
    /// A thin stroke of straight edges (`thin`: county lines, borders — thousands on a map) takes
    /// the mesh its path was last drawn with, however many buckets away: only its joins were
    /// shaped by that width, which doesn't show this thin, and a long zoom would otherwise leave
    /// the neighbours behind within a few buckets and re-tessellate every border in one frame.
    fn stand_in(&self, key: &MeshKey, thin: bool) -> Option<MeshKey> {
        // In motion, a thin stroke takes its path's last mesh at once — no lookup of an exact
        // bucket that would be stale a frame later (thousands of borders, every frame).
        if thin && self.moving && !self.warming {
            if let Some(k) = self.thin_last.get(&thin_id(key)).filter(|k| self.meshes.contains(k) || self.recent.contains_key(*k)) {
                return Some(k.clone());
            }
        }
        if self.tessellated_now < self.tess_budget() || self.meshes.contains(key) {
            return None;
        }
        if thin {
            if let Some(k) = self.thin_last.get(&thin_id(key)).filter(|k| self.meshes.contains(k) || self.recent.contains_key(*k)) {
                return Some(k.clone());
            }
        }
        const NEAR: [i32; 4] = [1, -1, 2, -2];
        let candidates: Vec<MeshKey> = match key {
            MeshKey::Fill { tol, .. } if *tol == STRAIGHT => Vec::new(),
            MeshKey::Fill { path, rule, tol } => NEAR.iter().map(|d| MeshKey::Fill { path: *path, rule: *rule, tol: tol + d }).collect(),
            MeshKey::Stroke { path, space, width, cap, join, miter, dash: None, tol } => {
                let at = |dw: i32, dt: i32| MeshKey::Stroke { path: *path, space: *space, width: width + dw, cap: *cap, join: *join, miter: *miter, dash: None, tol: tol + dt };
                let mut v: Vec<MeshKey> = NEAR.iter().map(|&dw| at(dw, 0)).collect();
                for dt in [1, -1] {
                    v.push(at(0, dt));
                    v.extend(NEAR.iter().map(|&dw| at(dw, dt)));
                }
                v
            }
            _ => Vec::new(),
        };
        candidates.into_iter().find(|k| self.meshes.contains(k))
    }

    /// Look up or build a mesh. A mesh gets GPU buffers of its own the second frame in a row it
    /// is drawn; the first time it goes into this frame's arena and is kept CPU-side for one
    /// frame. Anything that stays on screen is retained from its second frame on; a morph's
    /// outlines, new every frame, never touch the cache (thousands of them per frame would
    /// otherwise each cost two buffers and a cache entry, and evict the city).
    fn mesh(&mut self, key: MeshKey, build: impl FnOnce(&mut Tessellators) -> Mesh) -> GpuMesh {
        if let Some(m) = self.meshes.touch(&key, self.frame) {
            self.stats.cache_hits += 1;
            return m.clone();
        }
        let stroke = matches!(key, MeshKey::Stroke { .. } | MeshKey::Halo { .. } | MeshKey::Run { width: Some(_), .. });
        if let Some((mesh, seen)) = self.recent.swap_remove(&key) {
            if seen + 1 >= self.frame {
                self.stats.cache_hits += 1;
                // A few buffers per frame: the dots of a staggered transition that haven't set off
                // yet look the same two frames running, and thousands of buffers at once would be
                // the frame's hitch (then garbage, once they move). The rest draw from the arena
                // again, which costs a copy, and settle into the cache over the next frames.
                if self.promoted < ((PROMOTE_PER_FRAME as f64 * self.work) as u32).max(16) {
                    self.promoted += 1;
                    let g = self.pool.put(&self.gpu.device, &mesh, stroke);
                    return self.meshes.insert(key, g.clone(), mesh.bytes() + 64, self.frame).clone();
                }
                let g = self.arena.push(&mesh, stroke);
                self.recent.insert(key, (mesh, self.frame));
                return g;
            }
        }
        let mesh = build(&mut self.tess);
        self.stats.tessellations += 1;
        self.stats.tessellated_kinds[match key {
            MeshKey::Fill { .. } => 0,
            MeshKey::Stroke { .. } => 1,
            MeshKey::Glyph { .. } | MeshKey::Run { width: None, .. } => 2,
            MeshKey::Halo { .. } | MeshKey::Run { .. } => 3,
        }] += 1;
        self.tessellated_now += 1;
        // A big mesh (a basemap tile, a coastline) is geometry that stays: retained at once, not
        // copied through the arena frame after frame while it waits its turn. Morph outlines are
        // small. Warming (a list not drawn) keeps everything.
        if mesh.bytes() >= RETAIN_AT_ONCE || self.warming {
            let g = self.pool.put(&self.gpu.device, &mesh, stroke);
            return self.meshes.insert(key, g.clone(), mesh.bytes() + 64, self.frame).clone();
        }
        let g = self.arena.push(&mesh, stroke);
        self.recent.insert(key, (mesh, self.frame));
        g
    }

    /// Set a draw's paint uniforms (gradient geometry relative to the mesh centre).
    /// `centre`: the local-space point that vertex space (mapped through `u.gm`) is relative to.
    fn paint(&mut self, u: &mut DrawU, paint: &DPaint, centre: Vec2, opacity: f32) {
        u.misc[0] = opacity;
        match paint {
            DPaint::Solid(c) => {
                u.color = [c.r, c.g, c.b, c.a];
            }
            DPaint::Linear { p0, p1, stops } => {
                let row = self.ramps.row_for(stops, self.frame);
                let (a, b) = (*p0 - centre, *p1 - centre);
                u.grad = [a.x as f32, a.y as f32, b.x as f32, b.y as f32];
                u.misc[1] = 1.0;
                u.misc[2] = row as f32;
            }
            DPaint::Radial { c, r, stops } => {
                let row = self.ramps.row_for(stops, self.frame);
                let a = *c - centre;
                u.grad = [a.x as f32, a.y as f32, *r as f32, 0.0];
                u.misc[1] = 2.0;
                u.misc[2] = row as f32;
            }
        }
    }

    /// Plan a frame.
    pub(crate) fn plan(&mut self, list: &DisplayList, glyphs: &dyn GlyphSource) -> Frame {
        let bg = list.background;
        let lin = |v: f32| if self.srgb_target { datars_color::to_linear(v as f64) } else { v as f64 };
        let a = bg.a.clamp(0.0, 1.0) as f64;
        let clear = wgpu::Color { r: lin(bg.r) * a, g: lin(bg.g) * a, b: lin(bg.b) * a, a };
        self.arena.clear();
        self.promoted = 0;
        self.tessellated_now = 0;
        self.tess_elements_now = 0;
        self.runs_now = 0;
        self.stand_ins_now = 0;
        let mut fr = Frame {
            items: Vec::with_capacity(list.ops.len() + 2),
            uniforms: Vec::with_capacity(list.ops.len()),
            instances: Vec::new(),
            level: 0,
            max_level: 0,
            depth: 0,
            clips: Vec::new(),
            layers: Vec::new(),
            dpr: self.dpr,
            vp: [self.size.0 as f32, self.size.1 as f32, 0.0, 0.0],
            srgb: if self.srgb_target { 1.0 } else { 0.0 },
            skipped: 0,
            kept_sets: 0,
            owned: 0,
            rebuilt: 0,
        };
        fr.begin(0, Some(clear));
        for op in &list.ops {
            match op {
                Op::Fill { path, xf, paint, rule, opacity } => self.fill_op(&mut fr, path, xf, paint, *rule, *opacity),
                Op::Stroke { path, xf, style, paint, opacity } => self.stroke_op(&mut fr, path, xf, style, paint, *opacity),
                Op::Glyphs { font, size, glyphs: g, origin, scale, rotate, color, halo, .. } => {
                    self.glyphs_op(&mut fr, glyphs, font, *size, g, *origin, *scale, *rotate, *color, *halo)
                }
                Op::Instances(i) => self.instances_op(&mut fr, i),
                Op::Image { .. } => fr.skipped += 1,
                Op::PushClip { path, xf, rule } => self.push_clip(&mut fr, path, xf, *rule),
                Op::PopClip => pop_clip(&mut fr),
                Op::PushLayer { opacity, blend } => push_layer(&mut fr, *opacity, *blend),
                Op::PopLayer => pop_layer(&mut fr),
            }
        }
        while !fr.layers.is_empty() {
            pop_layer(&mut fr);
        }
        fr.items.push(Item::End);
        fr
    }

    fn fill_op(&mut self, fr: &mut Frame, path: &SharedPath, xf: &Affine, paint: &DPaint, rule: FillRule, opacity: f32) {
        if opacity <= 0.0 {
            return;
        }
        if let Some(chunks) = self.chunks_of(path, paint, true) {
            for c in chunks.iter() {
                self.fill_op(fr, c, xf, paint, rule, opacity);
            }
            return;
        }
        let dev = device(fr.dpr, xf);
        let Some(tol) = tol_bucket(&dev) else { return };
        // All straight edges (buildings, water, land): the triangles don't depend on the curve
        // tolerance, so one mesh serves every zoom — a camera flight doesn't re-tessellate the city.
        let key = MeshKey::Fill { path: path.hash, rule: rule as u8, tol: if path.straight { STRAIGHT } else { tol } };
        let key = match self.stand_in(&key, false) {
            Some(k) => {
                self.stand_ins_now += 1;
                k
            }
            None => key,
        };
        if self.defer(&key, opacity, path.path.els.len()) {
            self.stand_ins_now += 1;
            return;
        }
        let before = self.tessellated_now;
        let mesh = self.mesh(key, |t| t.fill(&path.path, rule, tol_local(tol)));
        if self.tessellated_now > before {
            self.tess_elements_now += path.path.els.len();
        }
        let mut u = fr.base(&dev, mesh.centre);
        self.paint(&mut u, paint, mesh.centre, opacity);
        fr.draw_mesh(Pipe::Fill, u, mesh);
    }

    fn stroke_op(&mut self, fr: &mut Frame, path: &SharedPath, xf: &Affine, style: &StrokeStyle, paint: &DPaint, opacity: f32) {
        let s = xf.scale_factor();
        if opacity <= 0.0 || !positive(style.width_px) || !positive(s) || !s.is_finite() {
            return;
        }
        if style.dash.is_none() {
            if let Some(chunks) = self.chunks_of(path, paint, false) {
                for c in chunks.iter() {
                    self.stroke_op(fr, c, xf, style, paint, opacity);
                }
                return;
            }
        }
        let dev = device(fr.dpr, xf);
        let Some(tol) = tol_bucket(&dev) else { return };
        // Under a similarity (translate, rotate, uniform scale) the mesh is built in local units:
        // exact, and shared across zoom levels. Under non-uniform scale or shear a local-space
        // stroke would come out thicker in one direction, so the mesh is built in device space
        // instead (still translation-free: panning keeps hitting the cache).
        let lin = Affine([dev.0[0], dev.0[1], dev.0[2], dev.0[3], 0.0, 0.0]);
        let (space, css_per_unit, tol_units, tol_key) = if is_similarity(&dev) {
            (None, s, tol_local(tol), tol)
        } else {
            (Some([lin.0[0], lin.0[1], lin.0[2], lin.0[3]].map(f64::to_bits)), 1.0 / fr.dpr, TOL_PX, 0)
        };
        let (w, alpha) = stroke_width_local(style.width_px, css_per_unit, fr.dpr);
        let wb = width_bucket(w);
        let dash: Option<Vec<f64>> = style.dash.as_ref().map(|d| d.iter().map(|v| v / css_per_unit).collect());
        let thin = space.is_none() && path.straight && dash.is_none() && w * css_per_unit * fr.dpr <= THIN_PX;
        let key = MeshKey::Stroke {
            path: path.hash,
            space,
            width: wb,
            cap: cap_u8(style.cap),
            join: join_u8(style.join),
            miter: style.miter_limit.to_bits(),
            dash: dash.as_ref().map(|d| d.iter().map(|v| v.to_bits()).collect()),
            tol: tol_key,
        };
        let spec = StrokeSpec { width: bucket_width(wb), cap: style.cap, join: style.join, miter_limit: style.miter_limit, dash };
        let key = match self.stand_in(&key, thin) {
            Some(k) => {
                self.stand_ins_now += 1;
                k
            }
            None => key,
        };
        if self.defer(&key, opacity, path.path.els.len()) {
            self.stand_ins_now += 1;
            return;
        }
        let built_before = self.tessellated_now;
        let mesh = match space {
            None => self.mesh(key.clone(), |t| t.stroke(&path.path, &spec, tol_units)),
            Some(_) => self.mesh(key.clone(), |t| t.stroke(&path.path.transform(&lin), &spec, tol_units)),
        };
        // A new mesh for a thin stroke: its path's stand-in from now on (noted when built, not on
        // every draw — thousands of borders a frame).
        if self.tessellated_now > built_before {
            self.tess_elements_now += path.path.els.len();
            if thin {
                self.note_thin(&key);
            }
        }
        let (mut u, reference) = match (space, lin.inverse()) {
            (Some(_), Some(inv)) => {
                let mut u = fr.base(&Affine::translate(dev.0[4], dev.0[5]), mesh.centre);
                u.gm = [inv.0[0] as f32, inv.0[1] as f32, inv.0[2] as f32, inv.0[3] as f32];
                (u, inv.apply(mesh.centre))
            }
            _ => (fr.base(&dev, mesh.centre), mesh.centre),
        };
        u.t[2] = (w / 2.0) as f32;
        let op = opacity * alpha;
        self.paint(&mut u, paint, reference, op);
        fr.draw_stroke(u, mesh, op < 1.0 || !opaque(paint));
    }

    #[allow(clippy::too_many_arguments)]
    fn glyphs_op(
        &mut self,
        fr: &mut Frame,
        src: &dyn GlyphSource,
        font: &std::sync::Arc<str>,
        size: f64,
        glyphs: &[GlyphPos],
        origin: Vec2,
        scale: f64,
        rotate: f64,
        color: Color,
        halo: Option<(Color, f64)>,
    ) {
        if !positive(size) || !positive(scale) || glyphs.is_empty() || !origin.is_finite() || !rotate.is_finite() {
            return;
        }
        // Outlines come at the bucket size `bs` and scale by k; placement is
        // dpr · (origin + R(rotate) · scale · (glyph offset + k · outline)).
        let sb = size_bucket(size);
        let bs = bucket_size(sb);
        let k = size / bs;
        let run = Affine::scale(fr.dpr, fr.dpr).mul(Affine::translate(origin.x, origin.y)).mul(Affine::rotate(rotate)).mul(Affine::scale(scale, scale));
        let Some(tol) = tol_bucket(&run.mul(Affine::scale(k, k))) else { return };
        let place = |g: &GlyphPos| run.mul(Affine::translate(g.x as f64, g.y as f64)).mul(Affine::scale(k, k));
        // The run as one outline, in outline units (glyph places ÷ k): one draw for its fill and
        // one for its halo, where each glyph was two — on GL (phones, the Android emulator, WebGL)
        // every draw costs, and a map's labels were most of its draws. As the CPU reference
        // draws a run: one path. Joined from the run's second frame running (numbers counting up
        // are new text every frame, and stay per glyph, from the glyph cache), a few a frame.
        let whole = glyphs.len() > 1 && self.run_joins(font, glyphs, k, sb, tol);
        let hash = if whole { run_hash(glyphs, k) } else { 0 };
        let outline = || {
            let mut p = datars_math::PathData::new();
            for g in glyphs {
                if let Some(o) = src.outline(font, g.id, bs) {
                    p.extend(&o.transform(&Affine::translate(run_place(g.x, k), run_place(g.y, k))));
                }
            }
            p
        };
        // Halos first, as strokes underneath: a `w`-wide stroke of the outline (w/2 beyond it),
        // scaling with the text — as the CPU reference and the SVG writer draw it.
        if let Some((hc, hw)) = halo {
            if hw > 0.0 && hc.a > 0.0 {
                let (w_local, alpha) = stroke_width_local(hw * scale, scale * k, fr.dpr);
                let wb = width_bucket(w_local);
                let spec = StrokeSpec { width: bucket_width(wb), cap: Cap::Round, join: Join::Round, miter_limit: 4.0, dash: None };
                let halo = |fr: &mut Frame, mesh: GpuMesh, at: Affine| {
                    let mut u = fr.base(&at, mesh.centre);
                    u.t[2] = (w_local / 2.0) as f32;
                    u.color = [hc.r, hc.g, hc.b, hc.a];
                    u.misc[0] = alpha;
                    fr.draw_stroke(u, mesh, hc.a < 1.0 || alpha < 1.0);
                };
                if whole {
                    let key = MeshKey::Run { font: font.clone(), glyphs: hash, size: sb, width: Some(wb), tol };
                    let mesh = self.mesh(key, |t| t.stroke(&outline(), &spec, tol_local(tol)));
                    halo(fr, mesh, run.mul(Affine::scale(k, k)));
                } else {
                    for g in glyphs {
                        let key = MeshKey::Halo { font: font.clone(), glyph: g.id, size: sb, width: wb, tol };
                        let mesh = self.mesh(key, |t| src.outline(font, g.id, bs).map(|p| t.stroke(&p, &spec, tol_local(tol))).unwrap_or_default());
                        halo(fr, mesh, place(g));
                    }
                }
            }
        }
        if color.a <= 0.0 {
            return;
        }
        let fill = |fr: &mut Frame, mesh: GpuMesh, at: Affine| {
            let mut u = fr.base(&at, mesh.centre);
            u.color = [color.r, color.g, color.b, color.a];
            fr.draw_mesh(Pipe::Fill, u, mesh);
        };
        if whole {
            let key = MeshKey::Run { font: font.clone(), glyphs: hash, size: sb, width: None, tol };
            let mesh = self.mesh(key, |t| t.fill(&outline(), FillRule::NonZero, tol_local(tol)));
            fill(fr, mesh, run.mul(Affine::scale(k, k)));
        } else {
            for g in glyphs {
                let key = MeshKey::Glyph { font: font.clone(), glyph: g.id, size: sb, tol };
                let mesh = self.mesh(key, |t| src.outline(font, g.id, bs).map(|p| t.fill(&p, FillRule::NonZero, tol_local(tol))).unwrap_or_default());
                fill(fr, mesh, place(g));
            }
        }
    }

    /// Whether this run draws as one mesh this frame: it already has one, or it was drawn last
    /// frame too and this frame still has room to join runs.
    fn run_joins(&mut self, font: &std::sync::Arc<str>, glyphs: &[GlyphPos], k: f64, sb: i32, tol: i32) -> bool {
        let hash = run_hash(glyphs, k);
        let key = MeshKey::Run { font: font.clone(), glyphs: hash, size: sb, width: None, tol };
        if self.meshes.contains(&key) || self.recent.contains_key(&key) {
            return true;
        }
        if self.runs_seen.len() > 4096 {
            self.runs_seen.clear();
        }
        let last = self.runs_seen.insert(hash, self.frame);
        let join = (self.warming || last.is_some_and(|f| f + 1 >= self.frame)) && self.runs_now < ((RUNS_PER_FRAME as f64 * self.work) as u32).max(4);
        if join {
            self.runs_now += 1;
        }
        join
    }

    fn instances_op(&mut self, fr: &mut Frame, op: &InstancesOp) {
        let n = op.len().min(op.y.len());
        let dev = device(fr.dpr, &op.xf);
        if n == 0 || tol_bucket(&dev).is_none() {
            return;
        }
        let rect = matches!(op.proto, Proto::Rect);
        // A big set drawn with the same columns as last frame (the engine's flatten keeps an
        // unchanged set's columns) draws from a buffer of its own, uploaded once — not rebuilt
        // and re-uploaded every frame (400,000 dots on a map during a camera move).
        let key = (n >= INST_OWN).then(|| inst_key(op));
        let own = key.as_ref().and_then(|k| self.inst_bufs.get_mut(k)).and_then(|b| {
            b.last = self.frame;
            b.kept.map(|span| (span, b.count, b.centre))
        });
        let before = fr.instances.len();
        let (c, count, kept, offset) = match own {
            Some((span, count, c)) => {
                fr.kept_sets += 1;
                (c, count, Some(span), span.offset)
            }
            None => {
                // Positions are stored relative to the set's centre (precision, as for meshes).
                let (mut lo, mut hi) = (Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN));
                for i in 0..n {
                    let (x, y) = (op.x[i], op.y[i]);
                    if x.is_finite() && y.is_finite() {
                        lo = Vec2::new(lo.x.min(x), lo.y.min(y));
                        hi = Vec2::new(hi.x.max(x), hi.y.max(y));
                    }
                }
                if lo.x > hi.x {
                    return;
                }
                let c = (lo + hi) * 0.5;
                let fallback = op.fill.first().copied().unwrap_or(Color::BLACK);
                let offset = (fr.instances.len() * std::mem::size_of::<InstanceV>()) as u64;
                for i in 0..n {
                    let (x, y) = (op.x[i], op.y[i]);
                    let o = op.opacity.get(i).copied().unwrap_or(1.0);
                    if !(x.is_finite() && y.is_finite()) || !positive(o as f64) {
                        continue;
                    }
                    let size = if rect {
                        let w = op.w.as_ref().and_then(|w| w.get(i)).copied().unwrap_or(1.0);
                        let h = op.h.as_ref().and_then(|h| h.get(i)).copied().unwrap_or(1.0);
                        [w as f32, h as f32]
                    } else {
                        let s = op.size.get(i).copied().unwrap_or(3.0) as f32;
                        [s, s]
                    };
                    if !(size[0].is_finite() && size[1].is_finite()) {
                        continue;
                    }
                    fr.instances.push(InstanceV {
                        pos: [(x - c.x) as f32, (y - c.y) as f32],
                        size,
                        color: op.fill.get(i).copied().unwrap_or(fallback).to_rgba8(),
                        opacity: o.min(1.0),
                    });
                }
                let count = (fr.instances.len() - before) as u32;
                fr.rebuilt += count;
                match key {
                    // Seen with these columns last frame too: its own buffer from now on.
                    Some(k) if count > 0 && fr.owned < ((OWN_PER_FRAME as f64 * self.work) as u32).max(4) && self.inst_bufs.get(&k).is_some_and(|b| b.kept.is_none()) => {
                        fr.owned += 1;
                        let span = self.kept.put(&self.gpu.device, bytemuck::cast_slice(&fr.instances[before..]));
                        fr.instances.truncate(before);
                        self.inst_bufs.insert(k, InstBuf { kept: Some(span), count, centre: c, last: self.frame, keep: inst_cols(op) });
                        (c, count, Some(span), span.offset)
                    }
                    Some(k) => {
                        self.inst_bufs.entry(k).or_insert_with(|| InstBuf { kept: None, count, centre: c, last: 0, keep: inst_cols(op) }).last = self.frame;
                        (c, count, None, offset)
                    }
                    None => (c, count, None, offset),
                }
            }
        };
        if count == 0 {
            return;
        }
        let mut u = fr.base(&dev, c);
        if let Some((sc, sw)) = op.stroke {
            if sw > 0.0 && sc.a > 0.0 {
                u.color = [sc.r, sc.g, sc.b, sc.a];
                u.inst[0] = (sw * fr.dpr) as f32;
            }
        }
        u.inst[1] = (op.size_scale * fr.dpr) as f32;
        u.inst[2] = 1.0;
        // The groups' opacity, for the whole set: a fade changes this, never the uploaded buffer.
        u.misc[0] = op.alpha.clamp(0.0, 1.0);
        if let Proto::Symbol { symbol } = op.proto {
            // The SDF polygon comes from the same geometry the CPU reference fills.
            if symbol != datars_scene::SymbolKind::Circle {
                let path = datars_scene::Geom::Symbol { kind: symbol, x: 0.0, y: 0.0, size: 1.0 }.to_path();
                let pts: Vec<Vec2> = path.flatten(1e-3).into_iter().next().map(|(p, _)| p).unwrap_or_default();
                let pts = &pts[..pts.len().min(12)];
                for (j, p) in pts.iter().enumerate() {
                    u.poly[j / 2][(j % 2) * 2] = p.x as f32;
                    u.poly[j / 2][(j % 2) * 2 + 1] = p.y as f32;
                }
                u.inst[2] = pts.iter().map(|p| p.len()).fold(0.0, f64::max) as f32;
                u.inst[3] = pts.len() as f32;
            }
        } else {
            // A rect needs non-degenerate axes for its pixel-space distance field.
            let [a, b, cc, d, ..] = dev.0;
            if a * a + b * b == 0.0 || cc * cc + d * d == 0.0 {
                if kept.is_none() {
                    fr.instances.truncate(before);
                }
                return;
            }
        }
        let slot = fr.push_uniform(u);
        let pipe = if rect { Pipe::Rects } else { Pipe::Symbols };
        fr.items.push(Item::Inst { pipe, slot, offset, count, stencil: fr.depth, kept });
    }

    fn push_clip(&mut self, fr: &mut Frame, path: &SharedPath, xf: &Affine, rule: FillRule) {
        if fr.depth >= MAX_CLIP_DEPTH {
            fr.clips.push(None);
            return;
        }
        let dev = device(fr.dpr, xf);
        let mesh = match tol_bucket(&dev) {
            Some(tol) => self.mesh(MeshKey::Fill { path: path.hash, rule: rule as u8, tol }, |t| t.fill(&path.path, rule, tol_local(tol))),
            // A degenerate clip hides everything: count nothing, so no sample reaches depth + 1.
            None => GpuMesh::empty(Vec2::ZERO),
        };
        let u = fr.base(&dev, mesh.centre);
        let slot = fr.push_uniform(u);
        if mesh.count > 0 {
            fr.items.push(Item::Mesh { pipe: Pipe::ClipPush, slot, mesh: mesh.clone(), stencil: fr.depth });
        }
        fr.depth += 1;
        fr.clips.push(Some((slot, mesh)));
    }

    /// Upload and encode a planned frame into `view`.
    pub(crate) fn submit(&mut self, fr: Frame, view: &wgpu::TextureView) -> Result<(), GpuError> {
        let stride = self.gpu.ustride as usize;
        let slots = fr.uniforms.len().max(1) as u64;
        if slots > self.ucap {
            self.ucap = slots.next_power_of_two();
            self.ubuf.destroy();
            (self.ubuf, self.ubind) = self.gpu.uniform_buffer(self.ucap);
        }
        let mut bytes = vec![0u8; stride * fr.uniforms.len()];
        for (i, u) in fr.uniforms.iter().enumerate() {
            bytes[i * stride..i * stride + std::mem::size_of::<DrawU>()].copy_from_slice(bytemuck::bytes_of(u));
        }
        if !bytes.is_empty() {
            self.gpu.queue.write_buffer(&self.ubuf, 0, &bytes);
        }
        let ibytes: &[u8] = bytemuck::cast_slice(&fr.instances);
        if ibytes.len() as u64 > self.icap {
            self.icap = (ibytes.len() as u64).next_power_of_two();
            self.ibuf.destroy();
            self.ibuf = crate::instance_buffer(&self.gpu.device, self.icap);
        }
        if !ibytes.is_empty() {
            self.gpu.queue.write_buffer(&self.ibuf, 0, ibytes);
        }
        self.arena.upload(&self.gpu.device, &self.gpu.queue);
        self.pool.upload(&self.gpu.queue);
        self.kept.upload(&self.gpu.queue);
        self.upload_ramps();
        while self.layers.len() < fr.max_level {
            self.layers.push(self.gpu.layer_target(self.size));
        }

        let mut enc = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("datars-frame") });
        let (mut draws, mut passes, mut instances) = (0u32, 0u32, 0u32);
        let items = &fr.items;
        let mut i = 0;
        while i < items.len() {
            let Item::Begin { level, clear } = &items[i] else {
                i += 1;
                continue;
            };
            let (level, clear) = (*level, *clear);
            let end = items[i..].iter().position(|it| matches!(it, Item::End)).map_or(items.len(), |p| i + p);
            // Keep multisampled contents only if a later pass on this level loads them.
            let reloaded = items[end..].iter().find_map(|it| match it {
                Item::Begin { level: l, clear: c } if *l == level => Some(c.is_none()),
                _ => None,
            });
            let last = !items[end..].iter().any(|it| matches!(it, Item::Begin { .. }));
            let (msaa, resolve) = if level == 0 {
                (self.root_msaa.as_ref(), view)
            } else {
                let t = &self.layers[level - 1];
                (t.msaa.as_ref(), &t.view)
            };
            let load = match clear {
                Some(c) => wgpu::LoadOp::Clear(c),
                None => wgpu::LoadOp::Load,
            };
            let color = match msaa {
                Some(ms) => wgpu::RenderPassColorAttachment {
                    view: ms,
                    resolve_target: Some(resolve),
                    depth_slice: None,
                    ops: wgpu::Operations { load, store: if reloaded == Some(true) { wgpu::StoreOp::Store } else { wgpu::StoreOp::Discard } },
                },
                None => wgpu::RenderPassColorAttachment { view: resolve, resolve_target: None, depth_slice: None, ops: wgpu::Operations { load, store: wgpu::StoreOp::Store } },
            };
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("datars-pass"),
                color_attachments: &[Some(color)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.stencil,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: Some(wgpu::Operations {
                        load: if passes == 0 { wgpu::LoadOp::Clear(0) } else { wgpu::LoadOp::Load },
                        store: if last { wgpu::StoreOp::Discard } else { wgpu::StoreOp::Store },
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            passes += 1;
            pass.set_bind_group(1, &self.ramp_bind, &[]);
            let (mut cur_pipe, mut cur_ref) = (None, None);
            let mut set = |pass: &mut wgpu::RenderPass, pipe: Pipe, stencil: u32, slot: u32| {
                if cur_pipe != Some(pipe) {
                    pass.set_pipeline(self.gpu.pipes.get(pipe));
                    cur_pipe = Some(pipe);
                }
                if cur_ref != Some(stencil) {
                    pass.set_stencil_reference(stencil);
                    cur_ref = Some(stencil);
                }
                pass.set_bind_group(0, &self.ubind, &[(slot as u64 * stride as u64) as u32]);
            };
            // The buffers the last mesh drew from: meshes in a row from one pool page or the arena
            // bind them once (on GL, rebinding re-specifies every vertex attribute).
            let mut bound: Option<(*const wgpu::Buffer, *const wgpu::Buffer)> = None;
            for it in &items[i + 1..end] {
                match it {
                    Item::Mesh { pipe, slot, mesh, stencil } => {
                        let found = match (&mesh.arena, &mesh.pool) {
                            (Some(span), _) => self.arena.buffers(span).map(|(vb, ib)| (vb, ib, span.first)),
                            (None, Some(span)) => self.pool.buffers(span).map(|(vb, ib)| (vb, ib, span.first)),
                            _ => None,
                        };
                        let Some((vb, ib, first)) = found else { continue };
                        set(&mut pass, *pipe, *stencil, *slot);
                        if bound != Some((vb as *const _, ib as *const _)) {
                            pass.set_vertex_buffer(0, vb.slice(..));
                            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                            bound = Some((vb as *const _, ib as *const _));
                        }
                        pass.draw_indexed(first..first + mesh.count, 0, 0..1);
                        draws += 1;
                    }
                    Item::Inst { pipe, slot, offset, count, stencil, kept } => {
                        let buf = match kept {
                            Some(span) => match self.kept.buffer(span) {
                                Some(b) => b,
                                None => continue,
                            },
                            None => &self.ibuf,
                        };
                        set(&mut pass, *pipe, *stencil, *slot);
                        bound = None;
                        let len = *count as u64 * std::mem::size_of::<InstanceV>() as u64;
                        pass.set_vertex_buffer(0, buf.slice(*offset..*offset + len));
                        pass.draw(0..6, 0..*count);
                        draws += 1;
                        instances += *count;
                    }
                    Item::Composite { pipe, slot, level } => {
                        let Some(bind) = self.layers[*level - 1].bind.as_ref() else { continue };
                        set(&mut pass, *pipe, 0, *slot);
                        pass.set_bind_group(1, bind, &[]);
                        pass.draw(0..3, 0..1);
                        pass.set_bind_group(1, &self.ramp_bind, &[]);
                        draws += 1;
                    }
                    Item::Begin { .. } | Item::End => {}
                }
            }
            drop(pass);
            i = end + 1;
        }
        self.gpu.queue.submit(Some(enc.finish()));
        self.stats.draws = draws;
        self.stats.passes = passes;
        self.stats.instances = instances;
        self.stats.skipped = fr.skipped;
        self.stats.instance_sets_kept = fr.kept_sets;
        self.stats.instances_rebuilt = fr.rebuilt;
        self.stats.stand_ins = self.stand_ins_now;
        self.stats.tessellated = self.tessellated_now;
        (self.stats.arena_meshes, self.stats.arena_bytes) = self.arena.size();
        Ok(())
    }

    /// Grow the ramp atlas if needed and upload new rows.
    fn upload_ramps(&mut self) {
        let needed = self.ramps.needed();
        let mut rows = std::mem::take(&mut self.ramps.dirty);
        if needed > self.ramp_cap {
            self.ramp_cap = needed.next_power_of_two();
            self.ramp_tex.destroy();
            (self.ramp_tex, self.ramp_bind) = self.gpu.ramp_atlas(self.ramp_cap);
            rows = self.ramps.all();
        }
        for row in rows {
            if let Some(data) = self.ramps.data(row) {
                self.gpu.queue.write_texture(
                    wgpu::TexelCopyTextureInfo { texture: &self.ramp_tex, mip_level: 0, origin: wgpu::Origin3d { x: 0, y: row, z: 0 }, aspect: wgpu::TextureAspect::All },
                    data,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(RAMP_W * 4), rows_per_image: Some(1) },
                    wgpu::Extent3d { width: RAMP_W, height: 1, depth_or_array_layers: 1 },
                );
            }
        }
    }
}

/// Undo the innermost clip (unbalanced pops, and clips past the stencil's range, do nothing).
fn pop_clip(fr: &mut Frame) {
    if let Some(Some((slot, mesh))) = fr.clips.pop() {
        if mesh.count > 0 {
            fr.items.push(Item::Mesh { pipe: Pipe::ClipPop, slot, mesh, stencil: fr.depth });
        }
        fr.depth -= 1;
    }
}

fn push_layer(fr: &mut Frame, opacity: f32, blend: Blend) {
    fr.items.push(Item::End);
    fr.level += 1;
    fr.max_level = fr.max_level.max(fr.level);
    fr.layers.push((opacity, blend));
    fr.begin(fr.level, Some(wgpu::Color::TRANSPARENT));
}

fn pop_layer(fr: &mut Frame) {
    let Some((opacity, blend)) = fr.layers.pop() else { return };
    fr.items.push(Item::End);
    fr.level -= 1;
    fr.begin(fr.level, None);
    if opacity <= 0.0 {
        return;
    }
    let mut u = DrawU { vp: fr.vp, ..DrawU::default() };
    u.misc[0] = opacity.min(1.0);
    let slot = fr.push_uniform(u);
    let level = fr.level + 1;
    let pipes: &[Pipe] = match blend {
        Blend::Normal => &[Pipe::CompNormal],
        Blend::Screen => &[Pipe::CompScreen],
        Blend::Multiply => &[Pipe::CompMulA, Pipe::CompMulB],
    };
    for &pipe in pipes {
        fr.items.push(Item::Composite { pipe, slot, level });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn big_paths_split_between_polygons_keeping_their_holes() {
        use datars_math::{PathData, PathEl};
        // 1,500 squares, each with a hole wound the other way: 15,000 elements.
        let mut p = PathData::new();
        for i in 0..1500 {
            let (x, y) = ((i % 50) as f64 * 10.0, (i / 50) as f64 * 10.0);
            p.move_to(Vec2::new(x, y));
            p.line_to(Vec2::new(x + 8.0, y));
            p.line_to(Vec2::new(x + 8.0, y + 8.0));
            p.line_to(Vec2::new(x, y + 8.0));
            p.close();
            p.move_to(Vec2::new(x + 2.0, y + 2.0));
            p.line_to(Vec2::new(x + 2.0, y + 6.0));
            p.line_to(Vec2::new(x + 6.0, y + 6.0));
            p.line_to(Vec2::new(x + 6.0, y + 2.0));
            p.close();
        }
        let chunks = split_path(&p, true).expect("split");
        assert!(chunks.len() >= 3, "{} chunks", chunks.len());
        assert_eq!(chunks.iter().map(|c| c.path.els.len()).sum::<usize>(), p.els.len(), "nothing lost");
        for c in &chunks {
            // Every chunk starts with an outer ring and holds whole squares (outer + hole).
            assert_eq!(c.path.els.iter().filter(|e| matches!(e, PathEl::Move { .. })).count() % 2, 0);
            assert!(matches!(c.path.els[1], PathEl::Line { p } if p.x > 0.0 || p.y == 0.0));
        }
        assert!(split_path(&PathData::rect(datars_math::Rect::new(0.0, 0.0, 1.0, 1.0)), true).is_none(), "short paths stay whole");
    }

    use super::*;

    #[test]
    fn buckets() {
        assert_eq!(tol_bucket(&Affine::scale(1.0, 1.0)), Some(0));
        assert_eq!(tol_bucket(&Affine::scale(3.0, 3.0)), Some(2));
        assert_eq!(tol_bucket(&Affine::scale(0.3, 5.0)), Some(3), "the larger stretch decides");
        assert_eq!(tol_bucket(&Affine::scale(0.0, 1.0)), None);
        assert!(tol_local(2) <= TOL_PX / 3.0);
        let w = 1.3;
        let b = bucket_width(width_bucket(w));
        assert!((b / w - 1.0).abs() < 0.1, "within a quarter octave: {b}");
        assert!((bucket_size(size_bucket(12.0)) / 12.0 - 1.0).abs() < 0.05);
        assert_eq!(stroke_width_local(0.5, 1.0, 1.0), (1.0, 0.5), "hairlines: one pixel at half alpha");
        assert_eq!(stroke_width_local(4.0, 2.0, 2.0), (2.0, 1.0));
        let r = Affine::rotate(0.7).mul(Affine::scale(2.0, 2.0));
        assert!((max_stretch(&r) - 2.0).abs() < 1e-12);
        assert!(is_similarity(&r.mul(Affine::translate(5.0, 1.0))));
        assert!(is_similarity(&Affine::scale(-3.0, 3.0)), "reflections keep strokes even");
        assert!(!is_similarity(&Affine::scale(10.0, 1.0)));
        assert!(!is_similarity(&Affine([1.0, 0.0, 0.5, 1.0, 0.0, 0.0])), "shear");
    }
}
