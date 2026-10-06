// facet: one small chart per city, in a grid, on shared scales so the panels compare at a glance.
// The chart is an ordinary plot; facet hands each panel its group's rows.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { facet, plot, line, bar } from "@datars/std";

// Average temperature by month (°C, rounded climate normals).
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const normals = { Oslo: [-4, -4, 0, 5, 11, 15, 16, 15, 11, 6, 1, -3], Madrid: [6, 8, 11, 13, 17, 22, 26, 25, 21, 15, 10, 7], Cairo: [14, 15, 18, 21, 25, 27, 28, 28, 27, 24, 19, 16] };
const rows = Object.entries(normals).flatMap(([city, t]) => t.map((temp, i) => ({ city, month: months[i], temp })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const lines = plot({ x: "month", y: "temp", xType: "point", suffix: "°", children: [line()] });

export default doc({
  title: "Average temperature, one panel per city",
  description: "Three cities' monthly temperatures as small multiples on shared scales, then each panel on its own scale, then as bars.",
  size: [640, 320],
  data: { temps: data.values(rows, { key: ["city", "month"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      facet({ data: "temps", by: "city", columns: 3, chart: lines }, at("default")),
      facet({ data: "temps", by: "city", columns: 3, chart: lines, shared: false }, at("unshared")),
      facet({ data: "temps", by: "city", columns: 3, chart: plot({ x: "month", y: "temp", suffix: "°", children: [bar()] }) }, at("bars")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "facet({ by: \"city\", chart })", text: "One panel per city; every panel on the same scales." }),
      step("unshared", { title: "shared: false", text: "Each panel fits its own rows: shapes compare, levels don't." }),
      step("bars", { title: "chart: plot(… bar())", text: "Any chart can be repeated." }),
    ],
  }),
});
