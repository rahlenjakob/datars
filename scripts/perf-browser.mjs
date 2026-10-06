#!/usr/bin/env node
// Every chart on a site, every transition, in a real browser with the frame profiler on
// (`?datars-perf`, packages/web/src/perf.ts): the stall before a step moves, frame rate and its
// worst moment, frames dropped, and each frame split into engine and raster time.
//
//   node scripts/perf-browser.mjs http://localhost:8960 gallery/ articles/ [--dpr 2] [--json out.json]
//   node scripts/perf-browser.mjs http://localhost:8931/datars/ --pages examples/prices/,examples/votes/
//
// Real conditions: --network fast4g|slow4g|3g throttles every request (latency and bandwidth, as
// Chrome's DevTools presets), --cold disables the HTTP cache (a first visit), --cpu 4 slows the
// CPU fourfold (a mid-range phone). Streaming shows as frames drawn with tiles still missing and
// how long after the motion the view was complete.
//
// Scrolling: --scroll [px/s] scrolls each page top to bottom at a steady speed (default 1500) instead
// of stepping charts, and reports the frame rate the page kept, its long tasks, and which chart
// starting up (opening its bundle, first frame, GPU, poster) each long task overlapped.
//
// Where the time goes: --profile [filter] records a JavaScript CPU profile (wasm included, 0.1 ms
// samples) of each transition whose chart or label contains `filter` ("all" for every one), and
// prints the functions that took the most time over the transition and in its busiest 50 ms — the
// spike a frame-time readout can only point at. --profile-out dir keeps each as a .cpuprofile
// (open in Chrome DevTools' Performance panel). The shipped engines carry no function names; with
// `NAMES=1 scripts/build-wasm.sh` run first, the pages get the named builds in target/wasm-names/
// instead (the same code), and profiles name Rust functions rather than `wasm-function[956]`.
//
// Pages are paths under the base URL; each is searched for `datars-view` elements (mounted by the
// page, or slots — `.chart[data-src]`, or `[data-datars-view]` from @datars/react — scrolled into
// view to mount them). Needs Playwright
// (`npm i -g playwright` or DATARS_PLAYWRIGHT=/path/to/playwright/index.mjs) — a dev tool, not a
// dependency. Exit code 1 when a transition misses a budget (--budget-fps, default 50; --budget-stall
// ms, default 100).

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : def;
};
const SLOTS = ".chart[data-src], [data-datars-view]";
// Client-rendered pages (React, Vue…) put their charts on the page after `load`: wait for the
// first one (or give up quietly after a few seconds on a page without charts).
const rendered = (page) => page.waitForFunction((slots) => document.querySelector(`${slots}, datars-view`), SLOTS, { timeout: 8000 }).catch(() => {});
const flagged = new Set(["--dpr", "--json", "--budget-fps", "--budget-stall", "--pages", "--width", "--height", "--browser", "--network", "--cpu", "--scroll", "--profile", "--profile-out"].flatMap((f) => [f, opt(f.slice(2))]));
const positional = args.filter((a) => !a.startsWith("--") && !flagged.has(a));
const base = (positional[0] ?? "http://localhost:8960").replace(/\/$/, "");
const pages = (opt("pages") ? opt("pages").split(",") : positional.slice(1)).map((p) => p.replace(/^\//, ""));
const dpr = Number(opt("dpr", 2));
const budgetFps = Number(opt("budget-fps", 50));
const budgetStall = Number(opt("budget-stall", 100));

async function loadPlaywright() {
  for (const spec of [process.env.DATARS_PLAYWRIGHT, "playwright", "@playwright/test"].filter(Boolean)) {
    try {
      const m = await import(spec);
      return m.chromium ?? m.default?.chromium;
    } catch {}
  }
  console.error("perf-browser: Playwright not found (npm i -g playwright, or set DATARS_PLAYWRIGHT)");
  process.exit(2);
}

const chromium = await loadPlaywright();
// The installed Google Chrome (its real GPU stack, what readers have) unless --browser chromium:
// Playwright's own Chromium builds can leave WebGPU adapter requests unanswered on busy pages.
const browser =
  opt("browser") === "chromium"
    ? await chromium.launch({ args: ["--enable-unsafe-webgpu", "--ignore-gpu-blocklist"] })
    : await chromium.launch({ channel: "chrome", headless: true }).catch(() => {
        console.error("perf-browser: no Google Chrome installed; using Playwright's Chromium");
        return chromium.launch({ args: ["--enable-unsafe-webgpu", "--ignore-gpu-blocklist"] });
      });
const ctx = await browser.newContext({ viewport: { width: Number(opt("width", 1280)), height: Number(opt("height", 900)) }, deviceScaleFactor: dpr });
if (args.includes("--profile")) {
  // The engines with function names, served in place of the shipped ones (glue and wasm together).
  const fs = await import("node:fs");
  const named = new URL("../target/wasm-names/", import.meta.url);
  if (fs.existsSync(new URL("datars_core_bg.wasm", named))) {
    const built = fs.statSync(new URL("datars_core_bg.wasm", named)).mtime;
    console.error(`perf-browser: profiling with the named engines in target/wasm-names/ (built ${built.toISOString().slice(0, 16).replace("T", " ")})`);
    await ctx.route(/\/wasm\/(datars_[a-z_]+?(?:_bg\.wasm|\.js))(\?.*)?$/, async (route, request) => {
      const file = new URL(request.url().match(/\/wasm\/(datars_[a-z_]+?(?:_bg\.wasm|\.js))(\?.*)?$/)[1], named);
      if (!fs.existsSync(file)) return route.continue();
      await route.fulfill({ body: fs.readFileSync(file), contentType: file.pathname.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
    });
  } else {
    console.error("perf-browser: no named engines (NAMES=1 scripts/build-wasm.sh); wasm functions show as indices");
  }
}
const rows = [];
// Chrome DevTools' presets: latency ms, download / upload bytes per second.
const NETWORKS = {
  fast4g: { latency: 60, downloadThroughput: (9 * 1024 * 1024) / 8, uploadThroughput: (1.5 * 1024 * 1024) / 8 },
  slow4g: { latency: 150, downloadThroughput: (1.6 * 1024 * 1024) / 8, uploadThroughput: (750 * 1024) / 8 },
  "3g": { latency: 300, downloadThroughput: (780 * 1024) / 8, uploadThroughput: (330 * 1024) / 8 },
};
const network = opt("network");
if (network && !NETWORKS[network]) {
  console.error(`perf-browser: --network is one of ${Object.keys(NETWORKS).join(", ")}`);
  process.exit(2);
}
// --profile: which transitions to record, and where to keep the profiles.
const profileFilter = args.includes("--profile") ? (opt("profile") && !opt("profile").startsWith("--") ? opt("profile") : "all") : null;
const profileOut = opt("profile-out");
const profiled = (chart, label) => profileFilter !== null && (profileFilter === "all" || chart.includes(profileFilter) || label.includes(profileFilter));

/** A CPU profile's time by function: self time over the whole run and, for its busiest 50 ms
 * window (the spike), self and inclusive time there. Times in ms. */
function analyse(profile) {
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const parent = new Map();
  for (const n of profile.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
  const label = (n) => {
    const f = n.callFrame;
    const file = f.url ? f.url.split("/").pop().split("?")[0] : "";
    return `${f.functionName || "(anonymous)"}${file ? ` (${file})` : ""}`;
  };
  const idle = (n) => n.callFrame.functionName === "(idle)" || n.callFrame.functionName === "(root)";
  let t = profile.startTime;
  const at = profile.timeDeltas.map((d) => (t += d));
  const dur = at.map((x, i) => (i + 1 < at.length ? at[i + 1] - x : 0));
  const tally = (from, to, inclusive) => {
    const m = new Map();
    for (let i = from; i < to; i++) {
      let n = byId.get(profile.samples[i]);
      if (idle(n)) continue;
      const seen = new Set();
      do {
        const k = label(n);
        if (!seen.has(k)) m.set(k, (m.get(k) ?? 0) + dur[i] / 1000);
        seen.add(k);
        n = inclusive ? byId.get(parent.get(n.id)) : null;
      } while (n && !idle(n));
    }
    return [...m].sort((a, b) => b[1] - a[1]);
  };
  // The busiest 50 ms: a window sliding over the samples.
  const WINDOW = 50_000;
  let best = [0, 0, 0], busy = 0, lo = 0;
  for (let hi = 0; hi < at.length; hi++) {
    if (!idle(byId.get(profile.samples[hi]))) busy += dur[hi];
    while (at[hi] - at[lo] > WINDOW) {
      if (!idle(byId.get(profile.samples[lo]))) busy -= dur[lo];
      lo++;
    }
    if (busy > best[0]) best = [busy, lo, hi + 1];
  }
  const total = at.length ? (at[at.length - 1] - at[0]) / 1000 : 0;
  return { total, busy: tally(0, at.length, false).reduce((n, [, ms]) => n + ms, 0), self: tally(0, at.length, false), spike: { busy: best[0] / 1000, at: (at[best[1]] - at[0]) / 1000, self: tally(best[1], best[2], false), inclusive: tally(best[1], best[2], true) } };
}

async function conditions(page) {
  if (!network && !flagged.has("--cold") && !args.includes("--cold") && !opt("cpu")) return;
  const cdp = await ctx.newCDPSession(page);
  await cdp.send("Network.enable");
  if (args.includes("--cold")) await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
  if (network) await cdp.send("Network.emulateNetworkConditions", { offline: false, ...NETWORKS[network] });
  if (opt("cpu")) await cdp.send("Emulation.setCPUThrottlingRate", { rate: Number(opt("cpu")) });
}

// A scroll run: a page scrolled top to bottom, frames and long tasks recorded in the page.
async function scrollRun(p, speed) {
  const page = await ctx.newPage();
  await conditions(page);
  await page.addInitScript(() => {
    window.__longTasks = [];
    new PerformanceObserver((l) => l.getEntries().forEach((e) => window.__longTasks.push([Math.round(e.startTime), Math.round(e.duration)]))).observe({ type: "longtask", buffered: true });
    // Layout shifts (the page jumping): how much, and what moved.
    window.__shifts = [];
    const describe = (n) => (n && n.nodeType === 1 ? `${n.localName}${n.id ? "#" + n.id : ""}${n.className && typeof n.className === "string" ? "." + n.className.split(" ").join(".") : ""}${n.dataset?.src ? "[" + n.dataset.src + "]" : n.getAttribute?.("src") ? "[" + n.getAttribute("src") + "]" : ""}` : String(n?.nodeName ?? "?"));
    new PerformanceObserver((l) => l.getEntries().forEach((e) => { if (!e.hadRecentInput) window.__shifts.push([Math.round(e.startTime), Math.round(e.value * 1e4) / 1e4, (e.sources ?? []).map((s) => describe(s.node)).slice(0, 3)]); })).observe({ type: "layout-shift", buffered: true });
  });
  const url = new URL(`${base}/${p}`);
  url.searchParams.set("datars-perf", "");
  await page.goto(url.href, { waitUntil: "load" });
  await rendered(page);
  await page.waitForTimeout(2500);
  const r = await page.evaluate(async (speed) => {
    const gaps = [];
    const t0 = performance.now();
    const tasksBefore = window.__longTasks.length;
    await new Promise((done) => {
      let last = performance.now();
      const step = (now) => {
        gaps.push(now - last);
        const dt = Math.min(now - last, 100);
        last = now;
        // Instant: pages with `scroll-behavior: smooth` would otherwise restart a smooth scroll every
        // frame and crawl.
        scrollBy({ top: (speed * dt) / 1000, behavior: "instant" });
        if (innerHeight + scrollY >= document.documentElement.scrollHeight - 2) done();
        else requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    });
    const ms = performance.now() - t0;
    return { ms, gaps, tasks: window.__longTasks.slice(tasksBefore), t0, starts: window.__datarsStarts ?? [], shifts: window.__shifts };
  }, speed);
  await page.close();
  const gaps = r.gaps.slice(1);
  const dropped = gaps.reduce((n, g) => n + Math.max(0, Math.round(g / (1000 / 60)) - 1), 0);
  const worst = [...gaps].sort((a, b) => b - a);
  const fps = Math.round((gaps.length * 1000) / r.ms);
  console.log(`\n${p || "/"} scrolled at ${speed} px/s: ${(r.ms / 1000).toFixed(1)} s, ${fps} fps, ${dropped} frames dropped, worst gaps ${worst.slice(0, 5).map((g) => Math.round(g)).join(", ")} ms`);
  console.log(`  ${r.tasks.length} long tasks, ${r.tasks.reduce((n, t) => n + t[1], 0)} ms in all`);
  const cls = r.shifts.reduce((n, s) => n + s[1], 0);
  console.log(`  layout shift (CLS, sum over the run): ${cls.toFixed(4)} in ${r.shifts.length} shifts`);
  for (const [at, v, src] of [...r.shifts].sort((a, b) => b[1] - a[1]).slice(0, 6)) console.log(`    ${v.toFixed(4)} at ${((at - r.t0) / 1000).toFixed(2)} s: ${src.join(", ")}`);
  // Each long task and the start-up phases overlapping it.
  const phases = r.starts.flatMap((st) => st.phases.map(([name, at, ms]) => ({ src: st.src.split("/").pop(), name, at, ms })));
  for (const [at, dur] of [...r.tasks].sort((a, b) => b[1] - a[1]).slice(0, 8)) {
    const over = phases.filter((ph) => ph.at < at + dur && ph.at + ph.ms > at && ph.ms >= 2).map((ph) => `${ph.src} ${ph.name} ${ph.ms} ms`);
    console.log(`  ${String(dur).padStart(4)} ms at ${((at - r.t0) / 1000).toFixed(2)} s  ${over.join(" · ") || "(no chart start-up)"}`);
  }
  return { page: p, speed, ms: r.ms, fps, dropped, worst: worst.slice(0, 10), tasks: r.tasks, starts: r.starts, shifts: r.shifts };
}

if (args.includes("--scroll")) {
  const speed = Number(opt("scroll")) || 1500;
  const runs = [];
  for (const p of pages.length ? pages : [""]) runs.push(await scrollRun(p, speed));
  await browser.close();
  if (opt("json")) (await import("node:fs")).writeFileSync(opt("json"), JSON.stringify({ base, dpr, scroll: runs }, null, 2));
  process.exit(runs.some((r) => r.fps < budgetFps) ? 1 : 0);
}

for (const p of pages.length ? pages : [""]) {
  const page = await ctx.newPage();
  page.on("pageerror", (e) => console.error(`[${p}] page error: ${e.message}`));
  await conditions(page);
  const url = new URL(`${base}/${p}`);
  url.searchParams.set("datars-perf", "");
  await page.goto(url.href, { waitUntil: "load" });
  await rendered(page);
  // Slots that mount a view when near the screen, or views already on the page.
  const count = await page.evaluate((slots) => Math.max(document.querySelectorAll(slots).length, document.querySelectorAll("datars-view").length), SLOTS);
  for (let i = 0; i < count; i++) {
    const view = `(() => { const s = document.querySelectorAll(${JSON.stringify(SLOTS)})[${i}]; return s ? s.querySelector("datars-view") : document.querySelectorAll("datars-view")[${i}]; })()`;
    await page.evaluate(([slots, i]) => (document.querySelectorAll(slots)[i] ?? document.querySelectorAll("datars-view")[i]).scrollIntoView({ block: "center" }), [SLOTS, i]);
    const ready = await page.waitForFunction(`${view}?.dataset.renderer`, null, { timeout: network ? 90000 : 30000 }).then(() => true, () => false);
    // The slot's source, or (slots that name it only once mounted) the view's.
    const name = await page.evaluate(([slots, i]) => {
      const s = document.querySelectorAll(slots)[i] ?? document.querySelectorAll("datars-view")[i];
      const src = s.dataset?.src ?? s.getAttribute("src") ?? s.querySelector("datars-view")?.getAttribute("src");
      return (src ?? `view ${i}`).split("/").filter(Boolean).pop();
    }, [SLOTS, i]);
    if (!ready) {
      rows.push({ page: p, chart: name, label: "(never ready)" });
      continue;
    }
    await page.waitForTimeout(1200); // the first state settles
    const states = await page.evaluate(`${view}.status?.states ?? []`);
    // Forward through every step, then back to the start in one jump.
    const steps = [...states.slice(1), ...(states.length > 2 ? [states[0]] : [])];
    for (const to of steps) {
      const before = await page.evaluate(() => (window.__datarsPerf ?? []).length);
      const from = states[steps.indexOf(to)] ?? "";
      const prof = profiled(name, `${from} → ${to}`) ? await ctx.newCDPSession(page) : null;
      if (prof) {
        await prof.send("Profiler.enable");
        await prof.send("Profiler.setSamplingInterval", { interval: 100 });
        await prof.send("Profiler.start");
      }
      await page.evaluate(`${view}.send(${JSON.stringify(`goto:${to}`)})`);
      await page
        .waitForFunction(([n, to]) => (window.__datarsPerf ?? []).slice(n).some((s) => s.label.endsWith(`→ ${to}`)), [before, to], { timeout: 20000 })
        .catch(() => {});
      // Tiles still downloading when the motion ended: wait (a while) for the view to complete.
      await page
        .waitForFunction((n) => (window.__datarsPerf ?? []).slice(n).every((s) => !s.pendingAtEnd || s.complete !== null), before, { timeout: 15000 })
        .catch(() => {});
      await page.waitForTimeout(250);
      const got = await page.evaluate((n) => (window.__datarsPerf ?? []).slice(n), before);
      const s = got.find((s) => s.label.endsWith(`→ ${to}`));
      const row = s ? { page: p, chart: name, ...s } : { page: p, chart: name, label: `→ ${to} (no motion)` };
      if (prof) {
        const { profile } = await prof.send("Profiler.stop");
        await prof.detach();
        row.profile = analyse(profile);
        if (profileOut) {
          const fs = await import("node:fs");
          fs.mkdirSync(profileOut, { recursive: true });
          const file = `${profileOut}/${name}-${row.label}`.replace(/[^\w./-]+/g, "_") + ".cpuprofile";
          fs.writeFileSync(file, JSON.stringify(profile));
          row.profile.file = file;
        }
      }
      rows.push(row);
    }
  }
  await page.close();
}
await browser.close();

const pad = (s, n) => String(s ?? "–").padEnd(n);
const over = (r) => (r.engine ? r.minFps < budgetFps * 0.5 || r.fps < budgetFps || (r.stall ?? 0) > budgetStall : r.label === "(never ready)");
// Work a fast machine hides: a transition that rebuilds and uploads this many instances every frame
// runs smoothly on a desktop and misses frames on a phone (as `datars profile` flags it).
const REBUILT_FLAG = 20000;
const heavy = (r) => r.engine && (r.rebuilt?.p50 ?? 0) >= REBUILT_FLAG;
const conds = [network && `network ${network}`, args.includes("--cold") && "cold cache", opt("cpu") && `cpu ÷${opt("cpu")}`].filter(Boolean).join(", ");
console.log(`${base} · dpr ${dpr}${conds ? ` · ${conds}` : ""}`);
console.log(pad("chart", 28), pad("transition", 26), pad("fps", 4), pad("min", 4), pad("drop", 5), pad("stall ms", 22), pad("engine p50/p95/max", 19), pad("raster p50/p95/max", 19), pad("gaps p95/max", 13), pad("tiles", 16), pad("rebuilt p50/max", 16), "ops");
for (const r of rows) {
  if (!r.engine) {
    console.log(pad(r.chart, 28), r.label);
    continue;
  }
  const stall = r.stall === null ? "–" : `${r.stall} (in ${r.input} + 1st ${r.first})`;
  const tiles = !r.pendingFrames ? "" : `${r.pendingFrames}f ≤${r.pendingMax}` + (r.pendingAtEnd ? (r.complete === null ? " +?" : ` +${r.complete}ms`) : "");
  console.log(
    pad(r.chart, 28),
    pad(r.label, 26),
    pad(r.fps, 4),
    pad(r.minFps, 4),
    pad(r.dropped, 5),
    pad(stall, 22),
    pad(`${r.engine.p50}/${r.engine.p95}/${r.engine.max}`, 19),
    pad(`${r.raster.p50}/${r.raster.p95}/${r.raster.max}`, 19),
    pad(r.other ? `${r.other.p95}/${r.other.max}` : "", 13),
    pad(tiles, 16),
    pad(r.rebuilt ? `${r.rebuilt.p50}/${r.rebuilt.max}` : "", 16),
    r.ops,
    over(r) ? "  ← slow" : heavy(r) ? "  ← rebuilds every frame (check on a phone)" : "",
  );
}
// Transitions that dropped frames: their slowest frame and what it did (the page's own record,
// so it holds without a profiler shifting the timing).
const spikes = rows.filter((r) => r.dropped > 0 && r.slowest);
if (spikes.length) console.log("\nslowest frames (where frames dropped):");
for (const r of spikes) {
  const s = r.slowest;
  console.log(`  ${r.chart} ${r.label}: frame ${s.frame}, engine ${s.engine} ms, raster ${s.raster} ms · ${s.tessellated} meshes tessellated · ${s.ops} ops · ${s.rebuilt} instances rebuilt${s.pending ? ` · ${s.pending} tiles downloading` : ""}`);
}
// Profiles: where each recorded transition's time went, and its spike.
const ms = (v) => `${v.toFixed(1)} ms`;
for (const r of rows.filter((r) => r.profile)) {
  const pr = r.profile;
  console.log(`\n${r.chart} ${r.label}: ${ms(pr.busy)} busy of ${ms(pr.total)} recorded${pr.file ? ` (${pr.file})` : ""}`);
  console.log(`  most time (self): ${pr.self.slice(0, 8).map(([k, v]) => `${k} ${ms(v)}`).join(" · ")}`);
  console.log(`  busiest 50 ms, at ${ms(pr.spike.at)}: ${ms(pr.spike.busy)} busy`);
  console.log(`    self: ${pr.spike.self.slice(0, 8).map(([k, v]) => `${k} ${ms(v)}`).join(" · ")}`);
  console.log(`    inclusive: ${pr.spike.inclusive.slice(0, 14).map(([k, v]) => `${k} ${ms(v)}`).join(" · ")}`);
}
if (opt("json")) (await import("node:fs")).writeFileSync(opt("json"), JSON.stringify({ base, dpr, rows: rows.map(({ profile, ...r }) => (profile ? { ...r, profile: { ...profile, self: profile.self.slice(0, 40), spike: { ...profile.spike, self: profile.spike.self.slice(0, 40), inclusive: profile.spike.inclusive.slice(0, 60) } } } : r)) }, null, 2));
process.exit(rows.some(over) ? 1 : 0);
