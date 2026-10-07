// The data page's lab (site/pages/features/data.html, features-data.js): a chart published once
// with a data slot, and rows the reader brings — pasted, dropped or picked — handed to it on the
// page with `provideData`. The engine parses the CSV (delimiters, decimal commas, missing-value
// markers), checks the rows against the slot, and animates the bars to the new rows by key: a
// changed number moves its bar, a new row grows in, a deleted one leaves.
import { doc, data, e, group, op, signal, text } from "@datars/sdk";
import { bar, plot } from "@datars/std";

// Until the page hands rows in: the Nordic capitals, people in the municipality (2023, rounded).
const sample = {
  label: ["Stockholm", "Oslo", "Helsinki", "Copenhagen", "Reykjavík"],
  value: [984_748, 709_037, 664_028, 653_664, 139_875],
};

export default doc({
  title: "Your rows, as bars",
  description: "A bar chart published with a data slot: it shows the Nordic capitals' populations until the page hands it other rows, then ranks the twenty largest values, each bar keyed by its label.",
  size: [640, 460],
  data: {
    rows: data.slot("rows", { key: "label", types: { label: "str", value: "num" }, sample }),
  },
  tables: {
    // The twenty largest, ranked: a new value re-ranks the bars, keyed by label.
    ranked: { from: "rows", ops: [op.filter(e("d.value != null")), op.top(20, "value"), op.sort(["value", "desc"])] },
  },
  signals: { heading: signal.str("Population of the Nordic capitals") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 18, 10, 10] },
    children: [
      text(e("heading"), [0, 0], { key: "heading", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } }),
      text(e('`${table.count("rows")} rows, keyed by their label${table.count("rows") > table.count("ranked") ? ` — the largest ${table.count("ranked")} shown` : ""}`'), [0, 0], { key: "sub", size: { h: "auto" }, style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
      plot({ data: "ranked", x: "value", y: "label", xType: "linear", yType: "band", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart" }),
    ],
  }),
});
