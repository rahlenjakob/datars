---
title: Nodes
titleTag: Nodes — SDK reference · datars docs
description: The scene builders of @datars/sdk — group, shape and every geom kind, text, repeat, instances, view, tiles and use — with live figures of geometry, strokes, text, layouts, repeats, instances and cameras.
lede: A scene is a tree of nodes. Each builder returns a plain JSON template — literal where it can be, expressions where it depends on data — that the engine resolves into marks every frame. Options that every node takes are on [Node options and scales](/docs/sdk/options/).
---

## Groups

{{sdk:group}}

A group draws nothing itself. It holds children and gives them what they share: a box to lay out in, a transform, a clip, opacity, scales, a coordinate system, a key prefix. `null`, `undefined` and `false` children are dropped, so `cond && node` works in a list.

```ts
group({ key: "root", layout: { type: "rows", gap: 8, padding: 16 }, children: [
  text("Sales by region", [0, 0], { key: "title", size: { h: "auto" } }),
  group({ key: "chart", scales: { /* … */ }, children: [ /* marks */ ] }),
] })
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-group-layouts" data-phone-ratio="1.67" data-caption></div></div><figcaption>The five <code>layout.type</code>s, each a group with <code>padding: 8, gap: 6</code> (its box tinted). Children fill what they're given — except in a flow, where each is as wide as what it draws. Sizes and alignment: <a href="/docs/sdk/options/#layout">Layout</a>. <a href="{{src}}/site/figures/sdk/group-layouts.ts">Figure source</a></figcaption>{{alt:sdk-group-layouts}}</figure>

## Shapes and geometry

{{sdk:shape}}

One drawable: a geometry, a `fill` and a `stroke` (both inks — `"$accent"`, `"#e8112d"`, or an expression returning one). `markers` puts an arrow or a dot on a line's ends: `{ start: { type: "dot", r: 4 }, end: { type: "arrow", size: 8 } }`.

```ts
shape(geom.rect({
  x: e("scale.x(d.region)"), w: e("scale.x.bandwidth()"),
  y: e("scale.y(d.v)"),      h: e("scale.y(0) - scale.y(d.v)"), r: 2,
}), {
  key: e("d.region"), fill: "$accent",
  semantics: { role: "datum", label: e("d.region + ': ' + d.v") },
})
```

{{sdk:geom}}

| Geometry | Draws |
|---|---|
| {{sig:geom.rect}} | a rectangle from its top-left corner; `r` rounds the corners |
| {{sig:geom.circle}} | a circle |
| {{sig:geom.ellipse}} | an ellipse with radii `rx`, `ry` |
| {{sig:geom.arc}} | an annular sector from `r0` (default 0: a pie slice) to `r1`, angles `a0` → `a1` in radians, clockwise from 12 o'clock |
| {{sig:geom.segment}} | a line between two points |
| {{sig:geom.polyline}} | a line through the rows of table `from` (`"@group"` inside a grouped repeat); `curve`: `"linear"` (default), `"monotone"`, `"catmull-rom"` (or `"smooth"`), `"step"`, `"step-before"`, `"step-after"`; `closed` joins the ends |
| {{sig:geom.area}} | the band between `y0` and `y1` through the rows of `from` |
| {{sig:geom.path}} | SVG path data — literal, or an expression that builds it (a sankey ribbon, a geodesic) |
| {{sig:geom.symbol}} | a symbol of radius `size` at `x, y`: `"circle"`, `"square"`, `"diamond"`, `"triangle"`, `"cross"`, `"star"` |
| {{sig:geom.feature}} | a region of a geo source by id, projected through the enclosing `coord: { type: "geo" }` |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-shape-geoms" data-phone-ratio="1.39" data-caption></div></div><figcaption>Every geometry kind, one per cell, each placed with expressions over its cell's box (<code>box.w</code>, <code>box.h</code>) so the figure re-lays itself out on a phone. The feature is an inline GeoJSON polygon through an equirectangular projection. <a href="{{src}}/site/figures/sdk/shape-geoms.ts">Figure source</a></figcaption>{{alt:sdk-shape-geoms}}</figure>

Any two geometries morph into each other: parametric pairs (rect to rect) interpolate their parameters, different kinds blend through an area-matched outline — see [`Rule.morph`](/docs/sdk/motion/#Rule).

{{sdk:Geom}}

{{sdk:StrokeOpts}}

`width` is in px of the node's space; with `nonScaling` it stays px-wide under a zooming camera (borders and roads on a map). `dash` is a list of on/off lengths.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-shape-strokes" data-phone-ratio="1.78" data-caption></div></div><figcaption>Stroke widths, dash patterns, caps (the dashed guides are the segment's ends: round and square caps reach past them), joins, and markers. <a href="{{src}}/site/figures/sdk/shape-strokes.ts">Figure source</a></figcaption>{{alt:sdk-shape-strokes}}</figure>

## Labels and text

{{sdk:text}}

Text is shaped, wrapped and measured by the engine with the fonts the chart carries, so it lays out the same on every platform. `content` is a string or an expression; `at` is the anchor point. `halo` is `[ink, width]`: an outline in the ink behind the letters, so a label reads over marks. `rotate` is in degrees about `at`; `offset` shifts the text in screen px from `at` (it stays beside its point at any camera zoom).

`number: { value, format }` draws a number instead of `content`, formatted each frame with a [d3-format](/docs/expressions/#text-and-numbers) spec in the document's locale — so when the value changes, the label counts through the numbers between.

{{sdk:TextStyleOpts}}

`font` is a font token (`"font.body"`, `"font.strong"`, `"font.title"`, `"font.number"`, or your theme's own) or a family name; `size` and `weight` are numbers or tokens (`"$size.label"`). `align` is `"start"`, `"middle"` or `"end"` (`"center"`, `"right"` also read); `baseline` is `"top"`, `"middle"`, `"alphabetic"` (default) or `"bottom"`.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-text-anchors" data-phone-ratio="0.83" data-caption></div></div><figcaption>Where a text sits relative to <code>at</code> (the dot): <code>align</code> across, <code>baseline</code> up and down. <a href="{{src}}/site/figures/sdk/text-anchors.ts">Figure source</a></figcaption>{{alt:sdk-text-anchors}}</figure>

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-text-options" data-phone-ratio="1.5" data-caption></div></div><figcaption>Font tokens, wrapping at <code>maxWidth</code> (dashed), a halo over busy marks, rotation, a screen offset — and <code>number</code>: step through and every format counts to the new value. <a href="{{src}}/site/figures/sdk/text-options.ts">Figure source</a></figcaption>{{alt:sdk-text-options}}</figure>

Texts never pile up if you ask: `contain: true` nudges a label back inside the canvas, and a group's [`declutter`](/docs/sdk/options/#NodeOpts) drops texts that would land on earlier ones.

## Repeats

{{sdk:repeat}}

The template once per item of `from`; inside it, `d` is the item. Give the template a key from the item (`key: e("d.region")`) so its marks keep their identity when the rows change.

{{sdk:RepeatFrom}}

| `from` | One child per | `d` has |
|---|---|---|
| `"sales"` | row of a table | the row's columns |
| `{ groups: "sales", by: "region" }` | group of rows | the group's first row; `"@group"` names its rows (for `geom.polyline({ from: "@group" })`, `group.sum("v")`) |
| `{ ticks: "x", count: 5 }` | tick of a scale (`count` is a hint, and may be an expression) | `value`, `label`, `pos`, `index`, `count`, `kind`, `major` |
| `{ legend: "color" }` | entry of a colour scale's domain | `value`, `ink`, `index` |
| `{ count: 12 }` | number | `index` |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-repeat-forms" data-phone-ratio="1.22" data-caption></div></div><figcaption>The five forms of <code>from</code>: bars from rows, a line per group, ticks from a scale, legend entries from a colour scale, and dots from a count. <a href="{{src}}/site/figures/sdk/repeat-forms.ts">Figure source</a></figcaption>{{alt:sdk-repeat-forms}}</figure>

A repeat inside a `grid` layout fills one cell per item — small multiples from a grouped repeat.

## Thousands of marks

{{sdk:instances}}

One prototype mark per row of `from`, drawn in one go: thousands to millions of points. Each column (`x`, `y`, `r`, `fill`, `opacity`) is an expression over the row; `instanceKey` gives every instance its own identity, so points move to new positions instead of fading. `screenSize` keeps marks px-sized under a zooming camera.

{{sdk:InstancesOpts}}

`r` defaults to the theme's `point.radius`; `rect` prototypes take `w` and `h`. `opacity` here is per instance. `label` is each instance's accessible name and tooltip; `hit: "line"` finds the nearest instance along the line through them (a line chart's value wherever it's hovered). With `lod`, rows beyond a frame's worth are indexed once into a pyramid and each frame draws a density-preserving sample of what's in view — the [big data](/docs/big-data/) guide.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-instances-points" data-phone-ratio="0.83" data-caption></div></div><figcaption>Three thousand generated rows in one <code>instances</code> node. Step: <code>x</code> and <code>y</code> switch to a rank column and every point, keyed by <code>instanceKey</code>, travels to its place in its group's block. <a href="{{src}}/site/figures/sdk/instances-points.ts">Figure source</a></figcaption>{{alt:sdk-instances-points}}</figure>

## Views and cameras

{{sdk:view}}

A window onto content in its own units, through a camera. It clips to its box unless `clip: false`. The camera is either explicit (`{ x, y, zoom }`: the content point at the view's centre, and its scale) or a fit.

{{sdk:FitCamera}}

A `bbox` can read signals, and `{ keys: "=focus" }` frames the marks whose own keys are in a key-set signal — when the fit changes, the camera flies there. A story step can reset an explored view by setting its signals.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-view-camera" data-phone-ratio="0.83" data-caption></div></div><figcaption>A 1000 × 600 plane in a view whose camera fits the marks named in a key-set signal (<code>fit: { keys: "=focus" }</code>): the plane itself, the towns of the north-east, then two towns. Drag and zoom it (<code>explore</code>). The labels are pinned, so they keep their size; keyed <code>label-…</code>, they aren't among the keys the camera fits. <a href="{{src}}/site/figures/sdk/view-camera.ts">Figure source</a></figcaption>{{alt:sdk-view-camera}}</figure>

## Map tiles

{{sdk:tiles}}

The source is a `data.tiles(url)` or `data.tiles.auto()`; the standard [`basemap`](/docs/std/basemap/) is written with this node, and [maps](/docs/maps/) is the guide.

{{sdk:TileLayer}}

```ts
tiles({ key: "tiles", source: "basemap", layers: [
  { layer: "water", template: shape(geom.feature(), { fill: "$map.water" }) },
  { layer: "roads", minzoom: 7,
    template: shape(geom.feature(), {
      stroke: { paint: "$map.road-major", width: 1.5, nonScaling: true },
    }) },
  { layer: "places", labels: true, priority: e("d.pop ?? 0"),
    template: text(e("d.name"), [e("d.$x"), e("d.$y")], {
      key: e("d.name"), halo: ["$map.label-halo", 2.5],
      style: { size: 11.5, ink: "$map.label", align: "middle", baseline: "middle" },
    }) },
] })
```

## Recipe instances

{{sdk:use}}

`use("@datars/std/bar", { labels: true })` is exactly what calling the recipe — `bar({ labels: true })` — returns; `use` is for recipes you refer to by name. See [Recipes](/docs/sdk/recipes/).

## Types

{{sdk:Template}}

What every builder returns: a JSON object with a `kind` (`"group"`, `"shape"`, `"text"`, `"repeat"`, `"instances"`, `"view"`, `"tiles"`, `"use"`). You can write templates by hand, or receive them from any language that writes JSON.

{{sdk:Json}}
