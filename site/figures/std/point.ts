// point: a dot per row at (x, y) — a scatter plot. Thirty apple trees in an orchard: trunk girth
// against the season's yield. Dots are instanced, so the same call draws a hundred thousand.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, point } from "@datars/std";

const girth = [30, 38, 47, 57, 66, 66, 63, 65, 79, 63, 43, 39, 45, 58, 25, 51, 70, 46, 48, 37, 44, 43, 74, 52, 75, 40, 42, 69, 27, 35];
const kg = [20, 31, 56, 48, 51, 65, 58, 52, 89, 58, 40, 39, 44, 53, 29, 35, 54, 58, 37, 28, 50, 34, 62, 61, 64, 29, 52, 61, 15, 33];
const age = [4, 9, 13, 18, 25, 22, 22, 25, 28, 19, 14, 13, 15, 18, 6, 17, 28, 13, 17, 11, 13, 10, 26, 17, 28, 11, 13, 27, 6, 6];
const trees = girth.map((g, i) => ({ tree: `Tree ${i + 1}`, girth: g, kg: kg[i], age: age[i], variety: ["Cox", "Gala", "Bramley"][i % 3] }));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "trees", x: "girth", y: "kg", xType: "linear", xLabel: "Trunk girth (cm)", title: "Apples picked per tree (kg)" } as const;
const label = e("`${d.tree}, ${d.variety}: ${d.girth} cm, ${d.kg} kg, ${d.age} years old`");

export default doc({
  title: "Trunk girth and yield of thirty apple trees",
  description: "Thirty apple trees as dots by trunk girth and yield, then coloured by variety, then sized by age.",
  size: [640, 320],
  data: { trees: data.values(trees, { key: "tree" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [point({ label })] }, at("default")),
      plot({ ...frame, color: "variety", legend: true, children: [point({ label })] }, at("color")),
      plot({ ...frame, color: "variety", legend: true, children: [point({ label, r: e("2 + d.age / 4"), opacity: 0.8 })] }, at("size")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "point()", text: "One dot per tree: girth across, yield up." }),
      step("color", { title: "color: \"variety\"", text: "The plot's colour field colours the dots, with a legend." }),
      step("size", { title: "r: e(\"2 + d.age / 4\")", text: "Radius as an expression over the row: older trees are bigger dots." }),
    ],
  }),
});
