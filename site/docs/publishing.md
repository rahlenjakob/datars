---
title: Publishing and hosting
description: Publish a chart as a manifest plus content-addressed chunks, host it on any static server or CDN, set cache headers, and republish without redeploying.
lede: One command turns a document into files any static host can serve. Republish, and every page and app that embeds the chart shows the new version.
---

## Publish a chart

```sh
datars publish chart.ts --alias election --to site/
```

```text
site/c/election  (14 chunks written, 0 already there)
```

`publish` compiles the document, builds every variant a runtime might need, and writes them into a *delivery folder*:

| Path | What it is | Changes |
|---|---|---|
| `c/<alias>` | The chart's **manifest**: which variants exist, which chunks each needs, the fonts and their licences. Embeds point here. | Every publish |
| `chunks/b3_<hash>` | **Chunks** named by the BLAKE3 hash of their bytes (`b3:…`, with `_` for `:` on disk): the poster, accessible text, scenes, plans, data, font subsets. | Never — a new version is a new file |
| `tiles/<source>.<hash>.pmtiles` | Archives read by byte range: [automatic basemaps](/docs/maps/) and [point pyramids](/docs/big-data/). Content-addressed too. | Never |
| anything else | Files the document fetches by relative URL at runtime — a live data file, a tile archive you made yourself — copied to where the browser will look for them. | When you republish |

The rules for aliases: letters, digits, `-` and `_`. Without `--alias`, the document's file name (or its folder's, for `doc.ts`) is used. Without `--to`, the folder is `out/site`.

Several charts can share one folder. Chunks with the same bytes are written once, so charts that share an atlas, a font subset or a dataset share its chunk, and a reader's browser caches it once.

`--json` prints what happened, for scripts and build tools:

```json
{ "alias": "election", "dir": "site/", "written": 14, "present": 0,
  "copied": ["tiles/basemap.e484e609f9a9.pmtiles"],
  "gzip": { "T0": 17486, "T2": 49360, "T3": 48504 } }
```

`gzip` is the compressed size of each variant — what a reader downloads for the chart (archives read by range are not included: a reader fetches only the parts in view).

### Try it locally

```sh
datars serve site/ --port 8787
```

`datars serve` is a small static server for a delivery folder — use it to check a publish before you upload it. It serves the web runtime under `/runtime/` and a page embedding every alias at `/` (unless the folder has its own `index.html`). It answers HTTP `Range` requests like a CDN, and sets the cache headers a production host should (below). It listens on `127.0.0.1` only.

A few helper pages for testing:

