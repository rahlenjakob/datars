// stacked: a segment per series in each category, stacked — or stretched to 100 % with
// `offset: "expand"`. How commuters in four towns get to work; rows keyed (mode, town).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, stacked } from "@datars/std";

const towns = ["Northby", "Eastwick", "Southam", "Westford"];
const people: Record<string, number[]> = { Car: [42, 70, 30, 55], Bus: [18, 26, 12, 30], Bike: [25, 9, 31, 14], Walk: [15, 12, 19, 9] };
const rows = Object.entries(people).flatMap(([mode, n]) => n.map((k, i) => ({ mode, town: towns[i], k })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "commute", color: "mode", legend: true, title: "Commuters by how they travel (thousands)" } as const;

export default doc({
  title: "How commuters in four towns get to work",
  description: "Four towns' commuters by mode as stacked bars, then stretched to 100 %, then horizontal with value labels.",
  size: [640, 320],
  data: { commute: data.values(rows, { key: ["mode", "town"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, x: "town", y: "k", children: [stacked()] }, at("default")),
      plot({ ...frame, x: "town", y: "k", children: [stacked({ offset: "expand" })] }, at("expand")),
      plot({ ...frame, x: "k", y: "town", xType: "linear", yType: "band", children: [stacked({ offset: "expand", labels: true })] }, at("horizontal")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "stacked()", text: "A bar per town, a segment per mode: the bars reach each town's total." }),
      step("expand", { title: "offset: \"expand\"", text: "Every bar stretched to 100 %: shares, not totals." }),
      step("horizontal", { title: "yType: \"band\", labels: true", text: "Horizontal, with each segment's count where it fits." }),
    ],
  }),
});
