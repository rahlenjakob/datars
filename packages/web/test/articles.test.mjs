// The showcase's article pages in a real browser: every page loads with no console error or
// exception, every chart on it renders (its <datars-view> gets a renderer), and a chart taken off
// the page far from the screen comes back when the reader scrolls back to it. Needs the built site
// (node scripts/build-site.mjs), the CLI (to serve it) and Chrome (CHROME, or the macOS default);
// skipped otherwise.
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const site = join(root, "out/pages");
const cli = join(root, "target/release/datars");
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const articles = existsSync(join(site, "articles")) ? readdirSync(join(site, "articles")).filter((a) => existsSync(join(site, "articles", a, "index.html"))).sort() : [];
const skip = !articles.length ? "build the site first (node scripts/build-site.mjs)" : !existsSync(cli) ? "build the CLI first" : !existsSync(chrome) ? "no Chrome (set CHROME)" : false;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** A headless Chrome tab driven over the DevTools protocol on a pipe (no port, no WebSocket:
 * messages are NUL-terminated JSON on fds 3 and 4); its profile is removed on close. */
async function browser() {
  const profile = mkdtempSync(join(tmpdir(), "datars-chrome-"));
  const proc = spawn(chrome, ["--headless=new", "--remote-debugging-pipe", "--no-first-run", "--no-default-browser-check", "--enable-unsafe-webgpu", `--user-data-dir=${profile}`, "--window-size=1280,900", "about:blank"], { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
  const [, , , toChrome, fromChrome] = proc.stdio;
  let id = 0;
  let session;
  const pending = new Map();
  const errors = [];
  let buf = "";
  fromChrome.on("data", (chunk) => {
    buf += chunk.toString();
    let end;
    while ((end = buf.indexOf("\0")) >= 0) {
      const msg = JSON.parse(buf.slice(0, end));
      buf = buf.slice(end + 1);
      if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result ?? msg.error); pending.delete(msg.id); }
      if (msg.method === "Runtime.exceptionThrown") errors.push(`exception: ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
      if (msg.method === "Runtime.consoleAPICalled" && msg.params.type === "error") errors.push(`console.error: ${msg.params.args.map((a) => a.value ?? a.description).join(" ")}`);
      if (msg.method === "Log.entryAdded" && msg.params.entry.level === "error") errors.push(`${msg.params.entry.text} ${msg.params.entry.url ?? ""}`);
    }
  });
  const call = (method, params = {}, sessionId = session) => new Promise((r) => {
    const i = ++id;
    pending.set(i, r);
    toChrome.write(`${JSON.stringify({ id: i, method, params, ...(sessionId ? { sessionId } : {}) })}\0`);
  });
  let page;
  for (let i = 0; i < 100 && !page; i++) {
    page = (await call("Target.getTargets", {}, null)).targetInfos?.find((t) => t.type === "page");
    if (!page) await sleep(100);
  }
  session = (await call("Target.attachToTarget", { targetId: page.targetId, flatten: true }, null)).sessionId;
  const send = (method, params = {}) => call(method, params);
  await send("Runtime.enable");
  await send("Log.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 900, deviceScaleFactor: 1, mobile: false });
  // A reader's browser, not "HeadlessChrome": public APIs a live chart reads (the Treasury's, in
  // us-debt) turn headless agents away, and their refusal would read as the page's error.
  await send("Emulation.setUserAgentOverride", { userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result?.value;
  };
  const close = async () => {
    const exited = new Promise((r) => proc.once("exit", r));
    proc.kill();
    await exited;
    rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  };
  return { send, evaluate, errors, close };
}

/** Scroll to chart `i` and wait for its view to render; its renderer, or null on timeout. */
const RENDER = (i, ms) => `(async () => {
  const slot = document.querySelectorAll(".chart[data-src]")[${i}];
  slot.scrollIntoView({ block: "center" });
  for (let t = 0; t < ${ms}; t += 100) {
    const v = slot.querySelector("datars-view");
    if (v?.dataset.renderer) return v.dataset.renderer;
    await new Promise((r) => setTimeout(r, 100));
  }
  return null;
})()`;

test("every article page loads cleanly and every chart on it renders", { skip, timeout: 30 * 60 * 1000 }, async (t) => {
  const port = 20000 + Math.floor(Math.random() * 20000);
  const server = spawn(cli, ["serve", site, "--port", String(port)], { stdio: ["ignore", "pipe", "inherit"] });
  await new Promise((r) => server.stdout.once("data", r));
  const b = await browser();
  try {
    await b.send("Page.navigate", { url: `http://127.0.0.1:${port}/articles/` });
    await sleep(1000);
    assert.equal(await b.evaluate(`document.querySelectorAll(".card-link").length`), articles.length, "a card per article");
    for (const a of articles) {
      b.errors.length = 0;
      await b.send("Page.navigate", { url: `http://127.0.0.1:${port}/articles/${a}/` });
      await sleep(1500);
      const n = await b.evaluate(`document.querySelectorAll(".chart[data-src]").length`);
      assert.ok(n > 0, `${a}: charts on the page`);
      const missing = [];
      for (let i = 0; i < n; i++) {
        const renderer = await b.evaluate(RENDER(i, 30000));
        if (!renderer) missing.push(await b.evaluate(`document.querySelectorAll(".chart[data-src]")[${i}].dataset.src`));
      }
      // A stepped figure's "Next" moves one step (not two), and its numbers follow.
      const stepped = await b.evaluate(`(async () => {
        const bar = document.querySelector("figure .stepper");
        if (!bar) return null;
        const fig = bar.closest("figure");
        fig.scrollIntoView({ block: "center" });
        const ready = () => fig.querySelector("datars-view")?.dataset.renderer && bar.querySelector(".next");
        for (let t = 0; t < 20000 && !ready(); t += 100) await new Promise((r) => setTimeout(r, 100));
        const pressed = () => [...bar.querySelectorAll("button:not(.next)")].findIndex((x) => x.getAttribute("aria-pressed") === "true");
        const before = pressed();
        bar.querySelector(".next").click();
        await new Promise((r) => setTimeout(r, 400));
        return [before, pressed()];
      })()`);
      if (stepped) assert.deepEqual(stepped, [0, 1], `${a}: Next moves one step`);
      // Far away, the first chart is taken off the page (its engine freed); back, it renders again.
      await b.evaluate(`scrollTo(0, document.documentElement.scrollHeight)`);
      await sleep(800);
      const far = await b.evaluate(`document.documentElement.scrollHeight > innerHeight * 12 ? !document.querySelector(".chart[data-src]").querySelector("datars-view") : null`);
      if (far !== null) assert.ok(far, `${a}: the first chart is unmounted when far away`);
      const again = await b.evaluate(RENDER(0, 30000));
      t.diagnostic(`${a}: ${n} charts${far ? ", unmounted and remounted" : ""}`);
      assert.deepEqual(missing, [], `${a}: charts that never rendered`);
      assert.ok(again, `${a}: the first chart renders again after scrolling back`);
      assert.deepEqual(b.errors, [], `${a}: console errors`);
    }
  } finally {
    await b.close();
    server.kill();
  }
});
