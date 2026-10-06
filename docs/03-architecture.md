# 03 — Architecture

## Layers

```
 authoring        TS SDK · Rust SDK · (Python) · visual editors · agents
                        │  build a Doc (the IR) + packages
                        ▼
 publishing       datars build: partial evaluation → bundle (manifest + content-addressed chunks,
                        │        variants per tier and runtime floor) → static storage / CDN
                        │  downloaded at runtime by URL — no host rebuild
                        ▼
 ┌─────────────────────────── datars engine (Rust, sans-IO, deterministic) ───────────────────────────┐
 │ open / load       verify bundle, pick variant, fetch chunks · or load source (dev)                 │
 │ check             schema, types, diagnostics (codes + paths + fixes)                               │
 │ graph             sources → transforms → scales → layouts → recipe expansion   (incremental)       │
 │ resolve           scene = keyed tree of primitives for the current signal state   (cached/state)   │
 │ motion            plan(prev, next) = match · choreograph · interpolate · route   (once/change)     │
 │ frame             evaluate clips + plans at t → concrete props (Rust / GPU)       (every frame)    │
 │ emit              display list · anchors · semantics delta · hit index · IO requests               │
 └─────────────────────────────────────────────────────────────────────────────────────────────────────┘
                        │
         backends       wgpu (WebGPU/WebGL2/Metal/Vulkan/DX12) · CPU reference · SVG · PDF
                        │
         hosts          web (wasm) · iOS · Android · desktop · headless (video/static/tests) · server
```

A **host** embeds the engine: it owns the surface, the clock, input, IO, accessibility bridging and
native overlays. A **backend** draws display lists. The engine in the middle is the same code on
every platform. The host plus engine plus backends is the **runtime** an app or site installs once;
charts reach it later as **bundles** ([12](12-delivery.md)). Depending on the bundle variant, some
phases already ran at publish time: a playback (T1) bundle arrives with scenes and plans resolved, so
the device only runs *frame* and *emit*.

## The pipeline, and the two budgets

| Phase | Runs when | Budget | Can call user code? |
|---|---|---|---|
| **Load** | a doc or package changes | whatever it takes (async) | no |
| **Resolve** | a signal or data change affects structure | ~1–50 ms (a map of 3,000 counties: ~40–85 ms); ahead of time in idle slices where it can (the steps next to the one on screen) | yes: recipes, kernels (sandboxed) |
| **Plan** | the resolved scene changes (step, filter, data tick, resize) | ~1–10 ms | yes: custom matchers, routes, curve generators |
| **Frame** | every display refresh while something moves | a 60 Hz frame; the costly work is capped by counts, not measured time (point rows built, tile bytes decoded, map features styled, meshes tessellated), scaled to the device by the host (`set_work_scale`) | **no** — Rust bytecode or GPU only |

