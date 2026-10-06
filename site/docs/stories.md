---
title: States and stories
description: Tell stories with datars — programs and steps, narration and on-canvas cards, stepped, scroll-driven and autoplaying stories, films, and click-to-drill chapters.
lede: A story is a list of states. Each step sets a few signals; the engine plans every transition between them and plays it — forwards, backwards, scrubbed by the scroll, or rendered to video.
---

## Steps

A program's states are built with `step(name, options)`. A step sets signals, and can carry narration:

```ts
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pie, card } from "@datars/std";

export default doc({
  title: "Vote share by party, Sweden 2022",
  size: [720, 440],
  data: { votes: data.values({ party, share }, { key: "party" }) },
  tables: { ranked: { from: "votes", ops: [{ op: "sort", by: [["share", "desc"]] }] } },
  signals: { shape: signal.str("bars") },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "votes", x: "party", y: "share", color: "party", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
    plot({ data: "ranked", x: "share", y: "party", xType: "linear", yType: "band", color: "party", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "ranked"') }),
    pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
    card({ title: e("narration.title"), text: e("narration.text"), width: 230 }, { key: "caption" }),
  ] }),
  program: story({ steps: [
    step("bars",   { set: { shape: "bars" },   title: "30.3%", text: "for the Social Democrats, still Sweden's largest party." }),
    step("ranked", { set: { shape: "ranked" }, text: "Ranked: eight parties cleared the 4% threshold." }),
    step("pie",    { set: { shape: "pie" },    text: "Each bar becomes a slice." }),
  ] }),
});
```

| Option | Does |
|---|---|
| `set` | signals to set in this state: `{ shape: "pie", focus: ["SWE", "NOR"] }` |
| `title`, `text` | the step's narration (see [narration](#narration)) |
| `hold` | seconds this step holds in an autoplaying story or a film (default 2.5) |
| `anchor` | the name of an anchor in the scene the narration belongs to, for hosts that place narration next to the chart |

The pattern is always the same: a signal says what the step is (`shape`), and parts of the scene exist `when` it has their value. Because the parts share a key (`"chart"`) and their marks share the data's keys, each step's chart morphs into the next. A step can also change a filter, a camera's fit, a highlighted set, a brushed range, a year — anything a signal drives.

<figure class="fig"><div class="frame"><div class="chart" data-chart="votes" data-caption></div></div><figcaption>The story above, live: the steps under the chart, the narration drawn on the canvas by <code>card</code>.</figcaption>{{alt:votes}}</figure>

## Narration

A step's `title` and `text` are its narration. It reaches the reader three ways:

- **On the canvas**, with [`card`](/docs/std/card/): `card({ title: e("narration.title"), text: e("narration.text") })` draws one card that narrates every step, placed where it covers the least data. Because the engine draws it, it's in the video, the PNG and the PDF too, and a card whose text changes morphs.
- **In the host.** The web element's `state` event carries the narration, so a page can show it in its own typography (this site does, under each chart). The runtime also announces it to screen readers through a live region.
- **In captions.** `datars video` writes each step's narration as WebVTT captions ([video, images and PDF](/docs/export/)).

## Programs

The program says how a reader moves through the states. `@datars/sdk` has presets:

| Preset | Moves by |
|---|---|
| `story({ steps })` | next/previous — buttons, arrow keys, taps, or the host; `loop: true` wraps around |
| `scrolly({ steps })` | the page's scroll position, scrubbing through the transitions |
| `autoplay({ steps, loop })` | each step holding for its `hold`, then moving on |
| `film(steps)` | every step held, rendered frame-exactly (for video) |
| `loop(steps)` | autoplay, forever |
| `interactive()` | one state; the reader drives it through signals (an explorable) |
| `dashboard()` | one state, several linked views |

Autoplay runs only while the chart is visible, and is off entirely for readers who ask for reduced motion.

Programs respond to events — `next`, `prev`, `goto:<state>`, `back` — from the reader, from an [`on` action](/docs/interaction/#intents-and-actions), or from the host (`view.send("goto:pie")` on the web).

```sh
datars states doc.ts
```

{{run:states examples/votes/doc.json}}

## Scroll stories on the web

Two ways to tie a story to the page's scroll:

**Trigger** — the page's own text blocks are the steps. Each element matching `steps` is a step; when it crosses the middle of the viewport, the chart goes there (by its `data-state` name, or by position) and the transition plays at its own pace:

```html
<div class="sticky-chart"><datars-view src="c/renewables" steps=".step"></datars-view></div>
<div class="step" data-state="world">A world of renewables…</div>
<div class="step" data-state="europe">Fly to Europe…</div>
<div class="step" data-state="nordics">The Nordics lead…</div>
```

**Scrub** — the transition follows the scroll exactly. With the `scrub` attribute, the element's scroll container (its nearest `[data-scrub]` ancestor, else its parent) maps its passage through the viewport to the story's positions: halfway between two steps is exactly halfway through that transition, forwards or backwards. Pair it with `scrolly()` in the document.

```html
<section data-scrub style="height: 400vh">
  <datars-view src="c/descent" scrub style="position: sticky; top: 0"></datars-view>
</section>
```

The home page of this site uses a trigger story (the renewables map that becomes bars). Scrubbing is exact because frames are a pure function of time: there's no clock to run backwards.

## Chapters

A chapter is a drill-down: a sub-story entered for one key — click a country, get three steps about that country, `back` to return. Declare it with `chapter(param, steps)`; `{param}` in a step's `set` values and narration is replaced by the key:

```ts
import { chapter, step, story } from "@datars/sdk";

program: story({
  steps: [
    step("europe",  { set: { scene: "europe" },  title: "Pick a country" }),
    step("ranking", { set: { scene: "ranking" }, text: "Or pick one from the ranking." }),
  ],
  chapters: {
    country: chapter("c", [
      step("{c}-zoom",  { set: { focus: "{c}", scene: "zoom" } }),
      step("{c}-trend", { set: { focus: "{c}", scene: "trend" } }),
    ]),
  },
}),
```

Recipes enter chapters when clicked: `bar({ chapter: "country" })` and `map({ chapter: "country" })` enter the chapter with the clicked bar's category or region's id. Your own marks can do it with `on: { activate: { chapter: "country", key: e("d.id") } }`. The [Europe drill-down]({{src}}/site/charts/europe-drill.ts) is a complete example.

## Structure per state

`when` is how a state changes what exists. Anything can have one — a chart, an annotation, a legend, a whole view:

```ts
plot({ data: "votes", x: "party", y: "share", children: [
  bar(),
  annotate({ x: e("scale.x('SD') + scale.x.bandwidth() / 2"), y: e("scale.y(20.5)"), text: "Second largest" },
    { key: "note", when: e('state == "bars"') }),
] })
```

The engine also exposes the current state itself as signals: `state` (its name) and `step` (its index). Prefer your own signals (`shape`, `focus`) for anything more than a one-off: they describe *what* a step shows, so several steps can share it and the host can set it.

## Checking a story

```sh
datars film doc.ts --from bars --to pie     # a filmstrip of the transition, with motion trails
datars render doc.ts --state pie            # one state as a PNG
datars lint doc.ts                          # e.g. marks that lose their identity between steps
```

`datars dev` lists the states next to the live chart; click one to go there. See [motion](/docs/motion/) for shaping the transitions themselves.
