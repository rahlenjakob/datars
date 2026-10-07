// The home page (site/pages/index.html). Everything through <datars-view>'s public API: `send`,
// `setSignal`, `provideData`, `pick`, `status`, and the `state` and `signal` events. site.js mounts
// each chart when it scrolls near (and wires the hero, the galaxy's tour and readout, and the look editor);
// this script adds:
// - #everywhere: one chart stepped on six targets at once — the live web runtime, and what the iOS
//   app, the desktop viewer and the CLI's PNG and SVG drew at each step (images swapped only once
//   decoded, so a step never shows a blank or half-loaded screen);
// - #touch: four gestures as tabs, one live chart at a time (a hidden tab's chart isn't mounted),
//   each with what the chart reports as the reader goes;
// - #write: a doc.ts whose numbers, names and recipe the reader edits, played by a published chart.
import { highlight } from "./site.js";

const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];

/** The view in a slot, once site.js has mounted it. */
function viewIn(slot) {
  return new Promise((resolve) => {
    const v = slot.querySelector("datars-view");
    if (v) return resolve(v);
    const mo = new MutationObserver(() => {
      const w = slot.querySelector("datars-view");
      if (w) { mo.disconnect(); resolve(w); }
    });
    mo.observe(slot, { childList: true });
  });
}
/** …and once its engine runs (its first `state` event, or a status already there). */
async function readyView(slot) {
  const v = await viewIn(slot);
  if (v.status) return v;
  await new Promise((r) => v.addEventListener("state", r, { once: true }));
  return v;
}
/** Write code into a live block and highlight it (only when it changed). */
function show(code, text) {
  if (!code || code.textContent === text) return;
  code.textContent = text;
  highlight(code);
}
const put = (el, text) => { if (el && el.textContent !== text) el.textContent = text; };

// ---- #everywhere: one chart, six targets ---------------------------------------------------------

function everywhere() {
  const section = $("#everywhere");
  if (!section) return;
  const row = $(".ev-row", section);
  const pills = $$("#platforms-everywhere-pills button", section);
  const status = $(".ev-status", section);
  const slot = $(".ev-web .chart", section);
  const states = pills.map((p) => p.dataset.state);
  const shots = $$(".ev-shots", section).map((box) => ({ box, srcs: box.dataset.srcs.trim().split(/\s+/) }));
  let step = 0, auto = !reduced, visible = false, view = null, swap = 0;

  // Each screen holds a 402 × 812 box, scaled to its frame (shown once it's scaled).
  const screens = $$(".ev-screen", section);
  const fit = () => {
    for (const s of screens) s.style.setProperty("--k", String(s.clientWidth / 402));
    row.classList.add("ev-sized");
  };
  new ResizeObserver(fit).observe(row);
  fit();

  // Every step's captures, fetched and decoded while the section is still a screen away.
  const decoded = new Map();
  const decode = (src) => {
    if (!decoded.has(src)) {
      const img = new Image();
      img.decoding = "async";
      img.src = src;
      decoded.set(src, img.decode().then(() => img, () => img));
    }
    return decoded.get(src);
  };
  new IntersectionObserver((entries, io) => {
    if (!entries.some((e) => e.isIntersecting)) return;
    io.disconnect();
    for (const { srcs } of shots) srcs.forEach(decode);
  }, { rootMargin: "100% 0px" }).observe(section);

  async function go(i, byReader) {
    if (byReader) auto = false;
    step = i;
    for (const [j, p] of pills.entries()) p.setAttribute("aria-pressed", String(j === i));
    put(status, `${pills[i].textContent}, on six targets.`);
    view?.send(`goto:${states[i]}`);
    const run = ++swap;
    // Each target's new capture is its own element, decoded before it's shown (the step's files are
    // in the cache by now): all of them swap together, and no frame waits on a decode.
    const next = shots.map(({ box, srcs }) => {
      const img = new Image(402, 812);
      img.decoding = "async";
      img.alt = box.querySelector("img")?.alt ?? "";
      img.src = srcs[i];
      return img;
    });
    await Promise.all(next.map((img) => img.decode().catch(() => {})));
    if (run !== swap) return;
    shots.forEach(({ box }, k) => {
      const old = [...box.querySelectorAll("img")];
      const img = next[k];
      if (!reduced) img.classList.add("ev-in");
      box.appendChild(img);
      const done = () => old.forEach((o) => o.remove());
      if (reduced) return done();
      requestAnimationFrame(() => requestAnimationFrame(() => img.classList.remove("ev-in")));
      setTimeout(done, 520);
    });
  }
  for (const [i, p] of pills.entries()) p.addEventListener("click", () => go(i, true));

  readyView(slot).then((v) => {
    view = v;
    if (step) v.send(`goto:${states[step]}`);
    // The renderer the element picked: the GPU (WebGPU, else WebGL2), or the CPU as a last resort.
    const r = v.dataset.renderer;
    if (r) put($(".ev-renderer", section), r === "cpu" ? "on the CPU" : "on the GPU");
  });

  // Plays through the steps while most of it is on screen, until the reader picks one.
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; }, { threshold: 0.5 }).observe(row);
  if (!reduced) {
    setInterval(() => {
      if (auto && visible && !document.hidden && view) go((step + 1) % states.length, false);
    }, 3600);
  }
}

