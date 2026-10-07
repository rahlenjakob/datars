---
title: Web: <datars-view>
description: Put published datars charts on any web page with the <datars-view> element — installing the runtime, attributes, events, methods, sizing without layout shift, scroll stories.
lede: One script on the page, then one element per chart. The element fetches the chart, shows its poster, plays it with the GPU and mirrors it for screen readers.
---

> **Tip** Using React, Vite or Next.js? See [React, Vite and Next.js](/docs/react/) for the component and build plugins; everything on this page still applies underneath.

## The shortest version

```html
<script type="module" src="/runtime/datars.js"></script>

<datars-view src="/c/election" height="440"></datars-view>
```

`src` is the chart's manifest — the `c/<alias>` file that [`datars publish`](/docs/publishing/) wrote. Republish the chart and this page shows the new version on its next load, with no change to the page.

## Install the runtime

The runtime is the `@datars/web` package: a small loader (`datars.js`) and two WebAssembly engines. It isn't on npm or a CDN yet; build it from the repository (`scripts/build-wasm.sh && pnpm -C packages/web build`) or depend on the package folder, and copy these files next to your pages:

```text
runtime/
  datars.js                    the loader and the <datars-view> element
  wasm/datars_core.js          the core engine: plays published charts (T0–T2) — nearly all
  wasm/datars_core_bg.wasm
  wasm/datars_host_web.js      the full engine: adds the recipe sandbox (T3 bundles, raw documents)
  wasm/datars_host_web_bg.wasm
  wasm/wasi_shim.js
```

The element loads the core engine unless a chart needs the sandbox, so most pages never download the full one. The core engine and loader are {{runtime}} gzipped, fetched once and cached for every chart on your site. Serve `.wasm` files as `application/wasm` (every mainstream host does).

Two optional folders are only for raw, unpublished documents (`doc=`, `setDocument`): `fonts/` (the default Inter faces) and `atlas/` (the built-in countries atlas). Published charts carry their own fonts and atlases.

A build script only needs to copy them, for example in Node:

```js
import { cpSync, statSync } from "node:fs";
import { join } from "node:path";

const web = "node_modules/@datars/web/dist";   // or the package folder in the repository
const out = "site";
cpSync(join(web, "datars.js"), join(out, "runtime/datars.js"));
cpSync(join(web, "wasm"), join(out, "runtime/wasm"), {
  recursive: true,
  filter: (f) => statSync(f).isDirectory() || /datars_(core|host_web)|wasi_shim/.test(f),
});
```

To try a chart without any of this, `datars serve site/` serves the runtime itself under `/runtime/`.

## Size it before it loads

A chart that grows into its space when it mounts pushes the page down. Reserve the space first:

- Give the element a **`height`** in CSS pixels. It takes that height from the first frame it's on the page, before the engine arrives. Without it, the element guesses 60 % of its width.
- The chart's own proportions are in its manifest (`"size": [width, height]`) and its document. Charts re-lay themselves out for whatever box they get, so you choose the ratio; the document's is a good default.

This site writes each chart's aspect ratio into the page at build time and reserves the box in CSS, so nothing moves when the chart mounts:

```html
<div class="chart" data-src="/c/riksdag" style="--aspect: 0.5789"></div>
```

```css
/* Until the chart is there, the slot holds its exact box. */
.chart:not(:has(datars-view)) { aspect-ratio: 1 / var(--aspect); }
.chart datars-view { display: block; width: 100%; }
```

```js
// Mount when the slot comes near the screen, at the slot's height.
const near = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    near.unobserve(e.target);
    const slot = e.target;
    const view = document.createElement("datars-view");
    view.setAttribute("src", slot.dataset.src);
    view.setAttribute("height", String(Math.round(slot.clientWidth * Number(slot.style.getPropertyValue("--aspect")))));
    slot.appendChild(view);
  }
}, { rootMargin: "700px 0px" });
document.querySelectorAll(".chart[data-src]").forEach((s) => near.observe(s));
```

Mounting charts as they come near the viewport keeps a long article light: each chart's bundle and first frame cost nothing until the reader gets close. The element also waits for the reader to pause scrolling before it opens a chart, and a chart taken off the page frees its engine memory at once.

## Attributes

