//! `datars-render-wgpu` — the GPU backend for display lists (docs/11-rendering.md).
//!
//! One renderer for every platform: WebGPU with a WebGL2 fallback in the browser
//! ([`Renderer::for_canvas`]), Metal / Vulkan / DX12 windows natively ([`Renderer::for_surface`]),
//! and an offscreen target with readback for tests, posters and native image export
//! ([`Renderer::headless`], [`Renderer::read_rgba`]).
//!
//! **Retained resources.** Every `SharedPath` is tessellated with lyon in its *local* coordinates
//! once and cached by content hash (fills and strokes separately; the stroke key includes a
//! quarter-octave bucket of the width in local units, cap, join, miter limit and dash pattern).
//! Glyph outlines from the [`GlyphSource`] are cached per font + glyph + size bucket. A frame is
//! then uniform writes, instance uploads and draw calls; meshes unseen for a while are evicted
//! ([`CachePolicy`]). Transforms (and the device pixel ratio) are applied in the vertex shader.
//!
//! **Precision.** Mesh vertices are stored relative to their path's centre and the translation
//! that puts them back is composed with the draw's transform in f64 on the CPU, so a map path far
//! from the origin under a huge zoom still lands within f32 precision on screen.
//!
//! **Fidelity to the CPU reference.** Colours are straight sRGB in the display list and the
//! target is a non-sRGB format, so blending happens in the same (non-linear) space as the CPU
//! rasterizer; blending is premultiplied; edges are 4× MSAA (paths, glyphs) or analytic coverage
//! (SDF instances). Translucent strokes blend each sample once (the stencil buffer counts), as
//! coverage rasterization does, instead of darkening where lyon's triangles overlap at joins.
//!
//! **Clips** count in the stencil buffer: a clip increments the samples it covers inside the
//! current clip, content draws where the count equals the clip depth, and popping decrements.
//! **Layers** render into an offscreen target and composite with opacity and Normal / Multiply /
//! Screen blending (Multiply as two fixed-function passes, exact for any backdrop alpha).
//!
//! **Instances** are one instanced draw per op: symbols as analytic SDFs (the polygon comes from
//! the same symbol geometry the CPU reference fills) and rects as per-axis box SDFs, with
//! per-instance colour, opacity and size and an optional outline. **Glyphs** are tessellated
//! outlines drawn as fills; a halo of width `w` is a `2w`-wide round-joined stroke underneath.
//!
//! ```no_run
//! # use datars_render_wgpu::{Renderer, NoGlyphs, DisplayList};
//! # fn frame(list: &DisplayList) -> Result<(), datars_render_wgpu::GpuError> {
//! let mut r = Renderer::headless(800, 600, 2.0)?; // physical px; CSS px × 2
//! r.render(list, &NoGlyphs)?;
//! let rgba = r.read_rgba(); // straight-alpha RGBA8, like the CPU reference's Pixmap
//! r.render(list, &NoGlyphs)?; // same list again: no tessellation, just uniforms and draws
//! # let _ = rgba; Ok(()) }
//! ```

mod cache;
mod frame;
mod gpu;
mod ramp;
mod tess;

pub use cache::CachePolicy;
pub use datars_render::{DisplayList, GlyphSource, NoGlyphs};
pub use tess::{dash_path, Mesh, StrokeSpec, Tessellators};
pub use wgpu;

use cache::{Lru, MeshKey};
use gpu::{ColorTarget, Gpu, GpuMesh};
use ramp::RampRows;

/// Why the GPU renderer couldn't do something.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuError {
    /// No adapter (GPU or software) is available — e.g. a CI machine without one.
    NoAdapter(String),
    /// The adapter refused to create a device.
    NoDevice(String),
    /// Creating, configuring or acquiring the window/canvas surface failed.
    Surface(String),
    /// Readback of an offscreen frame failed, or the renderer has no offscreen target.
    Readback(String),
    /// wgpu reported a validation error (a bug in this crate if it ever happens).
    Validation(String),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter(m) => write!(f, "no GPU adapter: {m}"),
            GpuError::NoDevice(m) => write!(f, "no GPU device: {m}"),
            GpuError::Surface(m) => write!(f, "surface: {m}"),
            GpuError::Readback(m) => write!(f, "readback: {m}"),
            GpuError::Validation(m) => write!(f, "wgpu validation: {m}"),
        }
    }
}