// ---- #touch: four gestures -------------------------------------------------------------------------

function touch() {
  const section = $("#touch");
  if (!section) return;
  const tabs = $$('[role="tab"]', section);
  const panels = tabs.map((t) => document.getElementById(t.getAttribute("aria-controls")));
  const box = $(".tc-panels", section);

  // Every tab's panel takes the same room, so switching never moves the page below: measured with
  // each shown in turn, within one task (no frame is drawn, so no hidden chart is seen and mounted).
  function hold() {
    const shown = panels.findIndex((p) => !p.hidden);
    box.style.minHeight = "";
    let h = 0;
    for (const p of panels) {
      for (const q of panels) q.hidden = q !== p;
      h = Math.max(h, box.offsetHeight);
    }
    panels.forEach((p, i) => { p.hidden = i !== shown; });
    box.style.minHeight = `${h}px`;
  }
  hold();
  let width = innerWidth;
  addEventListener("resize", () => { if (innerWidth !== width) { width = innerWidth; hold(); } });

  function select(tab, focus) {
    for (const [i, t] of tabs.entries()) {
      const on = t === tab;
      t.setAttribute("aria-selected", String(on));
      t.tabIndex = on ? 0 : -1;
      panels[i].hidden = !on;
    }
    if (focus) tab.focus();
  }
  for (const t of tabs) t.addEventListener("click", () => select(t, false));
  $(".tc-tabs", section).addEventListener("keydown", (e) => {
    const i = tabs.indexOf(document.activeElement);
    if (i < 0) return;
    const to = { ArrowRight: i + 1, ArrowDown: i + 1, ArrowLeft: i - 1, ArrowUp: i - 1, Home: 0, End: tabs.length - 1 }[e.key];
    if (to === undefined) return;
    e.preventDefault();
    select(tabs[(to + tabs.length) % tabs.length], true);
  });

  drag($("#tc-drag"));
  tap($("#tc-tap"));
  fly($("#tc-fly"));
  listen($("#tc-listen"));
}

/** Drag: the slider is the engine's; the page only reads the signal it sets. */
async function drag(panel) {
  const code = $("#tc-drag-live");
  const view = await readyView($(".chart", panel));
  const date = (day) => new Date(Date.UTC(2026, 0, day)).toLocaleDateString("en-GB", { day: "numeric", month: "long", timeZone: "UTC" });
  let changed = ["day"];
  const write = () => {
    const day = Math.round(Number(view.signals?.day ?? 172));
    show(code, `view.addEventListener("signal", (e) => …)\n// e.detail.changed: ${JSON.stringify(changed)}\nview.signals.day   // ${day}, ${date(day)}`);
  };
  view.addEventListener("signal", (e) => {
    if (!e.detail?.changed?.length) return;
    changed = e.detail.changed;
    write();
  });
  write();
}

