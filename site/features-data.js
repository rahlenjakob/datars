// The data page (site/pages/features/data.html): the data lab, the misbehaving API, and the
// spending chart's accounts.
//
// The lab hands the reader's rows to a published chart's data slot with `provideData` (the
// engine parses and checks them; the page only renames the two header columns the reader picks
// to the slot's), the live chart's requests are answered by a simulated API through the
// `datarequest` event, and the spending chart takes other accounts' rows — all through
// <datars-view>'s public API.
import { highlight } from "./site.js";

/** The slot's <datars-view> once its engine is running. */
function whenReady(slot) {
  return new Promise((resolve) => {
    const look = () => {
      const v = slot.querySelector("datars-view");
      if (!v) return false;
      if (v.status) resolve(v);
      else v.addEventListener("state", () => resolve(v), { once: true });
      return true;
    };
    if (look()) return;
    const mo = new MutationObserver(() => { if (look()) mo.disconnect(); });
    mo.observe(slot, { childList: true });
  });
}
const put = (el, t) => { if (el && el.textContent !== t) el.textContent = t; };
const show = (el, text) => { if (el.textContent !== text) { el.textContent = text; highlight(el); } };

// ---- the data lab ------------------------------------------------------------------------------

/** The header line of CSV text: its delimiter (the one the line has most of) and its names. */
function header(text) {
  const line = text.split(/\r?\n/, 1)[0] ?? "";
  const delim = [";", "\t", ","].map((d) => [d, line.split(d).length]).sort((a, b) => b[1] - a[1])[0][0];
  const names = [];
  let cur = "", quoted = false;
  for (const c of line) {
    if (c === '"') quoted = !quoted;
    else if (c === delim && !quoted) { names.push(cur.trim()); cur = ""; }
    else cur += c;
  }
  names.push(cur.trim());
  return { delim, names, rest: text.slice(line.length) };
}
const isJson = (t) => /^\s*[[{]/.test(t);
/** Records from JSON text: an array of records, an object of columns, or `{ data | rows | results: [...] }`. */
function jsonRecords(text) {
  let v = JSON.parse(text);
  if (!Array.isArray(v) && v && typeof v === "object") {
    const inner = ["data", "rows", "results", "items"].map((k) => v[k]).find(Array.isArray);
    if (inner) v = inner;
    else {
      const cols = Object.entries(v).filter(([, c]) => Array.isArray(c));
      const n = Math.max(0, ...cols.map(([, c]) => c.length));
      v = Array.from({ length: n }, (_, i) => Object.fromEntries(cols.map(([k, c]) => [k, c[i]])));
    }
  }
  if (!Array.isArray(v)) throw new Error("JSON: expected an array of records");
  return v;
}
/** A first guess at the label and value columns from a few rows: text for the label, numbers for the value. */
function guess(names, sample) {
  const numeric = (col) => sample.length > 0 && sample.every((r) => r[col] == null || r[col] === "" || /^(NA|\.\.|-)$/.test(String(r[col])) || /^-?[\d\s]*[.,]?\d+$/.test(String(r[col]).trim()));
  const label = names.find((n) => !numeric(n)) ?? names[0];
  const value = [...names].reverse().find((n) => n !== label && numeric(n)) ?? names.find((n) => n !== label) ?? names[0];
  return { label, value };
}
function csvSample(text, n = 6) {
  const { delim, names } = header(text);
  return text.split(/\r?\n/).slice(1, 1 + n).filter(Boolean).map((l) => Object.fromEntries(l.split(delim).map((v, i) => [names[i], v.replace(/^"|"$/g, "").trim()])));
}

function lab(section) {
  const slot = section.querySelector('.chart[data-chart="data-lab"]');
  const text = section.querySelector("#dx-text");
  const selLabel = section.querySelector("#dx-label");
  const selValue = section.querySelector("#dx-value");
  const verdict = section.querySelector(".dx-verdict");
  const codeEl = section.querySelector("#dx-code");
  const samples = Object.fromEntries([...section.querySelectorAll('[id^="dx-src-"]')].map((el) => [el.id.slice(7), (el.querySelector("code") ?? el).textContent.replace(/\s+$/, "") + "\n"]));
  let sample = "capitals";
  let names = [];
  let view = null;
  let timer = 0;
  let sent = null; // what the chart was last handed, for the code
  let prefer = "population"; // the value column a sample is best shown by

  function setSample(id) {
    sample = id;
    prefer = section.querySelector(`.dx-samples [data-sample="${id}"]`)?.dataset.value ?? null;
    for (const b of section.querySelectorAll(".dx-samples button")) b.setAttribute("aria-pressed", String(b.dataset.sample === id));
    for (const p of section.querySelectorAll(".dx-profile")) p.hidden = p.dataset.for !== id;
    if (samples[id]) text.value = samples[id];
    columns(true);
    send();
  }
  /** The columns on offer, from the header (CSV) or the records (JSON). */
  function columns(fresh) {
    let rows = [];
    try {
      if (isJson(text.value)) { rows = jsonRecords(text.value).slice(0, 6); names = [...new Set(rows.flatMap((r) => Object.keys(r)))]; }
      else { names = header(text.value).names.filter(Boolean); rows = csvSample(text.value); }
    } catch { names = []; }
    const keep = { label: selLabel.value, value: selValue.value };
    const g = guess(names, rows);
    if (fresh && prefer && names.includes(prefer)) g.value = prefer;
    for (const [sel, pick] of [[selLabel, fresh || !names.includes(keep.label) ? g.label : keep.label], [selValue, fresh || !names.includes(keep.value) ? g.value : keep.value]]) {
      sel.replaceChildren(...names.map((n) => new Option(n, n, false, n === pick)));
      sel.value = pick ?? "";
    }
  }
  /** The rows with the two picked columns renamed to the slot's `label` and `value` (other
   * columns of those names step aside) — header only; the engine reads the rest. */
  function renamed() {
    const l = selLabel.value, v = selValue.value;
    const rename = (n) => (n === l ? "label" : n === v ? "value" : n === "label" || n === "value" ? `${n}_` : n);
    if (isJson(text.value)) {
      const recs = jsonRecords(text.value).map((r) => Object.fromEntries(Object.entries(r).map(([k, x]) => [rename(k), x])));
      return { kind: "json", body: JSON.stringify(recs) };
    }
    const { delim, names: all, rest } = header(text.value);
    const head = all.map((n) => { const m = rename(n); return m.includes(delim) ? `"${m}"` : m; }).join(delim);
    return { kind: "csv", body: head + rest, delim };
  }
  function send() {
    clearTimeout(timer);
    if (!view) return;
    let r;
    try { r = renamed(); } catch (e) { return void put(verdict, `Not rows the page can read: ${e.message}`); }
    try {
      view.provideData("rows", r.body);
      view.setSignal("heading", `${selValue.value || "value"} by ${selLabel.value || "label"}`);
      sent = r;
      put(verdict, selLabel.value === selValue.value ? "Pick two different columns." : `Handed in: the engine read the rows and keyed them by ${selLabel.value}.`);
      verdict.classList.remove("bad");
    } catch (e) {
      // The engine refused the rows (missing the slot's key or columns): it says why, and the chart keeps what it showed.
      put(verdict, `Refused by the engine: ${String(e?.message ?? e)}`);
      verdict.classList.add("bad");
    }
    code();
  }
  function code() {
    const body = sent?.body ?? "";
    const lines = body.split("\n").filter(Boolean);
    const shown = lines.slice(0, 4).map((l) => l.replace(/\t/g, "\\t"));
    const more = lines.length - shown.length;
    const literal = sent?.kind === "json"
      ? `const rows = ${body.length > 160 ? body.slice(0, 157) + "…" : body};`
      : `const csv = \`${shown.join("\n")}${more > 0 ? `\n… ${more} more line${more > 1 ? "s" : ""}` : ""}\`;`;
    show(codeEl, [
      `// "${selLabel.value}" → label, "${selValue.value}" → value: the slot's columns`,
      literal,
      "",
      `view.provideData("rows", ${sent?.kind === "json" ? "rows" : "csv"});`,
      `view.setSignal("heading", ${JSON.stringify(`${selValue.value} by ${selLabel.value}`)});`,
    ].join("\n"));
  }

  section.querySelector(".dx-samples").addEventListener("click", (e) => {
    const b = e.target.closest("button[data-sample]");
    if (b) setSample(b.dataset.sample);
  });
  text.addEventListener("input", () => {
    if (sample !== "own" && text.value !== samples[sample]) {
      // Edited: still that sample's columns, but no longer the file the CLI profiled.
      for (const b of section.querySelectorAll(".dx-samples button")) b.setAttribute("aria-pressed", "false");
      for (const p of section.querySelectorAll(".dx-profile")) p.hidden = p.dataset.for !== "own";
      sample = "own";
    }
    const before = names.join("\u0000");
    columns(false);
    const fresh = names.join("\u0000") !== before;
    if (fresh) columns(true);
    clearTimeout(timer);
    timer = setTimeout(send, 300);
  });
  for (const sel of [selLabel, selValue]) sel.addEventListener("change", send);
  // Drop a file: read here, in the page; nothing is uploaded.
  const drop = section.querySelector(".dx-drop");
  drop.addEventListener("dragover", (e) => { e.preventDefault(); drop.classList.add("over"); });
  drop.addEventListener("dragleave", () => drop.classList.remove("over"));
  drop.addEventListener("drop", async (e) => {
    e.preventDefault();
    drop.classList.remove("over");
    const f = e.dataTransfer?.files?.[0];
    if (!f) return;
    const bytes = new Uint8Array(await f.arrayBuffer());
    // UTF-8 if it is; else a Windows-1252 export (what spreadsheets on Windows write).
    let t;
    try { t = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch { t = new TextDecoder("windows-1252").decode(bytes); }
    text.value = t.slice(0, 200_000);
    text.dispatchEvent(new Event("input"));
  });

  text.value = samples.capitals ?? "";
  columns(true);
  code();
  whenReady(slot).then((v) => {
    view = v;
    // The chart opens on its own sample (the same capitals): nothing to hand in until the reader acts.
    sent = renamed();
    code();
  });
}

// ---- the misbehaving API -----------------------------------------------------------------------

const REGIONS = [["Stockholm", 148], ["Oslo", 121], ["Copenhagen", 109], ["Helsinki", 96], ["Göteborg", 84], ["Malmö", 61], ["Reykjavík", 38], ["Tampere", 52], ["Aarhus", 57], ["Bergen", 49]];

function api(section) {
  const slot = section.querySelector('.chart[data-chart="data-live"]');
  const latency = section.querySelector("#dx-latency");
  const fail = section.querySelector("#dx-fail");
  const log = section.querySelector(".dx-log");
  const codeEl = section.querySelector("#dx-api-code");
  const pauseBtn = section.querySelector("#dx-pause");
  // The "server": regions open now, each with orders a minute that wander. A seeded walk, so
  // every reader sees a similar evening.
  let open = REGIONS.slice(0, 6).map(([r, v]) => ({ region: r, orders: v }));
  let tick = 0, n = 0, paused = false, seed = 7;
  const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  function step() {
    tick += 1;
    for (const r of open) r.orders = Math.max(8, Math.min(240, Math.round(r.orders + (rand() - 0.48) * 34)));
  }
  const fmt = (ms) => (ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${ms} ms`);
  function settings() {
    put(section.querySelector('output[for="dx-latency"]'), fmt(Number(latency.value)));
    put(section.querySelector('output[for="dx-fail"]'), `${fail.value}%`);
    show(codeEl, [
      'view.addEventListener("datarequest", (e) => {',
      '  if (e.detail.name !== "orders") return;',
      "  e.preventDefault();",
      `  e.detail.respond(later(${Number(latency.value)}, () =>`,
      `    Math.random() < ${(Number(fail.value) / 100).toFixed(2)}`,
      '      ? Promise.reject(new Error("503"))',
      `      : ${paused ? "lastRows" : "server.rows()"}));`,
      "});",
      "// later(ms, f): a promise of f() after ms",
    ].join("\n"));
  }
  function entry(id, text, cls) {
    let li = log.querySelector(`[data-id="${id}"]`);
    if (!li) {
      li = document.createElement("li");
      li.dataset.id = id;
      log.prepend(li);
      while (log.children.length > 6) log.lastElementChild.remove();
    }
    li.className = cls;
    put(li, text);
  }
  let last = JSON.stringify(open.map((r) => ({ ...r, tick: 0 })));
  function answer(e) {
    if (e.detail.name !== "orders") return;
    e.preventDefault();
    const id = ++n;
    const t0 = performance.now();
    const wait = Number(latency.value);
    const failed = rand() * 100 < Number(fail.value);
    entry(id, `#${id} GET orders.json …`, "wait");
    e.detail.respond(new Promise((ok, no) => setTimeout(() => {
      const ms = Math.round(performance.now() - t0);
      if (failed) {
        entry(id, `#${id} 503 after ${fmt(ms)} — chart keeps its rows`, "bad");
        return no(new Error("503 Service Unavailable (simulated)"));
      }
      if (!paused) { step(); last = JSON.stringify(open.map((r) => ({ ...r, tick }))); }
      entry(id, `#${id} 200 after ${fmt(ms)} — ${open.length} rows${paused ? ", unchanged" : ""}`, "ok");
      ok(last);
    }, wait)));
  }
  // Listen before the chart's first request: `chartmount` fires before the view starts.
  slot.addEventListener("chartmount", (e) => e.detail.view.addEventListener("datarequest", answer));
  const mounted = slot.querySelector("datars-view");
  if (mounted) mounted.addEventListener("datarequest", answer);

  latency.addEventListener("input", settings);
  fail.addEventListener("input", settings);
  section.querySelector("#dx-open").addEventListener("click", () => {
    const next = REGIONS.find(([r]) => !open.some((o) => o.region === r));
    if (next) open.push({ region: next[0], orders: next[1] });
  });
  section.querySelector("#dx-close").addEventListener("click", () => {
    if (open.length > 2) open.splice(Math.floor(rand() * open.length), 1);
  });
  pauseBtn.addEventListener("click", () => {
    paused = !paused;
    pauseBtn.setAttribute("aria-pressed", String(paused));
    pauseBtn.textContent = paused ? "Resume the API" : "Pause the API";
    settings();
  });
  settings();
}

// ---- the spending chart's accounts ---------------------------------------------------------------

function accounts() {
  const pills = document.getElementById("account-pills");
  const status = document.getElementById("account-status");
  if (!pills) return;
  pills.addEventListener("click", async (e) => {
    const b = e.target.closest("button");
    const view = document.querySelector('.chart[data-chart="spending"] datars-view');
    if (!b || !view) return;
    if ("broken" in b.dataset) {
      // Rows without the slot's `amount` column: the engine refuses them and the chart stays put.
      try {
        view.provideData("spending", JSON.stringify([{ month: "Apr", category: "Food", spent: 412 }]));
        put(status, "Accepted.");
      } catch (err) {
        put(status, `Refused: ${String(err?.message ?? err)}. The chart keeps what it showed.`);
      }
      return;
    }
    pills.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String(x === b)));
    put(status, "");
    view.provideData("spending", await (await fetch(b.dataset.src)).text());
  });
}

const labSection = document.getElementById("lab");
if (labSection) lab(labSection);
const liveSection = document.getElementById("live");
if (liveSection) api(liveSection);
accounts();
