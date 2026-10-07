// One table, every chart that fits it: a hundred visitors by the way they came, as columns, a
// pareto, bars, lollipops, dots, a parliament, a pie, a donut, a treemap and a waffle. Every mark
// is keyed by its channel and every unit by (channel, unit i), so each chart morphs into the next:
// row to row, a row splitting into its units, units gathering into their row. (Units of one recipe
// don't pair with another recipe's units — the waffle and the parliament crossfade — so they never
// follow each other here.) The rows are a data slot: the charts page hands in the reader's own with
// `provideData`, so nothing in the chart names the example's columns (a header would), and the
// units are percentage points of the total (a reader's thousands would be thousands of squares).
import { doc, data, e, group, motion, op, story, step } from "@datars/sdk";
import { plot, bar, lollipop, dot, pie, treemap, waffle, hemicycle, legend, pareto } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const xy = { data: "visits", x: "channel", y: "visitors", color: "channel" };
const yx = { data: "visits", x: "visitors", y: "channel", xType: "linear", yType: "band", color: "channel" };

export default doc({
  title: "Where a hundred visitors came from",
  description: "One table of six channels and a hundred visitors, drawn by ten recipes in turn — columns, pareto, bars, lollipops, dots, parliament, pie, donut, treemap and waffle — each morphing into the next by key.",
  size: [640, 360],
  data: {
    visits: data.slot("visits", {
      key: "channel",
      sample: { channel: ["Search", "Social", "Direct", "Newsletter", "Referral", "Ads"], visitors: [38, 21, 17, 12, 8, 4] },
    }),
  },
  // Each channel's share of the total in whole percentage points: the units of the waffle and the
  // parliament.
  tables: { points: { from: "visits", ops: [op.window("share_of_total", "visitors", "share"), op.derive("points", e("round(d.share * 100)"))] } },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [14, 18, 10, 10] },
    // One colour per channel, the same in every chart.
    scales: { color: { type: "categorical", domain: { data: "visits", field: "channel" }, range: "$categorical" } },
    children: [
      group({ key: "body", children: [
        plot({ ...xy, children: [bar({ labels: true })] }, at("columns")),
        plot({ ...xy, right: { domain: [0, 1], format: ".0%" }, children: [bar(), pareto()] }, at("pareto")),
        plot({ ...yx, children: [bar({ labels: true })] }, at("bars")),
        plot({ ...yx, children: [lollipop({ labels: true })] }, at("lollipops")),
        plot({ ...yx, children: [dot({ r: 7 })] }, at("dots")),
        hemicycle({ data: "points", category: "channel", value: "points" }, at("parliament")),
        pie({ data: "visits", category: "channel", value: "visitors" }, at("pie")),
        pie({ data: "visits", category: "channel", value: "visitors", inner: 0.58, total: true }, at("donut")),
        treemap({ data: "visits", category: "channel", value: "visitors" }, at("treemap")),
        waffle({ data: "points", category: "channel", value: "points" }, at("waffle")),
      ] }),
      legend({ scale: "color" }, { size: { h: "auto" }, when: e('state == "waffle" || state == "parliament"') }),
    ],
  }),
  program: story({
    steps: [
      step("columns", { title: "bar()", text: "Columns: the plot's x is a band scale." }),
      step("pareto", { title: "pareto()", text: "The running share of the total, read on the right axis." }),
      step("bars", { title: "bar(), on its side", text: "Categories on the y axis: long labels read across." }),
      step("lollipops", { title: "lollipop()", text: "Less ink: a stem and a dot." }),
      step("dots", { title: "dot()", text: "Just the dot — values far from zero need no baseline." }),
      step("parliament", { title: "hemicycle()", text: "Each dot splits into its seats: a seat per percentage point." }),
      step("pie", { title: "pie()", text: "The seats gather into their slices." }),
      step("donut", { title: "pie({ inner: 0.58, total: true })", text: "A donut, the total in the hole." }),
      step("treemap", { title: "treemap()", text: "Area for share, labels where they fit." }),
      step("waffle", { title: "waffle()", text: "Each tile breaks into squares, a square per percentage point — and back into columns." }),
    ],
  }),
});
