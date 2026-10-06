// grouped: a group of bars per category, one bar per series side by side. Cups sold at a café,
// by weekday and drink; rows keyed (drink, day) — the keys stacked segments use too.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, grouped, stacked } from "@datars/std";

const days = ["Mon", "Tue", "Wed", "Thu", "Fri"];
const cups: Record<string, number[]> = { Coffee: [120, 135, 128, 140, 160], Tea: [60, 55, 62, 58, 70], Cocoa: [20, 18, 25, 22, 35] };
const rows = Object.entries(cups).flatMap(([drink, n]) => n.map((cups, i) => ({ drink, day: days[i], cups })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "sales", x: "day", y: "cups", color: "drink", legend: true, title: "Cups sold at the café" } as const;

export default doc({
  title: "Cups sold at a café by weekday",
  description: "Three drinks' daily sales as grouped bars, then with value labels, then morphing into stacked bars.",
  size: [640, 320],
  data: { sales: data.values(rows, { key: ["drink", "day"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [grouped()] }, at("default")),
      plot({ ...frame, children: [grouped({ labels: true })] }, at("labels")),
      plot({ ...frame, children: [stacked()] }, at("stacked")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "grouped()", text: "A group per day, a bar per drink: the plot's colour field is the series." }),
      step("labels", { title: "labels: true", text: "Value labels where they fit a bar's width." }),
      step("stacked", { title: "stacked()", text: "The same (drink, day) keys: every bar morphs into its segment." }),
    ],
  }),
});
