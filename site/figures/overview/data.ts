// Data from anywhere: a slot the page fills with `provideData()` — a rolling window of the last
// twenty readings, a new one every couple of seconds, so the axis rescales as it slides.
// Illustrative figures.
import { data, doc, group, interactive } from "@datars/sdk";
import { line, plot } from "@datars/std";
import { PAD, SIZE } from "./_kit";

const t = Array.from({ length: 20 }, (_, i) => i + 1);
const v = t.map((i) => Math.round(60 + 18 * Math.sin(i / 3) + 6 * Math.sin(i * 1.7)));

export default doc({
  id: "overview-data",
  title: "Requests per second, live",
  description: "A line of the last twenty readings of requests per second, refreshed as new readings arrive.",
  size: SIZE,
  data: { feed: data.slot("feed", { key: "t", sample: { t, v } }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ data: "feed", x: "t", y: "v", xType: "linear", zero: false, children: [line({ points: true })] }, { key: "chart" }),
  ] }),
  program: interactive(),
});
