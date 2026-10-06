// P1 across targets: every example state rendered by the wasm runtime has the same pixels as the
// reference goldens (`datars test`, any machine). Needs `scripts/build-wasm.sh` first. The web
// runtime has no font compiled in: raw documents ask for their faces (`font:` requests, the
// default family as `datars:fonts/…`), answered here from disk as <datars-view> does over HTTP.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm"));

test("wasm pixels equal the goldens for every example state", { skip: !ready && "run scripts/build-wasm.sh first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  let checked = 0;
  for (const name of readdirSync(join(root, "tests/golden"))) {
    const docPath = join(root, "examples", name, "doc.json");
    if (!existsSync(docPath)) continue;
    const doc = JSON.parse(readFileSync(docPath, "utf8"));
    if (Object.values(doc.data ?? {}).some((s) => s.atlas || (s.url && !s.url.includes("://")))) continue; // needs files: native tests cover it
    const golden = JSON.parse(readFileSync(join(root, "tests/golden", name, "golden.json"), "utf8"));
    const view = new m.View(true, []);
    view.load_doc(JSON.stringify(doc));
    view.set_size(doc.size.width, doc.size.height, 1);
    // Files next to the example (fonts, tile archives by range): answer the engine's requests from
    // disk until a frame needs nothing more.
    const serve = () => {
      let n = 0;
      for (const r of JSON.parse(view.data_requests())) {
        if (!r.url) continue;
        const file = readFileSync(r.url.startsWith("datars:") ? join(root, r.url.slice("datars:".length)) : join(root, "examples", name, r.url));
        if (r.range) {
          const [offset, length] = r.range;
          view.provide_range(r.name, offset, file.subarray(offset, offset + length));
        } else {
          view.provide_source(r.name, file);
        }
        n++;
      }
      return n;
    };
    golden.states.forEach((st, i) => {
      view.goto(i);
      let t = i * 100000 + 50000;
      // As a host does: frame again, its clock moving on, while data is being fetched or the
      // engine is still moving (point tiles are prepared a budget of rows per frame, and new
      // detail fades in).
      for (let k = 0; k < 400; k++) {
        t += 1000 / 60;
        const moving = view.frame(t);
        if (serve() === 0 && !moving) break;
      }
      assert.equal(view.pixel_hash(t), st.pixels, `${name} / ${st.name}`);
      checked++;
    });
    view.free();
  }
  assert.ok(checked >= 15, `only ${checked} states checked`);
});
