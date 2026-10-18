// The web runtime reads no SVG and decodes no picture (docs/12-delivery.md §Icons and pictures):
// a raw document gets the default set's icons one compact file at a time from beside the runtime,
// and its pictures — PNG and JPEG here — are decoded by the browser. A published bundle's picture
// is decoded before the chart's first frame, so it never pops in after it.
//
// Needs `scripts/build-wasm.sh`, `pnpm -C packages/web build`, the release CLI (for the bundle) and
// Playwright, which this repo doesn't depend on — point at any install:
//
//   DATARS_PLAYWRIGHT=/path/to/node_modules/@playwright/test/index.mjs node --test packages/web/test/pictures.browser.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { execFileSync } from "node:child_process";
import { readFileSync, existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { dirname, join, extname, normalize } from "node:path";
import { deflateSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "../../..");
const dist = join(root, "packages/web/dist");
const cli = join(root, "target/release/datars");
const playwright = process.env.DATARS_PLAYWRIGHT;
const skip = !playwright ? "set DATARS_PLAYWRIGHT to a Playwright module" : !existsSync(join(dist, "wasm/datars_host_web_bg.wasm")) || !existsSync(join(dist, "icons/lucide/heart.json")) ? "build the runtime first" : !existsSync(cli) ? "build the release CLI first" : false;

/** An 8 × 8 PNG of one opaque colour. */
function png(r, g, b) {
  const crc = (buf) => {
    let c = ~0;
    for (const x of buf) { c ^= x; for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1)); }
    return ~c >>> 0;
  };
  const chunk = (type, data) => {
    const t = Buffer.concat([Buffer.from(type), data]);
    const out = Buffer.alloc(8 + data.length + 4);
    out.writeUInt32BE(data.length, 0); t.copy(out, 4); out.writeUInt32BE(crc(t), 8 + data.length);
    return out;
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(8, 0); ihdr.writeUInt32BE(8, 4); ihdr[8] = 8; ihdr[9] = 2;
  const rows = Buffer.concat(Array.from({ length: 8 }, () => Buffer.from([0, ...Array.from({ length: 8 }, () => [r, g, b]).flat()])));
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(rows)), chunk("IEND", Buffer.alloc(0))]);
}

const doc = {
  datars: 1, title: "Pictures", size: { width: 300, height: 120 },
  scene: { kind: "group", key: "root", children: [
    { kind: "shape", key: "bg", geom: { type: "rect", x: 0, y: 0, w: 300, h: 120 }, fill: "#ffffff" },
    { kind: "icon", key: "heart", icon: "heart", at: [40, 60], icon_size: 48, ink: "#0000ff" },
    { kind: "image", key: "png", asset: "pic.png", rect: [90, 20, 80, 80], fit: "fill" },
    { kind: "image", key: "jpg", asset: "pic.jpg", rect: [190, 20, 80, 80], fit: "fill" },
  ] },
};

const page = `<!doctype html><meta charset="utf-8"><body style="margin:0"><script type="module" src="/runtime/datars.js"></script>
<datars-view id="raw" cpu doc="/doc.json" style="display:block;width:300px"></datars-view>
<datars-view id="bundle" cpu src="/site/c/pics" style="display:block;width:300px"></datars-view>`;

const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".ttf": "font/ttf", ".png": "image/png", ".jpg": "image/jpeg" };