impl std::error::Error for GpuError {}

/// Counters for tests, devtools and budgets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Frames rendered.
    pub frame: u64,
    /// Meshes tessellated since creation (fills, strokes, glyphs, halos), and by kind in that
    /// order.
    pub tessellations: u64,
    pub tessellated_kinds: [u64; 4],
    /// Mesh lookups served from the cache since creation.
    pub cache_hits: u64,
    /// Meshes evicted since creation.
    pub evictions: u64,
    /// Meshes resident now, and their bytes.
    pub meshes: usize,
    pub mesh_bytes: u64,
    /// The pool pages retained meshes live in, and their GPU bytes.
    pub pool_pages: usize,
    pub pool_bytes: u64,
    /// Last frame: draw calls, instances drawn, render passes.
    pub draws: u32,
    pub instances: u32,
    pub passes: u32,
    /// Last frame: ops this backend can't draw yet (images).
    pub skipped: u32,
    /// Last frame: meshes drawn from the frame arena (first seen), and its bytes.
    pub arena_meshes: u32,
    pub arena_bytes: u64,
    /// Last frame: big instance sets drawn from buffers of their own (unchanged since an earlier
    /// frame: not rebuilt, not uploaded).
    pub instance_sets_kept: u32,
    /// Last frame: instances built on the CPU and uploaded (all but the kept sets'). Hundreds of
    /// thousands every frame of a transition is the cost a fast machine hides and a phone shows.
    pub instances_rebuilt: u32,
    /// Last frame: meshes tessellated (what a raster spike is usually made of).
    pub tessellated: u32,
    /// Last frame: meshes drawn with a stand-in (a neighbouring bucket's, a thin stroke's last) while
    /// the exact one waits for a frame's tessellation share. A settled frame with stand-ins isn't
    /// final yet: hosts draw again until this is 0.
    pub stand_ins: u32,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
