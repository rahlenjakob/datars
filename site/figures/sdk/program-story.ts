// A three-step `story`: each `step` sets signals and carries narration, which the scene draws
// itself (`narration.title`, `narration.text`). The bars are keyed by region, so every step morphs.
import { data, doc, e, geom, group, op, repeat, shape, signal, step, story, text } from "@datars/sdk";

// Where a region's bar sits: its rank when sorted, else its place in the data.
const slot = "(sorted ? d.rank - 1 : d.i)";
const mid = `${slot} * box.w / 5 + box.w / 10`;

export default doc({
  id: "sdk-program-story",
  title: "A small story",
  description: "Five regions' sales as bars in three steps: all regions, then North highlighted, then the bars sorted from largest to smallest.",
  size: [640, 260],
  data: {
    sales: data.values({
      region: ["North", "South", "East", "West", "Centre"],
      i: [0, 1, 2, 3, 4],
      v: [42, 28, 35, 19, 31],
    }, { key: "region" }),
  },
  tables: { ranked: { from: "sales", ops: [op.window("rank", "v", "rank", { order: "-v" })] } },
  signals: { focus: signal.str(""), sorted: signal.bool(false) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [14, 16, 12, 16] },
    children: [
      group({ key: "narration", size: { h: 40 }, children: [
        text(e("narration.title"), [0, 0], {
          key: "title", style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" },
        }),
        text(e("narration.text"), [0, 20], {
          key: "text", style: { size: "$size.label", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") },
        }),
      ] }),
      group({
        key: "bars",
        scales: { y: { type: "linear", domain: [0, 45], range: ["=box.h - 18", 14] } },
        children: [
          repeat("ranked", group({ key: e("d.region"), children: [
            shape(geom.rect({
              x: e(`${slot} * box.w / 5 + 6`), w: e("box.w / 5 - 12"),
              y: e("scale.y(d.v)"), h: e("scale.y(0) - scale.y(d.v)"), r: 2,
            }), {
              key: "bar",
              fill: e('focus == "" || focus == d.region ? "$accent" : "$muted"'),
              opacity: e('focus == "" || focus == d.region ? 1 : 0.45'),
              semantics: { role: "datum", label: e("d.region + ': ' + d.v") },
            }),
            text(e("d.region"), [e(mid), e("box.h")], {
              key: "name", style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "bottom" },
            }),
            text(e("String(d.v)"), [e(mid), e("scale.y(d.v) - 4")], {
              key: "value", style: { size: "$size.label", ink: "$ink", align: "middle", baseline: "bottom" },
            }),
          ] })),
        ],
      }),
    ],
  }),
  program: story({ steps: [
    step("all", { set: { focus: "", sorted: false },
      title: "Sales by region", text: "Five regions, one bar each." }),
    step("north", { set: { focus: "North", sorted: false },
      title: "North leads", text: "A step sets a signal; the bars read it to fade the others." }),
    step("sorted", { set: { focus: "", sorted: true },
      title: "Sorted", text: "Each bar keeps its key, so it slides to its new place." }),
  ] }),
});
