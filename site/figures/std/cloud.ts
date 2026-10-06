// cloud: 80,000 generated points in an explorable view (drag to pan, scroll or pinch to zoom). A
// frame draws at most `points` of them — a sample that keeps the density — and every row up close.
import { doc, data, e, group, signal, story, step, text } from "@datars/sdk";
import { cloud } from "@datars/std";

// Three clusters of seeded normal draws: the same points on every platform, nothing stored.
const centre = { x: "[-4, 3, 0.5]", y: "[-1, 2.2, -3.8]", sx: "[1.3, 0.8, 2.6]", sy: "[1.1, 0.6, 0.7]" };
const points = data.generate(80_000, {
  c: e("floor(rand(d.i, 1) * 3)"),
  x: e(`round(${centre.x}[d.c] + randn(d.i, 2) * ${centre.sx}[d.c], 3)`),
  y: e(`round(${centre.y}[d.c] + randn(d.i, 3) * ${centre.sy}[d.c], 3)`),
}, { keep: ["x", "y", "c"] });

export default doc({
  title: "80,000 points",
  description: "Eighty thousand generated points drawn as a sample of 20,000, then with a glow that shows where they are dense, then zoomed to the edge of a cluster, where every point is drawn.",
  size: [640, 320],
  data: { points },
  // The camera's box, in the points' own units: the steps move it.
  signals: { x0: signal.num(-9), y0: signal.num(-6.5), x1: signal.num(9), y1: signal.num(5) },
  scene: group({ key: "root", layout: { type: "rows", gap: 4, padding: [16, 20, 12, 12] }, children: [
    text("80,000 points in three clusters", [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } }),
    cloud({
      data: "points", x: "x", y: "y", bbox: [e("x0"), e("y0"), e("x1"), e("y1")], points: 20_000, r: 1.4,
      fill: e('["$categorical[0]", "$categorical[1]", "$categorical[2]"][d.c]'),
      glow: e('state == "default" ? 0 : 7'), glowOpacity: 0.06, name: "80,000 generated points",
    }, { key: "chart" }),
  ] }),
  program: story({
    steps: [
      step("default", { set: { x0: -9, y0: -6.5, x1: 9, y1: 5 }, title: "cloud({ points: 20_000 })", text: "A uniform sample of a quarter of the rows: the density reads the same." }),
      step("glow", { set: { x0: -9, y0: -6.5, x1: 9, y1: 5 }, title: "glow: 7", text: "A faint, wide dot under a sample of the rows: where they crowd, it brightens." }),
      step("zoom", { set: { x0: 3.3, y0: 1.2, x1: 5.7, y1: 3.2 }, title: "Zoomed to a cluster's edge", text: "Fewer rows in view than `points`: every one is drawn. Hover one." }),
    ],
  }),
});
