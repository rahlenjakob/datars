//! Device-side state: pipelines, the per-draw uniform layout, and render targets.

use crate::ramp::RAMP_W;
use crate::GpuError;
use std::sync::{Arc, Mutex};

const SHADER: &str = include_str!("shader.wgsl");

/// The stencil (clip counting) buffer. Depth is unused; the combined format is the one every
/// backend, WebGL2 included, can multisample.
pub const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

/// One draw's uniforms (see `Draw` in shader.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DrawU {
    pub m: [f32; 4],
    pub t: [f32; 4],
    pub color: [f32; 4],
    pub grad: [f32; 4],
    /// Vertex space → gradient space (2×2, column-major); identity unless the mesh was built in
    /// device space.
    pub gm: [f32; 4],
    pub misc: [f32; 4],
    pub inst: [f32; 4],
    pub vp: [f32; 4],
    pub poly: [[f32; 4]; 6],
}

/// One instance (24 bytes): position relative to the op's centre, size (radius or w/h), straight
/// sRGB fill colour, opacity.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceV {
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub color: [u8; 4],
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pipe {
    Fill,
    Stroke,
    /// Translucent strokes: each sample blends once (stencil counts it), then…
    StrokeOnce,
    /// …the count is undone.
    StrokeRestore,
    ClipPush,
    ClipPop,
    Symbols,
    Rects,
    CompNormal,
    CompScreen,
    CompMulA,
    CompMulB,
}

pub struct Pipelines {
    pub fill: wgpu::RenderPipeline,
    pub stroke: wgpu::RenderPipeline,
    pub stroke_once: wgpu::RenderPipeline,
    pub stroke_restore: wgpu::RenderPipeline,
    pub clip_push: wgpu::RenderPipeline,
    pub clip_pop: wgpu::RenderPipeline,
    pub symbols: wgpu::RenderPipeline,
    pub rects: wgpu::RenderPipeline,
    pub comp_normal: wgpu::RenderPipeline,
    pub comp_screen: wgpu::RenderPipeline,
    pub comp_mul_a: wgpu::RenderPipeline,
    pub comp_mul_b: wgpu::RenderPipeline,
}

impl Pipelines {
    pub fn get(&self, p: Pipe) -> &wgpu::RenderPipeline {
        match p {
            Pipe::Fill => &self.fill,
            Pipe::Stroke => &self.stroke,
            Pipe::StrokeOnce => &self.stroke_once,
            Pipe::StrokeRestore => &self.stroke_restore,
            Pipe::ClipPush => &self.clip_push,
            Pipe::ClipPop => &self.clip_pop,
            Pipe::Symbols => &self.symbols,
            Pipe::Rects => &self.rects,
            Pipe::CompNormal => &self.comp_normal,
            Pipe::CompScreen => &self.comp_screen,
            Pipe::CompMulA => &self.comp_mul_a,
            Pipe::CompMulB => &self.comp_mul_b,
        }
    }
}

#[derive(Clone, Copy)]
enum Stencil {
    /// Draw where stencil == ref (inside every active clip).
    Test,
    /// …and count the sample.
    TestIncr,
    /// …and uncount it.
    TestDecr,
    Ignore,
}

fn stencil_state(s: Stencil) -> wgpu::DepthStencilState {
    let face = |compare, pass_op| wgpu::StencilFaceState { compare, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op };
    use wgpu::{CompareFunction as C, StencilOperation as O};
    let (f, write) = match s {
        Stencil::Test => (face(C::Equal, O::Keep), 0),
        Stencil::TestIncr => (face(C::Equal, O::IncrementClamp), 0xff),
        Stencil::TestDecr => (face(C::Equal, O::DecrementClamp), 0xff),
        Stencil::Ignore => (face(C::Always, O::Keep), 0),
    };
    wgpu::DepthStencilState {
        format: STENCIL_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(C::Always),
        stencil: wgpu::StencilState { front: f, back: f, read_mask: 0xff, write_mask: write },
        bias: wgpu::DepthBiasState::default(),
    }
}

const PREMUL: wgpu::BlendState = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;

fn blend(src: wgpu::BlendFactor, dst: wgpu::BlendFactor, asrc: wgpu::BlendFactor, adst: wgpu::BlendFactor) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent { src_factor: src, dst_factor: dst, operation: wgpu::BlendOperation::Add },
        alpha: wgpu::BlendComponent { src_factor: asrc, dst_factor: adst, operation: wgpu::BlendOperation::Add },
    }
}

