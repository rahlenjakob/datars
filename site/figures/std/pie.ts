// pie: parts of a whole as slices, labelled beside the pie. The same keyed rows become a donut
// with its total in the middle, then slices sorted largest first.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { pie } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A household's monthly spending",
  description: "Six spending categories as a pie, then as a donut with its total, then with the slices sorted largest first.",
  size: [640, 320],
  data: {
    spending: data.values({ item: ["Food", "Housing", "Leisure", "Transport", "Savings", "Other"], eur: [640, 1200, 380, 420, 520, 340] }, { key: "item" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      pie({ data: "spending", value: "eur", category: "item" }, at("default")),
      pie({ data: "spending", value: "eur", category: "item", inner: 0.58, total: true, format: ",.0f" }, at("donut")),
      pie({ data: "spending", value: "eur", category: "item", inner: 0.58, total: true, format: ",.0f", sort: "desc" }, at("sorted")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "pie()", text: "One slice per row, coloured by category." }),
      step("donut", { title: "inner: 0.58, total: true", text: "A hole in the middle holds the total, in euros." }),
      step("sorted", { title: "sort: \"desc\"", text: "Largest slice first, clockwise from the top." }),
    ],
  }),
});
