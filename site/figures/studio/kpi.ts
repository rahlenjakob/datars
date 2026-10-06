// Theme studio: two key figures on the theme's card surface — the card's fill, line, corner radius
// and inks, and positive and negative for whether a change is good. Illustrative figures.
import { doc, data, e, geom, group, shape, type Template } from "@datars/sdk";
import { kpi } from "@datars/std";

const months = ["2025-10", "2025-11", "2025-12", "2026-01", "2026-02", "2026-03", "2026-04", "2026-05", "2026-06", "2026-07", "2026-08", "2026-09"].map((m) => `${m}-01`);
const readers = [3.1, 3.2, 3.6, 3.3, 3.4, 3.5, 3.6, 3.8, 3.7, 3.9, 4.0, 4.2];
const churn = [0.052, 0.049, 0.061, 0.058, 0.055, 0.051, 0.05, 0.047, 0.049, 0.046, 0.048, 0.041];

// A tile: the card surface (its radius a token too), the figure inside it.
const tile = (key: string, inside: Template) => group({ key, children: [
  shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: e('token("radius.card")') }), { key: "card", fill: "$card", stroke: { paint: "$card-line", width: 1 } }),
  group({ key: "inside", layout: { type: "stack", padding: 14 }, children: [inside] }),
] });

export default doc({
  title: "Readers and churn this month",
  description: "Two key figures on cards: readers, up on last month, and churn, down — which is good — each with a year's sparkline.",
  size: [480, 330],
  data: { year: data.values({ month: months, readers, churn }, { key: "month", types: { month: "date" } }) },
  scene: group({
    key: "root",
    layout: { type: "columns", gap: 12, padding: [14, 14, 14, 14], wrap: 300 },
    children: [
      tile("readers", kpi({ label: "Readers", data: "year", x: "month", y: "readers", format: ".1f", suffix: "M", compareLabel: "vs August" })),
      tile("churn", kpi({ label: "Churn", data: "year", x: "month", y: "churn", format: ".1%", better: "down", compareLabel: "vs August" })),
    ],
  }),
});
