// @datars/compile — chart files (`doc.ts`) to what a page needs, in Node, for bundler plugins
// (@datars/vite, @datars/next). The same approach as the CLI's compile_ts.mjs: bundle the file with
// esbuild against the project's own @datars/sdk and @datars/std, evaluate it, take the default
// export. On top of that, what a bundler needs to know:
//
// - the files the document was built from (to watch them: HMR),
// - the local files it reads at run time (`data.url("./x.csv")`, tile archives, font files), so
//   the plugin can serve and emit them and point the document at their public URLs,
// - and, opt-in, a published bundle: the `datars` CLI's `publish` (pre-expanded for the smaller
//   core engine, a poster, fonts subset and shipped — Google Fonts too, automatic basemaps).
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { cpus, tmpdir } from "node:os";
import { basename, dirname, extname, join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

/** A datars document (the JSON IR `doc()` returns). */
export type Doc = Record<string, unknown> & { size?: { width: number; height: number } | [number, number]; title?: string; data?: Record<string, Record<string, unknown>> };

/** A local file the document reads at run time, and where in the document its URL is. */
export interface ChartAsset {
  /** Path into the document to the URL string (`["data", "sales", "url"]`). */
  at: (string | number)[];
  /** The file on disk. */
  file: string;
  kind: "data" | "tiles" | "font";
}

export interface CompiledChart {
  file: string;
  doc: Doc;
  /** The document's authored size, [width, height] — the aspect a page reserves for it. */
  size: [number, number];
  title?: string;
  /** The program's steps, by name (a page can draw its stepper before the chart loads). */
  states: string[];
  /** Every source file the document was built from (its module graph, the project's own files and
   * packages): a change to any of them changes the document. */
  dependencies: string[];
  /** Local files the document reads at run time (by relative URL). */
  assets: ChartAsset[];
  /** Things that work differently in a raw document than in a published bundle. */
  warnings: string[];
  /** The file's own say on publish mode: `export const publish = true` (or false) next to the
   * default export. */
  publish?: boolean;
}

export interface PublishedChart {
  file: string;
  alias: string;
  /** The manifest, relative to the delivery dir: `c/<alias>`. */
  manifest: string;
  size: [number, number];
  title?: string;
  states: string[];
  /** Every file of this chart in the delivery dir (manifest, chunks, copied data), relative. */
  files: string[];
  /** Source files to watch (the document's module graph and the data files it reads). */
  dependencies: string[];
  /** The CLI's report (`--json`): sizes per tier, files copied. */
  report: Record<string, unknown>;
}

// ---------------------------------------------------------------------------------------------
// Compiling

let evalCount = 0;

/** `import()` of a URL known only at run time, out of sight of bundlers that trace the tools they
 * run (webpack's build-dependency tracking parses this file and warns about every `import(x)`). */
const importUrl = new Function("url", "return import(url)") as (url: string) => Promise<Record<string, unknown>>;

/** The project's esbuild if it has one (the same compiler its other tools use), else ours. */
async function esbuildFor(dir: string): Promise<typeof import("esbuild")> {
  try {
    return createRequire(join(dir, "__datars__.js"))("esbuild");
  } catch {
    return createRequire(import.meta.url)("esbuild");
  }
}

/** Compile a chart file (`.ts`, `.js`, `.mts`, `.mjs`; `.json` is read as is) to its document. */
export async function compileChart(file: string): Promise<CompiledChart> {
  file = resolve(file);
  const dir = dirname(file);
  let doc: Doc;
  let publish: boolean | undefined;
  let dependencies: string[] = [file];
  if (file.endsWith(".json")) {
    doc = JSON.parse(readFileSync(file, "utf8"));
  } else {
    const esbuild = await esbuildFor(dir);
    let result;
    try {
      result = await esbuild.build({
        entryPoints: [file],
        absWorkingDir: dir,
        bundle: true,
        write: false,
        metafile: true,
        format: "esm",
        platform: "node",
        target: "node18",
        sourcemap: "inline",
        logLevel: "silent",
        // CommonJS dependencies inside an ES module still find `require`.
        banner: { js: `import { createRequire as __datarsRequire } from "node:module"; const require = __datarsRequire(${JSON.stringify(pathToFileURL(file).href)});` },
      });
    } catch (e) {
      throw new Error(`${file}: ${formatEsbuildError(e)}`);
    }
    dependencies = Object.keys(result.metafile.inputs).map((p) => resolve(dir, p.replace(/^[a-z-]+:/, "")));
    const mod = await evaluate(result.outputFiles[0].text, file);
    doc = (await mod.default) as Doc;
    if (typeof mod.publish === "boolean") publish = mod.publish;
  }
  if (!doc || typeof doc !== "object" || !("scene" in doc)) {
    throw new Error(`${file}: the default export isn't a datars document (\`export default doc({ … })\` from @datars/sdk)`);
  }
  doc = await embedLocalRecipes(doc, dir);
  const warnings: string[] = [];
  const assets = findAssets(doc, dir, warnings);
  return { file, doc, size: sizeOf(doc), title: typeof doc.title === "string" ? doc.title : undefined, states: statesOf(doc), dependencies: [...new Set(dependencies)], assets, warnings, publish };
}

/** Run a bundled module in this process (a fresh module each time: edits must show). */
async function evaluate(code: string, file: string): Promise<{ default: unknown; publish?: unknown }> {
  const dir = join(tmpdir(), `datars-compile-${process.pid}`);
  mkdirSync(dir, { recursive: true });
  const out = join(dir, `${basename(file).replace(/\W+/g, "_")}-${++evalCount}.mjs`);
  writeFileSync(out, code);
  try {
    return (await importUrl(pathToFileURL(out).href)) as { default: unknown; publish?: unknown };
  } catch (e) {
    throw new Error(`${file}: ${(e as Error)?.stack ?? e}`);
  } finally {
    rmSync(out, { force: true });
  }
}

function formatEsbuildError(e: unknown): string {
  const errors = (e as { errors?: { text: string; location?: { file: string; line: number; column: number } }[] }).errors;
  if (!errors?.length) return String((e as Error)?.message ?? e);
  return errors.map((x) => (x.location ? `${x.location.file}:${x.location.line}:${x.location.column}: ${x.text}` : x.text)).join("\n");
}

/** The names of a document's program steps, in order. */
export function statesOf(doc: Record<string, unknown>): string[] {
  const states = (doc.program as { states?: { name?: unknown }[] } | undefined)?.states;
  return Array.isArray(states) ? states.map((s) => s?.name).filter((n): n is string => typeof n === "string") : [];
}

export function sizeOf(doc: { size?: unknown }): [number, number] {
  const s = doc.size as { width?: number; height?: number } | [number, number] | undefined;
  if (Array.isArray(s)) return [Number(s[0]) || 800, Number(s[1]) || 480];
  if (s && typeof s === "object") return [Number(s.width) || 800, Number(s.height) || 480];
  return [800, 480];
}

// Local recipes (`@local/<file>/…` → `recipes/<file>.ts` next to the document) and `{ name, file }`
// packages are embedded as source, as the CLI's compile_ts.mjs does.

function localModules(v: unknown, out = new Set<string>()): Set<string> {
  if (Array.isArray(v)) v.forEach((x) => localModules(x, out));
  else if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    if (o.kind === "use" && typeof o.recipe === "string" && o.recipe.startsWith("@local/")) out.add(o.recipe.split("/").slice(0, 2).join("/"));
    Object.values(o).forEach((x) => localModules(x, out));
  }
  return out;
}

