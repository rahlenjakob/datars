// Position scales, each declared on a group (`scales: { x: { type, domain, range: "width" } }`) and
// drawn as an axis with `repeat({ ticks: "x" })`: the tick spacing shows how each one maps values.
import { data, doc, e, geom, group, repeat, shape, text, type ScaleDecl, type Template } from "@datars/sdk";

const AXIS_Y = 14;
// Ticks carry `d.value`, `d.label`, `d.pos` (the position), `d.kind` and, on a log scale, `d.major`
// (the powers of ten): minor log ticks get a mark but no label. `only` thins labels further.
const tickMarks = (only = "true"): Template[] => [
  shape(geom.segment({ x1: 0, y1: AXIS_Y, x2: e("box.w"), y2: AXIS_Y }), { key: "domain", stroke: { paint: "$rule", width: 1 } }),
  repeat({ ticks: "x" }, group({ key: e("d.label"), children: [
    shape(geom.segment({ x1: e("d.pos + scale.x.bandwidth() / 2"), y1: AXIS_Y - 4, x2: e("d.pos + scale.x.bandwidth() / 2"), y2: AXIS_Y + 4 }), { key: "tick", stroke: { paint: "$ink-2", width: 1 } }),
    text(e("d.label"), [e("d.pos + scale.x.bandwidth() / 2"), AXIS_Y + 7], { key: "label", when: e(`(d.kind != "log" || d.major) && (${only})`), style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "top", contain: true } }),
  ] })),
];
// Bands and points: a mark per category, at `scale.x(d.c)`.
const bands = [repeat("cats", shape(geom.rect({ x: e("scale.x(d.c)"), y: AXIS_Y - 11, w: e("scale.x.bandwidth()"), h: 9, r: 2 }), { key: e("d.c"), fill: "$accent", opacity: 0.35 }))];
const points = [repeat("cats", shape(geom.circle({ cx: e("scale.x(d.c)"), cy: AXIS_Y - 7, r: 3.5 }), { key: e("d.c"), fill: "$accent" }))];

const row = (name: string, decl: Omit<ScaleDecl, "range">, note: string, marks: Template[] = [], only?: string) => group({
  key: name,
  layout: { type: "columns", gap: 14 },
  children: [
    group({ key: "label", size: { w: 128 }, children: [
      text(`type: "${decl.type}"`, [0, 2], { key: "type", style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
      text(note, [0, 17], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
    ] }),
    group({ key: "axis", layout: { padding: [0, 10, 0, 6] }, children: [group({ key: "scale", scales: { x: { ...decl, range: "width" } as ScaleDecl }, children: [...marks, ...tickMarks(only)] })] }),
  ],
});

export default doc({
  id: "sdk-scales-position",
  title: "Position scales",
  description: "Seven axes, one per position scale: linear, log, sqrt and symlog over numbers, time over a year of dates, and band and point over six categories.",
  size: [640, 320],
  data: {
    year: data.values({ day: ["2024-01-01", "2024-12-31"] }, { key: "day", types: { day: "date" } }),
    cats: data.values({ c: ["A", "B", "C", "D", "E", "F"] }, { key: "c" }),
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [12, 12, 4, 12] },
    children: [
      row("linear", { type: "linear", domain: [0, 100] }, "domain [0, 100]"),
      row("log", { type: "log", domain: [1, 10000] }, "domain [1, 10000]"),
      row("sqrt", { type: "sqrt", domain: [0, 100] }, "domain [0, 100]"),
      row("symlog", { type: "symlog", domain: [-1000, 1000] }, "domain [-1000, 1000]", [], "abs(d.value) == 1000 || abs(d.value) == 200 || d.value == 0"),
      row("time", { type: "time", domain: { data: "year", field: "day" } }, "a year of dates"),
      row("band", { type: "band", domain: ["A", "B", "C", "D", "E", "F"], padding: 0.2 }, "padding: 0.2", bands),
      row("point", { type: "point", domain: ["A", "B", "C", "D", "E", "F"] }, "six categories", points),
    ],
  }),
});
