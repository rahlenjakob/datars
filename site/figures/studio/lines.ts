// Theme studio: lines labelled at their ends, a dot per month — line width, point size, the
// categorical palette and the body type. Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { plot, line } from "@datars/std";

const platforms = ["Web", "iOS", "Android"];
const rows = platforms.flatMap((device, di) => Array.from({ length: 12 }, (_, m) => ({
  device,
  month: m + 1,
  readers: Math.round((62 - di * 16) * (1 + (m + 1) * (0.04 + di * 0.014)) + 4 * Math.sin((m + di * 2) * 0.9)),
})));

export default doc({
  title: "Monthly readers by platform",
  description: "Readers per month on the web, iOS and Android as three lines, labelled at their ends.",
  size: [480, 330],
  data: { readers: data.values(rows, { key: ["device", "month"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 16, 10, 10] },
    children: [
      plot({ data: "readers", x: "month", y: "readers", xType: "linear", color: "device", zero: false, labelSpace: 64, title: "Monthly readers (k)",
        children: [line({ labels: true, points: true })] }, { key: "chart" }),
    ],
  }),
});
