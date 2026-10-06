// treemap: one rectangle per category, its area proportional to its value. The second step
// colours the same rectangles by another field, on piecewise stops.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { treemap } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A greengrocer's sales by product",
  description: "Eight products sized by this year's sales, coloured by product, then by whether they grew on last year.",
  size: [640, 320],
  data: {
    sales: data.values({
      product: ["Apples", "Bananas", "Tomatoes", "Potatoes", "Berries", "Citrus", "Salad", "Herbs"],
      sales: [182, 150, 121, 96, 88, 64, 41, 23],
      growth: [4, -3, 9, -8, 21, 2, -12, 15],
    }, { key: "product" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      treemap({ data: "sales", value: "sales", category: "product" }, at("default")),
      treemap({ data: "sales", value: "sales", category: "product", color: "growth", colorType: "piecewise", stops: "$negative -1 · $positive 1" }, at("growth")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "treemap()", text: "Area shows sales; colour tells the products apart." }),
      step("growth", { title: "color: \"growth\", colorType: \"piecewise\"", text: "Colour now shows whether sales fell or grew on last year." }),
    ],
  }),
});
