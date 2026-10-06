// Colour scales: `scale.c(v)` returns an ink. Each row sweeps its domain with 64 thin rectangles
// (`repeat({ count: 64 })`), or draws one swatch per category, and labels a few values.
import { doc, e, geom, group, repeat, shape, text, type ScaleDecl, type Template } from "@datars/sdk";

const N = 64;
/** A ramp over [lo, hi] and labels at `marks`. */
const ramp = (lo: number, hi: number, marks: number[]): Template[] => [
  repeat({ count: N }, shape(geom.rect({ x: e(`d.index * box.w / ${N}`), y: 2, w: e(`box.w / ${N} + 0.6`), h: 16 }), { key: e("d.index"), fill: e(`scale.c(${lo} + (d.index + 0.5) * ${(hi - lo) / N})`) })),
  ...marks.map((m, i) => text(String(m), [e(`${(m - lo) / (hi - lo)} * box.w`), 22], { key: `m${i}`, style: { size: "$size.small", ink: "$ink-2", align: i === 0 ? "start" : i === marks.length - 1 ? "end" : "middle", baseline: "top" } })),
];
const swatches = (names: string[]): Template[] => names.map((n, i) => group({ key: n, children: [
  shape(geom.rect({ x: e(`${i} * box.w / ${names.length}`), y: 2, w: e(`box.w / ${names.length} - 4`), h: 16, r: 2 }), { key: "swatch", fill: e(`scale.c("${n}")`) }),
  text(n, [e(`${i} * box.w / ${names.length}`), 22], { key: "name", style: { size: "$size.small", ink: "$ink-2", baseline: "top" } }),
] }));

const row = (name: string, decl: ScaleDecl, note: string, body: Template[]) => group({
  key: name,
  layout: { type: "columns", gap: 14 },
  children: [
    group({ key: "label", size: { w: 150 }, children: [
      text(`type: "${decl.type}"`, [0, 2], { key: "type", style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
      text(note, [0, 17], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
    ] }),
    group({ key: "ramp", layout: { padding: [0, 8, 0, 0] }, children: [group({ key: "scale", scales: { c: decl }, children: body })] }),
  ],
});

const CATS = ["North", "South", "East", "West", "Centre", "Isles"];

export default doc({
  id: "sdk-scales-color",
  title: "Colour scales",
  description: "Six colour scales: categorical swatches, a sequential and a diverging ramp, a threshold scale with three breaks, a quantize scale in five classes and a piecewise scale with pinned stops.",
  size: [640, 300],
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [12, 12, 4, 12] },
    children: [
      row("categorical", { type: "categorical", domain: CATS, range: "$categorical" }, 'range: "$categorical"', swatches(CATS)),
      row("sequential", { type: "sequential", domain: [0, 100], range: "$sequential" }, 'range: "$sequential"', ramp(0, 100, [0, 50, 100])),
      row("diverging", { type: "diverging", domain: [-40, 60], range: "$diverging", mid: 0 }, "mid: 0 (spans ±60)", ramp(-60, 60, [-60, 0, 60])),
      row("threshold", { type: "threshold", domain: [0, 100], thresholds: [25, 50, 75], range: ["#dbe6fb", "#9db7ef", "#5b83dc", "#27479b"] }, "thresholds: [25, 50, 75]", ramp(0, 100, [0, 25, 50, 75, 100])),
      row("quantize", { type: "quantize", domain: [0, 100], range: "$sequential" }, "five equal classes", ramp(0, 100, [0, 20, 40, 60, 80, 100])),
      row("piecewise", { type: "piecewise", domain: [0, 10], stops: "#3ca951 2 · #efb118 5 · #ff725c 8" }, "stops at 2, 5 and 8", ramp(0, 10, [0, 2, 5, 8, 10])),
    ],
  }),
});
