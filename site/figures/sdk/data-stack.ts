// `op.stack`: the rows as they come (side by side), stacked (`offset: "zero"`), and as shares of
// each column (`offset: "expand"`). One derived table holds both: two stack operations write their
// extents into different columns (`as`), and the bars read the pair for the step.
import { data, doc, e, geom, group, op, repeat, shape, signal, step, story, text } from "@datars/sdk";

const Q = ["Q1", "Q2", "Q3", "Q4"], S = ["a", "b", "c"];
const V = [[12, 8, 5], [15, 9, 8], [11, 14, 7], [18, 10, 12]];
const rows = Q.flatMap((q, i) => S.map((s, j) => ({ q, s, j, v: V[i][j] })));

const mode = (m: string) => `mode == "${m}"`;
const y0 = e(`${mode("rows")} ? scale.y(0) : ${mode("stacked")} ? scale.y(d.y0) : scale.p(d.e0)`);
const y1 = e(`${mode("rows")} ? scale.y(d.v) : ${mode("stacked")} ? scale.y(d.y1) : scale.p(d.e1)`);
const x = e(`scale.x(d.q) + (${mode("rows")} ? d.j * scale.x.bandwidth() / 3 : 0)`);
const w = e(`${mode("rows")} ? scale.x.bandwidth() / 3 - 2 : scale.x.bandwidth()`);

export default doc({
  id: "sdk-data-stack",
  title: "Stacking",
  description: "Three series over four quarters: side by side, then stacked on each other, then stretched to shares of each quarter's total.",
  size: [640, 280],
  data: { sales: data.values(rows, { key: ["q", "s"] }) },
  tables: {
    stacked: { from: "sales", ops: [
      op.stack({ x: "q", series: "s", value: "v", as: ["y0", "y1"] }),
      op.stack({ x: "q", series: "s", value: "v", offset: "expand", as: ["e0", "e1"] }),
    ] },
  },
  signals: { mode: signal.str("rows") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [12, 16, 10, 16] },
    children: [
      text(e(`${mode("rows")} ? "the rows: one bar per quarter and series" : ${mode("stacked")} ? 'op.stack({ x: "q", series: "s", value: "v" })  →  y0, y1' : 'op.stack({ …, offset: "expand" })  →  shares 0…1'`), [0, 0], { key: "caption", size: { h: 16 }, style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
      group({
        key: "chart",
        scales: {
          x: { type: "band", domain: Q, range: [34, "=box.w"], padding: 0.25 },
          y: { type: "linear", domain: [0, 40], range: ["=box.h - 18", 0] },
          p: { type: "linear", domain: [0, 1], range: ["=box.h - 18", 0] },
        },
        children: [
          repeat({ ticks: "y", count: 4 }, group({ key: e("'y' + d.value"), when: e(`!(${mode("expand")})`), children: [
            shape(geom.segment({ x1: 34, y1: e("scale.y(d.value)"), x2: e("box.w"), y2: e("scale.y(d.value)") }), { key: "grid", stroke: { paint: "$grid", width: 1 } }),
            text(e("d.label"), [26, e("scale.y(d.value)")], { key: "label", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "middle" } }),
          ] })),
          repeat({ ticks: "p", count: 4 }, group({ key: e("'p' + d.value"), when: e(mode("expand")), children: [
            shape(geom.segment({ x1: 34, y1: e("scale.p(d.value)"), x2: e("box.w"), y2: e("scale.p(d.value)") }), { key: "grid", stroke: { paint: "$grid", width: 1 } }),
            text(e("format(d.value, '.0%')"), [26, e("scale.p(d.value)")], { key: "label", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "middle" } }),
          ] })),
          repeat("stacked", shape(geom.rect({ x, y: y1, w, h: e(`(${y0.expr}) - (${y1.expr})`) }), {
            key: e("d.q + '-' + d.s"), fill: e("'$categorical[' + d.j + ']'"), stroke: { paint: "$paper", width: 1 },
            semantics: { role: "datum", label: e("d.q + ' ' + d.s + ': ' + d.v") },
          })),
          repeat({ ticks: "x" }, text(e("d.label"), [e("d.pos + scale.x.bandwidth() / 2"), e("box.h")], { key: e("'q-' + d.label"), style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "bottom" } })),
        ],
      }),
    ],
  }),
  program: story({ steps: [step("rows", { set: { mode: "rows" } }), step("stacked", { set: { mode: "stacked" } }), step("expand", { set: { mode: "expand" } })] }),
});
