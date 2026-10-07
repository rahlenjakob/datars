// The interaction page (site/pages/features/interaction.html): the lab.
//
// The chart is one document (site/figures/interaction/lab.ts) with four interaction lines. The
// page only listens and asks, through <datars-view>'s public API:
// - `signals` and the `signal` event: the readout of what the reader's brush, clicks, drag and
//   camera wrote, and which line wrote it;
// - `pick`: the row a click or tap landed on;
// - `setSignal`: the page's own buttons;
// - `setDocument`: a switch takes its line out of the document (published at
//   /play/interaction-lab.json) and hands the chart the result.
// Nothing here draws in the chart or handles its pointer.
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

// ---- the document's lines -----------------------------------------------------------------------

/** Each switch: the line as TypeScript, on and off, and how to take it out of the document's JSON. */
const LINES = {
  brush: {
    on: 'plot({ data: "perDay", x: "day", y: "trips", brush: "days" }),',
    off: 'plot({ data: "perDay", x: "day", y: "trips" }),',
    sig: "days",
    strip: (doc) => { delete find(doc.scene, (n) => n.key === "days").params.brush; },
  },
  click: {
    on: 'bar({ selected: "kinds" }),',
    off: "bar(),",
    sig: "kinds",
    strip: (doc) => { delete find(doc.scene, (n) => n.recipe === "@datars/std/bar").params.selected; },
  },
  explore: {
    on: 'view({ camera: { fit, explore: "zoom", maxZoom: 12 } }),',
    off: "view({ camera: { fit } }),",
    sig: "zoom",
    strip: (doc) => { const c = find(doc.scene, (n) => n.key === "trips").camera; delete c.explore; delete c.max_zoom; delete c.min_zoom; },
  },
  drag: {
    on: 'group({ on: { drag: scrub("limit", { axis: "y", step: 1 }) } }),',
    off: "group({ /* the dashed line, nothing to drag */ }),",
    sig: "limit",
    strip: (doc) => { delete find(doc.scene, (n) => n.key === "limit").on; },
  },
};

/** The first node under `n` (a scene template) that `test` accepts. */
function find(n, test) {
  if (!n || typeof n !== "object") return null;
  if (n.kind && test(n)) return n;
  for (const v of Object.values(n)) {
    const kids = Array.isArray(v) ? v : v && typeof v === "object" ? [v] : [];
    for (const k of kids) {
      const hit = find(k, test);
      if (hit) return hit;
    }
  }
  return null;
}

/** The code panel's lines: [text, the signal it's about (or "")]. */
function codeLines(on) {
  const L = (k) => [on[k] ? LINES[k].on : LINES[k].off, on[k] ? LINES[k].sig : ""];
  return [
    ["signals: {", ""],
    ["  days: signal.range(),    // days.lo, days.hi, days.active", "days"],
    ["  kinds: signal.keyset(),  // the bikes picked", "kinds"],
    ["  limit: signal.num(30),   // minutes", "limit"],
    ["},", ""],
    ["tables: { shown: { from: \"trips\", ops: [", ""],
    ["  op.filter(brushed(\"days\", \"d.day\")),", "days"],
    ["  op.filter(e(\"kinds.isEmpty() || kinds.has(d.kind)\")),", "kinds"],
    ["] } },", ""],
    ["// the marks", ""],
    L("brush"),
    L("click"),
    L("explore"),
    L("drag"),
    ["instances({ from: \"shown\", opacity: e(\"d.min > limit ? 0.95 : 0.3\") }),", "limit"],
  ];
}

// ---- formatting --------------------------------------------------------------------------------

const r1 = (x) => (Number.isFinite(x) ? Math.round(x * 10) / 10 : "–");
const NAMES = { classic: "Classic", ebike: "E-bike", cargo: "Cargo" };
const VALUE = {
  days: (s) => (s["days.active"] && Number.isFinite(s["days.lo"]) ? `{ lo: ${r1(s["days.lo"])}, hi: ${r1(s["days.hi"])} }` : "not brushed"),
  kinds: (s) => JSON.stringify(s.kinds ?? []),
  zoom: (s) => (Number.isFinite(s["zoom.zoom"]) || Number.isFinite(s["zoom.x"]) ? `{ x: ${Math.round(s["zoom.x"] ?? 0)}, y: ${Math.round(s["zoom.y"] ?? 0)}, zoom: ${r1(s["zoom.zoom"] ?? 1)} }` : "fitted"),
  limit: (s) => String(s.limit ?? "–"),
  inspected: (s) => (s.inspected ? String(s.inspected) : "—"),
};
/** Which row a changed signal belongs to. */
const ROW = (name) => (name.startsWith("days") ? "days" : name.startsWith("zoom.") ? "zoom" : name === "kinds" || name === "limit" || name === "inspected" ? name : null);
const BY = { days: "brush", kinds: "activate → toggle", zoom: "pan, zoom", limit: "drag → scrub", inspected: "inspect" };

