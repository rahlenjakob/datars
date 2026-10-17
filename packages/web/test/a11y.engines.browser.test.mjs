// What a screen reader and a keyboard get from <datars-view> in each browser engine Playwright
// drives — Chromium, WebKit (Safari's) and Gecko (Firefox's) — read from each one's own
// accessibility tree (`page.accessibility.snapshot()`): the chart's name and summary, its data as
// a table, the controls' roles and states, the arrow keys through the data, a step announced, a
// brush operated from the keyboard and a click's result said by the live region, forced colours,
// and (Firefox) the reader's default font size as the text size.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright with its
// browsers (`npx playwright install webkit firefox`); an engine that isn't installed is skipped:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/a11y.engines.browser.test.mjs
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

const use = (recipe, params) => ({ kind: "use", recipe: `@datars/std/${recipe}`, params });
const docs = {
  bars: {
    datars: 1, title: "Fruit sold on Saturday", description: "Kilograms of each fruit a market stall sold.",
    size: { width: 600, height: 320 },
    data: { fruit: { values: { name: ["Apples", "Pears", "Plums"], kg: [42, 18, 27] }, key: ["name"] } },
    scene: use("plot", { data: "fruit", x: "name", y: "kg", title: "Fruit sold (kg)", children: [use("bar", {})] }),
    program: { states: [{ name: "all", narration: { title: "Saturday", text: "Apples sold best." } }, { name: "again", narration: { title: "Still Saturday", text: "Pears sold least." } }] },
  },
  controls: {
    datars: 1, title: "Controls", size: { width: 600, height: 200 },
    signals: { on: { type: "bool", default: true }, shown: { type: "keyset", default: ["a"] }, q: { type: "str", default: "q1" } },
    scene: { kind: "group", key: "root", layout: { type: "rows", gap: 8, padding: 8 }, children: [
      use("toggle", { signal: "on", label: "Averages" }),
      use("checklist", { signal: "shown", options: ["a", "b"], labels: ["Apples", "Pears"] }),
      use("segmented", { signal: "q", options: ["q1", "q2"], labels: ["Q1", "Q2"], label: "Quarter" }),
    ] },
  },
  /** A brushable area, and legend entries a click filters by. */
  brush: {
    datars: 1, title: "Brush", size: { width: 600, height: 200 },
    data: { fruit: { values: { name: ["Apples", "Pears", "Plums"], kg: [42, 18, 27] }, key: ["name"] } },
    signals: { hidden: { type: "keyset", default: [] } },
    tables: { shown: { from: "fruit", ops: [{ op: "filter", expr: { expr: "!hidden.has(d.name)" } }] } },
    scene: { kind: "group", key: "root", children: [
      { kind: "group", key: "area", scales: { x: { type: "linear", domain: [0, 100], range: [0, 600] } }, semantics: { role: "group", label: "Prices" }, on: { brush: { brush: "sel" } },
        children: [{ kind: "shape", key: "bg", geom: { type: "rect", x: 0, y: 0, w: 600, h: 60 }, fill: "$surface" }] },
      { kind: "group", key: "total", semantics: { role: "group", label: { expr: "'Total: ' + format(table.sum('shown', 'kg'), 'd') + ' kg'" } }, children: [] },
      { kind: "group", key: "legend", children: [{ kind: "repeat", from: "fruit", template: { kind: "shape", key: "=d.name", geom: { type: "rect", x: "=d.index * 80", y: 80, w: 60, h: 20 }, fill: "$accent",
        semantics: { role: "legend-item", label: { expr: "d.name + (hidden.has(d.name) ? ', hidden' : '')" } }, on: { activate: { toggle: "hidden", value: "=d.name" } } } }] },
      { kind: "group", key: "bars", children: [{ kind: "repeat", from: "shown", template: { kind: "shape", key: "=d.name", geom: { type: "rect", x: "=d.index * 80", y: 120, w: 60, h: "=d.kg" }, fill: "$accent",
        semantics: { role: "datum", label: { expr: "d.name + ': ' + d.kg" }, value: "=d.kg" } } }] },
    ] },
  },
};

const page = (name, h) => `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 40px }</style><button id="before">Before</button>
<datars-view doc="/${name}.json" height="${h}" style="width:600px"></datars-view>`;
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    const m = /^\/(\w+)(\.json)?$/.exec(path);
    if (m && docs[m[1]]) return m[2] ? send(JSON.stringify(docs[m[1]]), "application/json") : send(page(m[1], docs[m[1]].size.height), "text/html; charset=utf-8");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

/** The accessibility tree as lines: `role "name" checked=…`, indented by depth. */
function lines(n, d = 0, out = []) {
  if (!n) return out;
  out.push(`${"  ".repeat(d)}${n.role}${n.name ? ` "${n.name}"` : ""}${n.checked !== undefined ? ` checked=${n.checked}` : ""}`);
  for (const c of n.children ?? []) lines(c, d + 1, out);
  return out;
}

const shadow = (p, f) => p.evaluate(f);
const live = (p) => shadow(p, () => document.querySelector("datars-view").shadowRoot.querySelector("[aria-live]").textContent);

