---
title: Custom recipes and scenes
description: Write your own chart types in TypeScript with the same API the standard library uses, or eject a standard recipe and change it — with motion, accessibility and every platform included.
lede: Every chart in the standard library is a recipe — a TypeScript function from typed parameters to a scene. Yours are written the same way and get the same motion, accessibility, theming and platforms.
---

## When you need one

Most charts are a standard recipe with parameters. Reach for your own when you want:

- **a variation** of a standard chart — different labels, a different mark shape: [eject](#eject-a-standard-recipe) the recipe and edit it;
- **a composition** you use often — ranked bars with a target line and a callout: a recipe of ten lines that calls others;
- **a scene no library ships** — countries leaving a globe to orbit as planets: a recipe drawn from primitives.

<figure class="fig"><div class="frame"><div class="chart" data-chart="worlds" data-caption></div></div><figcaption><b>Eight billion, four ways</b> — one custom recipe of about sixty lines draws all four layouts. Every country keeps its key, so its coastline morphs into its planet and its planet into its block.</figcaption>{{alt:worlds}}</figure>

## A recipe from scratch

A lollipop chart — a dot on a stem per row — as a recipe you can drop into any `plot`:

```ts
// recipes/lollipop.ts
import { e, geom, group, recipe, repeat, shape, t, type Prop } from "@datars/sdk";

interface LollipopParams { data: string; x: string; y: string; r: number; fill: Prop }

export const lollipop = recipe<LollipopParams>({
  id: "@local/lollipop/lollipop",
  doc: "A dot on a stem per row: a lighter bar chart for many categories.",
  params: {
    data: t.table(),
    x: t.field("The category column (on the plot's band scale)."),
    y: t.field("The value column."),
    r: t.number(5, "Dot radius (px)."),
    fill: t.prop("Dot ink or an expression over the row (default $mark)."),
  },
  tokens: ["mark", "ink-2"],
  expand(p, cx) {
    const mid = `scale.x(d.${p.x}) + scale.x.bandwidth() / 2`;
    const top = `scale.y(d.${p.y})`;
    return group({ key: "lollipops", children: [
      repeat(p.data, group({ key: e(`d.${p.x}`), children: [
        shape(geom.segment({ x1: e(mid), y1: e("scale.y(0)"), x2: e(mid), y2: e(top) }),
          { key: "stem", stroke: { paint: "$ink-2", width: 1.5 }, semantics: { role: "decoration" } }),
        shape(geom.circle({ cx: e(mid), cy: e(top), r: p.r }),
          { key: "dot", fill: p.fill ?? "$mark", pickable: true,
            semantics: { role: "datum", label: e(`\`\${key.name(d.${p.x})}: \${format(d.${p.y}, ",.1~f")}\``) } }),
      ] })),
    ] });
  },
  // Dots grow in from nothing when rows appear.
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }],
});
```

Use it like any standard mark — inside a `plot` it inherits `data`, `x`, `y` and `color`:

```ts
import { plot } from "@datars/std";
import { lollipop } from "./recipes/lollipop";

