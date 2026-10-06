# 10 — Platforms: web, iOS, Android, desktop, video, static

Multi-platform is a first-order requirement, not a port. The strategy: **one engine, thin hosts, one
contract.** The engine is sans-IO, deterministic Rust behind the small host interface in
[03](03-architecture.md); each platform adds a host that owns the surface, clock, input, IO,
accessibility bridge and native overlays. A document authored once behaves the same everywhere —
same layout, same line breaks, same motion, same hit-testing — because everything that decides those
things runs in the engine. And it reaches every platform the same way: as one published bundle that
any installed runtime downloads, with no rebuild of the app or site that shows it.

## The matrix

| Platform | Host | GPU backend | Bindings / distribution | Accessibility bridge | Input |
|---|---|---|---|---|---|
| **Web** (desktop + mobile browsers) | `datars-host-web` | wgpu → WebGPU; browsers without a WebGPU adapter load a second engine build with wgpu → WebGL2; the CPU reference into a 2D canvas as the last resort | wasm-bindgen → `@datars/web`; `<datars-view>` web component; `@datars/react` (`<DatarsView>`), `@datars/vite`, `@datars/next` | parallel DOM tree with ARIA + data tables, generated from semantics; chart text as selectable DOM text (`text_layer`) | Pointer Events, keyboard, wheel, scroll position |
| **iOS / iPadOS / macOS** | `DatarsKit` (Swift package) over `datars-ffi` | wgpu → Metal on the view's `CAMetalLayer`; the CPU reference into a `CGImage` where there's no GPU | the C ABI (`datars-ffi`, `datars.h`) → Swift package; `DatarsView` for UIKit and AppKit, `DatarsChart` for SwiftUI | semantics → `UIAccessibilityElement`s (actionable items activate) | taps inspect what they land on (a tooltip above the finger) and activate it; clicks; drags and pinches not yet |
| **Android** | `datars-android` (Kotlin library) over `datars-ffi` | wgpu → Vulkan, else GLES, on the view's `TextureView` surface; the CPU reference into a `Bitmap` where there's no GPU | the C ABI through JNI shims → Kotlin library with prebuilt `.so`s (arm64-v8a, x86_64); `DatarsView` for Views (Jetpack Compose planned) | semantics → an `AccessibilityNodeProvider` (engine-drawn controls as range items) | `MotionEvent` (down, move, up) as the engine's pointer; taps inspect what they land on (a tooltip above the finger) and activate it |
| **Desktop** (macOS, Windows, Linux) | `datars-host-native` | wgpu → Metal / DX12 / Vulkan | crates.io; a winit window on any wgpu surface target (rendering into a host's own wgpu device or texture — egui, Bevy, Tauri — is planned) | AccessKit → NSAccessibility, UI Automation, AT-SPI | winit events |
| **Video** | `datars-headless` | CPU reference or wgpu offscreen | CLI + library; encoder adapters (ffmpeg process; VideoToolbox / MediaCodec on devices) | captions (WebVTT/SRT) and audio-description text from narration + semantics | none (film program) |
| **Static** | `datars-headless` | CPU reference (PNG), SVG, PDF | CLI + library | SVG with `<title>`/`<desc>` and ARIA roles; tagged PDF; alt text | none |
| **Server** | `datars-headless` in a service | CPU reference | container image | alt text | none |
| **Notebooks** (Jupyter, VS Code, Colab) | `packages/python` (`import datars`): the web runtime as a widget (runtime over the kernel connection) or as HTML over a static poster; exports through the CLI ([guide](guides/python.md)) | web | pip | as web | as web |
| **Other runtimes** (later) | the C ABI (`datars-ffi`, already what iOS and Android use) | as host | Flutter (FFI + texture), React Native (native modules over the iOS/Android hosts), Unity | via host | via host |

## Install once, deliver over the network

The matrix above is the **runtime** — what an app or site integrates once. Charts are not compiled
into it: they're **bundles** downloaded at runtime by URL ([12](12-delivery.md)), cached on the device,
and updated or rolled back without an app release or a site deploy (P16).

| Platform | Integrate once | Show a chart |
|---|---|---|
| Web | `@datars/web` from npm (with `@datars/react`, `@datars/vite` or `@datars/next` for those stacks), or the runtime folder served with the site; a CDN build pinned to a major version is planned | `<datars-view src="https://…/c/chart-id">`, or `<DatarsView chart={…}>` in React; an iframe/oEmbed URL for CMSs without scripts is planned |
| iOS / macOS | add the `DatarsKit` Swift package | `DatarsChart(source: .url(…))` in SwiftUI; `DatarsView()` then `load(.url(…))` in UIKit/AppKit |
| Android | add the `datars-android` library | `DatarsView`, then `load(url)` |
| Desktop / Rust | the `datars` crate with feature flags | `Engine::open(manifest)` through the native host |

Because native apps can't download native code, each app's runtime has a fixed **capability
profile** (which modules it compiled in, which runtime version it is). Bundles carry variants for
different profiles and runtime versions, and every bundle has a static, accessible fallback, so an
app shipped two years ago still shows today's charts — interactively where its runtime can, as a
poster with accessible text where it can't.

## What a host does (and nothing more)

1. **Surface:** create the canvas/layer/window or accept one; hand wgpu a surface; resize.
2. **Clock:** vsync-driven frames when `next_wake` asks for them; sleep otherwise; a virtual clock for
   video and tests.
3. **Input:** translate platform events into the engine's unified `InputEvent` (pointer id and kind,
   pressure, keys, wheel, pinch, scroll position). Gesture recognition happens in the engine so it
   behaves identically everywhere.
