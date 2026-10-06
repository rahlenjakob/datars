// Stroke options on `shape`: width, dash, cap, join and markers. Each panel is a `rows` layout of
// samples; each sample draws in its own box, so the lines follow the width they're given.
import { data, doc, e, geom, group, shape, text, type StrokeOpts, type Template } from "@datars/sdk";

const L = "box.w - 10";
const caption = (s: string) => text(s, [2, 0], { key: "caption", style: { size: "$size.small", ink: "$muted", baseline: "top" } });
const sample = (name: string, draw: Template[]) => group({ key: name, children: [caption(name), ...draw] });
const line = (stroke: Partial<StrokeOpts>, extra: object = {}) =>
  shape(geom.segment({ x1: 6, y1: 26, x2: e(L), y2: 26 }), { key: "line", stroke: { paint: "$accent", width: 2, ...stroke }, ...extra });
const panel = (title: string, samples: Template[]) => group({
  key: title,
  layout: { type: "rows", gap: 0 },
  children: [
    text(title, [2, 0], { key: "title", size: { h: 22 }, style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } }),
    ...samples,
  ],
});

// Guides at a segment's ends, so the caps show how far past them they reach.
const guides = [6, L].map((x, i) => shape(geom.segment({ x1: e(String(x)), y1: 14, x2: e(String(x)), y2: 38 }), { key: `guide-${i}`, stroke: { paint: "$muted", width: 1, dash: [2, 2] } }));

const panels = (): Template[] => [
  panel("width", [1, 2, 4, 8].map((w) => sample(`width: ${w}`, [line({ width: w })]))),
  panel("dash", ([[8, 4], [2, 4], [12, 4, 2, 4], [1, 5]] as number[][]).map((d, i) => sample(`dash: [${d.join(", ")}]`, [line({ dash: d, width: 2, cap: i === 3 ? "round" : "butt" })]))),
  panel("cap", (["butt", "round", "square"] as const).map((c) => sample(`cap: "${c}"`, [line({ width: 10, cap: c }), ...guides]))),
  panel("join", (["miter", "round", "bevel"] as const).map((j) => sample(`join: "${j}"`, [
    shape(geom.polyline({ from: "zig", x: e(`8 + d.i * (${L} - 14) / 4`), y: e("40 - d.up * 22") }), { key: "line", stroke: { paint: "$accent", width: 7, join: j } }),
  ]))),
  panel("markers", [
    sample("end: arrow", [line({ width: 2 }, { markers: { end: { type: "arrow", size: 8 } } })]),
    sample("start: dot", [line({ width: 2 }, { markers: { start: { type: "dot", r: 4 } } })]),
    sample("both", [line({ width: 2 }, { markers: { start: { type: "dot", r: 4 }, end: { type: "arrow", size: 8 } } })]),
  ]),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 18, padding: [12, 14, 8, 12] }, children: panels() });

export default doc({
  id: "sdk-shape-strokes",
  title: "Stroke options",
  description: "Stroke widths 1 to 8, four dash patterns, butt, round and square caps, miter, round and bevel joins, and arrow and dot markers.",
  size: [640, 300],
  data: { zig: data.values({ i: [0, 1, 2, 3, 4], up: [0, 1, 0, 1, 0] }, { key: "i" }) },
  scene: group({ key: "figure", children: [grid(5, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
});
