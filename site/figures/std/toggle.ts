// toggle: a switch for a boolean signal — here whether last year's line is drawn for comparison.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, line, toggle } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

export default doc({
  title: "Visitors a month (thousands)",
  description: "A switch shows or hides last year's line.",
  size: [640, 320],
  data: { v: data.values({
    month: [...months, ...months], year: [...months.map(() => "2025"), ...months.map(() => "2024")],
    n: [21, 23, 28, 34, 41, 52, 60, 58, 44, 33, 25, 27, 18, 20, 26, 30, 37, 47, 55, 54, 40, 31, 22, 24],
  }, { key: ["year", "month"] }) },
  signals: { compare: signal.bool(false) },
  tables: { shown: { from: "v", ops: [op.filter(e('compare || d.year == "2025"'))] } },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] }, children: [
    toggle({ signal: "compare", label: "Compare with 2024" }, { key: "compare", size: { w: 240, h: 28 } }),
    plot({ data: "shown", x: "month", y: "n", color: "year", yDomain: [0, 65], legend: true, children: [line({})] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("off", { set: { compare: false }, title: "compare = false" }),
    step("on", { set: { compare: true }, title: "compare = true" }),
  ] }),
});
