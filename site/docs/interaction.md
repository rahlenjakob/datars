---
title: Signals and interaction
description: Make datars charts respond — signals, hover tooltips, click to select or drill, brushing, sliders, dropdowns, switches and other controls, linked views, explorable maps and plots, and talking to the page or app around the chart.
lede: Interactions write signals; the scene reads them. That one loop gives you selection, filtering, brushing, sliders and linked views — with the same behaviour on the web, in apps and on touch screens.
---

## Signals

Declare the values your chart depends on in `signals`:

```ts
import { signal } from "@datars/sdk";

signals: {
  shape:    signal.str("bars"),     // a string
  income:   signal.num(32000),      // a number
  labels:   signal.bool(true),      // a boolean
  selected: signal.keyset(),        // a set of keys (a selection)
  focus:    signal.key(),           // one key, or null
  range:    signal.range(),         // a brushed range: range.lo, range.hi, range.active
  spin:     signal.clock(8),        // counts up 8 units a second while on screen
}
```

Anything can read a signal: a mark's expression (`opacity: e("selected.isEmpty() || selected.has(d.region) ? 1 : 0.3")`), a table's filter, a node's `when`, a camera's fit. When a signal changes, what depends on it is recomputed and the change animates — filtered-out bars shrink away, the rest slide into their new places.

Signals are set by story steps ([states and stories](/docs/stories/)), by the interactions below, and by the page or app around the chart.

## Hover and tooltips

Hovering (or touching, or focusing with the keyboard) a mark shows its label as a tooltip. The label is the mark's accessible name, so tooltips and screen readers always agree. Recipes build a sensible one (`Sweden Democrats: 20.5`); most let you write your own:

```ts
bar({ label: e("`${key.name(d.party)}: ${format(d.share, '.1f')} %`") })
```

On your own marks, set `semantics: { role: "datum", label }` and `pickable: true`. While the pointer is over a mark, the built-in signal `inspected` holds its key.

## Click to select

Many recipes take a `selected` signal: clicking a bar or a line toggles its key in the set, selected marks stay strong and the rest recede.

```ts
signals: { selected: signal.keyset() },
tables: {
  totals: { from: "sales", ops: [op.aggregate(["region"], { total: ["sum", "sales"] })] },
  trend:  { from: "sales", ops: [op.filter(e("selected.isEmpty() || selected.has(d.region)"))] },
},
scene: group({ key: "root", layout: { type: "columns", gap: 24 }, children: [
  plot({ data: "totals", x: "region", y: "total", color: "region", children: [bar({ selected: "selected" })] }, { key: "totals", size: { w: "40%" } }),
  plot({ data: "trend", x: "month", y: "sales", xType: "linear", color: "region", children: [line({ points: true })] }, { key: "trend" }),
] }),
```

That's a complete cross-filter dashboard: signals are global, so the second chart's table filters on what the first one selects, and its lines morph as series come and go. No glue code.

<figure class="fig"><div class="frame"><div class="chart" data-chart="dashboard"></div></div><figcaption>Click a region's bar to filter the monthly trend; click again to clear it.</figcaption>{{alt:dashboard}}</figure>

## Intents and actions

Underneath, every interaction is an **intent** bound to an **action** with `on`. Intents are device-neutral: `activate` is a click, a tap, Enter, or a double-tap with VoiceOver or TalkBack; `inspect` is hover, touch or focus.

```ts
shape(geom.rect({ /* … */ }), {
  key: e("d.region"),
  pickable: true,
  on: { activate: { toggle: "selected", value: e("d.region") } },
})
```

| Intent | When |
|---|---|
| `inspect` | the pointer (or focus) is over the mark |
| `activate` | click, tap, Enter, or an assistive double-tap |
| `brush` | a drag across the node selects a range |
| `drag` | press and drag over the node (sliders, scrubbers) |

