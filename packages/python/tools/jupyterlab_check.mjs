// End to end in a real JupyterLab: open a notebook, run every cell, then check that each chart
// went live (a <datars-view> with a renderer — widgets get the runtime over the kernel
// connection), click the first chart and ask the kernel what its click handler received.
// A dev tool (tools/check_browser.py covers the pieces without a server):
//
//   jupyter lab --no-browser --port 8993 --ServerApp.token= --LabApp.expose_app_in_browser=True &
//   node tools/jupyterlab_check.mjs http://127.0.0.1:8993/lab/tree/examples/out/check.ipynb out.png [waitMs]
//
// The notebook's first chart must be a widget bound to `w`, with `picked` collecting clicks.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url, out, waitArg = "45000"] = process.argv.slice(2);
const chrome = process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const profile = mkdtempSync(join(tmpdir(), "datars-chrome-"));
const proc = spawn(chrome, ["--headless=new", "--remote-debugging-pipe", "--no-first-run", "--no-default-browser-check",
  "--enable-unsafe-webgpu", `--user-data-dir=${profile}`, "--window-size=1200,1400", "about:blank"],
  { stdio: ["ignore", "ignore", "ignore", "pipe", "pipe"] });
const [, , , toChrome, fromChrome] = proc.stdio;
let id = 0, session, buf = "";
const pending = new Map();
const logs = [];
fromChrome.on("data", (chunk) => {
  buf += chunk.toString();
  let end;
  while ((end = buf.indexOf("\0")) >= 0) {
    const msg = JSON.parse(buf.slice(0, end));
    buf = buf.slice(end + 1);
    if (msg.id && pending.has(msg.id)) { pending.get(msg.id)(msg.result ?? { error: msg.error }); pending.delete(msg.id); }
    if (msg.method === "Runtime.exceptionThrown") logs.push(`exception: ${msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text}`);
    if (msg.method === "Runtime.consoleAPICalled" && ["error", "warning"].includes(msg.params.type)) {
      const text = msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ");
      if (/datars|widget|anywidget/i.test(text)) logs.push(`console.${msg.params.type}: ${text.slice(0, 300)}`);
    }
  }
});
const send = (method, params = {}, sid = session) => new Promise((resolve) => {
  const i = ++id;
  pending.set(i, resolve);
  toChrome.write(JSON.stringify({ id: i, method, params, ...(sid ? { sessionId: sid } : {}) }) + "\0");
});
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  return r.exceptionDetails ? { error: r.exceptionDetails.exception?.description ?? r.exceptionDetails.text } : r.result?.value;
};
const { targetId } = await send("Target.createTarget", { url: "about:blank" }, null);
({ sessionId: session } = await send("Target.attachToTarget", { targetId, flatten: true }, null));
await send("Runtime.enable");
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width: 1200, height: 1400, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url });
// The app, the notebook and an idle kernel.
let ready = false;
for (let i = 0; i < 120 && !ready; i++) {
  await sleep(500);
  ready = await evaluate(`(() => { const a = window.jupyterapp; const w = a && a.shell.currentWidget; return !!(w && w.sessionContext && w.sessionContext.session && w.sessionContext.session.kernel && w.sessionContext.session.kernel.status === "idle"); })()`);
}
const report = { ready };
report.run = await evaluate(`window.jupyterapp.commands.execute("notebook:run-all-cells").then(() => "started", (e) => String(e))`);
await sleep(Number(waitArg));
report.views = await evaluate(`[...document.querySelectorAll("datars-view")].map((v) => v.dataset.renderer || null)`);
report.widgets = await evaluate(`document.querySelectorAll(".datars-widget").length`);
report.widgetErrors = await evaluate(`[...document.querySelectorAll(".jp-OutputArea-output")].map((o) => o.innerText).filter((t) => /Error displaying widget|Traceback|Error:/.test(t)).map((t) => t.slice(0, 300))`);
// Click the first chart's first bar, then ask the kernel what `picked` got.
const box = await evaluate(`(() => { const v = document.querySelector("datars-view"); v.scrollIntoView({ block: "center" }); const r = v.getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; })()`);
await sleep(800);
const box2 = await evaluate(`(() => { const r = document.querySelector("datars-view").getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; })()`);
if (Array.isArray(box2)) {
  const [x, y, w, h] = box2;
  const px = x + w * 0.1, py = y + h * 0.6;
  for (const type of ["mouseMoved", "mousePressed", "mouseReleased"]) {
    await send("Input.dispatchMouseEvent", { type, x: px, y: py, button: "left", buttons: type === "mousePressed" ? 1 : 0, clickCount: 1 });
  }
  report.clicked = [Math.round(px), Math.round(py)];
}
await sleep(1500);
report.picked = await evaluate(`new Promise((resolve) => {
  const k = window.jupyterapp.shell.currentWidget.sessionContext.session.kernel;
  const f = k.requestExecute({ code: "print(picked)" });
  let text = "";
  f.onIOPub = (m) => { if (m.header.msg_type === "stream") text += m.content.text; };
  f.done.then(() => resolve(text.trim()));
})`);
report.state = await evaluate(`new Promise((resolve) => {
  const k = window.jupyterapp.shell.currentWidget.sessionContext.session.kernel;
  const f = k.requestExecute({ code: "w.state = w.states[-1]; import time; print(w.states)" });
  let text = "";
  f.onIOPub = (m) => { if (m.header.msg_type === "stream") text += m.content.text; };
  f.done.then(() => resolve(text.trim()));
})`);
await sleep(2500);
const shot = await send("Page.captureScreenshot", { format: "png" });
if (out) writeFileSync(out, Buffer.from(shot.data, "base64"));
report.logs = logs;
console.log(JSON.stringify(report, null, 1));
proc.kill();
try { rmSync(profile, { recursive: true, force: true }); } catch {}
process.exit(0);
