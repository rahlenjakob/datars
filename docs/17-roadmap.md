# 17 — Roadmap

datars was built bottom-up in phases, each ending with exit criteria that proved it before the next
layer went on top. The phases below are how the engine got here; [19](19-status.md) has the
evidence. The rest of the page is what comes next.

**Tools ship with each phase** (P11): a capability isn't done until the CLI, the dev tools and the
tests can see it.

## How it was built

| Phase | What it proved | Exit criteria | State |
|---|---|---|---|
| **0 — Determinism harness** | The foundation before any chart: libm wrapper, colour, keys, a minimal scene, the CPU reference renderer | A hand-built scene has the same scene hash and CPU pixels on every target, checked on every commit | done |
| **1 — Primitives, text, GPU, platforms** | Every node kind, engine-shaped text, the wgpu/SVG/CPU backends, thin hosts on web, desktop, iOS and Android | Identical line breaks everywhere; GPU vs CPU perceptual checks pass; size budgets set | done |
| **2 — Motion and bundles** | Matchers, choreographies, routes, path morphs, retargeting; bundles and the loader in every host | Motion invariants green on every transition; a chart published after the hosts were built plays in them, and a republish updates them | done |
| **3 — Data, expressions, interaction** | Typed keyed tables, vectorized expressions, scales, signals, intents, brushing, session replay, point pyramids | Linked views animate through plans; sessions replay bit-exactly | done |
| **4 — Recipes and the standard library** | The sandbox, `@datars/sdk`, `@datars/std` (~60 recipes), eject | Every chart in [15](15-chart-coverage.md) is a recipe; zero chart vocabulary in engine crates; std builds as a third-party package | done |
| **5 — Programs** | Statecharts: steps, scroll scrub and triggers, autoplay, live sources, films, an image server, engine-drawn controls | A scrolly story, a cross-filter dashboard, an election-night replay, a film in three aspect ratios — no program code in the renderer | done |
| **6 — Geo** | Projections, atlases, our own vector tiles, van Wijk flights, automatic basemaps | World → street flights on our own tiles, map ↔ chart morphs, no geo code in the scene or motion crates | done |
| **7 — Publish compiler, agents, 1.0** | Partial evaluation into tiers, fonts that travel, PDF/video, MCP, the IR schema and migrations | Budgets met; bundles open on every runtime since phase 2; the bundle format frozen as 1.0 | in progress |

## Next

Roughly in order; each item lands with its tests and tools.

**Toward 1.0**

- **Packages on registries:** the CLI on crates.io (and as prebuilt binaries), `@datars/*` on npm,
  wheels on PyPI with the CLI inside, the Swift package and the Android AAR on their registries.
- **Freeze the bundle format and IR 1.0** with a published support window for runtimes in the field.
- **Signing from the CLI** — `datars publish --sign` with keys the author holds (the manifest
  signature and host-side publisher pinning exist; the CLI doesn't expose a key yet).
- **Physical devices:** run the iOS and Android libraries in shipped apps on real phones, not only
  simulators and emulators; drive TalkBack and desktop screen readers for real.
- **CI on GitHub** for the whole matrix: engine, wasm, goldens, Apple and Android.

**Performance**

- GPU-side instance interpolation, for million-point morphs (instances interpolate on the CPU today:
  linear in n, ~17 ms at 10⁶).
- Incremental resolve through `datars-graph` (today a signal change re-resolves the scene, cached by
  signal signature).
- Draw batching for choropleths with thousands of regions, and splitting the first raster of very
  large meshes across frames on phones.

**Authoring**

- A pointer-following crosshair and hover marker for line charts.
- WOFF2 input and CFF-outline subsetting for fonts; lazy script subsets in the Swift and Kotlin hosts.
- A shared default-font subset across the charts of one page.
- More atlases (per-zoom-band detail) and a tile-cell source fit for batch builds.

## Risks

| Risk | Mitigation |
|---|---|
| IR churn breaks users | Schema versioning and migrations since the first public document format |
| QuickJS in the runtime is too slow or too big | Recipes are O(1) in rows; pre-expansion (T2) removes the sandbox from most bundles; heavy code as Rust transforms |
| Text stack size in wasm (shaping, fonts, CJK) | No font compiled into the web runtime; subsets per chart; per-script subsets loaded lazily |
| GPU output diverges from the CPU reference | Same formulas, perceptual tests on every example (`datars gpu`), a CPU fallback |
| Old app runtimes stay in the field for years | Runtime floors, variant fallbacks, a poster and accessible text always present, version-skew tests |
| A bad or hostile bundle hurts the host app | Hash-verified chunks, signed manifests with publisher pinning, no IO in the sandbox, budgets, fallback to the poster |
| OSM licensing for distributed tiles | Attribution drawn by the std recipe; share-alike noted for distributed archives ([16](16-licensing.md)) |
| The web runtime is paid once per site, not once per browser (cache partitioning) | Keep the core small; the poster tier is always instant |

## Decisions taken since the design

| Question | Decision |
|---|---|
| Text stack | `harfrust` shaping + `skrifa` outlines, our own line breaking and layout |
| Box layout | A small layout of our own in `datars-layout` (rows, columns, overlay stacks, grids; sizes in px, %, `auto` from measured text, or `fill`), no external crate |
| Tile schema | A minimal schema of our own (`kind` on roads, water, land, places), cut from OpenStreetMap and Natural Earth |
| Recipes on the web | QuickJS everywhere, with the engine's own libm — same output as native |
| Standard library in the runtime | Built in, and pinned by content hash: a bundle made with another std plays its pre-expanded tier instead |
| Python | A first version for notebooks, before 1.0 ([guide](guides/python.md)) |

## Open questions

1. **A tiny JS playback player** for baked (T1) content, for pages that can't afford the wasm core —
   only if it passes the same conformance tests.
2. **Where manifests are signed** for teams: with keys each author holds, in a shared key service, or
   both? Self-hosters need the first.
3. **Compact binary scenes** for T1, and per-zoom-band atlases, to shrink first loads further.