4. **IO:** fulfil the engine's requests — bundle manifests and chunks, data from declared endpoints,
   tiles and fonts — with `fetch` (and `Range`) on the web, the platform's HTTP stack in apps, and
   `mmap` for cached or app-shipped files; persist the content-addressed cache.
5. **Accessibility:** apply semantics deltas to the platform's accessibility API; forward assistive
   actions (activate, increment, scroll-to) back as intents.
6. **Overlays:** position native UI at the engine's anchors each frame (narration, rich tooltips,
   menus) when the app wants native chrome. The chart's words can be real text too: `text_layer`
   says where the settled frame drew every line of every text (box, baseline, size, rotation,
   font, role; whether a press there starts the chart's own drag). The web runtime lays
   transparent spans exactly there, in the chart's own font, so readers select and copy titles
   and labels, find-in-page finds them and crawlers read them; hover and clicks still reach the
   chart through them.
7. **Lifecycle:** pause when backgrounded, drop GPU caches on memory warnings, restore on resume.
8. **Preferences:** pass reduced motion, contrast, colour scheme, text size and locale as signals.

Everything else — layout, text, motion, picking, semantics — is the engine's.

## Interaction that works on every device

Documents bind **intents**, not mouse events ([04](04-primitives.md)): `inspect` is hover with a mouse,
tap-and-hold or tap on touch, focus with a keyboard, and "explore by touch" with a screen reader.
`activate` is click, tap, Enter or a double-tap with VoiceOver/TalkBack. `pan`/`zoom` are drag and
wheel, or one- and two-finger gestures. The engine finds what the pointer is on (a line anywhere
along it, its nearest point's value; a tap with a finger's reach) and hands the host its label:
the web runtime, DatarsKit and datars-android show it as a tooltip, beside the mouse or above the
finger, a tap's pinned until the next tap, a scroll or a step. Apps can show it their own way
(`onTap` on iOS and Android, the `pick` event on the web) or attach native UI through anchors.

## Text and fonts

- Fonts **travel with charts**: a theme's font tokens say where each face comes from (a project
  file, a URL, a Google Fonts family, or the default Inter), and bundles carry subsets of every
  face they draw with ([18](18-themes.md), [12](12-delivery.md)); `datars-text` shapes them on
  every platform. That is what makes line breaks, label collisions and layouts identical across
  web, iOS, Android and video.
- **System fonts** are available as an opt-in for apps that want a native look; the scene is then
  deterministic *per platform* rather than across platforms, and the conformance suite knows it.
- The platform's **text size preference** (Dynamic Type, Android font scale) is a signal; std recipes
  scale label sizes and re-lay out.
