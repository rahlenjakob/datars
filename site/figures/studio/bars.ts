// Theme studio: grouped bars — the categorical palette, bar corners, the gaps between bars, the
// title and label type. Every colour and size here is a theme token. Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { plot, grouped } from "@datars/std";

const quarters = ["Q1", "Q2", "Q3", "Q4"];
const sold: Record<string, number[]> = { Maps: [24, 33, 32, 41], Stories: [35, 37, 49, 51], Dashboards: [36, 51, 56, 71] };
const rows = Object.entries(sold).flatMap(([product, n]) => n.map((revenue, i) => ({ product, quarter: quarters[i], revenue })));

export default doc({
  title: "Revenue by product",
  description: "Quarterly revenue of three products as grouped bars, with a legend.",
  size: [480, 330],
  data: { sales: data.values(rows, { key: ["product", "quarter"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 16, 10, 10] },
    children: [
      plot({ data: "sales", x: "quarter", y: "revenue", color: "product", legend: true, title: "Revenue by product (M)", children: [grouped()] }, { key: "chart" }),
    ],
  }),
});
