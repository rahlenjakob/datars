// A scroll story (`steps=".step"`, steps by index as the article pages have them): a step that
// reaches the middle of the screen after the chart has been still for a while — the reader was
// reading — plays its transition. The runtime renders on demand, so the engine's clock is the last
// frame's; a step that didn't bring it to now started its transition seconds in the past, and the
// chart jumped to the end. And a fast scroll past several steps lands on the last, settled.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright, which this
// repo doesn't depend on — point at any install:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/steps.browser.test.mjs
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

// Four states (bars, ranked, pie, donut); a sticky chart and a step per state, no `data-state`.
const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 0 } .stage { position: sticky; top: 0; height: 100vh } .steps { margin-top: -100vh } .step { height: 120vh }</style>
<div class="stage"><datars-view doc="/doc.json" steps=".step" height="420" style="display:block;width:640px"></datars-view></div>
<div class="steps"><div class="step"></div><div class="step"></div><div class="step"></div><div class="step"></div></div>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

test("a scroll step after a pause plays its transition; a fast scroll lands settled on the last", { skip, timeout: 120_000 }, async () => {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page, "text/html; charset=utf-8");
    if (path === "/doc.json") return send(readFileSync(join(root, "examples/votes/doc.json")), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { chromium } = await import(playwright);
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    const still = () => p.waitForFunction(() => { const v = document.querySelector("datars-view"); return v.dataset.renderer && v.view && !v.view.transitioning(); }, null, { timeout: 30_000 });
    await still();
    // The reader reads: the chart draws nothing for longer than any of its transitions.
    await p.waitForTimeout(2500);
    // The second step reaches the middle; how many frames the chart then spends in transition.
    const moving = await p.evaluate(() => new Promise((resolve) => {
      const v = document.querySelector("datars-view");
      scrollTo({ top: innerHeight * 1.3, behavior: "instant" }); // the middle of the screen in step 2
      let frames = 0;
      const t0 = performance.now();
      const poll = () => {
        if (v.view.transitioning()) frames++;
        if (performance.now() - t0 < 500) requestAnimationFrame(poll);
        else resolve({ frames, index: v.view.index() });
      };
      requestAnimationFrame(poll);
    }));
    assert.equal(moving.index, 1, "the second step is the chart's state");
    assert.ok(moving.frames >= 5, `the step's transition plays (in transition for ${moving.frames} frames, not jumped to its end)`);
    await still();
    // A flick through the third step to the fourth, a frame apart.
    await p.evaluate(async () => {
      for (const k of [2, 2.5, 3]) {
        scrollTo({ top: innerHeight * (1.2 * k + 0.1), behavior: "instant" });
        await new Promise((r) => requestAnimationFrame(r));
      }
    });
    await still();
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").view.index()), 3, "the fast scroll lands on the last step");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