test("raw documents fetch default icons one compact file each; browsers decode pictures, before a bundle's first frame", { skip, timeout: 180_000 }, async () => {
  const work = mkdtempSync(join(tmpdir(), "datars-pics-"));
  const files = { "/pic.png": png(220, 20, 30), "/pic.jpg": readFileSync(join(root, "crates/datars-render/tests/grey8.jpg")) };
  writeFileSync(join(work, "doc.json"), JSON.stringify(doc));
  writeFileSync(join(work, "pic.png"), files["/pic.png"]);
  writeFileSync(join(work, "pic.jpg"), files["/pic.jpg"]);
  execFileSync(cli, ["publish", join(work, "doc.json"), "--alias", "pics", "--to", join(work, "site")], { stdio: "ignore" });
  const hits = [];
  const missing = [];
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url, "http://x").pathname);
    hits.push(path);
    const send = (body, type) => { res.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
    if (path === "/") return send(page, "text/html; charset=utf-8");
    if (path === "/doc.json") return send(JSON.stringify(doc), "application/json");
    if (files[path]) return send(files[path], types[extname(path)]);
    if (path.startsWith("/site/")) {
      const f = normalize(join(work, path));
      if (f.startsWith(work) && existsSync(f)) return send(readFileSync(f), "application/octet-stream");
    }
    const file = normalize(join(dist, path.replace(/^\/runtime\//, "")));
    if (path.startsWith("/runtime/") && file.startsWith(dist) && existsSync(file)) return send(readFileSync(file), types[extname(file)] ?? "application/octet-stream");
    missing.push(path);
    res.writeHead(404).end();
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { chromium } = await import(playwright);
  const browser = await chromium.launch({ channel: process.env.DATARS_BROWSER_CHANNEL ?? "chrome", headless: true });
  try {
    const p = await browser.newPage({ deviceScaleFactor: 1 });
    const errors = [];
    p.on("pageerror", (e) => errors.push(e.message));
    p.on("console", (m) => { if ((m.type() === "error" || m.type() === "warning") && !/404/.test(m.text())) errors.push(m.text()); });
    // Every frame each chart draws: the colour at (x, y), so a picture that popped in after the
    // first frame would show as a frame without it.
    await p.addInitScript(() => {
      window.frames_ = { raw: [], bundle: [] };
      const draw = CanvasRenderingContext2D.prototype.putImageData;
      CanvasRenderingContext2D.prototype.putImageData = function (img, ...rest) {
        draw.call(this, img, ...rest);
        const id = this.canvas.getRootNode()?.host?.id;
        if (!id || !window.frames_[id]) return;
        const at = (x, y) => Array.from(img.data.slice((y * img.width + x) * 4, (y * img.width + x) * 4 + 3));
        // The heart is an outline: how many of its box's pixels are its blue.
        let heart = 0;
        for (let y = 36; y < 84; y++) for (let x = 16; x < 64; x++) { const [r, g, b] = at(x, y); heart += b > 150 && r < 120 && g < 120; }
        window.frames_[id].push({ png: at(130, 60), jpg: at(230, 60), heart });
      };
    });
    await p.goto(`http://127.0.0.1:${server.address().port}/`);
    const last = (id) => p.evaluate((id) => window.frames_[id].at(-1) ?? null, id);
    await p.waitForFunction(() => ["raw", "bundle"].every((id) => { const f = window.frames_[id].at(-1); return f && f.png[0] > 150 && f.jpg[0] > 100 && f.jpg[0] < 160; }), null, { timeout: 60_000 });
    for (const id of ["raw", "bundle"]) {
      const f = await last(id);
      assert.ok(f.png[0] > 200 && f.png[1] < 40 && f.png[2] < 50, `${id}: the PNG drawn as stored: ${f.png}`);
      assert.ok(Math.abs(f.jpg[0] - 128) <= 3 && Math.abs(f.jpg[1] - 128) <= 3, `${id}: the JPEG decoded by the browser: ${f.jpg}`);
    }
    // The bundle carries its picture: decoded before its first frame, so every frame has it.
    const bundle = await p.evaluate(() => window.frames_.bundle);
    assert.ok(bundle.length > 0 && bundle.every((f) => f.png[0] > 200), `every bundle frame has its picture: ${JSON.stringify(bundle.map((f) => f.png))}`);
    // The raw document's icon: its compact file beside the runtime, never SVG or the whole set.
    await p.waitForFunction(() => window.frames_.raw.at(-1)?.heart > 40, null, { timeout: 30_000 });
    assert.ok(hits.includes("/runtime/icons/lucide/heart.json"), hits.join(", "));
    assert.ok(!hits.some((h) => h.endsWith(".svg") || h.endsWith("icons.json")), hits.join(", "));
    assert.ok(!hits.some((h) => h.startsWith("/site/") && h.includes("heart")), "a bundle asks for no icon");
    assert.deepEqual(errors, []);
    assert.deepEqual(missing.filter((m) => m !== "/favicon.ico"), [], "nothing asked for that isn't there");
  } finally {
    await browser.close();
    server.close();
    rmSync(work, { recursive: true, force: true });
  }
});
