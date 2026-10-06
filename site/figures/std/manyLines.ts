// manyLines: 1,000 generated weather stations, a year of weekly mean temperatures each — 52,000
// rows as 1,000 faint lines whose bulk shows where most stations run, with the ones the story is
// about in the accent colour. Hover a line for its station.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, manyLines } from "@datars/std";

// Seeded draws: each station its own mean, seasonal swing and week-to-week noise.
const weeks = data.generate(52_000, {
  s: e("floor(d.i / 52)"),
  week: e("d.i % 52 + 1"),
  station: e('"Station " + (d.s + 1)'),
  temp: e("round(9 + randn(d.s, 1) * 5 - (7 + rand(d.s, 2) * 7) * cos(6.2832 * (d.week - 3) / 52) + randn(d.i, 3) * 1.6, 1)"),
}, { keep: ["station", "week", "temp"], key: ["station", "week"] });

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "weeks", x: "week", y: "temp", xType: "linear", zero: false, xDomain: [1, 52], title: "A year at 1,000 weather stations", xLabel: "week", yLabel: "weekly mean, °C", labelSpace: 90 } as const;
const common = { series: "station", name: "1,000 weather stations" };

export default doc({
  title: "1,000 stations at once",
  description: "A year of weekly mean temperatures at a thousand generated weather stations, each a faint line; then one station highlighted and labelled; then three.",
  size: [640, 400],
  data: { weeks },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [manyLines(common)] }, at("default")),
      plot({ ...frame, children: [manyLines({ ...common, highlight: "Station 417" })] }, at("one")),
      plot({ ...frame, children: [manyLines({ ...common, highlight: ["Station 417", "Station 88", "Station 902"] })] }, at("three")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "manyLines({ series: \"station\" })", text: "One faint path per station: where they overlap, the bulk darkens. Hover a line for its station." }),
      step("one", { title: "highlight: \"Station 417\"", text: "One station forward, in the accent colour, named at its end." }),
      step("three", { title: "highlight: [ …three stations ]", text: "A list highlights several: each against the thousand." }),
    ],
  }),
});