enum Target {
    /// An offscreen texture, read back with `read_rgba`.
    Offscreen { tex: wgpu::Texture },
    /// A window or canvas.
    Surface { surface: wgpu::Surface<'static>, config: wgpu::SurfaceConfiguration },
}

/// Draws display lists on the GPU. See the crate docs.
pub struct Renderer {
    gpu: Gpu,
    target: Target,
    /// Physical pixels.
    size: (u32, u32),
    dpr: f64,
    /// The target stores sRGB-encoded texels (only when the platform offers no linear format):
    /// the shader then linearizes so the hardware's encode round-trips.
    srgb_target: bool,
    root_msaa: Option<wgpu::TextureView>,
    stencil: wgpu::TextureView,
    layers: Vec<ColorTarget>,
    meshes: Lru<MeshKey, GpuMesh>,
    /// Meshes first drawn last frame or this one (CPU-side), retained when drawn again.
    recent: indexmap::IndexMap<MeshKey, (Mesh, u64), cache::Fx>,
    /// The mesh each thin stroke's path was last built with, by path, cap, join and miter (see
    /// `Renderer::stand_in`).
    thin_last: std::collections::BTreeMap<u64, MeshKey>,
    /// The frame being drawn is part of motion ([`Renderer::set_moving`]).
    moving: bool,
    /// Big instance sets' uploaded instances, by their columns' identity (see `instances_op`), and
    /// the slab they're kept in.
    inst_bufs: std::collections::BTreeMap<[usize; 8], frame::InstBuf>,
    kept: gpu::Slab,
    /// Big paths split into chunks of whole polygons, by path hash (see `frame::CHUNK_ELEMENTS`).
    chunks: indexmap::IndexMap<u64, std::rc::Rc<[datars_render::SharedPath]>, cache::Fx>,
    /// The host's share of the per-frame tessellation and upload budgets ([`Renderer::set_work_scale`]).
    work: f64,
    /// This frame's meshes first seen (drawn from one arena), the retained ones' pages, and how
    /// many were retained this frame.
    arena: gpu::Arena,
    pool: gpu::Pool,
    promoted: u32,
    /// Meshes tessellated this frame (past `TESS_PER_FRAME`, cached neighbours stand in), and
    /// meshes a stand-in drew.
    tessellated_now: u32,
    /// Path elements tessellated this frame (see `frame::TESS_ELEMENTS_PER_FRAME`).
    tess_elements_now: usize,
    /// Text runs drawn, by their glyphs' hash, and the frame they were last drawn; runs joined
    /// into one mesh this frame (see `frame::RUNS_PER_FRAME`).
    runs_seen: indexmap::IndexMap<u64, u64, cache::Fx>,
    runs_now: u32,
    stand_ins_now: u32,
    /// Building meshes for a list that isn't drawn ([`Renderer::warm`]): straight into the cache.
    warming: bool,
    tess: Tessellators,
    ramps: RampRows,
    ramp_tex: wgpu::Texture,
    ramp_bind: wgpu::BindGroup,
    ramp_cap: u32,
    ubuf: wgpu::Buffer,
    ubind: wgpu::BindGroup,
    ucap: u64,
    ibuf: wgpu::Buffer,
    icap: u64,
    frame: u64,
    stats: Stats,
    policy: CachePolicy,
}

impl Renderer {
    fn build(gpu: Gpu, target: Target, size: (u32, u32), dpr: f64) -> Result<Renderer, GpuError> {
        let (ubuf, ubind) = gpu.uniform_buffer(256);
        let icap = 64 * 1024;
        let ibuf = instance_buffer(&gpu.device, icap);
        let ramp_cap = 16;
        let (ramp_tex, ramp_bind) = gpu.ramp_atlas(ramp_cap);
        let srgb_target = gpu.format.is_srgb();
        let size = clamp_size(size, &gpu.device);
        let stencil = stencil_view(&gpu, size);
        let root_msaa = gpu::msaa_view(&gpu.device, size, gpu.format, gpu.samples);
        let r = Renderer {
            gpu,
            target,
            size,
            dpr: sane_dpr(dpr),
            srgb_target,
            root_msaa,
            stencil,
            layers: Vec::new(),
            meshes: Lru::default(),
            recent: indexmap::IndexMap::default(),
            thin_last: std::collections::BTreeMap::new(),
            moving: false,
            inst_bufs: std::collections::BTreeMap::new(),
            kept: gpu::Slab::new("instances-kept", wgpu::BufferUsages::VERTEX),
            chunks: indexmap::IndexMap::default(),
            work: 1.0,
            arena: gpu::Arena::new(),
            pool: gpu::Pool::new(),
            promoted: 0,
            tessellated_now: 0,
            tess_elements_now: 0,
            runs_seen: indexmap::IndexMap::default(),
            runs_now: 0,
            stand_ins_now: 0,
            warming: false,
            tess: Tessellators::new(),
            ramps: RampRows::default(),
            ramp_tex,
            ramp_bind,
            ramp_cap,
            ubuf,
            ubind,
            ucap: 256,
            ibuf,
            icap,
            frame: 0,
            stats: Stats::default(),
            policy: CachePolicy::default(),
        };
        r.gpu.take_errors()?;
        Ok(r)
    }

    /// Resize the target (physical pixels) and set the device pixel ratio (display-list CSS px ×
    /// `dpr` = physical px).
    pub fn resize(&mut self, width: u32, height: u32, dpr: f64) {
        self.dpr = sane_dpr(dpr);
        let size = clamp_size((width, height), &self.gpu.device);
        if size == self.size {
            return;
        }
        self.size = size;
        match &mut self.target {
            Target::Offscreen { tex } => {
                tex.destroy();
                *tex = offscreen_texture(&self.gpu, size);
            }
            Target::Surface { surface, config } => {
                config.width = size.0;
                config.height = size.1;
                surface.configure(&self.gpu.device, config);
            }
        }
        self.stencil = stencil_view(&self.gpu, size);
        self.root_msaa = gpu::msaa_view(&self.gpu.device, size, self.gpu.format, self.gpu.samples);
        self.layers.clear();
    }

