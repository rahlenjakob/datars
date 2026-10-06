# 11 — Rendering and determinism

## The display list

The frame's output is a backend-neutral **display list over retained resources** — meshes uploaded
once, per-frame transforms and lookup tables — for everything the scene model can draw.

```rust
pub struct DisplayListDelta {
    pub added: Vec<(ResId, Resource)>,     // content-addressed: same bytes → same id
    pub removed: Vec<ResId>,
    pub ops: Vec<Op>,                      // this frame, in paint order
}
pub enum Resource {
    FillMesh(Mesh), StrokeMesh(Mesh), Path(PathData),       // tessellated or GPU-path geometry
    Instances(InstanceBuffers),                              // from/to columns + windows (GPU interpolation)
    GlyphRun(ShapedRun),                                     // positioned glyph ids + font ref
    Image(ImageRef), Lut(Vec<Rgba>), Gradient(Stops), Pattern(PatternDesc), Field(GridTexture),
}
pub enum Op {
    Draw { res: ResId, xf: Affine, paint: PaintParams, opacity: f32, blend: Blend },
    DrawInstances { res: ResId, xf: Affine, t: f32, paint: PaintParams },
    PushClip(ClipRef), PopClip, PushLayer { opacity: f32, blend: Blend, cache: Option<CacheKey> }, PopLayer,
}
```

- **Content-addressed resources** make per-frame deltas tiny (a settled chart re-emits only ops), let
  hosts cache across figures, and give every frame a stable hash.
- **Paint order** is z then tree order, deterministic.
- **Layers with cache keys** let backends cache static subtrees (a basemap tile, an axis) as textures.

## Geometry on the GPU

| Primitive | Technique |
|---|---|
| Circles, rounded rects, arcs/wedges, symbols | analytic SDF in the fragment shader, instanced — crisp at any zoom, cheap to morph (parameters only) |
| Polygons (regions, areas, ribbons) | tessellated once per shape (`lyon` or earcut), cached; in-flight morphs use LOD-reduced outlines |
| Strokes | screen-space extrusion in the vertex shader (width in px, zoom-stable), joins/caps/dashes |
| Large point sets | instanced quads; unchanged sets keep their buffer across frames (camera moves, fades as a per-draw alpha); GPU interpolation of from/to columns at uniform `t` is planned |
| Fields | data texture + colormap LUT, bilinear or nearest; contours as geometry |
| Text | glyph atlas (coverage, with an SDF path for large or transformed text), from engine-shaped runs |

A compute-based path renderer (Vello-style) is a later option for WebGPU and native; the display list
already carries `Path` resources so it can be added as a backend without changing the engine.

## Backends

| Backend | Where | Role |
|---|---|---|
| `datars-render-wgpu` | web (WebGPU, WebGL2), macOS/iOS (Metal), Android (Vulkan/GLES), Windows (DX12), Linux (Vulkan) | interactive rendering everywhere |
| `datars-render-cpu` | anywhere, including Linux CI and cloud agents | the **reference**: deterministic, bit-exact pixels; posters, server images, golden tests, video without a GPU |
| `datars-render-svg` | build time / anywhere | vector export, no-JS fallback, SEO; text as text with subset fonts, or as paths for exactness |
| `datars-render-pdf` (later) | anywhere | print and reports; tagged for accessibility |

The CPU reference rasterizer closes the biggest tooling gap a GPU-only renderer leaves: frames can
be rendered in any container, with no GPU and no browser, so tests and coding agents can always
*see* the output.
It uses analytic coverage anti-aliasing with a fixed sample pattern and integer accumulation, so its
output is identical on every CPU.

## Text

- `datars-text` does font parsing, shaping (a HarfBuzz-class shaper in Rust), bidi, line breaking
  (UAX #14), rich-text layout and measurement. Candidates: `rustybuzz`/`harfrust` + `skrifa`/`swash`,
  or `parley` on the same stack — chosen in the Phase 1 spike by correctness, wasm size and speed.
- **One layout everywhere:** the engine positions every glyph; backends only draw glyph runs. No
  backend measures text on its own (measuring in page JS, a Rust rasterizer and SVG separately
  gives three different layouts).
- Fonts are resources that travel with the chart: theme font tokens name their faces' sources
  (project files, URLs, Google Fonts; Inter is the default), the publish compiler ships subsets
  ([18](18-themes.md), [12](12-delivery.md)), and fallback chains are declared as data (a token's
  family stack; document fonts for other scripts join every chain).
- Numbers that animate re-format each frame; digit glyphs are pre-rasterized so counting is cheap.

## Determinism: the contract and how it's enforced

**Contract** (P1):

1. The **scene** and **display list** for given inputs are bit-identical on every target.
2. The **CPU reference** rasterizer's pixels are bit-identical on every target.
3. **GPU** output is perceptually equivalent to the CPU reference (`datars gpu`: thresholds on mean
   and p99 ΔE, on the share of visibly different pixels and on solid differences), not
   bit-identical — GPUs differ in precision and rasterization rules.

**Enforcement:**

| Source of nondeterminism | Rule |
|---|---|
| Transcendental math differs between platform libms | all engine math goes through `datars-math` (a pure-Rust libm); `f64::sin` & co. are banned by `clippy::disallowed_methods` |
| JS `Math.*` differs between QuickJS builds | QuickJS's `Math` is replaced with `datars-math` functions |
| Fused multiply-add / fast-math | never enabled; no `mul_add` unless via `datars-math` with a defined fallback |
| Hash-map iteration order | `std::collections::HashMap/HashSet` banned in engine crates (`disallowed_types`); ordered maps or fixed-seed hashers only |
| Parallel reductions | fixed chunking and ordered merges |
| Float sorting and NaN | total order everywhere; NaN canonicalized before hashing |
| Time, randomness, IO | none in the core (P10); seeded RNG only; the sandbox has no `Date`, `Math.random` or IO |
| Text | engine shaping with the fonts bundles carry; system fonts opt-in and marked per-platform |
| WASM kernels | NaN canonicalization; relaxed SIMD disabled |
| Compiler differences | the conformance suite runs every example on every target and compares scene hashes, so any leak is caught within a day |

## Size and startup

- The web engine ships as three builds, one of which each chart loads ([12](12-delivery.md)):
  **core** (everything the publish compiler pre-expanded, T0–T2: resolve, motion, tiles and point
  pyramids, the WebGPU and CPU renderers — nearly every bundle), **core-gl** (the same with the
  WebGL2 fallback, loaded only by browsers without a WebGPU adapter), and **full** (adds the QuickJS
  sandbox for T3 bundles and raw documents). The runtime asks for a WebGPU adapter as it loads and
  picks per chart; a finer split into modules (geo, shaping) is planned.
- One engine instance per page is shared by every figure on it.
- Byte budgets per module are tracked in CI; targets are set in the Phase 1 spike and not allowed to
  regress without a decision.
