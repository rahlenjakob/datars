// The text layer in a real browser: a chart's title can be selected and copied, a drag across
// several texts copies them a line each, and the chart still gets its pointer — a tooltip over a
// label on a mark, a click through a label, a pan that starts on a place name.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright, which this
// repo doesn't depend on — point at any install:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/textlayer.browser.test.mjs
//
// Chrome itself (`channel: "chrome"`; DATARS_BROWSER_CHANNEL overrides): bundled Chromium builds
// can hang on WebGPU.
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

/** A header (a title, a wrapped note, a clickable bar with a label on it, what was picked) over
 * an explorable view with a place name. */
const doc = {
  datars: 1, size: { width: 600, height: 400 },
  signals: { picked: { type: "str", default: "nothing" }, lit: { type: "bool", default: false } },
  scene: { kind: "group", key: "root", layout: { type: "rows" }, children: [
    { kind: "group", key: "head", size: { h: 120 }, semantics: { role: "group", label: "" }, children: [
      { kind: "text", key: "title", text: "Selectable chart title", at: [20, 36], style: { size: 22, weight: 700 }, semantics: { role: "title", label: "Selectable chart title" } },
      { kind: "text", key: "note", text: "A subtitle long enough to wrap onto a second line", at: [20, 50], style: { size: 13, max_width: 200, baseline: "top" } },
      { kind: "shape", key: "bar", geom: { type: "rect", x: 300, y: 20, w: 200, h: 80 }, fill: "$accent", pickable: true,
        semantics: { role: "datum", label: "Bar A: 42" }, on: { activate: { set: "picked", value: "A" } } },
      { kind: "text", key: "barlabel", text: "Label on the bar", at: [400, 60], style: { size: 14, align: "middle", baseline: "middle", ink: "#ffffff" } },
      { kind: "text", key: "picked", text: "=`Picked: ${picked}`", at: [300, 116], style: { size: 12 } },
      { kind: "shape", key: "switch", geom: { type: "rect", x: 520, y: 20, w: 60, h: 30 }, fill: "$muted", pickable: true,
        semantics: { role: "control", label: "=lit ? \"Light: on\" : \"Light: off\"" }, on: { activate: { set: "lit", value: "=!lit" } } }] },
    { kind: "view", key: "map", size: { h: 280 }, camera: { fit: { bbox: [0, 0, 600, 280] }, padding: 0, explore: "cam" }, children: [
      { kind: "shape", key: "land", geom: { type: "rect", x: 0, y: 0, w: 600, h: 280 }, fill: "$grid" },
      { kind: "text", key: "town", text: "Borlänge", at: [300, 140], style: { size: 14, align: "middle", baseline: "middle" } }] }] },
};

const page = `<!doctype html><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 60px }</style>
<p>Before the chart.</p><datars-view doc="/doc.json" height="400" style="width:600px"></datars-view><p>After the chart.</p>`;

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

