// line: one line per series through x/y — here a city's average temperature through the year.
// Rows are keyed (city, month), so every line keeps its identity across the steps.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, line } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const normals: Record<string, number[]> = {
  Oslo: [-4.3, -4, -0.2, 4.5, 10.8, 15.2, 16.4, 15.2, 10.8, 6.3, 0.7, -3.1],
  Madrid: [6.3, 7.9, 11.2, 12.9, 16.7, 22.4, 25.6, 25.1, 20.9, 15.1, 9.9, 6.9],
  Cairo: [14, 15.3, 17.7, 21.5, 25, 27.4, 28.3, 28.3, 26.4, 23.6, 19.4, 15.6],
};
const rows = Object.entries(normals).flatMap(([city, t]) => t.map((c, i) => ({ city, month: months[i], c })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "temps", x: "month", y: "c", xType: "point", color: "city", title: "Average temperature (°C)" } as const;

export default doc({
  title: "Average temperature through the year",
  description: "Three cities' monthly average temperatures as lines, then labelled at their ends, then with a dot at every month.",
  size: [640, 320],
  data: { temps: data.values(rows, { key: ["city", "month"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, legend: true, children: [line()] }, at("default")),
      plot({ ...frame, children: [line({ labels: true })] }, at("labels")),
      plot({ ...frame, children: [line({ labels: true, points: true })] }, at("points")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "line()", text: "One line per city: the plot's colour field splits the rows into series." }),
      step("labels", { title: "labels: true", text: "Each line is named at its last point instead of in a legend." }),
      step("points", { title: "points: true", text: "A dot at every data point." }),
    ],
  }),
});
