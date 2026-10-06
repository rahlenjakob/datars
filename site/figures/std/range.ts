// range: two thumbs for a low and a high signal — here the years the bars keep. The thumbs can't
// cross; drag either end.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, range } from "@datars/std";

const years = Array.from({ length: 21 }, (_, i) => 2004 + i);
const tonnes = [5.9, 6.0, 6.2, 6.1, 5.8, 5.5, 5.7, 5.4, 5.2, 5.0, 4.9, 4.7, 4.6, 4.4, 4.3, 4.1, 3.7, 3.9, 3.8, 3.6, 3.5];

export default doc({
  title: "Emissions per person (tonnes CO₂)",
  description: "A range picks the years; bars outside it fade.",
  size: [640, 320],
  data: { em: data.values({ year: years, t: tonnes }, { key: "year" }) },
  signals: { from: signal.num(2008), to: signal.num(2018) },
  scene: group({ key: "root", layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] }, children: [
    range({ lo: "from", hi: "to", min: 2004, max: 2024, step: 1, label: "Years", format: "d" }, { key: "years", size: { h: 44 } }),
    plot({ data: "em", x: "year", y: "t", xType: "band", yDomain: [0, 7], children: [bar({ fill: e("d.year >= from && d.year <= to ? \"$accent\" : \"$muted@0.4\"") })] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("decade", { set: { from: 2008, to: 2018 }, title: "from = 2008, to = 2018" }),
    step("recent", { set: { from: 2015, to: 2024 }, title: "from = 2015, to = 2024" }),
  ] }),
});