async function embedLocalRecipes(input: Doc, dir: string): Promise<Doc> {
  let doc = input;
  type Pkg = { name: string; file?: string; source?: string };
  const pkgs = (doc.packages ?? []) as Pkg[];
  if (pkgs.some((p) => p.file)) {
    doc = { ...doc, packages: pkgs.map((p) => {
      if (!p.file) return p;
      const { file, ...rest } = p;
      const path = resolve(dir, file);
      if (!existsSync(path)) throw new Error(`package ${p.name}: expected ${path}`);
      return { ...rest, source: readFileSync(path, "utf8") };
    }) };
  }
  const mods = [...localModules(doc.scene)];
  if (!mods.length) return doc;
  const esbuild = await esbuildFor(dir);
  const packages = ((doc.packages ?? []) as Pkg[]).filter((p) => !mods.includes(p.name));
  for (const name of mods) {
    const file = join(dir, "recipes", `${name.slice("@local/".length)}.ts`);
    if (!existsSync(file)) throw new Error(`${name}: expected ${file}`);
    const r = await esbuild.build({ entryPoints: [file], absWorkingDir: dir, bundle: true, write: false, format: "esm", platform: "neutral", target: "es2020", logLevel: "silent", external: ["@datars/sdk", "@datars/std"] });
    packages.push({ name, source: r.outputFiles[0].text });
  }
  return { ...doc, packages };
}