pub struct Layouts {
    pub uniforms: wgpu::BindGroupLayout,
    pub texture: wgpu::BindGroupLayout,
}

pub fn layouts(device: &wgpu::Device) -> Layouts {
    let uniforms = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("datars-uniforms"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64),
            },
            count: None,
        }],
    });
    let texture = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("datars-texture"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    Layouts { uniforms, texture }
}

pub fn pipelines(device: &wgpu::Device, layouts: &Layouts, format: wgpu::TextureFormat, samples: u32) -> Pipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("datars"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("datars"),
        bind_group_layouts: &[Some(&layouts.uniforms), Some(&layouts.texture)],
        immediate_size: 0,
    });
    let fill_vb = [Some(wgpu::VertexBufferLayout { array_stride: 8, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x2] })];
    let stroke_vb = [Some(wgpu::VertexBufferLayout {
        array_stride: 16,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
    })];
    let inst_vb = [Some(wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<InstanceV>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Unorm8x4, 3 => Float32],
    })];
    let make = |label: &str, vs: &str, fs: &str, buffers: &[Option<wgpu::VertexBufferLayout>], st: Stencil, blend: Option<wgpu::BlendState>, color: bool| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), buffers, compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(fs),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: if color { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() },
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(stencil_state(st)),
            multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
            multiview_mask: None,
            cache: None,
        })
    };
    use wgpu::BlendFactor as F;
    Pipelines {
        fill: make("fill", "vs_fill", "fs_paint", &fill_vb, Stencil::Test, Some(PREMUL), true),
        stroke: make("stroke", "vs_stroke", "fs_paint", &stroke_vb, Stencil::Test, Some(PREMUL), true),
        stroke_once: make("stroke-once", "vs_stroke", "fs_paint", &stroke_vb, Stencil::TestIncr, Some(PREMUL), true),
        stroke_restore: make("stroke-restore", "vs_stroke", "fs_paint", &stroke_vb, Stencil::TestDecr, Some(PREMUL), false),
        clip_push: make("clip-push", "vs_fill", "fs_paint", &fill_vb, Stencil::TestIncr, Some(PREMUL), false),
        clip_pop: make("clip-pop", "vs_fill", "fs_paint", &fill_vb, Stencil::TestDecr, Some(PREMUL), false),
        symbols: make("symbols", "vs_symbol", "fs_symbol", &inst_vb, Stencil::Test, Some(PREMUL), true),
        rects: make("rects", "vs_rect", "fs_rect", &inst_vb, Stencil::Test, Some(PREMUL), true),
        comp_normal: make("composite", "vs_full", "fs_composite", &[], Stencil::Ignore, Some(PREMUL), true),
        comp_screen: make("composite-screen", "vs_full", "fs_composite", &[], Stencil::Ignore, Some(blend(F::One, F::OneMinusSrc, F::One, F::OneMinusSrcAlpha)), true),
        comp_mul_a: make("composite-multiply-a", "vs_full", "fs_composite_mul", &[], Stencil::Ignore, Some(blend(F::Zero, F::Src, F::Zero, F::One)), true),
        comp_mul_b: make("composite-multiply-b", "vs_full", "fs_composite", &[], Stencil::Ignore, Some(blend(F::OneMinusDstAlpha, F::One, F::One, F::OneMinusSrcAlpha)), true),
    }
}

/// A mesh on the GPU: retained in a [`Pool`] page, or a span of this frame's [`Arena`]. Cheap to
/// clone into a frame.
#[derive(Clone)]
pub struct GpuMesh {
    pub count: u32,
    pub centre: datars_math::Vec2,
    pub arena: Option<ArenaSpan>,
    pub pool: Option<PoolSpan>,
}

/// Where a mesh sits in the frame arena: its first index, and which vertex buffer (stroke
/// vertices carry normals) its indices point into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaSpan {
    pub first: u32,
    pub stroke: bool,
}

/// Where a retained mesh sits: its vertices' and indices' places in the [`Pool`], and its first
/// index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolSpan {
    pub verts: SlabSpan,
    pub idx: SlabSpan,
    pub stroke: bool,
    pub first: u32,
}

impl GpuMesh {
    pub fn empty(centre: datars_math::Vec2) -> GpuMesh {
        GpuMesh { count: 0, centre, arena: None, pool: None }
    }
}

