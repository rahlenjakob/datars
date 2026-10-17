// What a screen reader and a keyboard get from <datars-view> in a real browser (Chrome's
// accessibility tree): the chart's name and summary on the element, its data as a table (name,
// value) rather than role words, axes as one line each, the canvas hidden, one tab stop into the
// data and the arrow keys through it with the mark outlined and named on the chart, and a step
// change announced once by the live region.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/a11y.browser.test.mjs
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

/** A std bar chart in a two-step story with narration. */
const use = (recipe, params) => ({ kind: "use", recipe: `@datars/std/${recipe}`, params });
const doc = {
  datars: 1, title: "Fruit sold on Saturday", description: "Kilograms of each fruit a market stall sold.",
  size: { width: 600, height: 320 },
  data: { fruit: { values: { name: ["Apples", "Pears", "Plums"], kg: [42, 18, 27] }, key: ["name"] } },
  scene: use("plot", { data: "fruit", x: "name", y: "kg", title: "Fruit sold (kg)", children: [use("bar", {})] }),
  program: { states: [
    { name: "all", narration: { title: "Saturday", text: "Apples sold best." } },
    { name: "again", narration: { title: "Still Saturday", text: "Pears sold least." } },
  ] },
};

const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 40px }</style><button id="before">Before</button>
<datars-view doc="/doc.json" height="320" style="width:600px"></datars-view>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

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

test("a screen reader gets the chart's name, summary and data; a keyboard walks the data", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const ctx = await browser.newContext({ viewport: { width: 800, height: 500 } });
    const p = await ctx.newPage();
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    await p.locator("datars-view .mirror table").waitFor({ state: "attached", timeout: 30_000 });
    await p.waitForFunction(() => document.querySelector("datars-view").getAttribute("aria-description"), null, { timeout: 10_000 });

    // The accessibility tree as Chrome builds it.
    const cdp = await ctx.newCDPSession(p);
    const { nodes } = await cdp.send("Accessibility.getFullAXTree");
    const v = (x) => x?.value ?? "";
    const figure = nodes.find((n) => v(n.role) === "figure");
    assert.equal(v(figure.name), "Fruit sold on Saturday", "named by the document's title");
    assert.equal(v(figure.description), "Kilograms of each fruit a market stall sold. 3 data points, from 18 to 42.");
    const roles = nodes.filter((n) => !n.ignored).map((n) => `${v(n.role)}:${v(n.name)}`);
    assert.ok(!roles.some((r) => r.startsWith("Canvas")), "the canvas is a picture: not in the tree");
    for (const want of ["rowheader:Apples", "cell:42", "rowheader:Pears", "cell:18", "rowheader:Plums", "cell:27"]) assert.ok(roles.includes(want), `${want} in ${roles.join(" | ")}`);
    const tree = await p.locator("datars-view").ariaSnapshot();
    assert.match(tree, /y axis: 0, 10, 20, 30, 40/, tree);
    assert.match(tree, /x axis: Apples, Pears, Plums/, tree);
    assert.doesNotMatch(tree, /(datum|tick|series|title): /, "no role words read out");
    assert.equal((tree.match(/Fruit sold \(kg\)/g) ?? []).length, 1, `the plot's title once (its group's name), not again as a title: ${tree}`);
    assert.match(tree, /table "kg by name"/, "the marks' table is named by their series");

    // The keyboard: the chart itself (it steps the story), its Previous and Next, then one stop
    // into the data.
    await p.locator("#before").focus();
    await p.keyboard.press("Tab");
    assert.equal(await p.evaluate(() => document.activeElement?.tagName), "DATARS-VIEW");
    const tabs = [];
    for (let i = 0; i < 3; i++) {
      await p.keyboard.press("Tab");
      tabs.push(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.activeElement?.textContent));
    }
    assert.deepEqual(tabs, ["←", "→", "Apples"]);
    const focused = () => p.evaluate(() => {
      const v = document.querySelector("datars-view");
      const a = v.shadowRoot.activeElement;
      const ring = v.shadowRoot.querySelector(".focus"), tip = v.shadowRoot.querySelector(".tip");
      const b = v.getBoundingClientRect(), r = ring.getBoundingClientRect();
      return { text: a?.textContent, ring: ring.hidden ? null : [r.x - b.x, r.y - b.y, r.width, r.height], tip: tip.hidden ? null : tip.textContent };
    });
    let f = await focused();
    assert.equal(f.text, "Apples");
    assert.ok(f.ring && f.ring[2] > 20 && f.ring[3] > 50, `the bar is outlined: ${JSON.stringify(f)}`);
    assert.equal(f.tip, "Apples: 42", "and named on the chart, as a pointer's tooltip would");
    await p.keyboard.press("ArrowDown");
    f = await focused();
    assert.equal(f.text, "Pears");
    assert.equal(f.tip, "Pears: 18");
    await p.keyboard.press("End");
    assert.equal((await focused()).text, "Plums");
    await p.keyboard.press("ArrowDown");
    assert.equal((await focused()).text, "Plums", "the last stays the last");
    await p.keyboard.press("Home");
    assert.equal((await focused()).text, "Apples");
    // Tab leaves the data in one press (one stop), and the outline goes.
    await p.keyboard.press("Tab");
    assert.notEqual(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.activeElement?.dataset?.nav), "");
    assert.equal((await focused()).ring, null);

    // A step: the live region says its narration once.
    await p.locator("datars-view").focus();
    const live = () => p.evaluate(() => document.querySelector("datars-view").shadowRoot.querySelector("[aria-live]").textContent);
    assert.equal(await live(), "", "the first state isn't announced: the name and summary say it");
    await p.keyboard.press("ArrowRight");
    await p.waitForFunction(() => document.querySelector("datars-view").shadowRoot.querySelector("[aria-live]").textContent !== "", null, { timeout: 10_000 });
    assert.equal(await live(), "Still Saturday. Pears sold least.");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});