// ---------------------------------------------------------------------------------------------
// Files a document reads

/** A relative file reference (not a URL with a scheme, not root-relative). */
export function isRelativeFile(u: unknown): u is string {
  return typeof u === "string" && u.length > 0 && !/^[a-z][a-z0-9+.-]*:/i.test(u) && !u.startsWith("/") && !u.startsWith("#");
}

function findAssets(doc: Doc, dir: string, warnings: string[]): ChartAsset[] {
  const out: ChartAsset[] = [];
  const add = (at: (string | number)[], ref: string, kind: ChartAsset["kind"]) => {
    const file = resolve(dir, ref);
    if (existsSync(file) && statSync(file).isFile()) out.push({ at, file, kind });
    else warnings.push(`${ref}: no such file next to the document (${file})`);
  };
  for (const [name, src] of Object.entries(doc.data ?? {})) {
    if (!src || typeof src !== "object") continue;
    if (isRelativeFile(src.url)) add(["data", name, "url"], src.url, "data");
    if (src.tiles === "auto") {
      // An automatic basemap is cut by the CLI; a raw document can use the archive it keeps next
      // to the document (`<source>.auto.pmtiles`) — current as long as the cameras didn't change.
      const cached = join(dir, `${name}.auto.pmtiles`);
      if (existsSync(cached)) out.push({ at: ["data", name, "tiles"], file: cached, kind: "tiles" });
      else warnings.push(`data.${name}: an automatic basemap (data.tiles.auto()) is built by the datars CLI — use publish mode for this chart, or run \`datars publish\` once to cut ${name}.auto.pmtiles`);
    } else if (isRelativeFile(src.tiles)) add(["data", name, "tiles"], src.tiles, "tiles");
    if (isRelativeFile(src.font)) add(["data", name, "font"], src.font, "font");
    if (typeof src.font === "string" && src.font.startsWith("google:")) warnings.push(`data.${name}: ${src.font} — Google Fonts ship in published bundles only (publish mode); a raw document draws with its fallbacks`);
  }
  // Theme font tokens with a file `src` (font.file), in the document's themes and token overrides.
  const theme = doc.theme as { themes?: { tokens?: Record<string, unknown>; modes?: Record<string, Record<string, unknown>> }[]; tokens?: Record<string, unknown> } | undefined;
  const tokenSets: [(string | number)[], Record<string, unknown> | undefined][] = [[["theme", "tokens"], theme?.tokens]];
  (theme?.themes ?? []).forEach((t, i) => {
    tokenSets.push([["theme", "themes", i, "tokens"], t.tokens]);
    for (const [m, toks] of Object.entries(t.modes ?? {})) tokenSets.push([["theme", "themes", i, "modes", m], toks]);
  });
  let google = false;
  for (const [at, tokens] of tokenSets) {
    for (const [k, v] of Object.entries(tokens ?? {})) {
      if (!v || typeof v !== "object") continue;
      const t = v as { src?: unknown; google?: unknown };
      if (isRelativeFile(t.src)) add([...at, k, "src"], t.src, "font");
      if (typeof t.google === "string") google = true;
    }
  }
  if (google) warnings.push("font.google(…): Google Fonts ship in published bundles only (publish mode); a raw document draws those texts with the fallback faces");
  return out;
}

/** Set a value at a path in a (JSON) object. */
export function setAt(obj: unknown, at: (string | number)[], value: unknown): void {
  let o = obj as Record<string | number, unknown>;
  for (const k of at.slice(0, -1)) o = o[k] as Record<string | number, unknown>;
  o[at[at.length - 1]] = value;
}

