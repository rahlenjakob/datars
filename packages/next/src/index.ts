// @datars/next — datars charts in a Next.js app without build scripts.
//
//   // next.config.ts
//   import { withDatars } from "@datars/next";
//   export default withDatars({ /* your config */ });
//
//   // app/page.tsx (a server component) — or any client component, or pages/
//   import sales from "./sales.chart";
//   import { DatarsView } from "@datars/react";
//   export default function Page() { return <DatarsView chart={sales} />; }
//
// Chart files (`*.chart.ts`) are compiled in Node by a loader — Turbopack and webpack alike — into
// a plain object (the document, or a published bundle's URL) that server components can pass to
// the client component. The runtime's engines, fonts and atlas are copied to `public/_datars/`
// (a self-ignoring folder) and the runtime is pointed at them (`process.env.DATARS_RUNTIME`).
import { cpSync, existsSync, mkdirSync, readdirSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { findRuntime, runtimeFiles } from "@datars/compile";

export interface DatarsNextOptions {
  /**
   * Publish charts with the datars CLI instead of shipping the raw document (default `false`):
   * the smaller core engine, a poster while loading, fonts subset and shipped (Google Fonts too),
   * automatic basemaps. Needs the `datars` CLI (`$DATARS`, else on PATH). A chart file can decide
   * for itself with `export const publish = true` (or false).
   */
  publish?: boolean;
  /** The datars CLI for publish mode (default `$DATARS`, else `datars` on PATH). */
  cli?: string;
  /** Extra environment for the CLI (e.g. `{ DATARS_OVERPASS_BUDGET: "0,0" }`). */
  env?: Record<string, string>;
  /** The folder under `public/` for the runtime, chart data and published charts (default `_datars`). */
  path?: string;
  /** Load the runtime's engines from elsewhere (a CDN serving @datars/web's `dist/`) instead of
   * copying them to `public/`. */
  runtime?: string;
  /** The project folder (default: the current directory, where Next runs). */
  root?: string;
  /** Chart file extensions the loader compiles (default `.chart.ts`, `.chart.tsx`, `.chart.mts`,
   * `.chart.js`, `.chart.mjs`). */
  extensions?: string[];
}

/** What the loader needs (JSON: Turbopack passes loader options across processes). */
export interface LoaderOptions {
  root: string;
  /** `public/<path>` on disk, and its URL (with the basePath). */
  publicDir: string;
  publicPath: string;
  publish: boolean;
  cli?: string;
  env?: Record<string, string>;
}

type NextConfig = Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
type NextConfigFn = (phase: string, ctx: { defaultConfig: NextConfig }) => NextConfig | Promise<NextConfig>;

const LOADER = fileURLToPath(new URL("../loader.cjs", import.meta.url));

/** Wrap a Next.js config: chart file imports, and the runtime served from `public/`. */
export function withDatars(nextConfig: NextConfig | NextConfigFn = {}, options: DatarsNextOptions = {}): NextConfig | NextConfigFn {
  if (typeof nextConfig === "function") {
    const fn = nextConfig;
    return async (phase, ctx) => withDatars(await fn(phase, ctx), options) as NextConfig;
  }
  const root = resolve(options.root ?? process.cwd());
  const folder = (options.path ?? "_datars").replace(/^\/+|\/+$/g, "");
  const basePath = (nextConfig.basePath as string | undefined) ?? "";
  const publicDir = join(root, "public", folder);
  const publicPath = `${basePath}/${folder}`;
  let runtimeUrl = options.runtime;
  if (!runtimeUrl) {
    const rt = findRuntime(root);
    copyRuntime(rt.dir, join(publicDir, `runtime-${rt.hash}`), publicDir);
    runtimeUrl = `${publicPath}/runtime-${rt.hash}/`;
  }
  // Plain JSON (no `undefined`s): Turbopack refuses loader options it can't serialize.
  const loaderOptions: LoaderOptions = JSON.parse(JSON.stringify({ root, publicDir, publicPath, publish: !!options.publish, cli: options.cli, env: options.env }));
  const exts = options.extensions ?? [".chart.ts", ".chart.tsx", ".chart.mts", ".chart.js", ".chart.mjs"];
  const test = new RegExp(`(${exts.map((e) => e.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})$`);
  const loader = { loader: LOADER, options: loaderOptions };

  const turbopackRules: Record<string, unknown> = {};
  for (const e of exts) turbopackRules[`*${e}`] = { loaders: [loader], as: "*.js" };

  return {
    ...nextConfig,
    env: { ...(nextConfig.env ?? {}), DATARS_RUNTIME: runtimeUrl },
    turbopack: {
      ...(nextConfig.turbopack ?? {}),
      rules: { ...(nextConfig.turbopack?.rules ?? {}), ...turbopackRules },
    },
    webpack(config: NextConfig, ctx: unknown) {
      // First, on the chart file's own source (before Next's TypeScript loader sees it).
      config.module.rules.push({ test, enforce: "pre", use: [loader] });
      config.module.rules.push({ resourceQuery: /(^|[?&])datars(&|$)/, enforce: "pre", use: [loader] });
      return typeof nextConfig.webpack === "function" ? nextConfig.webpack(config, ctx) : config;
    },
  };
}

/** Copy the runtime's files to `public/<path>/runtime-<hash>/` once (a new runtime gets a new
 * folder; old ones go). The folder ignores itself for git. */
function copyRuntime(from: string, to: string, publicDir: string) {
  mkdirSync(publicDir, { recursive: true });
  const ignore = join(publicDir, ".gitignore");
  if (!existsSync(ignore)) writeFileSync(ignore, "# Written by @datars/next (the datars runtime, chart data, published charts): not source.\n*\n");
  if (!existsSync(join(to, "wasm"))) {
    // Next can load its config in several processes at once: copy aside, then move in place.
    const tmp = `${to}.${process.pid}.tmp`;
    rmSync(tmp, { recursive: true, force: true });
    for (const rel of runtimeFiles(from)) {
      mkdirSync(join(tmp, rel, ".."), { recursive: true });
      cpSync(join(from, rel), join(tmp, rel));
    }
    try {
      renameSync(tmp, to);
    } catch {
      rmSync(tmp, { recursive: true, force: true }); // another process was first
    }
  }
  for (const name of readdirSync(publicDir)) {
    if (name.startsWith("runtime-") && resolve(publicDir, name) !== resolve(to) && !name.endsWith(".tmp")) rmSync(join(publicDir, name), { recursive: true, force: true });
  }
}

export default withDatars;
