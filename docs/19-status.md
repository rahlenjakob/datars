# 19 — Implementation status

What is built, how it is verified, and what is still open, measured against the roadmap's phases
([17](17-roadmap.md)). Updated as work lands; the tests named here are the evidence.

## How to verify everything

```sh
cargo test --workspace --release              # Rust: unit + integration tests (~1,000)
target/release/datars test                    # goldens: every example state, 64-sample sweeps of every transition, motion invariants
target/release/datars gpu                     # wgpu vs CPU reference on this machine's GPU (ΔE, solid-region check)
target/release/datars budgets                 # bundle and runtime sizes against budgets.json
bash scripts/build-wasm.sh && node --test packages/web/test/*.test.mjs   # wasm = goldens; publish → serve → open → republish
(cd apple/DatarsKit && swift test)            # macOS; the iOS simulator via xcodebuild after scripts/build-apple.sh
scripts/test-android.sh                       # C-ABI goldens on an Android emulator
node --experimental-websocket scripts/browser-check.mjs http://127.0.0.1:8787/ out/browser.png   # <datars-view> in Chrome
```

## Cross-platform determinism (P1) — verified

The same example states produce **the same pixel hashes** on macOS arm64 and x86_64 (the C-ABI golden
test, `crates/datars-ffi/tests/goldens.rs`), wasm32 in Node (`packages/web/test/determinism.test.mjs`),
the iOS simulator (DatarsKit `DeterminismTests`) and an Android arm64 emulator
(`scripts/test-android.sh`). The goldens are written by `datars test` on any machine. GPU output
matches the CPU reference perceptually for every state (`datars gpu`: mean ΔE ≤ 0.15, no
solid-difference regions). Bundle tiers are equivalent: T1 (baked scenes), T2 (pre-expanded) and T3
(source) give the golden scenes (`crates/datars-build/tests/tiers.rs`). Sessions replay bit-exactly
(`crates/datars-engine/tests/replay.rs`), including sessions recorded in the wasm runtime and
replayed natively.

## By phase

