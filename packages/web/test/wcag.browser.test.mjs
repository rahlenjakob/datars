// WCAG 2.2 success criteria the web element answers for, in a real browser (Chrome), each test
// named for its criterion (`wcag_2_1_2_…`): scripts/a11y-report.mjs runs them with the engine's
// own (crates/datars-test/tests/wcag.rs) and the site's accessibility page shows the results.
// With `DATARS_AXE` set to axe-core's `axe.min.js` (a dev tool, not a dependency), axe's WCAG A
// and AA rules run over the element too.
//
// Needs `scripts/build-wasm.sh` and `pnpm -C packages/web build` first, and Playwright:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/wcag.browser.test.mjs
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
const fruit = { values: { name: ["Apples", "Pears", "Plums"], kg: [42, 18, 27] }, key: ["name"] };
/** Bars a pointer hovers, in a two-step story. */
const bars = {
  datars: 1, title: "Fruit sold", description: "Kilograms of each fruit.", size: { width: 600, height: 320 },
  data: { fruit }, scene: use("plot", { data: "fruit", x: "name", y: "kg", children: [use("bar", {})] }),
  program: { states: [{ name: "a", narration: { title: "Saturday", text: "Apples sold best." } }, { name: "b" }] },
};
/** A story that plays by itself. */
const autoplay = { ...bars, program: { drivers: ["autoplay", "steps", "keys"], states: [{ name: "a", hold: 1.5 }, { name: "b", hold: 1.5 }, { name: "c", hold: 1.5 }, { name: "d", hold: 1.5 }] } };
/** A view to pan and zoom. */
const pannable = {
  datars: 1, title: "Dots", size: { width: 600, height: 320 },
  data: { d: { values: { x: [10, 50, 90, 30, 70], y: [20, 80, 40, 60, 10] }, key: ["x"] } },
  scene: { kind: "view", key: "v", camera: { fit: { bbox: [0, 0, 100, 100] }, explore: "cam" }, children: [
    { kind: "repeat", from: "d", template: { kind: "shape", key: "=d.x", geom: { type: "circle", cx: "=d.x", cy: "=d.y", r: 3 }, fill: "$mark", semantics: { role: "datum", label: "=`${d.x}, ${d.y}`" } } },
  ] },
};
/** A source the page feeds again and again (a live feed). */
const live = { datars: 1, title: "Live", size: { width: 600, height: 240 }, data: { fruit: { ...fruit, live: { mode: "snapshot" } } },
  scene: use("plot", { data: "fruit", x: "name", y: "kg", children: [use("bar", {})] }) };

const DOCS = { bars, autoplay, pannable, live };
const page = (name) => `<!doctype html><html lang="en"><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 40px }</style><button id="before">Before</button>
<datars-view doc="/${name}.json" height="320" style="width:600px"></datars-view><button id="after">After</button></html>`;
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    const m = path.match(/^\/(\w+)(\.json)?$/);
    if (m && DOCS[m[1]]) return m[2] ? send(JSON.stringify(DOCS[m[1]]), "application/json") : send(page(m[1]), "text/html; charset=utf-8");
    if (path === "/axe.js" && process.env.DATARS_AXE) return send(readFileSync(process.env.DATARS_AXE), "text/javascript");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

/** A page on one document, the chart drawn; `fn(p, server)`, then everything closed. */
async function on(name, fn, opts = {}) {
  const { chromium } = await import(playwright);
  const server = await serve();
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ viewport: { width: 800, height: 520 }, ...opts });
    await p.goto(`http://127.0.0.1:${server.address().port}/${name}`);
    await p.locator("datars-view .mirror p.summary").waitFor({ state: "attached", timeout: 30_000 });
    await p.waitForTimeout(400);
    await fn(p, server);
  } finally {
    await browser.close();
    server.close();
  }
}

const shadow = (p, fn, arg) => p.evaluate(([f, a]) => new Function("v", "a", `return (${f})(v, a)`)(document.querySelector("datars-view"), a), [fn.toString(), arg]);
const box = (p, sel) => shadow(p, (v, s) => { const r = v.shadowRoot.querySelector(s)?.getBoundingClientRect(); return r && { x: r.x, y: r.y, w: r.width, h: r.height }; }, sel);
/** A bar's centre on the page (from the element's semantics). */
const barAt = (p, name) => p.evaluate((n) => {
  const v = document.querySelector("datars-view"), r = v.getBoundingClientRect();
  const i = v.readout.find((x) => x.label.startsWith(n) && x.rect);
  return i && { x: r.x + i.rect[0] + i.rect[2] / 2, y: r.y + i.rect[1] + i.rect[3] / 2, top: r.y + i.rect[1] };
}, name);

