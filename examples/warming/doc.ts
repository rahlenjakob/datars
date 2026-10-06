// Global temperature anomaly (°C vs 1951–1980) — as a line, as stripes, as a heatmap of decades.
// NOTE: approximate annual values in the shape of NASA GISTEMP (illustrative, rounded).
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, line, area, rule, stripes, span } from "@datars/std";

const years: number[] = [], anomaly: number[] = [];
for (let y = 1880; y <= 2024; y++) {
  years.push(y);
  const t = (y - 1880) / 144;
  const wobble = Math.sin(y * 0.9) * 0.08 + Math.sin(y * 0.37) * 0.06;
  anomaly.push(Math.round((-0.25 + 0.05 * t + Math.pow(Math.max(0, t - 0.55), 2) * 5.2 + wobble) * 100) / 100);
}

export default doc({
  id: "warming",
  title: "A warming world",
  size: [760, 400],
  data: { temps: data.values({ year: years, anomaly }, { key: "year" }) },
  signals: { view: signal.str("line") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 24, 12, 12] },
    children: [
      plot({
        data: "temps", x: "year", y: "anomaly", xType: "linear", zero: false, title: "Temperature anomaly, °C", format: "+.1f",
        children: [span({ axis: "x", from: 1951, to: 1980, label: "Baseline" }), rule({ axis: "y", value: 0, dashed: false }), area({ opacity: 0.18 }), line({ width: 1.8 })],
      }, { key: "chart", when: e('view == "line"') }),
      stripes({ data: "temps", x: "year", value: "anomaly" }, { key: "chart", when: e('view == "stripes"') }),
    ],
  }),
  program: story({ steps: [step("line", { set: { view: "line" } }), step("stripes", { set: { view: "stripes" }, text: "Each year a stripe: blue cooler, red warmer." })] }),
});
