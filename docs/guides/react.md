# Using datars with React (Vite, Next.js)

Charts are TypeScript files you import like any module. A bundler plugin compiles them in Node
(with your project's own `@datars/sdk` and `@datars/std`), `<DatarsView>` shows them, and the web
runtime (engines, fonts, atlas) is served and shipped for you. No build scripts.

```tsx
import sales from "./sales.chart.ts";
import { DatarsView } from "@datars/react";

export function Report() {
  return <DatarsView chart={sales} />;
}
```

| Package | What it does |
|---|---|
| `@datars/react` | `<DatarsView>`: SSR-safe, sized before it loads, state/signals/tokens/data as props, the element's API on the ref, `useDatarsState` |
| `@datars/vite` | Vite plugin: chart imports, HMR, the runtime served in dev and emitted in the build |
| `@datars/next` | `withDatars(nextConfig)`: chart imports for Turbopack and webpack, App and Pages Router |
| `@datars/compile` | the Node side both plugins share (you don't install it yourself) |

## Vite + React

```sh
npm install @datars/react @datars/sdk @datars/std
npm install -D @datars/vite
```

```ts
// vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import datars from "@datars/vite";

export default defineConfig({ plugins: [react(), datars()] });
```

A chart file is any file named `*.chart.ts` (or `.tsx`, `.mts`, `.js`, `.mjs`) whose default
export is a document:

```ts
// src/votes.chart.ts
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, pie } from "@datars/std";

export default doc({
  title: "Vote share",
  size: [720, 440],                                   // the aspect <DatarsView> reserves
  data: { votes: data.url("./votes.csv", { key: "party" }) },   // files next to it just work
  signals: { shape: signal.str("bars") },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    plot({ data: "votes", x: "party", y: "share", color: "party", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
    pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
  ] }),
  program: story({ steps: [step("bars", { set: { shape: "bars" } }), step("pie", { set: { shape: "pie" } })] }),
});
```

A file that can't be renamed (a `doc.ts` other tools use) is imported with a query:
`import votes from "./stories/votes/doc.ts?datars"`. For its type, add `@datars/vite/client` to
`compilerOptions.types` (next to `vite/client`).

### A story with buttons, a theme switch

```tsx
import { useRef, useState } from "react";
import { DatarsView, useDatarsState, type DatarsViewHandle } from "@datars/react";
import votes from "./votes.chart.ts";

export function Votes() {
  const ref = useRef<DatarsViewHandle>(null);
  const s = useDatarsState(ref);                       // null until the chart has drawn
  const [mode, setMode] = useState<"light" | "dark">("light");
  const steps = s?.states ?? votes.states ?? [];      // the steps are known before it loads
  return (
    <figure>
      <DatarsView ref={ref} chart={votes} mode={mode} />
      {steps.map((name) => (
        <button key={name} aria-pressed={s?.state === name} onClick={() => ref.current?.goto(name)}>{name}</button>
      ))}
      <button onClick={() => setMode(mode === "dark" ? "light" : "dark")}>{mode}</button>
    </figure>
  );
}
```

`npm run dev`: edit `votes.chart.ts` (or the CSV, or a module it imports) and the chart morphs in
place, on the step it was on. `npm run build` writes `dist/` with the runtime under
`dist/datars/runtime-<hash>/`; `npm run preview` serves it.

## Next.js

```sh
npm install @datars/react @datars/sdk @datars/std
npm install -D @datars/next
```

```ts
// next.config.ts
import { withDatars } from "@datars/next";

export default withDatars({ /* your config */ });
```

Chart files are imported the same way, in the App Router or the Pages Router, with Turbopack (the
default) or `--webpack`:

```tsx
// app/page.tsx — a server component
import { DatarsView } from "@datars/react";
import votes from "../charts/votes.chart";

export default function Page() {
  // Compiled at build time; the server renders the chart's box into the HTML, the browser
  // hydrates it and the chart appears in it.
  return <DatarsView chart={votes} />;
}
```

`<DatarsView>` is a client component (`"use client"` is in the package), so server components can
render it; hooks (`useDatarsState`, refs) belong in your own client components, as in the Vite
example. A chart import is a plain object, so a server component can pass it down as a prop.

The runtime is copied to `public/_datars/runtime-<hash>/` when Next loads its config (the folder
has a `.gitignore` of its own), and the files your charts read to `public/_datars/assets/`. With
a `basePath`, URLs follow it.

## Raw documents or published bundles

By default a chart import carries the **raw document** and plays on the full engine, which
expands recipes in the browser. That needs nothing but Node. **Publish mode** runs the `datars`
CLI's `publish` at dev and build time instead, and the import carries the URL of a published
bundle:

| | raw document (default) | publish mode |
|---|---|---|
| needs | Node | the `datars` CLI (`$DATARS`, else on PATH) |
| engine | full (`datars_host_web`, ~1.8 MB gzip) | core (`datars_core`, ~1.4 MB gzip) |
| first paint | the chart when the engine is ready | a poster (T0 SVG) while it loads, on slow starts |
| fonts | default Inter, `font.file(…)` | subset and shipped, **Google Fonts too** |
| automatic basemaps (`data.tiles.auto()`) | only the archive already cut next to the document | cut from the geodata cache |
| data | the document's files, as assets | embedded in the bundle's chunks |
| page JS | the document is in your bundle | a manifest URL; chunks load on demand |
| HMR | morphs in place (~100 ms) | republished, swapped in (~1–3 s) |

Turn it on for everything, for some files, or for one import:

```ts
datars({ publish: true })                              // Vite
datars({ publish: (file) => file.includes("/maps/") })
withDatars(config, { publish: true })                  // Next.js
```
```ts
import map from "./map.chart.ts?datars&publish";       // one import (Vite); ?datars&raw turns it off
export const publish = true;                           // in the chart file itself (Vite and Next.js)
```

The plugin warns when a raw document uses something only publish mode ships (a Google font, an
automatic basemap that hasn't been cut). Published charts are cached in `node_modules/.vite` /
`node_modules/.cache`, keyed by every file the chart reads, so a restart doesn't publish again.
Pass CLI environment with `env` (e.g. `{ DATARS_OVERPASS_BUDGET: "0,0" }` to build basemaps from the
local cache only).

## How it works

**Chart imports.** The plugin bundles the chart file with esbuild, evaluates it in Node and takes
the default export — the same approach as the CLI. The module it becomes is
`{ kind: "datars-chart", id, size, title, states, doc }` (or `src` instead of `doc`). Local files the
document reads by relative URL (`data.url("./x.csv")`, tile archives, `font.file`) become assets:
`?url` imports in Vite (hashed, or inlined when tiny), copies under their content hash in
`public/_datars/assets/` in Next.js. `<DatarsView chart>` also takes a bare document object.

**HMR.** Every file the chart was built from is watched: the chart file, modules it imports
(a theme, shared helpers), JSON it imports and data files it reads. In Vite the chart module
accepts its own update and hands the new chart to the `<DatarsView>`s showing it; in Next.js the
new module reaches the component through Fast Refresh (or a server component refresh). A new
document morphs from what's on screen, keeping the step (`setDocument`); a new bundle URL is a new
element that replaces the old one once it has drawn, at the same step. The page doesn't reload
and React state is kept.

**The runtime.** `@datars/web` loads its engine with a native `import()` of a URL it computes at
run time, and fetches fonts and the atlas the same way — things a bundler can't see. So the
plugins put its files (both engines, the WASI shim, default fonts, the atlas) in one public folder
named by their content hash, and point the runtime at it: Vite serves it in dev
(`/datars/runtime-<hash>/`, with byte ranges) and emits it into the build; Next.js copies it to
`public/`. The `@datars/web` module itself is bundled with your code as a lazy chunk (pages
without charts never load it); Vite's dependency pre-bundling leaves it alone. Serving the runtime
from a CDN instead: `datars({ runtime: "https://cdn…/datars/web/0.1.0/" })`,
`withDatars(config, { runtime })`, or `<DatarsView runtime>`.

**No layout shift.** `<DatarsView>` renders a box: `aspect-ratio` from the document's size (or
`height`, or `aspectRatio`), identical on the server and on the first client render. The
`<datars-view>` element is created after hydration, positioned inside the box, and follows the
box's height — it never sizes the page. Its own arrow buttons are off (`no-controls`): steppers are
yours, and can be drawn before the chart loads (`chart.states`).

**Near the viewport only.** By default a chart mounts when its box comes within 150% of the
viewport's height and unmounts far away (every chart on a page shares one engine memory); the step
a reader was on comes back with it. `lazy={false}` mounts after hydration and stays;
`lazy="50% 0px"` sets the margin.

## `<DatarsView>`

| Prop | |
|---|---|
| `chart` | a chart import, or a document object |
| `document` | a document (object or JSON text) the app holds — a new one morphs from what's shown |
| `src` / `doc` | a published bundle's manifest URL / a raw document's URL |
| `state` | the step to go to (name or index) when it changes; also where the chart opens |
| `mode` | `"light"`, `"dark"`, `"high-contrast"` (default: `prefers-color-scheme`) |
| `reducedMotion` | `"reduce"` or `"no-preference"` (a reader who opted in to the full motion); default: `prefers-reduced-motion` |
| `height`, `aspectRatio` | the box's size (default: the document's aspect across the width) |
| `signals`, `tokens`, `data` | signals, theme token overrides, data slots — applied as they change, and before the first frame |
| `lazy` | mount near the viewport (default `true`), `false`, or a rootMargin |
| `label` | accessible name (default: from the chart's semantics) |
| `attributes` | other `<datars-view>` attributes: `steps`, `scrub`, `cpu`, `no-script`, `perf`… |
| `runtime` | where the runtime's files are served from (the plugins set it) |
| `onStateChange`, `onReady`, `onError` | the step changed / the first frame / the runtime failed to load |
| `className`, `style`, … | on the box (a `div`) |

The ref (`DatarsViewHandle`): `goto(name | index)`, `next()`, `prev()`, `seek(position)` (a position
in states; a fraction is the transition that far through), `send(event)`,
`setSignal(name, value)`, `setTokens(tokens)`, `setData(name, rows)`, `element`, `status`,
`subscribe(listener)`. `useDatarsState(ref)` re-renders with `{ state, index, states, narration }`.

A scroll story: `<DatarsView chart={story} height="100%" attributes={{ steps: "#story .step" }} />`
inside a sticky container — each `.step` crossing the middle of the screen moves the chart.

## Plugin options

`datars(options)` (Vite) and `withDatars(config, options)` (Next.js):

| Option | |
|---|---|
| `publish` | publish mode: `true`, or (Vite) a function of the file |
| `cli` | the CLI for publish mode (default `$DATARS`, else `datars`) |
| `env` | extra environment for the CLI |
| `runtime` | load the runtime from a URL instead of shipping it |
| `include` (Vite) / `extensions` (Next.js) | which files are chart files |
| `path` | the public folder (default `datars/` in Vite, `_datars` in `public/` for Next.js) |

## Notes

- **Base URLs.** Vite: an absolute `base` (`/`, `/app/`) or a relative one (`./`) both work — with
  `./`, URLs are computed from the chunk's own location, so the site works from any folder. Next.js:
  `basePath` is followed; `assetPrefix` doesn't apply to `public/`.
- **Monorepos and linked packages.** The Vite plugin allows the dev server to read `@datars/web`
  and `@datars/react` where they really are; with `link:`/`file:` dependencies add
  `resolve: { dedupe: ["react", "react-dom"] }`.
- **TypeScript.** `import x from "./x.chart.ts"` is typed from the file (the document), which
  `<DatarsView chart>` takes; `?datars` imports are typed by `@datars/vite/client`. Importing `.ts`
  extensions needs `allowImportingTsExtensions` (or drop the extension).
- **Without a bundler plugin** (another framework, a CMS): publish with the CLI and use
  `<DatarsView src="/c/votes" aspectRatio={16 / 9} />`, serving `@datars/web/dist` somewhere and
  passing `runtime`; or use `<datars-view>` directly ([getting started](getting-started.md)).
