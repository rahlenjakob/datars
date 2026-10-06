# datars — design

How datars works and why, for people working *on* it (contributors, maintainers, the curious). If
you want to *use* datars, start with the [documentation site](../site/docs/index.md) or the
[getting-started guide](guides/getting-started.md).

## The thesis in one paragraph

A visualization is a **pure function** of a document, its data, its signals, and time:
`frame = F(doc, data, signals, t)`. The engine — Rust, deterministic, sans-IO — evaluates that
function identically on the web, iOS, Android, desktop, in video and in static exports. It knows
**primitives** (keyed shapes, text, instances, fields, tiles, views; scales, coordinates,
transitions, signals), never chart types. Every chart, axis, legend, map style, transition style
and program (story, dashboard, live monitor, film) is a **recipe** in a standard library written
against the same public API a developer or coding agent uses — ejectable, editable, replaceable.
**Code builds the graph; the engine runs the graph**: authors work in real languages (TypeScript
first, Rust), the heavy lifting happens in Rust, and nothing user-written runs per frame. Apps and
sites **install the runtime once**; charts then arrive over the network as small, signed,
content-addressed **bundles** and can be added, updated or rolled back **without rebuilding the app or
site**. A **publish compiler** partially evaluates each document to decide what goes into a bundle —
a poster and accessible text always, then the least that still does the job for each runtime version
in the field.

## Five ideas

1. **Primitives, not chart types.** Few node kinds, typed animatable props, first-class keys, scales,
   coordinates and text. Chart types are recipes.
2. **Programs, not stories.** A story is one program (a linear statechart driven by scroll or
   steps). Dashboards, explorables, live monitors, films and loops are peers.
3. **Determinism you can test.** The same document gives the same scene and display list on every
   target, and the same pixels on the CPU reference — so a golden hash is a cross-platform test.
4. **Dogfooding.** The built-in charts, maps, axes and transition styles are ordinary recipes.
   If the standard library needs something the public API can't express, the API is wrong.
5. **Charts are content, not host code.** Apps and sites embed the runtime once; charts are
   downloaded bundles that update without an app release or a site deploy — and keep working on
   runtimes that are years old.

## Reading order

| # | Doc | What it settles |
|---|---|---|
| 01 | [background.md](01-background.md) | The lessons from an earlier prototype that shaped this design |
| 02 | [principles.md](02-principles.md) | The invariants every crate, recipe and tool must obey |
| 03 | [architecture.md](03-architecture.md) | Layers, crates, the pipeline, resolve vs frame, the host contract |
| 04 | [primitives.md](04-primitives.md) | Keys, values, nodes, geometry, paint, text, instances, coordinates, scales, semantics |
| 05 | [time-and-motion.md](05-time-and-motion.md) | Clocks, clips, transitions (matcher · choreography · interpolators · routes), custom everything |
| 06 | [data-and-reactivity.md](06-data-and-reactivity.md) | Typed keyed tables, transforms, reactivity (cached resolves, shared tables), signals, expressions, live data |
| 07 | [extensibility.md](07-extensibility.md) | Recipes, the control ladder, std as dogfood, eject, kernels, SDKs |
| 08 | [programs.md](08-programs.md) | Beyond stories: static, interactive, dashboard, story, live, film, loop — as statecharts |
| 09 | [geo.md](09-geo.md) | Maps as ordinary primitives; our own OSM → tiles pipeline; world-to-street zoom |
| 10 | [platforms.md](10-platforms.md) | Web, iOS, Android, desktop, video, static, server — one engine, thin hosts |
| 11 | [rendering.md](11-rendering.md) | Display list, backends (GPU, CPU reference, SVG/PDF), text, determinism enforcement |
| 12 | [delivery.md](12-delivery.md) | Install the runtime once, download charts as bundles; partial evaluation, tiers, version skew, security |
| 13 | [testing.md](13-testing.md) | Visual + motion testing: scene snapshots, dense transition capture, exact goldens, fast |
| 14 | [devtools-and-agents.md](14-devtools-and-agents.md) | CLI, inspector, provenance, lint, the agent interface (MCP) |
| 15 | [chart-coverage.md](15-chart-coverage.md) | The charts and features the standard library covers, and how each maps onto primitives |
| 16 | [licensing.md](16-licensing.md) | What's published (everything here), what may be hosted on top, licensing, stability |
| 17 | [roadmap.md](17-roadmap.md) | How it was built, phase by phase; what's next; risks; decisions; open questions |
| 18 | [themes.md](18-themes.md) | Custom themes: typed tokens, derived colours, modes, locks, checks, late-bound inks |
| 19 | [status.md](19-status.md) | What is built and how it is verified, phase by phase; sizes; performance; known gaps |

