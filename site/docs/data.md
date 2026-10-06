---
title: Data and tables
description: Every way to get data into a datars chart — inline values, CSV, files and URLs, live feeds, rows an app hands in, atlases, tiles, generated rows — and every table operation.
lede: Sources bring rows in; derived tables reshape them. Every row has a key, and every operation keeps it.
---

## Sources

Rows can travel in the bundle, come from your server when the chart opens or on a schedule, be handed in by the app on the reader's device, be answered by your own code, or be generated on the device — [Data from anywhere](/features/data/) compares them, with live demos. A document's `data` names its sources. Each `data.*` helper makes one:

```ts
import { data } from "@datars/sdk";

data: {
  votes:   data.values({ party: ["S", "SD", "M"], share: [30.3, 20.5, 19.1] }, { key: "party" }),
  prices:  data.csv("day,price\n1,96.2\n2,101.4", { key: "day" }),
  count:   data.url("count.json", { key: "party", live: { every: 2 } }),
  world:   data.atlas("countries"),
  basemap: data.tiles.auto(),
}
```

| Helper | Rows come from |
|---|---|
| `data.values(v, opts)` | inline columns `{ col: [...] }` or records `[{ … }, …]` |
| `data.csv(text, opts)` | inline CSV text |
| `data.url(url, opts)` | a CSV or JSON file the host fetches — relative to the document, or any URL |
| `data.slot(name, opts)` | the app or page, at runtime (see [data slots](#data-slots)) |
| `data.geojson(fc, { id })`, `data.topojson(topo, { id })` | inline geographic features (a geo source: ids, properties and shapes) |
| `data.atlas("countries")` | a built-in atlas of regions (countries by ISO 3166-1 alpha-3 code) |
| `data.tiles(url)`, `data.tiles.auto()` | a vector-tile archive, read by byte range; `auto()` makes one for your map's views — see [maps](/docs/maps/) |
| `data.font(url)` | a font file whose glyphs join every label's fallbacks (other scripts) — see [themes](/docs/theming/) |
| `data.generate(rows, columns, opts)` | rows the engine generates from expressions (see [generated rows](#generated-rows)) |

The engine never fetches anything itself: URL sources become requests the host fulfils (the browser's `fetch`, the app's HTTP stack, the CLI's file reads). `datars publish` copies the files a document references by relative URL into the published site, so they travel with it.

### Keys and types

Options shared by every tabular source:

- **`key`** — the column (or columns: `{ key: ["region", "month"] }`) that identifies a row. Keys are what make marks morph instead of fade; give every source one. `datars data profile` suggests candidates.
- **`types`** — override inferred column types: `{ types: { day: "date", code: "str" } }`. Types are `num`, `str`, `bool` and `date`.

Types are inferred when you don't say: numbers, booleans, ISO dates, else text. CSV reading copes with real-world files — `,`, `;` or tab delimiters, quoted fields, Windows-1252 exports, a decimal comma with `;`, missing-value markers like `NA` or `..`. Zero-padded codes (`0114`) stay text so joins still work, and plain four-digit years stay numbers.

Dates are calendar days. In expressions they're numbers (days since 1970-01-01); plots put them on time scales, and `formatDate(d.day, "%b %Y")` writes them out.

## Profile before you chart

```sh
datars data profile count.json
```

{{run:data profile examples/election/count.json}}

The profile says what each column is, what's missing, which columns (or pairs) can be keys, and hints: percentages, years stored as numbers, spans wide enough for a log scale, ISO codes that join to an atlas.

## Live sources

A URL source with `live` is fetched again on a schedule, and each new snapshot animates into place — keyed bars slide to their new ranks, entering rows grow, leaving rows fade:

```ts
data: { count: data.url("count.json", { key: "party", live: { every: 2 } }) },   // seconds
```

Each refresh replaces the table, matched by key. `datars publish` copies the current snapshot next to the chart so the poster and first frame have data. To try a feed locally, put numbered snapshots in a folder named after the file — `count.json.d/001.json`, `002.json`, … — and `datars serve` replays them one every two seconds as `count.json` (the [election example]({{src}}/examples/election/doc.ts) does this).

> **Note** `live` also accepts `mode` and `window` fields in the document format; today every refresh is a snapshot, and those two are not acted on yet.

## Data slots

A chart can leave its data to the app that shows it. The chart ships once — built and published, never per user — and each user's rows are handed in on the device, where they stay:

```ts
data: {
  spending: data.slot("spending", {
    key: ["month", "category"],
    types: { amount: "num" },
    sample: { month: ["Apr", "Apr"], category: ["Food", "Housing"], amount: [412, 1200] },
  }),
},
```

Until real rows arrive, `sample` shows — in previews, tests and the static poster — and it says which columns the app's rows must have. Rows missing the key or those columns are refused with the reason, and the chart keeps what it showed. Hand rows in:

- web: `view.data = { spending: rows }` or `view.provideData("spending", json)`;
- iOS: `DatarsChart(source: …, data: ["spending": json])`;
- Android: `view.provideData("spending", json)`;
- previews: `datars render doc.ts --data spending=user.json`.

The [spending example]({{src}}/examples/spending/doc.ts) is a banking-style chart with a page that switches accounts.

## Answering requests yourself

The engine never fetches: it asks its host. On the web, every URL source — and every refresh of a live one — goes past the page first as a `datarequest` event, so the page can answer it: with its own API client and sign-in, a cache, a mock in tests, or a replayed feed.

```js
view.addEventListener("datarequest", (e) => {
  if (e.detail.name !== "sales") return;          // other sources: the element fetches them
  e.preventDefault();
  e.detail.respond(api.get("/sales", { auth }));   // bytes, text, an object, or a promise of one
});
```

The rows go through the same checks as fetched ones, and keyed rows animate in the same way. Apps do the same through their host: DatarsKit and datars-android fetch with the app's own HTTP stack, and `provideData` hands in rows at any time.

## Generated rows

For synthetic data too big to write down — simulations, load tests, procedural datasets — describe the columns as expressions and let the engine make the rows:

```ts
data: {
  pts: data.generate(200_000, {
    g: e("floor(rand(d.i, 1) * 3)"),
    x: e("d.g * 4 + randn(d.i, 2)"),
    y: e("d.g * 2 + randn(d.i, 3) * 1.5"),
  }, { keep: ["x", "y", "g"] }),
},
```

`d.i` is the row number; each column can read the ones before it. `rand(key, stream)` and `randn(key, stream)` are seeded uniform and normal draws — the same rows on every platform, every time. `keep` lists the columns the table ends up with. The [galaxy]({{src}}/examples/galaxy/doc.ts) generates four million stars this way; see [big data](/docs/big-data/).

## Derived tables

`tables` defines new tables from a source or another table, as a list of operations:

```ts
import { op } from "@datars/sdk";

tables: {
  totals: { from: "sales", ops: [op.aggregate(["region"], { total: ["sum", "sales"] })] },
  trend:  { from: "sales", ops: [op.filter(e("selected.isEmpty() || selected.has(d.region)"))] },
  ranked: { from: "votes", ops: [op.sort(["share", "desc"])] },
},
```

`table(from, ...ops)` is shorthand for `{ from, ops }`. Operations run in the engine (the heavy ones as Rust algorithms), and a table that reads a signal — like `trend` above — is recomputed when the signal changes, so the chart animates to the new rows. Marks and recipes name a table by its name; recipes also make their own internally.

### Rows and columns

| Operation | What it does |
|---|---|
| `op.filter(expr)` | keeps rows where the expression is true |
| `op.derive(as, expr)` | adds (or replaces) a column computed per row |
| `op.sort(...by)` | sorts by columns: `op.sort("year")`, `op.sort(["share", "desc"])` |
| `op.top(n, by, other?)` | the `n` rows with the largest `by`; with `other`, the rest folded into one row labelled `other` (numbers summed) |
| `op.sample(n, seed)` | a seeded random sample of `n` rows |

### Grouping and windows

| Operation | What it does |
|---|---|
| `op.aggregate(groupby, { as: [fn, field] })` | one row per group; `fn` is `count`, `sum`, `mean`, `median`, `min`, `max`, `distinct`, `first`, `last`, or a quantile like `q25`, `q90` |
| `op.window(fn, field, as, { partition, order, k })` | a running value per row: `cumsum`, `rank`, `dense_rank`, `lag`, `lead` (by `k`), `rolling_mean` (over `k`), `share_of_total`, `share_of_first`, `pct_change`; `order: "-pop"` sorts descending |
| `op.bin(field, as, { step \| count \| unit \| part })` | buckets: numbers by `step` or into about `count` nice bins; dates by `unit` (`year`, `quarter`, `month`, `week`, `day`, `weekday`); text by `part` (`first-word`, `first-letter`, `after:<sep>`, `before:<sep>`) |
| `op.interpolate(key, time, value, at)` | one row per key with `value` interpolated at time `at` — usually a signal, for bar-chart races and maps that recolour through the years |

```ts
op.aggregate(["continent"], { n: ["count"], people: ["sum", "pop"] })
op.window("rank", "pop", "rank", { partition: ["continent"], order: "-pop" })
op.interpolate("country", "year", "gdp", e("year"))   // `year` is a signal: scrub it
```

### Combining and reshaping

| Operation | What it does |
|---|---|
| `op.join(table, on, kind)` | adds another table's columns by key; `kind` is `"left"` (default) or `"inner"` |
| `op.union(table)` | appends another table's rows |
| `op.pivot(key, value, index)` | long to wide: one column per value of `key` |
| `op.unpivot(columns, [keyAs, valueAs])` | wide to long |

### Layout algorithms

These compute positions and add them as columns — the standard library's charts are built on them, and your own recipes can use them too:

| Operation | Adds |
|---|---|
| `op.stack({ x, series, value, offset })` | stacked extents (`offset`: `zero`, `expand`, `silhouette`, `wiggle`) |
| `op.pie({ value, sort, pad, start, end })` | start and end angles |
| `op.treemap({ value, width, height })` | squarified rectangles |
| `op.pack({ value, width, height, padding })` | packed circles |
| `op.waffle({ columns, rows, width, height, gap })` | grid cells |
| `op.units({ value })` | one row per unit (a value of 12 becomes 12 rows keyed under the parent — how a bar splits into dots) |
| `op.parliament({ cx, cy, r0, r1, rows })` | seat positions on concentric arcs |
| `op.beeswarm({ position, radius })` | offsets that pack dots without overlap |
| `op.spread({ position, gap, min, max })` | positions pushed apart by at least `gap` (labels at the ends of lines) |
| `op.calendar({ date, cell, weekStart })` | week-column × weekday-row cells |
| `op.sankeyNodes(…)`, `op.sankeyLinks(…)` | node boxes and ribbon paths for flows |
| `op.waterfall({ value, total })` | floating extents from a running total |
| `op.scatterIn({ geo, count, seed })` | seeded dots inside each region of a geo source (dot density) |

`width`, `height` and `position` are expressions, so layouts follow the box they're drawn in (`e("box.w")`, `e("scale.x(d.v)")`).
