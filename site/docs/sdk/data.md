---
title: Data
titleTag: Data — SDK reference · datars docs
description: The data sources (data.*) and table operations (op.*) of @datars/sdk — values, CSV, URLs, slots, geo sources, generated rows, filters, windows, joins and layout algorithms — with figures of stack, bin and window.
lede: A document's `data` names its sources; its `tables` derive new tables from them with operations the engine runs — the heavy ones as Rust algorithms. Rows keep their keys through every operation, which is what lets marks morph when a table changes.
---

## Sources

{{sdk:data}}

Each helper returns one source for the document's `data`. The engine never fetches: URLs become requests the host fulfils (the browser, the app, the CLI), and `datars publish` ships files a document names by relative path.

| Helper | Rows come from |
|---|---|
| {{sig:data.values}} | inline columns (`{ region: [...], v: [...] }`) or records (`[{ region, v }, …]`) |
| {{sig:data.csv}} | inline CSV text: delimiter, quoting, decimal commas and missing-value markers are detected |
| {{sig:data.url}} | a CSV, JSON or GeoJSON file by URL; `rows` says where a JSON API keeps its records (`"data"`, `"results.items"`), `id` names a GeoJSON feature property |
| {{sig:data.slot}} | the host app, at runtime — each user's own rows; `sample` shows until they arrive |
| {{sig:data.geojson}} | an inline GeoJSON FeatureCollection (a geo source, for `geom.feature`) |
| {{sig:data.topojson}} | an inline TopoJSON topology |
| {{sig:data.atlas}} | a built-in atlas: `"countries"`, keyed by ISO 3166-1 alpha-3 |
| {{sig:data.tiles}} | a vector-tile archive (PMTiles), read by byte range, drawn by [`tiles`](/docs/sdk/nodes/#tiles) |
| {{sig:data.tiles.auto}} | an automatic basemap: the build cuts an archive to the document's own cameras (network at build time only) |
| {{sig:data.font}} | a font file, or `"google:Family"`, whose glyphs join every text's fallbacks (other scripts) |
| {{sig:data.generate}} | rows the engine generates: each column an expression over `d.i` (the row number) and earlier columns; `rand(key, stream)` and `randn(key, stream)` are seeded |

```ts
data: {
  sales:  data.values({ region: ["N", "S"], v: [42, 28] }, { key: "region" }),
  prices: data.csv("day,price\n1,96.2\n2,101.4", { key: "day" }),
  count:  data.url("count.json", { key: "party", live: { every: 2 } }),
  world:  data.atlas("countries"),
  pts:    data.generate(3000, { id: e("d.i"), x: e("randn(d.i, 1)") }, { key: "id" }),
}
```

`data.generate` keeps only the columns you write: `d.i` is the row number while generating, not a column — add one (`id: e("d.i")`) to key the rows by it. The [instances figure](/docs/sdk/nodes/#instances) generates its three thousand points this way.

{{sdk:SourceOpts}}

Types are inferred when you don't give them: numbers, booleans, ISO dates (as days since 1970-01-01 in expressions), else text; zero-padded codes stay text. `live.mode` and `live.window` are recorded but not acted on yet: every refresh replaces the table (see [live sources](/docs/data/#live-sources)).

## Derived tables

{{sdk:table}}

`table("sales", op.filter(…), op.sort(…))` is shorthand for `{ from: "sales", ops: [...] }` in the document's `tables`. A table can read signals (`op.filter(e("selected.has(d.region)"))`); it is recomputed when they change and the marks that read it animate to the new rows. Recipes make their own with [`cx.table`](/docs/sdk/recipes/#Cx).

{{sdk:Op}}

{{sdk:op}}

### Rows and columns

| Operation | Does |
|---|---|
| {{sig:op.filter}} | keeps the rows where the expression is true |
| {{sig:op.derive}} | adds (or replaces) column `as`, computed per row |
| {{sig:op.sort}} | sorts by columns: `op.sort("year")`, `op.sort(["share", "desc"], "name")` |
| {{sig:op.top}} | the `n` rows with the largest `by`; with `other`, the rest folded into one row labelled `other` (numbers summed) |
| {{sig:op.sample}} | a seeded random sample of `n` rows |

### Groups and windows

| Operation | Does |
|---|---|
| {{sig:op.aggregate}} | one row per group: `{ as: [fn, field] }` with `fn` one of `count`, `sum`, `mean`, `median`, `min`, `max`, `distinct`, `first`, `last` or a quantile (`q25`, `q90`) |
| {{sig:op.window}} | a value per row over its partition, in `order` (`"-v"` descends): `cumsum`, `rank`, `dense_rank`, `lag`, `lead`, `rolling_mean`, `rolling_sum`, `rolling_std`, `rolling_min`, `rolling_max`, `ema`, `cummax`, `cummin`, `first`, `last`, `share_of_total`, `pct_change` |
| {{sig:op.bin}} | a bucket per row in column `as`: numbers by `step` or into about `count` nice bins, dates by `unit` (`year` … `day`, `weekday`), text by `part` (`first-word`, `first-letter`, `after:<sep>`, `before:<sep>`) |
| {{sig:op.interpolate}} | one row per `key`, its `value` interpolated along `time` at `at` — usually a signal (bar-chart races, maps through the years) |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-data-bin" data-caption></div></div><figcaption><code>op.bin</code> then <code>op.window("rank", …, { partition: ["bin"] })</code>: every row drops into its bin and stacks by its rank there — a histogram made of the rows themselves. <a href="{{src}}/site/figures/sdk/data-bin.ts">Figure source</a></figcaption>{{alt:sdk-data-bin}}</figure>

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-data-window" data-caption></div></div><figcaption><code>op.window</code>: a 14-day <code>rolling_mean</code> (with <code>min: 14</code>, the first thirteen days are null and filtered out) and a running <code>cumsum</code>, on the same rows. <a href="{{src}}/site/figures/sdk/data-window.ts">Figure source</a></figcaption>{{alt:sdk-data-window}}</figure>

### Combining and reshaping

| Operation | Does |
|---|---|
| {{sig:op.join}} | adds another table's columns, matched on `on`; `"left"` keeps unmatched rows, `"inner"` drops them |
| {{sig:op.union}} | appends another table's rows |
| {{sig:op.values}} | these rows instead of the input's, written into the op (columns or records, keyed): a recipe's own lookup table — a tile map's grid — to join data onto |
| {{sig:op.pivot}} | long to wide: one column per value of `key`, holding `value`, one row per `index` |
| {{sig:op.unpivot}} | wide to long: `columns` become rows of `[keyAs, valueAs]` |

### Layout algorithms

These compute positions and sizes and add them as columns, so a recipe's marks only read them. `width`, `height` and `position` are expressions (`e("box.w")`, `e("scale.x(d.v)")`), so layouts follow the box they're drawn in.

| Operation | Adds |
|---|---|
| {{sig:op.stack}} | `y0`, `y1` (or `as`): each series stacked on the ones before it per `x`; `offset` `"expand"` makes shares, `"silhouette"` centres, `"wiggle"` streams; `order` `"inside-out"` puts the biggest series in the middle (a streamgraph's), `"ascending"`/`"descending"` sorts them by total |
| {{sig:op.pie}} | `a0`, `a1` (or `as`): start and end angles in radians, clockwise from 12 o'clock — what [`geom.arc`](/docs/sdk/nodes/#geom) takes |
| {{sig:op.treemap}} | `x0`, `y0`, `x1`, `y1` (or `as`): squarified rectangles |
| {{sig:op.pack}} | `cx`, `cy`, `r`: packed circles |
| {{sig:op.beeswarm}} | `offset` (or `as`): how far each dot moves across so none overlap |
| {{sig:op.spread}} | `spread` (or `as`): 1-D positions pushed at least `gap` apart, moved as little as possible (labels at line ends) |
| {{sig:op.lanes}} | `lane` (or `as`): 1-D intervals — a label's left and right edge, `start` and `end` — packed into as few lanes as possible, `gap` apart; null past `max` lanes (labels along a timeline) |
| {{sig:op.units}} | one row per unit of `value`, keyed under its parent row (`("S", 12)`), so a bar can split into its units; the unit number is the column `as` (default `__unit`) |
| {{sig:op.parliament}} | `x`, `y`, `r`: seats on concentric arcs (after `op.units`) |
| {{sig:op.waffle}} | `x`, `y`, `w`, `h`: grid cells (10 × 10 by default) |
| {{sig:op.calendar}} | `cx`, `cy`, `cell`: week-column × weekday-row cells for dates |
| {{sig:op.sankeyNodes}} | a table of nodes: `name`, `x0`, `y0`, `x1`, `y1`, `value` |
| {{sig:op.sankeyLinks}} | `path`: each link's ribbon, as SVG path data for [`geom.path`](/docs/sdk/nodes/#geom) |
| {{sig:op.waterfall}} | `start`, `end`, `is_total`: floating extents from a running total |
| {{sig:op.scatterIn}} | a table of seeded dots inside each row's region of a geo source: `id`, the key, `lon`, `lat` |
| {{sig:op.kde}} | a table of density curves: per group (`groupby`), `steps` rows (64) on one shared grid with the grid `value` and the Gaussian kernel `density` there (each curve integrates to 1; bandwidth by Silverman's rule unless given); `trim` gives each group its own extent, `extend` widens it by that many bandwidths — the outlines of violins and ridgelines |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-data-stack" data-caption></div></div><figcaption><code>op.stack</code>, twice on one table: <code>as: ["y0", "y1"]</code> for the stack and <code>offset: "expand", as: ["e0", "e1"]</code> for shares. The bars read the pair for the step and morph between them. <a href="{{src}}/site/figures/sdk/data-stack.ts">Figure source</a></figcaption>{{alt:sdk-data-stack}}</figure>

### Graphs and hierarchies

A graph is a nodes table (keyed by `id`, default the table's key) and a `links` table of `source` and `target` ids; a hierarchy is one table whose `parent` column holds each row's parent id (none for a root). The ops run on the nodes (or the hierarchy) and add columns; a link's endpoints come from the laid-out nodes by `op.lookup`. Nodes are matched by their id text, so numeric and text ids both work.

| Operation | Adds |
|---|---|
| {{sig:op.force}} | `x`, `y` (or `as`), `degree`: a deterministic force-directed layout — starts seeded by each node's id, a fixed number of `iterations` of link springs (`distance`), many-body `charge`, `gravity` and collision of circles of `radius` (an expression per node), all inside `width` × `height`; computed once per data and box, never per frame |
| {{sig:op.communities}} | `community` (0 = the largest), `degree` and `order`: clusters of densely linked nodes (Louvain modularity), and a position that keeps each cluster together — sort by it to order a matrix or an arc diagram |
| {{sig:op.lookup}} | another table's columns (`values`, named `as`) from its row whose `key` equals this row's `field` — each link's endpoint positions from the laid-out nodes |
| {{sig:op.tree}} | `x`, `y`, `px`, `py` (the parent's position), `depth`, `leaf`, `path`: the tidy tree (Reingold–Tilford), or a dendrogram with `method: "cluster"`; lay out in angle × radius (`width: 2π`) for a radial tree |
| {{sig:op.partition}} | `x0`, `y0`, `x1`, `y1`, `depth`, `leaf`, `sum`, `share`, `branch`, `path`: a band per depth, each node spanning its parent's share ∝ its `value` (icicles; sunbursts in angle × radius); `sort` puts the largest siblings first |
| {{sig:op.chordGroups}} | a table of a chord diagram's groups: `name`, `a0`, `a1` (radians clockwise from 12 o'clock), `value`, `index` |
| {{sig:op.chordRibbons}} | `sa0`, `sa1`, `ta0`, `ta1`: each link's span on its source and target group, and with `r` its ribbon as SVG `path` data for [`geom.path`](/docs/sdk/nodes/#geom) |

### Big data: bins, contours and paths

A million rows are drawn as what they add up to. These operations bin the rows in one pass, in Rust, in the rows' own units: they read nothing but the rows and numbers, so their tables are computed once per data and kept across states, layout passes and hovers — a mark per bin, thousands instead of millions. The [`hexbin`](/docs/std/hexbin/), [`heatmap2d`](/docs/std/heatmap2d/), [`contours`](/docs/std/contours/) and [`manyLines`](/docs/std/manyLines/) recipes are made of them.

| Operation | Makes |
|---|---|
| {{sig:op.hexbin}} | a table of non-empty hexagons, `columns` across `extent` (default: the rows' own) and regular on screen when the extent is drawn `aspect` times as tall as wide: `hex` (the key), `x`, `y` (centre), `count`, `value` (`fn` of the column `value`, else the count), `hw`, `hh` (corners at `(x, y ± hh)` and `(x ± hw, y ± hh/2)`), `col`, `row` |
| {{sig:op.bin2d}} | a table of the non-empty cells (all with `empty`) of a `columns` × `rows` grid: `cell` (the key), `x0`, `x1`, `y0`, `y1`, `x`, `y`, `count`, `value`, `col`, `row` |
| {{sig:op.contours}} | ring vertices of density contours — a Gaussian density (`bandwidth` in grid cells) cut where the region inside holds 1/(levels+1), 2/(levels+1)… of the rows (or `shares`, or `by: "density"`): `level` (0 the outermost), `ring`, `x`, `y`, `share`, `density` (rows per unit²) |
| {{sig:op.paths}} | one row per group (`by`) with its rows' points (`x`, `y`: expressions, usually through the scales) as SVG path data for [`geom.path`](/docs/sdk/nodes/#geom) — a new subpath where `ring` changes or a point is missing, closed with `closed` — plus `end_x`, `end_y` (the last point) |

```ts
// Binned once per data; then, per resolve, a few thousand vertices through the scales.
const rings = cx.table("rings", "trips", op.contours({ x: "km", y: "min", columns: 160, rows: 100, bandwidth: 3 }));
const levels = cx.table("levels", rings, op.paths({ by: "level", ring: "ring", closed: true, x: e("scale.x(d.x)"), y: e("scale.y(d.y)") }));
```

Keep the context-free binning and the per-resolve `paths` in two tables, as above: a table whose operations read a scale is recomputed on every resolve, all of its operations with it.

The guide to data — types, profiling, live sources, data slots — is [Data and tables](/docs/data/).
