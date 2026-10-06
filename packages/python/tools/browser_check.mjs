// Open a page in headless Chrome (DevTools protocol over a pipe), wait, optionally click and
// evaluate expressions, save a screenshot, and print a JSON report: every <datars-view>'s
// renderer and state, console errors, and the evaluated values. A dev tool for the Python
// package's live outputs (used by tools/check_browser.py):
//
//   node tools/browser_check.mjs <url> <out.png> [--wait 6000] [--click x,y] [--eval "expr"]
//
// Clicks are real mouse events (Input.dispatchMouseEvent) at page coordinates, run in order,
// each followed by a short pause; evals run after the clicks.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const args = process.argv.slice(2);
const [url, out] = args;
let wait = 6000;
const clicks = [];
const evals = [];
for (let i = 2; i < args.length; i++) {
  if (args[i] === "--wait") wait = Number(args[++i]);
  else if (args[i] === "--click") clicks.push(args[++i].split(",").map(Number));
  else if (args[i] === "--eval") evals.push(args[++i]);
}
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = mkdtempSync(join(tmpdir(), "datars-chrome-"));
const proc = spawn(chrome, ["--headless=new", "--remote-debugging-pipe", "--no-first-run", "--no-default-browser-check",
  "--enable-unsafe-webgpu", "--allow-file-access-from-files", `--user-data-dir=${profile}`, "--window-size=1000,900", "about:blank"],
  { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
const [, , , toChrome, fromChrome] = proc.stdio;
let id = 0;
let session;
const pending = new Map();
const logs = [];
let buf = "";
fromChrome.on("data", (chunk) => {
  buf += chunk.toString();
  let end;
  while ((end = buf.indexOf("\0")) >= 0) {
    const msg = JSON.parse(buf.slice(0, end));
    buf = buf.slice(end + 1);
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result ?? { error: msg.error }); pending.delete(msg.id); }
    if (msg.method === "Runtime.exceptionThrown") logs.push(`exception: ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
    if (msg.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(msg.params.type)) logs.push(`console.${msg.params.type}: ${msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ")}`);
  }
});
const send = (method, params = {}, sid = session) => new Promise((resolve) => {
  const i = ++id;
  pending.set(i, resolve);
  toChrome.write(JSON.stringify({ id: i, method, params, ...(sid ? { sessionId: sid } : {}) }) + "\0");
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const { targetId } = await send("Target.createTarget", { url: "about:blank" }, null);
({ sessionId: session } = await send("Target.attachToTarget", { targetId, flatten: true }, null));
await send("Runtime.enable");
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 1000, height: 900, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url });
await sleep(wait);
for (const [x, y] of clicks) {
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    await send("Input.dispatchMouseEvent", { type, x, y, button: "left", buttons: type === "mousePressed" ? 1 : 0, clickCount: 1 });
  }
  await sleep(800);
}
const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  return r.exceptionDetails ? { error: r.exceptionDetails.exception?.description ?? r.exceptionDetails.text } : r.result?.value;
};
const views = await evaluate(`JSON.stringify([...document.querySelectorAll("datars-view")].map(v => ({ renderer: v.dataset.renderer ?? null, engine: v.dataset.engine ?? null, poster: !!document.querySelector(".datars-poster") })))`);
const results = [];
for (const e of evals) results.push(await evaluate(e));
const shot = await send("Page.captureScreenshot", { format: "png" });
if (out) writeFileSync(out, Buffer.from(shot.data, "base64"));
console.log(JSON.stringify({ url, views: JSON.parse(views ?? "[]"), evals: results, logs }, null, 1));
proc.kill();
try { rmSync(profile, { recursive: true, force: true }); } catch {}
process.exit(0);
