---
title: Preview, check and debug
description: The datars toolchain for making charts — a live preview that morphs each edit in, check, lint, render, explain, film, diff, profile, tests, and the in-browser stats panel.
lede: Everything runs locally, without a GPU or a browser, and prints text you — or a coding agent — can read. Every command takes a doc.ts or a doc.json, and --json for machine-readable output.
---

## The loop

```sh
datars dev doc.ts                     # a live page: edit, save, watch it morph
datars check doc.ts                   # does it load? diagnostics with the reason
datars lint doc.ts                    # chart-design mistakes, with a fix for each
datars render doc.ts --state 2        # look at a state as a PNG
datars film doc.ts --from 0 --to 1    # look at a transition as a filmstrip
datars explain doc.ts --key '("SE",)' # why does this element look like that?
```

## `datars dev` — the live page

```sh
datars dev mychart/doc.ts             # http://127.0.0.1:8788/
datars dev mychart/doc.ts --port 9000
```

A page with your chart, its states and everything the tools have to say about it. Save the document (or a local recipe under `recipes/`) and the page rebuilds it and **morphs the new version in** from what's on screen — the chart doesn't reload, so you see exactly what your edit changed. If a build fails, the page keeps the last good version on screen and says why.

<figure class="shot"><img class="thumb thumb-light" src="/img/dev-explain.png" alt="The datars dev page: a bar chart with its states under it, and the Explain panel listing the recipes, data row and expression values behind the clicked bar" width="1280" height="760" loading="lazy"><img class="thumb thumb-dark" src="/img/dev-explain-dark.png" alt="" aria-hidden="true" width="1280" height="760" loading="lazy"><figcaption>The live page: the chart, its states, and — for the bar you clicked — the recipes that made it, its data row and every expression's value.</figcaption></figure>