test("wcag_1_4_12_text_spacing: the element's own text takes a reader's spacing without being cut", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    // WCAG's spacing, as a reader's style sheet sets it on the page (it reaches the element's
    // own text through inheritance: the narration card, the story buttons, a tooltip).
    await p.addStyleTag({ content: "* { line-height: 1.5 !important; letter-spacing: 0.12em !important; word-spacing: 0.16em !important; } p { margin-bottom: 2em !important; }" });
    await p.waitForTimeout(300);
    const cut = await shadow(p, (v) => [...v.shadowRoot.querySelectorAll(".card, .card *, .controls button, .playback button")]
      .filter((el) => el.offsetParent !== null && (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1)).map((el) => el.className || el.tagName));
    assert.deepEqual(cut, []);
  });
});

test("wcag_1_4_13_content_on_hover_or_focus: a tooltip can be dismissed with Esc, hovered, and stays", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    const a = await barAt(p, "Apples");
    await p.mouse.move(a.x, a.y);
    await p.waitForFunction(() => !document.querySelector("datars-view").shadowRoot.querySelector(".tip").hidden, null, { timeout: 5000 });
    const tip = await box(p, ".tip");
    const text = await shadow(p, (v) => v.shadowRoot.querySelector(".tip").textContent);
    assert.match(text, /Apples/);
    // Hoverable: the pointer moves onto the tooltip; it stays, saying the same.
    for (let k = 1; k <= 6; k++) await p.mouse.move(a.x + (tip.x + tip.w / 2 - a.x) * k / 6, a.y + (tip.y + tip.h / 2 - a.y) * k / 6);
    await p.waitForTimeout(200);
    assert.equal(await shadow(p, (v) => !v.shadowRoot.querySelector(".tip").hidden && v.shadowRoot.querySelector(".tip").textContent), text);
    // Persistent: it doesn't time out.
    await p.waitForTimeout(2500);
    assert.equal(await shadow(p, (v) => !v.shadowRoot.querySelector(".tip").hidden), true);
    // Dismissible without moving the pointer or the keyboard: Esc (the chart doesn't have focus).
    await p.mouse.move(a.x, a.y);
    await p.keyboard.press("Escape");
    assert.equal(await shadow(p, (v) => v.shadowRoot.querySelector(".tip").hidden), true);
    // Still on the same bar, it stays away; on another, it's back.
    await p.mouse.move(a.x + 1, a.y + 1);
    assert.equal(await shadow(p, (v) => v.shadowRoot.querySelector(".tip").hidden), true);
    const b = await barAt(p, "Plums");
    await p.mouse.move(b.x, b.y);
    await p.waitForFunction(() => /Plums/.test(document.querySelector("datars-view").shadowRoot.querySelector(".tip").textContent), null, { timeout: 5000 });
  });
});

test("wcag_2_1_1_keyboard and wcag_2_1_2_no_keyboard_trap: Tab goes into the chart, through it, and out of it both ways", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    await p.focus("#before");
    const where = () => p.evaluate(() => { const a = document.activeElement; const s = a?.shadowRoot?.activeElement; return s ? `${s.tagName}:${s.getAttribute("aria-label") ?? s.textContent}` : `${a?.tagName}#${a?.id}`; });
    const seen = [];
    for (let i = 0; i < 8; i++) {
      await p.keyboard.press("Tab");
      seen.push(await where());
      if (seen.at(-1) === "BUTTON#after") break;
    }
    assert.equal(seen.at(-1), "BUTTON#after", `out the far side: ${seen.join(" → ")}`);
    assert.ok(seen.some((s) => s.startsWith("TH:") || s.startsWith("BUTTON:Apples") || s.includes("Apples")), `the data is a stop: ${seen.join(" → ")}`);
    for (let i = 0; i < 8; i++) {
      await p.keyboard.press("Shift+Tab");
      if ((await where()) === "BUTTON#before") break;
    }
    assert.equal(await where(), "BUTTON#before", "and back out the near side");
  });
});

