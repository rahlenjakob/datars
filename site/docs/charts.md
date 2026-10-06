---
title: Charts, scales and marks
description: How datars charts are put together — plot and the marks inside it, charts that stand alone, small multiples, how scales are built from data, and the primitives underneath.
lede: Most charts are a `plot` with marks inside it. When a chart type isn't enough, the same primitives the standard library uses are yours.
---

## A plot and its marks

`plot` sets up a cartesian frame — x and y scales from your data, axes and gridlines sized to their labels, a title — and draws its `children` inside it. Marks inherit the plot's `data`, `x`, `y` and `color`, so they usually need no settings at all:

```ts
import { plot, bar, line, area, rule } from "@datars/std";

plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share, 2022 (%)",
  children: [bar({ labels: true })] })

plot({ data: "prices", x: "day", y: "price", xType: "linear", zero: false, format: ".0f",
  children: [area({ opacity: 0.18 }), line({ width: 2 }), rule({ value: 80, label: "Average" })] })
```

Which way bars point follows the scales: a band scale on x gives vertical bars, a band scale on y (`xType: "linear", yType: "band"`) horizontal ones.

<figure class="fig"><div class="frame"><div class="chart" data-chart="business" data-caption></div></div><figcaption>Three plots in one document — a waterfall, a funnel and stacked bars. Step through them: every bar morphs into its next shape.</figcaption>{{alt:business}}</figure>

The plot's own settings, from `datars describe std/plot`:

