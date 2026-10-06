# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Until 1.0, minor versions
may contain breaking changes to the IR, the SDK and the host APIs. These will be called out here.

## [Unreleased]

The first public version. This section summarises what exists today. The detailed, evidence-backed
status is in [docs/19-status.md](docs/19-status.md).

### Added

- **Engine.** A deterministic, sans-IO Rust engine that takes documents through tables, scenes and
  transitions to frames. It uses primitives (keyed shapes, text, instances, tiles, views; scales,
  coordinates, signals), never chart types. The same example states give the same pixel hashes on
  macOS (arm64, x86_64), in wasm, on the iOS simulator and on an Android emulator.
- **Document format (IR 1).** Versioned JSON with a generated JSON Schema
  (`docs/reference/ir.schema.json`, `datars schema`) and migrations (`datars migrate`).
- **Rendering.** A bit-exact CPU reference rasterizer, SVG and PDF writers, and a wgpu backend:
  WebGPU with a WebGL2 fallback in the browser, and Metal, Vulkan or GLES on iOS, macOS and
  Android. The GPU output is checked against the CPU reference (`datars gpu`).
- **Text.** Shaping (harfrust), bidi, line breaking, and locale-aware number and date formatting.
  Fonts travel with charts as subsets, with their licences recorded in the bundle.
- **Motion.** Keyed matching, choreography, path morphs, routes, easings and camera flights
  (van Wijk–Nuij). Transitions are pure and seekable, and reduced motion is honoured.
- **Data and interaction.** Typed, keyed columnar tables, transforms, scales and a vectorized
  expression language. Signals, and intents for inspect, activate, brush and explore (pan/zoom).
  Engine-drawn controls: slider, range, segmented, select (the platform's own picker on phones and
  tablets — web, iOS, Android), toggle, checklist and button, with hover states (`hover()`) and
  the cursor the engine asks for (a hand, grab, crosshair). Tooltips on hover and on tap (touch
  screens), with a line's value anywhere along it. Also
  cross-filtering dashboards, live sources, and session record and exact replay.
- **Programs.** A statechart runtime with steps, scroll scrub, scroll triggers, autoplay,
  engine-drawn controls and films.
- **Maps.** Projections, a countries atlas, choropleths, and vector tiles from our own
  OpenStreetMap/Natural Earth → PMTiles pipeline (`datars-geo-build`). Also automatic basemaps
  cut to a document's cameras (`tiles: "auto"`).
- **Big data.** Point pyramids (level of detail) for millions of points, streamed as point
  archives.
- **Standard library and SDK.** `@datars/sdk` (TypeScript authoring) and `@datars/std`: charts,
  guides, map styles, financial charts and program presets, written against the public SDK. They
  run in a deterministic QuickJS sandbox and can be ejected into a project (`datars eject`).
- **Delivery.** Signed, content-addressed bundles with T0–T3 variants chosen by a publish
  compiler (`datars bundle`, `datars publish`, `datars serve`), and size budgets
  (`datars budgets`).
- **Hosts.** `@datars/web` (`<datars-view>`), `@datars/react`, `@datars/vite`, `@datars/next` and
  `@datars/compile`. Also a desktop viewer (`datars-view`), a C ABI (`datars-ffi`) with a Swift
  package (`apple/DatarsKit`) and an Android library (`android/datars`), and a Python package for
  notebooks (`packages/python`). A page restyles charts it didn't publish with `setTokens`, font
  faces included: set before a chart opens, they are in its first frame (the website's "Make it
  yours" keeps a reader's look for every chart it shows).
- **Accessibility.** A generated semantics tree. It is mirrored to ARIA on the web and exposed to
  VoiceOver on iOS/macOS, to Android's accessibility services (an AccessibilityNodeProvider) and
  to AccessKit on the desktop. Also accessible SVG posters and selectable chart text on the web.
- **Tools.** `datars dev`, a live page that morphs each saved edit in, with preview widths and
  modes, lint findings with fixes, an explanation of any mark you click, and a profiler (live
  frames, each transition scored, the engine's profile re-run on every save). The `datars` CLI (`render`, `film`, `video`, `check`, `lint`, `inspect`,
  `explain`, `profile`, `diff`, `data profile`, `new`, `describe`, `docs`, …), the visual and
  motion test harness (`datars test`: scene snapshots, 64-sample transition sweeps, motion
  invariants, exact goldens), and an MCP server for coding agents (`datars-mcp`).

[Unreleased]: https://github.com/rahlenjakob/datars/commits/main
