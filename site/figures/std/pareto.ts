// pareto: the running share of the total, largest category first, as a line on the plot's right
// axis — over bars sorted from largest to smallest, a Pareto chart.
import { doc, data, group, story, step } from "@datars/sdk";
import { plot, bar, pareto } from "@datars/std";

export default doc({
  title: "Why bikes came back to a repair shop",
  description: "Repeat repairs by cause, largest first, with the running share of all repairs on the right axis: the first three causes make up almost 80%.",
  size: [640, 320],
  data: {
    repairs: data.values({ cause: ["Punctures", "Brakes", "Gears", "Chain", "Lights", "Spokes", "Other"], count: [58, 34, 27, 14, 9, 6, 4] }, { key: "cause" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "repairs", x: "cause", y: "count", title: "Repeat repairs by cause, last year",
        right: { domain: [0, 1], format: ".0%" },
        children: [bar(), pareto()],
      }, { key: "chart" }),
    ],
  }),
  program: story({
    steps: [step("default", { title: "pareto()", text: "The line climbs by each cause's share; the right axis reads it as a percentage." })],
  }),
});
