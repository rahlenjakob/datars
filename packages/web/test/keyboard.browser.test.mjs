// The keyboard in a real browser: on the chart itself the arrows and Space step the story; on a
// control in its accessible list they're the control's — a slider's arrows move its thumb, Space
// presses a button — and the story stays where it is.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright (any
// install): DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/keyboard.browser.test.mjs
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

/** A slider (a scrub over a track), a button, and a story of two steps. */
const doc = {
  datars: 1, size: { width: 400, height: 200 },
  signals: { n: { type: "num", default: 5 }, pressed: { type: "bool", default: false } },
  scene: { kind: "group", key: "root", children: [
    { kind: "group", key: "slider", scales: { s: { type: "linear", domain: [0, 10], range: [0, 300] } },
      on: { drag: { scrub: "n", axis: "x", scale: "s", step: 1 } }, semantics: { role: "control", label: "Amount" },
      children: [{ kind: "shape", key: "track", geom: { type: "rect", x: 0, y: 20, w: 300, h: 20 }, fill: "$grid", pickable: true }] },
    { kind: "shape", key: "press", geom: { type: "rect", x: 0, y: 80, w: 100, h: 40 }, fill: "$accent", pickable: true,
      semantics: { role: "control", label: "Press" }, on: { activate: { set: "pressed", value: true } } }] },
  program: { preset: "story", states: [{ name: "one" }, { name: "two" }] },
};

const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<button id="before">Before</button><datars-view doc="/doc.json" height="200" style="width:400px"></datars-view>`;

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

test("the arrows step the story on the chart, and move a slider on the slider", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage();
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    await p.waitForFunction(() => document.querySelector("datars-view")?.signals, null, { timeout: 30_000 });
    const chart = () => p.evaluate(() => { const v = document.querySelector("datars-view"); return { n: v.signals.n, pressed: v.signals.pressed, index: v.status.index }; });
    // Tab from the page to the chart: the arrows step the story.
    await p.focus("#before");
    await p.keyboard.press("Tab");
    assert.equal(await p.evaluate(() => document.activeElement?.tagName), "DATARS-VIEW");
    await p.keyboard.press("ArrowRight");
    assert.equal((await chart()).index, 1);
    await p.keyboard.press("ArrowLeft");
    assert.equal((await chart()).index, 0);
    // Tab on (past the story's own Previous and Next): the slider. Its arrows move it, the story
    // stays.
    const focused = () => p.evaluate(() => document.querySelector("datars-view").shadowRoot.activeElement?.getAttribute("aria-label"));
    for (let i = 0; i < 4 && (await focused()) !== "Amount"; i++) await p.keyboard.press("Tab");
    assert.equal(await focused(), "Amount");
    await p.keyboard.press("ArrowRight");
    await p.keyboard.press("ArrowRight");
    let s = await chart();
    assert.equal(s.n, 7);
    assert.equal(s.index, 0, "the story didn't move");
    // Tab again: the button. Space presses it.
    await p.keyboard.press("Tab");
    await p.keyboard.press(" ");
    s = await chart();
    assert.equal(s.pressed, true);
    assert.equal(s.index, 0, "Space pressed the button, not the story");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