Continuous signals (hover, a brush while dragging, data time during playback) re-resolve the scene
for their new values, and the caches make that cheap: resolved scenes are kept per signal
signature (state path, viewport, every signal's value — returning to a hovered mark is a lookup),
derived tables that read no signal, layout box or scale are shared across resolves rather than
recomputed (a 400,000-dot table is built once per data it reads), and sandboxed recipe expansions
are cached by recipe, parameters and context. A re-resolve re-evaluates templates and layout, not
data pipelines. (`datars-graph`, a finer-grained incremental graph, exists as a crate but isn't
wired into the engine yet — see [06](06-data-and-reactivity.md).)

## Crates

Engine core — all deterministic, sans-IO, `#![forbid(unsafe_code)]` where possible:

| Crate | Responsibility | Notable deps |
|---|---|---|
| `datars-math` | Deterministic numerics (own libm wrapper), vectors, affine transforms, béziers, geometry predicates, seeded RNG, fixed hashing | `libm` |
| `datars-color` | sRGB/linear/OKLab/OKLCH, palettes, contrast (WCAG/APCA), colour-vision simulation | — |
| `datars-data` | Typed columnar keyed tables, transforms, scales, CSV/JSON/Arrow IPC readers (bytes in, tables out) | — |
| `datars-expr` | Expression language: parser, type checker, bytecode VM (vectorized over columns), WGSL codegen | — |
| `datars-graph` | Reactive incremental graph with versioned inputs and deterministic, demand-driven evaluation — built and tested, not yet used by the engine (which caches whole resolves instead, see above) | — |
| `datars-text` | Font loading, shaping, bidi, line breaking, rich-text layout, measurement, glyph outlines, number/date formatting, locales | shaping + font crates (see [11](11-rendering.md)) |
| `datars-geo` | CRS & projections, adaptive resampling, clipping, simplification, triangulation, GeoJSON/TopoJSON, MVT, PMTiles (sans-IO) | `geo` boolean ops (tile build only) |
| `datars-layout` | Box layout (rows, columns, stack, grid) for composition, label placement with priorities and collision | — (its own) |
| `datars-motion` | Interpolation traits, easings (bézier, spring, steps, LUT, expression), path morph strategies, routes, matchers, choreography, clips and timeline algebra | — |
| `datars-scene` | Scene nodes (a keyed tree of groups, views, shapes, texts, images, instance sets), semantics, anchors — the resolved scene's types; the IR is `datars-ir`, resolving and picking live in `datars-engine`, plans in `datars-motion` | — |
| `datars-render` | The display list (flat paint-order ops), flattening a scene into one (inks resolved late, big unchanged instance sets kept across frames); tessellation and the mesh cache belong to `datars-render-wgpu` | — (`lyon` in `datars-render-wgpu`) |
| `datars-sandbox` | The recipe and kernel host: QuickJS with `Math` patched to `datars-math` (WASM kernels are planned) | `rquickjs` |
| `datars-bundle` | The bundle format ([12](12-delivery.md)): manifest, chunks, variants, signatures; the sans-IO **loader** (variant selection by capability and policy, chunk verification) and the content-addressed cache policy | `ed25519` verification, `blake3` |
| `datars-engine` | The façade: `Engine` + the host contract below; resolve, tiles and point pyramids, picking, programs, sessions | — |
| `datars-runtime` | The installed runtime's core shared by every host: bundle loading, the status hosts read, frames into pixels | — |

Standard library:

| Crate / package | Responsibility |
|---|---|
| `datars-algo` (Rust) | Heavy algorithms as public `Transform`s: stack, dodge, bin, pie, treemap, pack, partition, sankey, beeswarm, parliament, waffle, calendar, force (seeded, fixed iterations), voronoi, contour, hexbin, `scatter_in` (points inside shapes), polylabel, geodesics |
| `@datars/std` (TypeScript) | Recipes: marks, guides (axes, legends, gridlines), charts, annotations, tooltips, controls, map styles, transition presets, program presets, themes |

Backends: `datars-render-wgpu` (WebGPU, WebGL2, Metal, Vulkan, DX12), `datars-render-cpu` (the
reference rasterizer: goldens, posters, the fallback), `datars-render-svg`, `datars-render-pdf`.

Hosts and bindings: `datars-host-web` (wasm-bindgen → `@datars/web`), `datars-host-native`
(a winit window on any wgpu surface target, AccessKit for accessibility; rendering into a host's
own wgpu device or texture is planned), `datars-ffi` (a C ABI with a hand-written header: the Swift
package `DatarsKit` and, through JNI shims, the Kotlin `datars-android` library wrap it),
`datars-headless` (frames, filmstrips, video, static export). The Python package
(`packages/python`) drives the web runtime as a notebook widget and the CLI for exports.

Tooling: `datars-cli` (the `datars` binary), `datars-build` (the publish compiler: document →
bundle),
`datars-geo-build` (OSM / Natural Earth → tiles and atlases),
`datars-test` (the visual and motion test harness), `datars-devtools` (the devtools protocol),
`datars-mcp`.

npm: `@datars/web` (the `<datars-view>` element and the wasm engines), `@datars/sdk`,
`@datars/std`, `@datars/compile` (doc.ts → document; lambda → `Expr`), `@datars/react`
(`<DatarsView>`), `@datars/vite` (import a `doc.ts` as a chart), `@datars/next` (the same for
Next.js).

### Dependency direction

```
math ─► color ─► data ─► expr ─► graph ─┐
  └──► text ──────────────────────────────┤
  └──► geo ───────────────────────────────┼─► layout ─► motion ─► scene ─► engine ─► hosts
                                          │                         ▲         ▲
                          render ◄────────┴─────────────────────────┘         │
                          render-{wgpu,cpu,svg} ◄──────────────────────────────┘
                          sandbox ─────────────────────────────────► engine
                          algo ─(implements public traits)──────────► engine
```

No crate below `engine` knows about hosts, platforms, hosted services, or any chart vocabulary.

## The host contract

The engine exposes one small, platform-neutral interface. Every host — browser, iOS, Android,
desktop, headless, test harness, devtools — drives it the same way:

```rust
pub struct Engine { /* … */ }

impl Engine {
    pub fn new(config: EngineConfig) -> Engine;                    // fonts, budgets, feature set
    pub fn load(&mut self, doc: Doc, packages: &[Package]) -> Diagnostics;   // source form (dev, T3)
    pub fn open(&mut self, manifest: Bytes, policy: HostPolicy) -> Result<Opened, BundleError>;
                                                                   // a downloaded bundle: verify, pick
                                                                   // a variant, request its chunks
    pub fn patch(&mut self, ops: &[PatchOp]) -> Diagnostics;       // editors, agents, hot reload
    pub fn provide(&mut self, id: RequestId, res: Resource);       // data, tile, font, asset bytes
    pub fn event(&mut self, ev: InputEvent);                       // pointer, key, scroll, gesture
    pub fn set_signal(&mut self, name: &str, v: SignalValue);      // host-bound controls, programs
    pub fn resize(&mut self, viewport: Viewport);                  // size, pixel ratio, size class
    pub fn frame(&mut self, now: Time) -> FrameOutput;
}

pub struct FrameOutput {
    pub display: DisplayListDelta,      // retained resources added/removed + this frame's ops
    pub anchors: Vec<Anchor>,           // keyed screen points for host overlays
    pub semantics: SemanticsDelta,      // → ARIA / UIAccessibility / AccessibilityNodeInfo / UIA
    pub requests: Vec<Request>,         // IO the host must fulfil (tiles, chunks, fonts)
    pub cursor: Cursor,
    pub next_wake: Wake,                // Now | At(t) | Idle — hosts render on demand
    pub hash: SceneHash,                // determinism + caching
}
```

Properties that matter:

- **Charts arrive as data.** In production a host calls `open` with a bundle's manifest; the loader
  asks for the chunks it needs through `requests`, from the shared content-addressed cache or the
  network. Hosts never compile charts in, so charts update without an app release or site deploy (P16).
- **Sans-IO.** `requests` are fulfilled by the host (native: mmap/filesystem/HTTP; web: `fetch` with
  `Range`; mobile: app bundle). No platform code in the engine.
- **On-demand rendering.** `next_wake` lets hosts sleep when nothing moves (battery on phones).
- **Replayable.** A recorded sequence of `load/patch/provide/event/set_signal/resize/frame` calls is
  an exact reproduction of a session (bug reports, tests, agent debugging).
- **Inspectable.** The devtools protocol ([14](14-devtools-and-agents.md)) is a view onto this same
  interface plus read-only queries (scene tree, graph, plans, provenance).

## Threading

The engine core is single-threaded and deterministic. Hosts may split it:

- **Web:** resolve + plan in a worker (engine instance #1), frame + render on the main thread or an
  `OffscreenCanvas` worker (engine instance #2) that receives resolved scenes and plans as
  transferable buffers. Results are identical because both are the same pure code.
- **Native / mobile:** resolve on a background thread, frame on the render thread.
- **Data-parallel work** (transforms over large tables, tile decoding, CPU rasterization) uses fixed
  chunking with ordered merges, so results don't depend on scheduling.

## Where things live — a checklist for new features

1. Is it a new *kind of drawable* or a new *kind of time/interpolation*? → engine (`scene`, `motion`,
   `render`), with a public trait if users should be able to add their own.
2. Is it an *algorithm over data* (a layout, a statistic)? → `datars-algo`, behind the public
   `Transform` trait.
3. Is it a *composition* (a chart, a guide, a map style, a transition style, a program)? →
   `@datars/std`, written with the public SDK.
4. Is it about *teams, hosting, editing UI, brand management*? → a hosted service built on the
   public API ([16](16-licensing.md)), not this repo.
5. Whatever it is: which tool shows it, which test covers it, which MCP tool exposes it? (P11)
