// Performance tables from benchmark JSON (`scripts/perf-browser.mjs --json`), committed in
// site/perf/: numbers are data, rendered at build time — never retyped into prose.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { escapeHtml } from "./markdown.mjs";

const load = (dir, name) => {
  const f = join(dir, `${name}.json`);
  if (!existsSync(f)) throw new Error(`site/perf/${name}.json missing`);
  return JSON.parse(readFileSync(f, "utf8"));
};
const n1 = (v) => (v === null || v === undefined || Number.isNaN(v) ? "—" : Number(v).toLocaleString("en", { maximumFractionDigits: 1 }));
const moving = (rows) => rows.filter((r) => r.frames);

/** Humanize an alias: `article--story` → `article · story`. */
const chartName = (c) => c.replace("--", " · ");

/** A transitions table: every chart's every transition, as the reader's browser played it. */
export function transitionsTable(dir, name, { limit } = {}) {
  const j = load(dir, name);
  const rows = moving(j.rows).slice(0, limit ?? Infinity);
  const cls = (r) => (r.dropped === 0 && r.fps >= 59 ? "ok" : r.fps >= 55 ? "meh" : "bad");
  return `<div class="table-wrap"><table class="perf"><thead><tr><th>Chart</th><th>Transition</th><th class="num">Avg fps</th><th class="num">Dropped frames</th><th class="num">Worst frame (ms)</th><th class="num">Stall (ms)</th><th class="num">Engine p95 (ms)</th><th class="num">Raster p95 (ms)</th></tr></thead><tbody>${rows.map((r) => `<tr class="${cls(r)}"><td>${escapeHtml(chartName(r.chart))}</td><td>${escapeHtml(r.label)}</td><td class="num">${n1(r.fps)}</td><td class="num">${n1(r.dropped)}</td><td class="num">${n1(r.worst)}</td><td class="num">${n1(r.stall)}</td><td class="num">${n1(r.engine?.p95)}</td><td class="num">${n1(r.raster?.p95)}</td></tr>`).join("")}</tbody></table></div>`;
}

/** Native apps (`scripts/bench-native.mjs`): each platform's device and renderer, then every
 * transition of every chart the sample app stepped through. The first frame after a step is the
 * stall (the step handled), as in the browser tables. Each frame's CPU time splits into the engine
 * and the render (GPU encode, submit and present); draws are the GPU draw calls a frame made. */
export function nativeTable(dir, name) {
  const j = load(dir, name);
  const cls = (r) => (r.dropped === 0 && r.fps >= 59 ? "ok" : r.fps >= 55 ? "meh" : "bad");
  const head = `<thead><tr><th>Chart</th><th>Transition</th><th class="num">Avg fps</th><th class="num">Dropped frames</th><th class="num">Worst frame (ms)</th><th class="num">CPU per frame p50 / p95 (ms)</th><th class="num">Engine / render p95 (ms)</th><th class="num">Draws p50</th></tr></thead>`;
  const platforms = [...new Set(j.runs.map((r) => r.platform))];
  return platforms.map((p) => {
    const runs = j.runs.filter((r) => r.platform === p && r.rows.length);
    if (!runs.length) return "";
    const first = runs[0];
    // wgpu's backend names, as readers know them.
    const renderer = (first.renderer ?? "CPU").replace(/^Gl\b/, "OpenGL ES");
    const rows = runs.flatMap((run) => run.rows.map((r) => `<tr class="${cls(r)}"><td>${escapeHtml(chartName(run.chart))}</td><td>${escapeHtml(r.label)}</td><td class="num">${n1(r.fps)}</td><td class="num">${n1(r.dropped)}</td><td class="num">${n1(r.worst)}</td><td class="num">${n1(r.cpu?.p50)} / ${n1(r.cpu?.p95)}</td><td class="num">${r.engine ? `${n1(r.engine.p95)} / ${n1(r.render?.p95)}` : "—"}</td><td class="num">${r.draws ? n1(r.draws.p50) : "—"}</td></tr>`)).join("");
    return `<h3 class="perf-platform">${escapeHtml(p)}: ${escapeHtml(first.device)} · ${escapeHtml(renderer)}</h3><div class="table-wrap"><table class="perf">${head}<tbody>${rows}</tbody></table></div>`;
  }).join("");
}

/** Counts over a run: how many transitions held 60 fps, dropped nothing, and the medians. */
export function transitionsSummary(dir, name) {
  const rows = moving(load(dir, name).rows);
  const steady = rows.filter((r) => r.fps >= 59 && r.dropped <= 1).length;
  const clean = rows.filter((r) => r.dropped === 0).length;
  const med = (xs) => { const s = [...xs].sort((a, b) => a - b); return s.length ? s[s.length >> 1] : null; };
  return { total: rows.length, steady, clean, stall: med(rows.map((r) => r.stall).filter((v) => v !== null)), engine: med(rows.map((r) => r.engine?.p95).filter((v) => v !== undefined)) };
}

