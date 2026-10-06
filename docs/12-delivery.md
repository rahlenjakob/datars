# 12 — Delivery: an installed runtime, downloadable bundles

An app or a site integrates the **datars runtime once**. After that, charts arrive over the network
as **bundles** — small, content-addressed, signed, versioned — and can be added, updated or
rolled back **without rebuilding or redeploying the app or site** (P16). It's the Lottie/Rive model
(install a player, ship files), but the files are data-aware, interactive and accessible, and they
can carry live data and reader interaction.

The catalogue is broad — two dozen chart types, maps from the whole world down to a single street,
100,000-point scatters, 400,000-dot density maps — so what goes into a bundle is decided by a
compiler, with one principle (P14): **send the least that still does the job.**

## Three things, three update rates

| Thing | Installed / delivered | Changes when | Who ships it |
|---|---|---|---|
| **Runtime** | inside the app (Swift package, Android AAR, Rust crate) or on the page (one script from a CDN, or npm) | an app release; on the web, automatically within a pinned major version | the app/site team, rarely |
| **Bundle** | fetched over the network by URL or id; cached on the device | a chart is published or republished | the chart author (editor, CLI, agent) |
| **Data** | fetched by the runtime from declared endpoints, or provided by the host app | on the data's own schedule (live, snapshots) or per user | the data plane, or the app itself |

Decoupling the three is the point: a newsroom can publish tonight's election map into an app that
shipped in the spring; a bank app can show a new spending chart without an App Store review; a live
chart updates without republishing anything.

## The runtime

| Platform | How it's installed | Modules |
|---|---|---|
| Web | `@datars/web` from npm, or its runtime folder served with the site (`@datars/vite` and `@datars/compile` copy it, named by a hash of its content and served immutable); a CDN build pinned to a major version (`https://cdn…/datars/1/runtime.js`, integrity-checked) is planned | one of three engine builds per chart: **core** (T0–T2 bundles), **core-gl** (the same with WebGL2, for browsers without a WebGPU adapter), **full** (adds the sandbox: T3 and raw documents) |
| iOS / iPadOS / macOS | Swift package `DatarsKit` | compiled in; the recipe sandbox and the default fonts are build features apps can leave out |
| Android | the `datars-android` library | compiled in, as on iOS |
| Desktop / Rust | the `datars` crate with feature flags | compiled in |

Native apps can't download native code, so their **capability profile** is fixed at app build time;
bundles adapt to it (below). The runtime also carries **built-ins** that bundles reference by hash
instead of shipping: the standard library's bytecode for its version, and small common atlases
(world countries at low resolution). A bundle that references a built-in whose hash matches
downloads nothing for it. Fonts are not built-ins: every bundle carries subsets of the faces it
draws with (§Fonts), so a chart looks the same in a runtime from any year.

