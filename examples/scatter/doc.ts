// 20,000 points (seeded synthetic data) — instanced marks; the second state recolours and resizes.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, point } from "@datars/std";

let s = 42;
const rnd = () => ((s = (s * 1103515245 + 12345) % 2147483648) / 2147483648);
const n = 20000, x: number[] = [], y: number[] = [], g: string[] = [], id: number[] = [];
for (let i = 0; i < n; i++) {
  const c = i % 3;
  const r = Math.sqrt(-2 * Math.log(rnd() + 1e-9)) * 0.9, a = rnd() * 6.283;
  x.push(Math.round(((c - 1) * 2.2 + r * Math.cos(a)) * 1000) / 1000);
  y.push(Math.round(((c === 1 ? 1.5 : -0.5) + r * Math.sin(a)) * 1000) / 1000);
  g.push(["A", "B", "C"][c]);
  id.push(i);
}

export default doc({
  id: "scatter",
  title: "20,000 points",
  size: [720, 460],
  data: { pts: data.values({ id, x, y, g }, { key: "id" }) },
  signals: { emphasis: signal.str("all") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [plot({ data: "pts", x: "x", y: "y", xType: "linear", zero: false, color: "g", title: "Three clusters", children: [
      point({ r: e('emphasis == "B" ? (d.g == "B" ? 2.4 : 1.2) : 1.6'), opacity: e('emphasis == "B" && d.g != "B" ? 0.25 : 0.8') }),
    ] })],
  }),
  program: story({ steps: [step("all", { set: { emphasis: "all" } }), step("focus", { set: { emphasis: "B" }, text: "Cluster B sits apart." })] }),
});
