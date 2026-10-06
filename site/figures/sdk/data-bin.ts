// `op.bin` then `op.window("rank", …)`: 160 values as a strip of dots, then each dot dropped into its
// bin (steps of 10) and stacked by its rank inside the bin — a histogram made of the rows themselves.
import { data, doc, e, geom, group, op, repeat, shape, signal, step, story, text } from "@datars/sdk";

// A seeded, lumpy sample (integer maths, the same every build).
let seed = 5;
const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const values = Array.from({ length: 160 }, (_, id) => {
  const hump = id % 3 === 0 ? 68 : 38;
  return { id, v: Math.max(1, Math.min(99, Math.round(hump + (rnd() + rnd() + rnd() - 1.5) * 26))), jitter: Math.round(rnd() * 100) / 100 };
});

const binned = 'mode == "binned"';

export default doc({
  id: "sdk-data-bin",
  title: "Binning",
  description: "One hundred and sixty values between 0 and 100 as a strip of dots, then gathered into bins of ten and stacked into a histogram of dots.",
  size: [640, 260],
  data: { sample: data.values(values, { key: "id" }) },
  tables: {
    bins: { from: "sample", ops: [op.bin("v", "bin", { step: 10 }), op.window("rank", "id", "n", { partition: ["bin"], order: "id" })] },
  },
  signals: { mode: signal.str("rows") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [12, 16, 8, 16] },
    children: [
      text(e(`${binned} ? 'op.bin("v", "bin", { step: 10 }), then op.window("rank", "id", "n", { partition: ["bin"] })' : "160 rows: x is the value"`), [0, 0], { key: "caption", size: { h: 16 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top", maxWidth: e("box.w") } }),
      group({
        key: "chart",
        scales: { x: { type: "linear", domain: [0, 100], range: [4, "=box.w - 4"] } },
        children: [
          shape(geom.segment({ x1: 0, y1: e("box.h - 16"), x2: e("box.w"), y2: e("box.h - 16") }), { key: "base", stroke: { paint: "$rule", width: 1 } }),
          repeat({ ticks: "x", count: 10 }, text(e("d.label"), [e("scale.x(d.value)"), e("box.h")], { key: e("'t' + d.value"), style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "bottom", contain: true } })),
          repeat("bins", shape(geom.circle({
            // Binned: the dot's column is its bin's middle, its height its rank in the bin.
            cx: e(`${binned} ? scale.x(d.bin + 5) + ((d.n - 1) % 2 - 0.5) * 7 : scale.x(d.v)`),
            cy: e(`${binned} ? box.h - 22 - floor((d.n - 1) / 2) * 7 : 10 + d.jitter * (box.h - 40)`),
            r: 3,
          }), { key: e("d.id"), fill: "$accent", opacity: 0.85, semantics: { role: "datum", label: e("'value ' + d.v + ', bin ' + d.bin") } })),
        ],
      }),
    ],
  }),
  program: story({ steps: [step("rows", { set: { mode: "rows" } }), step("binned", { set: { mode: "binned" } })] }),
});
