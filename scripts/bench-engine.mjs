#!/usr/bin/env node
// The engine's own costs, platform-free: `datars profile --json` over a light chart and the heavy
// kinds (thousands of regions, tens of thousands of moving points, millions of points, a street-level
// map flight) — every state built cold and cached, every transition stepped at 60 fps on a headless
// GPU — collected into site/perf/engine.json for the performance page.
//
//   node scripts/bench-engine.mjs [--json site/perf/engine.json]
//
// Needs the CLI (`cargo build --release -p datars-cli`). Native, release build: a desktop's numbers.
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const opt = (name, def) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : def; };
const cli = join(root, "target/release/datars");

const CHARTS = [
  { chart: "votes", doc: "examples/votes/doc.json", what: "8 bars, a ranking, a pie" },
  { chart: "renewables", doc: "examples/renewables/doc.json", what: "242 regions, camera moves" },
  { chart: "counties", doc: "site/articles/us-election-2024/county-map/doc.ts", what: "3,100 US counties, each its own colour" },
  { chart: "scatter", doc: "examples/scatter/doc.json", what: "20,000 points, all moving" },
  { chart: "galaxy", doc: "examples/galaxy/doc.json", what: "4,000,000 points, level of detail" },
  { chart: "descent", doc: "examples/descent/doc.json", what: "a street-level map flight" },
];

/** p50, p95 and max of numbers, to 0.01. */
function spread(xs) {
  const s = [...xs].sort((a, b) => a - b);
  const at = (q) => s[Math.min(s.length - 1, Math.floor(q * s.length))] ?? 0;
  const r = (v) => Math.round(v * 100) / 100;
  return { p50: r(at(0.5)), p95: r(at(0.95)), max: r(s[s.length - 1] ?? 0) };
}

const runs = CHARTS.map((c) => {
  const out = execFileSync(cli, ["profile", join(root, c.doc), "--dpr", "2", "--json"], { cwd: join(root, dirname(c.doc)), maxBuffer: 1 << 28 }).toString();
  const p = JSON.parse(out);
  console.log(`${c.chart}: ${p.states.length} states, ${p.transitions.length} transitions`);
  // Per transition, the spread of each per-frame count (the frames themselves: `datars profile --timeline`).
  const transitions = p.transitions.map((t) => {
    const f = (k) => spread(t.frames.map((x) => x[k]));
    return { label: `${t.from} → ${t.to}`, frames: t.frames.length, dropped: t.dropped, stall_ms: t.stall_ms, unprepared_input_ms: t.unprepared_input_ms, engine: f("engine_ms"), gpu: f("raster_ms"), gpu_cpu: f("raster_cpu_ms"), draws: f("draws"), tessellated: f("tessellated"), rebuilt: f("instances_rebuilt") };
  });
  return { ...c, raster: p.raster, dpr: p.dpr, states: p.states, transitions };
});

const out = resolve(root, opt("json") ?? "site/perf/engine.json");
const machine = execFileSync("sysctl", ["-n", "machdep.cpu.brand_string"]).toString().trim();
writeFileSync(out, JSON.stringify({ machine, date: new Date().toISOString().slice(0, 10), runs }, null, 2) + "\n");
console.log(`wrote ${out}`);