/// Bytes of a slab page (data bigger than that gets a page to itself).
const PAGE_BYTES: u64 = 1 << 20;
/// Pages kept empty for reuse per slab; more are destroyed.
const SPARE_PAGES: usize = 2;

/// Where data sits in a [`Slab`]: its page (and the page's generation when it was put there — a
/// page recycled since holds other data) and byte offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlabSpan {
    pub page: u32,
    pub gen: u32,
    pub offset: u64,
}

/// GPU data that stays, sub-allocated from shared pages of one buffer usage instead of a buffer
/// of its own each. On OpenGL every buffer created or destroyed is a trip through the driver's
/// context lock — on the Android emulator a synchronous round trip to the host; retaining a map
/// flight's dozens of meshes a frame took 80 % of its frames — and WebGPU makes each buffer a
/// round trip to the GPU process. Here data appends to the current page and goes up in one write
/// per page a frame, and draws in a row from one page share its buffer. A page is recycled once
/// everything in it has been released.
pub struct Slab {
    label: &'static str,
    usage: wgpu::BufferUsages,
    pages: Vec<Option<SlabPage>>,
    current: Option<usize>,
}

struct SlabPage {
    buf: wgpu::Buffer,
    cap: u64,
    used: u64,
    live: u32,
    gen: u32,
    /// Appended since the last upload, from `pend_at`.
    pend: Vec<u8>,
    pend_at: u64,
}

impl Slab {
    pub fn new(label: &'static str, usage: wgpu::BufferUsages) -> Slab {
        Slab { label, usage, pages: Vec::new(), current: None }
    }

    /// Keep `bytes` (a multiple of 4); where they are.
    pub fn put(&mut self, device: &wgpu::Device, bytes: &[u8]) -> SlabSpan {
        let n = bytes.len() as u64;
        let fits = |p: &SlabPage| p.used + n <= p.cap;
        let at = match self.current.filter(|&i| self.pages[i].as_ref().is_some_and(fits)) {
            Some(i) => i,
            None => {
                // An empty page big enough, or a new one.
                let spare = self.pages.iter().position(|p| p.as_ref().is_some_and(|p| p.live == 0 && p.cap >= n));
                let i = spare.unwrap_or_else(|| {
                    let cap = n.next_power_of_two().max(PAGE_BYTES);
                    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some(self.label), size: cap, usage: self.usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
                    let page = SlabPage { buf, cap, used: 0, live: 0, gen: 0, pend: Vec::new(), pend_at: 0 };
                    match self.pages.iter().position(Option::is_none) {
                        Some(i) => {
                            self.pages[i] = Some(page);
                            i
                        }
                        None => {
                            self.pages.push(Some(page));
                            self.pages.len() - 1
                        }
                    }
                });
                self.current = Some(i);
                i
            }
        };
        let p = self.pages[at].as_mut().expect("a live page");
        if p.pend.is_empty() {
            p.pend_at = p.used;
        }
        let offset = p.used;
        p.pend.extend_from_slice(bytes);
        p.used += n;
        p.live += 1;
        SlabSpan { page: at as u32, gen: p.gen, offset }
    }

    /// Data is gone: its page is recycled once it holds nothing else.
    pub fn release(&mut self, span: SlabSpan) {
        let Some(p) = self.pages.get_mut(span.page as usize).and_then(Option::as_mut) else { return };
        if p.gen != span.gen || p.live == 0 {
            return;
        }
        p.live -= 1;
        if p.live > 0 {
            return;
        }
        p.gen += 1;
        p.used = 0;
        p.pend.clear();
        let spare = self.pages.iter().flatten().filter(|q| q.live == 0).count();
        if spare > SPARE_PAGES && self.current != Some(span.page as usize) {
            if let Some(p) = self.pages[span.page as usize].take() {
                p.buf.destroy();
            }
        }
    }

    /// Write what was appended since the last upload: one write per page.
    pub fn upload(&mut self, queue: &wgpu::Queue) {
        for p in self.pages.iter_mut().flatten().filter(|p| !p.pend.is_empty()) {
            queue.write_buffer(&p.buf, p.pend_at, &p.pend);
            p.pend.clear();
        }
    }

    /// The buffer a span is in, if its page still holds it.
    pub fn buffer(&self, span: &SlabSpan) -> Option<&wgpu::Buffer> {
        let p = self.pages.get(span.page as usize)?.as_ref()?;
        (p.gen == span.gen).then_some(&p.buf)
    }

