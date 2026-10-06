// Charts for the boardroom: a waterfall bridge, a funnel, stacked revenue. (Illustrative figures.)
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, waterfall, funnel, stacked, legend } from "@datars/std";

export default doc({
  id: "business",
  title: "Charts for the boardroom",
  size: [760, 420],
  data: {
    bridge: data.values({ item: ["2023", "Price", "Volume", "Mix", "FX", "2024"], delta: [100, 12, 18, -6, -4, 120], total: [false, false, false, false, false, true] }, { key: "item" }),
    stages: data.values({ stage: ["Visits", "Sign-ups", "Trials", "Paid"], n: [12000, 3100, 1400, 520] }, { key: "stage" }),
    revenue: data.values({
      quarter: ["Q1", "Q1", "Q1", "Q2", "Q2", "Q2", "Q3", "Q3", "Q3", "Q4", "Q4", "Q4"],
      region: ["EU", "US", "APAC", "EU", "US", "APAC", "EU", "US", "APAC", "EU", "US", "APAC"],
      value: [30, 42, 12, 33, 45, 15, 31, 50, 19, 38, 55, 24],
    }, { key: ["region", "quarter"] }),
  },
  signals: { view: signal.str("bridge") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "bridge", x: "item", y: "delta", title: "Revenue bridge, 2023 → 2024", yDomain: { values: [0, 130] }, grid: true, children: [waterfall({ total: "total" })] }, { key: "chart", when: e('view == "bridge"') }),
      funnel({ data: "stages", stage: "stage", value: "n", title: "Sign-up funnel, last quarter" }, { key: "chart", when: e('view == "funnel"') }),
      plot({ data: "revenue", x: "quarter", y: "value", color: "region", title: "Revenue by region", yDomain: { values: [0, 130] }, legend: true, children: [stacked({ series: "region" })] }, { key: "chart", when: e('view == "stacked"') }),
    ],
  }),
  program: story({ steps: ["bridge", "funnel", "stacked"].map((s) => step(s, { set: { view: s } })) }),
});