- **States** — the pills under the chart, or <kbd>←</kbd> <kbd>→</kbd>.
- **Preview** — the authored size, tablet, phone or full width (the chart lays itself out again at each width; it isn't scaled), light or dark mode.
- **Problems** — `check` diagnostics and `lint` findings, each with its fix.
- **Explain** — click any mark: the recipes that made it, its data row and every expression's value, with the same `datars explain` command to run in a terminal.
- **Profile** — what it costs, three ways: this browser's frames live (engine and raster time per frame, missed refreshes, the share of a 60 Hz frame used); every transition you play, scored — frames per second, drops, the stall before it moves, and its slowest frame taken apart; and the engine's own profile, the same as `datars profile`: every state resolved cold and warm and every transition stepped at 60 fps on a headless GPU, re-run on each save (turn that off for heavy documents). The small stats overlay on the chart is still there too: <kbd>Shift</kbd>+<kbd>D</kbd>.
- **Document** — its size and states, its data sources and keys, the recipes it uses, the command to publish it.

<figure class="shot"><img class="thumb thumb-light" src="/img/dev-profile.webp" alt="The Profile panel for a 20,000-point scatter: live frame times split into engine and raster, and the transitions played, one with a dropped frame and 20,000 instances rebuilt per frame flagged as heavy on phones" width="1280" height="760" loading="lazy"><img class="thumb thumb-dark" src="/img/dev-profile-dark.webp" alt="" aria-hidden="true" width="1280" height="760" loading="lazy"><figcaption>Profile: this browser's frames as they happen, each transition you play scored, and the engine's own profile of the whole program.</figcaption></figure>

<div class="shot-pair"><figure class="shot"><img class="thumb" src="/img/dev-problems.png" alt="The Problems panel with a lint warning about a data source without keys, and its fix" width="1280" height="760" loading="lazy"><figcaption>Problems: diagnostics and lint findings, each with a fix.</figcaption></figure><figure class="shot"><img class="thumb" src="/img/dev-phone-dark.png" alt="The dev page previewing a pie chart at phone width in dark mode" width="1280" height="760" loading="lazy"><figcaption>Phone width, dark mode: the chart lays itself out again.</figcaption></figure></div>

Maps with an [automatic basemap](/docs/maps/#where-the-tiles-come-from) show at once from what the geodata cache holds; street-level data is fetched in the background and swapped in as it arrives, including where you pan and zoom. Fonts are served the way a bundle would ship them, so the preview draws what readers will see.

## Check and lint

`datars check` loads the document and reports what's wrong: unknown recipes and parameters (with the closest match), expressions that don't parse, missing data, fonts with no source.

{{run:check examples/votes/doc.json}}

`datars lint` goes further — it resolves every state and looks for charting mistakes:

| Rule | Finds |
|---|---|
| `identity/row-keys` | a data source without a key: transitions would pair the wrong rows when the data changes |
| `identity/mismatch` | a transition where most data marks lose their identity and cross-fade instead of moving |
| `legibility/label-collisions` | overlapping labels |
| `legibility/offscreen` | labels cut off at the canvas edge |
| `encoding/too-many-colours` | more categorical colours than a reader can tell apart |
| `a11y/semantics` | interactive elements without an accessible label |
| `maps/attribution` | vector tiles drawn without the OpenStreetMap credit |
| `fonts/embedding` | a font whose licence restricts embedding |
| `theme` | the theme's checks: contrast, palette distinctness, monotone ramps |

Each finding comes with a fix:

```sh
$ datars lint doc.ts
warning  identity/row-keys source `sales` has no key: rows are identified by position, so transitions pair the wrong rows when data changes
         fix: declare a key column: data.values(…, { key: "id" })
```

Lint exits with an error code when it finds errors, so it fits in CI.

## Look at it

| Command | Gives you |
|---|---|
| `datars render doc.ts --state 2 --size 360x640 --dpr 2 --mode dark` | one state as a PNG from the CPU reference renderer — the same pixels on every machine. `--out x.svg` or `--out x.pdf` for vector output; `--hash` prints the pixel hash to compare targets |
| `datars film doc.ts --from 0 --to 1 --frames 8` | a filmstrip of a transition plus its motion trails — where every mark travels |
| `datars inspect doc.ts --state 1` | the scene as text: every node's key, geometry, inks and role |
| `datars semantics doc.ts --state 1` | the accessibility tree — what a screen reader gets |
| `datars states doc.ts` | the program's states, by index and name |
| `datars diff doc.ts --states bars,pie` | what changes between two states, element by element; or `datars diff a.ts b.ts` between two versions of a document |
| `datars fonts doc.ts` | every font token, the face it resolved to, its source and licence |

`datars film examples/votes/doc.json --from 1 --to 2 --frames 6`, made when this page was built — the ranked bars fold into discs in flight and settle as slices, each keeping its party:

<figure class="shot">{{strip:examples/votes/doc.json|1|2|6}}</figure>

States can be named or numbered everywhere: `--state europe` or `--state 1`. `--data sales=rows.csv` fills a data slot with other rows, to preview a chart with a user's data.

## `datars explain` — why does it look like that?

Point at an element by its key and `explain` walks back from the pixels: which recipes made it, the template, the data row, and every expression with the value it produced.

{{run:explain examples/votes/doc.json --key '("SD",)'|14}}

The key path is as `datars inspect` prints it; its last part (`'("SD",)'`) is usually enough. On a live chart in the browser, `view.explain(path)` gives the same answer.

## Before you chart: `datars data profile`

```sh
$ datars data profile count.json
8 rows, 3 columns

party  str  8 distinct
  → unique: a key — rows keep their identity through filters, joins and transitions

share  num  8 distinct  3.5 … 27.6
  → percentages 0–100: a share of a whole (stacked bars, a waffle, a pie for few parts)

counted  num  1 distinct  6.0 … 6.0
  → 1 distinct integers: an ordinal? (a band axis)

keys: [party]
```

Column types, missing values, ranges, candidate keys (single columns and pairs) and hints: years stored as numbers, columns of country codes that join to the atlas, spans that want a log scale.

## Profile: where the time goes

```sh
datars profile doc.ts                 # every state cold and warm, then every transition frame by frame
datars profile doc.ts --timeline      # … with each frame of each transition
datars profile doc.ts --cpu --dpr 2   # raster on the CPU reference instead of the GPU
datars profile doc.ts --json
```

States are resolved cold and warm; then every transition is stepped exactly as a 60 Hz display shows it — the stall before it moves, each frame's engine time (interpolation, tiles, flattening) and raster time, and the frames that stand out. Explorable views are panned and zoomed too. Times are this machine's, natively; the web runtime is 1.5–3× slower, so look at *where* the time goes.

## In the browser

Every `<datars-view>` carries a frame profiler:

- Focus a chart and press <kbd>Shift</kbd>+<kbd>D</kbd> for the "stats for nerds" panel: the renderer (WebGPU or CPU), frame times and a frame graph, the renderer's caches, the transition in flight.
- Add `?datars-perf` to the page's URL, set the element's `perf` attribute, or `localStorage["datars:perf"] = "1"`, and the profiler runs from the start: a corner readout while things move, one console line per transition, a `perf` event on the element, and every summary in `window.__datarsPerf`.
- `view.stats` says what the last frame drew from big data: rows, points drawn, tiles, bytes fetched.

```js
chart.addEventListener("perf", (e) => console.log(e.detail.label, e.detail.fps, e.detail.dropped));
```

To measure whole pages the way readers load them, the repository has `scripts/perf-browser.mjs`: it opens a site in Chrome, steps every chart through every transition and reports frame rates, dropped frames, stalls and engine/raster times. `--network fast4g|slow4g|3g` throttles, `--cold` disables the cache, `--cpu 4` slows the CPU fourfold like a mid-range phone, and `--scroll` scrolls each page top to bottom and reports the frame rate, long tasks and layout shift. The [performance page](/performance/) is made with it.

## Tests: goldens for pictures and motion

```sh
datars test                  # every document under examples/
datars test votes            # just the ones matching "votes"
datars test --update         # accept the current output as the new goldens
```

`datars test` runs over the `examples/` folder where you run it. For each document it records every state and 64 sampled frames of every transition, and checks motion invariants — no flashes, no blinks, no marks popping in, no NaNs, exact endpoints. Goldens live in `tests/golden/<example>/`; a change shows as a filmstrip of the frames that differ. Because rendering is deterministic, the goldens hold on any machine — and in CI.

Accept new goldens only after looking at the renders: they are what every later change is compared with.

## Replay a session

The engine can record a reader's session — every input, with its time — as JSON. `datars replay session.json` plays it back exactly, as a filmstrip and the final state, which turns "the chart did something odd when I clicked" into a reproducible case.

## See also

- [CLI reference](/docs/cli/) — every command and flag.
- [Coding agents and MCP](/docs/agents/) — the same tools for agents.
- [Performance](/performance/) — measured numbers and how they're measured.
