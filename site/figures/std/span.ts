// span: a shaded band between two values on the x (or y) scale — a period, a target range —
// behind a plot's marks. A lake's water level through a year with a dry summer.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, line, span } from "@datars/std";

// Weekly levels (m): a seasonal swing, and a dip around week 29.
const level = Array.from({ length: 52 }, (_, i) =>
  Math.round((2.5 + 0.25 * Math.cos(((i - 15) / 52) * 2 * Math.PI) - 0.45 * Math.exp(-(((i - 28) / 3) ** 2))) * 100) / 100);
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "lake", x: "week", y: "m", xType: "linear", xDomain: [1, 52], zero: false, xLabel: "Week", title: "Water level of the lake (m)" } as const;

export default doc({
  title: "A lake's water level through a dry summer",
  description: "A year of weekly water levels with the dry weeks shaded, then the shading labelled, then a second band across the y axis.",
  size: [640, 320],
  data: { lake: data.values({ week: level.map((_, i) => i + 1), m: level }, { key: "week" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [span({ from: 25, to: 33 }), line()] }, at("default")),
      plot({ ...frame, children: [span({ from: 25, to: 33, label: "Dry spell" }), line()] }, at("label")),
      plot({ ...frame, children: [span({ from: 25, to: 33, label: "Dry spell" }),
        span({ axis: "y", from: 2.2, to: 2.7, label: "Normal range", ink: "$accent", opacity: 0.1 }), line()] }, at("axis")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "span({ from: 25, to: 33 })", text: "Weeks 25 to 33 shaded behind the line." }),
      step("label", { title: "label: \"Dry spell\"", text: "A label in the band's top-left corner." }),
      step("axis", { title: "axis: \"y\"", text: "A band across the y axis: the normal range, in the accent ink." }),
    ],
  }),
});
