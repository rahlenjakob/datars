// slider: an engine-drawn control that sets a numeric signal — here the yearly return, which the
// bars read through a derived table. Drag it; the steps set it to three values.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, bar, slider } from "@datars/std";

export default doc({
  title: "What 1,000 grows to",
  description: "A slider sets the yearly return; the bars show what 1,000 saved grows to at 2, 4 and 7 percent.",
  size: [640, 320],
  data: { years: data.values({ year: [0, 5, 10, 15, 20, 25, 30] }, { key: "year" }) },
  // Every bar reads the signal: the balance after `year` years at `rate` percent.
  tables: { grown: { from: "years", ops: [op.derive("balance", e("1000 * pow(1 + rate / 100, d.year)"))] } },
  signals: { rate: signal.num(2) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] },
    children: [
      slider({ signal: "rate", min: 0, max: 8, step: 0.5, label: "Yearly return (%)", format: ".1f" }, { key: "control", size: { h: 44 } }),
      plot({ data: "grown", x: "year", y: "balance", yDomain: [0, 12000], format: ",.0f", title: "1,000 saved, after each five years", xLabel: "Years",
        children: [bar({ labels: true, format: ",.0f", fill: "$accent" })] }, { key: "chart" }),
    ],
  }),
  program: story({
    steps: [
      step("low", { set: { rate: 2 }, title: "rate = 2", text: "The thumb and every bar follow the signal." }),
      step("middle", { set: { rate: 4 }, title: "rate = 4", text: "Labels count to their new values." }),
      step("high", { set: { rate: 7 }, title: "rate = 7", text: "After thirty years, more than seven times the start." }),
    ],
  }),
});