| URL | Shows |
|---|---|
| `/` | Every published alias, one `<datars-view>` each |
| `/steps/<alias>` | The chart beside text steps that scroll it (`steps=".step"`) |
| `/scrolly/<alias>` | The chart pinned while the page scrolls through it (`scrub`) |
| `/view/<path/to/doc.json>` | A document from the served folder, not yet published |
| `/render/<alias>.png` | An image of a published chart — see [the image server](/docs/export/#the-image-server) |

To simulate a live feed, put numbered snapshots in a folder next to the data file: a request for `count.json` next to a folder `count.json.d/` gets that folder's files in name order, one more every `DATARS_REPLAY_PERIOD` seconds (default 2).

## Host it anywhere static

A delivery folder is plain files. Anything that serves files works: GitHub Pages, Netlify, Cloudflare Pages, an S3, R2 or GCS bucket behind a CDN, nginx, a CMS's asset store. There is no datars server to run, no API key and no per-view cost.

### Cache headers

Chunks never change, so they can be cached forever. The manifest must be revalidated, or readers keep the old chart.

| Path | `Cache-Control` |
|---|---|
| `chunks/*` | `public, max-age=31536000, immutable` |
| `tiles/*.pmtiles` (content-addressed archives) | `public, max-age=31536000, immutable` |
| `c/*` (manifests) | `no-cache` (or a short `max-age` with `stale-while-revalidate`) |
| other copied files (live data, your own archives) | `no-cache` |

On hosts that read a `_headers` file (Netlify, Cloudflare Pages):

```text
/chunks/*
  Cache-Control: public, max-age=31536000, immutable
/tiles/*
  Cache-Control: public, max-age=31536000, immutable
/c/*
  Cache-Control: no-cache
```

GitHub Pages doesn't let you set headers; charts work there as they are (this site is hosted that way). Add an empty `.nojekyll` file so Pages serves every file untouched.

### Range requests

Tile and point archives are read in small byte ranges as the camera moves — a reader zooming into the four-million-star galaxy downloads a few hundred kilobytes of its {{archive:galaxy}} archive. Every mainstream static host and bucket supports `Range`. If yours doesn't, the runtime still works: it receives the whole archive once and reads it from memory.

### Charts on another origin

Charts can live on a different host from the pages that embed them — one chart host for a whole newsroom, say. The chart host then needs CORS headers on `c/`, `chunks/`, `tiles/` and data files:

```text
Access-Control-Allow-Origin: https://www.example.com
Access-Control-Allow-Headers: Range
```

Embeds then use absolute URLs: `<datars-view src="https://charts.example.com/c/election">`. Everything else the chart fetches is resolved relative to its manifest, so a delivery folder can move to a new host or subpath without changes.

## Republish and roll back

Run `publish` again with the same alias:

```sh
datars publish chart.ts --alias election --to site/
```

New chunks are written first and the manifest last (by an atomic rename), so a reader loading the chart mid-publish never sees a manifest whose chunks aren't there. Unchanged chunks stay as they are — a correction to a label re-uploads a few kilobytes, not the chart.

Every page and every app that embeds `c/election` shows the new version on its next load. No site deploy, no app release.

`publish` never deletes chunks, so older manifests keep working. That gives you two simple tools:

- **Pin a version** by publishing it under a second alias as well (`--alias election-2026-09-14`), and embed that where the chart must never change.
- **Roll back** by publishing the previous document again (keep documents in version control), or by copying an older `c/<alias>` file back. Its chunks are still there.

`--revision <label>` records a label of your choice in the manifest (`datars bundle inspect` and the runtime show it); without it, the revision is `dev`.

## Tiers: what each runtime gets

Every published chart carries several **variants**. A runtime picks the richest one it can play:

| Tier | Contains | Plays |
|---|---|---|
| **T0** | SVG poster + accessible text | Anywhere: shown before any script runs, and by runtimes that can't play anything else |
| **T1** | Baked scenes for every state, transition plans | States, transitions, scrubbing, tooltips, camera flights |
| **T2** | The document with recipes pre-expanded, data by hash | Everything reactive: signals, brushes, linked views, live data, data slots |
| **T3** | The source document | Recipes run on the device, in the engine's sandbox |

The compiler skips T1 when baked scenes would dwarf the source. The web runtime loads its smaller **core** engine for any chart that has a T0–T2 variant — nearly all of them — and the **full** engine (with the recipe sandbox) only for T3-only charts, raw documents and `.datars` files. `<datars-view no-script>` refuses T3 altogether; iOS and Android apps choose the same with `allowScript`.

The charts on this site, per tier, gzipped:

{{sizes:table}}

The web runtime itself is {{runtime}} gzipped, loaded once and cached by the browser for every chart on your site.

## One file instead of a folder

```sh
datars bundle chart.ts --out chart.datars
datars bundle inspect chart.datars
```

`datars bundle` writes the same variants into a single `.datars` file (manifest, chunks and an index), with any archives beside it as `<name>.<source>.pmtiles`. Use it to ship a chart inside an app for offline use, to email a chart, or to host it where a folder of files is awkward. `--explain` lists every chunk with its size; `--json` gives the compiler's decisions.

`bundle inspect` shows the variants, their sizes, the fonts with their licences, and which variant each kind of runtime would play:

```text
votes (revision dev)
  T3  81604 bytes  24370 gzipped  [poster, a11y, doc, font, font, font]
  T2  102772 bytes  26490 gzipped  [poster, a11y, doc, font, font, font]
  T1  165838 bytes  37658 gzipped  [poster, a11y, font, font, font, program, scene, scene, scene, scene]
  T0  34616 bytes  7809 gzipped  [poster, a11y]
fonts:
  Inter 400  OFL-1.1  from datars:fonts/Inter-Regular.ttf
  Inter 600  OFL-1.1  from datars:fonts/Inter-SemiBold.ttf
  Inter 700  OFL-1.1  from datars:fonts/Inter-Bold.ttf
picks:
  web (core engine)                            T2
  web (full engine)                            T3
  app (DatarsKit / Android, script allowed)    T3
  app without script                           T2
  playback-only runtime                        T1
```

(The output of `datars bundle inspect` for the votes example.) Building the bundle also prints the compiler's decisions — which fonts were subset to how many glyphs, which states were baked, whether recipes were pre-expanded.

`<datars-view src="chart.datars">`, `DatarsChart(source: .url(…))` on iOS and `DatarsView.loadAsset("chart.datars")` on Android all open it.

## Data that arrives later

### Live sources

A source with `live` is fetched again on a schedule, and each new snapshot animates into place — rows keep their keys, so bars move to their new ranks:

```ts
data: { count: data.url("count.json", { key: "party", live: { every: 2 } }) },
```

`every` is in seconds. `mode` is `"snapshot"` (the default: keys missing from the new file exit), `"upsert"` or `"append"`. The file itself is plain JSON or CSV on any host; update it however you like and the chart follows, with no republish. See the [election-night example]({{src}}/examples/election/doc.ts).

### Data slots: each reader's own data

A slot is a source the chart ships **without** — the page or app fills it at runtime. One published chart serves every user, and their data never leaves their device:

```ts
data: {
  spending: data.slot("spending", {
    key: ["month", "category"],
    types: { amount: "num" },
    sample,            // shown until real rows arrive: previews, tests, the poster
  }),
},
```

Fill it on the web with `chart.data = { spending: rows }` (or `chart.provideData("spending", json)`), on iOS with `DatarsChart(source:…, data: ["spending": json])`, on Android with `view.provideData("spending", json)`, and in previews with `datars render chart.ts --data spending=user.json`. Rows can be handed in before the chart has loaded; new rows transition the chart. Rows missing the columns the chart relies on are refused, and the chart keeps what it showed.

## Fonts travel with the chart

The build acquires every face the chart draws with — project files, `https://` URLs and Google Fonts families, downloaded once into a local cache — and ships a **subset** cut to the characters the chart can show. No runtime ever fetches a font from a font service. Text that can arrive later (live data, slots) gets per-script subsets that load only if needed.

Each font's licence, as its own name table states it, is recorded in the manifest. `datars fonts chart.ts` lists every font token with its source and licence; `datars lint` flags fonts whose licence restricts embedding.

## Signed charts

A page can play only charts you signed. Make a key once, sign as you publish, and name the key in the embed:

```sh
datars keygen --out newsroom.key          # prints the public key: ed25519:…
datars publish chart.ts --alias votes --to site/ --sign newsroom.key
```

```html
<datars-view src="/c/votes" publishers="ed25519:…"></datars-view>
```

The manifest carries the publisher and an ed25519 signature over everything in it, chunk hashes included, so no chunk can be swapped either. A chart that isn't signed, is signed by another key, or changed after signing is refused before any of it shows (not even its poster); the element fires an `error` event with `detail.refused` and says so in the chart's box. In CI, `DATARS_SIGNING_KEY` (the key's 64 hex digits) does what `--sign` does. Keep the key file out of version control: `keygen` writes it readable by you alone and never overwrites one.

## Next

- [Embed on the web](/docs/embed/web/), [in iOS and macOS apps](/docs/embed/ios/), [in Android apps](/docs/embed/android/)
- [Video, images and PDF](/docs/export/) from the same document
- [Charts are content](/features/delivery/) — why datars delivers charts this way
