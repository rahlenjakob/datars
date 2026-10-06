// plot: the frame most charts live in — scales from the data, axes, gridlines, a title and a
// legend — with marks as children. The steps change the frame, not the line inside it.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, line } from "@datars/std";

// Average temperature by month (°C, rounded climate normals).
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const normals = { Oslo: [-4, -4, 0, 5, 11, 15, 16, 15, 11, 6, 1, -3], Madrid: [6, 8, 11, 13, 17, 22, 26, 25, 21, 15, 10, 7], Cairo: [14, 15, 18, 21, 25, 27, 28, 28, 27, 24, 19, 16] };
const rows = Object.entries(normals).flatMap(([city, t]) => t.map((temp, i) => ({ city, month: months[i], temp })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "temps", x: "month", y: "temp", color: "city", xType: "point" as const, title: "Average temperature" };

export default doc({
  title: "Average temperature in three cities",
  description: "Monthly temperatures in a plot with a title and legend, then with a subtitle and a unit, then zoomed to the summer on explicit domains.",
  size: [640, 320],
  data: { temps: data.values(rows, { key: ["city", "month"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, legend: true, children: [line()] }, at("default")),
      plot({ ...frame, legend: true, subtitle: "Monthly means, rounded", suffix: "°C", children: [line()] }, at("text")),
      plot({ ...frame, legend: true, subtitle: "Monthly means, rounded", suffix: "°C", xDomain: ["May", "Jun", "Jul", "Aug", "Sep"], yDomain: [10, 30], clip: true,
        children: [line()] }, at("zoom")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "plot({ x, y, color, legend: true })", text: "Scales, axes and gridlines come from the data." }),
      step("text", { title: "subtitle, suffix", text: "A subtitle under the title and a unit on every value label." }),
      step("zoom", { title: "xDomain, yDomain, clip", text: "Explicit domains are exact: a zoom into the summer." }),
    ],
  }),
});
