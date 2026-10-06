# @datars/vite

A Vite plugin for [datars](../../README.md): import chart files as modules.

```ts
// vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import datars from "@datars/vite";

export default defineConfig({ plugins: [react(), datars()] });
```

```tsx
import sales from "./sales.chart.ts";            // any *.chart.ts|tsx|mts|js|mjs
import votes from "./stories/votes/doc.ts?datars"; // any file, by query
<DatarsView chart={sales} />                      // @datars/react
```

- Chart files are compiled in Node with the project's own `@datars/sdk` and `@datars/std`
  (esbuild, as the CLI does). Files the document reads (`data.url("./x.csv")`, tile archives, font
  files) become Vite assets.
- **HMR:** editing the chart, a module it imports or a data file it reads updates the chart in
  place, on the step it was on.
- **The runtime** (engines, fonts, atlas from `@datars/web`) is served in dev and emitted into the
  build under `datars/runtime-<hash>/`, and the runtime is pointed at it — absolute or relative
  `base` alike.
- **Publish mode** (`publish: true`, per file, or `?datars&publish`): charts are published with the
  `datars` CLI — the smaller core engine, posters, fonts shipped (Google Fonts too), automatic
  basemaps — and emitted under `datars/c/` and `datars/chunks/`.

Options: `publish`, `cli`, `env`, `include`, `path`, `runtime`. Types for `?datars` imports:
`"types": ["vite/client", "@datars/vite/client"]`.

Everything else: [Using datars with React](../../docs/guides/react.md).