    /// Target size in physical pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }
    pub fn dpr(&self) -> f64 {
        self.dpr
    }
    /// The wgpu backend in use ("Metal", "Vulkan", "BrowserWebGpu", "Gl", …).
    pub fn backend(&self) -> &str {
        &self.gpu.backend
    }
    /// MSAA samples per pixel (4, or 1 where the platform can't).
    pub fn samples(&self) -> u32 {
        self.gpu.samples
    }
    pub fn stats(&self) -> Stats {
        let (pool_pages, pool_bytes) = self.pool.size();
        Stats { meshes: self.meshes.len(), mesh_bytes: self.meshes.bytes(), pool_pages, pool_bytes, ..self.stats }
    }
    pub fn set_cache_policy(&mut self, policy: CachePolicy) {
        self.policy = policy;
    }
    /// Drop every retained mesh (hosts call this on OS memory warnings); the next frame
    /// re-tessellates what it draws.
    pub fn clear_caches(&mut self) {
        self.meshes.clear();
        self.pool.clear();
        self.recent.clear();
        self.layers.clear();
    }
    /// The underlying device and queue (for hosts that share them, e.g. to render into their own
    /// textures).
    pub fn device(&self) -> (&wgpu::Device, &wgpu::Queue) {
        (&self.gpu.device, &self.gpu.queue)
    }

    /// Build and keep the meshes `list` would draw, without drawing it. A host warms the renderer
    /// with the scene the next step lands on while the reader reads (the engine prepares it), so
    /// the step's frames find what enters already tessellated and on the GPU. Returns how many
    /// meshes it built.
    pub fn warm(&mut self, list: &DisplayList, glyphs: &dyn GlyphSource) -> u64 {
        let before = self.stats.tessellations;
        self.warming = true;
        drop(self.plan(list, glyphs));
        self.warming = false;
        self.arena.clear();
        self.pool.upload(&self.gpu.queue);
        self.kept.upload(&self.gpu.queue);
        self.stats.tessellations - before
    }

    /// Whether the next frames are motion (a transition, a camera flight) — hosts pass the
    /// engine's `animating`. Moving, thin strokes keep the mesh their path was last drawn with
    /// instead of spending the frame's tessellation on widths gone by the next frame; settled
    /// frames (and [`Renderer::warm`]) build the exact ones.
    pub fn set_moving(&mut self, moving: bool) {
        self.moving = moving;
    }

    /// How much of the per-frame tessellation and buffer-upload budgets a frame may spend (0.1–1):
    /// a slower device's host hands out less, and new meshes arrive over more frames.
    pub fn set_work_scale(&mut self, scale: f64) {
        self.work = if scale.is_finite() { scale.clamp(0.1, 1.0) } else { 1.0 };
    }

    /// Draw `list` into the target (and present it, for window and canvas targets).
    pub fn render(&mut self, list: &DisplayList, glyphs: &dyn GlyphSource) -> Result<(), GpuError> {
        self.frame += 1;
        self.stats.frame = self.frame;
        let frame = self.plan(list, glyphs);
        let result = match &self.target {
            Target::Offscreen { tex } => {
                let view = tex.create_view(&Default::default());
                self.submit(frame, &view)
            }
            Target::Surface { .. } => self.render_surface(frame),
        };
        for m in self.meshes.evict(self.frame, &self.policy) {
            self.stats.evictions += 1;
            self.pool.release(&m);
        }
        // Only this frame's first sightings wait for the next frame; last frame's that weren't
        // drawn again (a morph's outlines) go.
        let f = self.frame;
        self.recent.retain(|_, (_, seen)| *seen == f);
        // Instance buffers not drawn for a couple of frames go (their sets changed or left).
        let kept = &mut self.kept;
        self.inst_bufs.retain(|_, b| {
            let keep = b.last + 2 >= f;
            if let (false, Some(span)) = (keep, b.kept) {
                kept.release(span);
            }
            keep
        });
        self.ramps.evict(self.frame, self.policy.max_unseen_frames);
        result.and_then(|_| self.gpu.take_errors())
    }