test("wcag_2_1_4_character_key_shortcuts: Shift+D works only on a focused chart", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    const panel = () => shadow(p, (v) => !v.shadowRoot.querySelector(".perf").hidden);
    await p.focus("#before");
    await p.keyboard.press("Shift+D");
    assert.equal(await panel(), false, "not when the chart doesn't have the keyboard");
    await p.locator("datars-view").focus();
    await p.keyboard.press("Shift+D");
    await p.waitForTimeout(100);
    assert.equal(await panel(), true, "on the chart: its stats panel");
  });
});

test("wcag_2_2_2_pause_stop_hide: what plays by itself has a Pause that holds it", { skip, timeout: 120_000 }, async () => {
  await on("autoplay", async (p) => {
    await p.waitForFunction(() => !document.querySelector("datars-view").shadowRoot.querySelector(".playback .pause").hidden, null, { timeout: 10_000 }).catch(() => assert.fail("no Pause shown for autoplay"));
    const pause = p.locator("datars-view .playback .pause");
    assert.equal(await pause.textContent(), "Pause");
    const size = await box(p, ".playback .pause");
    assert.ok(size.w >= 24 && size.h >= 24, `a target to hit (${size.w}×${size.h})`);
    await pause.click();
    assert.equal(await pause.textContent(), "Play");
    const at = await p.evaluate(() => document.querySelector("datars-view").status.index);
    await p.waitForTimeout(2500);
    assert.equal(await p.evaluate(() => document.querySelector("datars-view").status.index), at, "paused, the story stays");
    await pause.click();
    await p.waitForFunction((i) => document.querySelector("datars-view").status.index !== i, at, { timeout: 10_000 }).catch(() => assert.fail("played again, the story didn't move on"));
  });
  // A live feed: what arrives while paused waits, then comes in.
  await on("live", async (p) => {
    const feed = (kg) => p.evaluate((k) => document.querySelector("datars-view").provideData("fruit", { name: ["Apples", "Pears", "Plums"], kg: k }), kg);
    await feed([1, 2, 3]);
    await feed([4, 5, 6]);
    await p.waitForFunction(() => !document.querySelector("datars-view").shadowRoot.querySelector(".playback .pause").hidden, null, { timeout: 10_000 }).catch(() => assert.fail("no Pause shown for a live feed"));
    const summary = () => p.evaluate(() => document.querySelector("datars-view").shadowRoot.querySelector(".mirror p.summary")?.textContent ?? "");
    await p.waitForFunction(() => /from 4 to 6/.test(document.querySelector("datars-view").shadowRoot.querySelector(".mirror p.summary")?.textContent ?? ""), null, { timeout: 10_000 });
    await p.locator("datars-view .playback .pause").click();
    await feed([7, 8, 9]);
    await p.waitForTimeout(1500);
    assert.match(await summary(), /from 4 to 6/, "paused: the new rows wait");
    await p.locator("datars-view .playback .pause").click();
    await p.waitForFunction(() => /from 7 to 9/.test(document.querySelector("datars-view").shadowRoot.querySelector(".mirror p.summary")?.textContent ?? ""), null, { timeout: 10_000 });
  });
});

test("wcag_2_4_3_focus_order: the chart's name and summary, its data, then its story buttons", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    const order = await shadow(p, (v) => [...v.shadowRoot.querySelectorAll(".mirror, .playback, .controls")].map((el) => el.className.split(" ").pop()));
    assert.deepEqual(order, ["mirror", "playback", "controls"]);
    const first = await shadow(p, (v) => v.shadowRoot.querySelector(".mirror").firstElementChild?.className);
    assert.equal(first, "summary");
  });
});