test("chart text can be selected and copied, and the chart keeps its pointer", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const ctx = await browser.newContext({ viewport: { width: 800, height: 600 } });
    await ctx.grantPermissions(["clipboard-read", "clipboard-write"], { origin });
    const p = await ctx.newPage();
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(origin);
    const settled = () => p.waitForFunction(() => {
      const layer = document.querySelector("datars-view")?.shadowRoot?.querySelector(".texts");
      return layer && !layer.classList.contains("moving") && layer.querySelectorAll("span[data-key]").length >= 6;
    }, null, { timeout: 30_000 });
    await settled();
    const span = (text) => p.evaluate((text) => {
      const s = [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].find((s) => s.textContent === text);
      return s ? { ...s.getBoundingClientRect().toJSON(), pass: s.classList.contains("pass") } : null;
    }, text);
    const copy = async () => {
      await p.keyboard.press(process.platform === "darwin" ? "Meta+C" : "Control+C");
      return p.evaluate(() => navigator.clipboard.readText());
    };
    const tip = () => p.evaluate(() => { const t = document.querySelector("datars-view").shadowRoot.querySelector(".tip"); return t.hidden ? null : t.textContent; });
    const clear = () => p.evaluate(() => document.getSelection().removeAllRanges());

    // The layer is for the eye and the pointer; the semantics mirror is what screen readers read.
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.querySelector(".texts").getAttribute("aria-hidden")), "true");

    // A drag across the title selects it; copying copies it.
    const title = await span("Selectable chart title");
    assert.ok(title && !title.pass, "the title is in the layer");
    const mid = title.y + title.height / 2;
    await p.mouse.move(title.x + 1, mid);
    await p.mouse.down();
    await p.mouse.move(title.x + title.width - 1, mid, { steps: 6 });
    await p.mouse.up();
    assert.equal(await copy(), "Selectable chart title");
    assert.equal(await tip(), null, "no tooltip left over from the drag");

    // Overshooting, fast: past the title's end over the bare canvas, past its start and out of the
    // chart — still all of the title (the drag runs to the end of the text beside the pointer, as
    // in a paragraph; the page's own text isn't pulled in).
    for (const [from, to, how] of [[title.x + 1, title.x + title.width + 60, "past its end"], [title.x + title.width - 1, title.x - 40, "past its start, out of the chart"]]) {
      await clear();
      await p.mouse.move(from, mid);
      await p.mouse.down();
      await p.mouse.move((from + to) / 2, mid, { steps: 2 });
      await p.mouse.move(to, mid, { steps: 2 });
      await p.mouse.up();
      assert.equal(await copy(), "Selectable chart title", how);
    }

    // Several texts: a line each, a wrapped text's lines joined as the sentence they are.
    await clear();
    const note = await p.evaluate(() => [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].filter((s) => s.dataset.key.includes("note")).map((s) => s.getBoundingClientRect().toJSON()));
    assert.ok(note.length >= 2, "the note wraps");
    const last = note[note.length - 1];
    await p.mouse.move(title.x + 1, mid);
    await p.mouse.down();
    await p.mouse.move(last.x + last.width - 1, last.y + last.height / 2, { steps: 10 });
    await p.mouse.up();
    const several = "Selectable chart title\nA subtitle long enough to wrap onto a second line";
    assert.equal(await copy(), several);
    // The browser reads the layer the same way (find-in-page follows it: no match across labels).
    assert.equal(await p.evaluate(() => String(document.getSelection())), several);

    // Hovering a label on the bar reaches the bar: its tooltip.
    await clear();
    const label = await span("Label on the bar");
    await p.mouse.move(label.x + label.width / 2, label.y + label.height / 2, { steps: 3 });
    await p.waitForFunction(() => !document.querySelector("datars-view").shadowRoot.querySelector(".tip").hidden, null, { timeout: 5000 });
    assert.equal(await tip(), "Bar A: 42");

    // A click on the label is a click on the bar.
    assert.ok(await span("Picked: nothing"));
    await p.mouse.click(label.x + label.width / 2, label.y + label.height / 2);
    await p.waitForFunction(() => [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].some((s) => s.textContent === "Picked: A"), null, { timeout: 10_000 });

    // A press on a place name in the explorable view pans the view (no text selection).
    const town = await span("Borlänge");
    assert.ok(town?.pass, "text the chart drags takes no pointer");
    await p.mouse.move(town.x + town.width / 2, town.y + town.height / 2);
    await p.mouse.down();
    await p.mouse.move(town.x + town.width / 2 + 80, town.y + town.height / 2 + 30, { steps: 8 });
    await p.mouse.up();
    assert.equal(await p.evaluate(() => String(document.getSelection())), "");
    await p.waitForFunction((x0) => {
      const s = [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].find((s) => s.textContent === "Borlänge");
      return s && Math.abs(s.getBoundingClientRect().x - x0 - 80) < 2;
    }, town.x, { timeout: 10_000 });
    const panned = await span("Borlänge");
    assert.ok(Math.abs(panned.y - town.y - 30) < 2, "the place name moved with the map, and its text with it");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});

test("what a click acts on is a button in the semantics mirror: a keyboard presses it and keeps its place", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    // The page's chrome reads the brief status (no semantics tree): its `actions` are the buttons.
    const button = p.locator("datars-view ul.sr button", { hasText: "Bar A: 42" });
    await button.waitFor({ state: "attached", timeout: 30_000 });
    await button.focus();
    await p.keyboard.press("Enter");
    await p.waitForFunction(() => [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].some((s) => s.textContent === "Picked: A"), null, { timeout: 10_000 });
    // The mirror is rebuilt after a press: the keyboard stays on the same button.
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.activeElement?.textContent), "Bar A: 42");
    // The list is hidden: where its focus is shows on the chart, round the bar the button stands for.
    const ring = await p.evaluate(() => {
      const v = document.querySelector("datars-view");
      const f = v.shadowRoot.querySelector(".focus");
      const a = f.getBoundingClientRect(), b = v.getBoundingClientRect();
      return { hidden: f.hidden, x: a.x - b.x, y: a.y - b.y, w: a.width, h: a.height };
    });
    assert.equal(ring.hidden, false);
    assert.ok(Math.abs(ring.x - 300) < 2 && Math.abs(ring.y - 20) < 2 && Math.abs(ring.w - 200) < 2 && Math.abs(ring.h - 80) < 2, JSON.stringify(ring));
    await p.keyboard.press("Shift+Tab");
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.querySelector(".focus").hidden), true, "gone when the keyboard leaves the list");
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});

