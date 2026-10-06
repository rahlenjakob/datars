// An explorable: drag the income slider and every category's amount follows. The slider is drawn by
// the engine (the same on web, iOS, Android and in video), the bars read the signal in their
// expressions — no host code. Illustrative shares, not official statistics.
import { doc, data, group, interactive, signal, e } from "@datars/sdk";
import { plot, bar, slider } from "@datars/std";

export default doc({
  id: "budget",
  title: "Where a monthly income goes",
  size: [720, 440],
  data: {
    shares: data.values({ category: ["Housing", "Food", "Transport", "Savings", "Leisure", "Other"], share: [32, 16, 12, 15, 13, 12] }, { key: "category" }),
  },
  tables: { amounts: { from: "shares", ops: [{ op: "derive", as: "amount", expr: { expr: "d.share * income / 100" } }] } },
  signals: { income: signal.num(32000) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] },
    children: [
      slider({ signal: "income", min: 10000, max: 80000, step: 1000, label: "Monthly income (SEK)", format: ",.0f" }, { size: { h: 44 } }),
      plot({ data: "amounts", x: "amount", y: "category", xType: "linear", yType: "band", xDomain: [0, 26000], title: "Per month (SEK)", format: ",.0f",
        children: [bar({ labels: true, format: ",.0f", fill: "$accent" })] }, { key: "chart" }),
    ],
  }),
  program: interactive(),
});