/** Scrolling runs: frame rate, dropped frames, long tasks and layout shift (CLS) per page. */
export function scrollTable(dir, name, label) {
  const j = load(dir, name);
  return `<div class="table-wrap"><table class="perf"><thead><tr><th>Page</th><th class="num">Scrolled (s)</th><th class="num">Avg fps</th><th class="num">Dropped frames</th><th class="num">Worst frame (ms)</th><th class="num">Long tasks</th><th class="num">Layout shift (CLS)</th></tr></thead><tbody>${j.scroll.map((s) => {
    const cls = (s.shifts ?? []).reduce((a, x) => a + x[1], 0);
    return `<tr><td>${escapeHtml(label?.(s.page) ?? (s.page || "home"))}</td><td class="num">${n1(s.ms / 1000)}</td><td class="num">${n1(s.fps)}</td><td class="num">${n1(s.dropped)}</td><td class="num">${n1(s.worst?.[0])}</td><td class="num">${(s.tasks ?? []).length}</td><td class="num">${s.shifts ? cls.toFixed(3) : "—"}</td></tr>`;
  }).join("")}</tbody></table></div>`;
}

/** Median of numbers (null for none). */
const median = (xs) => {
  const s = xs.filter((x) => typeof x === "number" && !Number.isNaN(x)).sort((a, b) => a - b);
  return s.length ? s[s.length >> 1] : null;
};

/** A chart start's main-thread work ([name, at, ms] phases): opening its chunks, layout and the first
 * frame — not the network, not the GPU set-up (wall time), and not the wait for the reader to pause
 * scrolling (charts deliberately don't start mid-scroll). */
function openWork(start) {
  const ph = (start.phases ?? []).filter((p) => !/wall|poster|manifest/.test(p[0]));
  return ph.some((p) => p[0] === "first frame") ? ph.reduce((a, p) => a + p[2], 0) : null;
}

const steadyCount = (rows) => rows.filter((r) => r.fps >= 59 && r.dropped <= 1).length;
const cards = (items) => `<div class="perf-cards">${items.filter(Boolean).map((it) => `<article class="perf-card"><p class="pc-what">${escapeHtml(it.what)}</p><p class="pc-num"><b>${escapeHtml(it.num)}</b></p><p class="pc-note">${escapeHtml(it.note)}</p></article>`).join("")}</div>`;
/** A native run's transitions on one platform (`iOS`, `Android`), and its device and renderer. */
const nativeRows = (j, platform) => {
  const runs = j.runs.filter((r) => r.platform === platform && r.rows.length);
  return { rows: runs.flatMap((r) => r.rows.map((x) => ({ ...x, chart: r.chart }))), device: runs[0]?.device, renderer: (runs[0]?.renderer ?? "CPU").replace(/^Gl\b/, "OpenGL ES") };
};

/** What the runs say datars delivers on every platform, as headline cards — every number computed
 * from the JSON: the web (warm, a cold load, a phone's screen and CPU), iOS apps, and the heaviest
 * chart across both. Android's emulator figures stay in their table: its OpenGL translation layer
 * makes them the emulator's numbers more than a phone's, so they don't headline until a phone's
 * do. */
export function delivered(dir, { warm, cold, phone, native }) {
  const w = moving(load(dir, warm).rows);
  const c = cold ? moving(load(dir, cold).rows) : [];
  const ph = phone ? moving(load(dir, phone).rows) : [];
  const n = native ? load(dir, native) : null;
  const ios = n ? nativeRows(n, "iOS") : null;
  const galaxy = [...w, ...(ios?.rows ?? [])].filter((r) => r.chart === "galaxy");
  const range = (xs) => {
    const a = Math.round(Math.min(...xs)), b = Math.round(Math.max(...xs));
    return a === b ? `${a}` : `${a}–${b}`;
  };
  return cards([
    { what: "Web: transitions at a steady 59–60 fps", num: `${steadyCount(w)} of ${w.length}`, note: "every transition of every chart on this site, in Chrome with WebGPU" },
    c.length && { what: "Web, a first visit over throttled Fast 4G", num: `${steadyCount(c)} of ${c.length}`, note: "HTTP cache off: bundles and map tiles stream in while charts move" },
    ph.length && { what: "Web at phone size, with a quarter of the CPU", num: `${steadyCount(ph)} of ${ph.length}`, note: "390 × 844, Chrome's CPU slowed fourfold: roughly a mid-range phone" },
    ios?.rows.length && { what: "iOS apps (simulator): transitions steady", num: `${steadyCount(ios.rows)} of ${ios.rows.length}`, note: `${ios.device}, ${ios.renderer}` },
    galaxy.length && { what: "Four million points, camera tours", num: `${range(galaxy.map((r) => r.fps))} fps`, note: "in Chrome and on iOS, streaming a level-of-detail point archive" },
  ]);
}

/** The web's own headline cards: the stall before a step moves, the work to open a chart as its page
 * scrolls, and layout shift. */
