// The element's playback API in a real browser: `seek(position)` shows the exact frame part-way
// through a transition and holds it, and the `reduced-motion` attribute overrides the reader's
// `prefers-reduced-motion` both ways — a scrubber and a "show me the motion" button need no
// reaching into the engine.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/playback.browser.test.mjs
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

/** A box that moves from x = 100 to x = 500 between two states, over two seconds, linearly. */
const doc = {
  datars: 1, size: { width: 640, height: 200 },
  scene: { kind: "group", key: "root", children: [
    { kind: "shape", key: "box", geom: { type: "rect", x: "=state == \"b\" ? 500 : 100", y: 80, w: 40, h: 40 }, fill: "$accent", semantics: { role: "datum", label: "Box" } }] },
  program: { states: [{ name: "a" }, { name: "b" }] },
  motion: { rules: [{ duration: 2, easing: "linear" }] },
};

const page = (attrs) => `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 0 }</style><datars-view doc="/doc.json" height="200" style="width:640px" ${attrs}></datars-view>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".geojson": "application/json" };

function serve() {
  const server = createServer((req, res) => {
    const url = new URL(req.url, "http://x");
    const path = decodeURIComponent(url.pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page(url.searchParams.get("attrs") ?? ""), "text/html; charset=utf-8");
    if (path === "/doc.json") return send(JSON.stringify(doc), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

test("seek shows a transition part-way and holds it; reduced-motion overrides the reader's setting", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const open = async (attrs = "", reducedMotion = "no-preference") => {
      const ctx = await browser.newContext({ viewport: { width: 700, height: 300 }, reducedMotion });
      const p = await ctx.newPage();
      const errors = [];
      p.on("pageerror", (e) => errors.push(e.message));
      await p.goto(`${origin}/?attrs=${encodeURIComponent(attrs)}`);
      await p.waitForFunction(() => document.querySelector("datars-view")?.dataset.renderer, null, { timeout: 30_000 });
      return { p, errors };
    };
    /** The box's left edge, from the element's hit test across its row. */
    const boxX = (p) => p.evaluate(() => {
      const el = document.querySelector("datars-view");
      for (let x = 0; x < 640; x += 2) if (el.hitTest(x, 100).some((h) => h.path.includes("box"))) return x;
      return null;
    });
    const near = (x, want, what) => assert.ok(x !== null && Math.abs(x - want) <= 4, `${what}: box at ${x}, want ${want}`);

    // seek: a fraction is the frame that far through, exactly, and it holds.
    {
      const { p, errors } = await open();
      near(await boxX(p), 100, "at rest");
      const states = [];
      await p.evaluate(() => { window.__states = []; document.querySelector("datars-view").addEventListener("state", (e) => window.__states.push(e.detail.index)); });
      await p.evaluate(() => document.querySelector("datars-view").seek(0.25));
      await p.waitForTimeout(300);
      near(await boxX(p), 200, "seek(0.25)");
      await p.waitForTimeout(600);
      near(await boxX(p), 200, "seek(0.25), held");
      await p.evaluate(() => document.querySelector("datars-view").seek(0.75));
      await p.waitForTimeout(200);
      near(await boxX(p), 400, "seek(0.75)");
      // Out of range clamps; the last state is a state of its own (the program moved: an event).
      await p.evaluate(() => document.querySelector("datars-view").seek(7));
      await p.waitForTimeout(200);
      near(await boxX(p), 500, "seek(7) → the last state");
      states.push(...(await p.evaluate(() => window.__states)));
      assert.ok(states.includes(1), `a state event for the last state (${states})`);
      assert.deepEqual(errors, []);
      await p.context().close();
    }

    // seek before the chart has opened: applied when it does.
    {
      const ctx = await browser.newContext({ viewport: { width: 700, height: 300 } });
      const p = await ctx.newPage();
      await p.addInitScript(() => {
        customElements.whenDefined("datars-view").then(() => document.querySelector("datars-view")?.seek(0.5));
      });
      await p.goto(origin);
      await p.waitForFunction(() => document.querySelector("datars-view")?.dataset.renderer, null, { timeout: 30_000 });
      await p.waitForTimeout(300);
      near(await boxX(p), 300, "seek(0.5) before opening");
      await ctx.close();
    }

    /** Where the box is 0.6 s into a step from a to b: in flight (≈ 220) with the full motion; at
     * one end or the other with reduced motion (a crossfade moves nothing). */
    const midStep = async (p) => {
      await p.evaluate(() => document.querySelector("datars-view").send("next"));
      await p.waitForTimeout(600);
      return boxX(p);
    };
    const moving = (x) => x !== null && x > 140 && x < 460;

    // The reader asks for reduced motion: by default the chart follows…
    {
      const { p } = await open("", "reduce");
      assert.equal(await p.evaluate(() => document.querySelector("datars-view").reducedMotion), true);
      assert.ok(!moving(await midStep(p)), "the reader's reduced motion is followed");
      await p.context().close();
    }
    // …unless the page says the reader opted in to the motion here.
    {
      const { p } = await open('reduced-motion="no-preference"', "reduce");
      assert.equal(await p.evaluate(() => document.querySelector("datars-view").reducedMotion), false);
      assert.ok(moving(await midStep(p)), "reduced-motion=no-preference plays the full motion");
      await p.context().close();
    }
    // Forced the other way, and switched live.
    {
      const { p } = await open('reduced-motion="reduce"');
      assert.ok(!moving(await midStep(p)), "reduced-motion=reduce, for a reader without the setting");
      await p.evaluate(() => { const el = document.querySelector("datars-view"); el.removeAttribute("reduced-motion"); el.send("prev"); });
      await p.waitForTimeout(2300);
      assert.ok(moving(await midStep(p)), "the attribute removed: the full motion again");
      await p.context().close();
    }
  } finally {
    await browser.close();
    server.close();
  }
});
