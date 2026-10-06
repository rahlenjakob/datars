---
title: Core concepts
description: The seven ideas behind datars — documents, keys, signals, states, recipes, late-bound themes and bundles — and the one engine that plays them the same everywhere.
lede: Seven ideas carry everything in datars. Once they click, the rest of the docs are details.
---

## Documents

A chart is a **document**: its data, its derived tables, its signals, a scene of keyed marks, motion rules and a program of states. You write it in TypeScript with `@datars/sdk` and `@datars/std`, but what you produce is data — plain JSON the engine evaluates.

```ts
export default doc({
  title: "Sales by region",
  size: [720, 420],
  data: { sales: data.values({ region: ["N", "S", "E", "W"], sales: [120, 98, 143, 87] }, { key: "region" }) },
  scene: plot({ data: "sales", x: "region", y: "sales", children: [bar({ labels: true })] }),
});
```

Your code runs once, to *build* the document. Nothing you write runs per frame: positions, colours and labels are [expressions](/docs/expressions/) the engine evaluates in Rust. That's why a document can be validated, diffed (`datars diff`), upgraded (`datars migrate`), written by an agent, and rendered identically on every platform. More in [documents](/docs/documents/).

## Keys are identity

Every row has a **key** — `{ key: "region" }` — and every mark drawn from a row carries it. Keys are how the engine knows that the bar for “North” in one state is the slice for “North” in the next.

So when anything changes — a story step, a filter, new data, a resize, a theme — the engine matches keys and plans the motion: the bar morphs into its slice, a country's outline lifts off the map into its bar, a bar splits into one dot per unit. You write no animation code. Without keys, marks pair by position and the linter warns you.

<figure class="fig"><div class="frame"><div class="chart" data-chart="how-match" data-phone-ratio="1.33" data-caption></div></div><figcaption><b>Who becomes whom.</b> Step through: keyed bars pair across scenes and move, a new key grows in, a missing one shrinks away, and a bar whose key is the parent of six unit keys splits into six pieces — then merges back. No animation code, no matcher set: this is the default.</figcaption>{{alt:how-match}}</figure>

## Signals

A **signal** is a named value the scene reads: which chart is showing, the selected regions, a slider's value, a brushed range, the camera of an explorable map.

```ts
signals: { shape: signal.str("bars"), selected: signal.keyset() },
```

Marks read signals in expressions (`selected.has(d.region) ? 1 : 0.3`), tables filter on them, `when` shows or hides parts of the scene by them. Interactions write them: a click toggles a key, a drag sets a range, a slider sets a number. A host page or app can set them too. Change a signal and the scene re-resolves — and animates. See [signals and interaction](/docs/interaction/).

## States and programs

A **program** is a small state machine. A story is the common one: a list of steps, each setting a few signals.

```ts
program: story({ steps: [
  step("bars", { set: { shape: "bars" }, title: "Four parties" }),
  step("pie",  { set: { shape: "pie" },  title: "Shares of the whole" }),
] }),
```

The reader moves between states with buttons, arrow keys, taps, the page's scroll, or autoplay; each move is a transition the engine plans once and plays. Dashboards and explorables are programs with one state, driven by signals instead. See [states and stories](/docs/stories/).

## Recipes

The engine knows **primitives** — shapes, text, instances, tiles, views, scales, coordinates — never “bar chart”. Charts, axes, legends, maps and cards are **recipes**: TypeScript functions from typed parameters to a scene, written against the same public SDK you use.

The standard library, `@datars/std`, is {{count:recipes}} recipes ([the reference](/docs/std/)). When one doesn't do what you need, `datars eject std/bar` copies it into your project to edit, or you write your own from primitives — and it gets the same motion, accessibility and platforms as the built-in ones. See [custom recipes and scenes](/docs/custom-recipes/).

<figure class="fig"><div class="frame"><div class="chart" data-chart="how-tree" data-min="370" data-phone-ratio="1.3" data-caption></div></div><figcaption><b>A recipe call, expanded and resolved.</b> The document holds <code>use</code> nodes; publishing (or the device) expands them into primitives; resolving against the data makes one keyed node per row. The <a href="/docs/sdk/">SDK reference</a> covers every primitive.</figcaption>{{alt:how-tree}}</figure>

## Themes and inks

Colours in a scene are **inks**: `"$accent"`, `"$categorical[2]"`, `"$ink@0.4"` (the ink colour at 40 %), or a literal `"#e8112d"`. Theme tokens stay symbolic until the frame is drawn. So:

- dark mode, high contrast and your brand apply to every chart, built-in or custom;
- a published chart can be restyled by the page or app that shows it, without republishing;
- switching theme or mode re-inks the next frame, with no re-layout — and new type or shapes morph in.

Data colours are different: a party's colour is data, declared once in the document's `keys`. See [themes and brands](/docs/theming/).

## Bundles and tiers

`datars publish` compiles a document into a **bundle**: a small manifest plus content-addressed chunks, written to any static host. Pages and apps install the runtime once; charts then arrive as content.

| Tier | What it carries | Who plays it |
|---|---|---|
| T0 | an SVG poster and the accessible text | anything — even without a runtime |
| T1 | baked scenes for every state | playback-only runtimes |
| T2 | the recipes pre-expanded, data by hash | the default web runtime |
| T3 | the source document | runtimes with the recipe sandbox |

<figure class="fig hood-fig">{{variants:votes}}<figcaption><b>One chart's four variants</b>, each bar as long as its gzipped first load and made of its chunks, measured when this site was built. Hover a segment for its chunk.</figcaption></figure>

A runtime picks the best variant it can play. Republishing moves the alias; every embed follows. See [publishing and hosting](/docs/publishing/).

## One engine, everywhere

A frame is a pure function of the document, its data, its signals and time. The same deterministic Rust engine evaluates it in the browser (WebAssembly), in iOS, macOS and Android apps, on the desktop, and in the CLI that renders video, PNG, SVG and PDF. It shapes text with fonts that travel inside the bundle, so line breaks and label placement are identical too.

Every example in the repository renders to the same pixel hashes on macOS, in wasm, on the iOS simulator and on an Android emulator. For you, that means the chart you previewed is the chart your readers see — on every screen, and in the video.
