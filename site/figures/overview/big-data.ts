// Big data: 60,000 generated points (seeded: the same on every platform, nothing stored), a sample
// drawn per frame that keeps the density, and the camera's box moved by each step. Illustrative.
import { data, doc, e, group, signal, step, story } from "@datars/sdk";
import { cloud } from "@datars/std";
import { PAD, SIZE } from "./_kit";

const centre = { x: "[-4, 3, 0.5]", y: "[-1, 2.2, -3.8]", sx: "[1.3, 0.8, 2.6]", sy: "[1.1, 0.6, 0.7]" };
const points = data.generate(60_000, {
  c: e("floor(rand(d.i, 1) * 3)"),
  x: e(`round(${centre.x}[d.c] + randn(d.i, 2) * ${centre.sx}[d.c], 3)`),
  y: e(`round(${centre.y}[d.c] + randn(d.i, 3) * ${centre.sy}[d.c], 3)`),
}, { keep: ["i", "x", "y", "c"], key: "i" });

export default doc({
  id: "overview-big-data",
  title: "60,000 points",
  description: "Sixty thousand generated points in three clusters, then zoomed into the edge of one, then into another.",
  size: SIZE,
  data: { points },
  signals: { x0: signal.num(-9), y0: signal.num(-6.5), x1: signal.num(9), y1: signal.num(5) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    cloud({ data: "points", x: "x", y: "y", bbox: [e("x0"), e("y0"), e("x1"), e("y1")], points: 9_000, r: 1.2,
      fill: e('["$categorical[0]", "$categorical[1]", "$categorical[2]"][d.c]'), name: "60,000 generated points" }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("all", { set: { x0: -9, y0: -6.5, x1: 9, y1: 5 } }),
    step("edge", { set: { x0: 2.2, y0: 0.8, x1: 5.4, y1: 3.4 } }),
    step("south", { set: { x0: -3.5, y0: -5.6, x1: 4.5, y1: -2 } }),
  ] }),
});
