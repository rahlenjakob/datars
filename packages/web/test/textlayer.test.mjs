// The text layer through the wasm runtime: `View.text_layer()` describes where the last frame drew
// each text (what <datars-view> lays its selectable spans over), and `View.face_bytes()` hands the
// page the font it was drawn in. Needs `scripts/build-wasm.sh` first.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm"));

test("the text layer describes the drawn texts, and the page can have their font", { skip: !ready && "run scripts/build-wasm.sh first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const doc = JSON.parse(readFileSync(join(root, "examples/business/doc.json"), "utf8"));
  const view = new m.View(true, []);
  view.load_doc(JSON.stringify(doc));
  view.set_size(doc.size.width, doc.size.height, 1);
  assert.equal(view.text_layer(), "[]", "nothing drawn yet");
  // Answer the font requests from disk, as the page does over HTTP.
  for (let k = 0; k < 20; k++) {
    const moving = view.frame(1000 + k);
    let served = 0;
    for (const r of JSON.parse(view.data_requests())) {
      if (!r.url?.startsWith("datars:")) continue;
      view.provide_source(r.name, readFileSync(join(root, r.url.slice("datars:".length))));
      served++;
    }
    if (!served && !moving) break;
  }
  const json = view.text_layer();
  const texts = JSON.parse(json);
  const title = texts.find((t) => t.role === "title");
  assert.ok(title, json.slice(0, 400));
  assert.equal(title.text, "Revenue bridge, 2023 → 2024");
  assert.equal(title.lines.length, 1);
  const [line] = title.lines;
  // The chart's `title` sits at the chart group's origin (12, 16), baseline top.
  assert.ok(Math.abs(line.x - 12) < 1e-6 && Math.abs(line.y - 16) < 1e-6 && line.w > 150 && line.h > 17, JSON.stringify(line));
  assert.ok(texts.some((t) => t.text === "Price") && texts.every((t) => t.lines.length > 0 && !t.drag && !t.place));
  assert.equal(view.text_layer(), json, "the same frame, the same layer: the page rebuilds only on change");
  // The font the title was drawn in, for the page to lay the spans in.
  const bytes = view.face_bytes(title.faces[0]);
  assert.ok(bytes instanceof Uint8Array && bytes.length > 10_000, `face ${title.faces[0]}: ${bytes?.length}`);
  assert.equal(view.face_bytes("No-Such-Face"), undefined);
  view.free();
});
