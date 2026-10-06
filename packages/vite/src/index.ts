// @datars/vite — datars charts in a Vite app without build scripts.
//
//   // vite.config.ts
//   import datars from "@datars/vite";
//   export default defineConfig({ plugins: [react(), datars()] });
//
//   // App.tsx
//   import sales from "./sales.chart.ts";        // or "./stories/votes/doc.ts?datars"
//   <DatarsView chart={sales} />
//
// A chart file is compiled in Node (esbuild + the project's own @datars/sdk and @datars/std) when
// it's imported; the module it becomes carries the document (or, in publish mode, the URL of a
// bundle the datars CLI published). Edits reach the page through HMR and the chart morphs in place.
// The runtime's engines, fonts and atlas are served in dev and emitted into the build under one
// public folder (`<base>datars/`), and the runtime is pointed at it.
import { existsSync, mkdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import type { IncomingMessage, ServerResponse } from "node:http";
import { dirname, join, relative, resolve, sep } from "node:path";
import type { Plugin, ResolvedConfig } from "vite";
import { aliasFor, compileChart, findRuntime, mimeOf, publishChart, runtimeFiles, type Runtime } from "@datars/compile";

export interface DatarsPluginOptions {
  /** Which imported files are chart files (default: `*.chart.ts|tsx|mts|js|mjs`). An import with
   * a `?datars` query is always one (`import votes from "./votes/doc.ts?datars"`). */
  include?: RegExp | ((file: string) => boolean);
  /**
   * Publish charts with the datars CLI instead of shipping the raw document (default `false`).
   * A published chart plays on the smaller core engine, shows a poster while loading, ships its
   * fonts subset (Google Fonts too) and its automatic basemaps; it needs the `datars` CLI
   * (`$DATARS`, else on PATH) at dev and build time. `true`, or a function of the chart file.
   * Per import: `?datars&publish` (on) or `?datars&raw` (off).
   */
  publish?: boolean | ((file: string) => boolean);
  /** The datars CLI for publish mode (default `$DATARS`, else `datars` on PATH). */
  cli?: string;
  /** Extra environment for the CLI (e.g. `{ DATARS_OVERPASS_BUDGET: "0,0" }`). */
  env?: Record<string, string | undefined>;
  /** The public folder for the runtime and published charts, under Vite's `base` (default `"datars/"`). */
  path?: string;
  /** Load the runtime's engines from elsewhere (a CDN serving @datars/web's `dist/`): nothing is
   * emitted for it. */
  runtime?: string;
}

const CHART = /\.chart\.(ts|tsx|mts|js|mjs|json)$/;

export default function datars(options: DatarsPluginOptions = {}): Plugin {
  let config: ResolvedConfig;
  let runtime: Runtime | null = null;
  let path = (options.path ?? "datars/").replace(/^\/+/, "");
  if (!path.endsWith("/")) path += "/";
  /** Where published charts are merged (dev serves it; build emits from it). */
  let site = "";
  let publishCache = "";
  let isBuild = false;
  /** Files already emitted in this build (charts share chunks). */
  const emitted = new Map<string, string>();
  /** Runtime-relative public path, e.g. `/datars/runtime-1a2b3c/`. */
  const runtimeDirName = () => `${path}runtime-${runtime!.hash}/`;
  const absoluteBase = () => config.base.startsWith("/") || /^https?:/.test(config.base);

  const isChart = (file: string, query: URLSearchParams) => {
    if (query.has("datars")) return true;
    if (query.has("url") || query.has("raw") || query.has("inline")) return false;
    if (typeof options.include === "function") return options.include(file);
    return (options.include ?? CHART).test(file);
  };
  /** Publish mode: the import's query, else the file's own `export const publish`, else the option. */
  const wantsPublish = (file: string, query: URLSearchParams, own: boolean | undefined) => {
    if (query.has("publish")) return true;
    if (query.has("raw")) return false;
    if (own !== undefined) return own;
    return typeof options.publish === "function" ? options.publish(file) : !!options.publish;
  };

  return {
    name: "datars",
    enforce: "pre",

    config() {
      return {
        // The runtime loads its engine with a native `import()` of a URL known at run time, and
        // must reach the transform below (it's pointed at the runtime folder there): not pre-bundled.
        optimizeDeps: { exclude: ["@datars/web"] },
      };
    },

    configResolved(resolved) {
      config = resolved;
      isBuild = resolved.command === "build";
      const cache = join(resolved.cacheDir, "datars");
      site = join(cache, isBuild ? "build-site" : "site");
      publishCache = join(cache, "published");
      try {
        runtime = findRuntime(resolved.root);
      } catch (e) {
        resolved.logger.warn(`[datars] ${(e as Error).message}`);
      }
      // Linked packages (a monorepo, `link:` deps) live outside the project: let the dev server
      // read the runtime and the React bindings from where they really are.
      const allow = resolved.server?.fs?.allow;
      if (allow && runtime) {
        for (const d of [dirname(runtime.dir), packageDir(resolved.root, "@datars/react")]) {
          if (!d) continue;
          const real = safeRealpath(d);
          if (!allow.some((a) => real === a || real.startsWith(a.endsWith(sep) ? a : a + sep))) allow.push(real);
        }
      }
    },

    configureServer(server) {
      const prefix = `${config.base.replace(/\/?$/, "/")}${path}`;
      server.middlewares.use((req: IncomingMessage, res: ServerResponse, next: () => void) => {
        const url = req.url?.split("?")[0] ?? "";
        if (!url.startsWith(prefix)) return next();
        const rel = decodeURIComponent(url.slice(prefix.length));
        let file = "";
        if (runtime && rel.startsWith(`runtime-${runtime.hash}/`)) {
          const r = rel.slice(`runtime-${runtime.hash}/`.length);
          if (runtimeFiles(runtime.dir).includes(r)) file = join(runtime.dir, r);
        } else if (!rel.split("/").includes("..")) {
          file = join(site, rel);
        }
        if (!file || !existsSync(file) || !statSync(file).isFile()) return next();
        sendFile(req, res, file, rel);
      });
    },

    async load(id) {
      const [file, q = ""] = id.split("?");
      const query = new URLSearchParams(q);
      if (!isChart(file, query) || !existsSync(file)) return null;
      const chartId = relative(config.root, file).replace(/\\/g, "/");
      const c = await compileChart(file);
      if (!wantsPublish(file, query, c.publish)) {
        for (const d of c.dependencies) if (!d.includes(`${sep}node_modules${sep}`)) this.addWatchFile(d);
        for (const w of c.warnings) this.warn(`${chartId}: ${w}`);
        // Files the document reads become Vite assets (`?url`): served in dev, hashed in the build.
        const imports = c.assets.map((a, i) => `import __datars_asset_${i} from ${JSON.stringify(`${a.file}?url`)};`);
        const sets = c.assets.map((a, i) => `__datars_set(doc, ${JSON.stringify(a.at)}, __datars_asset_${i});`);
        return {
          code: [
            ...imports,
            `const doc = JSON.parse(${JSON.stringify(JSON.stringify(c.doc))});`,
            `function __datars_set(o, at, v) { for (const k of at.slice(0, -1)) o = o[k]; o[at[at.length - 1]] = v; }`,
            ...sets,
            `const chart = { kind: "datars-chart", id: ${JSON.stringify(chartId)}, size: ${JSON.stringify(c.size)}, title: ${JSON.stringify(c.title ?? null)}, states: ${JSON.stringify(c.states)}, doc };`,
            `export default chart;`,
            hmr(),
          ].join("\n"),
          map: { mappings: "" },
        };
      }
      // Publish mode: the CLI publishes into the delivery folder; the module is the manifest URL.
      mkdirSync(site, { recursive: true });
      const alias = aliasFor(file, config.root);
      const p = await publishChart(file, { alias, outDir: site, cli: options.cli, env: options.env, cacheDir: publishCache }, c);
      for (const d of p.dependencies) if (!d.includes(`${sep}node_modules${sep}`)) this.addWatchFile(d);
      let src: string;
      if (isBuild) {
        let manifestRef = "";
        for (const rel of p.files) {
          const fileName = `${path}${rel}`;
          const source = readFileSync(join(site, rel));
          const ref = emitOnce(this, emitted, fileName, source);
          if (rel === p.manifest) manifestRef = ref;
        }
        src = absoluteBase() ? JSON.stringify(`${config.base.replace(/\/?$/, "/")}${path}${p.manifest}`) : `import.meta.ROLLUP_FILE_URL_${manifestRef}`;
      } else {
        // A new URL per version: the page sees the chart changed (HMR) and loads it again.
        const v = hashFile(join(site, p.manifest));
        src = JSON.stringify(`${config.base.replace(/\/?$/, "/")}${path}${p.manifest}?v=${v}`);
      }
      return {
        code: [
          `const chart = { kind: "datars-chart", id: ${JSON.stringify(chartId)}, size: ${JSON.stringify(p.size)}, title: ${JSON.stringify(p.title ?? null)}, states: ${JSON.stringify(p.states)}, src: ${src} };`,
          `export default chart;`,
          hmr(),
        ].join("\n"),
        map: { mappings: "" },
      };
    },

    // The runtime (@datars/web) learns where its engines, fonts and atlas are served from.
    transform(code, id) {
      if (!runtime || !isRuntimeModule(id, runtime)) return null;
      if (options.runtime !== undefined) return { code: `${code}\nsetRuntimeBase(${JSON.stringify(options.runtime)});\n`, map: null };
      const env = (this as unknown as { environment?: { name?: string; config?: { consumer?: string } } }).environment;
      const ssr = env ? env.config?.consumer === "server" : !!config.build.ssr;
      if (!isBuild) return { code: `${code}\nsetRuntimeBase(${JSON.stringify(`${config.base.replace(/\/?$/, "/")}${runtimeDirName()}`)});\n`, map: null };
      if (ssr) return null;
      // Build: the runtime's files go out with the app, under their content hash.
      let anchor = "";
      for (const rel of runtimeFiles(runtime.dir)) {
        const ref = emitOnce(this, emitted, `${runtimeDirName()}${rel}`, readFileSync(join(runtime.dir, rel)));
        if (rel === "wasm/wasi_shim.js" || (!anchor && rel.startsWith("wasm/"))) anchor = `${ref}|${rel}`;
      }
      const base = absoluteBase()
        ? JSON.stringify(`${config.base.replace(/\/?$/, "/")}${runtimeDirName()}`)
        : `import.meta.ROLLUP_FILE_URL_${anchor.split("|")[0]}.slice(0, -${anchor.split("|")[1].length})`;
      return { code: `${code}\nsetRuntimeBase(${base});\n`, map: null };
    },

    buildStart() {
      emitted.clear();
    },
  };
}

/** The chart module accepts its own updates and hands the new chart to the views showing it
 * (`<DatarsView>` listens): the chart morphs in place, the app keeps its state. */
function hmr(): string {
  return `if (import.meta.hot) import.meta.hot.accept((m) => { if (m && typeof dispatchEvent === "function") dispatchEvent(new CustomEvent("datars:update", { detail: m.default })); });`;
}

type EmitContext = { emitFile(f: { type: "asset"; fileName: string; source: Uint8Array | string }): string };

/** Emit an asset once per build (charts share chunks; the runtime is emitted with the first
 * module that loads it). */
function emitOnce(ctx: EmitContext, emitted: Map<string, string>, fileName: string, source: Uint8Array): string {
  const hit = emitted.get(fileName);
  if (hit) return hit;
  const ref = ctx.emitFile({ type: "asset", fileName, source });
  emitted.set(fileName, ref);
  return ref;
}

function isRuntimeModule(id: string, runtime: Runtime): boolean {
  const file = id.split("?")[0];
  if (file === join(runtime.dir, "index.js") || file === join(runtime.dir, "datars.js")) return true;
  // The same file through a symlink (linked packages).
  try {
    const real = realpathSync(runtime.dir);
    return file === join(real, "index.js") || file === join(real, "datars.js");
  } catch {
    return false;
  }
}

function packageDir(root: string, name: string): string | null {
  let dir = resolve(root);
  for (;;) {
    const p = join(dir, "node_modules", name);
    if (existsSync(join(p, "package.json"))) return p;
    const up = dirname(dir);
    if (up === dir) return null;
    dir = up;
  }
}

function safeRealpath(p: string): string {
  try {
    return realpathSync(p);
  } catch {
    return p;
  }
}

function hashFile(file: string): string {
  let h = 0;
  const b = readFileSync(file);
  for (let i = 0; i < b.length; i++) h = (Math.imul(h, 31) + b[i]) | 0;
  return (h >>> 0).toString(36);
}

/** Serve a runtime or delivery file, with byte ranges (tile archives are read by range). */
function sendFile(req: IncomingMessage, res: ServerResponse, file: string, rel: string) {
  const body = readFileSync(file);
  res.setHeader("Content-Type", mimeOf(rel));
  res.setHeader("Accept-Ranges", "bytes");
  res.setHeader("Cache-Control", rel.startsWith("runtime-") || rel.startsWith("chunks/") ? "public, max-age=31536000, immutable" : "no-cache");
  const range = /^bytes=(\d*)-(\d*)$/.exec(String(req.headers.range ?? ""));
  if (range) {
    const start = range[1] ? Number(range[1]) : Math.max(0, body.length - Number(range[2]));
    const end = range[1] && range[2] ? Math.min(Number(range[2]), body.length - 1) : body.length - 1;
    if (start > end || start >= body.length) {
      res.statusCode = 416;
      res.setHeader("Content-Range", `bytes */${body.length}`);
      return res.end();
    }
    res.statusCode = 206;
    res.setHeader("Content-Range", `bytes ${start}-${end}/${body.length}`);
    res.setHeader("Content-Length", String(end - start + 1));
    return res.end(req.method === "HEAD" ? undefined : body.subarray(start, end + 1));
  }
  res.setHeader("Content-Length", String(body.length));
  res.end(req.method === "HEAD" ? undefined : body);
}

export { datars };
