---
title: Big data
description: Draw hundreds of thousands to millions of rows in datars — density maps of hexagons and grid cells, density contours, thousands of series at once, point clouds with level of detail, generated rows, and point archives streamed by range.
lede: Tens of thousands of points are just marks. Millions are drawn as what they add up to — hexagons, grid cells, contours, a thousand faint lines, binned once in the engine — or as a point cloud, indexed into a pyramid of tiles and drawn a density-preserving sample at a time.
---

## Pick the right tool

| Rows | Use | What happens |
|---|---|---|
| up to ~100,000 | [`point`](/docs/std/point/) in a `plot` (or `instances`) | every row is drawn, in one instanced draw; transitions are planned once and each frame is cheap |
| thousands, every one visible | [`swarm`](/docs/std/swarm/) | a beeswarm packed so no dot overlaps — about a second to pack 20,000 |
| any number: where they crowd | [`hexbin`](/docs/std/hexbin/), [`heatmap2d`](/docs/std/heatmap2d/) | counted into hexagons or grid cells a few px across, once per data; a frame draws the bins — a few thousand — not the rows |
| any number: the shape of a crowd | [`contours`](/docs/std/contours/) | a smoothed density cut into nested levels, each holding a share of the rows |
| hundreds to thousands of series | [`manyLines`](/docs/std/manyLines/) | a faint path per series, so their bulk shows; the ones that matter highlighted and named |
| hundreds of thousands to millions, explored | [`cloud`](/docs/std/cloud/) (or `instances` with `lod`) | indexed once into a pyramid of tiles; each frame draws only what the camera shows, a sample that keeps the density |

Measured, natively on an Apple Silicon Mac (`datars profile`): the 20,000-point scatter plans its transition in about 10 ms, then each animation frame costs 0.34 ms of engine time. The four-million-star galaxy draws 0.2–1.3 ms per panning frame with up to 50,000 points on screen; in Chrome with WebGPU its frames take 5 ms at the median while zooming and panning. The [performance page](/performance/) has the browser measurements.

## A density of millions of points

A scatter of a million dots paints over itself: past a few thousand you see ink, not how many rows are where. Count them instead. [`hexbin`](/docs/std/hexbin/) counts the rows into hexagons, [`heatmap2d`](/docs/std/heatmap2d/) into the cells of a regular grid, and each bin takes its colour from the count — or from an aggregate of a column.

```ts
plot({ data: "trips", x: "km", y: "min", xType: "linear", zero: false, children: [
  hexbin({ radius: 8, unit: "trips" }),              // hexagons 8 px across, coloured by how many trips each holds
] })

plot({ data: "pickups", x: "x", y: "y", xType: "linear", zero: false, children: [
  heatmap2d({ cell: 6, value: "fare", format: "$,.0f" }),   // cells 6 px across, coloured by their mean fare
] })
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="std-hexbin" data-caption></div></div><figcaption><b>200,000 trips in hexagons</b> — walking, cycling, bus and car each make a ridge of their own speed. Hover a hexagon for its count. <a href="{{src}}/site/figures/std/hexbin.ts">Figure source</a></figcaption>{{alt:std-hexbin}}</figure>

- **Sized by the screen.** `radius` (hexagons) and `cell` (grid cells) are px: the recipe lays the lattice over the plot area it's drawn in, so the bins stay that size on a phone and on a wide screen.
- **Counts or aggregates.** Without `value`, a bin's colour is its count, on a square-root ramp so sparse bins stay visible beside dense ones (`ramp: "log"` for counts that span orders of magnitude). With `value`, it's the `mean` of that column (`fn`: `sum`, `min`, `max`, `count`), on a linear ramp.
- **Sized, too.** `hexbin({ size: true })` shrinks each hexagon with its count as well, so the sparse edges fade away; with a single `fill`, size alone carries the count.
- **A key and tooltips.** A small colour key sits in a corner of the plot area (`legend`, or `"none"`); `unit` names what a row is (`"trips"`: *Trips per hexagon*, *1,204 trips near 12, 34*).
- **Give `extent`** when the plot shows less than the rows cover (an explicit `xDomain` or `yDomain`): the lattice covers the rows' own extent by default, and a few far-off rows would stretch it.

<figure class="fig"><div class="frame"><div class="chart" data-chart="std-heatmap2d" data-caption></div></div><figcaption><b>300,000 pickups on a grid</b> — counted into cells, then coarser cells, then coloured by the mean fare. <a href="{{src}}/site/figures/std/heatmap2d.ts">Figure source</a></figcaption>{{alt:std-heatmap2d}}</figure>

## The shape of a crowd

[`contours`](/docs/std/contours/) draws where the rows are, not the rows: a smoothed density of their points cut into nested levels. By default each level holds a further share of the rows — with four levels, the innermost line encloses the densest fifth, the next the densest two fifths — so a reader can say "half of all eruptions fall inside this line".

```ts
plot({ data: "eruptions", x: "minutes", y: "wait", xType: "linear", zero: false, children: [
  contours({ levels: 4, bandwidth: 14, unit: "eruptions" }),   // filled bands, darker inside
  // contours({ style: "lines", dots: 3000 })                  // lines over a seeded sample of the rows
  // contours({ shares: [0.9, 0.5, 0.25] })                    // exactly these shares
] })
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="std-contours" data-caption></div></div><figcaption><b>The shape of 150,000 eruptions</b> — two crowds, short eruptions and long ones. Hover a band for the share of eruptions inside it. <a href="{{src}}/site/figures/std/contours.ts">Figure source</a></figcaption>{{alt:std-contours}}</figure>

