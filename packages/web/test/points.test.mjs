// Big data over HTTP (docs/12-delivery.md, "Big data stays interactive"): the galaxy example's four
// million generated stars publish as a point archive, and the core runtime — no sandbox, nothing
// generated on the device — opens the bundle, streams the tiles its views need by `Range` requests
// and renders every state with the goldens' pixels (the ones `datars test` made from the source
// document natively). The bundle is small; only a fraction of the archive crosses the wire.
// Needs the wasm runtime, the CLI and the goldens.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, mkdtempSync, rmSync, statSync, readdirSync } from "node:fs";
import { execFileSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const ready = existsSync(join(wasmDir, "datars_core_bg.wasm")) && existsSync(cli) && existsSync(join(root, "tests/golden/galaxy/golden.json"));

test("four million stars play from a point archive streamed by range, as the goldens", { skip: !ready && "build the wasm runtime, the CLI and the goldens first" }, async (t) => {
  const core = await import(join(wasmDir, "datars_core.js"));
  core.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
  const site = mkdtempSync(join(tmpdir(), "datars-points-"));
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn(cli, ["serve", site, "--port", String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((res) => server.stdout.once("data", res));
  const base = `http://127.0.0.1:${port}`;
  try {
    execFileSync(cli, ["publish", join(root, "examples/galaxy/doc.json"), "--alias", "galaxy", "--to", site]);
    const archive = readdirSync(join(site, "tiles")).find((f) => f.startsWith("stars."));
    assert.ok(archive, "the stars ship as an archive beside the chart");
    const archiveBytes = statSync(join(site, "tiles", archive)).size;
    // What <datars-view> does: the manifest, then the chunks it asks for.
    const view = new core.View(false, []);
    let opened = 0;
    let need = view.open_manifest(new Uint8Array(await (await fetch(`${base}/c/galaxy`)).arrayBuffer()));
    while (need.length) {
      const got = await Promise.all(need.map(async (h) => [h, new Uint8Array(await (await fetch(`${base}/chunks/${h.replace(":", "_")}`)).arrayBuffer())]));
      for (const [h, b] of got) {
        opened += b.length;
        need = view.provide_chunk(h, b);
      }
    }
    assert.equal(view.status().tier, "T2");
    const golden = JSON.parse(readFileSync(join(root, "tests/golden/galaxy/golden.json"), "utf8"));
    const size = JSON.parse(readFileSync(join(root, "examples/galaxy/doc.json"), "utf8")).size;
    view.set_size(size.width, size.height, 1);
    let fetched = 0;
    const pump = async () => {
      const reqs = JSON.parse(view.data_requests()).filter((r) => r.range);
      await Promise.all(reqs.map(async (r) => {
        const [offset, length] = r.range;
        const res = await fetch(new URL(r.url, `${base}/c/galaxy`), { headers: { Range: `bytes=${offset}-${offset + length - 1}` } });
        assert.equal(res.status, 206);
        const bytes = new Uint8Array(await res.arrayBuffer());
        fetched += bytes.length;
        view.provide_range(r.name, offset, bytes);
      }));
      return reqs.length;
    };
    let first = 0;
    for (const [i, st] of golden.states.entries()) {
      view.goto(i);
      let now = i * 100000 + 50000;
      // Frame while ranges are in flight or the engine is still preparing tiles (a budget of rows
      // per frame) and new detail is fading in, as the page does — its clock moving on.
      for (let k = 0; k < 400; k++) {
        now += 1000 / 60;
        const moving = view.frame(now);
        if ((await pump()) === 0 && !moving) break;
      }
      if (i === 0) first = fetched;
      assert.equal(view.pixel_hash(now), st.pixels, `galaxy / ${st.name}`);
      assert.equal(view.stats().rows, 4_000_000);
    }
    t.diagnostic(`bundle ${opened} bytes; archive ${archiveBytes} bytes, ${first} fetched for the first view, ${fetched} for all ${golden.states.length}`);
    assert.ok(opened < 400_000, `the bundle is small (${opened} bytes)`);
    assert.ok(first < archiveBytes / 20, `the first view reads a sliver of the archive (${first} of ${archiveBytes})`);
    view.free();
  } finally {
    server.kill();
    rmSync(site, { recursive: true, force: true });
  }
});
