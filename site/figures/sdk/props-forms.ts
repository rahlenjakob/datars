// One property — a dot's radius and fill — written as a literal, with `e()`, with `expr` splicing a
// field name, as a lambda, and reading a signal. Step: only the rows that read `k` change.
import { data, doc, e, expr, geom, group, lit, repeat, shape, signal, step, story, text, type Prop } from "@datars/sdk";

const field = "v";
const ROWS: [string, Prop, Prop][] = [
  ['r: 7, fill: "$muted"', 7, "$muted"],
  ['r: e("sqrt(d.v) * 1.4")', e("sqrt(d.v) * 1.4"), "$accent"],
  ["r: expr`sqrt(d.${field}) * 1.4`", expr`sqrt(d.${field}) * 1.4`, expr`d.${field} > 50 ? ${lit("$accent")} : ${lit("$muted")}`],
  ['fill: (d) => d.v > 50 ? "$accent" : "$muted"', 6, (d: { v: number }) => (d.v > 50 ? "$accent" : "$muted")],
  ['r: e("sqrt(d.v) * k")  (signal k)', e("sqrt(d.v) * k"), e('k > 1.5 ? "$highlight" : "$accent"')],
];

export default doc({
  id: "sdk-props-forms",
  title: "Five ways to write a property",
  description: "Five rows of the same ten values as dots, sized and coloured by a literal, an expression string, a template, a lambda and an expression that reads a signal.",
  size: [640, 250],
  data: { vals: data.values({ i: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], v: [4, 12, 25, 36, 49, 58, 64, 72, 88, 100] }, { key: "i" }) },
  signals: { k: signal.num(1.4) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 4, padding: [10, 14, 8, 14] },
    children: ROWS.map(([label, r, fill], j) => group({
      key: `row${j}`,
      layout: { type: "columns", gap: 12, wrap: 520 },
      children: [
        text(label, [0, e("box.h / 2")], { key: "code", size: { w: 280 }, style: { size: "$size.label", ink: "$ink-2", baseline: "middle" } }),
        group({ key: "dots", children: [repeat("vals", shape(geom.circle({ cx: e("(d.i + 0.5) * box.w / 10"), cy: e("box.h / 2"), r }), { key: e("d.i"), fill }))] }),
      ],
    })),
  }),
  program: story({ steps: [step("with k = 1.4", { set: { k: 1.4 } }), step("with k = 2", { set: { k: 2 } })] }),
});
