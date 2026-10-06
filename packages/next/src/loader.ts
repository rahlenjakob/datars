// The chart-file loader (webpack and Turbopack): a chart file becomes a plain, serializable object
// — `{ kind, id, size, title, doc }`, or `{ …, src }` in publish mode — that server components can
// hand to <DatarsView>. Files the document reads are copied to `public/<path>/assets/` under their
// content hash; published charts are merged into `public/<path>/` (`c/`, `chunks/`).
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { aliasFor, assetFileName, compileChart, publishChart, setAt } from "@datars/compile";
import type { LoaderOptions } from "./index.js";

interface LoaderContext {
  resourcePath: string;
  resourceQuery?: string;
  addDependency(file: string): void;
  cacheable?(flag?: boolean): void;
  emitWarning?(e: Error): void;
}

export async function load(ctx: LoaderContext, options: LoaderOptions): Promise<string> {
  ctx.cacheable?.(true);
  const file = ctx.resourcePath;
  const query = new URLSearchParams((ctx.resourceQuery ?? "").replace(/^\?/, ""));
  const id = relative(options.root, file).replace(/\\/g, "/");
  const c = await compileChart(file);
  for (const d of c.dependencies) if (!d.includes(`${sep}node_modules${sep}`)) ctx.addDependency(d);
  const publish = query.has("publish") || (!query.has("raw") && (c.publish ?? options.publish));
  if (publish) {
    mkdirSync(options.publicDir, { recursive: true });
    const p = await publishChart(file, { alias: aliasFor(file, options.root), outDir: options.publicDir, cli: options.cli, env: options.env, cacheDir: join(options.root, "node_modules", ".cache", "datars", "published") }, c);
    for (const d of p.dependencies) if (!d.includes(`${sep}node_modules${sep}`)) ctx.addDependency(d);
    const chart = { kind: "datars-chart", id, size: p.size, title: p.title ?? null, states: p.states, src: `${options.publicPath}/${p.manifest}` };
    return `export default ${JSON.stringify(chart)};\n`;
  }
  for (const w of c.warnings) ctx.emitWarning?.(new Error(`${id}: ${w}`));
  // The files the document reads, public under their content hash.
  const doc = c.doc;
  if (c.assets.length) mkdirSync(join(options.publicDir, "assets"), { recursive: true });
  for (const a of c.assets) {
    const name = assetFileName(a.file);
    const to = join(options.publicDir, "assets", name);
    if (!existsSync(to)) copyFileSync(a.file, to);
    setAt(doc, a.at, `${options.publicPath}/assets/${name}`);
    ctx.addDependency(a.file);
  }
  const chart = { kind: "datars-chart", id, size: c.size, title: c.title ?? null, states: c.states, doc };
  return `export default JSON.parse(${JSON.stringify(JSON.stringify(chart))});\n`;
}
