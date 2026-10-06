// Interaction through signals. Top: drag across the dots — `on: { brush: brush("range") }` writes
// `range.lo`, `range.hi` and `range.active`, and `brushed()` keeps the rows inside. Bottom: press or
// drag — `on: { drag: scrub("at", { step: 1 }) }` sets `at` to the value under the pointer. The
// second step sets both signals, as a story step can.
import { brush, brushed, data, doc, e, geom, group, op, repeat, scrub, shape, signal, step, story, text } from "@datars/sdk";

// Sixty seeded values (integer maths, the same every build).
let seed = 11;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const values = Array.from({ length: 60 }, (_, i) => ({ id: i, v: Math.round((rnd() + rnd() + rnd()) / 3 * 1000) / 10, lane: Math.round(rnd() * 100) / 100 }));
const series = Array.from({ length: 31 }, (_, i) => ({ day: i + 1, y: Math.round(50 + 30 * Math.sin(i / 4) + (rnd() - 0.5) * 16) }));

export default doc({
  id: "sdk-interaction-brush",
  title: "Brush and scrub",
  description: "Sixty dots along a value axis with a brushed range highlighting the dots inside it, and a line over thirty-one days with a cursor set by scrubbing.",
  size: [640, 280],
  data: {
    dots: data.values(values, { key: "id" }),
    days: data.values(series, { key: "day" }),
  },
  tables: { inside: { from: "dots", ops: [op.filter(brushed("range", "d.v"))] } },
  signals: { range: signal.range(), at: signal.num(8) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 18, padding: [12, 16, 10, 16] },
    children: [
      group({
        key: "brushing",
        layout: { type: "rows", gap: 4 },
        children: [
          text(e("range.active ? `brush: ${format(range.lo, '.0f')} to ${format(range.hi, '.0f')}, ${table.count('inside')} of 60 rows` : 'drag across the dots to brush a range'"), [0, 0], { key: "status", size: { h: 16 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
          group({
            key: "strip",
            scales: { x: { type: "linear", domain: [0, 100], range: "width" } },
            on: { brush: brush("range", "x") },
            children: [
              shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h") }), { key: "hit", fill: "$surface", pickable: true, semantics: { role: "control", label: "Brush a range of values" } }),
              shape(geom.rect({ x: e("scale.x(range.lo)"), y: 0, w: e("scale.x(range.hi) - scale.x(range.lo)"), h: e("box.h") }), { key: "extent", when: e("range.active"), fill: "$accent", opacity: 0.14 }),
              repeat("dots", shape(geom.circle({ cx: e("scale.x(d.v)"), cy: e("8 + d.lane * (box.h - 16)"), r: 4 }), {
                key: e("d.id"),
                fill: e(`(${brushed("range", "d.v").expr}) ? "$accent" : "$muted"`),
                opacity: e(`(${brushed("range", "d.v").expr}) ? 1 : 0.4`),
              })),
            ],
          }),
        ],
      }),
      group({
        key: "scrubbing",
        layout: { type: "rows", gap: 4 },
        children: [
          text(e("'press or drag: at = ' + at"), [0, 0], { key: "label", size: { h: 16 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
          group({
            key: "chart",
            scales: { x: { type: "linear", domain: [1, 31], range: "width" }, y: { type: "linear", domain: [0, 100], range: "-height" } },
            on: { drag: scrub("at", { step: 1 }) },
            pickable: true,
            semantics: { role: "control", label: e("'Day ' + at") },
            children: [
              shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h") }), { key: "hit", fill: "$surface" }),
              shape(geom.polyline({ from: "days", x: e("scale.x(d.day)"), y: e("scale.y(d.y)") }), { key: "line", stroke: { paint: "$accent", width: 2 } }),
              shape(geom.segment({ x1: e("scale.x(at)"), y1: 0, x2: e("scale.x(at)"), y2: e("box.h") }), { key: "cursor", stroke: { paint: "$ink", width: 1 } }),
              repeat("days", shape(geom.circle({ cx: e("scale.x(d.day)"), cy: e("scale.y(d.y)"), r: 4 }), { key: e("d.day"), when: e("d.day == at"), fill: "$ink" })),
            ],
          }),
        ],
      }),
    ],
  }),
  program: story({ steps: [
    step("start", { set: { "range.active": false, at: 8 } }),
    step("set by a step", { set: { "range.active": true, "range.lo": 40, "range.hi": 62, at: 21 } }),
  ] }),
});
