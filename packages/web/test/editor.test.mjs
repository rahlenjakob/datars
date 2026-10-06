// Hooks for editors on the live engine: what's under the pointer (any element, not only
// interactive ones), why it looks as it does, and atlases for raw documents.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm"));

test("hit_test finds titles, axes and marks; explain says where they came from", { skip: !ready && "build the wasm runtime first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const doc = readFileSync(join(root, "examples/votes/doc.json"), "utf8");
  const view = new m.View(true, []);
  view.load_doc(doc);
  // The default fonts ship next to the runtime (`datars:fonts/…`): answered from disk as the
  // element does over HTTP.
  for (const r of JSON.parse(view.data_requests())) {
    if (r.url?.startsWith("datars:fonts/")) view.provide_source(r.name, readFileSync(join(root, "packages/web/dist/fonts", r.url.slice("datars:fonts/".length))));
  }
  view.set_size(720, 440, 1);
  view.pixels(0);
  // The title (not interactive) is found by its text.
  const title = view.hit_test(60, 36).find((h) => h.kind === "text");
  assert.ok(title, JSON.stringify(view.hit_test(60, 36)));
  assert.match(title.text, /Vote share/);
  // The tallest bar (Social Democrats) sits left in the plot.
  const bar = view.hit_test(80, 300).find((h) => h.role === "datum");
  assert.ok(bar && bar.path.includes('("S",)'), JSON.stringify(view.hit_test(80, 300)));
  assert.equal(bar.bounds.length, 4);
  const why = view.explain(bar.path);
  assert.ok(why.length > 0 && JSON.stringify(why).includes("@datars/std/bar"), JSON.stringify(why).slice(0, 400));
});

test("a raw document asks the page for its atlas", { skip: !ready && "build the wasm runtime first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const view = new m.View(true, []);
  view.load_doc(JSON.stringify({ datars: 1, data: { world: { atlas: "countries" } }, scene: { kind: "group", key: "root", children: [] } }));
  const reqs = JSON.parse(view.data_requests());
  assert.deepEqual(reqs.find((r) => r.atlas), { name: "world", atlas: "countries" });
  view.provide_source("world", readFileSync(join(root, "packages/web/dist/atlas/countries.geojson")));
  assert.ok(!JSON.parse(view.data_requests()).some((r) => r.atlas), "fulfilled");
});

test("a map's credit is a real link: links() says where it is", { skip: !ready && "build the wasm runtime first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const view = new m.View(true, []);
  view.load_doc(JSON.stringify({ datars: 1, size: { width: 400, height: 200 }, scene: { kind: "use", recipe: "@datars/std/attribution", params: {} } }));
  for (const r of JSON.parse(view.data_requests())) {
    if (r.url?.startsWith("datars:fonts/")) view.provide_source(r.name, readFileSync(join(root, "packages/web/dist/fonts", r.url.slice("datars:fonts/".length))));
  }
  view.set_size(400, 200, 1);
  view.pixels(0);
  const links = view.links();
  assert.equal(links.length, 1, JSON.stringify(links));
  assert.equal(links[0].href, "https://www.openstreetmap.org/copyright");
  assert.match(links[0].label, /OpenStreetMap contributors/);
  const [x, y, w, h] = links[0].bounds;
  assert.ok(x > 150 && x + w <= 400 && y + h <= 200 && w > 100 && h > 8, JSON.stringify(links[0].bounds));
});
