// Tiles over HTTP (docs/09-geo.md, docs/12-delivery.md): the wasm runtime streams a basemap from a
// static server by `Range` requests — what <datars-view> does — and every state renders the
// goldens' pixels, the same as the native headless path that reads the ranges from disk. Only the
// header, directories and the tiles the views need cross the wire. Needs the wasm runtime and the CLI.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, statSync } from "node:fs";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm")) && existsSync(cli) && existsSync(join(root, "tests/golden/descent/golden.json"));

test("a basemap streams by HTTP range requests and renders the goldens", { skip: !ready && "build the wasm runtime, the CLI and the goldens first" }, async (t) => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn(cli, ["serve", root, "--port", String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((res) => server.stdout.once("data", res));
  const docUrl = `http://127.0.0.1:${port}/examples/descent/doc.json`;
  try {
    const doc = await (await fetch(docUrl)).text();
    const golden = JSON.parse(readFileSync(join(root, "tests/golden/descent/golden.json"), "utf8"));
    const size = JSON.parse(doc).size;
    const view = new m.View(true, []);
    view.load_doc(doc);
    view.set_size(size.width, size.height, 1);
    let fetched = 0;
    let partial = 0;
    // What <datars-view>'s fetchData does: fetch every range request with a Range header.
    const pump = async () => {
      // Label fonts: the default family next to the runtime (`datars:fonts/…`).
      for (const r of JSON.parse(view.data_requests())) if (r.url?.startsWith("datars:")) view.provide_source(r.name, readFileSync(join(root, r.url.slice("datars:".length))));
      const reqs = JSON.parse(view.data_requests()).filter((r) => r.range);
      await Promise.all(reqs.map(async (r) => {
        const [offset, length] = r.range;
        const res = await fetch(new URL(r.url, docUrl), { headers: { Range: `bytes=${offset}-${offset + length - 1}` } });
        const bytes = new Uint8Array(await res.arrayBuffer());
        if (res.status === 206) partial++;
        fetched += bytes.length;
        view.provide_range(r.name, offset, bytes);
      }));
      return reqs.length;
    };
    for (const [i, st] of golden.states.entries()) {
      view.goto(i);
      let now = i * 100000 + 50000;
      // Frames until the ranges are in and decoded (a frame decodes a budget of tiles).
      for (let k = 0; k < 120; k++) {
        now += 1000 / 60;
        const moving = view.frame(now);
        if ((await pump()) === 0 && !moving) break;
      }
      assert.equal(view.pixel_hash(now), st.pixels, `descent / ${st.name}`);
    }
    const archive = statSync(join(root, "assets/tiles/descent.pmtiles")).size;
    t.diagnostic(`${partial} range requests, ${fetched} of ${archive} archive bytes fetched`);
    assert.ok(partial > 3, `served as partial content (${partial})`);
    assert.ok(fetched < archive / 2, `fetched ${fetched} of ${archive} bytes: only what the views need`);
    view.free();
  } finally {
    server.kill();
  }
});
