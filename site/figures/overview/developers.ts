// For developers: one line of the document changes — `bar()`, `line()`, `area()` — and the chart
// morphs; the overview page shows that line beside it. Illustrative figures.
import { data, doc, group, motion, step, story } from "@datars/sdk";
import { area, bar, line, plot } from "@datars/std";
import { PAD, SIZE, at } from "./_kit";

const month = Array.from({ length: 12 }, (_, i) => i + 1);
const users = month.map((m) => Math.round(20 + m * 4 + 6 * Math.sin(m)));
const p = { data: "users", x: "month", y: "users", xType: "linear" } as const;

export default doc({
  id: "overview-developers",
  title: "Monthly active users (k)",
  description: "Monthly active users over a year as bars, then a line, then an area.",
  size: SIZE,
  data: { users: data.values({ month, users }, { key: "month" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ ...p, xType: "band", children: [bar()] }, at("bar")),
    plot({ ...p, children: [line({ points: true })] }, at("line")),
    plot({ ...p, children: [area()] }, at("area")),
  ] }),
  motion: motion({ duration: 1.1 }),
  program: story({ steps: [step("bar"), step("line"), step("area")] }),
});