/** A short content hash (hex) of files or strings. */
export function hashOf(...parts: (string | Buffer)[]): string {
  const h = createHash("sha256");
  for (const p of parts) h.update(p).update("\0");
  return h.digest("hex").slice(0, 12);
}

/** A public file name for an asset: its name, and a content hash (cacheable forever). */
export function assetFileName(file: string): string {
  const ext = extname(file);
  const stem = basename(file, ext).replace(/[^\w.-]+/g, "_");
  return `${stem}-${hashOf(readFileSync(file))}${ext}`;
}

// ---------------------------------------------------------------------------------------------
// Publishing with the CLI

export interface PublishOptions {
  /** The alias the manifest is published under (`c/<alias>`). */
  alias: string;
  /** The delivery dir charts are merged into (chunks are content-addressed and shared). */
  outDir: string;
  /** The CLI: default `$DATARS`, else `datars` on PATH. */
  cli?: string;
  /** Extra environment for the CLI (e.g. `DATARS_OVERPASS_BUDGET`). */
  env?: Record<string, string | undefined>;
  /** Keep publish results here, keyed by everything that goes in, so a restart doesn't publish
   * every chart again (default: none). */
  cacheDir?: string;
}

/** How many CLI publishes run at once (each is a process; basemaps can be heavy). */
const publishSlots = Math.max(1, Math.min(4, Math.floor(cpus().length / 2)));
let running = 0;
const waiting: (() => void)[] = [];
async function slot<T>(f: () => Promise<T>): Promise<T> {
  if (running >= publishSlots) await new Promise<void>((r) => waiting.push(r));
  running++;
  try {
    return await f();
  } finally {
    running--;
    waiting.shift()?.();
  }
}

export function cliPath(cli?: string): string {
  return cli ?? process.env.DATARS ?? "datars";
}

/** Publish a chart file with the datars CLI into a delivery dir (`c/<alias>`, `chunks/`, data files
 * it copies) and report the files that belong to it. */
export async function publishChart(file: string, opts: PublishOptions, compiled?: CompiledChart): Promise<PublishedChart> {
  file = resolve(file);
  // Compiled here too: a readable error before the CLI runs, the aspect, and what to watch.
  compiled ??= await compileChart(file);
  const dependencies = [...compiled.dependencies, ...compiled.assets.map((a) => a.file).filter((f) => !/\.auto\.(pmtiles|job\.json)$/.test(f))];
  const cli = cliPath(opts.cli);
  const key = opts.cacheDir
    ? hashOf(opts.alias, cliStamp(cli), JSON.stringify(opts.env ?? {}), ...dependencies.filter((f) => existsSync(f)).map((f) => `${f}\0${hashOf(readFileSync(f))}`))
    : "";
  // The same chart asked for twice at once (a bundler compiles it for client and server): once.
  const memo = `${opts.outDir}\0${key || file}`;
  const pending = publishing.get(memo);
  if (pending) return pending;
  const job = publishOnce(file, opts, compiled, dependencies, cli, key).finally(() => publishing.delete(memo));
  publishing.set(memo, job);
  return job;
}

const publishing = new Map<string, Promise<PublishedChart>>();

async function publishOnce(file: string, opts: PublishOptions, compiled: CompiledChart, dependencies: string[], cli: string, key: string): Promise<PublishedChart> {
  const cached = key ? join(opts.cacheDir!, key) : "";
  let stage: string;
  let report: Record<string, unknown>;
  let scratch = "";
  if (cached && existsSync(join(cached, "report.json"))) {
    stage = join(cached, "site");
    report = JSON.parse(readFileSync(join(cached, "report.json"), "utf8"));
  } else {
    scratch = mkdtempSync(join(tmpdir(), "datars-publish-"));
    stage = join(scratch, "site");
    report = await slot(() => runCli(cli, ["publish", file, "--alias", opts.alias, "--to", stage, "--json"], dirname(file), opts.env));
    if (cached) {
      mkdirSync(dirname(cached), { recursive: true });
      writeFileSync(join(scratch, "report.json"), JSON.stringify(report));
      // Into the cache in one step; if another process got there first, its copy is the same.
      try {
        renameSync(scratch, cached);
        stage = join(cached, "site");
        scratch = "";
      } catch {
        /* keep ours for this merge */
      }
    }
  }
  const files = listFiles(stage);
  mergeInto(stage, opts.outDir, files, file, join(opts.cacheDir ?? tmpdir(), `owners-${hashOf(resolve(opts.outDir))}.json`));
  if (scratch) rmSync(scratch, { recursive: true, force: true });
  const manifest = `c/${opts.alias}`;
  const m = JSON.parse(readFileSync(join(opts.outDir, manifest), "utf8"));
  return { file, alias: opts.alias, manifest, size: m.size ? [Number(m.size[0]), Number(m.size[1])] : compiled.size, title: m.title ?? compiled.title, states: compiled.states, files, dependencies, report };
}