    fn render_surface(&mut self, frame: frame::Frame) -> Result<(), GpuError> {
        let Target::Surface { surface, config } = &self.target else { return Ok(()) };
        let mut acquired = surface.get_current_texture();
        if matches!(acquired, wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost) {
            surface.configure(&self.gpu.device, config);
            acquired = surface.get_current_texture();
        }
        let tex = match acquired {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return Ok(()), // skip this frame
            _ => return Err(GpuError::Surface("surface texture unavailable".into())),
        };
        let view = tex.texture.create_view(&wgpu::TextureViewDescriptor { format: Some(self.gpu.format), ..Default::default() });
        let r = self.submit(frame, &view);
        self.gpu.queue.present(tex);
        r
    }
}

// ---- native: headless and windows ------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
impl Renderer {
    /// An offscreen renderer of `width × height` physical pixels drawing display lists at `dpr`
    /// (CSS px × dpr = physical px). Errors with [`GpuError::NoAdapter`] where there is no GPU
    /// (honours `WGPU_BACKEND` / `WGPU_ADAPTER_NAME`).
    pub fn headless(width: u32, height: u32, dpr: f64) -> Result<Renderer, GpuError> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
            let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, None).await.map_err(|e| GpuError::NoAdapter(e.to_string()))?;
            let (device, queue) = request_device(&adapter).await?;
            let format = wgpu::TextureFormat::Rgba8Unorm;
            let samples = gpu::pick_samples(&adapter, format);
            let gpu = Gpu::new(device, queue, format, samples, format!("{:?}", adapter.get_info().backend))?;
            let size = clamp_size((width, height), &gpu.device);
            let tex = offscreen_texture(&gpu, size);
            Renderer::build(gpu, Target::Offscreen { tex }, size, dpr)
        })
    }

    /// A renderer on a native window (anything wgpu accepts as a surface target, e.g.
    /// `Arc<winit::window::Window>`), `width × height` physical pixels at `dpr`.
    pub fn for_surface(target: impl Into<wgpu::SurfaceTarget<'static>>, width: u32, height: u32, dpr: f64) -> Result<Renderer, GpuError> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
            let surface = instance.create_surface(target).map_err(|e| GpuError::Surface(e.to_string()))?;
            let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface)).await.map_err(|e| GpuError::NoAdapter(e.to_string()))?;
            let (device, queue) = request_device(&adapter).await?;
            Renderer::on_surface(adapter, device, queue, surface, (width, height), dpr)
        })
    }

    /// A renderer on a surface an app hands over as a raw pointer through the C ABI: a
    /// `CAMetalLayer` (iOS, macOS: `SurfaceTargetUnsafe::CoreAnimationLayer`) or an Android
    /// `ANativeWindow` (`SurfaceTargetUnsafe::RawHandle`), `width × height` physical pixels at `dpr`.
    ///
    /// One backend at a time, the platform's own first (Metal, Vulkan), then GL: a surface made
    /// for every backend at once can leave the window taken — on an Android emulator Vulkan's did,
    /// and GL's then failed (`EGL_BAD_ALLOC`). `WGPU_BACKEND` narrows it to the backends it names.
    ///
    /// # Safety
    /// The layer or window must stay valid until the renderer is dropped (hosts drop it before
    /// they release the surface: a view's teardown, Android's `surfaceDestroyed`).
    pub unsafe fn for_raw_surface(target: impl Fn() -> wgpu::SurfaceTargetUnsafe, width: u32, height: u32, dpr: f64) -> Result<Renderer, GpuError> {
        // SAFETY: as this function's.
        unsafe { Renderer::for_raw_surface_in(&[wgpu::Backends::PRIMARY, wgpu::Backends::GL], target, width, height, dpr) }
    }

    /// [`Renderer::for_raw_surface`], trying `order`'s backends in turn: GL first where a host
    /// knows the platform's own backend misbehaves (an Android emulator's Vulkan, gfxstream, aborts
    /// the process allocating memory on macOS hosts — nothing a caller can catch).
    ///
    /// # Safety
    /// As [`Renderer::for_raw_surface`].
    pub unsafe fn for_raw_surface_in(order: &[wgpu::Backends], target: impl Fn() -> wgpu::SurfaceTargetUnsafe, width: u32, height: u32, dpr: f64) -> Result<Renderer, GpuError> {
        let env = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
        let tries = if std::env::var_os("WGPU_BACKEND").is_some() { vec![env.backends] } else { order.to_vec() };
        let mut failed = GpuError::NoAdapter("no backend to try".into());
        for backends in tries {
            let made = pollster::block_on(async {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends, ..wgpu::InstanceDescriptor::new_without_display_handle_from_env() });
                // SAFETY: the caller keeps the surface's layer/window alive for the renderer's life.
                let surface = unsafe { instance.create_surface_unsafe(target()) }.map_err(|e| GpuError::Surface(e.to_string()))?;
                let adapter = wgpu::util::initialize_adapter_from_env_or_default(&instance, Some(&surface)).await.map_err(|e| GpuError::NoAdapter(e.to_string()))?;
                let (device, queue) = request_device(&adapter).await?;
                Renderer::on_surface(adapter, device, queue, surface, (width, height), dpr)
            });
            match made {
                Ok(r) => return Ok(r),
                Err(e) => failed = e,
            }
        }
        Err(failed)
    }

    /// Wait until the GPU has finished everything submitted (for timing a frame whole: `render`
    /// returns once the work is queued).
    pub fn finish(&self) -> Result<(), GpuError> {
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely()).map(|_| ()).map_err(|e| GpuError::Readback(e.to_string()))
    }

    /// The last rendered frame as straight-alpha RGBA8, row-major from the top-left (the CPU
    /// reference's `Pixmap.data` layout). Empty if this isn't a headless renderer or readback
    /// failed (see [`Renderer::try_read_rgba`]).
    pub fn read_rgba(&mut self) -> Vec<u8> {
        self.try_read_rgba().unwrap_or_default()
    }

    /// [`Renderer::read_rgba`] with the error.
    pub fn try_read_rgba(&mut self) -> Result<Vec<u8>, GpuError> {
        let Target::Offscreen { tex } = &self.target else { return Err(GpuError::Readback("not a headless renderer".into())) };
        let (w, h) = self.size;
        let unpadded = w * 4;
        let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let device = &self.gpu.device;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &readback, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.gpu.queue.submit(Some(enc.finish()));
        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv().map_err(|e| GpuError::Readback(e.to_string()))?.map_err(|e| GpuError::Readback(e.to_string()))?;
        let data = slice.get_mapped_range().map_err(|e| GpuError::Readback(format!("{e:?}")))?;
        let mut out = Vec::with_capacity((unpadded * h) as usize);
        for row in 0..h as usize {
            let s = row * padded as usize;
            out.extend_from_slice(&data[s..s + unpadded as usize]);
        }
        drop(data);
        readback.unmap();
        readback.destroy();
        unpremultiply(&mut out);
        self.gpu.take_errors()?;
        Ok(out)
    }
}