| Action | Does |
|---|---|
| `{ set: "signal", value }` | sets a signal to a value (an expression over the row) |
| `{ toggle: "signal", value }` | adds or removes a key in a key-set signal |
| `{ event: "next" }` | sends the program an event: `next`, `prev`, `goto:<state>`, `back` |
| `{ chapter: "name", key }` | enters a drill-down chapter for this key (see [chapters](/docs/stories/#chapters)) |
| `brush(signal, axis)` | selects a range while dragging (below) |
| `scrub(signal, { axis, step })` | sets a number to the value under the pointer while dragging |

## Brushing

A plot with `brush` lets the reader drag across it to select a range; `brushed()` keeps the rows inside it. Overview and detail in a few lines:

```ts
import { brushed, signal } from "@datars/sdk";

signals: { range: signal.range() },
tables: { detail: { from: "prices", ops: [op.filter(brushed("range", "d.day"))] } },
scene: group({ key: "root", layout: { type: "rows", gap: 10 }, children: [
  plot({ data: "detail", x: "day", y: "price", xType: "linear", children: [area({ opacity: 0.18 }), line()] }, { key: "detail" }),
  plot({ data: "prices", x: "day", y: "price", xType: "linear", brush: "range",
    children: [area({ opacity: 0.3 }), line({ width: 1 })] }, { key: "overview", size: { h: 130 } }),
] }),
```

On a continuous axis the brush writes `range.lo` and `range.hi` in data units and `range.active`; on a band axis it writes the key set `range`. A click without a drag clears it. Story steps can set a brush too — `set: { "range.active": true, "range.lo": 1, "range.hi": 59 }` — which is how the [prices example]({{src}}/examples/prices/doc.ts) tours the year.

## Controls

The standard library draws the controls a chart needs on its own canvas. The engine draws them, so they look and work the same on the web, in apps and in a video export; they answer at the pace of a UI (a fifth of a second), not a chart; they show no tooltips; and each is offered to keyboards and screen readers as a native control — a range input for a slider's thumb, a button for everything you click.

| Recipe | Sets | Use it for |
|---|---|---|
| [`slider`](/docs/std/slider/) | a number | an amount, a year, a rate |
| [`range`](/docs/std/range/) | two numbers, `lo` ≤ `hi` | a span of years or values; the thumbs can't cross |
| [`segmented`](/docs/std/segmented/) | one of a few values | a view, a metric, a unit — two to five options |
| [`select`](/docs/std/select/) | one of many values | a country, a product; the list floats over the chart |
| [`toggle`](/docs/std/toggle/) | a boolean | show a comparison, labels, a reference line |
| [`checklist`](/docs/std/checklist/) | a key set | which series or regions to show |
| [`button`](/docs/std/button/) | an event or a value | Previous and Next, a reset, a preset |

<figure class="fig"><div class="frame"><div class="chart" data-chart="controls" data-caption></div></div><figcaption><b>Every control, one chart.</b> A segmented quarter, a dropdown highlight, a switch for an average line, checkboxes for the regions, a range of values and a reset button — all engine-drawn signals the bars read.</figcaption>{{alt:controls}}</figure>

```ts
signals: { quarter: signal.str("q1"), region: signal.str("North"), shown: signal.keyset(regions), lo: signal.num(0), hi: signal.num(50) },
tables: { view: { from: "sales", ops: [op.filter(e("shown.has(d.region) && d.value >= lo && d.value <= hi"))] } },
scene: group({ key: "root", layout: { type: "columns", gap: 24 }, children: [
  group({ key: "panel", size: { w: 230 }, layout: { type: "rows", gap: 16 }, children: [
    segmented({ signal: "quarter", options: ["q1", "q2"], labels: ["Q1", "Q2"], label: "Quarter" }),
    select({ signal: "region", options: regions, label: "Highlight" }),
    checklist({ signal: "shown", options: regions, label: "Regions" }),
    range({ lo: "lo", hi: "hi", min: 0, max: 50, label: "Sales between" }),
    button({ label: "Show every region", set: "shown", value: regions }),
  ] }),
  plot({ data: "view", x: "region", y: "value", children: [bar({ fill: e('d.region == region ? "$accent" : "$muted@0.4"') })] }),
] }),
program: interactive(),
```

Option labels come from the document's `keys` when you don't give `labels` (a key `SE` with `name: "Sweden"` says Sweden). A `select` near the bottom of a chart opens upward with `open: "up"`, and a long list opens in columns (`rows`, 6 by default) so it fits a small chart. Its list floats above the whole scene, like any node with `z` of 1000 or more — use that for popovers of your own.

On phones and tablets a `select` opens the platform's own picker instead of the drawn list: a native `<select>` in the browser (the list or wheel the phone shows for any web form), a menu on iOS, a list dialog on Android — bigger, scrollable, and what readers and their screen readers already know. The chart declares the choices (the `pick` intent, see the [SDK reference](/docs/sdk/options/#interaction)); each host lays its picker over the box and sets the signal to what's chosen.

With a mouse, controls show where the pointer is — a wash over the option or button under it, a halo round a thumb — through the `hover()` expression, and the pointer turns into a hand over anything a click acts on (a grab hand over a view that pans, a crosshair over a brushable area). Hosts show the cursor the engine asks for (`engine.cursor()`).

A control is a recipe like any other: `datars eject std/select` copies it into your project to restyle. The page or app can also drive the same signals with its own inputs — `view.setSignal("region", "SE")` — see [talking to the page](#talking-to-the-page-or-app).

> **Note** Signals also accept a `control` (`signal.num(30000, control.slider(10000, 80000, 1000))`). It is recorded in the document as a hint for hosts and editors; no host draws it — use the recipes above to put a control on the canvas.

## Explorable views

Give a view's camera an `explore` name and readers can pan (drag) and zoom (wheel or pinch) it, between `minZoom` and `maxZoom` times the fitted view:

```ts
view({
  coord: { type: "geo", projection: "web-mercator" },
  camera: { fit: { geo: [10.5, 55.2, 24.5, 69.2] }, explore: "map", maxZoom: 40 },   // west, south, east, north
  children: [ /* a map, a scatter, anything */ ],
})
```

A camera can also fit a box in its own units (`fit: { bbox: [x0, y0, x1, y1] }`) or the marks with some keys (`fit: { keys: ["SWE", "NOR"] }`, or `keys: "=focus"` to follow a key-set signal).

The reader's position lives in signals — `map.x`, `map.y`, `map.zoom` — and a story step flies the camera back into the narrative. [`cloud`](/docs/std/cloud/) is explorable out of the box: that's how the four-million-star galaxy is browsed. See [maps](/docs/maps/) for cameras and flights.

## Talking to the page or app

The host can set signals and move the program, and hears when the state changes. On the web:

```html
<datars-view id="chart" src="c/dashboard"></datars-view>
<script type="module">
  const chart = document.getElementById("chart");
  chart.setSignal("selected", ["North", "East"]);     // a key set
  chart.setSignal("range", { lo: 10, hi: 40 });       // a range
  chart.setSignal("income", 45000);                   // a number
  chart.send("goto:ranked");                          // next, prev, goto:<state>, back
  chart.addEventListener("state", (e) => console.log(e.detail.state, e.detail.narration));
  chart.addEventListener("signal", (e) => console.log(e.detail.changed, e.detail.signals));  // what the reader did
  chart.addEventListener("pick", (e) => console.log(e.detail.row));                          // the row a click landed on
</script>
```

`setSignal` takes a number, string or boolean, an array of keys (a key set), `{ lo, hi }` (a range), or `null` to clear. `chart.signals` reads them back the same way — `range.lo`, `range.hi` and `range.active` for a brush, `map.x`, `map.y` and `map.zoom` for an explored view, `inspected` for the key under the pointer — and the `signal` event says when the reader changed one. In apps, DatarsKit's engine has `setSignal(name, value)` for numbers and `goto(index)`, and SwiftUI's `DatarsChart(source:state:)` follows your state; the Android view has `send(event)`, `goTo(index)` and `seek(position)`. Details: [web](/docs/embed/web/), [iOS and macOS](/docs/embed/ios/), [Android](/docs/embed/android/).
