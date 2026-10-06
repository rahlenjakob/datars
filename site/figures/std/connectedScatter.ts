// connectedScatter: points at (x, y) joined in time order — how two measures moved together. A
// fictional country's unemployment and inflation, year by year, with an arrow at the end.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, connectedScatter } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const year = Array.from({ length: 15 }, (_, i) => 2010 + i);
const unemployment = [8.1, 7.8, 7.4, 7.6, 7.1, 6.5, 6.0, 5.6, 5.1, 4.9, 7.9, 6.8, 5.2, 4.8, 5.3];
const inflation = [1.6, 2.9, 2.1, 1.4, 0.4, 0.1, 0.9, 1.9, 2.3, 1.8, 0.6, 3.4, 8.9, 5.7, 2.6];
const other = [6.2, 6.4, 6.9, 7.3, 7.0, 6.6, 6.1, 5.8, 5.5, 5.4, 6.9, 6.3, 5.9, 6.0, 6.1];
const other2 = [2.0, 2.7, 2.4, 1.6, 0.8, 0.3, 0.5, 1.4, 1.9, 1.5, 0.3, 2.6, 7.4, 6.0, 3.1];
const rows = [
  ...year.map((y, i) => ({ country: "Northland", year: y, unemployment: unemployment[i], inflation: inflation[i] })),
  ...year.map((y, i) => ({ country: "Southland", year: y, unemployment: other[i], inflation: other2[i] })),
];
const frame = { x: "unemployment", y: "inflation", xType: "linear", title: "Inflation against unemployment (%), 2010–2024", xLabel: "Unemployment", yLabel: "Inflation" } as const;

export default doc({
  title: "Unemployment and inflation, year by year",
  description: "A fictional country's unemployment and inflation from 2010 to 2024 as a connected scatter plot labelled at its ends, then every third year, then beside a neighbour.",
  size: [640, 380],
  data: {
    economy: data.values(rows, { key: ["country", "year"] }),
    north: data.values(rows.filter((r) => r.country === "Northland"), { key: "year" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, data: "north", children: [connectedScatter({ order: "year" })] }, at("default")),
      plot({ ...frame, data: "north", children: [connectedScatter({ order: "year", every: 3 })] }, at("every")),
      plot({ ...frame, data: "economy", color: "country", legend: true, children: [connectedScatter({ order: "year" })] }, at("series")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "connectedScatter({ order: \"year\" })", text: "One point a year, joined in order: the arrow ends in 2024." }),
      step("every", { title: "every: 3", text: "Every third year labelled too; labels that would collide give way." }),
      step("series", { title: "color: \"country\"", text: "A path per country, on the plot's colour scale." }),
    ],
  }),
});