// ---- web: a <canvas> via WebGPU, WebGL2 fallback -----------------------------------------------

#[cfg(target_arch = "wasm32")]
impl Renderer {
    /// A renderer on `canvas` (its `width × height` attributes are the physical size), drawing
    /// display lists at `dpr`. Uses WebGPU where the browser can actually create an adapter and
    /// falls back to WebGL2 otherwise.
    pub async fn for_canvas(canvas: web_sys::HtmlCanvasElement, dpr: f64) -> Result<Renderer, GpuError> {
        let size = (canvas.width().max(1), canvas.height().max(1));
        // WebGPU when the browser has an adapter for it.
        let desc = wgpu::InstanceDescriptor { backends: wgpu::Backends::BROWSER_WEBGPU, ..wgpu::InstanceDescriptor::new_without_display_handle() };
        let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
        // The adapter and device before the surface: a WebGPU surface claims the canvas for good
        // (`getContext("webgpu")`), and a browser that offers WebGPU without an adapter (the iOS
        // simulator, a blocklisted GPU) would leave no context for the fallbacks — no chart at
        // all. On the web a WebGPU adapter doesn't depend on the surface.
        let webgpu = async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false, ..Default::default() })
                .await
                .map_err(|e| GpuError::NoAdapter(e.to_string()))?;
            let (device, queue) = request_device(&adapter).await?;
            Ok::<_, GpuError>((adapter, device, queue))
        };
        let err = match webgpu.await {
            Ok((adapter, device, queue)) => {
                let surface = instance.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| GpuError::Surface(e.to_string()))?;
                return Renderer::on_surface(adapter, device, queue, surface, size, dpr);
            }
            Err(e) => e,
        };
        // Else WebGL2 (builds with the `webgl` feature): Safari before iOS 26, browsers with WebGPU
        // off. Its context comes from the canvas, so the surface comes first here.
        #[cfg(feature = "webgl")]
        {
            let gl = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::GL, ..wgpu::InstanceDescriptor::new_without_display_handle() });
            let surface = gl.create_surface(wgpu::SurfaceTarget::Canvas(canvas)).map_err(|e| GpuError::Surface(e.to_string()))?;
            let adapter = gl
                .request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false, ..Default::default() })
                .await
                .map_err(|e| GpuError::NoAdapter(format!("{err}; WebGL2: {e}")))?;
            let (device, queue) = request_device(&adapter).await?;
            return Renderer::on_surface(adapter, device, queue, surface, size, dpr);
        }
        #[cfg(not(feature = "webgl"))]
        {
            let _ = canvas;
            Err(err)
        }
    }

}