Inside the runtime, a **loader** fetches manifests and chunks (through the host's networking, P10),
verifies signatures and hashes, picks a variant, and fills a **content-addressed cache** shared by
every chart in the app (on the web: every chart on the origin). Chunks are immutable, so the cache is
trivially correct, works offline after the first load, and deduplicates across charts.

## The bundle

A bundle is a small **manifest** plus **chunks**, addressed by the manifest's hash:

```jsonc
{
  "format": 1,
  "id": "electricity-prices", "revision": "2026-09-26T09:14Z",
  "publisher": "ed25519:9f2c…", "signature": "…",
  "variants": [
    { "tier": "T2", "requires": { "runtime": ">=1.2", "modules": ["core", "graph"] }, "entry": "b3:71ac…" },
    { "tier": "T1", "requires": { "runtime": ">=1.0", "modules": ["core"] },          "entry": "b3:0d4e…" },
    { "tier": "T0", "requires": {},                                                   "entry": "b3:c9e1…" }
  ],
  "chunks": [
    { "hash": "b3:0d4e…", "kind": "scene", "state": "overview", "bytes": 3412 },
    { "hash": "b3:5a10…", "kind": "plan", "from": "overview", "to": "north", "bytes": 1180, "lazy": true },
    { "hash": "b3:ee02…", "kind": "font", "bytes": 9240, "meta": { "face": "Inter-400", "subset": "chart", "licence": "OFL-1.1" } },
    { "hash": "b3:91c7…", "kind": "font", "bytes": 21870, "lazy": true, "meta": { "face": "Inter-400", "part": "latin-ext", "unicodes": "U+0100-017F,…" } },
    { "hash": "b3:4471…", "kind": "atlas", "name": "se-kommuner@z6", "bytes": 212300, "shared": true }
  ],
  "data":   [ { "slot": "prices", "schema": "b3:…", "from": { "endpoint": "https://data…/prices", "every": "15m" } },
              { "slot": "household", "schema": "b3:…", "from": "host" } ],
  "fonts": [ { "family": "Inter", "weight": 400, "licence": "OFL-1.1", "from": "datars:fonts/Inter-Regular.ttf" } ],
  "tokens": { "themeable": ["accent", "ink", "paper", "font-body"] },
  "policy": { "network": ["https://data…/"], "budgets": { "memoryMB": 64, "resolveMs": 200 } }
}
```

- **Two packagings, one format:** loose chunks on a CDN (deduplicated across bundles; republishing
  downloads only changed chunks), or a single `.datars` file (manifest + chunks + an index,
  range-readable like PMTiles) for email-style distribution, shipping one default chart inside an app
  for offline-first use, or hosting on any static server.
- **Self-hosting:** `datars publish doc.ts --alias x --to site/` writes the loose layout (`c/<alias>`
  plus `chunks/<hash>`), which any static host serves. `datars serve` is a small server for it (and
  for a site built around it): whole responses gzipped to clients that accept it (the web
  runtime's wasm travels at a third of its size), byte ranges for tile and point archives, and
  chunks and hashed runtime folders marked immutable wherever they sit, while manifests stay
  revalidated.
- **Channels:** an embed can point at an immutable manifest hash (pinned forever) or at an **alias**
  (`/c/electricity-prices` → the latest manifest, short TTL, stale-while-revalidate). Publishing moves
  the alias; rolling back moves it back.
- **Nothing executable but sandboxed bytecode.** T0–T2 variants are pure data (scenes, plans,
  expression bytecode for the engine's own VM, tables, geometry). T3 adds QuickJS bytecode that runs
  in the sandbox. Never native code.

## Building a bundle: partial evaluation with a cost model

`datars build` takes a document, its packages and data, and a **target** — a runtime floor (e.g.
"runtimes ≥ 1.2"), capability profiles to support (e.g. "web full", "iOS without script"), and
tiers to emit — and partially evaluates the graph:

- **Must stay live** (residual): anything depending on a runtime signal — a reader-controlled filter,
  a brush, live data endpoints, host-provided data slots, the viewport beyond the size classes that
  were precomputed.
- **Could be baked** (static): everything else — data, transforms, scales, recipe expansions, layouts,
  shaping, triangulation, the scenes of every reachable state, the plans between them, hit indices.

"Could be baked" isn't "must be baked". For each static node the compiler weighs **bytes to send**
against **work on the device** and what the target can do:

| Case | Usually best | Why |
|---|---|---|
| A 20-row bar chart story | ship the **source** (doc + data, ~1–3 KB) and let the runtime expand it with its built-in std | smaller than baked scenes for 3 size classes × 6 states |
| A 20,000-dot beeswarm, a sankey, a force layout | ship the **result** | seconds of layout on a phone vs. tens of KB |
| County geometry | ship **pre-triangulated, quantized** meshes per zoom band, as shared chunks | no earcut on the device; cached across every chart using the atlas |
| Text | ship **pre-shaped runs** unless it changes at runtime | the runtime needs no shaping module |
| The runtime's std version differs from the author's | ship the pinned std bytecode (shared chunk) or the baked variant | output must match what the author saw (P1) |

The compiler reports every decision (`datars build --explain`): "`beeswarm` baked (18 KB, saves
~1.1 s on mid-range Android); `bars` shipped as source (1.4 KB); `selected` is live, so the graph
module is required."

## Tiers

| Tier | What the reader gets | Runtime needs | Typical use |
|---|---|---|---|
| **T0 Static** | poster + accessible HTML (or native text alternative) | nothing (a plain `<img>` and HTML work) | email, RSS, print, strict CMSs, runtimes too old or too small |
| **T1 Playback** | states, transitions, scrubbing, inspection, tooltips, camera flights | **core** | most story charts, animated figures |
| **T2 Reactive** | + filters, brushes, inputs, linked views, live data, host-provided data | + **graph** | dashboards, explorables, live monitors, personal data |
| **T3 Programmable** | + recipes or kernels that must run on the device | + **sandbox** | structure that depends on runtime data |

A bundle carries several variants; the runtime picks the best one it supports under the host's policy
(an app can forbid T3; a data-saver setting can prefer a lighter variant). Every bundle has a T0
variant, so a chart never fails to show *something* meaningful and accessible.

## Size targets (to validate in Phase 7)

| Chart | First load (compressed) | Next chart in the same app |
|---|---|---|
| Simple animated bar/line story, default fonts | ≤ 10 KB | ≤ 5 KB (shared chunks cached) |
| Dashboard over a 10k-row table with filters | ≤ 150 KB | data chunks only |
| National choropleth at municipality level (290 kommuner) | ≤ 150 KB | ≤ 20 KB (atlas cached) |
| US counties (3,100) scrolly with states and cities | ≤ 800 KB | ≤ 50 KB |
| World → street descent | ≤ 300 KB + basemap tiles streamed along the flight | tiles cached |

The web runtime's **core** module is paid once per origin (browsers partition caches by site, so not
across sites); on native it's part of the app.

## Chunks

| Chunk | Format | Notes |
|---|---|---|
| **Poster** per size class | SVG for charts (text as text); AVIF/WebP for dense maps | rendered by the same engine, so the live view replaces it without a visible change |
| **Accessible text** | HTML fragment (web) / structured text (native): summary, per-state alternatives, data tables (paginated for large data) | available before the runtime has done anything |
| **Source** | the IR subset for runtime-expanded parts + data | smallest form for simple charts |
| **Scenes** | structure-of-arrays binary, quantized coordinates, delta-encoded against the previous state, dictionary-coded keys; **token references kept late-bound** where the doc marks paints themeable | one per reachable state, loaded progressively |
| **Plans** | correspondence tables, windows, route polylines, easing LUTs; reduced-motion variants | one per traversable state pair, fetched ahead of need |
| **Glyph runs + font subsets** | pre-shaped runs; a subset of every face the chart draws with, cut to the characters it can show (§Fonts); per-script subsets loaded lazily for text that arrives at runtime | per chart; script subsets on demand |
| **Geometry / atlases** | pre-triangulated meshes per zoom band; values separate as lookup tables | shared across bundles by hash |
| **Basemap** | references to regional PMTiles archives on static storage (range-read), or a camera-path extract | never a third-party service |
| **Residual graph + data** | typed graph + columnar data chunks | only for live parts |
| **Package bytecode** | QuickJS bytecode for T3 recipes/kernels | shared across bundles by hash |

## Fonts

Text must measure and render identically everywhere (P1), so fonts travel with charts and no
runtime supplies its own — system fonts are never used, and the web runtime has no font compiled in
at all. Where faces come from is the theme's business: font tokens name a project file, a URL, a
Google Fonts family, or the default Inter ([18](18-themes.md#fonts)).

**Acquired at build time.** `datars bundle`/`publish`/`render`/`dev` read project files, download
Google Fonts (static TrueType instances from the css2 API) and font URLs once into a disk cache,
and never leave that to a runtime (`crates/datars-build/src/fonts/acquire.rs`; the network is the
system `curl` behind an `Http` trait, so tests run on fixtures).

**Subset to what the chart can show.** Each face ships as a subset (`fonts/subset.rs`, pure Rust
over read-fonts/write-fonts) covering the chart's text: every label of every baked state, the
strings in the document that can become labels (values and the literals in expressions — not
keys, tokens or code), every character of the data files the build fetched, and what number
formatting produces in the locale (digits, separators, signs; SI prefixes for `s` formats; month
and weekday names where dates are formatted). A face that only draws what the baked states show —
a title in a story without interaction — gets just those characters (and the digits a tween passes
through); the body face, document fonts and every face of an interactive chart get the whole set.
So does a face whose text changes with size: hosts resize views, and a label shown only when it
fits draws other names in a narrower column, so the build re-resolves every state at a few other
sizes (narrower, wider, other aspects) and a face that draws anything new there isn't cut down.
Glyph ids are **kept**, dropped glyphs just have no outline: baked T1 glyph runs stay valid, GSUB,
GPOS and GDEF keep their meaning, and a subset shapes exactly like the whole font for the text it
covers (the tier tests open every bundle in a runtime without fonts and compare pixels). The
subsetter keeps what the shaper's default features can reach (the engine asks for no others),
filters substitutions, pair kerning, mark attachment and glyph classes to the kept glyphs, drops
hinting, bitmap, colour and variation tables, and keeps the licence records. CFF fonts ship whole.

**Runtime text.** When text can arrive after publishing — a host data slot, a live feed, streamed
tiles, a data URL fetched at runtime — each face also ships one **lazily loaded** chunk per script
it covers (Latin, Latin Extended, Greek, Cyrillic, … as Google Fonts splits them; each part also
covers the chart's own characters, so a label shaped with it kerns as with the whole font). A
runtime that lays out a character no loaded face has asks for the part covering it
(`Core::after_frame` → `requests()`; `<datars-view>` fetches it from `chunks/`), and the label
snaps to its real glyphs.

**Licences** are read from each font's name table and recorded in the manifest's `fonts`, with
where the build got the font. Restricted-embedding fonts are warned about; "no subsetting" fonts
ship whole.

**What it costs.** The web runtime lost Inter's 152 KB (gzipped: core 1,444,685 → 1,292,835 bytes,
full 1,772,770 → 1,620,934). A chart pays for its own glyphs instead: the votes story's first load
grew from 7.8 KB to 21.9 KB (Inter 400/600/700 subsets: 7.8, 1.0 and 4.9 KB), charts with
interaction or runtime text 13–30 KB more (every face covers the full set). Subsets are cut per
chart, so they aren't shared between charts; a shared per-script subset of the default family is
the next step if many small charts share an origin.

## Host integration

The host app keeps control of what surrounds the chart and can talk to it:

```swift
// iOS — the chart is downloaded; nothing about it is compiled into the app.
DatarsView(source: .url("https://charts.example.com/c/household-spending"),
           policy: .init(allowScript: false, publishers: [.key("ed25519:9f2c…")]))
    .tokens(AppTheme.current.datarsTokens)                        // match the app's look, dark mode
    .provide("household", rows: spending.map(\.datarsRow))        // private data never leaves the device
    .onEvent("activate") { e in router.open(category: e.key) }    // chart → app navigation
    .signal("period", period)                                     // app → chart
```

```kotlin
// Android (Compose)
Datars(source = DatarsSource.Url("https://charts.example.com/c/household-spending"),
       tokens = appTheme.datars, onEvent = { e -> if (e.name == "activate") nav.open(e.key) }) {
    provide("household", spendingRows)
}
```

```html
<!-- Any site or CMS: one script, then charts by URL. Republishing updates every embed. -->
<script type="module" src="https://cdn.example.com/datars/1/runtime.js" integrity="sha384-…"></script>
<datars-view src="https://charts.example.com/c/election-2026" state="overview"></datars-view>
```

- **Data slots** make remote *templates* possible: a bundle declares a typed slot, the host fills it
  with local rows, and the chart is resolved on the device (T2). A bank can ship a new chart design to
  every user without sending anyone's data to a server.
- **Tokens** stay late-bound where the document allows it, so an app can reskin remote charts (brand,
  dark mode, high contrast, Dynamic Type) without republishing.
- **Events and signals** connect the chart to the app: selections can drive navigation; app state can
  drive the chart.
- For CMSs that don't allow scripts, every bundle also has an iframe URL and an oEmbed endpoint (a
  page running the web runtime, served by whoever hosts the bundle) and a T0 image.

## Compatibility across runtime versions

Apps in the field run old runtimes for years; bundles must keep working (P16).

- **Format versioning:** each runtime reads the current bundle format and the previous ones within a
  published support window (e.g. two years).
- **Runtime floor per publish:** the compiler targets the oldest runtime to support and emits variants
  accordingly. Newer features are compiled down where possible (a new easing kind becomes a LUT; a new
  node kind becomes geometry or a pre-rendered image); where not possible, the variant for older
  runtimes drops to a lower tier.
- **Graceful degradation:** a runtime that can't satisfy any interactive variant shows T0 and reports
  why (to the host and to optional telemetry).
- **Version-skew tests:** CI renders every example bundle on the last N runtime releases
  ([13](13-testing.md)).

## Security and app-store policy

- **Integrity:** every chunk is verified against its hash; manifests are signed (ed25519); hosts can
  pin the publisher keys they accept.
- **No ambient authority:** bundles can reach only the data endpoints they declare *and* the host's
  policy allows; the sandbox has no IO at all.
- **Budgets:** per-bundle limits on memory, resolve time and bytes, so a broken or hostile bundle
  can't hang or bloat the app — it falls back to T0.
- **App stores:** T0–T2 bundles are content (like images, Lottie or Rive files). T3 bundles contain
  sandboxed interpreted code; apps that don't want that set `allowScript: false`, and the compiler
  pre-expands for the size classes those apps support.
- **Web:** the runtime needs `script-src` for its CDN and `'wasm-unsafe-eval'`; the iframe embed covers
  strict CSPs.

## Loading sequence

**Web**

1. The page's HTML contains the embed; if the site pre-renders, it can include the bundle's poster and
   accessible text directly (fetched at the site's build *or* request time — still no rebuild needed to
   update, since the embed also fetches live).
2. The runtime's core loads (cached after the first chart on the origin), then the manifest, then the
   initial state's chunks. Charts boot one at a time, nearest the viewport first, never during a scroll
   gesture (booting figures mid-scroll causes jank).
3. The live view replaces the poster in the frame it's ready — same engine, so nothing moves or
   flashes.
4. The program prefetches what it can predict: the next states' scenes and plans, basemap tiles along
   the next camera flight.
5. Heavier modules load only if the chosen variant needs them.

**Apps**

1. `DatarsView` shows the cached version immediately if there is one (offline included), otherwise the
   T0 poster as soon as it arrives.
2. The manifest is revalidated in the background; only changed chunks download.
3. Apps can **pin** bundles for offline use, **prefetch** bundles they'll show soon, and ship a
   `.datars` file in the app as a first-run default — same format, replaced by network updates.

## Big data stays interactive

- **Tiles and LOD for dense marks:** a 10⁶-point scatter becomes a quadtree of point tiles; the camera
  requests what's visible at the right density; semantics summarize per tile and drill in on focus.
  *Built:* an `instances` node with `lod` over a source publishes as a **point archive** (PMTiles
  of self-describing point tiles: bit-packed grid positions, row numbers, every column in the
  smallest exact codec) shipped beside the chart under a content hash; the shipped document reads it
  by range. The galaxy example: a 65 KB bundle, a 36 MB archive of 4,000,000 stars of which the
  first view reads ~350 KB, and the frame the core runtime draws from it is the golden the source
  document makes natively (`packages/web/test/points.test.mjs`). Posters draw a tenth of the sample.
- **Aggregation pushed to build time:** a dashboard over 10⁷ rows ships pre-aggregated cubes for the
  filters it offers, not the rows.
- **Time-chunked series:** scrubbing a long series loads the window in view.
- **Progressive scenes:** the first frame uses the coarsest LOD; detail streams in without layout shift.

## Accessibility is part of the bundle

- Every bundle carries accessible text (summary, per-state alternatives, data tables), usable before
  the runtime runs and as the initial state of the live accessibility tree.
- Keyboard order is compiled from semantics; focus moves across data in reading order.
- Reduced-motion plans are precomputed, so honoring the preference costs nothing.
- Dark and high-contrast token sets are lookup-table swaps, and hosts can supply their own.

## Other targets

| Target | Differences |
|---|---|
| Video | the whole timeline resolved at build; frames rendered in parallel by the CPU reference or GPU; captions generated |
| Static | poster(s) + accessible HTML or tagged PDF |
| Self-contained page | a single HTML file with the runtime and a `.datars` inlined, for offline sharing |

## A tiny playback player (option)

T1 variants — scenes, plans, hit index, semantics, anchors — are self-contained. The reference
player is the wasm core. A tiny dedicated JS player (Canvas2D, no wasm) could make the first chart on
a site even cheaper; it's worth doing only if it passes the same conformance tests as the reference.
Open question in [17](17-roadmap.md).

## Budgets in CI

Per example, variant and runtime version, on reference devices: bundle bytes (first load and warm
cache), runtime module bytes, time to poster, time to interactive, frame time p50/p95 during
transitions, memory high-water mark, and a flash/layout-shift check during the poster → live handover.
A regression fails the build unless accepted with a reason.

### A chart in an app, each user's data on the device

A document can leave its data to the app: `data.slot("spending", { key, types, sample })`. The
chart ships inside the app as a single `.datars` file (or is fetched once and cached) — built once,
never per user. The app hands in the signed-in user's rows on the device and they go nowhere else:

- web: `el.data = { spending: rows }` (or `el.provideData("spending", json)`), any time — before the
  bundle has loaded the rows wait for it, and new rows transition the chart;
- SwiftUI: `DatarsChart(source: .url(Bundle.main.url(forResource: "spending", withExtension: "datars")!), data: ["spending": json])`;
- Android: `view.loadAsset("spending.datars"); view.provideData("spending", json)`;
- previews: `datars render doc.ts --data spending=user.json`.

Until rows arrive the `sample` shows (design previews, tests, the static tiers). Rows without the
columns the chart relies on (its key, typed columns and the sample's columns) are refused with the
reason, and the chart keeps what it showed. `examples/spending` is a banking-style example with a
web page (`examples/spending/web`) that switches between two accounts.
