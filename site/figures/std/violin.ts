// violin: each category's kernel density mirrored around its centre — where values bunch, not
// just five numbers — with a bar over the middle half and a dot at the median.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, violin } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Minutes door to door, from a fixed hash (the same numbers every build). Buses are either quick
// (a direct line) or slow (a change): two humps a box plot would hide.
const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const modes = ["Bike", "Car", "Bus", "Train"];
const person = Array.from({ length: 320 }, (_, i) => `#${i + 1}`);
const mode = person.map((_, i) => modes[i % 4]);
const noise = (i: number) => u(i, 1) + u(i, 2) + u(i, 3) - 1.5;
const minutes = person.map((_, i) => {
  const m = mode[i];
  const mean = m === "Bike" ? 17 : m === "Car" ? 27 : m === "Bus" ? (u(i, 4) < 0.5 ? 22 : 42) : 45;
  return Math.max(4, Math.round(mean + (m === "Bus" ? 7 : 11) * noise(i)));
});
const frame = { data: "commutes", x: "mode", y: "minutes", color: "mode", title: "Time to get to work (minutes)" } as const;

export default doc({
  title: "Commuting times as violins",
  description: "320 commutes as a violin per mode of travel, then with a narrower kernel, then every violin as wide as its band.",
  size: [640, 320],
  data: { commutes: data.values({ person, mode, minutes }, { key: "person" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [violin()] }, at("default")),
      plot({ ...frame, children: [violin({ bandwidth: 1.5 })] }, at("bandwidth")),
      plot({ ...frame, children: [violin({ normalize: "width" })] }, at("width")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "violin()", text: "The bus has two humps — a direct line and a change — that a box plot would hide." }),
      step("bandwidth", { title: "bandwidth: 1.5", text: "A narrower kernel (1.5 minutes) follows the data more closely, noise and all." }),
      step("width", { title: "normalize: \"width\"", text: "Every violin as wide as its band at its peak: shapes compare, sizes don't." }),
    ],
  }),
});