/** Tap: a tap (or the button) sends the story its next step, and back to the start after the last. */
async function tap(panel) {
  const code = $("#tc-tap-live");
  const said = $("#tc-tap-said");
  const dots = $$(".tc-dots i", panel);
  const view = await readyView($(".chart", panel));
  let states = [], index = 0;
  const next = () => {
    if (!states.length) return;
    const last = index >= states.length - 1;
    const ev = last ? `goto:${states[0]}` : "next";
    const to = states[last ? 0 : index + 1];
    view.send(ev);
    show(code, `view.send(${JSON.stringify(ev)})   // → "${to}"`);
  };
  view.addEventListener("state", (e) => {
    const s = e.detail ?? {};
    states = s.states ?? states;
    index = s.index ?? index;
    dots.forEach((d, i) => d.classList.toggle("on", i === index));
    const n = s.narration ?? {};
    const text = [n.title, n.text].filter(Boolean).join(" — ");
    if (said.textContent === text) return;
    said.replaceChildren();
    if (n.title) said.appendChild(Object.assign(document.createElement("b"), { textContent: n.title }));
    if (n.text) said.appendChild(document.createTextNode(`${n.title ? " — " : ""}${n.text}`));
  });
  const st = view.status;
  states = st?.states ?? [];
  index = st?.index ?? 0;
  // A tap, not the end of a drag or a pinch.
  let down = null;
  view.addEventListener("pointerdown", (e) => { down = { x: e.clientX, y: e.clientY }; });
  view.addEventListener("click", (e) => {
    if (down && Math.hypot(e.clientX - down.x, e.clientY - down.y) > 8) return;
    next();
  });
  $("[data-tc-next]", panel).addEventListener("click", next);
}

/** Fly: the build's step pills drive it (site.js); the page shows the call and the narration. */
async function fly(panel) {
  const code = $("#tc-fly-live");
  const said = $("#tc-fly-said");
  const view = await readyView($(".chart", panel));
  let first = true;
  view.addEventListener("state", (e) => {
    const s = e.detail ?? {};
    if (!first) show(code, `view.send("goto:${s.state}")   // the camera flies`);
    first = false;
    const n = s.narration ?? {};
    const text = [n.title, n.text].filter(Boolean).join(" — ");
    if (said.textContent === text) return;
    said.replaceChildren();
    if (n.title) said.appendChild(Object.assign(document.createElement("b"), { textContent: n.title }));
    if (n.text) said.appendChild(document.createTextNode(`${n.title ? " — " : ""}${n.text}`));
  });
}

/** Listen: what a screen reader is told — the engine's semantics, walked in reading order, or the
 * mark under the pointer. */
async function listen(panel) {
  const voice = $("#tc-voice");
  const code = $("#tc-listen-live");
  const ring = $(".tc-ring", panel);
  const slot = $(".chart", panel);
  const view = await readyView(slot);
  // The items a screen reader walks (as the element's own hidden list: named, or actionable).
  const items = () => (view.status?.semantics ?? []).filter((i) => !(i.role === "control" && !i.actionable) && (i.actionable || i.label));
  let at = -1;
  const say = (label, role, how) => {
    voice.replaceChildren(Object.assign(document.createElement("span"), { textContent: `“${label}”` }));
    const roles = { datum: "a data point", series: "a series", axis: "an axis", tick: "an axis tick", title: "the title", group: "the chart", control: "a control", region: "a map region", legend: "a legend" };
    if (role) voice.appendChild(Object.assign(document.createElement("small"), { textContent: roles[role] ?? role }));
    show(code, how);
  };
  const outline = (b) => {
    if (!b) { ring.hidden = true; return; }
    const sr = slot.closest(".tc-stage").getBoundingClientRect(), vr = view.getBoundingClientRect();
    Object.assign(ring.style, { left: `${vr.left - sr.left + b[0] - 3}px`, top: `${vr.top - sr.top + b[1] - 3}px`, width: `${b[2] + 6}px`, height: `${b[3] + 6}px` });
    ring.hidden = false;
  };
  for (const b of $$("[data-tc-walk]", panel)) {
    b.addEventListener("click", () => {
      const list = items();
      if (!list.length) return;
      at = (at + Number(b.dataset.tcWalk) + list.length) % list.length;
      const it = list[at];
      say(it.label ?? it.role, it.role, `view.status.semantics[${at}]   // ${at + 1} of ${list.length}\n// role: ${JSON.stringify(it.role)}, label: ${JSON.stringify(it.label ?? "")}`);
      outline(it.box ?? it.bounds ?? null);
    });
  }
  let queued = null;
  view.addEventListener("pointermove", (e) => {
    if (queued) { queued = e; return; }
    queued = e;
    requestAnimationFrame(() => {
      const ev = queued;
      queued = null;
      const r = view.getBoundingClientRect();
      const x = Math.round(ev.clientX - r.left), y = Math.round(ev.clientY - r.top);
      const hit = view.pick(x, y).hits.find((h) => h.label && (h.role === "datum" || h.role === "series" || h.role === "region"));
      if (!hit) return;
      ring.hidden = true;
      say(hit.label, hit.role, `view.pick(${x}, ${y}).hits[0]\n// role: ${JSON.stringify(hit.role)}, label: ${JSON.stringify(hit.label)}`);
    });
  });
}

