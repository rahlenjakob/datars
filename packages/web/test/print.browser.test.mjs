// Printing a page of charts (Cmd+P, `window.print()`, a PDF): every chart that has drawn prints
// its current state — not blank, not its poster — whether it is on screen, was on screen and left,
// or never came into view; in light (the default `print-mode`) even when it shows dark; one that
// starts while the page is printing prints in the next print; and afterwards nothing is held.
//
// Chromium prints a real PDF (`page.pdf`); with `pdftoppm` (poppler) on the PATH its pages are
// rasterized and each chart's page is compared with the chart's own render. WebKit and Firefox
// have no PDF in Playwright: their print-media rendering is checked through screenshots.
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/print.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { copyFileSync, readFileSync, existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const dist = join(root, "packages/web/dist");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(dist, "wasm/datars_host_web_bg.wasm")) || !existsSync(join(dist, "datars.js")) ? "build the runtime first" : false;
const pdftoppm = (() => { try { execFileSync("pdftoppm", ["-v"], { stdio: "ignore" }); return true; } catch { return false; } })();

const use = (recipe, params) => ({ kind: "use", recipe: `@datars/std/${recipe}`, params });
const bars = (title, kg) => ({
  datars: 1, title, size: { width: 600, height: 300 },
  data: { fruit: { values: { name: ["Apples", "Pears", "Plums", "Figs"], kg }, key: ["name"] } },
  scene: use("plot", { data: "fruit", x: "name", y: "kg", title, children: [use("bar", {})] }),
});
const line = (title) => ({
  datars: 1, title, size: { width: 600, height: 300 },
  data: { t: { values: { x: [1, 2, 3, 4, 5, 6], y: [3, 5, 2, 6, 4, 7] }, key: ["x"] } },
  scene: use("plot", { data: "t", x: "x", y: "y", title, children: [use("line", {})] }),
});
const DOCS = { a: bars("On screen", [42, 18, 27, 9]), b: line("Seen, then left"), c: bars("Seen in dark, then left", [5, 30, 12, 22]), d: line("Never seen"), e: bars("Never seen either", [12, 12, 30, 40]), late: bars("Started for the print", [1, 2, 3, 4]) };
// One chart per printed page; on screen, far apart.
const PAGE = `<!doctype html><html lang="en"><meta charset="utf-8"><script type="module" src="/runtime/datars.js"></script>
<style>body { margin: 20px; font: 14px system-ui; } .gap { height: 1600px; } datars-view { display: block; width: 100%; }
@media print { .gap { display: none; } section:not(:first-of-type) { break-before: page; } }</style>
<section><h2>a</h2><datars-view id="a" doc="/a.json" height="300"></datars-view></section><div class="gap"></div>
<section><h2>b</h2><datars-view id="b" doc="/b.json" height="300"></datars-view></section><div class="gap"></div>
<section><h2>c</h2><datars-view id="c" doc="/c.json" height="300" mode="dark"></datars-view></section><div class="gap"></div>
<section><h2>d</h2><datars-view id="d" doc="/d.json" height="300"></datars-view></section><div class="gap"></div>
<section><h2>e</h2><datars-view id="e" doc="/e.json" height="300"></datars-view></section>
<section id="late"><h2>late</h2></section>
<script>
  // As a page that mounts charts only near the viewport would: this one starts when printing.
  matchMedia("print").addEventListener("change", (e) => {
    if (!e.matches || document.querySelector("#late datars-view")) return;
    const v = document.createElement("datars-view"); v.id = "late-view"; v.setAttribute("doc", "/late.json"); v.setAttribute("height", "300");
    document.querySelector("#late").appendChild(v);
  });
</script></html>`;
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf" };