test("the mirror reads every labelled mark, not only what a click acts on", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    // Steps stay cheap (the brief status has only what a click acts on); the whole semantics
    // tree follows in idle time: a screen reader finds the title, not only the buttons.
    const title = p.locator("datars-view ul.sr li", { hasText: "title: Selectable chart title" });
    await title.waitFor({ state: "attached", timeout: 30_000 });
    const tree = await p.locator("datars-view").ariaSnapshot();
    assert.match(tree, /listitem: "title: Selectable chart title"/, tree);
    assert.match(tree, /button "Bar A: 42"/, tree);
    const unnamed = await p.evaluate(() => [...document.querySelector("datars-view").shadowRoot.querySelectorAll("ul.sr li")].filter((li) => li.textContent.trim() === "group:").length);
    assert.equal(unnamed, 0, "a group with no name says nothing");
    // A button whose label follows the state says the new state at once, not when the tree
    // catches up after the transition.
    await p.locator("datars-view ul.sr button", { hasText: "Light: off" }).focus();
    await p.keyboard.press("Enter");
    await p.waitForTimeout(60);
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").shadowRoot.activeElement?.textContent), "Light: on");
    // A press rebuilds the mirror: the whole tree is back once the chart settles.
    await p.locator("datars-view ul.sr button", { hasText: "Bar A: 42" }).focus();
    await p.keyboard.press("Enter");
    await p.waitForFunction(() => [...document.querySelector("datars-view").shadowRoot.querySelectorAll(".texts span[data-key]")].some((s) => s.textContent === "Picked: A"), null, { timeout: 10_000 });
    await title.waitFor({ state: "attached", timeout: 10_000 });
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});

test("the page reads the signals and hears what the reader's input changed", { skip, timeout: 120_000 }, async () => {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ viewport: { width: 800, height: 600 } });
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    await p.goto(`http://127.0.0.1:${server.address().port}`);
    await p.evaluate(() => {
      window.__changes = [];
      document.querySelector("datars-view").addEventListener("signal", (e) => window.__changes.push(e.detail.changed));
    });
    // Steps stay cheap (the brief status has only what a click acts on); the whole semantics
    // tree follows in idle time: a screen reader finds the title, not only the buttons.
    const title = p.locator("datars-view ul.sr li", { hasText: "title: Selectable chart title" });
    await title.waitFor({ state: "attached", timeout: 30_000 });
    const tree = await p.locator("datars-view").ariaSnapshot();
    assert.match(tree, /listitem: "title: Selectable chart title"/, tree);
    assert.match(tree, /button "Bar A: 42"/, tree);
    // The page reads the signals as it would set them, and hears when the reader changes one.
    assert.equal((await p.evaluate(() => document.querySelector("datars-view").signals)).picked, "nothing");
    await p.locator("datars-view ul.sr button", { hasText: "Bar A: 42" }).focus();
    await p.keyboard.press("Enter");
    await p.waitForFunction(() => window.__changes.some((c) => c.includes("picked")), null, { timeout: 10_000 });
    assert.equal((await p.evaluate(() => document.querySelector("datars-view").signals)).picked, "A");
    // A drag on the explorable view: its camera's signals.
    const box = await p.locator("datars-view").boundingBox();
    await p.mouse.move(box.x + 100, box.y + 300);
    await p.mouse.down();
    await p.mouse.move(box.x + 160, box.y + 320, { steps: 6 });
    await p.mouse.up();
    await p.waitForFunction(() => window.__changes.some((c) => c.includes("cam.x")), null, { timeout: 10_000 });
    const cam = await p.evaluate(() => document.querySelector("datars-view").signals);
    assert.equal(typeof cam["cam.x"], "number");
    // The mirror still has the whole tree after the press rebuilt it.
    await title.waitFor({ state: "attached", timeout: 10_000 });
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
    server.close();
  }
});