// ---- #write: the reader's document ---------------------------------------------------------------

const SCENES = {
  bar: { imports: "plot, bar", scene: 'plot({ data: "sales", x: "region", y: "sales",\n    children: [bar({ labels: true })] }),' },
  pie: { imports: "pie", scene: 'pie({ data: "sales", value: "sales",\n    category: "region", inner: 0.55 }),' },
  treemap: { imports: "treemap", scene: 'treemap({ data: "sales", value: "sales",\n    category: "region" }),' },
  waffle: { imports: "waffle", scene: 'waffle({ data: "sales", value: "sales",\n    category: "region" }),' },
};

function write() {
  const section = $("#write");
  if (!section) return;
  const slot = $(".chart", section);
  const names = $$(".wr-name", section), nums = $$(".wr-num", section);
  const ran = $("#wr-ran");
  const recipes = $$("[data-recipe]", section);
  let view = null;
  viewIn(slot).then((v) => { view = v; });

  const lit = (src) => {
    const c = document.createElement("code");
    c.dataset.lang = "ts";
    c.textContent = src;
    highlight(c);
    return c.innerHTML;
  };
  function recipe(name) {
    for (const b of recipes) b.setAttribute("aria-pressed", String(b.dataset.recipe === name));
    $("#wr-import").innerHTML = lit(SCENES[name].imports);
    $("#wr-scene").innerHTML = lit(SCENES[name].scene);
    view?.setSignal("shape", name);
    show(ran, `view.setSignal("shape", ${JSON.stringify(name)})`);
  }
  for (const b of recipes) b.addEventListener("click", () => recipe(b.dataset.recipe));

  function rows() {
    const seen = new Set();
    let ok = true;
    const out = names.map((n, i) => {
      const region = n.value.trim();
      const raw = nums[i].value.replace(/[\s_,]/g, "");
      const sales = Number(raw);
      const badName = !region || seen.has(region);
      const badNum = raw === "" || !Number.isFinite(sales) || sales < 0;
      seen.add(region);
      n.setAttribute("aria-invalid", String(badName));
      nums[i].setAttribute("aria-invalid", String(badNum));
      if (badName || badNum) ok = false;
      return { region, sales };
    });
    return ok ? out : null;
  }
  function edited(e) {
    const el = e.target;
    el.style.setProperty("--n", String(Math.max(1, el.value.length)));
    const r = rows();
    if (!r) return put(ran, "// not sent: every region needs its own name, and every sales figure a number ≥ 0");
    try {
      view?.provideData("sales", r);
      show(ran, `view.provideData("sales", [${r.map((x) => `{ region: ${JSON.stringify(x.region)}, sales: ${x.sales} }`).join(", ")}])`);
    } catch (err) {
      put(ran, `// the chart refused it: ${err.message ?? err}`);
    }
  }
  for (const el of [...names, ...nums]) el.addEventListener("input", edited);
  // Hand edits made before the chart mounted to it as it does.
  document.addEventListener("chartmount", (e) => {
    if (e.target !== slot) return;
    const r = rows();
    const edits = r && r.some((row, i) => row.region !== names[i].defaultValue || String(row.sales) !== nums[i].defaultValue);
    if (edits) e.detail.view.provideData("sales", r);
    const on = recipes.find((b) => b.getAttribute("aria-pressed") === "true")?.dataset.recipe;
    if (on && on !== "bar") e.detail.view.setSignal("shape", on);
  });
}

everywhere();
touch();
write();
