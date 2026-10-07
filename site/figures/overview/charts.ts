// Batteries included: the same six rows as std's bars, a donut and a treemap — keyed by drink, so
// each bar becomes its slice and its tile. Illustrative figures.
import { data, doc, group, motion, step, story } from "@datars/sdk";
import { bar, pie, plot, treemap } from "@datars/std";
import { PAD, SIZE, at } from "./_kit";

export default doc({
  id: "overview-charts",
  title: "Coffee orders, three ways",
  description: "Six drinks ordered at a café as bars, then as a donut, then as a treemap.",
  size: SIZE,
  data: { orders: data.values({ drink: ["Latte", "Espresso", "Flat white", "Mocha", "Tea", "Cortado"], n: [42, 31, 24, 15, 12, 9] }, { key: "drink" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ data: "orders", x: "drink", y: "n", color: "drink", children: [bar()] }, at("bars")),
    pie({ data: "orders", value: "n", category: "drink", inner: 0.55, labels: false }, at("donut")),
    treemap({ data: "orders", value: "n", category: "drink", format: ".0f" }, at("treemap")),
  ] }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  program: story({ steps: [step("bars"), step("donut"), step("treemap")] }),
});