// ---- the lab -----------------------------------------------------------------------------------

function lab(section) {
  const slot = section.querySelector('.chart[data-chart="interaction-lab"]');
  const rows = Object.fromEntries([...section.querySelectorAll(".il-signals tr[data-sig]")].map((tr) => [tr.dataset.sig, tr]));
  const pre = section.querySelector(".il-code");
  const hostCode = section.querySelector("#il-host");
  const log = section.querySelector(".il-log");
  const what = section.querySelector(".il-what");
  const on = { brush: true, click: true, explore: true, drag: true };
  let view = null, base = null;
  /** Signals the page itself is setting right now (its own buttons): credited to the page. */
  let fromPage = null;
  /** Signals being put back after a switch (see `apply`): shown, not credited or lit up. */
  let restoring = false;
  const seen = {};

  // The code panel, one highlighted line each (so a line can light up when its signal changes).
  function renderCode() {
    pre.replaceChildren(...codeLines(on).map(([text, sig]) => {
      const c = document.createElement("code");
      c.dataset.lang = "ts";
      c.textContent = text;
      if (sig) c.dataset.sig = sig;
      highlight(c);
      return c;
    }));
  }
  /** Light up what changes while it changes, and let it fade a moment after the last change: a
   * brush writes its signal at every pointer move, and a highlight that stays put while it does
   * costs the page nothing per frame (an animation restarted at every move cost it frames). */
  const fading = new WeakMap();
  function flash(el) {
    el.classList.add("il-on");
    clearTimeout(fading.get(el));
    fading.set(el, setTimeout(() => el.classList.remove("il-on"), 700));
  }

  /** The readout: every row's value, and the rows in `touched` lit up and credited (to the page,
   * for those in `page`). */
  function show(s, touched, page = new Set()) {
    touched = new Set(touched);
    for (const [k, tr] of Object.entries(rows)) {
      const v = VALUE[k](s);
      const cell = tr.querySelector(".il-v");
      if (seen[k] !== v) {
        cell.textContent = v;
        seen[k] = v;
      }
      if (touched.has(k)) {
        const by = page.has(k) ? "the page: setSignal" : BY[k];
        const byCell = tr.querySelector(".il-by");
        if (byCell.textContent !== by) byCell.textContent = by;
        if (k !== "inspected") {
          flash(tr);
          for (const line of pre.querySelectorAll(`code[data-sig="${k}"]`)) flash(line);
        }
      }
    }
  }

  // The log: one entry per gesture (a brush's many moves update the entry on top).
  let lastKey = "", lastAt = 0;
  function note(kind, text, key = "") {
    const now = performance.now();
    if (key && key === lastKey && now - lastAt < 1200 && log.firstElementChild) {
      log.firstElementChild.querySelector("span").textContent = text;
    } else {
      const li = document.createElement("li");
      const b = document.createElement("b");
      b.textContent = kind;
      const t = document.createElement("span");
      t.textContent = text;
      li.append(b, t);
      log.prepend(li);
      while (log.children.length > 5) log.lastElementChild.remove();
    }
    lastKey = key;
    lastAt = now;
  }

  // Signal events come at every pointer move of a drag: the panel is redrawn once a frame, with
  // what changed since the last one (who changed it is noted as each event comes).
  let pending = null, drawing = 0;
  function onSignal(e) {
    const { signals: s, changed } = e.detail;
    pending ??= { changed: new Set(), page: new Set(), quiet: new Set() };
    pending.s = s;
    for (const name of changed) {
      const k = ROW(name);
      if (!k) continue;
      pending.changed.add(k);
      if (fromPage) pending.page.add(k);
      if (restoring) pending.quiet.add(k);
    }
    drawing ||= requestAnimationFrame(draw);
  }
  function draw() {
    drawing = 0;
    const q = pending;
    pending = null;
    if (!q) return;
    const loud = [...q.changed].filter((k) => !q.quiet.has(k));
    show(q.s, loud, q.page);
    const rowsChanged = loud.filter((k) => k !== "inspected");
    if (rowsChanged.length) {
      const text = rowsChanged.map((k) => `${k} → ${VALUE[k](q.s)}`).join(", ");
      note("signal", text, `signal:${rowsChanged.join()}`);
      const who = rowsChanged.every((k) => q.page.has(k)) ? "The page" : "You";
      what.textContent = `${who} changed ${rowsChanged.join(" and ")}: ${countLine(q.s)}`;
    }
    if (q.quiet.size) show(q.s, [], q.page);
  }
  /** What the chart shows now, from the status the page can read (its narration is empty: the
   * figure is interactive; so it's said from the signals). */
  function countLine(s) {
    const bits = [];
    if (s["days.active"] && Number.isFinite(s["days.lo"])) bits.push(`days ${Math.ceil(s["days.lo"])}–${Math.floor(s["days.hi"])}`);
    if (s.kinds?.length) bits.push(s.kinds.map((k) => NAMES[k] ?? k).join(" and "));
    bits.push(`trips over ${s.limit} min stand out`);
    return bits.join(", ");
  }
  function onPick(e) {
    const row = e.detail.row;
    if (!row?.fields) return;
    const f = row.fields;
    const text = "kind" in f && "km" in f ? `${NAMES[f.kind] ?? f.kind}, ${f.day} Oct, ${f.km} km, ${f.min} min`
      : "kind" in f ? `${NAMES[f.kind] ?? f.kind}: ${f.trips} trips`
      : "day" in f ? `${f.day} October: ${f.trips} trips` : JSON.stringify(f);
    note("pick", `row ${text}`);
  }

  // ---- the switches ----
  /** The camera the reader had explored to, kept while pan and zoom are switched off. */
  let camera = null;
  async function apply() {
    if (!view || !base) return;
    const doc = structuredClone(base);
    for (const [k, line] of Object.entries(LINES)) if (!on[k]) line.strip(doc);
    // A new document starts from its signals' defaults: the page puts back what the reader had
    // set (the same `setSignal` its buttons use), so only the switched line changes anything.
    const kept = view.signals ?? {};
    if (Number.isFinite(kept["zoom.zoom"])) camera = { x: kept["zoom.x"], y: kept["zoom.y"], zoom: kept["zoom.zoom"] };
    await view.setDocument(doc);
    restoring = true;
    try {
      if (on.click && kept.kinds?.length) view.setSignal("kinds", kept.kinds);
      if (Number.isFinite(kept.limit)) view.setSignal("limit", kept.limit);
      if (on.brush && kept["days.active"] && Number.isFinite(kept["days.lo"])) view.setSignal("days", { lo: kept["days.lo"], hi: kept["days.hi"] });
      if (on.explore && camera) for (const k of ["x", "y", "zoom"]) view.setSignal(`zoom.${k}`, camera[k]);
    } finally {
      restoring = false;
    }
    show(view.signals ?? {}, []);
  }
  for (const box of section.querySelectorAll(".il-switches input[data-line]")) {
    box.addEventListener("change", () => {
      const k = box.dataset.line;
      on[k] = box.checked;
      renderCode();
      for (const line of pre.querySelectorAll("code")) if (line.textContent === (on[k] ? LINES[k].on : LINES[k].off)) flash(line);
      // A line switched off leaves its signal where the reader put it; clear what it filtered by.
      if (!on[k] && view) {
        if (k === "brush") host(() => view.setSignal("days", null));
        if (k === "click") host(() => view.setSignal("kinds", []));
      }
      apply();
      what.textContent = on[k] ? `Back in the document: ${LINES[k].on}` : `Out of the document: the ${k === "explore" ? "camera" : k} no longer answers.`;
    });
  }

  // ---- the page's own buttons ----
  function host(f) {
    fromPage = true;
    try { f(); } finally { fromPage = null; }
  }
  const HOST = {
    week: [() => view.setSignal("days", { lo: 24, hi: 30 }), 'chart.setSignal("days", { lo: 24, hi: 30 });'],
    ebikes: [() => view.setSignal("kinds", ["ebike"]), 'chart.setSignal("kinds", ["ebike"]);'],
    long: [() => view.setSignal("limit", 40), 'chart.setSignal("limit", 40);'],
    closer: [() => view.setSignal("zoom.zoom", 3), 'chart.setSignal("zoom.zoom", 3);  // around the middle'],
    reset: [() => { view.setSignal("days", null); view.setSignal("kinds", []); view.setSignal("limit", 30); for (const k of ["x", "y", "zoom"]) view.setSignal(`zoom.${k}`, null); },
      'chart.setSignal("days", null);  // null clears: kinds, limit, zoom.* too'],
  };
  for (const b of section.querySelectorAll(".il-host button[data-host]")) {
    b.addEventListener("click", () => {
      const [run, code] = HOST[b.dataset.host];
      hostCode.textContent = code;
      highlight(hostCode);
      if (view) host(run);
    });
  }

  renderCode();
  const docUrl = new URL(slot.dataset.src.replace(/\/c\/([^/]+)$/, "/play/$1.json"), location.href);
  const docReady = fetch(docUrl).then((r) => r.json());
  whenReady(slot).then((v) => {
    view = v;
    v.addEventListener("signal", onSignal);
    v.addEventListener("pick", onPick);
    const s = v.signals;
    if (s) show(s, []);
  });
  docReady.then((d) => { base = d; });
}

const el = document.getElementById("lab");
if (el) lab(el);