test("more contrast is the high-contrast mode; forced colours paint the chart's text in the system's", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    // The reader's settings as media features (CDP: Playwright's own options don't cover them all).
    const open = async (features = []) => {
      const ctx = await browser.newContext({ viewport: { width: 800, height: 500 } });
      const p = await ctx.newPage();
      await (await ctx.newCDPSession(p)).send("Emulation.setEmulatedMedia", { features });
      await p.goto(`http://127.0.0.1:${server.address().port}`);
      await p.waitForFunction(() => document.querySelector("datars-view")?.dataset.renderer, null, { timeout: 30_000 });
      await p.waitForTimeout(300);
      return p;
    };
    // The element mirrors the theme's inks as CSS properties (for its own chrome): read them.
    const inks = (p) => p.evaluate(() => { const s = document.querySelector("datars-view").style; return { ink: s.getPropertyValue("--ink"), paper: s.getPropertyValue("--paper"), muted: s.getPropertyValue("--muted") }; });
    const plain = await inks(await open());
    const more = await inks(await open([{ name: "prefers-contrast", value: "more" }]));
    assert.equal(more.ink, "#000000", `high contrast's ink: ${JSON.stringify(more)}`);
    assert.notEqual(more.muted, plain.muted, "muted text is darker in high contrast");
    const forced = await open([{ name: "forced-colors", value: "active" }, { name: "prefers-color-scheme", value: "dark" }]);
    const system = await forced.evaluate(() => {
      const probe = document.createElement("span");
      probe.style.cssText = "color: CanvasText; background-color: Canvas";
      document.body.appendChild(probe);
      const cs = getComputedStyle(probe);
      return { ink: cs.color, paper: cs.backgroundColor };
    });
    const f = await inks(forced);
    const rgb = (hex) => `rgb(${[1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)).join(", ")})`;
    assert.equal(rgb(f.ink), system.ink, `the chart's ink is CanvasText: ${JSON.stringify({ f, system })}`);
    assert.equal(rgb(f.paper), system.paper, "its paper is Canvas");
    assert.equal(rgb(f.muted), system.ink, "muted text too: no grey a contrast theme doesn't have");
  } finally {
    await browser.close();
    server.close();
  }
});
