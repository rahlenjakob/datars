---
title: Programs
titleTag: Programs — SDK reference · datars docs
description: Programs in @datars/sdk — step, the story, scrolly, autoplay, loop, film, interactive and dashboard presets, chapters for drill-downs, and drivers — with a live three-step story.
lede: A program is a small statechart: named states, each setting a few signals, and what moves the reader between them. The presets build the common ones; the engine plans and plays every transition.
---

## Steps

{{sdk:step}}

{{sdk:StepOpts}}

A step is a state: a `name` (what `goto:<name>` and the step pills use), the signals it `set`s, how long it `hold`s in autoplay and films (default 2.5 s), and its narration — `title` and `text`, with an `anchor` naming a node anchor the host can place the narration by. The scene reads the narration as the signals `narration.title` and `narration.text`, so it can draw its own caption; hosts also show it (this site under each chart), screen readers announce it and `datars video` writes it as captions.

{{sdk:Step}}

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-program-story" data-caption></div></div><figcaption>A three-step <code>story</code>: the steps set <code>focus</code> and <code>sorted</code>; the bars and the narration at the top read them. <a href="{{src}}/site/figures/sdk/program-story.ts">Figure source</a></figcaption>{{alt:sdk-program-story}}</figure>

## Presets

{{sdk:story}}

Next and previous move through the steps — the host's controls, arrow keys, taps, or events (`next`, `prev`, `goto:<state>`, `back`) from an [action](/docs/sdk/options/#Action) or the page (`view.send("goto:sorted")`). `loop: true` adds an edge from the last step back to the first.

{{sdk:scrolly}}

The page's scroll position scrubs through the steps: halfway between two steps is exactly halfway through that transition. On the web, `<datars-view scrub>` maps its scroll container's progress to the program ([scroll stories](/docs/stories/#scroll-stories-on-the-web)).

{{sdk:autoplay}}

{{sdk:loop}}

`loop(steps)` is an autoplaying story that wraps around — for ambient charts. Autoplay runs only while the chart is visible, and not at all for readers who ask for reduced motion.

{{sdk:film}}

Every step held for its `hold` and played frame-exactly: what `datars video` renders.

{{sdk:interactive}}

{{sdk:dashboard}}

One state, `main`: the reader drives the chart through signals — sliders, brushes, selections — rather than steps. `dashboard()` says the same for several linked views.

## Chapters

{{sdk:chapter}}

{{sdk:Chapter}}

A chapter is a sub-story entered for one key — click a country, get its steps, `back` to return. `{param}` in a chapter's step names, `set` values and narration is replaced by the key:

```ts
program: story({
  steps: [step("europe", { set: { scene: "europe" }, title: "Pick a country" })],
  chapters: {
    country: chapter("c", [
      step("{c}-zoom",  { set: { focus: "{c}", scene: "zoom" } }),
      step("{c}-trend", { set: { focus: "{c}", scene: "trend" }, title: "{c} by year" }),
    ]),
  },
}),
// … a mark enters it: on: { activate: { chapter: "country", key: e("d.id") } }
```

## The program object

{{sdk:Program}}

The presets fill `preset` (a name for tools), `states`, `drivers` and, for loops, `edges`. You can write one by hand: `initial` names the state to open on (default the first).

{{sdk:Driver}}

| Driver | Moves the program |
|---|---|
| `"steps"` | the host's step controls |
| `"keys"` | arrow keys |
| `"autoplay"` | each state holds for its `hold`, then next — run by the engine, paused by hosts off screen, on interaction and for reduced motion |
| `{ scroll: "scrub" }`, `{ scroll: "trigger" }` | the page's scroll: exact scrubbing, or a transition when a step's element crosses the middle of the viewport |
| `{ timer: seconds }` | recorded in the document; no host acts on it yet |

The engine acts on `autoplay` itself; the others tell hosts how the author meant the program to be driven. The guide: [States and stories](/docs/stories/).