/** The CLI binary's identity for the cache (its path, size and modification time). */
function cliStamp(cli: string): string {
  const candidates = cli.includes(sep) ? [cli] : (process.env.PATH ?? "").split(":").map((d) => join(d, cli));
  for (const c of candidates) {
    try {
      const s = statSync(c);
      if (s.isFile()) return `${c}:${s.size}:${s.mtimeMs}`;
    } catch {
      /* next */
    }
  }
  return cli;
}

function runCli(cli: string, args: string[], cwd: string, env?: Record<string, string | undefined>): Promise<Record<string, unknown>> {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(cli, args, { cwd, env: { ...process.env, ...env }, stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    let err = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (err += d));
    child.on("error", (e: NodeJS.ErrnoException) => {
      if (e.code === "ENOENT") {
        reject(new Error(`publish mode needs the datars CLI, and \`${cli}\` wasn't found: install it (cargo install --path crates/datars-cli in the datars repo) or set $DATARS to its path — or turn publish off to play the raw document`));
      } else reject(e);
    });
    child.on("close", (code) => {
      if (code !== 0) return reject(new Error(`datars ${args.slice(0, 2).join(" ")} failed (${code}):\n${err.trim()}`));
      try {
        const lines = out.trim().split("\n");
        resolvePromise(JSON.parse(lines[lines.length - 1]));
      } catch {
        reject(new Error(`datars ${args[0]}: unexpected output:\n${out}\n${err}`));
      }
    });
  });
}

function listFiles(dir: string, prefix = ""): string[] {
  if (!existsSync(dir)) return [];
  const out: string[] = [];
  for (const name of readdirSync(dir).sort()) {
    const rel = prefix ? `${prefix}/${name}` : name;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) out.push(...listFiles(full, rel));
    else out.push(rel);
  }
  return out;
}

/** Copy a chart's published files into the shared delivery dir: chunks are content-addressed
 * (present = identical); a data file another chart copied under the same name must be the same
 * (the same chart may replace its own). Every file lands in one step (a server reading the folder
 * meanwhile sees the old file or the new one, never half of one). */
function mergeInto(stage: string, outDir: string, files: string[], source: string, ownersFile: string) {

  let owners: Record<string, string> = {};
  try {
    owners = JSON.parse(readFileSync(ownersFile, "utf8"));
  } catch {
    /* none yet */
  }
  let changed = false;
  for (const rel of files) {
    const from = join(stage, rel);
    const to = join(outDir, rel);
    if (existsSync(to)) {
      if (rel.startsWith("chunks/")) continue;
      if (readFileSync(to).equals(readFileSync(from))) continue;
      if (!rel.startsWith("c/") && owners[rel] && owners[rel] !== source) {
        throw new Error(`${source}: publishing copies ${rel}, and ${owners[rel]} already copied a different ${rel} — give one of the files another name`);
      }
    }
    mkdirSync(dirname(to), { recursive: true });
    const tmp = `${to}.${process.pid}.tmp`;
    copyFileSync(from, tmp);
    renameSync(tmp, to);
    if (!rel.startsWith("chunks/") && owners[rel] !== source) {
      owners[rel] = source;
      changed = true;
    }
  }
  if (changed) {
    const tmp = `${ownersFile}.${process.pid}.tmp`;
    writeFileSync(tmp, JSON.stringify(owners, null, 1));
    renameSync(tmp, ownersFile);
  }
}

// ---------------------------------------------------------------------------------------------
// The web runtime (@datars/web's dist: engines, default fonts, atlas)