Crate contracts for parallel work: [dev/contracts.md](dev/contracts.md). Guides that the site also
publishes: [guides/](guides/). Generated references: [reference/std.md](reference/std.md) (every std
recipe's parameters) and [reference/ir.schema.json](reference/ir.schema.json) (the document format).

## The primitive set at a glance

| Layer | Primitives |
|---|---|
| Data | `Table` (typed, columnar, keyed) · `Column` · `Key` · `Transform` |
| Mapping | `Scale` · `Coord` (cartesian, polar, geo, planar, custom) · `Expr` |
| Visual | `Group` · `View` · `Shape` (`Geom` + `Paint` + `Stroke`) · `Text` · `Image` · `Instances` · `Field` · `Tiles` |
| Time | `Signal` · `Clip` · `Transition` = `Matcher` + `Choreography` + `Interpolator`s + `Route`s + `Easing` |
| Behavior | `Event` → `Intent` → `Binding` · `State` (statechart) |
| Meta | `Semantics` · `Anchor` · `HitRegion` · `Provenance` |

## Decisions log

| Decision | Choice | Rejected alternatives (why) |
|---|---|---|
| Engine language | Rust, deterministic, sans-IO core | Engines in C++ or TypeScript (Rust gives one safe core for wasm and native, and a deterministic toolchain) |
| Engine vocabulary | Primitives only; chart types live in the standard library | Closed chart-type enum (doesn't compose, every feature touches every path) |
| Authoring surface | The IR is the contract; **TypeScript SDK** first, Rust SDK second, Python later | Custom text DSL as the primary surface (no ecosystem, no types, no training data for agents; it grew into option explosions). A DSL can exist later as a product-level format. |
| Built-in charts | Recipes in TypeScript through the public SDK, running in the engine's deterministic JS sandbox; heavy algorithms as Rust transforms behind public traits | Built-ins as special-cased engine code (custom charts become second-class, and the extension API never becomes sufficient) |
| Per-frame work | Rust only: compiled expressions (CPU bytecode or WGSL), precomputed plans | User JS per frame (slow, non-portable) |
| User code sandbox | QuickJS with the engine's own libm for JS; WASM for speed | Host JS engines (V8/JSC disagree in the last bits of `Math.*`; no uniform sandbox) |
| Determinism | Bit-exact **scene and display list** on every target; bit-exact pixels on the CPU reference rasterizer; perceptual equivalence on GPUs | "Same pixels everywhere" (impossible across GPU vendors) |
| Text | Shaped and laid out by the engine with fonts that travel with the chart, on every target | Each target's own text machinery, e.g. Canvas2D measuring in page JS (different line breaks per target) |
| Programs | A small statechart runtime; story, dashboard, film, … are presets | A story-shaped root object |
| Rendering | wgpu (WebGPU/WebGL2/Metal/Vulkan/DX12) + CPU reference + SVG/PDF | GPU-only offline rendering (a headless GPU isn't available in Linux CI or a cloud agent's sandbox) |
| Maps | Ordinary primitives + a `Tiles` node; our own OSM/Natural Earth → PMTiles pipeline; tiles served from static storage or bundled | A separate map engine with a bridge to charts; third-party tile services at runtime |
| Delivery | An installed runtime (app SDK or one script tag) + downloadable, signed, content-addressed bundles with variants per tier and runtime version; charts and data update without rebuilding the host | Compiling charts into the app or site (every change needs a release); always shipping the full runtime + data |
| What goes into a bundle | Partial evaluation with a cost model: bake what's expensive to compute on a device, ship source for what's cheap and smaller to send | Bake everything (bigger bundles for simple charts) or compute everything on the device (slow on phones) |
| Dependencies | Curated: pure Rust, deterministic, wasm-friendly, permissive licenses, vetted with cargo-deny | Zero-dependency dogma (hand-rolled parsers; never hand-roll text shaping) |
| Licensing | Everything in this repository — engine, std, SDKs, CLI, dev tools, MCP, geo pipeline — is published as source, proprietary for now; hosted services build on the public API only ([16](16-licensing.md)) | Product features in engine types (caption-card styles and page CSS in the IR) |
| Themes | Typed tokens with colour expressions, modes, inheritance, locks and checks; scenes hold late-bound inks resolved per frame (animatable, host-overridable) | Fixed theme fields, colour remapping and page CSS |

## Status

Built and verified, phase by phase: [19-status.md](19-status.md). What's next: [17-roadmap.md](17-roadmap.md).

For coding agents: `../llms.txt` (a compact index) and `reference/std.md` (every std recipe's
parameters), both generated by `datars docs`.
