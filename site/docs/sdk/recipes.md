---
title: Recipes
titleTag: Recipes — SDK reference · datars docs
description: recipe(), RecipeDef, the t.* parameter types, the expansion context Cx and scale references in @datars/sdk — with a complete local recipe and its live chart.
lede: A recipe is a function from typed parameters to a scene template, run by the engine's sandbox. The standard library is written with exactly this API; yours get the same motion, accessibility, theming and platforms. The guide is [Custom recipes and scenes](/docs/custom-recipes/).
---

## A recipe

{{sdk:recipe}}

`recipe(def)` returns a function you call like any chart — `dumbbell({ data: "scores", y: "region", a: "before", b: "after" }, { key: "chart" })` — which returns a [`use`](/docs/sdk/nodes/#use) node. When the engine expands it, parameters get their defaults, `expand` runs once per distinct set of parameters (never per row), and parameters the recipe doesn't have come back as diagnostics with the nearest name it does have.

This one is complete — a dumbbell chart, used by the figure below:

{{code:site/figures/sdk/recipes/dumbbell.ts}}

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-recipe-dumbbell" data-phone-ratio="0.72" data-caption></div></div><figcaption>The recipe above in a document: <code>import { dumbbell } from "./recipes/dumbbell"</code>, embedded as a package when the document is built. Hover a dot: its label is the recipe's <code>semantics</code>. <a href="{{src}}/site/figures/sdk/recipe-dumbbell.ts">Figure source</a></figcaption>{{alt:sdk-recipe-dumbbell}}</figure>

{{sdk:RecipeDef}}

`id` is the recipe's full name — `@local/<file>/<export>` for a file in `recipes/` next to the document, the package name as a prefix for a shared module. `motion` holds the recipe's own motion rules (or a function of its parameters), scoped to where it's used and always under the document's.

{{sdk:Recipe}}

## Parameters

{{sdk:ParamSpec}}

{{sdk:t}}

| Helper | Declares |
|---|---|
| {{sig:t.table}} | a table name |
| {{sig:t.field}} | a column name |
| {{sig:t.number}} | a number, with a default |
| {{sig:t.string}} | a string |
| {{sig:t.bool}} | a boolean |
| {{sig:t.ink}} | a colour or token (`"$accent"`, `"#e8112d"`) |
| {{sig:t.prop}} | a value or an expression — a per-row size, a conditional colour |
| {{sig:t.oneOf}} | one of a fixed set of strings (typed as their union) |
| {{sig:t.children}} | child nodes: marks inside a frame, like a plot's |
| {{sig:t.json}} | anything JSON |

The `doc` strings are what `datars describe`, the [chart reference](/docs/std/), `llms.txt` and the MCP server show.

## The expansion context

{{sdk:Cx}}

`cx` is what `expand` can ask of the engine while it builds the scene: the box it's expanded for (a hint — layout happens later, in the engine), the size class and locale, late-bound inks and tokens, text measured by the engine's shaper with the chart's own fonts (identical on every platform), and derived tables — named by their content, so two instances over different data get different tables and identical ones share.

{{sdk:scale}}

{{sdk:ScaleRef}}

`scale("x")` (or `cx.scale("x")`) is a scale by name as expression builders: `x(v)` is `scale.x(v)` as an expression, `x.bandwidth()`, `x.step()`, `x.ink(v)`, `x.min()`, `x.max()` likewise. The scale itself is whichever `x` is in scope where the recipe is used — the enclosing plot's, or one the recipe declares on its own group, as the dumbbell does.

A recipe never sees rows: per-row positions are expressions, per-row layout is a [table operation](/docs/sdk/data/#op), repetition is [`repeat`](/docs/sdk/nodes/#repeat). That's why a recipe that works for ten rows works for a million.
