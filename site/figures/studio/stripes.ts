// Theme studio: warming stripes — the diverging ramp, one band per year around the baseline.
// Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { stripes, title } from "@datars/std";

// Yearly temperature against the 1951–1980 average (°C), for an illustrative inland town.
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1; // a fixed hash
const year = Array.from({ length: 75 }, (_, i) => 1950 + i);
const anomaly = year.map((_, i) => Math.round((0.0004 * Math.max(0, i - 25) ** 2 + 0.5 * (u(i, 1) + u(i, 2) - 1)) * 100) / 100);

export default doc({
  title: "Warming stripes",
  description: "75 years of temperature as colour stripes around the 1951–1980 average, on the diverging ramp.",
  size: [960, 260],
  data: { temps: data.values({ year, anomaly }, { key: "year" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [14, 16, 12, 12] },
    children: [
      title({ text: "Yearly temperature, 1950–2024", subtitle: "Colder than the 1951–1980 average at one end of the ramp, warmer at the other." }, { size: { h: "auto" } }),
      stripes({ data: "temps", x: "year", value: "anomaly" }, { key: "chart" }),
    ],
  }),
});
