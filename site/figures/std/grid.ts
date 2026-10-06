// grid: gridlines at a scale's ticks across the plot area. A plot draws them along its value axis;
// add grid() as a child for the other direction, or turn the plot's off with `grid: false`.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, point, grid } from "@datars/std";

// Apple trees in an orchard: age (years) and height (m).
const age = [2, 3, 3, 4, 5, 5, 6, 7, 8, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const height = [1.4, 1.9, 2.2, 2.4, 2.9, 2.7, 3.3, 3.6, 3.8, 4.1, 4.0, 4.4, 4.5, 4.8, 4.7, 5.0, 5.1, 5.0];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const chart = { data: "trees", x: "age", y: "height", xType: "linear" as const, title: "Apple trees: height (m) by age (years)" };

export default doc({
  title: "Apple trees by age and height",
  description: "A scatter plot with the plot's horizontal gridlines, then vertical ones added with grid(), then none.",
  size: [640, 320],
  data: { trees: data.values({ id: age.map((_, i) => `tree ${i + 1}`), age, height }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...chart, children: [point()] }, at("default")),
      plot({ ...chart, children: [grid({ scale: "x", orient: "vertical" }), point()] }, at("vertical")),
      plot({ ...chart, grid: false, children: [point()] }, at("none")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "grid: true", text: "The plot's own gridlines, at the value axis's ticks." }),
      step("vertical", { title: "grid({ scale: \"x\", orient: \"vertical\" })", text: "A second grid, at the x axis's ticks." }),
      step("none", { title: "grid: false", text: "No gridlines: the axes alone." }),
    ],
  }),
});
