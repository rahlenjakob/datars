// Theme studio: a donut with its total in the middle — the categorical palette in slices, and the
// number type. Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { pie, title } from "@datars/std";

export default doc({
  title: "Where readers come from",
  description: "Six traffic sources as a donut, with the total in the middle.",
  size: [480, 330],
  data: { sources: data.values({ source: ["Search", "Social", "Direct", "Newsletter", "Partners", "Other"], visits: [420, 310, 260, 180, 120, 70] }, { key: "source" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 4, padding: [14, 16, 10, 12] },
    children: [
      title({ text: "Visits by source (k)" }, { size: { h: "auto" } }),
      pie({ data: "sources", value: "visits", category: "source", inner: 0.58, total: true, format: ",.0f", sort: "desc" }, { key: "chart" }),
    ],
  }),
});