`bandwidth` is the smoothing, in px on screen: larger is rounder, smaller shows more of the crowd's shape (and more noise). `by: "density"` spaces the levels evenly up to the peak instead. Each level is one path with its holes, so a ring-shaped crowd draws right.

## Many series at once

A thousand series drawn as a thousand coloured lines is noise. [`manyLines`](/docs/std/manyLines/) draws every series as a faint grey path — the fainter the more there are, so where most of them run darkens — and brings the ones the story is about forward in the accent colour, named at their ends.

```ts
plot({ data: "weeks", x: "week", y: "temp", xType: "linear", zero: false, children: [
  manyLines({ series: "station", highlight: "Station 417" }),
] })
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="std-manylines" data-caption></div></div><figcaption><b>1,000 stations at once</b> — a year of weekly temperatures, then one station, then three. Hover a line for its station. <a href="{{src}}/site/figures/std/manyLines.ts">Figure source</a></figcaption>{{alt:std-manylines}}</figure>

`highlight` is a value of `series`, a list of them, or an expression — a signal a [select](/docs/std/select/) or a story step sets. With a mouse, the line under the pointer comes forward too (`hover`). Every series is one path, built from its rows through the plot's scales when the chart resolves; its rows must be in x order.

## How binning stays cheap

The recipes above describe; the engine computes. Each is a table operation — [`op.hexbin`, `op.bin2d`, `op.contours` and `op.paths`](/docs/sdk/data/#big-data-bins-contours-and-paths) — that runs in Rust over the whole table in one pass, in the rows' own units. Because those operations read nothing but the rows, their tables are computed once per data and kept: every story step, layout pass and hover reuses them, and a frame draws the bins, never the rows. The lattice's size is a number the recipe picks from its box when it expands; another size (a phone, `cell: 12`) is binned once more.

Measured with `datars profile` on an Apple Silicon Mac (release build): resolving a state of the hexbin figure takes 3–7 ms with 200,000 rows behind it, and an animation frame between its states 1–1.5 ms; the 150,000-row contours resolve in about 1 ms a state. The thousand-line chart draws 1,000 paths, about 4 ms of GPU time per animation frame, and resolving it — which a hover does — takes around 20 ms, since its paths are rebuilt from 52,000 rows through the scales.

## Many points in a plot

```ts
plot({ data: "pts", x: "x", y: "y", xType: "linear", zero: false, color: "g", children: [
  point({ r: e('emphasis == "B" ? (d.g == "B" ? 2.4 : 1.2) : 1.6'),
          opacity: e('emphasis == "B" && d.g != "B" ? 0.25 : 0.8') }),
] })
```

`point` is instanced: one draw call for every point, and a change of radius, colour or position animates every point by its key. Keep the expressions per row simple — they run in the engine, per row, in compiled bytecode.

<figure class="fig"><div class="frame"><div class="chart" data-chart="scatter" data-caption></div></div><figcaption><b>20,000 points</b> — step forward and every point resizes and recolours in one transition.</figcaption>{{alt:scatter}}</figure>

## A point cloud of any size

```ts
import { cloud } from "@datars/std";

