// The plugin in a real Vite: chart files become chart modules (the document, its data as assets,
// HMR), the runtime is served in dev and emitted into the build, the runtime module is pointed at
// it, and publish mode goes through the CLI.
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build, createServer } from "vite";
import datars from "../dist/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "fixture");
const cli = process.env.DATARS ?? join(here, "../../../target/release/datars");
const haveCli = existsSync(cli);

function files(dir, prefix = "") {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? files(join(dir, e.name), `${prefix}${e.name}/`) : [`${prefix}${e.name}`]));
}

test("dev: chart modules, the runtime served (with ranges), the runtime pointed at it", async () => {
  const server = await createServer({ root, logLevel: "silent", configFile: false, server: { port: 0 }, plugins: [datars({ include: /sales\.chart\.ts$/ })], optimizeDeps: { noDiscovery: true, include: [] } });
  await server.listen();
  try {
    const base = `http://localhost:${server.config.server.port ?? server.httpServer.address().port}`;
    const port = server.httpServer.address().port;
    const url = `http://localhost:${port}`;
    const mod = await server.transformRequest("/sales.chart.ts");
    assert.match(mod.code, /kind: "datars-chart"/);
    assert.match(mod.code, /states: \[\s*"first",\s*"second"\s*\]/);
    assert.match(mod.code, /sales\.csv\?(import&)?url/, "the CSV is an asset import");
    assert.match(mod.code, /import\.meta\.hot/, "the module accepts its own updates");
    // The runtime module learns where its files are.
    const web = await server.transformRequest("/@id/@datars/web").catch(() => null) ?? await server.transformRequest("@datars/web");
    assert.match(web.code, /setRuntimeBase\("\/datars\/runtime-[0-9a-f]{10}\/"\)/);
    const runtimeDir = /runtime-[0-9a-f]{10}/.exec(web.code)[0];
    const wasm = await fetch(`${url}/datars/${runtimeDir}/wasm/datars_core_bg.wasm`);
    assert.equal(wasm.status, 200);
    assert.equal(wasm.headers.get("content-type"), "application/wasm");
    const part = await fetch(`${url}/datars/${runtimeDir}/atlas/countries.geojson`, { headers: { Range: "bytes=0-9" } });
    assert.equal(part.status, 206);
    assert.equal((await part.arrayBuffer()).byteLength, 10);
    // Only the runtime's own files are served from its folder.
    assert.notEqual((await fetch(`${url}/datars/${runtimeDir}/index.js`)).headers.get("content-type"), "text/javascript");
    void base;
  } finally {
    await server.close();
  }
});

test("build: the runtime and published charts emitted under datars/, the runtime pointed at them", { skip: !haveCli && "publish mode needs the datars CLI" }, async () => {
  const outDir = mkdtempSync(join(tmpdir(), "datars-vite-build-"));
  await build({ root, logLevel: "silent", configFile: false, plugins: [datars({ include: /sales\.chart\.ts$/, cli })], build: { outDir, emptyOutDir: true } });
  const out = files(outDir);
  const runtime = out.filter((f) => /^datars\/runtime-[0-9a-f]{10}\//.test(f));
  for (const f of ["wasm/datars_core.js", "wasm/datars_core_bg.wasm", "wasm/datars_host_web.js", "wasm/datars_host_web_bg.wasm", "wasm/wasi_shim.js", "fonts/Inter-Regular.ttf", "atlas/countries.geojson"]) {
    assert.ok(runtime.some((r) => r.endsWith(`/${f}`)), f);
  }
  assert.ok(out.includes("datars/c/sales"), "the published manifest");
  assert.ok(out.some((f) => f.startsWith("datars/chunks/")), "its chunks");
  const js = out.filter((f) => f.endsWith(".js") && f.startsWith("assets/")).map((f) => readFileSync(join(outDir, f), "utf8")).join("\n");
  // The runtime module ends by pointing itself at its files (the call is minified: any name).
  assert.match(js, /\([`"']\/datars\/runtime-[0-9a-f]{10}\/[`"']\);?$/m);
  assert.match(js, /[`"']\/datars\/c\/sales[`"']/);
  assert.match(js, /Sales by region/, "the raw chart's document is in the bundle");
});

test("build with a relative base: URLs from the chunk's own location", { skip: !haveCli && "publish mode needs the datars CLI" }, async () => {
  const outDir = mkdtempSync(join(tmpdir(), "datars-vite-build-"));
  await build({ root, base: "./", logLevel: "silent", configFile: false, plugins: [datars({ include: /sales\.chart\.ts$/, cli })], build: { outDir, emptyOutDir: true } });
  const js = files(outDir).filter((f) => f.endsWith(".js") && f.startsWith("assets/")).map((f) => readFileSync(join(outDir, f), "utf8")).join("\n");
  assert.match(js, /new URL\(`\.\.\/datars\/c\/sales`,import\.meta\.url\)/);
  assert.match(js, /\(new URL\(`\.\.\/datars\/runtime-[0-9a-f]{10}\/wasm\/wasi_shim\.js`,import\.meta\.url\)\.href\.slice\(0,-17\)\)/);
});
