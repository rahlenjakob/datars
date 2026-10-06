// The edit loop in the browser runtime: a new version of a document morphs in from what is on
// screen (`<datars-view>.reload()`, driven by `datars dev`).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm"));

test("reload_doc morphs from the old version to the new one", { skip: !ready && "run scripts/build-wasm.sh first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const doc = JSON.parse(readFileSync(join(root, "examples/votes/doc.json"), "utf8"));
  const view = new m.View(true, []);
  view.load_doc(JSON.stringify(doc));
  view.set_size(doc.size.width, doc.size.height, 1);
  // The default fonts, as <datars-view> fetches them next to the runtime.
  const fonts = () => {
    for (const r of JSON.parse(view.data_requests())) if (r.url?.startsWith("datars:")) view.provide_source(r.name, readFileSync(join(root, r.url.slice("datars:".length))));
  };
  fonts();
  assert.deepEqual(JSON.parse(view.data_requests()), [], "the fonts arrived");
  view.goto(1);
  const before = view.pixel_hash(100000);
  const edited = structuredClone(doc);
  edited.data.votes.values.share[0] = 12; // the largest bar shrinks
  view.reload_doc(JSON.stringify(edited));
  assert.deepEqual(JSON.parse(view.data_requests()), [], "a reload keeps the fonts it fetched (same URLs)");
  assert.equal(view.status().state, "ranked", "the state is kept");
  view.pixel_hash(100000 + 10); // the morph's first frame (it starts there)
  const mid = view.pixel_hash(100000 + 150);
  const end = view.pixel_hash(100000 + 60000);
  assert.notEqual(mid, before, "moving");
  assert.notEqual(mid, end, "…between the versions");
  view.free();
});