| Setting | Does |
|---|---|
| `x`, `y`, `color` | the fields on each axis and in colour |
| `xType`, `yType` | `band`, `point`, `linear`, `log`, `sqrt` or `time` (default: band x, linear y) |
| `colorType` | `categorical` (default), `sequential`, `diverging` or `piecewise` (with `stops: "#bae6fd 7 · #0b1f3a 14.2"`) |
| `xDomain`, `yDomain` | explicit domains, e.g. `[0, 35]` — exact, never rounded |
| `zero`, `nice` | include zero in the value axis; round linear domains to nice numbers (both on by default) |
| `title`, `subtitle`, `xLabel`, `yLabel` | text around the frame |
| `format`, `prefix`, `suffix` | value-axis labels: `format: ".0%"`, `prefix: "$"`, `suffix: " kr"` |
| `axes`, `grid` | which axes to draw (`both`, `x`, `y`, `none`); gridlines along the value axis |
| `legend` | a colour legend below the plot |
| `right` | a second value axis, scale `y2`: `{ y, domain?, label?, format? }`; marks opt in with `yScale: "y2"` |
| `brush` | a signal to brush into — see [interaction](/docs/interaction/#brushing) |
| `clip`, `padding`, `labelSpace` | clip marks to the plot area; band padding; room for line-end labels |

### Marks inside a plot

| Mark | Draws |
|---|---|
| [`bar`](/docs/std/bar/) | bars from a band axis to a value, with optional value labels |
| [`line`](/docs/std/line/) | one line per series, drawn on as it enters; `labels` at the line ends |
| [`area`](/docs/std/area/) | filled areas from a baseline |
| [`point`](/docs/std/point/) | a dot per row, instanced — tens of thousands are fine |
| [`dot`](/docs/std/dot/) | a dot per category (a dot plot) |
| [`grouped`](/docs/std/grouped/), [`stacked`](/docs/std/stacked/) | bars side by side, or stacked (`offset: "expand"` for 100 %) |
| [`cell`](/docs/std/cell/) | heatmap cells on two band axes |
| [`waterfall`](/docs/std/waterfall/), [`pareto`](/docs/std/pareto/) | a bridge from a running total; a running-share line |
| [`rule`](/docs/std/rule/), [`span`](/docs/std/span/), [`annotate`](/docs/std/annotate/) | a reference line, a shaded band, a callout |

### Charts that stand alone

Parts of a whole, flows and distributions are recipes of their own, drawn in the box they're given: [`pie`](/docs/std/pie/) (donut with `inner`), [`treemap`](/docs/std/treemap/), [`waffle`](/docs/std/waffle/), [`hemicycle`](/docs/std/hemicycle/), [`funnel`](/docs/std/funnel/), [`sankey`](/docs/std/sankey/), [`swarm`](/docs/std/swarm/), [`stripes`](/docs/std/stripes/), [`calendar`](/docs/std/calendar/). Maps are [`map`](/docs/std/map/) and friends ([maps](/docs/maps/)); millions of points are [`cloud`](/docs/std/cloud/) ([big data](/docs/big-data/)).

```ts
pie({ data: "votes", value: "share", category: "party", inner: 0.58, total: true })
sankey({ data: "flows", source: "source", target: "target", value: "ej" })
```

### Small multiples

[`facet`](/docs/std/facet/) repeats a chart once per value of a field, in a grid, on shared scales so panels compare at a glance:

```ts
facet({ data: "sales", by: "region", columns: 2,
  chart: plot({ x: "month", y: "sales", xType: "linear", children: [line()] }) })
```

Every recipe's full settings — with defaults, the theme tokens it reads and a live example — are in [the chart reference](/docs/std/).

## Scales

A scale maps data values to positions, sizes or colours. `plot` builds its scales for you — `x` and `y` from its fields over the plot area, `color` from the colour field over the theme's palette — and marks read them in expressions: `scale.x(d.party)`, `scale.x.bandwidth()`, `scale.color(d.party)`.

Any group can declare scales of its own for everything inside it, with `scales`:

```ts
group({
  key: "chart",
  scales: {
    x: { type: "band", domain: { data: "votes", field: "party" }, range: "width", padding: 0.2 },
    y: { type: "linear", domain: { data: "votes", field: "share" }, range: "-height", zero: true, nice: true },
    c: { type: "diverging", domain: { data: "temps", field: "anomaly" }, range: "$diverging", mid: 0 },
  },
  children: [ /* marks using scale.x(…), scale.y(…), scale.c(…) */ ],
})
```

| Part | Options |
|---|---|
| `type` | positions: `linear`, `log`, `sqrt`, `symlog`, `time`, `band`, `point`; colours and classes: `categorical`, `ordinal`, `sequential`, `diverging`, `piecewise`, `threshold`, `quantize`, `quantile` |
| `domain` | a literal list (`[0, 100]`, `["S", "M"]`); or from data: `{ data, field }`, `{ data, fields: [...] }` (several columns), `{ data, expr }` |
| `range` | `"width"`, `"height"`, `"-width"`, `"-height"` (the box), `[a, b]` in pixels; for colours a palette token (`"$categorical"`, `"$sequential"`, `"$diverging"`) or a list of colours |
| options | `zero`, `nice`, `padding` (bands), `mid` (diverging), `stops` (piecewise), `thresholds` (threshold), `clamp` |

Two behaviours worth knowing:

- **An explicit domain is exact** — no zero, no rounding. That's how you zoom (with `clip`) or fix an axis across states so it doesn't rescale.
- **Categorical colours are identities.** Their domain comes from the whole source, so filtering a table never reassigns colours; key metadata colours override the palette.

A scale can be read in reverse (`scale.x.invert(px)`), formatted like its axis (`scale.x.label(v)`), and asked for its range ends (`scale.y.min()`, `scale.y.max()`) — see [expressions](/docs/expressions/#scales).

## Primitives

Underneath every recipe are a handful of node types. Use them directly for annotations, custom layouts, or a whole chart of your own — the [SDK reference](/docs/sdk/nodes/) has every option of each, with live figures:

| Node | Draws |
|---|---|
| `group({ children, layout, scales, coord })` | nothing itself: lays out, transforms, clips, scopes scales and coordinate systems |
| `shape(geom, { fill, stroke })` | one shape: `geom.rect`, `circle`, `ellipse`, `arc`, `segment`, `polyline`, `area`, `path`, `symbol`, `feature` (a geo feature) |
| `text(content, [x, y], { style, halo })` | text, shaped and wrapped (`style.maxWidth`) by the engine |
| `repeat(table, template)` | the template once per row (`d` is the row); also per group, per scale tick, per legend entry, or `count` times |
| `instances({ from, x, y, r, fill })` | one instanced mark per row — for thousands to millions of points (`lod` for level of detail) |
| `view({ camera, children })` | a camera: fit a box, a lon/lat box or some keys, and optionally let the reader pan and zoom (`explore`) |
| `tiles({ source, layers })` | features from a vector-tile archive for what the view shows |
| `use(recipe, params)` | a recipe instance — what `plot(…)` and every other recipe call returns |

```ts
import { e, group, repeat, shape, geom, text } from "@datars/sdk";

// A lollipop per row, from primitives, inside a plot's scales.
repeat("votes", group({ key: e("d.party"), children: [
  shape(geom.segment({ x1: e("scale.x(d.party) + scale.x.bandwidth() / 2"), y1: e("scale.y(0)"),
                       x2: e("scale.x(d.party) + scale.x.bandwidth() / 2"), y2: e("scale.y(d.share)") }),
        { key: "stem", stroke: { paint: "$muted", width: 1.5 } }),
  shape(geom.circle({ cx: e("scale.x(d.party) + scale.x.bandwidth() / 2"), cy: e("scale.y(d.share)"), r: 5 }),
        { key: "head", fill: e("scale.color(d.party)"),
          semantics: { role: "datum", label: e("`${key.name(d.party)}: ${d.share}`") }, pickable: true }),
] }))
```

Give every mark drawn from a row a key from that row (`key: e("d.party")`) so it morphs, and `semantics` with a `role` and a `label` so hovering and screen readers can tell what it is. To package this as a reusable chart type — with typed parameters, docs and motion defaults — write a recipe: [custom recipes and scenes](/docs/custom-recipes/).
