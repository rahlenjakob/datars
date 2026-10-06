// `op.window`: a value per row computed over its neighbours in order. Ninety noisy days, then their
// 14-day rolling mean (`rolling_mean`, k: 14; `min: 14` leaves the first 13 days empty), then the
// running total (`cumsum`) on its own scale. One line, keyed, morphs between the columns it reads.
import { data, doc, e, geom, group, op, repeat, shape, signal, step, story, text } from "@datars/sdk";

let seed = 3;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const days = Array.from({ length: 90 }, (_, i) => ({ day: i + 1, v: Math.round(40 + 18 * Math.sin(i / 11) + i * 0.15 + (rnd() - 0.5) * 34) }));

const is = (m: string) => `mode == "${m}"`;

export default doc({
  id: "sdk-data-window",
  title: "Window functions",
  description: "Ninety noisy daily values, then a fourteen-day rolling mean over them, then their running total.",
  size: [640, 260],
  data: { daily: data.values(days, { key: "day" }) },
  tables: {
    win: { from: "daily", ops: [
      op.window("rolling_mean", "v", "mean", { order: "day", k: 14, min: 14 }),
      op.window("cumsum", "v", "total", { order: "day" }),
    ] },
    // The rolling mean's line starts where the window is full.
    full: { from: "win", ops: [op.filter(e("d.mean != null"))] },
  },
  signals: { mode: signal.str("raw") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [12, 16, 8, 16] },
    children: [
      text(e(`${is("raw")} ? "90 rows: day, v" : ${is("mean")} ? 'op.window("rolling_mean", "v", "mean", { order: "day", k: 14, min: 14 })' : 'op.window("cumsum", "v", "total", { order: "day" })'`), [0, 0], { key: "caption", size: { h: 16 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top", maxWidth: e("box.w") } }),
      group({
        key: "chart",
        scales: {
          x: { type: "linear", domain: [1, 90], range: [30, "=box.w"] },
          y: { type: "linear", domain: [0, 100], range: ["=box.h - 4", 4] },
          t: { type: "linear", domain: [0, 5000], range: ["=box.h - 4", 4] },
        },
        children: [
          repeat({ ticks: "y", count: 4 }, group({ key: e("'y' + d.value"), when: e(`!(${is("total")})`), children: [
            shape(geom.segment({ x1: 30, y1: e("scale.y(d.value)"), x2: e("box.w"), y2: e("scale.y(d.value)") }), { key: "grid", stroke: { paint: "$grid", width: 1 } }),
            text(e("d.label"), [24, e("scale.y(d.value)")], { key: "label", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "middle" } }),
          ] })),
          repeat({ ticks: "t", count: 4 }, group({ key: e("'t' + d.value"), when: e(is("total")), children: [
            shape(geom.segment({ x1: 30, y1: e("scale.t(d.value)"), x2: e("box.w"), y2: e("scale.t(d.value)") }), { key: "grid", stroke: { paint: "$grid", width: 1 } }),
            text(e("format(d.value, '.2~s')"), [24, e("scale.t(d.value)")], { key: "label", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "middle" } }),
          ] })),
          // The raw values stay as dots; the line reads the column for the step.
          repeat("win", shape(geom.circle({ cx: e("scale.x(d.day)"), cy: e(`${is("total")} ? scale.t(d.total) : scale.y(d.v)`), r: 2 }), { key: e("d.day"), fill: "$muted", opacity: e(`${is("raw")} ? 0.9 : 0.35`) })),
          shape(geom.polyline({ from: "win", x: e("scale.x(d.day)"), y: e("scale.y(d.v)") }), { key: "line", when: e(is("raw")), stroke: { paint: "$accent", width: 1.5, join: "round" } }),
          shape(geom.polyline({ from: "full", x: e("scale.x(d.day)"), y: e("scale.y(d.mean)"), curve: "monotone" }), { key: "line", when: e(is("mean")), stroke: { paint: "$accent", width: 2.5, join: "round" } }),
          shape(geom.polyline({ from: "win", x: e("scale.x(d.day)"), y: e("scale.t(d.total)") }), { key: "line", when: e(is("total")), stroke: { paint: "$accent", width: 2.5, join: "round" } }),
        ],
      }),
    ],
  }),
  program: story({ steps: [step("raw", { set: { mode: "raw" } }), step("rolling mean", { set: { mode: "mean" } }), step("running total", { set: { mode: "total" } })] }),
});
