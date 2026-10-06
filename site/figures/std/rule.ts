// rule: a reference line at a value on the y (or x) scale — a target, an average, a date — drawn
// inside a plot over its marks. Two weeks of steps walked a day, against a daily goal.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, bar, rule } from "@datars/std";

const steps = [6200, 7400, 8100, 5300, 9100, 15200, 4800, 7000, 8600, 7900, 6600, 9800, 5600, 5100];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "walks", x: "day", y: "steps", xLabel: "Day in June", title: "Steps walked a day" } as const;

export default doc({
  title: "Steps walked a day against a daily goal",
  description: "Two weeks of daily steps as bars with a dashed goal line, then the line labelled, then a second rule across the x axis.",
  size: [640, 320],
  data: { walks: data.values({ day: steps.map((_, i) => i + 1), steps }, { key: "day" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [bar(), rule({ value: 8000 })] }, at("default")),
      plot({ ...frame, children: [bar(), rule({ value: 8000, label: "Goal: 8,000" })] }, at("label")),
      plot({ ...frame, children: [bar(), rule({ value: 8000, label: "Goal: 8,000" }),
        rule({ axis: "x", value: 6, label: "Hike", dashed: false, ink: "$ink" })] }, at("axis")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "rule({ value: 8000 })", text: "A dashed line across the plot at 8,000 steps." }),
      step("label", { title: "label: \"Goal: 8,000\"", text: "A label at the line's right end." }),
      step("axis", { title: "axis: \"x\"", text: "A solid rule at a day on the x axis, in full ink." }),
    ],
  }),
});
