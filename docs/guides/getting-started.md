# Getting started

This guide is for developers and coding agents. (Data scientists in notebooks: [python.md](python.md).) The loop is: **write a document → check → render →
film the transitions → lint → test → bundle.** Everything runs locally, without a GPU or a browser.

## 1. A document

Documents are built with `@datars/sdk` and the standard library `@datars/std`:

```ts
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, pie } from "@datars/std";

export default doc({
  title: "Vote share",
  size: [720, 440],
  data: { votes: data.values({ party: ["S", "SD", "M"], share: [30.3, 20.5, 19.1] }, { key: "party" }) },
  keys: { S: { name: "Social Democrats", color: "#e8112d" } },   // colours per key are data
  signals: { shape: signal.str("bars") },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "votes", x: "party", y: "share", color: "party", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
    pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
  ] }),
  program: story({ steps: [step("bars", { set: { shape: "bars" } }), step("pie", { set: { shape: "pie" } })] }),
});
```

Things to know:

- **Keys are identity.** `key: "party"` makes each party one element across every state, so the bar
  morphs into its slice. Without keys, transitions pair rows by position (the linter warns).
- **Expressions** (`e("…")`, or lambdas `d => d.share * 2`) run in the engine per row and per frame.
  They see the row (`d.field`), signals, scales (`scale.x(v)`, `scale.x.bandwidth()`), `box.w`/`box.h`,
  `format(v, ",.1f")`, `key.name(k)`, `token("size.label")`.
- **Colours are inks**: `"$accent"`, `"$categorical[2]"`, `"#e8112d"` — theme tokens stay late-bound,
  so dark mode and host apps can restyle without re-publishing ([themes](../18-themes.md)).
- **Recipes** are ordinary code: `datars eject std/bar` copies one into `recipes/bar.ts`; import it
  from there instead of `@datars/std` and edit freely — the document embeds it when it's built.
  Unedited, it renders exactly what the std recipe renders.

## 2. The loop

```sh
datars check   doc.ts                         # diagnostics
datars render  doc.ts --state 1               # PNG (CPU reference — identical everywhere)
datars inspect doc.ts --state 1               # the scene as text: keys, geometry, inks, roles
datars film    doc.ts --from 0 --to 1         # filmstrip + motion trails of the transition
datars semantics doc.ts                       # what a screen reader gets
datars lint    doc.ts                         # identity, encoding, legibility, accessibility, theme
datars profile doc.ts                         # where the time goes (resolve, plan, frame, raster)
datars test                                   # snapshots, dense transition sweeps, motion invariants
datars bundle  doc.ts --explain               # publish: T0 poster → T3 source variants, sizes per chunk
datars video   doc.ts --size 360x640 --dpr 3  # the program as a film (+ WebVTT captions)
```

Agents can use the same tools through MCP: `datars-mcp` (stdio) exposes `check`, `render`, `film`,
`inspect`, `lint`, `semantics`, `describe_recipes`, `run_tests`, `bundle`.

## 3. Shipping

`datars bundle` writes a `.datars` file; `datars publish doc.ts --alias votes --to site/` writes the
static delivery layout (`site/c/votes` + `site/chunks/`) that any static host serves —
`datars serve site/` tries it locally. Embed it:

```html
<script type="module" src="/runtime/datars.js"></script>
<datars-view src="/c/votes"></datars-view>
```

Attributes: `scrub` (scroll-driven stories), `mode="dark"`, `no-script` (refuse T3 bundles). The
element loads the smaller core engine unless a chart needs on-device recipes, sleeps between
autoplay holds, pauses off-screen, and follows `prefers-reduced-motion`.

In React apps (Vite, Next.js): chart files imported like modules, no build scripts — [Using datars with React](react.md).

In apps: `DatarsChart(source: .url(…))` (SwiftUI), `DatarsView.load(url)` (Android). Republishing
the bundle updates every embed; no app release or site deploy.
