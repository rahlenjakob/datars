// `choreo.stagger(order, spread)`: the same twelve bars change height in six orders. Each row has a
// motion rule selecting it by key path; the rows are listed in a shuffled order, so `data` (the
// order of the rows) differs from `left` (the order on screen). `value` reads each bar's
// `semantics.value`.
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, choreo, type Order, type Rule } from "@datars/sdk";

const ORDERS: [string, Order][] = [["data", "data"], ["left", "left"], ["right", "right"], ["center-out", "center-out"], ["value", "value"], ["{ random: 3 }", { random: 3 }]];
const key = (label: string) => label.replace(/[^a-z-]/g, "") || "random";

// Twelve bars: screen position `pos`, height `v`, listed in a shuffled order.
const pos = [5, 2, 9, 0, 11, 7, 3, 10, 1, 8, 4, 6];
const v = [0.55, 0.8, 0.35, 0.95, 0.5, 0.7, 0.4, 0.85, 0.6, 0.3, 0.75, 0.45];
const low = v.map((x) => Math.round((0.12 + x * 0.12) * 100) / 100);

const row = ([label]: [string, Order]) => group({
  key: key(label),
  layout: { type: "columns", gap: 10 },
  children: [
    text(label, [0, e("box.h / 2")], { key: "label", size: { w: 96 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "middle" } }),
    group({ key: "bars", children: [
      shape(geom.segment({ x1: 0, y1: e("box.h"), x2: e("box.w"), y2: e("box.h") }), { key: "base", stroke: { paint: "$rule", width: 1 } }),
      repeat("bars", shape(geom.rect({ x: e("d.pos * box.w / 12 + 1.5"), y: e("box.h * (1 - (tall ? d.v : d.low))"), w: e("box.w / 12 - 3"), h: e("box.h * (tall ? d.v : d.low)"), r: 1.5 }), {
        key: e("d.pos"), fill: "$accent",
        semantics: { role: "datum", label: e("'bar ' + d.pos"), value: e("d.v") },
      })),
    ] }),
  ],
});

export default doc({
  id: "sdk-motion-orders",
  title: "Stagger orders",
  description: "Six rows of the same twelve bars growing one after another, in data order, from the left, from the right, from the centre out, largest first, and in a seeded random order.",
  size: [640, 300],
  data: { bars: data.values({ pos, v, low }, { key: "pos" }) },
  signals: { tall: signal.bool(false) },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [12, 14, 10, 12] }, children: ORDERS.map(row) }),
  motion: motion(
    { duration: 1.8, easing: "cubic-out" },
    ...ORDERS.map(([label, order]): Rule => ({ select: { key: `root/${key(label)}` }, choreo: choreo.stagger(order, 0.75) })),
  ),
  program: story({ steps: [step("low", { set: { tall: false } }), step("tall", { set: { tall: true } })] }),
});
