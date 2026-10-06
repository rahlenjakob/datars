// Theme studio: a scatter coloured by group — dot size, the categorical palette, gridlines and
// muted tick labels. Illustrative figures.
import { doc, data, e, group } from "@datars/sdk";
import { plot, point } from "@datars/std";

const girth = [30, 38, 47, 57, 66, 66, 63, 65, 79, 63, 43, 39, 45, 58, 25, 51, 70, 46, 48, 37, 44, 43, 74, 52, 75, 40, 42, 69, 27, 35];
const kg = [20, 31, 56, 48, 51, 65, 58, 52, 89, 58, 40, 39, 44, 53, 29, 35, 54, 58, 37, 28, 50, 34, 62, 61, 64, 29, 52, 61, 15, 33];
const trees = girth.map((g, i) => ({ tree: `Tree ${i + 1}`, girth: g, kg: kg[i], variety: ["Cox", "Gala", "Bramley"][i % 3] }));

export default doc({
  title: "Trunk girth and yield of thirty apple trees",
  description: "Thirty apple trees as dots by trunk girth and yield, coloured by variety.",
  size: [480, 330],
  data: { trees: data.values(trees, { key: "tree" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 16, 10, 10] },
    children: [
      plot({ data: "trees", x: "girth", y: "kg", xType: "linear", xLabel: "Trunk girth (cm)", title: "Apples per tree (kg)", color: "variety", legend: true,
        children: [point({ label: e("`${d.tree}, ${d.variety}: ${d.girth} cm, ${d.kg} kg`") })] }, { key: "chart" }),
    ],
  }),
});