export function webCards(dir, { warm, scroll }) {
  const w = moving(load(dir, warm).rows);
  const sc = scroll ? load(dir, scroll).scroll : [];
  const opens = sc.flatMap((s) => (s.starts ?? []).map(openWork)).filter((x) => x !== null);
  const home = sc.find((s) => !s.page);
  const cls = (s) => (s?.shifts ?? []).reduce((a, x) => a + x[1], 0);
  const worstCls = sc.length ? Math.max(...sc.map(cls)) : null;
  return cards([
    { what: "Median stall from a step to the first moving frame", num: `${median(w.map((r) => r.stall))} ms`, note: "the next steps are prepared while the reader reads" },
    opens.length && { what: "Median main-thread work to open a chart", num: `${Math.round(median(opens))} ms`, note: `opening its bundle, layout and first frame — ${opens.length} charts opening as their pages scroll` },
    sc.length && { what: "Layout shift while pages load and scroll (CLS)", num: worstCls < 0.001 ? "0" : worstCls.toFixed(3), note: `worst of ${sc.length} pages${home ? `; the home page kept ${home.fps} fps while scrolling` : ""}` },
  ]);
}

/** `10 of 10`: a native platform's steady transitions. */
export function nativeSummary(dir, name, platform) {
  const { rows } = nativeRows(load(dir, name), platform);
  return `${steadyCount(rows)} of ${rows.length}`;
}

/** The engine's own costs per chart (`scripts/bench-engine.mjs`): building a state (cold, and from
 * the cache), a step handled without idle-time preparation, and per frame: engine and GPU time,
 * draw calls and instances rebuilt — medians over the chart's states and transitions, worst frames
 * over all of them. */
export function engineTable(dir, name) {
  const j = load(dir, name);
  const med = (xs) => median(xs);
  const max = (xs) => Math.max(...xs);
  const ms = (v) => (v === null ? "—" : v < 0.1 ? "<0.1" : n1(v));
  const rows = j.runs.map((r) => {
    const t = r.transitions;
    return `<tr><td>${escapeHtml(r.what)}</td><td class="num">${ms(med(r.states.map((s) => s.cold_ms)))} / ${ms(med(r.states.map((s) => s.warm_ms)))}</td><td class="num">${ms(med(t.map((x) => x.unprepared_input_ms)))}</td><td class="num">${ms(med(t.map((x) => x.engine.p50)))} / ${ms(max(t.map((x) => x.engine.max)))}</td><td class="num">${ms(med(t.map((x) => x.gpu.p50)))} / ${ms(max(t.map((x) => x.gpu.max)))}</td><td class="num">${n1(Math.round(med(t.map((x) => x.draws.p50))))}</td><td class="num">${n1(Math.round(med(t.map((x) => x.rebuilt.p50))))}</td></tr>`;
  }).join("");
  return `<div class="table-wrap"><table class="perf"><thead><tr><th>Chart</th><th class="num">Build a state, cold / cached</th><th class="num">Unprepared step</th><th class="num">Engine per frame</th><th class="num">GPU per frame</th><th class="num">Draws per frame</th><th class="num">Rebuilt per frame</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

/** Every transition that wasn't steady (under 59 fps, or more than one frame dropped) in any of the
 * runs — the web ones (`name`) and the native one (`native:name`), each with where it ran. */
export function missesTable(dir, specs) {
  const where = { transitions: "Web", "transitions-fast4g-cold": "Web, cold Fast 4G", "transitions-phone": "Web, phone size, ¼ CPU" };
  const rows = specs.flatMap((spec) => {
    const [kind, name] = spec.includes(":") ? spec.split(":") : ["web", spec];
    if (kind === "native") {
      const j = load(dir, name);
      return ["iOS", "Android"].flatMap((p) => nativeRows(j, p).rows.map((r) => ({ ...r, where: p === "iOS" ? "iOS app" : "Android app" })));
    }
    return moving(load(dir, name).rows).map((r) => ({ ...r, where: where[name] ?? name }));
  }).filter((r) => !(r.fps >= 59 && r.dropped <= 1)).sort((a, b) => b.dropped - a.dropped);
  if (!rows.length) return `<p>None: every transition in every run was steady.</p>`;
  return `<div class="table-wrap"><table class="perf"><thead><tr><th>Chart</th><th>Transition</th><th class="num">Avg fps</th><th class="num">Dropped frames</th><th class="num">Worst frame (ms)</th><th>Where</th></tr></thead><tbody>${rows.map((r) => `<tr class="${r.fps >= 55 ? "meh" : "bad"}"><td>${escapeHtml(chartName(r.chart))}</td><td>${escapeHtml(r.label)}</td><td class="num">${n1(r.fps)}</td><td class="num">${n1(r.dropped)}</td><td class="num">${n1(r.worst)}</td><td>${escapeHtml(r.where)}</td></tr>`).join("")}</tbody></table></div>`;
}
