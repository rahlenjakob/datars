// The Node side the bundler plugins share: a chart file to its document, what it reads, what to
// watch; published bundles merged into one delivery folder; the runtime's files.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { aliasFor, compileChart, findRuntime, isRelativeFile, mimeOf, publishChart, runtimeFiles, setAt } from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const fixture = join(here, "fixture/sales.chart.ts");
const cli = process.env.DATARS ?? join(here, "../../../target/release/datars");

test("a chart file compiles to its document, with its size, steps and publish flag", async () => {
  const c = await compileChart(fixture);
  assert.equal(c.doc.datars, 1);
  assert.equal(c.title, "Sales by region");
  assert.deepEqual(c.size, [640, 400]);
  assert.deepEqual(c.states, ["first", "second"]);
  assert.equal(c.publish, false);
});

test("the files a document was built from are its dependencies (a local module too)", async () => {
  const c = await compileChart(fixture);
  assert.ok(c.dependencies.includes(fixture));
  assert.ok(c.dependencies.includes(join(here, "fixture/lib.ts")));
});

test("local files the document reads are reported with where their URL is", async () => {
  const c = await compileChart(fixture);
  assert.deepEqual(c.assets.map((a) => [a.at, a.file, a.kind]), [[["data", "sales", "url"], join(here, "fixture/sales.csv"), "data"]]);
  // A Google font can't be fetched by a raw document: said at build time.
  assert.ok(c.warnings.some((w) => w.includes("Google Fonts")));
});

test("edits show: each compile evaluates the file afresh", async () => {
  const dir = mkdtempSync(join(tmpdir(), "datars-compile-test-"));
  // A copy next to the fixture's node_modules resolution: inside the package.
  const file = join(here, "fixture", `edited-${process.pid}.chart.ts`);
  const src = readFileSync(fixture, "utf8");
  try {
    writeFileSync(file, src.replace("size: [640, 400]", "size: [700, 350]"));
    assert.deepEqual((await compileChart(file)).size, [700, 350]);
    writeFileSync(file, src.replace("size: [640, 400]", "size: [500, 500]"));
    assert.deepEqual((await compileChart(file)).size, [500, 500]);
  } finally {
    (await import("node:fs")).rmSync(file, { force: true });
    (await import("node:fs")).rmSync(dir, { recursive: true, force: true });
  }
});

test("a file that isn't a document says so", async () => {
  const file = join(here, "fixture", `bad-${process.pid}.chart.ts`);
  writeFileSync(file, "export default { hello: 1 };\n");
  try {
    await assert.rejects(compileChart(file), /isn't a datars document/);
  } finally {
    (await import("node:fs")).rmSync(file, { force: true });
  }
});

test("helpers: relative files, paths into documents, aliases, content types", () => {
  assert.ok(isRelativeFile("../data.csv") && isRelativeFile("x.geojson"));
  assert.ok(!isRelativeFile("https://x/y.csv") && !isRelativeFile("/abs.csv") && !isRelativeFile("datars:fonts/a.ttf") && !isRelativeFile("google:Inter"));
  const o = { data: { a: { url: "x" } } };
  setAt(o, ["data", "a", "url"], "/assets/x-1.csv");
  assert.equal(o.data.a.url, "/assets/x-1.csv");
  assert.equal(aliasFor("/p/articles/a/stories/sd/doc.ts", "/p"), "articles-a-stories-sd-doc");
  assert.equal(aliasFor("/p/src/sales.chart.ts", "/p"), "src-sales");
  assert.equal(mimeOf("wasm/datars_core_bg.wasm"), "application/wasm");
  assert.equal(mimeOf("c/sales"), "application/json");
});

test("the runtime: both engines, the WASI shim, fonts and atlas, under a content hash", () => {
  const rt = findRuntime(here);
  const files = runtimeFiles(rt.dir);
  for (const f of ["wasm/datars_core.js", "wasm/datars_host_web.js", "wasm/wasi_shim.js"]) assert.ok(files.includes(f), f);
  assert.ok(!files.some((f) => f.includes("datars_lite")), "the lite build isn't loaded by the element");
  assert.match(rt.hash, /^[0-9a-f]{10}$/);
});

test("publishing merges charts into one delivery folder and caches the CLI's work", { skip: !existsSync(cli) && "needs the datars CLI (target/release/datars)" }, async () => {
  const out = mkdtempSync(join(tmpdir(), "datars-site-"));
  const cacheDir = mkdtempSync(join(tmpdir(), "datars-cache-"));
  const p = await publishChart(fixture, { alias: "sales", outDir: out, cli, cacheDir });
  assert.equal(p.manifest, "c/sales");
  assert.deepEqual(p.size, [640, 400]);
  assert.deepEqual(p.states, ["first", "second"]);
  assert.ok(p.files.some((f) => f.startsWith("chunks/")));
  for (const f of p.files) assert.ok(existsSync(join(out, f)), f);
  // Again, into a fresh folder: from the cache (one entry, no second CLI run), the same files.
  const out2 = mkdtempSync(join(tmpdir(), "datars-site-"));
  const again = await publishChart(fixture, { alias: "sales", outDir: out2, cli, cacheDir });
  assert.deepEqual(again.files, p.files);
  assert.equal(readdirSync(cacheDir).filter((n) => !n.startsWith("owners-")).length, 1);
  for (const f of again.files) assert.ok(existsSync(join(out2, f)), f);
});

test("publish mode without the CLI explains what to do", async () => {
  const out = mkdtempSync(join(tmpdir(), "datars-site-"));
  mkdirSync(out, { recursive: true });
  await assert.rejects(publishChart(fixture, { alias: "sales", outDir: out, cli: "/nonexistent/datars" }), /needs the datars CLI/);
});
