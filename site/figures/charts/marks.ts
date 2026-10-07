// The "Marks" family on one table: a café's cups by drink and weekday, every row keyed (drink, day),
// so each mark becomes the same row's mark in the next chart — grouped bars into stacked segments,
// into heatmap cells, into dots, into points on lines; then coffee alone as bars with a shaded
// span, a target rule and a callout.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { plot, bar, grouped, stacked, cell, dot, line, area, rule, span, annotate } from "@datars/std";

const days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const cups: Record<string, number[]> = {
  Coffee: [120, 135, 128, 140, 160, 182, 150],
  Tea: [60, 55, 62, 58, 70, 64, 80],
  Cocoa: [20, 18, 25, 22, 35, 48, 52],
};
const rows = Object.entries(cups).flatMap(([drink, n]) => n.map((c, i) => ({ drink, day: days[i], cups: c })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const title = "Cups sold at the café";
const frame = { data: "sales", x: "day", y: "cups", color: "drink", legend: true, title } as const;

export default doc({
  title: "A café's week, drawn by the standard marks",
  description: "Three drinks' sales over a week as grouped bars, stacked bars, a heatmap, a dot plot, lines with points and areas, then coffee alone as bars with a shaded range, a target line and a callout.",
  size: [640, 400],
  data: { sales: data.values(rows, { key: ["drink", "day"] }) },
  tables: { coffee: { from: "sales", ops: [{ op: "filter", expr: { expr: 'd.drink == "Coffee"' } }] } },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      plot({ ...frame, children: [grouped()] }, at("grouped")),
      plot({ ...frame, children: [stacked()] }, at("stacked")),
      plot({ data: "sales", x: "day", y: "drink", yType: "band", color: "cups", colorType: "sequential", padding: 0, grid: false, title, children: [cell({ gap: 3 })] }, at("heatmap")),
      plot({ data: "sales", x: "cups", y: "day", xType: "linear", yType: "band", color: "drink", legend: true, title, children: [dot({ r: 7 })] }, at("dots")),
      plot({ ...frame, xType: "point", children: [line({ labels: true, points: true })] }, at("lines")),
      plot({ ...frame, xType: "point", children: [area({ opacity: 0.22 }), line({ labels: true })] }, at("areas")),
      plot({ data: "coffee", x: "day", y: "cups", title: "Coffee, cups a day", children: [
        span({ axis: "y", from: 0, to: 100, opacity: 0.08 }),
        bar(),
        rule({ value: 140 }),
        annotate({ x: e('scale.x("Sat") + scale.x.bandwidth() / 2'), y: e("scale.y(182)"), text: "Best day: 182 cups", dx: -130, dy: -8 }, { key: "note" }),
      ] }, at("bars")),
    ],
  }),
  program: story({
    steps: [
      step("grouped", { title: "grouped()", text: "A group per day, a bar per drink." }),
      step("stacked", { title: "stacked()", text: "The same (drink, day) keys: every bar becomes its segment." }),
      step("heatmap", { title: "cell()", text: "Each segment becomes a heatmap cell, coloured by its cups." }),
      step("dots", { title: "dot()", text: "A dot per drink and day, on a value axis." }),
      step("lines", { title: "line({ points: true })", text: "A line per drink, a point per day, named at its end." }),
      step("areas", { title: "area()", text: "The same series, filled." }),
      step("bars", { title: "bar() · span() · rule() · annotate()", text: "Coffee alone: bars over a shaded break-even range, a target at 140 and a callout." }),
    ],
  }),
});
