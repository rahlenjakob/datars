---
title: Node options and scales
titleTag: Node options, layout and scales — SDK reference · datars docs
description: The options every @datars/sdk node takes — key, when, opacity, transform, clip, trim, semantics, interaction, layout and size, scales, backdrops and dodging — with figures of layouts, sizes, scales and brushing.
lede: Every builder takes the same options as its last argument (`shape(g, { … })`, `text(s, at, { … })`, `group({ … })`). They say who a node is, whether it exists, how it's placed and what it means — to readers, screen readers and pointers.
---

## Every node

{{sdk:NodeOpts}}

What the rest do:

| Option | Does |
|---|---|
| `key` | the node's identity, scoped by its parent's: `"chart"`, or `e("d.region")` for a mark drawn from a row. Marks with the same key path pair across states and morph; without one they crossfade. |
| `when` | the node exists only while this is true: `e('view == "bars"')`. Two nodes with the same key and opposite `when`s are one mark that changes form. |
| `opacity`, `isolate` | fades the node and what's under it; with `isolate` a group fades as one layer (overlaps don't show through) |
| `transform` | `scale`, then `rotate` (degrees), then `translate`, about the node's origin |
| `z` | drawing order among siblings: higher on top (default 0, then source order) |
| `clip` | `"box"` (the node's layout box) or `[x, y, w, h]`; a `view` clips to its box by default |
| `trim` | `[from, to]`: the share of a shape's outline drawn — a line that draws on |
| `semantics` | what the node is: a [`role`](#Role), an accessible `label` (also its tooltip), a `value` (read by `choreo.stagger("value")` and by sliders), a `link` |
| `pickable`, `on` | whether the pointer finds it, and what its [intents](#interaction) do |
| `anchors` | named points reported with every frame, for hosts that place their own UI (a step's narration `anchor` names one) |
| `layout`, `size` | how a group lays out its children, and this node's size in its parent's [layout](#layout) |
| `scales` | [scales](#scales) visible by name to everything under this group |
| `id`, `prov` | a stable id for patching and tools; the recipe that made the node (set by recipe expansion) |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-node-options" data-phone-ratio="1.33" data-caption></div></div><figcaption>A faded group without and with <code>isolate</code>, a clipped circle, a line trimmed to 60 %, a scaled and rotated rectangle (its original dashed), and three squares ordered by <code>z</code> against their source order. <a href="{{src}}/site/figures/sdk/node-options.ts">Figure source</a></figcaption>{{alt:sdk-node-options}}</figure>

## Layout

{{sdk:Layout}}

A group with a `layout` gives each child a box; without one (or with `stack`) every child gets the whole box. Inside its box a child draws in local coordinates — `box.w` and `box.h` in its expressions are that box's size. `padding` is a number or CSS-style `[top, right, bottom, left]`.

{{sdk:Size}}

A child's `size` counts along the layout's axis — `h` in `rows`, `w` in `columns` — and `"fill"` is the default. Across the axis children stretch, unless `align` places them: then a fixed or `auto` size across is kept.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-group-sizes" data-phone-ratio="1.28" data-caption></div></div><figcaption>Five children sized <code>90</code>, <code>"22%"</code>, <code>"auto"</code>, <code>"fill"</code> and <code>{ fill: 2 }</code>, each showing its width. Step to a narrower box: the fixed and auto children keep theirs, the rest re-share. Below, <code>align</code> on children of fixed heights. <a href="{{src}}/site/figures/sdk/group-sizes.ts">Figure source</a></figcaption>{{alt:sdk-group-sizes}}</figure>

The guide, with responsive layouts and `sizeClass`: [Documents → Layout](/docs/documents/#layout).

## Scales

{{sdk:ScaleDecl}}

A group's `scales` declares named scales for everything under it; expressions read them as `scale.<name>(v)`, `scale.x.bandwidth()`, `scale.x.invert(px)`, `scale.x.ink(v)` and more ([expressions](/docs/expressions/#scales)). `domain` is a literal list, or from data — `{ data, field }`, `{ data, fields: [...] }`, `{ data, expr }`. `range` is `"width"`, `"height"`, `"-width"`, `"-height"` (the group's box), `[a, b]` in px (`"=box.w - 10"` is an expression), or for colour scales a palette token (`"$categorical"`, `"$sequential"`, `"$diverging"`) or a list of colours. Other options: `zero`, `nice`, `padding` (band, point), `mid` (diverging), `stops` (piecewise), `thresholds` (threshold), `clamp`. An explicit domain is exact: no zero, no rounding.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-scales-position" data-phone-ratio="1" data-caption></div></div><figcaption>Position scales as axes drawn with <code>repeat({ ticks: "x" })</code>. On a log axis only the powers of ten (<code>d.major</code>) are labelled; the band and point rows mark each category. On a phone the tick counts follow the shorter axes. <a href="{{src}}/site/figures/sdk/scales-position.ts">Figure source</a></figcaption>{{alt:sdk-scales-position}}</figure>

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-scales-color" data-phone-ratio="0.94" data-caption></div></div><figcaption>Colour scales: <code>scale.c(v)</code> returns an ink, resolved against the theme — switch the page to dark mode and the sequential ramp follows. <a href="{{src}}/site/figures/sdk/scales-color.ts">Figure source</a></figcaption>{{alt:sdk-scales-color}}</figure>

`pow` maps through `|x|^exponent` (`exponent`, default 1; `sqrt` is `pow` with 0.5). `ordinal`, `quantize` and `quantile` take their colours from a palette token in `range`.

## Semantics

{{sdk:Role}}

A role says what a node is to screen readers, keyboard navigation, tooltips and tools: `datum` and `region` marks are the data (arrow keys move across them, and `datars lint` checks their keys and labels), `series` a line or area, `axis`, `tick`, `grid`, `legend`, `legend-item`, `annotation`, `title`, `label`, `tooltip` and `control` what their names say. `decoration` is hidden from assistive technology. Roles also select elements in [motion rules](/docs/sdk/motion/#Rule) (`select: { role: "datum" }`).

## Interaction

{{sdk:Intent}}

Intents are device-neutral: `inspect` is hover, touch or keyboard focus; `activate` a click, tap, Enter or an assistive double-tap; `brush` a drag across; `drag` a press and drag; `pan` and `zoom` what an explorable view does; `pick` a choice a host may offer with the platform's own picker (below). A node answers them with `on: { <intent>: action }`; set `pickable: true` so the pointer finds it.

{{sdk:Action}}

| Action | Does |
|---|---|
| `{ set: "focus", value: e("d.region") }` | sets a signal to a value (an expression over the row) |
| `{ toggle: "selected", value: e("d.region") }` | adds or removes a key in a key-set signal |
| `{ event: "next" }` | sends the program an event: `next`, `prev`, `goto:<state>`, `back` |
| `{ chapter: "country", key: e("d.id") }` | enters a [chapter](/docs/sdk/programs/#chapter) for this key |
| `brush("range")` | selects a range while dragging (below) |
| `scrub("at", { step: 1 })` | sets a number to the value under the pointer (below) |
| `pick("country", ["SE", "NO"])` | offers a choice to the host's own picker (below) |

{{sdk:scrub}}

{{sdk:pick}}

A `pick` is what lets a phone show its own select: the host reads the choices from the chart's controls and lays the platform's picker over the node — a native `<select>` on touch screens in the browser, a menu on iOS, a list dialog on Android — and sets the signal to what the reader chooses. With a mouse the node's `activate` runs as usual (std's `select` opens its drawn list).

{{sdk:brush}}

{{sdk:brushed}}

```ts
signals: { range: signal.range(), at: signal.num(8) },
tables: { inside: table("dots", op.filter(brushed("range", "d.v"))) },
// …
group({
  key: "strip",
  scales: { x: { type: "linear", domain: [0, 100], range: "width" } },
  on: { brush: brush("range", "x") },
  children: [ /* dots; a rect from scale.x(range.lo) to scale.x(range.hi) */ ],
})
group({
  key: "chart",
  scales: { /* x, y */ },
  on: { drag: scrub("at", { step: 1 }) }, pickable: true,
  children: [ /* the line; a cursor at scale.x(at) */ ],
})
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-interaction-brush" data-phone-ratio="0.83" data-caption></div></div><figcaption>Drag across the dots to brush a range (click to clear); press or drag on the line to scrub. The second step sets both signals, as a story step can. <a href="{{src}}/site/figures/sdk/interaction-brush.ts">Figure source</a></figcaption>{{alt:sdk-interaction-brush}}</figure>

The guide: [Signals and interaction](/docs/interaction/).
