// waterfall: a bridge from one total to the next — each change floats from the running total,
// rows marked as totals show the running total from zero. Lives inside a plot, like any mark.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, waterfall } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A café's profit, last year to this",
  description: "A profit bridge from last year to this year through four changes, then with a currency prefix and a unit suffix on the labels.",
  size: [640, 320],
  data: {
    bridge: data.values({
      item: ["Last year", "More guests", "Higher prices", "Rent", "Staff", "This year"],
      change: [84, 22, 9, -12, -18, 85],
      total: [false, false, false, false, false, true], // a total row shows the running total
    }, { key: "item" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "bridge", x: "item", y: "change", title: "Profit (thousand euros)", children: [waterfall({ total: "total" })] }, at("default")),
      plot({ data: "bridge", x: "item", y: "change", title: "Profit", prefix: "€", suffix: "k", children: [waterfall({ total: "total", prefix: "€", suffix: "k" })] }, at("affixes")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "waterfall({ total: \"total\" })", text: "The first bar starts at zero; rises in green, falls in red; the total row sums them." }),
      step("affixes", { title: "prefix: \"€\", suffix: \"k\"", text: "Labels (and the plot's axis) with a currency and a unit." }),
    ],
  }),
});
