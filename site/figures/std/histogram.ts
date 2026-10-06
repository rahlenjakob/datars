// histogram: rows counted into bins of equal width, a bar per bin. How long 300 people in a town
// take to get to work — the plot's axes fit the bins by themselves.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, histogram } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Minutes door to door, from a fixed hash (the same numbers every build): bikes are quick, cars
// in between, the train takes longest.
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const modes = ["Car", "Train", "Bike"];
const mean = { Car: 26, Train: 44, Bike: 17 } as Record<string, number>;
const person = Array.from({ length: 300 }, (_, i) => `#${i + 1}`);
const mode = person.map((_, i) => modes[i % 5 === 0 ? 2 : i % 3 === 0 ? 1 : 0]);
const minutes = person.map((_, i) => Math.max(4, Math.round(mean[mode[i]] + 11 * (u(i, 1) + u(i, 2) + u(i, 3) - 1.5))));
const frame = { data: "commutes", x: "minutes", xType: "linear", title: "Time to get to work (minutes)", yLabel: "People" } as const;

export default doc({
  title: "How long people take to get to work",
  description: "300 commutes as a histogram in about ten bins, then in two-minute bins, then with each bin split by how people travel.",
  size: [640, 320],
  data: { commutes: data.values({ person, mode, minutes }, { key: "person" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [histogram()] }, at("default")),
      plot({ ...frame, children: [histogram({ step: 2 })] }, at("step")),
      plot({ ...frame, color: "mode", legend: true, children: [histogram({ step: 5 })] }, at("color")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "histogram()", text: "About ten bins at a round width: most people take 15 to 35 minutes." }),
      step("step", { title: "step: 2", text: "Two-minute bins show more of the shape — and more noise." }),
      step("color", { title: "color: \"mode\"", text: "Each five-minute bin stacked by how people travel: the slow tail is the train." }),
    ],
  }),
});