export interface Runtime {
  /** `…/node_modules/@datars/web/dist`. */
  dir: string;
  version: string;
  /** A content hash of the engines: the public folder name changes when the runtime does. */
  hash: string;
}

const runtimeCache = new Map<string, Runtime>();

/** Find the project's @datars/web (from a directory inside the project). */
export function findRuntime(fromDir: string): Runtime {
  const project = createRequire(join(resolve(fromDir), "__datars__.js"));
  let pkg = "";
  try {
    pkg = project.resolve("@datars/web/package.json");
  } catch {
    // Not a direct dependency (pnpm keeps dependencies' dependencies private): the one
    // @datars/react brings, which is the one the pages import.
    try {
      pkg = createRequire(project.resolve("@datars/react/package.json")).resolve("@datars/web/package.json");
    } catch {
      throw new Error(`@datars/web isn't installed in ${fromDir} (npm install @datars/react, or @datars/web)`);
    }
  }
  const hit = runtimeCache.get(pkg);
  if (hit) return hit;
  const dir = join(dirname(pkg), "dist");
  if (!existsSync(join(dir, "wasm"))) throw new Error(`${dir}: @datars/web has no built runtime (dist/wasm)`);
  const version = JSON.parse(readFileSync(pkg, "utf8")).version as string;
  const stamp = runtimeFiles(dir).map((f) => {
    const s = statSync(join(dir, f));
    return `${f}:${s.size}`;
  });
  // The engines' content (glue and wasm) plus the sizes of everything else. The folder is served
  // as immutable under this hash: a rebuilt engine must never keep an old name (a returning
  // reader's cache would pair old wasm with new glue).
  const glue = runtimeFiles(dir).filter((f) => f.startsWith("wasm/")).map((f) => readFileSync(join(dir, f)));
  const rt = { dir, version, hash: hashOf(version, ...stamp, ...glue).slice(0, 10) };
  runtimeCache.set(pkg, rt);
  return rt;
}

/** The runtime's files a page may load, relative to its dist: the engine builds (core plays
 * published bundles, core-gl the same with WebGL2 for browsers without WebGPU, full plays raw
 * documents and T3 bundles), the WASI shim, default fonts and the built-in atlas. The bundled
 * script itself is left to the bundler. */
export function runtimeFiles(dir: string): string[] {
  const out: string[] = [];
  for (const f of ["wasm/datars_core.js", "wasm/datars_core_bg.wasm", "wasm/datars_core_gl.js", "wasm/datars_core_gl_bg.wasm", "wasm/datars_host_web.js", "wasm/datars_host_web_bg.wasm", "wasm/wasi_shim.js"]) if (existsSync(join(dir, f))) out.push(f);
  for (const sub of ["fonts", "atlas"]) for (const f of listFiles(join(dir, sub))) out.push(`${sub}/${f}`);
  return out;
}

const MIME: Record<string, string> = {
  ".wasm": "application/wasm", ".js": "text/javascript", ".mjs": "text/javascript", ".json": "application/json", ".geojson": "application/geo+json",
  ".csv": "text/csv", ".svg": "image/svg+xml", ".ttf": "font/ttf", ".otf": "font/otf", ".woff": "font/woff", ".woff2": "font/woff2", ".txt": "text/plain; charset=utf-8",
  ".pmtiles": "application/octet-stream", ".png": "image/png", ".datars": "application/octet-stream",
};

/** A content type for a runtime or delivery file (chunks and manifests have no extension). */
export function mimeOf(path: string): string {
  const ext = extname(path).toLowerCase();
  if (MIME[ext]) return MIME[ext];
  if (/(^|\/)c\/[^/]+$/.test(path)) return "application/json";
  return "application/octet-stream";
}

/** A chart's alias from its path in the project: readable and unique (`articles-a-stories-sd-doc`). */
export function aliasFor(file: string, root: string): string {
  const rel = relative(root, file).replace(/\\/g, "/").replace(/\.(chart\.)?(ts|tsx|mts|js|mjs|json)$/, "");
  const readable = rel.replace(/^(\.\.\/)+/, "").replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase();
  return readable.length > 0 && readable.length <= 80 ? readable : `chart-${hashOf(rel)}`;
}
