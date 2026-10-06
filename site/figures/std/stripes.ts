// stripes: one full-height band per year, coloured on the theme's diverging scale around `mid`.
// Moving `mid` recolours the same keyed stripes.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { stripes, title } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Yearly temperature against the 1951–1980 average (°C), for an illustrative inland town.
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1; // a fixed hash
const year = Array.from({ length: 75 }, (_, i) => 1950 + i);
const anomaly = year.map((_, i) => Math.round((0.0004 * Math.max(0, i - 25) ** 2 + 0.5 * (u(i, 1) + u(i, 2) - 1)) * 100) / 100);

export default doc({
  title: "Warming stripes for an inland town",
  description: "75 years of temperature as colour stripes around the 1951–1980 average, then around the average of the last 30 years.",
  size: [640, 320],
  data: { temps: data.values({ year, anomaly }, { key: "year" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 12, padding: [16, 20, 12, 12] },
    children: [
      title({ text: "Yearly temperature, 1950–2024", subtitle: "Blue: colder than the baseline; red: warmer." }, { size: { h: "auto" } }),
      group({ key: "body", children: [
        stripes({ data: "temps", x: "year", value: "anomaly" }, at("default")),
        stripes({ data: "temps", x: "year", value: "anomaly", mid: 0.5 }, at("mid")),
      ] }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "stripes()", text: "Centred on zero: the 1951–1980 average." }),
      step("mid", { title: "mid: 0.5", text: "Centred on the last 30 years' average: most years before 1990 turn blue." }),
    ],
  }),
});
