# @datars/next

Next.js integration for [datars](../../README.md): import chart files in the App or Pages Router,
with Turbopack or webpack.

```ts
// next.config.ts
import { withDatars } from "@datars/next";
export default withDatars({ /* your config */ });
```

```tsx
// app/page.tsx — a server component
import { DatarsView } from "@datars/react";
import votes from "../charts/votes.chart";

export default function Page() {
  return <DatarsView chart={votes} />; // the server renders the chart's box; the chart hydrates into it
}
```

- `*.chart.ts|tsx|mts|js|mjs` files go through a loader (Turbopack `turbopack.rules`, a webpack
  rule) that compiles them in Node with the project's own `@datars/sdk` and `@datars/std` into a
  plain object — serializable, so server components can pass it to the client component.
- The runtime's engines, fonts and atlas are copied to `public/_datars/runtime-<hash>/` when Next
  loads the config (the folder ignores itself for git); files the charts read go to
  `public/_datars/assets/`. `basePath` is followed. The runtime learns the folder through
  `process.env.DATARS_RUNTIME`.
- HMR through Fast Refresh: an edited chart morphs in place.
- **Publish mode** (`withDatars(config, { publish: true })`, or `export const publish = true` in a
  chart file) publishes with the `datars` CLI into `public/_datars/`.

Options: `publish`, `cli`, `env`, `path`, `runtime`, `extensions`, `root`.

Everything else: [Using datars with React](../../docs/guides/react.md).
