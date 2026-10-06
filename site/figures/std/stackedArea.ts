// stackedArea: each series a layer on the ones below it — here a fictional country's electricity by
// source. The same rows, keyed (source, year), as totals, as shares and as a streamgraph.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, stackedArea } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const years = Array.from({ length: 13 }, (_, i) => 2000 + 2 * i);
const twh: Record<string, (i: number) => number> = {
  Coal: (i) => 60 - 4.2 * i,
  Gas: (i) => 30 + 3 * i - 0.25 * i * i,
  Nuclear: (i) => 40 - (i > 8 ? 3 * (i - 8) : 0),
  Hydro: (i) => 22 + (i % 3) - 1,
  Wind: (i) => 1 + 0.45 * i * i,
  Solar: (i) => 0.2 + 0.18 * i * i * (i > 4 ? 1.2 : 0.6),
};
const rows = Object.entries(twh).flatMap(([source, f]) => years.map((year, i) => ({ source, year, twh: Math.round(Math.max(0, f(i)) * 10) / 10 })));
const frame = { data: "power", x: "year", y: "twh", xType: "linear", xFormat: "d", color: "source", legend: true } as const;

export default doc({
  title: "Where a country's electricity comes from",
  description: "Electricity generation by source from 2000 to 2024 as stacked areas, then stretched to 100 %, then as a streamgraph labelled inside.",
  size: [640, 340],
  data: { power: data.values(rows, { key: ["source", "year"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, title: "Electricity generated (TWh)", children: [stackedArea()] }, at("default")),
      plot({ ...frame, title: "Share of electricity generated", children: [stackedArea({ offset: "expand" })] }, at("expand")),
      plot({ ...frame, title: "Electricity generated, as a stream", axes: "x", grid: false, legend: false, children: [stackedArea({ offset: "wiggle", order: "inside-out", labels: true })] }, at("stream")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "stackedArea()", text: "Each source a layer on the ones below: the top edge is the total." }),
      step("expand", { title: "offset: \"expand\"", text: "Every year stretched to 100 %: coal's share shrinks as wind and solar grow." }),
      step("stream", { title: "offset: \"wiggle\", order: \"inside-out\"", text: "A streamgraph: layers flow around a moving centre, each named where it is thickest." }),
    ],
  }),
});
