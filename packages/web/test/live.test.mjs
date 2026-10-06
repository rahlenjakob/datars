// Live data (docs/06, docs/08): a published chart whose source is live keeps refetching it on the
// schedule the document declares; each new snapshot animates in. The feed is a replay directory
// served by `datars serve`. The host loop mirrors <datars-view>: frames, then data_requests().
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, mkdtempSync, cpSync } from "node:fs";
import { execFileSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const ready = existsSync(join(wasmDir, "datars_core_bg.wasm")) && existsSync(cli);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

test("a live source refreshes on schedule and its updates render", { skip: !ready && "build the wasm runtime and the CLI first" }, async () => {
  const m = await import(join(wasmDir, "datars_core.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
  const site = mkdtempSync(join(tmpdir(), "datars-live-"));
  execFileSync(cli, ["publish", join(root, "examples/election/doc.json"), "--alias", "election", "--to", site]);
  cpSync(join(root, "examples/election/count.json.d"), join(site, "c/count.json.d"), { recursive: true });
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn(cli, ["serve", site, "--port", String(port)], { stdio: ["ignore", "pipe", "inherit"], env: { ...process.env, DATARS_REPLAY_PERIOD: "0.4" } });
  await new Promise((res) => server.stdout.once("data", res));
  const base = `http://127.0.0.1:${port}/c/election`;
  try {
    const view = new m.View(true, []);
    let need = view.open_manifest(new Uint8Array(await (await fetch(base)).arrayBuffer()));
    while (need.length) {
      for (const h of need) need = view.provide_chunk(h, new Uint8Array(await (await fetch(`http://127.0.0.1:${port}/chunks/${h.replace(":", "_")}`)).arrayBuffer()));
    }
    view.set_size(720, 440, 1);
    const seen = new Set();
    let fetched = 0;
    const t0 = Date.now();
    // Run a compressed evening: the document refreshes every 2 s of frame time; we advance the frame
    // clock 4× faster than the wall clock and let the server's feed advance every 0.4 s.
    while (Date.now() - t0 < 3000) {
      const now = (Date.now() - t0) * 4;
      view.frame(now);
      for (const r of JSON.parse(view.data_requests())) {
        const b = new Uint8Array(await (await fetch(new URL(r.url, base), { cache: "no-cache" })).arrayBuffer());
        view.provide_source(r.name, b);
        fetched++;
      }
      seen.add(view.pixel_hash(now + 5000));
      await sleep(100);
    }
    assert.ok(fetched >= 3, `refetched ${fetched} times`);
    assert.ok(seen.size >= 3, `${seen.size} distinct settled frames — the count moved`);
    view.free();
  } finally {
    server.kill();
  }
});
