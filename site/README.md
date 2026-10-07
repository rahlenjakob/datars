# The datars website

A static site — HTML per page, no framework, no server — built by `scripts/build-site.mjs` into a
folder any static host serves (GitHub Pages included, under a subpath). Every chart on it is a live
`<datars-view>` playing a bundle the build publishes, exactly as `datars publish` writes it.

```sh
node scripts/build-site.mjs --out out/pages          # everything (≈ 4 min: the articles are most of it)
node scripts/build-site.mjs --out out/pages --fast   # skip the example articles (reuses a previous build's)
datars serve out/pages --port 8960                   # preview
```

Environment: `DATARS_SITE_URL` (absolute URL of the site's root, for canonical links, Open Graph and
the sitemap; the Pages workflow passes the deployment's), `DATARS_REPO_URL` (the GitHub link; else
the Actions repository, else this checkout's GitHub remote).

## Who it is for

People who make charts — reporters, analysts, product teams, app developers, and the coding agents
working for them. Not people working *on* datars: the design docs in `docs/*.md` are for them, and
the site links there only as "how it works".

## Information architecture

| URL | Page | Source |
|---|---|---|
| `/` | Home: the hero, then one section per strength, each with a live chart | `pages/index.html` |
| `/features/` | The twelve strengths, each a card with a small live chart the page drives through the public API (one card plays at a time; the pointer's plays at once) | `pages/features/index.html` + `features-index.js`; figures in `figures/overview/` |
| `/features/charts/` | Batteries included: the recipe playground (any std recipe's figure on the engine with the recipe sandbox: controls from `datars describe`, editable CSV, the doc.ts behind it), one table drawn by nine recipes that morph into each other (the reader's data via `provideData`), the catalogue as the playground's picker | `pages/features/charts.html` + `features-charts.js` (its doc.ts writer is checked by `features-charts.test.mjs`); figures in `figures/charts/` |
| `/features/data/` | The data lab (sample or your own CSV/JSON into a data slot with `provideData`, the CLI's profile of each sample), a live source answered by a simulated API through `datarequest`, a transform pipeline, the spending slot | `pages/features/data.html` + `features-data.js`; figures in `figures/data/` |
| `/features/theming/` | Five fictional brands over one published chart (`setTokens`, eased colours), one colour into a theme and on into the studio, locks a host can't break | `pages/features/theming.html` + `features-theming.js`; figures in `figures/theming/` |
| `/features/big-data/` | Four million points, point pyramids, streaming archives | `pages/features/big-data.html` |
| `/features/platforms/` | One chart on every target, laid over each other (swipe, difference, ΔE heatmap): live web, simulator and desktop captures (`figures/platforms/capture-*.sh`), and the build's PNG/SVG/PDF/MP4 (`{{export:…}}`) | `pages/features/platforms.html` + `features-platforms.js`; captures in `img/platforms-*` |
| `/features/maps/` | The map playground: fly to places one committed archive covers, layers by signal, basemap styles by token, the bytes and range requests read; projections, the Rio story | `pages/features/maps.html` + `features-maps.js`; figures and the archive in `figures/maps/` |
| `/features/animation/` | The motion playground (the reader edits a motion rule on the live engine), the vocabulary of moves, scroll scrubbing, reduced motion | `pages/features/animation.html` + `motion.js`; figures in `figures/motion/` |
| `/features/interaction/` | The interaction lab: linked views (brush, click-to-filter, explore, a dragged limit) with the signals as the engine holds them (`signals`, the `signal` event), the wiring lit as it fires, intents switched off and on | `pages/features/interaction.html` + `features-interaction.js`; figures in `figures/interaction/` |
| `/features/accessibility/` | One chart as each reader meets it: the screen reader's tree walked on the chart, the keyboard's log, colour-blind and high-contrast views, reduced motion | `pages/features/accessibility.html` + `features-accessibility.js`; figure `figures/accessibility/lab.ts` |
| `/features/extensibility/` | The recipe workbench: the reader edits the ejected std/bar, a recipe from scratch or a scene's JSON, and the engine's sandbox expands it in the page, morphing to the std chart beside it; eject proved with `datars diff`; the worlds recipe | `pages/features/extensibility.html` + `features-extensibility.js`; figures (and their local recipes) in `figures/extensibility/` |
| `/features/delivery/` | Republish a chart and watch three embeds follow, with the chunks fetched (Resource Timing); the tiers' chunk lists; a signed-manifest refusal | `pages/features/delivery.html` + `features-delivery.js`; figures in `figures/delivery/` |
| `/features/developers/` | A bench with lint's real findings fixed live, `explain()` on any mark, real CLI and MCP sessions replayed | `pages/features/developers.html` + `features-developers.js`; figures in `figures/developers/` |
| `/gallery/` | Every chart type (each std recipe's live figure, from `{{std:cards}}`, family by family) and every story, filtered together; the lab (`#lab=<alias>`) steps, scrubs, restyles, remixes and writes the embed code for any of them. Charts three screens away give their engines back and start again when near (`gallery.js`, `gallery.css`) | `pages/gallery.html` |
| `/gallery/documents/` | Every chart's TypeScript, tiers, tokens, films and exports — the lab fetches it on first use | `pages/gallery/documents.html` |
| `/themes/` | The theme studio: edit every token of a theme (presets, a brand colour, colours and expressions, generated palettes, Google Fonts, sizes, shapes, maps, cards) on ten live charts in light, dark and high contrast, with the theme's checks, and export it as TypeScript, JSON or a runtime layer | `pages/themes.html`, `studio.js` (the editor; a hidden `<datars-view>` resolves the theme per mode), `scripts/site/studio.mjs` (its markup); the charts are `figures/studio/` |
| `/articles/` | Ten data stories in one house style, every chart live (`scripts/build-articles.mjs`) | `articles/` |
| `/performance/` | Measured numbers, the method, the tools | `pages/performance.html` + `perf/*.json` |
| `/why/` | What datars does, what it costs, when to pick it | `pages/why.html` |
| `/under-the-hood/` | How it works, layer by layer: document, bundle, runtime, resolve, motion, maps, rendering, determinism, tests, platforms, delivery | `pages/under-the-hood.html` |
| `/docs/` | End-user documentation (Markdown) | `docs/**/*.md` |
| `/docs/std/…` | The chart reference, one page per recipe, each with a live example | generated from `datars describe --json`; the examples are `figures/std/<recipe>.ts` (the build fails for a recipe without one) |
| `/docs/sdk/…` | The SDK reference: every export of `@datars/sdk`, one page per part (documents, data, nodes, options and scales, props, motion, programs, themes, recipes), with live figures | `docs/sdk/*.md`; signatures, fields and doc comments from `packages/sdk/src` via `{{sdk:name}}` / `{{sig:name}}` (`scripts/site/sdk.mjs`: the build fails for an export no page covers); figures are `figures/sdk/<name>.ts` |
| `/docs/cli/` | CLI reference | `docs/cli.md` + generated from `datars help` |

`sitemap.xml`, `robots.txt` and `404.html` are generated; `why.html` redirects to `/why/`.

## How a page is made

A page is a body with front matter (`title`, `description`, optional `css` — its own stylesheet,
`site/<file>`, linked in the head — and optional `og` — a chart alias whose
render becomes the page's social card). The build wraps it in the shared layout (`scripts/site/`:
head with canonical/OG/Twitter tags, nav, footer), then fills placeholders:

| In the source | Becomes |
|---|---|
| `href="/docs/"`, `src="/…"` | page-relative URLs (the site works under any subpath) |
| `<div class="chart" data-chart="galaxy">` | a slot for the published chart: its address, aspect ratio and reserved space (`--aspect`, `--min`, `--phone`) written as CSS variables, so nothing moves when the chart mounts |
| `{{alt:galaxy}}` | the chart's text alternative — its title, description, and every state's narration and data labels — from the bundle's accessible-text chunk: real, crawlable text |
| `{{size:x}}`, `{{archive:x}}`, `{{runtime}}` | gzipped bundle sizes, archive sizes, runtime size — measured by this build |
| `{{variants:x}}` | a chart's variants (T0–T3) as bars to scale, each made of its chunks coloured by kind — measured by this build |
| `{{code:path}}`, `{{code:path#L10-40}}` | a file from the repo, highlighted |
| `{{src}}/examples/votes/doc.ts` | that file's page on GitHub (right whether this folder is the repository or a folder inside one) |
| `{{strip:examples/votes/doc.json\|1\|2\|6}}` | a transition as a filmstrip from `datars film` (document, from, to, frames), made by this build |
| `{{run:semantics examples/budget/doc.json}}` | a CLI command's real output, captured at build time |
| `{{perf:…}}` | headline cards and tables computed from the benchmark JSON in `perf/` |
| `{{std:list}}`, `{{mcp:tools}}`, `{{count:recipes}}` | generated lists and numbers |
| `{{look:presets}}`, `{{look:accents}}` | the "Make it yours" preset and accent buttons (the header's menu uses the same markup); `site.js` wires every `data-look-*` control on a page to the reader's look, saved in `localStorage` and applied to every chart but the articles' |
| `{{studio:colours}}` (`type`, `shape`, `maps`), `{{studio:checks}}`, `{{studio:data}}` | the theme studio's token rows and checks table, written with the built-in theme's values (resolved per mode by `datars theme --json`), and that data for `studio.js` |
| `{{tokens:x}}` | the theme tokens chart `x` reads, as chips: what its recipes declare, plus the inks its scene and its first resolved state draw with |
| `{{sdk:group}}`, `{{sig:geom.rect}}`, `{{sdk:index}}` | an SDK export's entry (heading, signature, doc comment, source line), its signature in a table cell, every export by module — read from `packages/sdk/src` |

**Figures.** Every document in `figures/<section>/<name>.ts` is a chart too, alias
`<section>-<name>` — the chart reference's examples (`std/`), the SDK reference's (`sdk/`), the
explanations' diagrams (`how/`), the animation page's (`motion/`) — live like the rest, but not
counted or listed with the site's charts. `thumb(alias)` in the build renders one's first state as
light and dark SVG thumbnails. A figure a page rebuilds in the browser (the motion playground, the recipe playground, the
workbench — via `setDocument`) is also published as its document, at `/play/<alias>.json` (the
build's `raw` list: `motion-playground` and every `std-`, `charts-` and `extensibility-` figure),
beside `/play/std.json` (every recipe's description and the catalogue's families). The runtime's
atlas is published at `runtime/atlas/` for such documents' maps.
A page's own script (`motion.js`) is copied beside `site.js` and loaded by that page alone.
Screenshots live in `img/` (a light and a dark capture where the page's theme should pick).

Docs are Markdown (`docs/**/*.md`, a small CommonMark subset: headings, lists, tables, fenced
code, inline HTML); `scripts/site/markdown.mjs` renders them and code is highlighted at build time.

## No jumping on load

Every chart slot reserves its exact box in CSS before any script runs (`site.css`:
`.chart[style*="--aspect"]:not(:has(datars-view))`, with an explicit width so a minimum height can't
widen it on phones). Inter is preloaded and never swaps in after first paint (`font-display:
optional` over metric-matched `"Inter Fallback"` faces, one per weight). A stepped chart's step bar
is written by the build under the slot, and the element's own arrows are hidden (they'd appear after
mount); in the scroll story the element's narration card is hidden too (it moves at every step). The
nav has a fixed height; images and videos carry their dimensions. Checked with
`scripts/perf-browser.mjs --scroll` (CLS) and a step-scroll probe that records every shift's source.

## Refreshing the performance numbers

The performance page renders `perf/*.json` — raw output of `scripts/perf-browser.mjs --json`
against a build of this site. To refresh, build, serve a copy of the output (the build deletes its
folder, so don't measure the folder you rebuild) and run:

```sh
export DATARS_PLAYWRIGHT=/path/to/playwright/index.mjs
node scripts/perf-browser.mjs http://127.0.0.1:8961 gallery/ --dpr 2 --json site/perf/transitions.json
node scripts/perf-browser.mjs http://127.0.0.1:8961 gallery/ --dpr 2 --network fast4g --cold --json site/perf/transitions-fast4g-cold.json
node scripts/perf-browser.mjs http://127.0.0.1:8961 "" gallery/ features/maps/ features/big-data/ --scroll --dpr 2 --json site/perf/scroll.json
```

Note the machine in `pages/performance.html` if it changes.
