// icicle: a hierarchy as bands, the root first. A fictional town's yearly budget by department and
// service; the bands are keyed by name, so they move when the orientation or the year changes.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { icicle } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// [item, part of, 2024, 2025] (millions) — services carry the numbers; departments add them up.
const budget: [string, string | null, number, number][] = [
  ["Budget", null, 0, 0],
  ["Schools", "Budget", 0, 0], ["Primary", "Schools", 42, 44], ["Secondary", "Schools", 35, 38], ["Meals", "Schools", 9, 10],
  ["Care", "Budget", 0, 0], ["Elderly", "Care", 38, 43], ["Disability", "Care", 17, 18], ["Children", "Care", 11, 11],
  ["Streets", "Budget", 0, 0], ["Roads", "Streets", 14, 11], ["Lighting", "Streets", 4, 4], ["Cleaning", "Streets", 6, 7],
  ["Leisure", "Budget", 0, 0], ["Library", "Leisure", 5, 5], ["Pools", "Leisure", 7, 4], ["Parks", "Leisure", 6, 6],
  ["Town hall", "Budget", 0, 0], ["Staff", "Town hall", 12, 12], ["Buildings", "Town hall", 5, 6],
];

export default doc({
  title: "Where a town's budget goes (millions)",
  description: "A fictional town's spending by department and service as an icicle: the whole budget at the left, then from the top, then with the next year's figures.",
  size: [640, 360],
  data: {
    budget: data.values({ item: budget.map((b) => b[0]), dept: budget.map((b) => b[1]), y2024: budget.map((b) => b[2]), y2025: budget.map((b) => b[3]) }, { key: "item" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      icicle({ data: "budget", id: "item", parent: "dept", value: "y2024" }, at("default")),
      icicle({ data: "budget", id: "item", parent: "dept", value: "y2024", orientation: "vertical" }, at("vertical")),
      icicle({ data: "budget", id: "item", parent: "dept", value: "y2025", orientation: "vertical" }, at("later")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "icicle()", text: "The whole budget at the left; each department spans its share, and each service its share of the department." }),
      step("vertical", { title: "orientation: \"vertical\"", text: "The same bands hanging from the top." }),
      step("later", { title: "value: \"y2025\"", text: "Next year's budget: care grows, roads and pools shrink." }),
    ],
  }),
});
