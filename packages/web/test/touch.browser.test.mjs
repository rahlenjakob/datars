// Touch in a real browser: a finger that lands on what the chart drags (a brush, a pan) keeps the
// gesture — the page doesn't scroll with it and cancel the drag a few pixels in — a finger
// anywhere else scrolls the page, and two fingers on an explorable view pinch-zoom it.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright (any
// install): DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/touch.browser.test.mjs
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

/** A brushable strip (0–100 across 360 px), a mark that's only clicked, and an explorable view. */
const doc = {
  datars: 1, size: { width: 360, height: 600 },
  signals: { sel: { type: "range" } },
  scene: { kind: "group", key: "root", layout: { type: "rows" }, children: [
    { kind: "group", key: "strip", size: { h: 150 }, scales: { x: { type: "linear", domain: [0, 100], range: [0, 360] } }, on: { brush: { brush: "sel" } },
      children: [{ kind: "shape", key: "bg", geom: { type: "rect", x: 0, y: 0, w: 360, h: 150 }, fill: "$grid" }] },
    { kind: "group", key: "mid", size: { h: 150 }, children: [
      { kind: "shape", key: "mark", geom: { type: "rect", x: 0, y: 0, w: 360, h: 150 }, fill: "$accent", pickable: true, semantics: { role: "datum", label: "A mark" } }] },
    { kind: "view", key: "map", size: { h: 300 }, camera: { fit: { bbox: [0, 0, 360, 300] }, padding: 0, explore: "cam" }, children: [
      { kind: "shape", key: "land", geom: { type: "rect", x: 0, y: 0, w: 360, h: 300 }, fill: "$surface" }] }] },
};

const page = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 0 15px }</style><div style="height: 120px"></div><datars-view doc="/doc.json" height="600" style="width:360px"></datars-view><div style="height: 1600px"></div>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".geojson": "application/json" };

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page, "text/html; charset=utf-8");
    if (path === "/doc.json") return send(JSON.stringify(doc), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

test("a touch on what the chart drags is the chart's; elsewhere it scrolls; two fingers pinch", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const ctx = await browser.newContext({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
    const p = await ctx.newPage();
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    await p.waitForFunction(() => document.querySelector("datars-view")?.signals, null, { timeout: 30_000 });
    await p.evaluate(() => {
      window.__cancels = 0;
      document.querySelector("datars-view").addEventListener("pointercancel", () => window.__cancels++, true);
    });
    const cdp = await ctx.newCDPSession(p);
    const box = await p.locator("datars-view").boundingBox();
    const touch = async (type, points) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points.map(([x, y], id) => ({ x: box.x + x, y: box.y + y, id })) });
    const drag = async (from, to, steps = 12) => {
      await touch("touchStart", [from]);
      for (let i = 1; i <= steps; i++) {
        await touch("touchMove", [[from[0] + ((to[0] - from[0]) * i) / steps, from[1] + ((to[1] - from[1]) * i) / steps]]);
        await p.waitForTimeout(16);
      }
      await touch("touchEnd", []);
      await p.waitForTimeout(150);
    };
    const signals = () => p.evaluate(() => document.querySelector("datars-view").signals);

    // A finger across the strip brushes all the way: 10 → 90.
    await drag([36, 75], [324, 75]);
    let s = await signals();
    assert.equal(s["sel.active"], true);
    assert.ok(Math.abs(s["sel.lo"] - 10) < 1 && Math.abs(s["sel.hi"] - 90) < 1, JSON.stringify(s));
    assert.equal(await p.evaluate(() => window.__cancels), 0, "the browser didn't take the gesture");

    // A finger up the explorable view pans it; the page stays where it was.
    const y0 = await p.evaluate(() => scrollY);
    await drag([180, 520], [180, 400]);
    s = await signals();
    assert.ok(Number(s["cam.y"]) > 100, JSON.stringify(s));
    assert.equal(await p.evaluate(() => scrollY), y0, "the page didn't scroll");

    // A finger up the mark that's only clicked scrolls the page.
    await drag([180, 260], [180, 140]);
    await p.waitForFunction((y) => scrollY > y, y0, { timeout: 5_000 });
    await p.evaluate(() => scrollTo(0, 0));
    await p.waitForTimeout(150);

    // Two fingers spreading over the view zoom it in.
    const b2 = await p.locator("datars-view").boundingBox();
    Object.assign(box, b2);
    await touch("touchStart", [[150, 450]]);
    await touch("touchStart", [[150, 450], [210, 450]]);
    for (let i = 1; i <= 10; i++) {
      await touch("touchMove", [[150 - 6 * i, 450], [210 + 6 * i, 450]]);
      await p.waitForTimeout(16);
    }
    await touch("touchEnd", []);
    await p.waitForTimeout(150);
    s = await signals();
    assert.ok(Number(s["cam.zoom"]) > 1.8, JSON.stringify(s));
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
