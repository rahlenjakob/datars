---
title: CLI
description: Every datars command — new, dev, check, lint, explain, render, film, video, publish, serve, bundle, test and more — with their flags and environment variables.
lede: One binary, `datars`, previews, checks, explains, renders, films, tests and publishes charts. Everything runs locally, without a browser or a GPU.
---

Install it from the repository with `cargo install --path crates/datars-cli` (see [getting started](/docs/getting-started/)). Every command that takes a document accepts a TypeScript document (`doc.ts`, compiled with your project's `@datars/sdk` and esbuild) or its JSON form (`doc.json`).

## Commands

{{cli:table}}

The same table is `datars help`. `datars-mcp` exposes the main commands to coding agents as MCP tools — see [coding agents and MCP](/docs/agents/).

## Flags every command understands

| Flag | Meaning |
|---|---|
| `--json` | Machine-readable output: diagnostics, findings, sizes and file paths as JSON, for scripts, CI and agents. |
| `--data name=file[,name=file]` | Fill data slots (or replace sources) from files: preview a chart with a real user's rows instead of its sample. |
| `--state N` or `--state name` | The program state to look at (commands that show one state). |
| `--size WxH` | Lay the document out for this size in CSS pixels — `--size 390x844` shows the phone layout. |
| `--mode dark` or `--mode high-contrast` | The theme mode. |
| `--dpr D` | Pixel density of rendered images (default 2; 1 for `video`). |

## Start a chart

```sh
datars new election --template story      # election/doc.ts: chart, story, explorable or map
datars data profile results.csv           # column types, gaps, ranges, candidate keys, hints
datars describe std/bar                   # a recipe's parameters, defaults and theme tokens
datars describe                           # every recipe
```

`new` refuses to overwrite an existing `doc.ts`. `data profile` is worth running before you chart anything: it suggests which column is the key, notices years stored as numbers, ISO country codes that join the built-in atlas, and ranges that want a log scale.

## Preview while you edit

```sh
datars dev election/doc.ts                # http://127.0.0.1:8788/
```

A live page with the chart, its states, diagnostics and lint findings. Save the document (or a local recipe under `recipes/`) and the chart morphs from what's on screen to the new version. Maps with automatic basemaps show at once and gain street detail as the map data arrives. `--port` picks another port; the server listens on `127.0.0.1` only.

## Check, lint, explain

```sh
datars check election/doc.ts
datars lint election/doc.ts
datars explain election/doc.ts --key '("SD",)' --state 1
```

- **`check`** loads the document and resolves every state. It reports diagnostics — unknown parameters (with *did you mean*), missing data, fonts without a source, labels that collide or leave the frame — and exits non-zero if there are any, so it can gate a build.
- **`lint`** runs the data-graphics checks: identity (keys), encoding, legibility, accessibility and theme contrast. Each finding has a severity and a suggested fix; errors make it exit non-zero.
- **`explain`** answers “why does this element look like that?” for one element: the recipes that made it, its template, the data row, and every expression with its value.

{{run:explain examples/votes/doc.json --key '("SD",)'|12}}

Other ways to look inside:

```sh
datars states election/doc.ts                     # the program's states
datars inspect election/doc.ts --state 2          # the scene as text: keys, geometry, inks, roles
datars semantics election/doc.ts                  # what a screen reader gets
datars diff election/doc.ts --states bars,pie     # what changes between two states
datars diff old.json new.json                     # or between two documents
datars fonts election/doc.ts                      # font tokens, sources, licences
datars profile election/doc.ts                    # where the time goes, every transition at 60 fps
```

## Render, film, video

```sh
datars render election/doc.ts --state 2 --out ranked.png    # also .svg and .pdf
datars film election/doc.ts --from 0 --to 1                 # filmstrip + motion trails
datars video election/doc.ts --size 360x640 --dpr 3         # vertical MP4 + WebVTT captions
```

See [video, images and PDF](/docs/export/) for every option.

## Publish

```sh
datars publish election/doc.ts --alias election --to site/  # c/election + chunks/ for any static host
datars serve site/ --port 8787                              # try it locally
datars bundle election/doc.ts --out election.datars         # one file instead
datars bundle inspect election.datars                       # variants, sizes, what each runtime plays
```

See [publishing and hosting](/docs/publishing/).

## Maps and themes

```sh
datars basemap rio/doc.ts            # the automatic basemap's plan: views, tiles per zoom, data to fetch
datars basemap rio/doc.ts --build    # build it now
datars theme brand.json --mode dark  # resolve a theme and run its checks
datars theme brand.json --specimen specimen.png   # every std chart in light, dark and high contrast
```

## Make a recipe yours

```sh
datars eject std/waterfall           # recipes/waterfall.ts, to edit (--to another folder, --force to overwrite)
```

Import it from `./recipes/waterfall` instead of `@datars/std`; the document embeds it when it's built. See [custom recipes](/docs/custom-recipes/).

## Tests

```sh
datars test                  # every examples/<name>/doc.json: snapshots, 64-sample sweeps of every transition, motion checks
datars test election         # one example
datars test --update         # accept new goldens, after looking at the renders
```

`test` works in any folder with an `examples/` directory: each `examples/<name>/doc.json` is rendered in every state and every transition is sampled; the results are compared with goldens in `tests/golden/<name>/`. It also fails states with unreadable labels, and transitions that flash, blink, pop or don't end exactly where they should. Reports and diff images go to `out/test/`.

## Documents over time

```sh
datars migrate old.json              # what would change to reach the current format
datars migrate old.json --write      # upgrade it in place
datars schema > ir.schema.json       # the document format as a JSON Schema
```

See [document format](/docs/ir/).

## Environment variables

| Variable | What it changes |
|---|---|
| `DATARS_CACHE` | Where downloaded fonts are cached (default: `~/Library/Caches/datars`, `~/.cache/datars` or `%LOCALAPPDATA%\datars`; also `$XDG_CACHE_HOME/datars`). |
| `DATARS_OFFLINE=1` | Fonts come from the cache only; nothing is downloaded. |
| `DATARS_GEO_CACHE` | Where map data for automatic basemaps is cached (default: `~/.cache/datars/geo`). |
| `DATARS_GEO_OFFLINE=1` | Automatic basemaps are built from the cache only. |
| `DATARS_GEO_SEED` | Folders with Natural Earth data to use before downloading it. |
| `DATARS_OVERPASS_URL` | An OpenStreetMap Overpass server of your own, used instead of the public ones (and without the daily budget). |
| `DATARS_OVERPASS_BUDGET` | The daily budget for public Overpass servers, as `queries,megabytes` (default `5000,500`). |
| `DATARS_GEO_CELLS` | A URL template (`{level}/{z}/{x}/{y}`) of pre-cut map data cells on a static server. |
| `DATARS_ASSETS` | An extra folder to find built-in atlases in. |
| `DATARS_REPLAY_PERIOD` | For `datars serve`: seconds between snapshots of a replayed live feed (default 2). |

The public Overpass servers are shared, donated machines: fine for an author building a few maps, not for a product or a CI farm. Point those at your own server (`DATARS_OVERPASS_URL`) or at pre-cut cells (`DATARS_GEO_CELLS`). See [maps](/docs/maps/).

## Exit codes

Commands exit with `0` on success and `1` on an error — including `check` with diagnostics, `lint` with error findings and `test` with failures — so any of them can gate a CI job. With `--json`, the details are on standard output either way.
