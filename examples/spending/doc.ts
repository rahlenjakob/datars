// A banking app's spending chart. The chart ships inside the app (a `.datars` file, built once);
// each user's rows are handed in on the device (`data.slot`), so no data leaves the phone and no
// chart is built per user. The sample below shows until the app provides the real rows (previews,
// tests, the static fallback) and says which columns the app's data must have.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, bar, stacked, card } from "@datars/std";

const months = ["Apr", "May", "Jun", "Jul", "Aug", "Sep"];
const categories = ["Housing", "Food", "Transport", "Shopping", "Leisure"];
const base = [9800, 4200, 1600, 2300, 1900];
const sample = months.flatMap((month, m) => categories.map((category, c) => ({ month, category, amount: Math.round(base[c] * (1 + 0.12 * Math.sin(m * 1.3 + c))) })));

export default doc({
  id: "spending",
  title: "Your spending",
  size: [390, 520],
  data: {
    spending: data.slot("spending", { key: ["month", "category"], types: { amount: "num" }, sample }),
  },
  keys: {
    Housing: { color: "#4269d0" }, Food: { color: "#efb118" }, Transport: { color: "#6cc5b0" },
    Shopping: { color: "#ff725c" }, Leisure: { color: "#a463f2" },
  },
  tables: {
    byCategory: { from: "spending", ops: [op.aggregate(["category"], { amount: ["sum", "amount"] }), op.sort(["amount", "desc"])] },
  },
  signals: { view: signal.str("months") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 16, 12, 12] },
    children: [
      plot({ data: "spending", x: "month", y: "amount", color: "category", title: "Spending per month", legend: true, format: ",.0f", suffix: " kr",
        children: [stacked({ series: "category" })] }, { key: "chart", when: e('view == "months"') }),
      plot({ data: "byCategory", x: "amount", y: "category", xType: "linear", yType: "band", color: "category", title: "Where it went",
        children: [bar({ labels: true, format: ",.0f", suffix: " kr" })] }, { key: "chart", when: e('view == "categories"') }),
      // The card reads the user's own rows: the total changes with their data.
      card({ title: e('`${format(table.sum("spending", "amount"), ",.0f")} kr`'), text: e("narration.text"), width: 200 }, { key: "caption" }),
    ],
  }),
  program: story({
    steps: [
      step("months", { set: { view: "months" }, text: "spent over the last six months." }),
      step("categories", { set: { view: "categories" }, text: "Housing is the biggest share; each month's bars add up here." }),
    ],
  }),
});
