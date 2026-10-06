// The five things `repeat` can repeat a template over: a table's rows, groups of rows, a scale's
// ticks, a colour scale's legend entries, and a count. `d` is the row (or tick, entry, index).
import { data, doc, e, geom, group, repeat, shape, text, type Template } from "@datars/sdk";

const title = (s: string) => text(s, [0, 0], { key: "title", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } });
const note = (s: string) => text(s, [0, e("box.h - 2")], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "bottom" } });
const panel = (key: string, t: string, n: string, body: Template) => group({ key, children: [title(t), body, note(n)] });
// The drawing area inside a panel: below the title, above the note.
const top = 30, bottom = "box.h - 26";

const panels = (): Template[] => [
  panel("table", 'repeat("sales", …)', "d: the row",
    repeat("sales", shape(geom.rect({ x: e("d.i * box.w / 5 + 3"), y: e(`${bottom} - d.v * (${bottom} - ${top}) / 10`), w: e("box.w / 5 - 6"), h: e(`d.v * (${bottom} - ${top}) / 10`), r: 2 }), { key: e("d.region"), fill: "$accent" }))),
  panel("groups", '{ groups: "t", by: "s" }', "@group: its rows",
    repeat({ groups: "series", by: "s" }, shape(geom.polyline({ from: "@group", x: e("4 + d.x * (box.w - 8) / 5"), y: e(`${bottom} - d.y * (${bottom} - ${top}) / 10`), curve: "monotone" }), { key: e("d.s"), stroke: { paint: e('d.s == "a" ? "$categorical[0]" : d.s == "b" ? "$categorical[1]" : "$categorical[2]"'), width: 2 } }))),
  // A scale over the axis group's box (`range: "width"`), inset by its parent's padding.
  panel("ticks", '{ ticks: "x", count: 3 }', "d.value, d.label", group({ key: "inset", layout: { padding: [0, 12, 0, 8] }, children: [group({
    key: "axis",
    scales: { x: { type: "linear", domain: [0, 100], range: "width" } },
    children: [
      shape(geom.segment({ x1: 0, y1: 70, x2: e("box.w"), y2: 70 }), { key: "domain", stroke: { paint: "$rule", width: 1 } }),
      repeat({ ticks: "x", count: 3 }, group({ key: e("d.value"), children: [
        shape(geom.segment({ x1: e("scale.x(d.value)"), y1: 70, x2: e("scale.x(d.value)"), y2: 75 }), { key: "tick", stroke: { paint: "$rule", width: 1 } }),
        text(e("d.label"), [e("scale.x(d.value)"), 78], { key: "label", style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "top" } }),
      ] })),
    ],
  })] })),
  panel("legend", '{ legend: "c" }', "d.value, d.ink", group({
    key: "legend",
    scales: { c: { type: "categorical", domain: ["North", "South", "East", "West"], range: "$categorical" } },
    children: [repeat({ legend: "c" }, group({ key: e("d.value"), children: [
      shape(geom.rect({ x: 0, y: e(`${top} + 4 + d.index * 20`), w: 12, h: 12, r: 2 }), { key: "swatch", fill: e("d.ink") }),
      text(e("d.value"), [18, e(`${top} + 10 + d.index * 20`)], { key: "label", style: { size: "$size.label", ink: "$ink", baseline: "middle" } }),
    ] }))],
  })),
  panel("count", "{ count: 12 }", "d.index",
    repeat({ count: 12 }, shape(geom.circle({ cx: e("box.w / 2 + sin(d.index / 12 * 6.2832) * 34"), cy: e(`(${top} + ${bottom}) / 2 - cos(d.index / 12 * 6.2832) * 34`), r: e("2 + d.index * 0.5") }), { key: e("d.index"), fill: "$accent" }))),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 16, padding: [12, 12, 8, 12] }, children: panels() });

export default doc({
  id: "sdk-repeat-forms",
  title: "What repeat repeats",
  description: "Bars from a table's rows, one line per group of rows, tick marks from a scale, legend entries from a colour scale, and twelve dots from a count.",
  size: [640, 220],
  data: {
    sales: data.values({ region: ["N", "S", "E", "W", "C"], i: [0, 1, 2, 3, 4], v: [4, 7, 5, 9, 6] }, { key: "region" }),
    series: data.values({
      s: ["a", "a", "a", "a", "a", "a", "b", "b", "b", "b", "b", "b", "c", "c", "c", "c", "c", "c"],
      x: [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5],
      y: [2, 3, 5, 4, 6, 8, 6, 5, 6, 7, 6, 5, 1, 2, 2, 3, 3, 2],
    }, { key: ["s", "x"] }),
  },
  scene: group({ key: "figure", children: [grid(5, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
});
