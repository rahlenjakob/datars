// Theme studio: a profit bridge — rises in `positive`, falls in `negative`, the total in the mark
// colour. Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { plot, waterfall } from "@datars/std";

export default doc({
  title: "Profit, last year to this",
  description: "A profit bridge from last year to this year through four changes: two rises and two falls.",
  size: [480, 330],
  data: {
    bridge: data.values({
      item: ["2025", "Guests", "Prices", "Rent", "Staff", "2026"],
      change: [84, 22, 9, -12, -18, 85],
      total: [false, false, false, false, false, true],
    }, { key: "item" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 16, 10, 10] },
    children: [plot({ data: "bridge", x: "item", y: "change", title: "Profit (k€)", children: [waterfall({ total: "total" })] }, { key: "chart" })],
  }),
});
