// The reader's text size in a real browser: <datars-view> follows the browser's default font size
// (Settings › Appearance › Font size) — live, when the reader changes it — and the `text-scale`
// attribute overrides it. Labels scale and what's measured with them lays out again; the change
// is a transition, not a jump.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/textscale.browser.test.mjs
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

/** A label in the theme's size and a bar as wide as the label measures. */
const doc = {
  datars: 1, size: { width: 640, height: 200 },
  scene: { kind: "group", key: "root", children: [
    { kind: "text", key: "label", text: "Monthly income", at: [20, 40], style: { size: "$size.label", baseline: "middle" } },
    { kind: "shape", key: "bar", geom: { type: "rect", x: 20, y: 100, w: "=measure(\"Monthly income\", token(\"size.label\")) * 3", h: 30 }, fill: "$accent", semantics: { role: "datum", label: "Bar" } }] },
};

const page = (attrs) => `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 0 }</style><datars-view doc="/doc.json" height="200" style="width:640px" ${attrs}></datars-view>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

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

test("text size: the browser's default font size, live, and the text-scale attribute", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const open = async (attrs = "", fontSize) => {
      const ctx = await browser.newContext({ viewport: { width: 700, height: 300 } });
      const p = await ctx.newPage();
      const cdp = await ctx.newCDPSession(p);
      // The browser's Font size setting (Chrome: Settings › Appearance › Font size).
      if (fontSize) await cdp.send("Page.setFontSizes", { fontSizes: { standard: fontSize, fixed: 13 } });
      const errors = [];
      p.on("pageerror", (e) => errors.push(e.message));
      await p.goto(`${origin}/?attrs=${encodeURIComponent(attrs)}`);
      await p.waitForFunction(() => document.querySelector("datars-view")?.dataset.renderer, null, { timeout: 30_000 });
      await p.waitForTimeout(400);
      return { p, cdp, errors };
    };
    /** The bar's width and the label's height, as the element's hit test sees them. */
    const sizes = (p) => p.evaluate(() => {
      const el = document.querySelector("datars-view");
      const bar = el.hitTest(24, 115).find((h) => h.path.includes("bar"));
      const label = el.hitTest(26, 40).find((h) => h.kind === "text");
      return { bar: bar?.bounds[2] ?? null, label: label?.bounds[3] ?? null, scale: el.textScale };
    });

    const base = await open();
    const one = await sizes(base.p);
    assert.equal(one.scale, 1, "a 16 px default is the theme's size");
    assert.ok(one.bar > 100 && one.label > 8, JSON.stringify(one));

    // The page says: twice the theme's.
    {
      const { p, errors } = await open('text-scale="2"');
      const two = await sizes(p);
      assert.equal(two.scale, 2);
      assert.ok(Math.abs(two.bar - 2 * one.bar) < 3, `the measured bar doubles: ${one.bar} → ${two.bar}`);
      assert.ok(two.label > 1.8 * one.label, `the label doubles: ${one.label} → ${two.label}`);
      assert.deepEqual(errors, []);
    }

    // A reader whose browser's default is 24 px gets 1.5 ×; `text-scale="1"` keeps the theme's.
    {
      const { p } = await open("", 24);
      const big = await sizes(p);
      assert.equal(big.scale, 1.5);
      assert.ok(Math.abs(big.bar - 1.5 * one.bar) < 3, `${one.bar} → ${big.bar}`);
      const fixed = await open('text-scale="1"', 24);
      assert.equal((await sizes(fixed.p)).bar, one.bar);
    }

    // Live: the reader changes the setting with the chart open; the chart moves to the new
    // layout over a transition (frames between), and the poster never comes back.
    {
      const { p, cdp } = await open();
      await p.evaluate(() => {
        const el = document.querySelector("datars-view");
        window.__widths = [];
        const tick = () => { const h = el.hitTest(24, 115).find((x) => x.path.includes("bar")); if (h) window.__widths.push(Math.round(h.bounds[2])); if (window.__widths.length < 400) requestAnimationFrame(tick); };
        requestAnimationFrame(tick);
      });
      await cdp.send("Page.setFontSizes", { fontSizes: { standard: 32, fixed: 13 } });
      await p.waitForTimeout(1600);
      const after = await sizes(p);
      assert.equal(after.scale, 2);
      assert.ok(Math.abs(after.bar - 2 * one.bar) < 3, `${one.bar} → ${after.bar}`);
      const widths = await p.evaluate(() => window.__widths);
      const between = widths.filter((w) => w > one.bar + 4 && w < after.bar - 4);
      assert.ok(between.length >= 3, `the bar grows over frames, not in one jump: ${[...new Set(widths)].join(",")}`);
      // The attribute takes over from the setting, and back to it.
      await p.evaluate(() => document.querySelector("datars-view").setAttribute("text-scale", "1"));
      await p.waitForTimeout(1600);
      assert.equal((await sizes(p)).bar, one.bar);
      await p.evaluate(() => { document.querySelector("datars-view").textScale = "auto"; });
      await p.waitForTimeout(1600);
      assert.equal((await sizes(p)).scale, 2);
    }
  } finally {
    await browser.close();
    server.close();
  }
});
