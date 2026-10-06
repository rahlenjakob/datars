---
title: Getting started
description: Install the datars CLI, make a chart from a template, preview it live in the browser, check it, publish it to a folder any static host serves, and embed it in a page.
lede: From nothing to a published, embedded, animated chart. About fifteen minutes, most of it the first build of the CLI.
---

> **Tip** **Using React, Vite or Next.js?** Read [React, Vite and Next.js](/docs/react/) instead: it covers the same steps inside a React app, without adding script tags by hand.

## 1. Install the CLI

datars isn't published to crates.io or npm yet, so you build it from the repository. You need [Rust](https://rustup.rs) and [Node 20+](https://nodejs.org) with [pnpm](https://pnpm.io).

```sh
git clone {{repo}} datars
cd datars
cargo install --path crates/datars-cli   # installs the `datars` command
pnpm install && pnpm build               # builds @datars/sdk and @datars/std
datars help
```

`datars help` lists every command; the [CLI reference](/docs/cli/) explains them. The CLI renders on the CPU, so it works on any machine — no GPU, no browser.

## 2. Make a project

A chart project is an ordinary Node project that depends on the SDK (`@datars/sdk`), the standard library of charts (`@datars/std`) and, for web pages, the runtime (`@datars/web`), plus esbuild to compile TypeScript documents. Until the packages are on npm, link them from your clone:

```json
{
  "name": "my-charts",
  "private": true,
  "type": "module",
  "dependencies": {
    "@datars/sdk": "link:../datars/packages/sdk",
    "@datars/std": "link:../datars/packages/std",
    "@datars/web": "link:../datars/packages/web"
  },
  "devDependencies": { "esbuild": "^0.25.0", "typescript": "^5.6.0" }
}
```

Save that as `package.json` next to your `datars` clone and run `pnpm install` (with npm, write `file:` instead of `link:`).

> **Note** The web runtime (`@datars/web`) contains WebAssembly built from Rust. If `packages/web/dist/wasm/` is empty in your clone, build it once: `bash scripts/build-wasm.sh && pnpm -C packages/web build` (the script lists its one-time setup at the top).

## 3. Start from a template

```sh
datars new sales --template story
```

That writes `sales/doc.ts`, a complete, working document. There are four templates:

| Template | What you get |
|---|---|
| `chart` | a labelled bar chart |
| `story` | a stepped story: bars that morph into a pie |
| `explorable` | a bar chart driven by a slider the engine draws |
| `map` | a world choropleth over the built-in countries atlas |

The story template, in full:

```ts
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pie } from "@datars/std";

export default doc({
  id: "sales",
  title: "Where the votes went",
  size: [720, 440],
  data: { votes: data.values({ party: ["A", "B", "C", "D"], share: [34.2, 27.9, 21.5, 16.4] }, { key: "party" }) },
  signals: { shape: signal.str("bars") },
  // Pair data marks by their own key across recipes (a bar and a slice are the same party).
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share (%)", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
      pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
    ],
  }),
  program: story({
    steps: [
      step("bars", { set: { shape: "bars" }, title: "Four parties" }),
      step("pie", { set: { shape: "pie" }, title: "Shares of the whole" }),
    ],
  }),
});
```

Three ideas carry the whole thing — they're explained in [core concepts](/docs/concepts/):

- **`key: "party"`** gives every row an identity, so party A's bar *becomes* party A's slice instead of fading out.
- **The signal `shape`** says which chart is showing; each chart's `when` reads it.
- **The program** is two steps; each sets the signal. Nothing else — the engine plans the motion.

## 4. Preview it live

```sh
datars dev sales/doc.ts
```

This opens a live page (on `http://127.0.0.1:8788` by default) with the chart, its states, its diagnostics and its lint findings. Save the file and the page rebuilds; the chart **morphs from what's on screen to your edit** rather than reloading, so you see what a change does. Local recipes under `recipes/` are watched too. Click any mark to see why it looks the way it does; switch to a phone width or dark mode from the toolbar.

<figure class="shot"><img class="thumb thumb-light" src="/img/dev-explain.png" alt="The datars dev page: a bar chart with its states under it, and the Explain panel listing the recipes, data row and expression values behind the clicked bar" width="1280" height="760" loading="lazy"><img class="thumb thumb-dark" src="/img/dev-explain-dark.png" alt="" aria-hidden="true" width="1280" height="760" loading="lazy"><figcaption>The live page: the chart, its states, and — for the bar you clicked — the recipes that made it, its data row and every expression's value.</figcaption></figure>

## 5. Check it

```sh
datars check sales/doc.ts     # does it load? unknown fields, missing fonts, bad expressions
datars lint sales/doc.ts      # chart checks: keys, encodings, legibility, contrast
```

`check` reports mistakes with the closest valid name (“recipe `@datars/std/bar`: has no setting labls (did you mean `labels`?)”). `lint` is about the chart, not the syntax: sources without keys, transitions where most marks would cross-fade instead of move, labels that overlap or are cut off at the edge, too many categorical colours, interactive marks without an accessible label, a map without its OpenStreetMap credit, and the theme's contrast checks. Both run in the live page too. More in [preview, check and debug](/docs/tools/).

When you want to see a transition frame by frame, `datars film sales/doc.ts --from 0 --to 1` writes a filmstrip with motion trails; `datars render sales/doc.ts --state pie --out pie.png` renders one state.

## 6. Publish it

```sh
datars publish sales/doc.ts --alias sales --to site/
datars serve site/
```

`publish` compiles the document into a bundle and writes the static delivery layout:

```text
site/c/sales            the manifest (the alias): embeds follow it; republishing rewrites it
site/chunks/b3_…        content-addressed chunks: immutable, cache them forever
```

Upload `site/` to any static host. `datars serve site/` tries it locally (on port 8787): a small server with an index page that embeds every chart it finds. Each bundle carries an SVG poster and a text alternative, pre-expanded variants for different runtimes and the fonts the chart draws with — see [publishing and hosting](/docs/publishing/).

## 7. Embed it

Copy the web runtime next to your page, from `node_modules/@datars/web/dist`: `datars.js` and, from `wasm/`, the files starting with `datars_core`, `datars_host_web` and `wasi_shim`. Then:

```html
<script type="module" src="runtime/datars.js"></script>

<datars-view src="c/sales"></datars-view>
```

That's the whole integration. The element loads the smaller engine unless a chart needs the recipe sandbox, shows the poster if loading is slow, pauses off screen, follows the reader's light/dark and reduced-motion settings, and gives screen readers the chart's content. Arrow keys step through the story when it has focus. Its attributes, events and methods are in [Web: `<datars-view>`](/docs/embed/web/).

Republish the chart and every page that embeds it shows the new version — no site deploy.

## 8. Put it in an app

The same published chart plays natively in apps:

```swift
// iOS / macOS, SwiftUI (DatarsKit)
DatarsChart(source: .url(URL(string: "https://charts.example.com/c/sales")!))
```

```kotlin
// Android
val chart = DatarsView(context)
chart.load("https://charts.example.com/c/sales")
```

See [iOS and macOS](/docs/embed/ios/) and [Android](/docs/embed/android/).

## Where next

- Swap the template's numbers for your own: [data and tables](/docs/data/) covers CSV, files, URLs and live data.
- Browse [the chart reference](/docs/std/) — {{count:recipes}} recipes, each with a live example and its source.
- Make it yours: [themes and brands](/docs/theming/).
- Tell a longer story: [states and stories](/docs/stories/), and [motion](/docs/motion/) to choreograph it.
