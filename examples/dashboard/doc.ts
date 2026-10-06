// A small cross-filter dashboard: click a region's bar to filter the trend. Signals are global, so
// views link without extra machinery (docs/08-programs.md).
import { doc, data, e, group, signal, interactive } from "@datars/sdk";
import { plot, bar, line } from "@datars/std";

const regions = ["North", "South", "East", "West"];
const months = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const region: string[] = [], month: number[] = [], sales: number[] = [];
regions.forEach((r, ri) => months.forEach((m) => { region.push(r); month.push(m); sales.push(Math.round(40 + ri * 12 + m * (3 + ri) + (m % 3) * 5)); }));

export default doc({
  id: "dashboard",
  title: "Sales by region",
  size: [860, 420],
  data: { sales: data.values({ region, month, sales }, { key: ["region", "month"] }) },
  tables: {
    totals: { from: "sales", ops: [{ op: "aggregate", groupby: ["region"], ops: [{ op: "sum", field: "sales", as: "total" }] }] },
    trend: { from: "sales", ops: [{ op: "filter", expr: { expr: "selected.isEmpty() || selected.has(d.region)" } }] },
  },
  signals: { selected: signal.keyset() },
  scene: group({
    key: "root",
    // Side by side; one above the other in a phone-narrow box.
    layout: { type: "columns", gap: 24, padding: [16, 20, 12, 12], wrap: 560 },
    children: [
      plot({ data: "totals", x: "region", y: "total", color: "region", title: "Total (click to filter)", children: [bar({ selected: "selected" })] }, { key: "totals", size: { w: "40%" } }),
      plot({ data: "trend", x: "month", y: "sales", xType: "linear", color: "region", title: "Monthly sales", zero: false, children: [line({ points: true })] }, { key: "trend" }),
    ],
  }),
  program: interactive(),
});