| Attribute | Value | What it does |
|---|---|---|
| `src` | URL | The chart: a manifest (`…/c/<alias>`) or a single `.datars` file. |
| `doc` | URL | A raw document (JSON) instead — for development; loads the full engine. |
| `height` | px | The height to reserve and draw at. The chart fills the element's width. |
| `mode` | `light`, `dark`, `high-contrast` | The theme mode. Without it the chart follows the reader's `prefers-color-scheme`, live. |
| `state` | number | Go to this program state (0-based); the transition plays. |
| `steps` | CSS selector | Scroll-triggered story: each matching element on the page is a step (see below). |
| `scrub` | — | Scroll-scrubbed story: the page's scroll position drives the program (see below). |
| `reduced-motion` | `reduce`, `no-preference` | Reduced motion for this chart: `reduce` always (short crossfades, no autoplay), `no-preference` never — for a reader who asked for reduced motion and then opted in to see the full motion here. Without it the chart follows the reader's `prefers-reduced-motion`, live. |
| `no-script` | — | Refuse bundles that need the recipe sandbox (T3); the chart plays a pre-expanded variant or its poster. |
| `allow-script` | — | Allow the sandbox even if `no-script` is also set. |
| `publishers` | keys | Space-separated `ed25519:…` keys; only manifests signed by one of them play. (The CLI doesn't sign bundles yet.) |
| `cpu` | — | Draw with the CPU renderer instead of WebGPU (the renderer used anyway where WebGPU is missing). |
| `perf` | — | Show the frame profiler panel (also: `?datars-perf` in the page URL, or Shift+D on a focused chart). |
| `engine` | URL | Load a specific engine build instead of the one the element picks. |
| `atlas-base` | URL | Where raw documents find built-in atlases (default: `atlas/` next to the runtime). |
| `font-server` | URL | Where raw documents acquire fonts (`datars dev` sets this). |

After loading, the element sets `data-engine` (`core` or `full`) and `data-renderer` (`gpu` or `cpu`) on itself.

## Talk to the chart

```js
const chart = document.querySelector("datars-view");

chart.send("next");                 // program events: next, prev, back, goto:<state>
chart.send("goto:ranked");
chart.setSignal("income", 42000);   // any signal the document declares
chart.setAttribute("mode", "dark");
chart.setTokens({ accent: "#b3261e", "radius.bar": 0, "stroke.line": 1.6 });

chart.addEventListener("state", (e) => {
  const { state, index, states, narration } = e.detail;
  caption.textContent = narration?.text ?? "";
});
```

### Methods and properties

| Member | What it does |
|---|---|
| `send(event)` | Fire a program event: `next`, `prev`, `back` (leave a chapter), `goto:<state>`. |
| `setSignal(name, value)` | Set a signal from the page — a filter, a slider, app state. Values are JSON (numbers, strings, arrays for key sets). |
| `setTokens(tokens)` | Override theme tokens (colours, sizes, corners, strokes, font tokens) on a published chart; the theme's locked tokens are kept. Colours re-ink on the next frame; type and shapes morph. A font token switches to a face the bundle carries, or to the TTF or OTF at the `src` it names (fetched like a data source, through `datarequest`); a Google Fonts name is for the build step and never fetched at runtime. Set before the chart opens, its faces arrive before the first frame (it waits up to 1.5 s), and no poster shows — the poster is the published look, and it would flash. See [themes](/docs/theming/). |
| `provideData(name, data)` | Fill a [data slot](/docs/publishing/#data-slots-each-readers-own-data) (or replace a source): CSV or JSON text, bytes, or rows (records or columns). Can be called before the chart has loaded; throws if the rows lack columns the chart needs. |
| `data = { slot: rows, … }` | Every slot at once. |
| `seek(position)` | Put the program at a position in states: `0` the first, `states − 1` the last, and a fraction the transition between two states that far through — the exact frame it would show then. For your own scrubbers and scroll-driven stories (`scrub` uses it). The chart holds the position until the next `seek`, step or event; asked before the chart has opened, it's applied when it does. |
| `reducedMotion` | Whether the chart plays reduced motion now: the `reduced-motion` attribute when set, else the reader's `prefers-reduced-motion`. |
| `setDocument(doc)` | Show a document held by the page (JSON text or object) — an editor's working copy; morphs from what's on screen. |
| `reload()` | Fetch the `doc` attribute's document again and morph to it (the edit loop). |
| `status` | The current state, all states, narration, the accessibility tree, resolved tokens, diagnostics. `null` until loaded. |
| `signals` | Every signal's value now, as `setSignal` takes them: numbers, strings, booleans, key sets as arrays, `null` for nothing — and the built-ins: `inspected` (the key under the pointer), a brush's `<name>.lo`, `.hi`, `.active`, an explorable view's `<name>.x`, `.y`, `.zoom`. `null` until loaded. |
| `stats` | For big data: rows, points drawn, tiles, level, archive bytes and requests so far. |
| `hitTest(x, y)` | Every element under a point (CSS px), topmost first — for editors and custom tooltips. |
| `explain(path)` | Why an element looks as it does: recipes, data row, expression values ([like `datars explain`](/docs/tools/)). |
| `showStats(on)` | Show or hide the frame profiler panel. |
| `frameRecord()` | The frame profiler's record for tools: the last few seconds of frames (engine ms, raster ms, gap), whether frames are coming, the renderer's stats. Starts the profiler, hidden; `datars dev`'s Profile tab draws it. |

### Events

| Event | `detail` |
|---|---|
| `state` | The chart's status (`state`, `index`, `states`, `narration`, …) — fired whenever the program moves: a step, autoplay, a scroll story, a click that drills down. |
| `perf` | A summary of each transition's frames, when the profiler is on (see [performance](/performance/)). |
| `datarequest` | `{ name, url, respond }` — the chart is about to fetch a source (a URL source, or a live source's refresh). Call `event.preventDefault()` and `detail.respond(data)` — bytes, text, an object, or a promise of one — to answer it yourself: your API client with its auth, a cache, a mock, a replayed feed. Unanswered, the element fetches `url`. |
| `pick` | What a click or tap landed on: `{ x, y, hits, row }` — for routing a click in an app. |
| `signal` | `{ signals, changed }` — after the reader's pointer, wheel, keyboard or screen reader, a step, or your own `setSignal` changed signals: all of them now (as `signals`), and the names that changed. For a page that follows the chart: a readout of the brush, an app's state. |

### Keyboard

A chart is focusable. <kbd>→</kbd> or <kbd>Space</kbd> steps forward, <kbd>←</kbd> back, <kbd>Esc</kbd> leaves a chapter, <kbd>Shift</kbd>+<kbd>D</kbd> toggles the stats panel. Interactive marks and engine-drawn sliders are reachable as buttons and range inputs in the accessibility mirror.

## Stories on the page

**Step buttons.** A chart with several states shows ← and → buttons under it. Hide them and drive it yourself with `send("goto:…")` — this site does, with pills under each chart.

**Scroll-triggered steps.** Put the chart in a sticky container beside your text, and name the step elements:

```html
<div class="steps">
  <section class="step" data-state="world">…</section>
  <section class="step" data-state="europe">…</section>
</div>
<datars-view src="/c/renewables" steps=".step" height="480"></datars-view>
```

When a step crosses the middle of the viewport, the chart goes to the state its `data-state` names (or, without one, to the step's position) and the transition plays.

**Scroll scrub.** With `scrub`, scroll position *is* the program position: the chart shows a transition part-way through as the reader scrolls, exactly. The scroll container is the nearest ancestor with `data-scrub` (else the element's parent); its passage through the viewport maps to the first … last state:

```html
<div data-scrub style="height: 400vh">
  <datars-view scrub src="/c/renewables" height="600" style="position: sticky; top: 0"></datars-view>
</div>
```

`datars serve` has test pages for both: `/steps/<alias>` and `/scrolly/<alias>`.

## Styling

The chart draws itself; the page styles what's around it. The element exposes CSS parts:

| Part | What it is |
|---|---|
| `stage` | The box the chart draws in |
| `poster` | The SVG poster shown while a slow first load finishes |
| `canvas` | The chart's canvas |
| `card` | The narration card (for charts that don't draw their own) |
| `tooltip` | The hover tooltip |
| `links` | Real links over elements that link somewhere (a map's OpenStreetMap credit) |
| `controls` | The ← → step buttons |
| `perf` | The frame profiler panel |

```css
datars-view::part(controls) { display: none; }          /* your own step controls instead */
datars-view::part(card) { font-family: "Source Serif 4", serif; }
```

The element sets the chart's resolved theme colours on itself as custom properties (`--ink`, `--paper`, `--muted`, `--rule`, `--surface`, `--surface-ink`, `--accent`), so page chrome next to the chart can match it.

## What the element does for you

- **Poster first, never a flash.** On a slow first load the chart's SVG poster shows while the engine downloads; a chart that's ready within 0.6 s shows no poster at all, and the live canvas fades in over it.
- **Renders only when something moves.** No frames while the chart is still; autoplay holds and live refreshes wake it at the right time.
- **Pauses off screen.** A chart out of view draws nothing and its autoplay stops; it resumes where it was.
- **Reduced motion.** With `prefers-reduced-motion: reduce`, transitions become short crossfades and autoplay stays paused. The `reduced-motion` attribute overrides it per chart: `reduce` to force it, `no-preference` for a reader who opted in to the full motion.
- **Fair to the chart being read.** Charts loading further down wait while another is mid-transition, and busy charts off the reader's focus draw at a lower rate.
- **Prepares the next step.** While the reader is on a step, the engine resolves and plans the neighbouring steps in idle time and fetches the map tiles their flights will need.
- **Accessible.** A hidden list mirrors the chart's semantics (roles, labels, reading order), story narration is announced through a live region, and engine-drawn sliders become native range inputs. See [accessibility](/docs/accessibility/).

## Browsers and security

The engine draws with **WebGPU** where the browser has it and falls back to its **CPU renderer** where it doesn't, with the same result: the GPU output is checked against the CPU reference.

A strict Content-Security-Policy needs:

- `script-src` allowing the runtime's origin, plus `'wasm-unsafe-eval'` to compile the WebAssembly engine;
- `connect-src` allowing wherever the charts, their chunks, data and archives are served from.

If charts are served from another origin than the page, that host needs CORS headers — see [publishing](/docs/publishing/#charts-on-another-origin).

## Next

- [Publishing and hosting](/docs/publishing/) — where `src` comes from
- [Signals and interaction](/docs/interaction/) — what `setSignal` can drive
- [Themes and brands](/docs/theming/) — tokens for `setTokens`
- [iOS and macOS](/docs/embed/ios/), [Android](/docs/embed/android/) — the same charts in native apps
