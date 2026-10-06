// A page answering the chart's data requests itself (`datarequest`): its own API client, auth,
// mocks, a replayed feed. A live source asks on every refresh; the page's answers animate in; the
// server is never asked.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright, which this
// repo doesn't depend on — point at any install:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/datarequest.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const dist = join(root, "packages/web/dist");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(dist, "wasm/datars_host_web_bg.wasm")) || !existsSync(join(dist, "datars.js")) ? "build the runtime first" : false;

/** One bar per row of a live source, refreshed every half second. */
const doc = {
  datars: 1, size: { width: 400, height: 120 },
  data: { count: { url: "count.json", key: ["k"], live: { every: 0.5 } } },
  scene: { kind: "repeat", key: "bars", from: "count", template: { kind: "shape", geom: { type: "rect", x: 0, y: 20, w: "=d.v", h: 30 }, fill: "$accent",
    semantics: { role: "datum", label: "=`${d.k}: ${d.v}`" } } },
};

const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<datars-view doc="/doc.json" style="width:400px"></datars-view>
<script>
  window.asked = 0;
  document.querySelector("datars-view").addEventListener("datarequest", (e) => {
    if (e.detail.name !== "count") return;
    e.preventDefault();
    window.asked++;
    // Some answers take a moment, like a real client.
    e.detail.respond(new Promise((ok) => setTimeout(() => ok([{ k: "a", v: 40 * window.asked }]), 20)));
  });
</script>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".geojson": "application/json" };

test("the page answers a live source's requests, and each answer animates in", { skip, timeout: 120_000 }, async () => {
  const hits = [];
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    hits.push(path);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page, "text/html; charset=utf-8");
    if (path === "/doc.json") return send(JSON.stringify(doc), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { chromium } = await import(playwright);
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage();
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    // What the bar says where the pointer would be: its label, if the bar reaches that far.
    const at = (x) => p.evaluate((x) => document.querySelector("datars-view").hitTest(x, 35).find((h) => h.role === "datum")?.label ?? null, x);
    await p.waitForFunction(() => window.asked >= 4, null, { timeout: 30_000 });
    // The refreshes kept coming, each answered by the page, and later answers animated in: the
    // bar (40 px after the first answer) now reaches past 110 px.
    await p.waitForFunction(() => document.querySelector("datars-view").hitTest(110, 35).some((h) => h.role === "datum"), null, { timeout: 10_000 });
    const n = await p.evaluate(() => window.asked);
    const shown = await at(110);
    const v = Number(shown?.match(/a: (\d+)/)?.[1]);
    assert.ok(v >= 120 && v <= 40 * n, `a later answer is on the chart: "${shown}" after ${n} requests`);
    assert.ok(!hits.includes("/count.json"), `the server was never asked: ${hits.join(", ")}`);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