- The web runtime has no font compiled in (bundles bring theirs; raw documents fetch the default
  family from the runtime package's `fonts/`). Native runtimes compile Inter in by default
  (`bundled-fonts`) so their raw-document development mode draws text; apps that only play bundles
  can build without it.

## Responsive by design

`viewport` is a signal carrying size, pixel ratio, **size class** (`phone`, `tablet`, `wide`) and
orientation. Recipes lay out for the real size (never scale a fixed canvas) and choose
detail by size class. Rotating a phone or resizing a window re-resolves and **morphs** to the new
layout, because keys don't change.

## Performance and battery on mobile

- **On-demand rendering:** no frames while nothing moves; `next_wake` schedules the next one.
- **Uploads once, draws many:** unchanged instance sets keep their GPU buffers through camera
  moves and fades, so a transition re-uploads only what moves (GPU interpolation of moving sets
  is planned, [05](05-time-and-motion.md)). Phones render at 2× at most, and lower the resolution
  when the GPU is the bottleneck.
- **Memory budgets** for tile meshes, glyph atlases and scene caches, with LRU eviction and responses to
  OS memory warnings.
- **Thermal awareness:** hosts can lower the frame rate cap during long animations (the engine's output
  is a function of time, not of frame count, so motion stays correct at any rate).
- **A work share per device:** the engine's per-frame budgets (points built, tiles decoded and
  styled, meshes tessellated and uploaded) are counts sized for a desktop. The web runtime,
  `DatarsView` on Apple platforms and on Android hand the engine a share of them
  (`set_work_scale`, `datars_view_set_work_scale`): half to start, cut when a moving frame nearly
  misses its refresh, raised again when frames have room. A slow device gets the same detail over
  a few more frames instead of dropped ones.
- **Few GPU objects per frame:** retained meshes and kept instance sets live in shared pages
  (sub-allocated, recycled when empty), and a text run is one mesh. On OpenGL (older Android
  phones, the emulator, WebGL) each buffer created or destroyed goes through the driver's context
  lock, and each draw call costs: a map flight on the Android emulator went from 26–29 fps to 52–53.
- **Offline after first view:** downloaded bundles, data snapshots, fonts and PMTiles archives live in
  a content-addressed cache and are memory-mapped; apps can pin bundles for offline use, prefetch the
  ones they'll show next, and ship a `.datars` file as a first-run default.

## App-store realities

- Bundles are **content**: T0–T2 variants are pure data (scenes, plans, tables, geometry, bytecode for
  the engine's own expression VM), like images or Lottie/Rive files. They never contain native code.
- T3 variants add sandboxed QuickJS bytecode (an interpreter, no JIT). Apps that don't want downloaded
  interpreted code set `allowScript: false`; the compiler then provides pre-expanded variants for the
  size classes the app supports.
- Signed manifests and pinned publisher keys let an app accept charts only from its own publishers.
- Native binary size per platform and capability profile (core, + graph, + sandbox, + geo, + full
  shaping) is tracked in CI.

## Video and static, as first-class targets

- **Film** documents and stories render frame-exactly at any frame rate and resolution; aspect
  variants (16:9, 9:16, 1:1) come from the same document through the `viewport` signal, so a vertical
  video is the phone layout, not a crop.
- **On-device export** (planned): the same headless path inside the iOS/Android hosts, so an app can
  let a user share a chart as a video or image, encoded with the platform's hardware encoder.
- **Static exports** (PNG, SVG, PDF) are single frames of the same pipeline; SVG and PDF keep text as
  text where possible (accessible, searchable) and keep the semantic structure.

## Proving it: cross-platform conformance

Every example's sampled frames hash identically on native x86-64 (CI, Linux) and arm64 (macOS), in
wasm32 under Node (CI), and through the C ABI: the Swift package's tests on macOS (CI) and the
Android emulator (`scripts/test-android.sh`); the iOS simulator runs the sample app against a
published chart (`scripts/test-ios-sample.sh`). GPU output is checked for perceptual equivalence
against the CPU reference with `datars gpu` wherever a GPU exists (Metal on macOS today); a device
matrix (WebGPU and WebGL2 across Chrome, Safari and Firefox; Vulkan and GLES on Android; DX12 on
Windows) is planned. See [13](13-testing.md).