test("wcag_2_4_7_focus_visible, wcag_2_4_11_focus_not_obscured and wcag_2_4_13_focus_appearance: the keyboard's mark is ringed, in full view", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    await p.locator("datars-view").focus();
    await p.keyboard.press("Tab");
    await p.waitForFunction(() => !document.querySelector("datars-view").shadowRoot.querySelector(".focus").hidden, null, { timeout: 5000 });
    const ring = await shadow(p, (v) => {
      const f = v.shadowRoot.querySelector(".focus"), cs = getComputedStyle(f), r = f.getBoundingClientRect();
      return { outline: parseFloat(cs.outlineWidth), offset: parseFloat(cs.outlineOffset), color: cs.outlineColor, inner: cs.boxShadow, x: r.x, y: r.y, w: r.width, h: r.height };
    });
    assert.ok(ring.outline >= 2, `a 2 px ring at least (${ring.outline})`);
    assert.match(ring.inner, /0px 0px 0px 2px/, "with the paper inside it: one of the two stands out on any mark");
    // The ring's colour against the paper: 3:1 at least.
    const lum = (c) => { const [r, g, b] = c.match(/[\d.]+/g).slice(0, 3).map((x) => +x / 255).map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4)); return 0.2126 * r + 0.7152 * g + 0.0722 * b; };
    const paper = await p.evaluate(() => getComputedStyle(document.querySelector("datars-view")).getPropertyValue("--paper") || "rgb(255,255,255)");
    const hexOrRgb = (c) => (c.startsWith("#") ? `rgb(${parseInt(c.slice(1, 3), 16)},${parseInt(c.slice(3, 5), 16)},${parseInt(c.slice(5, 7), 16)})` : c);
    const [l1, l2] = [lum(ring.color), lum(hexOrRgb(paper.trim()))];
    assert.ok((Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05) >= 3, "the ring stands out from the paper");
    // Not obscured: what names the mark (the tooltip) is beside it, not over it.
    const tip = await box(p, ".tip");
    if (tip && tip.w) {
      const overlap = Math.max(0, Math.min(tip.x + tip.w, ring.x + ring.w) - Math.max(tip.x, ring.x)) * Math.max(0, Math.min(tip.y + tip.h, ring.y + ring.h) - Math.max(tip.y, ring.y));
      assert.ok(overlap < ring.w * ring.h * 0.25, "the tooltip covers no more than a corner of the focused mark");
    }
  });
});

test("wcag_2_5_1_pointer_gestures and wcag_2_5_7_dragging_movements: a view zooms with one click, and Show all goes back", { skip, timeout: 120_000 }, async () => {
  await on("pannable", async (p) => {
    const r = await p.locator("datars-view").boundingBox();
    const zoom = () => p.evaluate(() => document.querySelector("datars-view").signals?.["cam.zoom"] ?? null);
    const before = await zoom();
    await p.mouse.dblclick(r.x + r.width * 0.7, r.y + 120);
    await p.waitForFunction((z) => (document.querySelector("datars-view").signals?.["cam.zoom"] ?? null) !== z, before, { timeout: 5000 });
    const reset = p.locator("datars-view .playback .reset");
    await reset.waitFor({ state: "visible", timeout: 5000 });
    assert.equal(await reset.textContent(), "Show all");
    const size = await box(p, ".playback .reset");
    assert.ok(size.w >= 24 && size.h >= 24, `a target to hit (${size.w}×${size.h})`);
    await reset.click();
    await p.waitForFunction((z) => (document.querySelector("datars-view").signals?.["cam.zoom"] ?? null) === z || !document.querySelector("datars-view").signals?.["cam.zoom"] || Math.abs(document.querySelector("datars-view").signals["cam.zoom"] - 1) < 1e-6, before, { timeout: 5000 });
    assert.equal(await reset.isVisible(), false, "back to all of it: Show all goes away");
  });
});

test("wcag_2_5_8_target_size: the element's own buttons are at least 24 × 24 px", { skip, timeout: 120_000 }, async () => {
  await on("bars", async (p) => {
    const small = await shadow(p, (v) => [...v.shadowRoot.querySelectorAll(".controls button")].map((b) => b.getBoundingClientRect()).filter((r) => r.width < 24 || r.height < 24).map((r) => `${r.width}×${r.height}`));
    assert.deepEqual(small, []);
  });
});

test("wcag_axe: axe-core's WCAG A and AA rules find nothing on the element (with DATARS_AXE)", { skip: skip || (!process.env.DATARS_AXE && "set DATARS_AXE to axe-core's axe.min.js"), timeout: 120_000 }, async () => {
  for (const name of ["bars", "pannable"]) {
    await on(name, async (p) => {
      await p.addScriptTag({ url: "/axe.js" });
      const res = await p.evaluate(async () => (await window.axe.run(document.querySelector("datars-view"), { runOnly: { type: "tag", values: ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"] } })).violations.map((v) => `${v.id}: ${v.nodes.length}`));
      assert.deepEqual(res, [], `${name}: ${res.join(", ")}`);
    });
  }
});