    /// Pages and their bytes on the GPU.
    pub fn size(&self) -> (usize, u64) {
        self.pages.iter().flatten().fold((0, 0), |(n, b), p| (n + 1, b + p.cap))
    }

    /// Drop every page.
    pub fn clear(&mut self) {
        for p in self.pages.drain(..).flatten() {
            p.buf.destroy();
        }
        self.current = None;
    }
}

/// Retained meshes: fill vertices (`[x, y]`), stroke vertices (`[x, y, nx, ny]`) and indices in
/// [`Slab`]s. Indices are rebased onto their vertices' page (no base vertex: WebGL2 has none).
pub struct Pool {
    fill: Slab,
    stroke: Slab,
    idx: Slab,
}

impl Pool {
    pub fn new() -> Pool {
        Pool { fill: Slab::new("pool-fill", wgpu::BufferUsages::VERTEX), stroke: Slab::new("pool-stroke", wgpu::BufferUsages::VERTEX), idx: Slab::new("pool-index", wgpu::BufferUsages::INDEX) }
    }

    /// Keep `m` (`stroke`: `[x, y, nx, ny]` vertices, else `[x, y]`).
    pub fn put(&mut self, device: &wgpu::Device, m: &crate::tess::Mesh, stroke: bool) -> GpuMesh {
        if m.is_empty() {
            return GpuMesh::empty(m.centre);
        }
        let verts = if stroke { &mut self.stroke } else { &mut self.fill };
        let v = verts.put(device, bytemuck::cast_slice(&m.verts));
        let base = (v.offset / if stroke { 16 } else { 8 }) as u32;
        let rebased: Vec<u32> = m.indices.iter().map(|i| i + base).collect();
        let i = self.idx.put(device, bytemuck::cast_slice(&rebased));
        GpuMesh { count: m.indices.len() as u32, centre: m.centre, arena: None, pool: Some(PoolSpan { verts: v, idx: i, stroke, first: (i.offset / 4) as u32 }) }
    }

    /// A retained mesh is gone.
    pub fn release(&mut self, m: &GpuMesh) {
        if let Some(span) = m.pool {
            if span.stroke { &mut self.stroke } else { &mut self.fill }.release(span.verts);
            self.idx.release(span.idx);
        }
    }

    pub fn upload(&mut self, queue: &wgpu::Queue) {
        for s in [&mut self.fill, &mut self.stroke, &mut self.idx] {
            s.upload(queue);
        }
    }

    /// The buffers a span draws from (vertices, indices), if its pages still hold it.
    pub fn buffers(&self, span: &PoolSpan) -> Option<(&wgpu::Buffer, &wgpu::Buffer)> {
        let v = if span.stroke { &self.stroke } else { &self.fill }.buffer(&span.verts)?;
        Some((v, self.idx.buffer(&span.idx)?))
    }

    /// Pages and their bytes on the GPU.
    pub fn size(&self) -> (usize, u64) {
        [&self.fill, &self.stroke, &self.idx].iter().map(|s| s.size()).fold((0, 0), |(n, b), (m, c)| (n + m, b + c))
    }

    pub fn clear(&mut self) {
        for s in [&mut self.fill, &mut self.stroke, &mut self.idx] {
            s.clear();
        }
    }
}

impl Default for Pool {
    fn default() -> Self {
        Pool::new()
    }
}

/// Meshes drawn this frame without buffers of their own: everything first seen this frame (a
/// morphing outline is new every frame, and a transition can morph thousands). Their vertices and
/// indices go into three shared buffers written once per frame, instead of two new GPU buffers
/// per mesh — on WebGPU each buffer is a round trip to the GPU process. Indices are rebased onto
/// the shared vertex buffers here, so draws need no base vertex (WebGL2 has none).
pub struct Arena {
    fill: Vec<f32>,
    stroke: Vec<f32>,
    idx: Vec<u32>,
    meshes: u32,
    bufs: Option<[(wgpu::Buffer, u64); 3]>,
}