for (const engine of ["chromium", "webkit", "firefox"]) {
  test(`${engine}: the tree, the keyboard, a brush, a click's result, forced colours`, { skip, timeout: 180_000 }, async (t) => {
    const pw = await import(playwright);
    let browser;
    try {
      browser = await pw[engine].launch(engine === "chromium" ? { channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome" } : {});
    } catch (e) {
      t.skip(`${engine} isn't installed: ${String(e.message).split("\n")[0]}`);
      return;
    }
    const server = await serve();
    const base = `http://127.0.0.1:${server.address().port}`;
    try {
      const ctx = await browser.newContext({ viewport: { width: 800, height: 500 } });
      const p = await ctx.newPage();
      const errors = [];
      p.on("pageerror", (e) => errors.push(e.message));
      await p.goto(`${base}/bars`);
      await p.locator("datars-view .mirror table").waitFor({ state: "attached", timeout: 30_000 });
      // The engine's own tree, asked until the data is in it (it lags the DOM a little).
      let tree = [];
      for (let i = 0; i < 50 && !tree.some((l) => l.includes("Plums")); i++) { tree = lines(await p.accessibility.snapshot()); await p.waitForTimeout(100); }
      const text = tree.join("\n");
      assert.match(text, /figure "Fruit sold on Saturday"/, text);
      assert.match(text, /"Kilograms of each fruit a market stall sold\. 3 data points, from 18 to 42\."/, text);
      assert.match(text, /rowheader( "Apples"|\n\s+text "Apples")/, `the data as a table: ${text}`);
      assert.match(text, /"y axis: 0, 10, 20, 30, 40, 50"/, text);
      assert.doesNotMatch(text, /(datum|tick|series): /, "no role words read out");
      // The arrow keys through the data, outlined and named on the chart.
      await p.locator("datars-view .mirror [data-nav]").first().focus();
      const walk = [];
      for (const k of ["ArrowDown", "End", "Home"]) {
        await p.keyboard.press(k);
        walk.push(await shadow(p, () => { const v = document.querySelector("datars-view").shadowRoot; return `${v.activeElement?.textContent}|${v.querySelector(".tip").hidden ? "" : v.querySelector(".tip").textContent}`; }));
      }
      assert.deepEqual(walk, ["Pears|Pears: 18", "Plums|Plums: 27", "Apples|Apples: 42"]);
      // A step: said once.
      await p.locator("datars-view").focus();
      await p.keyboard.press("ArrowRight");
      await p.waitForFunction(() => document.querySelector("datars-view").shadowRoot.querySelector("[aria-live]").textContent !== "", null, { timeout: 10_000 });
      assert.equal(await live(p), "Still Saturday. Pears sold least.");
      // Forced colours: the chart's text in the system's colours.
      await p.emulateMedia({ forcedColors: "active" });
      await p.waitForFunction(() => /^#(000000|ffffff)$/.test(document.querySelector("datars-view").style.getPropertyValue("--ink")), null, { timeout: 5_000 }).catch(() => {});
      const ink = await shadow(p, () => document.querySelector("datars-view").style.getPropertyValue("--ink"));
      assert.match(ink, /^#(000000|ffffff)$/, `the system's text colour: ${ink}`);

      // Controls: their roles and states in this engine's tree.
      const c = await ctx.newPage();
      await c.goto(`${base}/controls`);
      await c.locator("datars-view .mirror [role=switch]").waitFor({ state: "attached", timeout: 30_000 });
      let said = [];
      for (let i = 0; i < 50 && said.length < 5; i++) { said = lines(await c.accessibility.snapshot()).map((l) => l.trim()).filter((l) => /^(switch|checkbox|radio) /.test(l)); await c.waitForTimeout(100); }
      // (Firefox's snapshot leaves out `checked` when it's false.)
      assert.deepEqual(said.map((l) => l.replace(/ checked=false$/, "")), ['switch "Averages" checked=true', 'checkbox "Apples" checked=true', 'checkbox "Pears"', 'radio "Quarter: Q1" checked=true', 'radio "Quarter: Q2"']);

      // A brush from the keyboard, and a click's result, said by the live region.
      const b = await ctx.newPage();
      await b.goto(`${base}/brush`);
      const area = b.locator("datars-view .mirror [role=application]");
      await area.waitFor({ state: "attached", timeout: 30_000 });
      assert.equal(await area.getAttribute("aria-label"), "Prices, range selection: nothing selected");
      await area.focus();
      await b.keyboard.press("Enter");
      await b.keyboard.press("ArrowRight");
      assert.equal(await live(b), "0 to 20.");
      await b.keyboard.press("Enter");
      assert.equal(await live(b), "Selected 0 to 20.");
      assert.deepEqual(await b.evaluate(() => { const s = document.querySelector("datars-view").signals; return [s["sel.lo"], s["sel.hi"]]; }), [0, 20]);
      await b.locator("datars-view .mirror button", { hasText: /^Pears$/ }).focus();
      await b.keyboard.press("Enter");
      await b.waitForFunction(() => document.querySelector("datars-view").shadowRoot.querySelector("[aria-live]").textContent.startsWith("Pears"), null, { timeout: 10_000 });
      assert.equal(await live(b), "Pears, hidden. Total: 69 kg. Now 2 data points, from 27 to 42.");
      assert.deepEqual(errors, []);
    } finally {
      await browser.close();
      server.close();
    }
  });
}

test("firefox: the reader's default font size is the chart's text size", { skip, timeout: 120_000 }, async (t) => {
  const pw = await import(playwright);
  const server = await serve();
  try {
    const got = [];
    for (const px of [16, 24, 32]) {
      let browser;
      try {
        browser = await pw.firefox.launch({ firefoxUserPrefs: { "font.size.variable.x-western": px } });
      } catch (e) {
        t.skip(`firefox isn't installed: ${String(e.message).split("\n")[0]}`);
        return;
      }
      const p = await browser.newPage();
      await p.goto(`http://127.0.0.1:${server.address().port}/bars`);
      await p.locator("datars-view .mirror table").waitFor({ state: "attached", timeout: 30_000 });
      got.push(await p.evaluate(() => document.querySelector("datars-view").textScale));
      await browser.close();
    }
    assert.deepEqual(got, [1, 1.5, 2]);
  } finally {
    server.close();
  }
});
