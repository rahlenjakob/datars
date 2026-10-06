---
title: Document format
description: The JSON a datars document compiles to — its top-level fields, expressions, recipe instances and programs — with the JSON Schema, validation in your editor and migrations.
lede: A TypeScript document builds plain JSON. That JSON — the document format, or IR — is what the tools check, the publish step compiles and every runtime plays.
---

## TypeScript builds JSON

The builders in `@datars/sdk` and `@datars/std` return plain JSON objects; `doc()` fills in defaults. So this TypeScript:

{{code:examples/votes/doc.ts#L10-28}}

…is this JSON (the scene's first chart shown):

{{code:examples/votes/doc.json#L95-131}}

You rarely need to look at it: `datars` compiles `doc.ts` with your project's esbuild whenever you run a command. It matters when you generate documents from another language, store them in a database, have an agent write them, diff them in review, or validate them in an editor. `JSON.stringify` of a document's default export is its JSON; a `doc.json` next to a `doc.ts` works everywhere a `doc.ts` does.

## Top-level fields

| Field | Contains |
|---|---|
| `datars` | The format version: `1`. |
| `id`, `title`, `description` | Identity and text. The title and description are part of the chart's accessible text. |
| `size` | `{ "width", "height" }` — the authored size. Hosts resize charts; recipes lay out for the real size. Default 800 × 480. |
| `locale` | Number and date formatting, and the language of map labels. Default `"en"`. |
| `theme` | `{ "use": "<theme name>", "themes": [...], "tokens": {...} }` — which theme, extra theme definitions, and document-level token overrides. Default `{ "use": "datars/neutral" }`. |
| `data` | Sources by name: inline `values` or `csv`, a `url`, a `slot` the host fills, an `atlas`, `tiles`, `geojson`, `topojson`, a `font`, or `generate`d rows — each with an optional `key`, `types` and `live`. |
| `tables` | Derived tables: `{ "from": "<source>", "ops": [...] }` — filter, derive, aggregate, sort, join, stack, bin, window and the layout operations. |
| `signals` | Named values that interaction and states change: `{ "type": "num" \| "str" \| "bool" \| "keyset" \| "key" \| "range", "default": … }`. |
| `keys` | Metadata per key: display names and colours (`{ "SD": { "name": "Sweden Democrats", "color": "#dddd00" } }`) — data colours are data, not theme. |
| `scene` | The root of the scene template: nodes and recipe instances. The only required field. |
| `motion` | `{ "rules": [...] }` — how changes animate (matchers, durations, easings, choreography, routes). |
| `program` | The states and how they're driven: `{ "preset": "story", "states": [...], "drivers": [...] }`. |
| `packages` | User code the engine's sandbox loads: your own recipes, `{ "name", "source" }`. |

The chapters of these docs cover each: [data and tables](/docs/data/), [signals and interaction](/docs/interaction/), [charts, scales and marks](/docs/charts/), [states and stories](/docs/stories/), [motion](/docs/motion/), [themes](/docs/theming/).

## Values inside the scene

**Expressions** are objects with one `expr` field — source text the engine compiles and evaluates per row and per frame:

```json
{ "expr": "d.share * income / 100" }
```

`e("…")`, `` expr`…` `` and lambdas (`d => d.share * 2`) in TypeScript all become this. See [expressions](/docs/expressions/).

**Colours are inks**, resolved against the theme at draw time: `"#e8112d"`, `"$accent"`, `"$categorical[2]"`, `"$ink@0.4"` (a token at 40 % opacity). Because inks stay late-bound in published charts, modes and brand overrides restyle them without republishing.

**Nodes** have a `kind` — `group`, `view`, `shape`, `text`, `instances`, `tiles`, `repeat` — plus common options: `key` (identity across states), `when` (show only while an expression is true), `semantics`, `on` (interactions), `layout`, `transform`, `opacity`.

**Recipe instances** are nodes of kind `use`:

```json
{
  "kind": "use",
  "recipe": "@datars/std/bar",
  "params": { "labels": true },
  "key": "bars"
}
```

`recipe` names a standard-library recipe (`@datars/std/<name>`, see the [chart reference](/docs/std/)) or one of your own (`@local/<file>/<name>`, embedded as a package when the document is built). `params` are the recipe's parameters; node options (`key`, `when`, …) sit beside them. Publishing expands recipes ahead of time, so most runtimes never run recipe code at all.

**Programs** list states; each state sets signals and can narrate:

```json
{
  "preset": "story",
  "states": [
    { "name": "bars", "set": { "shape": "bars" },
      "narration": { "title": "30.3%", "text": "for the Social Democrats." } },
    { "name": "pie", "set": { "shape": "pie" } }
  ],
  "drivers": ["steps", "keys"]
}
```

## The schema

The format is published as a JSON Schema (draft 2020-12), generated from the engine's own types, so it can't drift from what the engine reads:

```sh
datars schema > ir.schema.json
```

It is also served by this site at [`/schema/ir-1.json`](/schema/ir-1.json), and its `$id` is `https://datars.dev/schema/ir-1.json`. Every example in the repository validates against it.

To get completion and validation for `doc.json` files in VS Code, map them to the schema in your settings (don't put a `$schema` key in the document itself — `datars check` reports unknown fields):

```json
{
  "json.schemas": [
    { "fileMatch": ["**/doc.json"], "url": "./ir.schema.json" }
  ]
}
```

For TypeScript documents, the SDK's types do the same job as you type.

## Unknown fields and old documents

`datars check` reports fields the format doesn't have — usually a typo — instead of silently ignoring them, and recipe parameters that don't exist come back with the closest one that does. When the format changes, documents are upgraded, not broken:

```sh
datars migrate chart.json            # what would change, and fields the format ignores
datars migrate chart.json --write    # upgrade the file in place
```

{{run:migrate examples/votes/doc.json}}

Published bundles record the format version and the versions of the standard library they were built with, and a runtime that has a different standard library plays the pre-expanded variant — so a chart looks the same years later, whatever runtime shows it. See [publishing](/docs/publishing/#tiers-what-each-runtime-gets).
