// A chart taken off the page gives back its WebGL context at once. Browsers allow only a handful
// of live contexts and a context otherwise outlives its canvas until garbage collection: a page
// that adds and removes charts (an editor, a gallery, a lazily mounted article) would run out —
// Chrome then kills the oldest context, perhaps a chart still on screen; Safari stops creating them.
//
// The WebGL engine (core-gl, named by the `engine` attribute: a raw document would load the full
// engine, which has none), in a browser made to look as if it had no WebGPU. Needs
// `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/webgl.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const dist = join(root, "packages/web/dist");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(dist, "wasm/datars_core_gl_bg.wasm")) || !existsSync(join(dist, "datars.js")) ? "build the runtime first" : false;

const doc = { datars: 1, size: { width: 300, height: 150 }, scene: { kind: "shape", key: "box", geom: { type: "rect", x: 10, y: 10, w: 100, h: 50 }, fill: "$accent" } };
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(`<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script><div id="host"></div>`, "text/html; charset=utf-8");
    if (path === "/doc.json") return send(JSON.stringify(doc), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

test("charts added and removed again and again don't run out of WebGL contexts", { skip, timeout: 180_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage();
    // A browser without WebGPU: the element loads the WebGL engine.
    await p.addInitScript(() => Object.defineProperty(Navigator.prototype, "gpu", { get: () => undefined }));
    const warnings = [];
    p.on("console", (m) => { if (/WebGL/i.test(m.text())) warnings.push(m.text()); });
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    await p.waitForFunction(() => customElements.get("datars-view"));
    const drawn = await p.evaluate(async () => {
      const out = [];
      for (let i = 0; i < 24; i++) {
        const v = document.createElement("datars-view");
        v.setAttribute("doc", "/doc.json");
        v.setAttribute("height", "150");
        v.setAttribute("engine", "/runtime/wasm/datars_core_gl.js");
        v.style.width = "300px";
        document.getElementById("host").appendChild(v);
        const t0 = performance.now();
        while (!v.dataset.renderer && performance.now() - t0 < 15000) await new Promise((r) => setTimeout(r, 30));
        const canvas = v.shadowRoot.querySelector("canvas");
        out.push(`${v.dataset.renderer}/${canvas.getContext("webgl2") ? "webgl2" : "other"}`);
        v.remove();
      }
      return out;
    });
    assert.deepEqual([...new Set(drawn)], ["gpu/webgl2"], `every chart drew with WebGL: ${drawn}`);
    assert.deepEqual(warnings, [], "no context was lost to the browser's limit");
  } finally {
    await browser.close();
    server.close();
  }
});
