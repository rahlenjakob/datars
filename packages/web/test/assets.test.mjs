// A page that supplies the runtime's files itself — every one a blob: URL, as a notebook widget
// does with a runtime that arrived over its kernel connection — draws a raw document: the script
// has no folder to find its engine, wasm or fonts in, so everything comes from `runtimeAssets`.
// And a click on a bar dispatches `pick` with the bar's data row. In Chrome (CHROME, or the macOS
// default), after the web build; skipped otherwise.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const dist = join(root, "packages/web/dist");
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const built = existsSync(join(dist, "datars.js")) && existsSync(join(dist, "wasm/datars_host_web_bg.wasm"));
const skip = !built ? "build the web runtime first" : !existsSync(chrome) ? "no Chrome (set CHROME)" : false;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const PAGE = `<!doctype html><meta charset="utf-8"><body style="margin:0">
<script type="module">
const get = (p) => fetch("/dist/" + p).then((r) => r.arrayBuffer());
const blob = (b, type) => URL.createObjectURL(new Blob([b], { type }));
// The engine's glue imports its WASI shim by a relative path: point it at the shim's blob.
const shim = blob(await get("wasm/wasi_shim.js"), "text/javascript");
const glue = new TextDecoder().decode(await get("wasm/datars_host_web.js")).replaceAll('"./wasi_shim.js"', JSON.stringify(shim));
const mod = await import(blob(await get("datars.js"), "text/javascript"));
const assets = customElements.get("datars-view").assets;
window.__sameMap = assets === mod.runtimeAssets;
assets.set("wasm/datars_host_web.js", blob(glue, "text/javascript"));
assets.set("wasm/datars_host_web_bg.wasm", blob(await get("wasm/datars_host_web_bg.wasm"), "application/wasm"));
for (const f of ["Inter-Regular.ttf", "Inter-SemiBold.ttf", "Inter-Bold.ttf"]) assets.set("fonts/" + f, blob(await get("fonts/" + f), "font/ttf"));
const v = document.createElement("datars-view");
v.setAttribute("height", "440");
v.style.cssText = "display:block;width:720px";
v.addEventListener("pick", (e) => { window.__pick = e.detail; });
v.setDocument(await (await fetch("/votes.json")).text());
document.body.append(v);
</script>`;

test("a page's own runtime files: blob: engine, wasm and fonts draw a raw document; a click picks the bar's row", { skip, timeout: 120000 }, async () => {
  const server = createServer((req, res) => {
    if (req.url === "/") return res.writeHead(200, { "content-type": "text/html" }).end(PAGE);
    if (req.url === "/votes.json") return res.writeHead(200, { "content-type": "application/json" }).end(readFileSync(join(root, "examples/votes/doc.json")));
    const path = normalize(join(dist, decodeURIComponent(req.url.replace(/^\/dist\//, ""))));
    if (!req.url.startsWith("/dist/") || !path.startsWith(dist) || !existsSync(path)) return res.writeHead(404).end();
    res.writeHead(200).end(readFileSync(path));
  });
  await new Promise((r) => server.listen(0, "127.0.0.1", r));
  const requested = [];
  server.on("request", (req) => requested.push(req.url));
  const profile = mkdtempSync(join(tmpdir(), "datars-chrome-"));
  const proc = spawn(chrome, ["--headless=new", "--remote-debugging-pipe", "--no-first-run", "--no-default-browser-check", "--enable-unsafe-webgpu", `--user-data-dir=${profile}`, "about:blank"], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
  const [, , , toChrome, fromChrome] = proc.stdio;
  let id = 0, session, buf = "";
  const pending = new Map();
  const errors = [];
  fromChrome.on("data", (chunk) => {
    buf += chunk.toString();
    let end;
    while ((end = buf.indexOf("\0")) >= 0) {
      const msg = JSON.parse(buf.slice(0, end));
      buf = buf.slice(end + 1);
      if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result ?? msg.error); pending.delete(msg.id); }
      if (msg.method === "Runtime.exceptionThrown") errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text);
    }
  });
  const send = (method, params = {}, sid = session) => new Promise((resolve) => {
    const i = ++id;
    pending.set(i, resolve);
    toChrome.write(JSON.stringify({ id: i, method, params, ...(sid ? { sessionId: sid } : {}) }) + "\0");
  });
  const evaluate = async (expression) => (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true })).result?.value;
  try {
    const { targetId } = await send("Target.createTarget", { url: "about:blank" }, null);
    ({ sessionId: session } = await send("Target.attachToTarget", { targetId, flatten: true }, null));
    await send("Runtime.enable");
    await send("Emulation.setDeviceMetricsOverride", { width: 900, height: 600, deviceScaleFactor: 1, mobile: false });
    await send("Page.navigate", { url: `http://127.0.0.1:${server.address().port}/` });
    let renderer = null;
    for (let t = 0; t < 30000 && !renderer; t += 200) {
      await sleep(200);
      renderer = await evaluate(`document.querySelector("datars-view")?.dataset.renderer ?? null`);
    }
    assert.ok(renderer, `the view rendered (errors: ${errors.join("; ")})`);
    assert.equal(await evaluate("window.__sameMap"), true, "the element exposes the module's asset map");
    // Only the page's own fetches reached the server: nothing was looked up next to the script.
    assert.ok(requested.every((u) => u === "/" || u === "/votes.json" || u === "/favicon.ico" || u.startsWith("/dist/")), requested.join(", "));
    await sleep(1500); // bars have grown in
    // The tallest bar (Social Democrats) sits left in the plot: a real click.
    for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
      await send("Input.dispatchMouseEvent", { type, x: 80, y: 300, button: "left", buttons: type === "mousePressed" ? 1 : 0, clickCount: 1 });
    }
    await sleep(300);
    const pick = await evaluate("window.__pick ?? null");
    assert.ok(pick, "a click dispatches pick");
    assert.ok(pick.hits.some((h) => h.role === "datum" && h.path.includes('("S",)')), JSON.stringify(pick.hits));
    assert.equal(pick.row?.fields?.party, "S", JSON.stringify(pick.row));
    assert.deepEqual(errors, []);
  } finally {
    proc.kill();
    server.close();
    try { rmSync(profile, { recursive: true, force: true }); } catch {}
  }
});
