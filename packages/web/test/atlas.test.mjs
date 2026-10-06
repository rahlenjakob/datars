// A published world choropleth in the wasm runtime: the publish compiler ships the countries atlas
// as a decimal topology (shared borders, integer deltas — 40 % smaller); the chart the core engine
// draws from the static files must still be the golden one, pixel for pixel.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, mkdtempSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const ready = existsSync(join(wasmDir, "datars_core_bg.wasm")) && existsSync(cli);

test("a published atlas chart draws the golden pixels", { skip: !ready && "build the wasm runtime and the CLI first" }, async () => {
  const m = await import(join(wasmDir, "datars_core.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
  const site = mkdtempSync(join(tmpdir(), "datars-atlas-"));
  execFileSync(cli, ["publish", join(root, "examples/renewables/doc.json"), "--alias", "renewables", "--to", site]);
  const view = new m.View(false, []);
  let need = view.open_manifest(readFileSync(join(site, "c/renewables")));
  let atlas = null;
  while (need.length) {
    for (const h of need) {
      const bytes = readFileSync(join(site, "chunks", h.replace(":", "_")));
      if (bytes.includes(Buffer.from('"type":"Topology"'))) atlas = bytes;
      need = view.provide_chunk(h, bytes);
    }
  }
  assert.ok(view.ready(), "the core engine plays it (T2)");
  assert.ok(atlas, "the atlas arrived as a topology");
  const doc = JSON.parse(readFileSync(join(root, "examples/renewables/doc.json"), "utf8"));
  view.set_size(doc.size.width, doc.size.height, 1);
  const golden = JSON.parse(readFileSync(join(root, "tests/golden/renewables/golden.json"), "utf8"));
  golden.states.forEach((st, i) => {
    view.goto(i);
    assert.equal(view.pixel_hash(i * 100000 + 50000), st.pixels, st.name);
  });
  view.free();
});

test("a published chart's fonts ship with it and draw the golden pixels", { skip: !ready && "build the wasm runtime and the CLI first" }, async () => {
  // The web runtime has no font compiled in: the default family (votes), a document's own font
  // (hebrew) and a theme's project fonts (serif) all arrive as subset chunks of the bundle.
  const m = await import(join(wasmDir, "datars_core.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
  for (const name of ["hebrew", "serif", "votes"]) {
    const site = mkdtempSync(join(tmpdir(), "datars-font-"));
    execFileSync(cli, ["publish", join(root, `examples/${name}/doc.json`), "--alias", name, "--to", site]);
    const manifest = JSON.parse(readFileSync(join(site, `c/${name}`), "utf8"));
    assert.ok(manifest.fonts.length > 0 && manifest.fonts.every((f) => f.licence === "OFL-1.1"), `${name}: font credits`);
    const view = new m.View(false, []);
    let need = view.open_manifest(readFileSync(join(site, `c/${name}`)));
    while (need.length) for (const h of need) need = view.provide_chunk(h, readFileSync(join(site, "chunks", h.replace(":", "_"))));
    assert.ok(view.ready());
    const doc = JSON.parse(readFileSync(join(root, `examples/${name}/doc.json`), "utf8"));
    view.set_size(doc.size.width, doc.size.height, 1);
    const golden = JSON.parse(readFileSync(join(root, `tests/golden/${name}/golden.json`), "utf8"));
    golden.states.forEach((st, i) => {
      view.goto(i);
      assert.equal(view.pixel_hash(i * 100000 + 50000), st.pixels, `${name} / ${st.name}`);
    });
    assert.deepEqual(JSON.parse(view.data_requests()), [], `${name}: no font left to fetch`);
    assert.deepEqual(view.chunk_requests(), [], `${name}: no lazily loaded subset needed`);
    view.free();
  }
});
