// Signed charts in a real browser: with `publishers`, a chart signed by a named key plays; one that
// isn't signed, or is signed by another key, is refused before any of it shows — no poster from the
// bundle — with an `error` event (`detail.refused`) and a note in the chart's box.
//
// Needs `scripts/build-wasm.sh`, `pnpm -C packages/web build`, the CLI (`cargo build --release -p
// datars-cli`) and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/signed.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const dist = join(root, "packages/web/dist");
const cli = join(root, "target/release/datars");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(dist, "datars.js")) ? "build the runtime first" : !existsSync(cli) ? "build the CLI first" : false;
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".geojson": "application/json", ".svg": "image/svg+xml" };

test("a page that names a publisher plays its signed charts and refuses the rest", { skip, timeout: 180_000 }, async () => {
  const dir = mkdtempSync(join(tmpdir(), "datars-signed-"));
  const run = (...a) => execFileSync(cli, a, { encoding: "utf8" });
  const key = (name) => JSON.parse(run("keygen", "--out", join(dir, name), "--json")).publisher;
  const mine = key("mine.key"), theirs = key("theirs.key");
  const doc = join(root, "examples/votes/doc.json");
  const site = join(dir, "site");
  run("publish", doc, "--alias", "signed", "--to", site, "--sign", join(dir, "mine.key"));
  run("publish", doc, "--alias", "unsigned", "--to", site);
  run("publish", doc, "--alias", "foreign", "--to", site, "--sign", join(dir, "theirs.key"));
  const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
${["signed", "unsigned", "foreign"].map((a) => `<datars-view id="${a}" src="/c/${a}" publishers="${mine}" height="300" style="width:500px;display:block"></datars-view>`).join("\n")}
<script>window.errors = []; for (const v of document.querySelectorAll("datars-view")) v.addEventListener("error", (e) => errors.push([v.id, e.detail.refused, e.detail.message]));</script>`;
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page, "text/html; charset=utf-8");
    const inDist = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && inDist.startsWith(dist) && existsSync(inDist)) return send(readFileSync(inDist), types[extname(inDist)] ?? "application/octet-stream");
    const inSite = normalize(join(site, path));
    if (inSite.startsWith(site) && existsSync(inSite)) return send(readFileSync(inSite), path.startsWith("/c/") ? "application/json" : "application/octet-stream");
    res.writeHead(404).end();
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const { chromium } = await import(playwright);
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage();
    const unhandled = [];
    p.on("pageerror", (e) => unhandled.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => document.getElementById("signed").dataset.renderer && window.errors.length >= 2, null, { timeout: 60_000 });
    await p.waitForTimeout(1200); // a late poster would have shown by now
    const state = await p.evaluate(() => Object.fromEntries([...document.querySelectorAll("datars-view")].map((v) => [v.id, {
      renderer: v.dataset.renderer ?? null, failed: v.dataset.failed ?? null,
      poster: v.shadowRoot.querySelector(".poster")?.innerHTML.length ?? 0,
      note: v.shadowRoot.querySelector(".fail")?.textContent ?? null,
    }])));
    assert.ok(state.signed.renderer, "the signed chart plays");
    assert.equal(state.signed.failed, null);
    for (const id of ["unsigned", "foreign"]) {
      assert.equal(state[id].renderer, null, `${id}: never drawn`);
      assert.equal(state[id].failed, "refused", id);
      assert.equal(state[id].poster, 0, `${id}: nothing from the bundle shows, not even its poster`);
      assert.match(state[id].note ?? "", /couldn't be verified as signed by a publisher this page trusts/);
    }
    const errors = await p.evaluate(() => window.errors);
    assert.deepEqual(errors.map(([id, refused]) => [id, refused]).sort(), [["foreign", true], ["unsigned", true]]);
    assert.ok(errors.find(([id]) => id === "foreign")[2].includes("not allowed"), errors);
    assert.deepEqual(unhandled, [], "no unhandled rejection");
  } finally {
    await browser.close();
    server.close();
    rmSync(dir, { recursive: true, force: true });
  }
});