plot({ data: "sales", x: "region", y: "sales", children: [lollipop({ r: 6 })] })
```

### The parts

| Field | What it's for |
|---|---|
| `id` | the recipe's full name. Local recipes use `@local/<file>/<export>`; packages use their package name as the prefix. |
| `doc` | one or two sentences — shown by `datars describe`, in reference docs and to agents |
| `params` | typed parameters with defaults and docs (below) |
| `expand(params, cx)` | returns the scene template; runs once per distinct set of parameters, never per row |
| `motion` | motion rules for the nodes this recipe makes — an array, or a function of the parameters |
| `tokens` | the theme tokens the recipe reads (for docs and theme specimens) |
| `examples` | paths of example documents |

### Parameter types

| Helper | Accepts |
|---|---|
| `t.table(doc?)` | a table name |
| `t.field(doc?)` | a column name |
| `t.number(default?, doc?)`, `t.string(…)`, `t.bool(…)` | literals |
| `t.ink(default?, doc?)` | a colour or token: `"#e8112d"`, `"$accent"` |
| `t.prop(doc?)` | a value or an expression (`e("d.share * 2")`) |
| `t.oneOf(["a", "b"], default?, doc?)` | one of a fixed set |
| `t.children(doc?)` | child nodes (marks inside a frame) |
| `t.json(doc?)` | anything JSON |

Defaults are filled in before `expand` runs. A parameter the recipe doesn't have is reported with the nearest one it does — *`titel` (did you mean `title`?)*.

### The expansion context

`cx` is what a recipe can ask of the engine while it builds the scene:

| Member | Gives |
|---|---|
| `cx.size`, `cx.sizeClass`, `cx.locale` | the box the recipe is expanded for (a hint — layout happens in the engine), `"phone"`, `"tablet"` or `"wide"`, and the document's locale |
| `cx.scale("x")` | the enclosing plot's scale as expression builders: `x(v)`, `x.bandwidth()`, `x.step()`, `x.ink(v)`, `x.min()`, `x.max()` |
| `cx.field("share")` | the expression `d.share` |
| `cx.ink("accent")` | the late-bound ink `"$accent"` |
| `cx.token("size.label")` | a theme number or text token, as an expression |
| `cx.measure(text, { size, weight, family })` | the text's width and height, measured by the engine's shaper with the chart's fonts — identical on every platform |
| `cx.table(name, from, ...ops)` | a derived table this recipe needs (filtered, aggregated, laid out); returns its name |
| `cx.uid(prefix)` | a unique id within this expansion |

## Recipes describe; the engine computes

A recipe never sees the rows. It returns a template — nodes whose properties are values, column references or expressions — plus the derived tables it needs, and the engine evaluates them per row, per frame, in Rust. So a recipe that works for ten rows works for a million, expansion time doesn't grow with the data, and the result is exact on every platform.

- **Per-row positions are expressions**: `e("scale.x(d.region)")`, or lambdas like `d => d.share * 2`.
- **Per-row layout is a table operation**: `op.treemap`, `op.stack`, `op.pie`, `op.beeswarm`, `op.waffle`, `op.parliament`, `op.sankeyNodes`, `op.window`, `op.aggregate`… all run as Rust algorithms. See [Data and tables](/docs/data/).
- **Repetition is `repeat`**: `repeat("rows", template)` draws the template once per row; `repeat({ groups: "rows", by: "series" }, template)` once per group.

### Primitives to draw with

Every builder, option, parameter type and `cx` member is in the [SDK reference](/docs/sdk/), with figures of what each one draws.

| Builder | Draws |
|---|---|
| `group({ children, layout, coord, scales, clip, … })` | a container; it can lay children out (`stack`, `rows`, `columns`, `grid`, `flow`), set a coordinate system (`geo`, `planar`) and declare scales |
| `view({ camera, children })` | a window with a camera: fit keys, a box or a lon/lat box; explorable |
| `shape(geom, { fill, stroke })` | `geom.rect`, `circle`, `ellipse`, `arc`, `segment`, `polyline`, `area`, `path`, `symbol`, `feature` (a map region) |
| `text(content, [x, y], { style, halo, number })` | text shaped by the engine; `number` makes values count when they change |
| `instances({ from, x, y, r, fill, lod })` | thousands to millions of marks in one draw |
| `tiles({ source, layers })` | vector-tile features for what the view shows |
| `repeat(from, template)` | one template per row, group, tick or legend entry |

Every node takes `key` (identity for motion — make it the row's key), `when` (show only while an expression is true), `semantics` (role and label for screen readers), `pickable` and `on` (interaction), `opacity`, `transform`, `z`. See [Charts, scales and marks](/docs/charts/).

## Motion comes free

Transitions work for custom recipes exactly as for standard ones. Key each datum's shape by its row key and the engine pairs shapes across states — even across recipes, even when their geometry differs: the worlds recipe's globe draws `geom.feature` outlines and its orbits draw `geom.circle`, and each country morphs from one to the other because the key is the same.

Two places to shape the motion:

- **The recipe's `motion`** sets defaults for its own nodes, scoped to where it's used: bars grow from their baseline, lines draw on.

  ```ts
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }],   // std/line: a new line draws on
  ```

- **The document's `motion`** overrides them per state pair — routes, staggers, ripples, durations. See [Motion](/docs/motion/).

  ```ts
  motion: motion(
    { when: { from: "globe", to: "orbits" }, select: { role: "datum" }, route: route.spiral(0.35), choreo: choreo.ripple(0.55) },
  ),
  ```

## A real one: the worlds recipe

The whole recipe behind the globe above: four layouts from one table, positions as expressions over each country's row, a treemap as a table operation, semantics on every mark.

{{code:examples/worlds/recipes/worlds.ts}}

## Where recipes live

### Local recipes

A file under `recipes/` next to the document, imported by path:

```ts
import { lollipop } from "./recipes/lollipop";
```

Its id must be `@local/<file>/<export>` — `@local/lollipop/lollipop` for `recipes/lollipop.ts`. When the document is built (`datars dev`, `check`, `render`, `publish`), the file is bundled and embedded in the document as a package, so the published chart carries its own recipe. `datars dev` rebuilds when a file under `recipes/` changes.

### Packages

A plain ES module written against `@datars/sdk`, shared by several documents, is listed in the document's `packages`; its recipes' ids start with the package name:

```ts
import { bubbles, petals } from "../../recipes/components.js";   // ids "@articles/components/…"

export default doc({
  packages: [{ name: "@articles/components", file: "../../recipes/components.js" }],
  // …
});
```

The module's source is embedded in the document when it's built.

## Eject a standard recipe

When a standard chart is almost right, copy it into your project:

```sh
$ datars eject std/waterfall
recipes/waterfall.ts
import { waterfall } from "./recipes/waterfall" in your doc; it's embedded when the doc is built
```

The copy has the id `@local/waterfall/waterfall` and imports what it needs from `@datars/std`. Change the import in your document from `@datars/std` to `./recipes/waterfall` and the chart renders exactly what it did before — then edit freely. `--to <dir>` writes elsewhere; `--force` overwrites.

## The sandbox

Recipes run in the engine's own JavaScript sandbox (QuickJS), the same on every platform, so a recipe expands identically on a server, in a browser and in an app:

- `Math` is replaced with the engine's deterministic maths, so `Math.sin` gives the same bits everywhere.
- There is no `Date`, no `Math.random`, no timers and no network or file access. For randomness, use seeded `rand(key, stream)` in expressions or generate values in the document.
- Every call runs under a time budget and a memory limit: an endless loop comes back as an error — *took too long (over its time budget)* — instead of freezing the page.

Published charts don't need the sandbox on the reader's device: the publish step pre-expands recipes into the bundle's T2 variant, which the default web runtime plays without running any recipe code. The bundle also carries the source (T3) for runtimes that include the sandbox, and apps can refuse downloaded script entirely. See [Publishing and hosting](/docs/publishing/).

## See also

- [Chart reference](/docs/std/) — every standard recipe; each page links to its source.
- [Expressions](/docs/expressions/) — what `e("…")` can say.
- [Extensibility feature page](/features/extensibility/).
