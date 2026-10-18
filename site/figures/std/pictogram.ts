// pictogram: an icon per unit. A city's households by how they heat their homes, one icon per
// thousand homes — in rows with names and values, then as a grid of a hundred with a legend.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { pictogram } from "@datars/std";

const icons = { "District heat": "heater", "Heat pump": "fan", Electric: "zap", Gas: "flame", Wood: "tree-pine" };
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "How a city heats its homes",
  description: "A fictional city's 100,000 households by heating: 38,000 on district heat, 27,000 heat pumps, 17,000 electric, 11,000 gas and 7,000 wood — first in rows, one icon per two thousand homes, then a hundred-icon grid.",
  size: [640, 340],
  data: { homes: data.values({ heat: Object.keys(icons), thousands: [38, 27, 17, 11, 7] }, { key: "heat" }) },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 16, 12, 12] },
    children: [
      pictogram({ data: "homes", value: "thousands", category: "heat", icons, per: 2, unit: "k" }, at("rows")),
      pictogram({ data: "homes", value: "thousands", category: "heat", icons, layout: "grid" }, at("grid")),
    ],
  }),
  program: story({
    steps: [
      step("rows", { title: "pictogram({ icons, per: 2 })", text: "A row per category, an icon per two thousand homes." }),
      step("grid", { title: "layout: \"grid\"", text: "A hundred icons, one per thousand, and a legend of them." }),
    ],
  }),
});
