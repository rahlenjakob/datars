// boxplot: a box per category over the middle half of its values, a line at the median, whiskers
// to the furthest values within 1.5 box-heights and the rest drawn as outliers. Commutes by how
// people travel; the boxes read the plot's scales like any mark.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, boxplot } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Minutes door to door, from a fixed hash (the same numbers every build), and a few very long trips.
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const modes = ["Bike", "Car", "Bus", "Train"];
const mean = { Bike: 17, Car: 26, Bus: 34, Train: 44 } as Record<string, number>;
const person = Array.from({ length: 240 }, (_, i) => `#${i + 1}`);
const mode = person.map((_, i) => modes[i % 4]);
const minutes = person.map((_, i) => Math.max(4, Math.round(mean[mode[i]] + (mode[i] === "Bike" ? 6 : 12) * (u(i, 1) + u(i, 2) + u(i, 3) - 1.5) + (i % 37 === 5 ? 30 : 0))));
const frame = { data: "commutes", title: "Time to get to work (minutes)" } as const;

export default doc({
  title: "Commuting times by how people travel",
  description: "240 commutes as a box plot per mode of travel, then coloured by mode, then turned horizontal.",
  size: [640, 320],
  data: { commutes: data.values({ person, mode, minutes }, { key: "person" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, x: "mode", y: "minutes", children: [boxplot()] }, at("default")),
      plot({ ...frame, x: "mode", y: "minutes", color: "mode", children: [boxplot()] }, at("color")),
      plot({ ...frame, x: "minutes", y: "mode", xType: "linear", yType: "band", color: "mode", children: [boxplot()] }, at("horizontal")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "boxplot()", text: "Half of each group's trips fall inside its box; the line is the median, the circles are unusually long trips." }),
      step("color", { title: "color: \"mode\"", text: "The boxes take the plot's colour scale." }),
      step("horizontal", { title: "yType: \"band\"", text: "Categories up the side: the same boxes, laid along x." }),
    ],
  }),
});
