---
title: Documents
description: The fields of a datars document — size, theme, locale, data, tables, signals, keys, scene, motion and program — how groups lay out, and how charts adapt to any screen size.
lede: A document is everything a chart is — data, scene, motion and states — in one object you build with `doc()`.
---

## The shape of a document

`doc()` from `@datars/sdk` takes one object and returns the document. Only `scene` is required.

```ts
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, card } from "@datars/std";

export default doc({
  id: "votes",
  title: "Vote share by party, Sweden 2022",
  description: "Eight parties' share of the vote, as bars, ranked, and as a pie.",
  size: [720, 440],
  locale: "en",
  theme: "datars/neutral",
  data: { votes: data.values({ party, share }, { key: "party" }) },
  tables: { ranked: { from: "votes", ops: [{ op: "sort", by: [["share", "desc"]] }] } },
  signals: { shape: signal.str("bars") },
  keys: { S: { name: "Social Democrats", color: "#e8112d" } /* … */ },
  scene: group({ /* … */ }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  program: story({ steps: [ /* … */ ] }),
});
```

| Field | What it is |
|---|---|
| `id` | A stable name for the chart (tools and bundles use it). |
| `title`, `description` | What the chart is and shows, in words. They open the chart's text alternative and its accessible name — write them for a reader who can't see it. |
| `size` | The authored size, `[width, height]` in CSS pixels (default 800 × 480). See [responsive](#responsive). |
| `theme` | A built-in theme's name, your own theme object, or `{ use, themes, tokens }` to pick one and override tokens. See [themes and brands](/docs/theming/). |
| `locale` | A BCP 47 tag (`"en"`, `"sv"`, `"de-CH"`) for number and date formatting and basemap names. Default `"en"`. |
| `data` | Named sources: inline values, CSV, URLs, data slots, atlases, tile archives, fonts, generated rows. See [data and tables](/docs/data/). |
| `tables` | Derived tables: `{ from, ops }` — filters, aggregates, joins, layouts. |
| `signals` | Named values the scene reads and interactions write. See [signals and interaction](/docs/interaction/). |
| `keys` | Metadata per key value: a display `name` and a data `color`. |
| `scene` | The marks: a tree of groups, recipes and primitives. |
| `motion` | Motion rules that override how changes animate. See [motion](/docs/motion/). |
| `program` | The states and what moves between them. See [states and stories](/docs/stories/). |
| `packages` | Your own recipe modules, embedded when the document is built (`{ name, file }`). See [custom recipes](/docs/custom-recipes/). |

## Key metadata

Keys identify rows; `keys` says how a key value reads and, optionally, what colour it is:

```ts
keys: {
  S:  { name: "Social Democrats", color: "#e8112d" },
  SD: { name: "Sweden Democrats", color: "#dddd00" },
  M:  { name: "Moderates",        color: "#1b49dd" },
},
```

Axes, legends, tooltips and accessible labels show `name` (`key.name(d.party)` in an expression). A `color` overrides the theme's categorical palette for that key wherever it's coloured by category — a party's colour is data, and follows it into every chart. Keys without one take palette colours.

## The scene

The scene is a tree. Its nodes are:

- **groups** — `group({ key, layout, children })`, which lay out and transform their children;
- **recipes** — `plot(…)`, `pie(…)`, `map(…)`, `card(…)` and the rest of [the standard library](/docs/std/), or your own;
- **primitives** — `shape`, `text`, `instances`, `repeat`, `view`, `tiles` — for anything a recipe doesn't draw ([charts, scales and marks](/docs/charts/#primitives)).

Every node takes the same options as its last argument: `key` (its identity, so it can morph), `when` (an expression: the node exists only while it's true), `opacity`, `transform`, `clip`, `semantics`, `on` (interactions), `size` and `layout`.

```ts
scene: group({
  key: "root",
  layout: { type: "stack", padding: [16, 20, 12, 12] },
  children: [
    plot({ data: "votes", x: "party", y: "share", children: [bar()] }, { key: "chart", when: e('shape == "bars"') }),
    pie({ data: "votes", value: "share", category: "party" },         { key: "chart", when: e('shape == "pie"') }),
  ],
}),
```

The two charts share the key `"chart"`: only one exists at a time, and when `shape` changes, the parts of one morph into the other.

## Layout

A group lays out its children with `layout`:

| `type` | Children are |
|---|---|
| `stack` (default) | on top of each other, each filling the box |
| `rows` | one under the other |
| `columns` | side by side |
| `grid` | in a grid of `columns` columns |
| `flow` | in a row that wraps at the box edge, each as wide as it measures (chips, legend entries) |

`gap` is the space between children, `padding` the space inside the box (a number, or `[top, right, bottom, left]` like CSS), and `align` (`start`, `center`, `end`, `stretch`) places children that have a fixed or `auto` size on the cross axis.

A child's `size: { w, h }` along the layout's axis is one of:

| Value | Meaning |
|---|---|
| `120` | fixed, in pixels |
| `"30%"` | a share of the box |
| `"auto"` | as large as its content measures (a title, a slider) |
| `"fill"` or `{ fill: 2 }` | shares what's left, by weight (the default is `fill`) |

```ts
group({
  key: "root",
  layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] },
  children: [
    slider({ signal: "income", min: 10000, max: 80000, step: 1000, label: "Monthly income" }, { size: { h: 44 } }),
    plot({ data: "amounts", x: "amount", y: "item", xType: "linear", yType: "band", children: [bar({ labels: true })] }),
  ],
})
```

Layout happens in the engine with measured text: an axis is exactly as wide as its widest label in the chart's own font, so margins never need guessing.

## Responsive

`size` is the size you design at and the aspect ratio an embed reserves by default — it isn't a fixed canvas. The page or app gives the chart its real box, and the chart is laid out for it: axes re-measure, labels re-place, recipes can choose less detail on a phone. Resizing, or rotating a phone, re-lays out and **morphs** to the new layout, because keys don't change.

To adapt explicitly, expressions can read three built-in signals:

| Signal | Value |
|---|---|
| `viewport.w`, `viewport.h` | the chart's box, CSS pixels |
| `sizeClass` | `"phone"` (narrower than 560), `"tablet"` (narrower than 960) or `"wide"` |

```ts
card({ title: e("narration.title"), text: e("narration.text"), width: 230 },
  { key: "caption", when: e('sizeClass != "phone"') })
```

Recipes see the same through their expansion context (`cx.size`, `cx.sizeClass`). A vertical video is rendered at a phone size, so it's the phone layout, not a crop ([video, images and PDF](/docs/export/)).

## TypeScript and JSON

What `doc()` returns is the document's JSON form — the IR, which is the real contract:

- `datars` commands accept either `doc.ts` or `doc.json`. For `.ts`, the CLI compiles it with your project's esbuild and `@datars/*` packages.
- Local recipes (`recipes/*.ts` next to the document) are embedded into the document when it's built, so a `doc.json` is self-contained.
- `datars schema` prints the IR as a JSON Schema; it's also at [/schema/ir-1.json](/schema/ir-1.json). See [document format](/docs/ir/).
- `datars migrate doc.json --write` upgrades an old document to the current format and reports fields it doesn't know.

Any language that can write JSON can make datars charts; TypeScript just gives you types, autocomplete and the standard library's helpers.

## Inspecting what you built

```sh
datars inspect doc.ts --state 1     # the resolved scene as text: keys, geometry, inks, roles
datars states doc.ts                # the program's states
datars diff doc.ts --states bars,pie   # what changes between two states
```

{{run:states examples/votes/doc.json}}
