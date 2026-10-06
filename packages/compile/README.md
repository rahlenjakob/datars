# @datars/compile

The Node side of the datars bundler plugins (`@datars/vite`, `@datars/next`), for writing another
one (Astro, Rspack, a CMS build step). You don't install it yourself for Vite or Next.js.

```ts
import { compileChart, publishChart, findRuntime, runtimeFiles } from "@datars/compile";

const c = await compileChart("src/sales.chart.ts");
c.doc;          // the document (JSON IR)
c.size;         // [width, height] — the aspect a page reserves
c.states;       // the program's steps, by name
c.dependencies; // every file it was built from: watch them
c.assets;       // local files it reads, and where their URLs are in the document
c.warnings;     // what a raw document can't do (Google fonts, uncut basemaps)
c.publish;      // the file's own `export const publish`

const p = await publishChart("src/sales.chart.ts", { alias: "sales", outDir: "site/datars", cacheDir: ".cache" });
p.manifest;     // "c/sales"; p.files: every file of it in outDir (chunks are shared)

const rt = findRuntime(process.cwd()); // @datars/web's dist, and a content hash for its folder name
runtimeFiles(rt.dir);                  // the files a page may load
```

Compiling works like the CLI's `compile_ts.mjs`: esbuild bundles the file against the project's own
`@datars/sdk` and `@datars/std`, Node evaluates it, the default export is the document; local
recipes (`@local/…`) and `{ name, file }` packages are embedded. Publishing runs `datars publish`
(`$DATARS`, else on PATH), caches the result by everything that went in, and merges it into a
shared delivery folder.
