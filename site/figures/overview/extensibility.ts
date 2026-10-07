// Extendable: a rose drawn by hand — one `geom.arc` per category, no recipe — and the standard
// library's bars, keyed by the same rows: each petal becomes its bar. Illustrative figures.
import { data, doc, e, geom, group, motion, repeat, shape, step, story } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { PAD, SIZE, at } from "./_kit";

const kind = ["Oak", "Birch", "Pine", "Spruce", "Beech", "Ash"];
const n = [34, 26, 21, 17, 11, 8];
const cx = "box.w / 2", cy = "box.h / 2";
const R = "min(box.w, box.h) * 0.47";
const k = (2 * Math.PI) / kind.length;
const rose = group({ ...at("rose"), children: [
  repeat("trees", shape(geom.arc({ cx: e(cx), cy: e(cy), r0: e(`${R} * 0.14`), r1: e(`${R} * (0.14 + 0.86 * sqrt(d.n / 34))`), a0: e(`d.i * ${k} + 0.05`), a1: e(`(d.i + 1) * ${k} - 0.05`) }), {
    key: e("d.kind"), fill: e(`[${kind.map((_, i) => `"$categorical[${i}]"`).join(", ")}][d.i]`), semantics: { role: "datum", label: e("d.kind + ': ' + d.n"), value: e("d.n") },
  })),
] });

export default doc({
  id: "overview-extensibility",
  title: "Trees in a park",
  description: "Six kinds of tree counted in a park, as a rose of hand-drawn petals, then as the standard library's bars.",
  size: SIZE,
  data: { trees: data.values({ kind, n, i: kind.map((_, i) => i) }, { key: "kind" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    rose,
    plot({ data: "trees", x: "kind", y: "n", color: "kind", children: [bar()] }, at("bars")),
  ] }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.2 }),
  program: story({ steps: [step("rose"), step("bars")] }),
});
