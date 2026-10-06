// Every std control on one chart: a segmented quarter, a dropdown highlight, a switch for an
// average line, checkboxes for the regions, a range of values and a button that puts every region
// back.
// All of them are engine-drawn signals the bars read; none is page code.
import { doc, data, e, group, interactive, op, signal } from "@datars/sdk";
import { plot, bar, button, checklist, range, rule, segmented, select, toggle } from "@datars/std";

const regions = ["North", "South", "East", "West", "Central"];

export default doc({
  title: "Sales by region",
  description: "Every control in the standard library, drawn by the engine, setting the signals the bars read.",
  size: [760, 470],
  data: { sales: data.values({ region: regions, q1: [42, 31, 18, 27, 36], q2: [38, 35, 22, 30, 41] }, { key: "region" }) },
  signals: {
    quarter: signal.str("q1"), region: signal.str("Central"), average: signal.bool(false),
    shown: signal.keyset(regions), lo: signal.num(0), hi: signal.num(50),
  },
  tables: {
    view: { from: "sales", ops: [
      op.derive("value", e('quarter == "q1" ? d.q1 : d.q2')),
      op.filter(e("shown.has(d.region) && d.value >= lo && d.value <= hi")),
    ] },
  },
  scene: group({ key: "root", layout: { type: "columns", gap: 28, padding: [18, 20, 14, 16], wrap: 560 }, children: [
    group({ key: "panel", size: { w: 236 }, layout: { type: "rows", gap: 14 }, children: [
      segmented({ signal: "quarter", options: ["q1", "q2"], labels: ["Q1", "Q2"], label: "Quarter" }, { key: "quarter" }),
      select({ signal: "region", options: regions, label: "Highlight" }, { key: "region" }),
      toggle({ signal: "average", label: "Average line" }, { key: "average" }),
      checklist({ signal: "shown", options: regions, label: "Regions" }, { key: "shown" }),
      range({ lo: "lo", hi: "hi", min: 0, max: 50, step: 1, label: "Sales between" }, { key: "range" }),
      button({ label: "Show every region", set: "shown", value: regions }, { key: "reset" }),
    ] }),
    plot({ data: "view", x: "region", y: "value", yDomain: [0, 50], title: "Sales (thousands)", children: [
      bar({ labels: true, fill: e('d.region == region ? "$accent" : "$muted@0.4"') }),
      // The mean of the bars shown, while the switch is on.
      rule({ value: e('table.mean("view", "value")'), label: "Average" }, { key: "average", when: e("average") }),
    ] }, { key: "chart" }),
  ] }),
  program: interactive(),
});
