// axis: tick labels, tick marks and a domain line for a scale. A plot draws its two axes with it
// and passes these settings through; labels are measured so they never collide.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, bar } from "@datars/std";

// Metropolitan populations, rounded.
const city = ["São Paulo", "Buenos Aires", "Rio de Janeiro", "Bogotá", "Lima", "Santiago"];
const people = [22_400_000, 15_600_000, 13_700_000, 11_500_000, 11_200_000, 6_900_000];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const chart = { data: "metros", x: "city", y: "people", title: "Largest cities of South America", children: [bar()] };

export default doc({
  title: "Largest cities of South America",
  description: "The same bars with the axes' default labels, then short number formats and a suffix, then fewer ticks and an axis title.",
  size: [640, 320],
  data: { metros: data.values({ city, people }, { key: "city" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot(chart, at("default")),
      plot({ ...chart, format: ".2~s", suffix: " people" }, at("format")),
      plot({ ...chart, format: ".2~s", yTicks: 3, yLabel: "Metropolitan population" }, at("ticks")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "axis()", text: "Numbers in full, a name under every bar; ticks as many as fit." }),
      step("format", { title: "format: \".2~s\", suffix", text: "A d3 number format and a suffix on every value label." }),
      step("ticks", { title: "yTicks: 3, yLabel", text: "A tick count hint, and a title above the value axis." }),
    ],
  }),
});