| Phase | State | Evidence and notes |
|---|---|---|
| **0 Determinism harness** | done | libm wrapper and clippy bans (`clippy.toml`); hashes and CPU pixels identical across five targets (above) |
| **1 Primitives, text, GPU, platforms** | done | Every node kind; `harfrust` shaping, our own line breaking; CPU, SVG, PDF and wgpu backends (WebGPU, optional WebGL2, Metal, Vulkan, GLES, DX12); hosts: web (`<datars-view>`), desktop (`datars-view`), iOS/macOS (DatarsKit), Android (JNI + Kotlin view, AAR) |
| **2 Motion and bundles** | done | Matchers (path, key, hierarchy, nearest), choreographies, routes, disc/resample morphs, motion rules; lines that run on grow along themselves and markers ride their point (`crates/datars-motion/tests/riders.rs`); 64-sample sweeps and invariants (flash, blink, pop, NaN, exact endpoints) on every transition; bundles and the loader in every host; **a chart published after the hosts were built plays in them and a republish updates them** (web: `packages/web/test/delivery.test.mjs`; Android: `scripts/test-android-sample.sh`; iOS: `scripts/test-ios-sample.sh`) |
| **3 Data, expressions, interaction** | done | Typed keyed tables, vectorized expressions (bytecode, WGSL), scales, signals; intents: inspect, activate, brush (continuous and band), explore (drag pan, wheel and pinch zoom), drag, chapters; **touch**: a tap inspects with a finger's reach and a tap on nothing clears it; **line charts show values anywhere along the line** (`crates/datars-engine/tests/hover.rs`); session record/replay; linked views by brushing (the **prices** example); `datars explain` and `datars data profile`. **Point pyramids** (`instances` with `lod`, `std/cloud`): tables of any size indexed into seeded density-preserving levels, published as archives read by range — the **galaxy** example (4,000,000 stars, `crates/datars-engine/tests/points.rs`, `packages/web/test/points.test.mjs`) |
| **4 Recipes and the standard library** | done | QuickJS sandbox with deterministic `Math`; `@datars/sdk` and `@datars/std` (~60 recipes) with zero chart vocabulary in engine crates; every chart in [15](15-chart-coverage.md) as a recipe, including **financial charts** (`examples/stocks`: candlesticks and OHLC on a trading-day axis, volume, SMA/EMA, Bollinger bands, indexed and drawdown comparisons, sparklines). `datars test` also fails data marks that cross the canvas edge and labels a reader can't read (overlapping, or cut off). `datars eject` round-trips. Open: a pointer-following crosshair |
| **5 Programs** | done | Statechart runtime (states, edges, chapters); drivers: steps and keys, autoplay (holds, pause, `wake_at` so hosts sleep), scroll scrub and scroll triggers (verified in Chrome), live sources (the **election** example replays a feed through `datars serve`); films in any aspect ratio (`datars video`, WebVTT captions); an image server (`datars serve` → `/render/<alias>.png\|svg\|pdf`); engine-drawn controls, offered as native controls (`<input type=range>` in the web mirror, adjustable VoiceOver and TalkBack elements) |
| **6 Geo** | done | Projections, atlases, choropleths, camera fit, map ↔ chart morphs (renewables); **vector tiles** from our own OSM-derived PMTiles (sans-IO range requests, per-frame fill from the camera, label placement, crossfades that never show bare background while zooming either way — `crates/datars-engine/src/tiles/`), van Wijk flights (the **descent** example, world → Stockholm, streamed by HTTP Range in the browser, on Android, in DatarsKit and headless); **automatic basemaps** (`tiles: "auto"`, [09](09-geo.md#automatic-basemaps-tiles-auto)): the build cuts an archive to the document's own cameras from open data cached locally (`examples/rio`: world → Copacabana, 1.2 MB). GPU output matches the CPU reference down to street level (`datars gpu descent`). Open: a cell source fit for batch builds (public Overpass instances are for authors, within a daily budget) |
| **7 Publish compiler, agents, 1.0** | in progress | `datars build/bundle/publish/serve` with tiers and a cost model, shared data chunks, simplified accessible SVG posters, gzip size reports (`--explain`), size budgets (`datars budgets`); **fonts travel with charts** (subsets per chart, licences in the manifest; no font compiled into the web runtime); `datars dev` morphs each edit in from what is on screen; the MCP server; video and PDF; reduced motion everywhere; **T3 pinned to the publisher's std/sdk** (content hashes): a runtime with another std plays T2 (`crates/datars-build/tests/tiers.rs`); **IR 1 as a JSON Schema** generated from the Rust types (`docs/reference/ir.schema.json`) with migrations (`datars migrate`). CI: `.github/workflows/ci.yml` (engine tests, goldens, JS packages). Open: packages on registries, the bundle format frozen as 1.0, signing exposed in the CLI |

## Notebooks (Python) — first version

`packages/python` (`import datars as dr`, [guide](guides/python.md)) builds documents from
DataFrames with the std recipes (the `dr.std` wrappers are generated from `datars describe --json`)
and shows them live in notebooks. Verified: pytest (no network; chart documents validate against
`ir.schema.json`, and `datars check` finds no diagnostics in any chart kind); in headless Chrome
(`tools/check_browser.py`): a standalone page with the runtime inlined, a notebook HTML output
loading the runtime by URL, and the anywidget front end (a click delivers the bar's data row to
Python, a state set from Python steps the chart); in JupyterLab 4 (`tools/jupyterlab_check.mjs`):
every chart a widget, a click reaching the kernel's handler; an `nbconvert --to html` export shows
the engine's SVG posters when the runtime can't load. Open: PyPI wheels (with the CLI inside), a
PyO3 module over `datars-headless`, brush and selection signals mirrored to Python.

## Sizes (gzipped, `datars budgets`)

| What | Now | Target ([12](12-delivery.md)) |
|---|---|---|
| Simple animated bar story (votes), first load | 24 KB (most of it the chart's Inter subsets) | ≤ 10 KB ✗ since fonts travel with each chart |
| Same, poster and accessible text only (the fallback tier) | 7.5 KB | — |
| World choropleth (renewables), first load | 116 KB (the countries atlas is most of it, shared by hash) | ≤ 150 KB for a national choropleth ✓ |
| Web runtime, core (WebGPU + CPU renderers; plays T0–T2, the default) | 1.40 MB | — |
| Web runtime, full (+ QuickJS for T3 and raw documents; loaded only when needed) | 1.72 MB | — |
| Web runtime with the WebGL2 fallback compiled in (a build option) | 2.04 MB | — |

Atlases and region files ship as **decimal topologies** (borders stored once, integer deltas, decoded
by dividing by 10^d so every coordinate is the double the GeoJSON parsed to): charts stay
bit-identical (`crates/datars-build/tests/tiers.rs`, `packages/web/test/atlas.test.mjs`). Posters use
whole-pixel coordinates and 1/250-em glyph outlines, and leave out marks that draw nothing (a line's
hover points). Serve the runtime with brotli (~25 % smaller than gzip). Next size work: wgpu backend
trimming, a compact binary scene format for T1, per-zoom-band atlases.

## Performance

`node scripts/bench-engine.mjs` (`datars profile --json` over a light chart and the heavy kinds,
release build, Apple M3 Pro, a headless GPU at devicePixelRatio 2; every transition stepped at
60 fps) — the numbers behind the site's performance page:

| Chart | Resolve a state (cold / warm) | Engine per frame, p95 | GPU raster per frame, p95 | Dropped frames |
|---|---|---|---|---|
| votes (8 bars → a pie) | 0.96 / 0.01 ms | 0.07 ms | 0.8 ms | 0 |
| renewables (242 regions, camera moves) | 8.6 / 0.02 ms | 0.16 ms | 2.2 ms | 0 |
| counties (3,100 regions, each its own colour) | 36 / 0.22 ms | 1.3 ms | 10.5 ms | 0 |
| scatter (20,000 moving points) | 18 / 0.01 ms | 0.7 ms | 2.4 ms | 0 |
| galaxy (4,000,000 points, level of detail) | 0.2 / 0.04 ms | 2.0 ms | 5.0 ms | 0 |
| descent (a street-level map flight) | 0.4 / 0.03 ms | 0.5 ms | 3.8 ms | 0 |

Frames are cheap because plans are precomputed (P7): a frame evaluates the plan, nothing else.
Retained meshes live in shared GPU slab pages, and each text run is one mesh, so a frame is a handful
of draws. On phones the web runtime and the native hosts measure their own frames and scale their
work share down while a transition runs. In the browser, `scripts/perf-browser.mjs` steps every chart
on a page (stall, fps, dropped frames, the slowest frame's breakdown); on devices,
`scripts/bench-native.mjs` does the same through the native hosts. Open: GPU-side instance
interpolation for million-point morphs (CPU today, ~17 ms at 10⁶), incremental resolve through
`datars-graph`, draw batching for choropleths.

## Known gaps

- **One published bundle, every host, no rebuilds** — done: the descent from `datars publish` +
  `datars serve` plays in a web page, the SwiftUI sample app, the Android sample app and the desktop
  viewer, tiles streamed by HTTP Range in each, and renders to PNG/SVG/PDF through the image server.
- **Devices:** the iOS library is verified on macOS and in the simulator, the Android library on an
  emulator (arm64 and x86_64 builds in the AAR). Neither has run on a physical phone yet.
- **Accessibility:** semantics with frames; the web mirror (ARIA list, live narration, native range
  inputs, buttons for interactive marks); iOS VoiceOver; macOS children; labelled SVG posters;
  reduced motion everywhere; Android TalkBack through an AccessibilityNodeProvider (the node tree
  checked on the emulator, not yet driven by TalkBack itself); desktop through AccessKit (tree and
  actions unit-tested, not yet driven by a screen reader).
- **Selectable chart text (web):** done — transparent text spans over the canvas (select, copy,
  find-in-page); `aria-hidden`, since the semantics mirror is what screen readers read.
- **Tooltips:** hover on the web and desktop; tap on touch screens (web, iOS, Android), placed above
  the finger and dismissed by a tap elsewhere or a scroll.
- **Fonts:** open — WOFF2 input, subsetting CFF outlines, lazy script subsets in the Swift and Kotlin
  hosts, a shared default-family subset across the charts of a page.
