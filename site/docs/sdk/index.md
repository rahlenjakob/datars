---
title: SDK reference
titleTag: SDK reference · datars docs
description: Every export of @datars/sdk — documents, data, scene nodes, props and expressions, motion, programs, themes and recipes — with signatures read from the source and live figures drawn with the SDK itself.
lede: `@datars/sdk` is the TypeScript you write documents and recipes in: builders that return plain JSON the engine runs. Every signature and doc comment on these pages is read from `packages/sdk/src` when the site is built, and every figure is a document drawn with the SDK's own primitives — each links to its source.
---

## How the pieces fit

A document is one object. `doc()` takes its parts and returns the JSON the engine reads (the [document format](/docs/ir/)); every other export builds one of those parts.

| Part | What it is | Built with | Page |
|---|---|---|---|
| The document | size, title, theme, locale — and everything below | `doc`, `signal.*` | [Documents](/docs/sdk/documents/) |
| Data | named sources, and tables derived from them | `data.*`, `op.*`, `table` | [Data](/docs/sdk/data/) |
| The scene | a tree of nodes: groups, shapes, text, repeats, instances, views, tiles, recipes | `group`, `shape`, `geom.*`, `text`, `repeat`, `instances`, `view`, `tiles`, `use` | [Nodes](/docs/sdk/nodes/) |
| What every node takes | key, `when`, layout and size, scales, semantics, interaction | `NodeOpts`, `Layout`, `ScaleDecl`, `brush`, `scrub` | [Node options and scales](/docs/sdk/options/) |
| Property values | a literal, or an expression over the row, signals and the box | `e`, `expr`, `field`, `lit`, lambdas | [Props and expressions](/docs/sdk/props/) |
| Motion | rules for how changes animate: easing, matching, choreography, routes, ghosts | `motion`, `choreo.*`, `route.*`, `ghost.*` | [Motion](/docs/sdk/motion/) |
| The program | the states and how the reader moves through them | `story`, `step`, `scrolly`, `autoplay`, `chapter`, … | [Programs](/docs/sdk/programs/) |
| The theme | typed tokens, inks, fonts, modes | `theme`, `font.*` | [Themes](/docs/sdk/themes/) |
| Recipes | a scene packaged as a reusable chart type | `recipe`, `t.*`, `scale`, `Cx` | [Recipes](/docs/sdk/recipes/) |

The standard library is written with exactly these exports: `plot`, `bar` and every other chart in [the chart reference](/docs/std/) are recipes that return nodes. You can mix both in one scene.

## A whole document

This one uses nothing but the SDK. The data is five rows; a derived table ranks them; a signal says whether the bars are sorted; the scene reads the signal and the story's narration; three steps set the signal. It is the source of the figure below it.

{{code:site/figures/sdk/program-story.ts}}

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-program-story" data-caption></div></div><figcaption>The document above, live. Step through it: the bars are keyed by region, so they fade and slide instead of being redrawn. <a href="{{src}}/site/figures/sdk/program-story.ts">Figure source</a></figcaption>{{alt:sdk-program-story}}</figure>

Three habits make everything else work:

- **Key what comes from rows.** `key: e("d.region")` is what lets a mark morph between states instead of fading out and in.
- **Put data-dependent values in expressions.** `e("scale.y(d.v)")` is evaluated by the engine, per row and per frame, on every platform — your TypeScript runs once, to build the document.
- **Change state through signals.** Steps, clicks, sliders and the host page all set signals; the scene and the tables read them.

## Every export

Grouped by the module that defines it. Each links to its entry; functions inside namespaces (`geom.rect`, `op.stack`, `choreo.stagger`, …) are listed on the namespace's page. The build fails if an export is missing from these pages, so this list is complete.

{{sdk:index}}

## Using the SDK

The packages aren't on npm yet: a chart project links `@datars/sdk` (and `@datars/std`) from a clone of the repository — [getting started](/docs/getting-started/) has the three commands and the `package.json`. Then:

```sh
datars new mychart --template chart     # a working doc.ts to start from
datars dev mychart/doc.ts               # live page: rebuilds on save
```

A document is a module whose default export is `doc({ … })`. The CLI compiles it with esbuild, runs it once and hands the JSON to the engine — nothing from your module runs in the reader's browser. [Documents](/docs/documents/) is the guide to the same parts in prose.
