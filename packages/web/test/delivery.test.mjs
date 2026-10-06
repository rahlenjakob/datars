// The delivery contract (docs/12-delivery.md; roadmap Phase 2 exit): a chart published after the
// host was built plays in it without a rebuild, over plain HTTP from a static layout, and a
// republish updates it. Every variant the runtime may choose (T3 with the sandbox, the pre-compiled
// ones without) renders the goldens' pixels. Needs the wasm runtime and the CLI built.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync, mkdtempSync, writeFileSync } from "node:fs";
import { execFileSync, spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const wasmDir = join(root, "packages/web/dist/wasm");
const cli = join(root, "target/release/datars");
const ready = existsSync(join(wasmDir, "datars_host_web_bg.wasm")) && existsSync(cli);

// What <datars-view> does: manifest → requested chunks → provide until nothing is missing.
async function open(m, base, alias, allowScript) {
  const view = new m.View(allowScript, []);
  let need = view.open_manifest(new Uint8Array(await (await fetch(`${base}/c/${alias}`)).arrayBuffer()));
  let rounds = 0;
  while (need.length && rounds++ < 8) {
    const got = await Promise.all(need.map(async (h) => [h, new Uint8Array(await (await fetch(`${base}/chunks/${h.replace(":", "_")}`)).arrayBuffer())]));
    for (const [h, b] of got) need = view.provide_chunk(h, b);
  }
  assert.ok(view.ready(), "the bundle opened");
  return view;
}

test("published charts play in an already-built host, and republishing updates them", { skip: !ready && "build the wasm runtime and the CLI first" }, async () => {
  const m = await import(join(wasmDir, "datars_host_web.js"));
  m.initSync({ module: readFileSync(join(wasmDir, "datars_host_web_bg.wasm")) });
  const site = mkdtempSync(join(tmpdir(), "datars-site-"));
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn(cli, ["serve", site, "--port", String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((res) => server.stdout.once("data", res));
  const base = `http://127.0.0.1:${port}`;
  try {
    // v1: published after this "host" (the code above) exists.
    const doc = JSON.parse(readFileSync(join(root, "examples/votes/doc.json"), "utf8"));
    execFileSync(cli, ["publish", join(root, "examples/votes/doc.json"), "--alias", "votes", "--to", site]);
    const golden = JSON.parse(readFileSync(join(root, "tests/golden/votes/golden.json"), "utf8"));
    for (const allowScript of [true, false]) {
      const view = await open(m, base, "votes", allowScript);
      view.set_size(doc.size.width, doc.size.height, 1);
      golden.states.forEach((st, i) => {
        view.goto(i);
        assert.equal(view.pixel_hash(i * 100000 + 50000), st.pixels, `votes/${st.name} (allowScript=${allowScript}, tier ${JSON.stringify(view.status().tier)})`);
      });
      view.free();
    }
    // The smaller core engine (no sandbox) plays the same bundle through its pre-expanded variant.
    const core = await import(join(wasmDir, "datars_core.js"));
    core.initSync({ module: readFileSync(join(wasmDir, "datars_core_bg.wasm")) });
    const cv = await open(core, base, "votes", true);
    assert.equal(cv.status().tier, "T2", "core picks the pre-expanded variant");
    cv.set_size(doc.size.width, doc.size.height, 1);
    golden.states.forEach((st, i) => {
      cv.goto(i);
      assert.equal(cv.pixel_hash(i * 100000 + 50000), st.pixels, `core engine: votes/${st.name}`);
    });
    cv.free();
    // v2: a republish (a new title and the dark mode's paper as background) under the same alias.
    const v2 = structuredClone(doc);
    v2.scene.children[0].params.title = "Vote share by party, 2022 — final count";
    const v2Path = join(site, "votes-v2.json");
    writeFileSync(v2Path, JSON.stringify(v2));
    execFileSync(cli, ["publish", v2Path, "--alias", "votes", "--to", site, "--revision", "2"]);
    const want = execFileSync(cli, ["render", v2Path, "--dpr", "1", "--hash", "--out", join(site, "v2.png")], { encoding: "utf8" }).split("\n")[0].trim();
    const view = await open(m, base, "votes", true);
    view.set_size(doc.size.width, doc.size.height, 1);
    const got = view.pixel_hash(0);
    assert.notEqual(got, golden.states[0].pixels, "the republished chart is different");
    assert.equal(got, want, "and the host shows exactly the new version");
    view.free();
  } finally {
    server.kill();
  }
});