function serve() {
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(PAGE, "text/html; charset=utf-8");
    const m = path.match(/^\/(\w+)\.json$/);
    if (m && DOCS[m[1]]) return send(JSON.stringify(DOCS[m[1]]), "application/json");
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    res.writeHead(404).end();
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

const ready = (p, id) => p.waitForFunction((i) => document.getElementById(i)?.dataset.renderer, id, { timeout: 30_000 });
/** Scroll each listed chart into view and hold it there a moment, then go back to the top. */
async function visit(p, ids) {
  for (const id of ids) {
    await p.locator(`#${id}`).scrollIntoViewIfNeeded();
    await p.waitForTimeout(700);
  }
  await p.evaluate(() => scrollTo(0, 0));
  await p.waitForTimeout(300);
}

/** Every chart's state under print media: its print copy's pixels (RGBA, its canvas's size), and
 * the same state rendered by the engine's CPU reference in the print mode (light) to compare with;
 * whether its poster or live canvas would print. */
const printState = (p) => p.evaluate(() => [...document.querySelectorAll("datars-view")].map((el) => {
  const sr = el.shadowRoot, copy = sr.querySelector("canvas.print");
  const live = sr.querySelector("canvas:not(.print)"), poster = sr.querySelector(".poster");
  const out = { id: el.id, copy: !!copy, liveShown: getComputedStyle(live).display !== "none", posterShown: poster.childElementCount > 0 && getComputedStyle(poster).display !== "none" };
  if (!copy) return out;
  const px = copy.getContext("2d").getImageData(0, 0, copy.width, copy.height).data;
  const view = el.view, screen = el.getAttribute("mode") ?? "light";
  view.set_mode("light");
  const ref = view.pixels(performance.now());
  view.set_mode(screen);
  let diff = 0, n = 0, sum = 0, sum2 = 0;
  for (let i = 0; i < px.length; i += 16) {
    const g = (px[i] + px[i + 1] + px[i + 2]) / 3;
    sum += g; sum2 += g * g; n++;
    if (ref.length === px.length) diff += (Math.abs(px[i] - ref[i]) + Math.abs(px[i + 1] - ref[i + 1]) + Math.abs(px[i + 2] - ref[i + 2])) / 3;
  }
  return { ...out, w: copy.width, h: copy.height, sameSize: ref.length === px.length, diff: diff / n, spread: Math.sqrt(sum2 / n - (sum / n) ** 2), corner: [...px.slice(0, 3)] };
}));

function checkCopies(states, ids) {
  for (const id of ids) {
    const s = states.find((x) => x.id === id);
    assert.ok(s?.copy, `${id}: a print copy`);
    assert.ok(!s.posterShown, `${id}: had drawn, so not its poster`);
    assert.ok(!s.liveShown, `${id}: the copy prints, not the live canvas`);
    assert.ok(s.spread > 12, `${id}: not blank (spread ${s.spread?.toFixed(1)})`);
    assert.ok(s.sameSize && s.diff < 10, `${id}: its own state, as the engine renders it (mean difference ${s.diff?.toFixed(2)})`);
    assert.ok(s.corner.every((c) => c > 235), `${id}: in light (the paper at its corner is ${s.corner})`);
  }
}

/** A printed page (PNG from pdftoppm) and the chart's render compared: both reduced to a 48 × 24
 * grey grid over the chart's box, correlated. */
function correlate(a, b) {
  const n = a.length, ma = a.reduce((s, x) => s + x, 0) / n, mb = b.reduce((s, x) => s + x, 0) / n;
  let c = 0, va = 0, vb = 0;
  for (let i = 0; i < n; i++) { c += (a[i] - ma) * (b[i] - mb); va += (a[i] - ma) ** 2; vb += (b[i] - mb) ** 2; }
  return c / Math.sqrt(va * vb || 1);
}

for (const engine of ["chromium", "webkit", "firefox"]) {
  test(`${engine}: printed, every chart that drew shows its current state, in light; one started for the print shows next time; nothing held after`, { skip, timeout: 240_000 }, async () => {
    const pw = await import(playwright);
    const server = await serve();
    const origin = `http://127.0.0.1:${server.address().port}`;
    const browser = await pw[engine].launch(engine === "chromium" ? { channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true } : { headless: true });
    const tmp = mkdtempSync(join(tmpdir(), "datars-print-"));
    try {
      const p = await browser.newPage({ viewport: { width: 1000, height: 700 } });
      await p.goto(origin);
      for (const id of ["a", "b", "c", "d", "e"]) await ready(p, id);
      await visit(p, ["b", "c"]); // drawn, then off screen; d and e never come into view
      await p.emulateMedia({ media: "print" });
      await p.waitForTimeout(200);
      checkCopies(await printState(p), ["a", "b", "c", "d", "e"]);
      if (engine === "chromium") {
        const pdf = join(tmp, "page.pdf");
        await p.pdf({ path: pdf, format: "A4", printBackground: true });
        if (process.env.DATARS_PRINT_OUT) copyFileSync(pdf, join(process.env.DATARS_PRINT_OUT, "print-test.pdf")); // to look at
        if (pdftoppm) {
          execFileSync("pdftoppm", ["-r", "96", "-gray", "-png", pdf, join(tmp, "pg")]);
          const pages = readdirSync(tmp).filter((f) => f.startsWith("pg")).sort();
          assert.ok(pages.length >= 5, `one page per chart (${pages.length})`);
          // Each page's ink against its chart's render, both on a grey grid (the browser decodes the PNG).
          const grids = await p.evaluate(async (pngs) => {
            // The box's darkness on a 48 × 24 grid, each cell the mean of every pixel in it (a thin
            // line still darkens its cells; a resampled picture would lose it).
            const grid = (img, box) => {
              const c = document.createElement("canvas"); c.width = Math.round(box[2]); c.height = Math.round(box[3]);
              const g = c.getContext("2d"); g.fillStyle = "#fff"; g.fillRect(0, 0, c.width, c.height); g.drawImage(img, box[0], box[1], box[2], box[3], 0, 0, c.width, c.height);
              const d = g.getImageData(0, 0, c.width, c.height).data, out = new Array(48 * 24).fill(0), n = new Array(48 * 24).fill(0);
              for (let y = 0; y < c.height; y++) for (let x = 0; x < c.width; x++) { const k = Math.floor((y * 24) / c.height) * 48 + Math.floor((x * 48) / c.width); out[k] += 255 - (d[(y * c.width + x) * 4] + d[(y * c.width + x) * 4 + 1] + d[(y * c.width + x) * 4 + 2]) / 3; n[k]++; }
              return out.map((v, k) => v / (n[k] || 1));
            };
            const out = [];
            // Where the ink is: the bounding box of pixels darker than the paper, from row `from`.
            const inkBox = (src, from = 0) => {
              const c = document.createElement("canvas"); c.width = src.width; c.height = src.height; const g = c.getContext("2d"); g.fillStyle = "#fff"; g.fillRect(0, 0, c.width, c.height); g.drawImage(src, 0, 0);
              const d = g.getImageData(0, 0, c.width, c.height).data; let x0 = c.width, y0 = c.height, x1 = -1, y1 = -1;
              for (let y = from; y < c.height; y++) for (let x = 0; x < c.width; x++) { const k = (y * c.width + x) * 4; if (d[k] + d[k + 1] + d[k + 2] < 600) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; } }
              return x1 < 0 ? null : [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
            };
            for (const [i, b64] of pngs.entries()) {
              const img = new Image(); img.src = `data:image/png;base64,${b64}`; await img.decode();
              const copy = document.querySelectorAll("datars-view")[i]?.shadowRoot.querySelector("canvas.print");
              // (Under the page's own heading: the section's h2 at the top of the page.)
              const pageBox = inkBox(img, Math.round(img.height * 0.06)), chartBox = copy && inkBox(copy);
              // The chart's ink on the page, as wide as its ink, as tall as its own ink box at that
              // scale, from the bottom up (above it is the page's heading).
              const k = pageBox && chartBox ? pageBox[2] / chartBox[2] : 1;
              const onPage = pageBox && chartBox && [pageBox[0], pageBox[1] + pageBox[3] - chartBox[3] * k, pageBox[2], chartBox[3] * k];
              out.push({ inked: !!pageBox, page: onPage && grid(img, onPage), chart: chartBox && grid(copy, chartBox), boxes: [pageBox, chartBox] });
            }
            return out;
          }, pages.slice(0, 5).map((f) => readFileSync(join(tmp, f)).toString("base64")));
          for (const [i, g] of grids.entries()) {
            assert.ok(g.inked, `PDF page ${i + 1}: the chart is printed (not blank)`);
            const r = correlate(g.page, g.chart);
            assert.ok(r > 0.6, `PDF page ${i + 1}: its chart's own picture (correlation ${r.toFixed(2)}; ink ${JSON.stringify(g.boxes)})`);
          }
        }
      } else {
        // The print-media rendering as the engine paints it: each chart's box, not blank.
        for (const id of ["a", "b", "c", "d", "e"]) {
          const png = await p.locator(`#${id}`).screenshot();
          const spread = await p.evaluate(async (b64) => {
            const img = new Image(); img.src = `data:image/png;base64,${b64}`; await img.decode();
            const c = document.createElement("canvas"); c.width = img.width; c.height = img.height; const g = c.getContext("2d"); g.drawImage(img, 0, 0);
            const d = g.getImageData(0, 0, c.width, c.height).data; let s = 0, s2 = 0, n = 0;
            for (let i = 0; i < d.length; i += 16) { const v = (d[i] + d[i + 1] + d[i + 2]) / 3; s += v; s2 += v * v; n++; }
            return Math.sqrt(s2 / n - (s / n) ** 2);
          }, png.toString("base64"));
          assert.ok(spread > 12, `${id}: printed, not blank (spread ${spread.toFixed(1)})`);
        }
      }
      // The chart a page starts only when printing: it loads meanwhile, and the next print has it.
      await ready(p, "late-view");
      await p.waitForFunction(() => document.getElementById("late-view").shadowRoot.querySelector("canvas.print"), null, { timeout: 10_000 });
      checkCopies(await printState(p), ["late-view"]);
      // Printing over: the copies and their styles go; the charts draw as before (dark stays dark).
      await p.emulateMedia({ media: "screen" });
      await p.waitForTimeout(200);
      const after = await p.evaluate(() => [...document.querySelectorAll("datars-view")].map((el) => ({ copies: el.shadowRoot.querySelectorAll("canvas.print").length, styles: el.shadowRoot.querySelectorAll("style").length, printed: el.shadowRoot.querySelector(".stage").classList.contains("printed") })));
      assert.deepEqual(after.filter((x) => x.copies || x.styles > 1 || x.printed), []);
      await p.locator("#c").scrollIntoViewIfNeeded();
      await p.waitForTimeout(500);
      const corner = await p.evaluate(() => { const el = document.getElementById("c"); el.view.frame(performance.now()); const c = document.createElement("canvas"); const live = el.shadowRoot.querySelector("canvas"); c.width = 4; c.height = 4; const g = c.getContext("2d"); g.drawImage(live, 0, 0, 4, 4, 0, 0, 4, 4); return [...g.getImageData(0, 0, 1, 1).data.slice(0, 3)]; });
      assert.ok(corner.every((v) => v < 60), `c is dark on screen again (${corner})`);
      // The picture a page keeps of a chart it unmounts (@datars/react does): the chart as it
      // prints, as a PNG — here of c, dark on screen, light in the picture; and c still dark.
      const pic = await p.evaluate(async () => {
        const el = document.getElementById("c"), blob = await el.printPicture();
        const bmp = await createImageBitmap(blob), c = document.createElement("canvas");
        c.width = bmp.width; c.height = bmp.height; const g = c.getContext("2d"); g.drawImage(bmp, 0, 0);
        const d = g.getImageData(0, 0, c.width, c.height).data; let s = 0, s2 = 0, n = 0;
        for (let i = 0; i < d.length; i += 16) { const v = (d[i] + d[i + 1] + d[i + 2]) / 3; s += v; s2 += v * v; n++; }
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        const live = el.shadowRoot.querySelector("canvas"); el.view.frame(performance.now());
        const k = document.createElement("canvas"); k.width = 1; k.height = 1; k.getContext("2d").drawImage(live, 0, 0, 1, 1, 0, 0, 1, 1);
        return { type: blob.type, w: bmp.width, spread: Math.sqrt(s2 / n - (s / n) ** 2), corner: [...d.slice(0, 3)], live: [...k.getContext("2d").getImageData(0, 0, 1, 1).data.slice(0, 3)] };
      });
      assert.equal(pic.type, "image/png");
      assert.ok(pic.w > 100 && pic.spread > 12 && pic.corner.every((v) => v > 235), `printPicture: the chart in light (${JSON.stringify(pic)})`);
      assert.ok(pic.live.every((v) => v < 60), `and on screen it is still dark (${pic.live})`);
    } finally {
      await browser.close();
      server.close();
      rmSync(tmp, { recursive: true, force: true });
    }
  });
}