impl Arena {
    pub fn new() -> Arena {
        Arena { fill: Vec::new(), stroke: Vec::new(), idx: Vec::new(), meshes: 0, bufs: None }
    }
    pub fn clear(&mut self) {
        self.fill.clear();
        self.stroke.clear();
        self.idx.clear();
        self.meshes = 0;
    }
    /// Add a mesh (`stroke`: `[x, y, nx, ny]` vertices, else `[x, y]`).
    pub fn push(&mut self, m: &crate::tess::Mesh, stroke: bool) -> GpuMesh {
        if m.is_empty() {
            return GpuMesh::empty(m.centre);
        }
        self.meshes += 1;
        let (verts, floats) = if stroke { (&mut self.stroke, 4) } else { (&mut self.fill, 2) };
        let base = (verts.len() / floats) as u32;
        verts.extend_from_slice(&m.verts);
        let first = self.idx.len() as u32;
        self.idx.extend(m.indices.iter().map(|i| i + base));
        GpuMesh { count: m.indices.len() as u32, centre: m.centre, arena: Some(ArenaSpan { first, stroke }), pool: None }
    }
    /// Meshes and bytes this frame.
    pub fn size(&self) -> (u32, u64) {
        (self.meshes, ((self.fill.len() + self.stroke.len() + self.idx.len()) * 4) as u64)
    }
    /// Write this frame's contents, growing the buffers (to powers of two) when they don't fit.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.idx.is_empty() {
            return;
        }
        let make = |label: &str, bytes: u64, usage: wgpu::BufferUsages| {
            let size = bytes.next_power_of_two().max(64 * 1024);
            (device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), size)
        };
        let data: [&[u8]; 3] = [bytemuck::cast_slice(&self.fill), bytemuck::cast_slice(&self.stroke), bytemuck::cast_slice(&self.idx)];
        let usage = [wgpu::BufferUsages::VERTEX, wgpu::BufferUsages::VERTEX, wgpu::BufferUsages::INDEX];
        let labels = ["arena-fill", "arena-stroke", "arena-index"];
        let bufs = self.bufs.get_or_insert_with(|| [0, 1, 2].map(|k| make(labels[k], 1, usage[k])));
        for k in 0..3 {
            if data[k].len() as u64 > bufs[k].1 {
                bufs[k].0.destroy();
                bufs[k] = make(labels[k], data[k].len() as u64, usage[k]);
            }
            if !data[k].is_empty() {
                queue.write_buffer(&bufs[k].0, 0, data[k]);
            }
        }
    }
    /// The buffers a span draws from: (vertices, indices).
    pub fn buffers(&self, span: &ArenaSpan) -> Option<(&wgpu::Buffer, &wgpu::Buffer)> {
        let b = self.bufs.as_ref()?;
        Some((if span.stroke { &b[1].0 } else { &b[0].0 }, &b[2].0))
    }
}

impl Default for Arena {
    fn default() -> Self {
        Arena::new()
    }
}

/// A colour target: multisampled when MSAA is on, resolved into `resolve`.
pub struct ColorTarget {
    pub msaa: Option<wgpu::TextureView>,
    pub view: wgpu::TextureView,
    /// For compositing (layers only).
    pub bind: Option<wgpu::BindGroup>,
}

pub fn texture(device: &wgpu::Device, label: &str, size: (u32, u32), format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

pub fn msaa_view(device: &wgpu::Device, size: (u32, u32), format: wgpu::TextureFormat, samples: u32) -> Option<wgpu::TextureView> {
    (samples > 1).then(|| texture(device, "msaa", size, format, samples, wgpu::TextureUsages::RENDER_ATTACHMENT).create_view(&Default::default()))
}

/// Everything created from the device that doesn't depend on the target size.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub layouts: Layouts,
    pub pipes: Pipelines,
    pub sampler: wgpu::Sampler,
    pub format: wgpu::TextureFormat,
    pub samples: u32,
    pub ustride: u64,
    pub errors: Arc<Mutex<Vec<String>>>,
    pub backend: String,
}

