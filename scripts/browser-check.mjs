// Open a page in headless Chrome over the DevTools protocol, wait, and report console errors and
// exceptions; save a screenshot. Used to check <datars-view> in a real browser:
//   node --experimental-websocket scripts/browser-check.mjs http://127.0.0.1:8787/ out/browser.png [waitMs] [script]
import { spawn } from "node:child_process";
import { writeFileSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url, out = "out/browser.png", wait = "6000", script = ""] = process.argv.slice(2);
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const port = 9300 + Math.floor(Math.random() * 500);
const proc = spawn(chrome, ["--headless=new", `--remote-debugging-port=${port}`, "--no-first-run", "--no-default-browser-check", "--enable-unsafe-webgpu", `--user-data-dir=${mkdtempSync(join(tmpdir(), "datars-chrome-"))}`, "--window-size=900,1600", "about:blank"], { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
for (let i = 0; i < 50 && !targets; i++) {
  await sleep(200);
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json(); } catch {}
}
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r));
let id = 0;
const pending = new Map();
const logs = [];
ws.addEventListener("message", (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result ?? msg.error); pending.delete(msg.id); }
  if (msg.method === "Runtime.consoleAPICalled") logs.push(`console.${msg.params.type}: ${msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ")}`);
  if (msg.method === "Runtime.exceptionThrown") logs.push(`exception: ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
  if (msg.method === "Log.entryAdded" && msg.params.entry.level !== "verbose") logs.push(`log.${msg.params.entry.level}: ${msg.params.entry.text} ${msg.params.entry.url ?? ""}`);
});
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
await send("Runtime.enable");
await send("Log.enable");
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 900, height: 1600, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url });
await sleep(Number(wait));
if (script) {
  // e.g. "scrollTo(0, document.body.scrollHeight * 0.45)" — then give the page a moment.
  await send("Runtime.evaluate", { expression: script });
  await sleep(1500);
}
const probe = await send("Runtime.evaluate", { expression: `JSON.stringify([...document.querySelectorAll("datars-view")].map(v => ({ src: v.getAttribute("src"), renderer: v.dataset.renderer ?? null, engine: v.dataset.engine ?? null, status: v.view?.status?.().tier ?? null })))`, returnByValue: true });
const shot = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(out, Buffer.from(shot.data, "base64"));
console.log(`views: ${probe.result?.value}`);
for (const l of logs) console.log(l);
console.log(`screenshot: ${out}`);
ws.close();
proc.kill();
process.exit(0);
