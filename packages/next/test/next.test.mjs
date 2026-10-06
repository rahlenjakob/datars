// withDatars: chart files through a loader for Turbopack and webpack, the runtime in public/, the
// runtime told where (process.env.DATARS_RUNTIME); the loader's output, a plain object.
import { test, after } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { withDatars } from "../dist/index.js";
import { load } from "../dist/loader.js";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "fixture");
after(() => rmSync(join(root, "public"), { recursive: true, force: true }));

test("the config: env, Turbopack rules, a webpack rule, the user's own webpack kept", () => {
  let userWebpack = 0;
  const config = withDatars({ basePath: "/docs", env: { A: "1" }, webpack: (c) => (userWebpack++, c) }, { root });
  assert.match(config.env.DATARS_RUNTIME, /^\/docs\/_datars\/runtime-[0-9a-f]{10}\/$/);
  assert.equal(config.env.A, "1");
  const rule = config.turbopack.rules["*.chart.ts"];
  assert.equal(rule.as, "*.js");
  assert.ok(rule.loaders[0].loader.endsWith("loader.cjs"));
  // Turbopack passes loader options across processes: plain JSON only.
  assert.deepEqual(JSON.parse(JSON.stringify(rule.loaders[0].options)), rule.loaders[0].options);
  const wp = config.webpack({ module: { rules: [] } }, {});
  assert.equal(userWebpack, 1);
  assert.ok(wp.module.rules.some((r) => r.enforce === "pre" && r.test.test("src/sales.chart.ts")));
  assert.ok(wp.module.rules.some((r) => r.resourceQuery?.test("?datars")));
});

test("the runtime is copied into public/_datars (which ignores itself for git)", () => {
  const config = withDatars({}, { root });
  const dir = join(root, "public", config.env.DATARS_RUNTIME);
  for (const f of ["wasm/datars_core_bg.wasm", "wasm/datars_host_web_bg.wasm", "wasm/wasi_shim.js", "fonts/Inter-Regular.ttf", "atlas/countries.geojson"]) assert.ok(existsSync(join(dir, f)), f);
  assert.match(readFileSync(join(root, "public/_datars/.gitignore"), "utf8"), /^\*$/m);
  // Loaded again (Next loads its config in several processes): one runtime folder.
  withDatars({}, { root });
  assert.equal(readdirSync(join(root, "public/_datars")).filter((n) => n.startsWith("runtime-")).length, 1);
});

test("a function config is wrapped", async () => {
  const fn = withDatars(async (phase) => ({ reactStrictMode: phase === "dev" }), { root });
  const config = await fn("dev", { defaultConfig: {} });
  assert.equal(config.reactStrictMode, true);
  assert.ok(config.env.DATARS_RUNTIME);
});

test("the loader: a serializable chart, its data public under a content hash, its files watched", async () => {
  const deps = [];
  const code = await load(
    { resourcePath: join(root, "sales.chart.ts"), resourceQuery: "", addDependency: (f) => deps.push(f), cacheable() {}, emitWarning() {} },
    { root, publicDir: join(root, "public/_datars"), publicPath: "/docs/_datars", publish: false },
  );
  const json = /JSON\.parse\((".*")\)/.exec(code)[1];
  const chart = JSON.parse(JSON.parse(json));
  assert.equal(chart.kind, "datars-chart");
  assert.equal(chart.id, "sales.chart.ts");
  assert.deepEqual(chart.size, [640, 400]);
  assert.deepEqual(chart.states, ["first", "second"]);
  const url = chart.doc.data.sales.url;
  assert.match(url, /^\/docs\/_datars\/assets\/sales-[0-9a-f]{12}\.csv$/);
  assert.ok(existsSync(join(root, "public", url.slice("/docs".length))));
  assert.ok(deps.includes(join(root, "lib.ts")) && deps.includes(join(root, "sales.csv")));
});