impl Gpu {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue, format: wgpu::TextureFormat, samples: u32, backend: String) -> Result<Gpu, GpuError> {
        // Validation errors become `GpuError`s instead of panics (the default handler panics).
        let errors: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = errors.clone();
        device.on_uncaptured_error(Arc::new(move |e: wgpu::Error| {
            if let Ok(mut v) = sink.lock() {
                v.push(e.to_string());
            }
        }));
        let layouts = layouts(&device);
        let pipes = pipelines(&device, &layouts, format, samples);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ramp"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let align = device.limits().min_uniform_buffer_offset_alignment.max(1) as u64;
        let ustride = (std::mem::size_of::<DrawU>() as u64).div_ceil(align) * align;
        let gpu = Gpu { device, queue, layouts, pipes, sampler, format, samples, ustride, errors, backend };
        gpu.take_errors()?;
        Ok(gpu)
    }

    /// Validation errors since the last call, as one `GpuError`.
    pub fn take_errors(&self) -> Result<(), GpuError> {
        let mut v = self.errors.lock().map_err(|_| GpuError::Validation("error sink poisoned".into()))?;
        if v.is_empty() {
            Ok(())
        } else {
            Err(GpuError::Validation(std::mem::take(&mut *v).join("\n")))
        }
    }

    pub fn texture_bind(&self, view: &wgpu::TextureView) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture"),
            layout: &self.layouts.texture,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        })
    }

    pub fn uniform_buffer(&self, slots: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: self.ustride * slots.max(1),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniforms"),
            layout: &self.layouts.uniforms,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer: &buf, offset: 0, size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64) }),
            }],
        });
        (buf, bind)
    }

    pub fn ramp_atlas(&self, rows: u32) -> (wgpu::Texture, wgpu::BindGroup) {
        let tex = texture(
            &self.device,
            "ramps",
            (RAMP_W, rows.max(1)),
            wgpu::TextureFormat::Rgba8Unorm,
            1,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        );
        let bind = self.texture_bind(&tex.create_view(&Default::default()));
        (tex, bind)
    }

    /// A layer target of the current size and format.
    pub fn layer_target(&self, size: (u32, u32)) -> ColorTarget {
        let resolve = texture(&self.device, "layer", size, self.format, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let view = resolve.create_view(&Default::default());
        let bind = Some(self.texture_bind(&view));
        ColorTarget { msaa: msaa_view(&self.device, size, self.format, self.samples), view, bind }
    }
}

/// 4× MSAA when the target and stencil formats both support it, else none.
pub fn pick_samples(adapter: &wgpu::Adapter, format: wgpu::TextureFormat) -> u32 {
    let ok = |f: wgpu::TextureFormat| {
        let feats = adapter.get_texture_format_features(f);
        feats.flags.sample_count_supported(4)
    };
    let resolve = adapter.get_texture_format_features(format).flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE);
    if ok(format) && ok(STENCIL_FORMAT) && resolve {
        4
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader must stay portable to the WebGL2 fallback: validate it with no optional
    /// capabilities and translate every entry point to GLSL ES 3.00 the way wgpu's GLES backend
    /// does (coordinate-space adjustment, one slot per resource kind).
    #[test]
    fn shader_translates_to_webgl2_glsl() {
        use naga::back::glsl;
        let module = naga::front::wgsl::parse_str(SHADER).expect("WGSL parses");
        let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .expect("validates with no optional capabilities");
        let mut binding_map = glsl::BindingMap::default();
        binding_map.insert(naga::ResourceBinding { group: 0, binding: 0 }, 0);
        binding_map.insert(naga::ResourceBinding { group: 1, binding: 0 }, 0);
        binding_map.insert(naga::ResourceBinding { group: 1, binding: 1 }, 0);
        let options = glsl::Options {
            version: glsl::Version::Embedded { version: 300, is_webgl: true },
            writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE | glsl::WriterFlags::FORCE_POINT_SIZE,
            binding_map,
            zero_initialize_workgroup_memory: true,
        };
        assert!(module.entry_points.len() >= 10);
        for ep in &module.entry_points {
            let po = glsl::PipelineOptions { shader_stage: ep.stage, entry_point: ep.name.clone(), multiview: None };
            let mut out = String::new();
            let mut w = glsl::Writer::new(&mut out, &module, &info, &options, &po, naga::proc::BoundsCheckPolicies::default())
                .unwrap_or_else(|e| panic!("{}: {e}", ep.name));
            w.write().unwrap_or_else(|e| panic!("{}: {e}", ep.name));
            assert!(out.starts_with("#version 300 es"), "{}", ep.name);
        }
    }

    #[test]
    fn uniform_block_fits_one_dynamic_offset_slot() {
        // 256 is the largest min_uniform_buffer_offset_alignment wgpu allows: one slot per draw.
        assert!(std::mem::size_of::<DrawU>() <= 256);
        assert_eq!(std::mem::size_of::<DrawU>() % 16, 0);
        assert_eq!(std::mem::size_of::<InstanceV>(), 24);
    }
}