cloud({
  data: "stars", x: "x", y: "y",
  bbox: [-40000, -40000, 40000, 40000],   // the extent to frame — give it for millions of rows
  explore: "sky", maxZoom: 3000,          // drag, wheel and pinch; `sky.x`, `sky.y`, `sky.zoom`
  points: 80_000,                         // the most dots a frame draws
  r: e("clamp(0.95 + 0.34 * d.lum, 0.75, 3.6)"),
  fill: e('d.temp < 3700 ? "#ffb06e" : "#f7f5ff"'),
  label: e("`Star ${format(d.$row + 1, \",\")}`"),
  name: "4,000,000 stars",               // for the accessible description
})
```

`cloud` puts the rows in an explorable view. The engine indexes them once into a quadtree whose levels are seeded samples of the data. Each frame draws the levels whose rows in view fit its budget of `points` (150,000 by default): far away, a uniform sample — dense regions stay dense, sparse stay sparse; close up, every row. Deeper levels fade in as you zoom, so nothing pops. Panning and zooming never re-read or re-compute a row.

Other parameters: `glow` (a large, faint halo drawn from a sample, so density reads at a glance), `opacity`, `budget` (rows per tile of the index, default 2048), `padding`, and `children` drawn over the dots in the same coordinates. See the [cloud reference](/docs/std/cloud/).

Under `cloud` is an ordinary primitive you can use in your own recipes: `instances` with `lod`:

```ts
instances({ from: "stars", x: e("d.x"), y: e("d.y"), r: 1.2, fill: "$mark", lod: { points: 100_000, budget: 2048 } })
```

With `lod`, rows sit at their own `x`/`y` (expressions that may read only the row); put the node in a `view` with a camera to map them to the screen.

<figure class="fig"><div class="frame"><div class="chart" data-chart="galaxy"></div></div><figcaption><b>Four million stars</b> — drag to pan, scroll or pinch to zoom, hover a star. The first view downloads a few hundred kilobytes of a {{archive:galaxy}} archive.</figcaption>{{alt:galaxy}}</figure>

## Data too big to write down

Four million rows don't belong in a document. `data.generate` describes them as expressions over the row number instead, evaluated by the engine — the same rows on every platform:

```ts
data: {
  stars: data.generate(4_000_000, {
    r: e("-9500 * log(1 - rand(d.i, 3) * 0.996)"),        // an exponential disk
    theta: e("rand(d.i, 4) * 6.2832"),
    x: e("round(d.r * cos(d.theta), 2)"),
    y: e("round(d.r * sin(d.theta), 2)"),
  }, { keep: ["x", "y"] }),
},
```

- `d.i` is the row number; each column can read the columns before it.
- `rand(key, stream)` is a seeded uniform draw in [0, 1), `randn(key, stream)` a seeded normal draw. Different `stream` numbers give independent draws for the same row.
- `keep` lists the columns the table ends with; the rest are working values.

Use it for simulations, load tests and synthetic examples. Real data works the same way from a file: `data.url("points.csv", { types: { x: "num", y: "num" } })`.

## Point archives: download only what's in view

When you publish a document whose `cloud` (or `lod` instances) reads a data source — rows written in the document, read from a file or generated — `datars publish` doesn't ship the rows. It indexes them exactly as a frame would and writes a **point archive** beside the chart, under a content-hashed name (`tiles/<source>.<hash>.pmtiles`). The published document reads it by HTTP range request, tile by tile, as the camera needs them.

For the galaxy that means a {{size:galaxy}} bundle, a {{archive:galaxy}} archive on the server, and a first view that reads about 350 KB of it. Nothing is generated on the reader's device, and what the archive draws is exactly what the rows draw — the runtime's tests compare the two.

Any static host that answers range requests serves an archive: GitHub Pages, S3, R2, a CDN, `datars serve`.

## Watch it work

On the web, every `<datars-view>` reports what its last frame drew from big data:

```js
const chart = document.querySelector("datars-view");
const { rows, drawn, tiles, level, bytes, requests } = chart.stats ?? {};
```

For a readout on the chart itself, focus it and press <kbd>Shift</kbd>+<kbd>D</kbd> — the "stats for nerds" panel shows the renderer, frame times and a frame graph. Add `?datars-perf` to the page's URL (or the `perf` attribute to the element) to turn the profiler on from the start. See [Preview, check and debug](/docs/tools/#in-the-browser).

## Tips

- **Give `bbox`** for clouds of millions of rows and for point archives: otherwise finding the extent reads every row.
- **Set `points` to what the screen can use.** 50,000–150,000 is plenty for a full-width chart; more costs frame time without adding detail.
- **Morph thousands, explore millions.** Transitions of instanced marks interpolate every point on the CPU (linear in the number of points — about 17 ms at a million), so a 20,000-point morph is smooth and a million-point morph is not. For millions, change the camera or the colour, not every position.
- **Keys matter less for clouds.** A cloud is identified by its tiles, not per-row keys; use keyed `point` marks when individual rows have to move between states.
- **Aggregate what you can.** A bar chart of ten million rows is ten bars: derive them with `op.aggregate` or before you publish, and ship the result. A scatter of ten million rows is a few thousand hexagons (`hexbin`) or grid cells (`heatmap2d`).
- **Bin once, draw often.** Keep the heavy operation in a table that reads only rows — binning, contours — and put what reads the scales (`op.paths`, positions) in a second table from it: a table whose operations read a scale is recomputed on every resolve, all of them.
- **Swarms are for thousands.** Packing is exact and takes about a second per 20,000 dots; beyond that, use `point` with some transparency, or a cloud.

## See also

- [Data and tables](/docs/data/) — sources, types and table operations.
- [Publishing and hosting](/docs/publishing/) — where archives go and what a host must serve.
- [Big data feature page](/features/big-data/) — the galaxy in depth.