impl Renderer {
    fn on_surface(adapter: wgpu::Adapter, device: wgpu::Device, queue: wgpu::Queue, surface: wgpu::Surface<'static>, size: (u32, u32), dpr: f64) -> Result<Renderer, GpuError> {
        let caps = surface.get_capabilities(&adapter);
        // A non-sRGB format so blending happens in sRGB space, like the CPU reference.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).or_else(|| caps.formats.first().copied()).ok_or_else(|| GpuError::Surface("surface supports no formats".into()))?;
        let alpha_mode = [wgpu::CompositeAlphaMode::PreMultiplied, wgpu::CompositeAlphaMode::Opaque]
            .into_iter()
            .find(|m| caps.alpha_modes.contains(m))
            .unwrap_or(caps.alpha_modes.first().copied().unwrap_or(wgpu::CompositeAlphaMode::Auto));
        let size = clamp_size(size, &device);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: Default::default(),
            width: size.0,
            height: size.1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        let samples = gpu::pick_samples(&adapter, format);
        let backend = format!("{:?}", adapter.get_info().backend);
        // The error sink first: a surface the adapter can't configure is an error, not a panic.
        let gpu = Gpu::new(device, queue, format, samples, backend)?;
        surface.configure(&gpu.device, &config);
        gpu.take_errors().map_err(|e| GpuError::Surface(e.to_string()))?;
        Renderer::build(gpu, Target::Surface { surface, config }, size, dpr)
    }
}

/// Everything this renderer uses fits the WebGL2 baseline (two bind groups, one dynamic uniform
/// buffer, four vertex attributes, no storage buffers), so that's what it asks for everywhere —
/// plus the adapter's real texture size and uniform alignment.
async fn request_device(adapter: &wgpu::Adapter) -> Result<(wgpu::Device, wgpu::Queue), GpuError> {
    let limits = adapter.limits();
    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("datars"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(limits.clone()).using_alignment(limits),
            ..Default::default()
        })
        .await
        .map_err(|e| GpuError::NoDevice(e.to_string()))
}

fn sane_dpr(dpr: f64) -> f64 {
    if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    }
}

fn clamp_size(size: (u32, u32), device: &wgpu::Device) -> (u32, u32) {
    let max = device.limits().max_texture_dimension_2d;
    (size.0.clamp(1, max), size.1.clamp(1, max))
}

fn offscreen_texture(gpu: &Gpu, size: (u32, u32)) -> wgpu::Texture {
    gpu::texture(&gpu.device, "offscreen", size, gpu.format, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC)
}

fn stencil_view(gpu: &Gpu, size: (u32, u32)) -> wgpu::TextureView {
    gpu::texture(&gpu.device, "stencil", size, gpu::STENCIL_FORMAT, gpu.samples, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default())
}

fn instance_buffer(device: &wgpu::Device, bytes: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some("instances"), size: bytes, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
}

/// Premultiplied → straight alpha, in place (the readback layout matches the CPU reference).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn unpremultiply(px: &mut [u8]) {
    for i in (0..px.len() / 4).map(|k| k * 4) {
        let a = px[i + 3] as u32;
        if a == 0 {
            px[i..i + 3].fill(0);
        } else if a < 255 {
            for c in &mut px[i..i + 3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiply_restores_straight_alpha() {
        let mut px = vec![64, 32, 0, 128, 10, 10, 10, 0, 1, 2, 3, 255];
        unpremultiply(&mut px);
        assert_eq!(px, vec![128, 64, 0, 128, 0, 0, 0, 0, 1, 2, 3, 255]);
    }
}
