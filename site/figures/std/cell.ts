// cell: a heatmap — a rectangle per (x, y) on two band axes, coloured by a value. Visitors to a
// library by weekday and time of day; cells are keyed (y, x), the keys grouped bars use too.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, cell, grouped } from "@datars/std";

const slots = ["9–11", "11–13", "13–15", "15–17", "17–19", "19–21"];
const visits: Record<string, number[]> = {
  Mon: [18, 40, 52, 47, 60, 25], Tue: [22, 44, 58, 50, 66, 31], Wed: [20, 47, 61, 55, 72, 34],
  Thu: [24, 50, 63, 58, 75, 38], Fri: [15, 35, 44, 40, 38, 12],
};
const rows = Object.entries(visits).flatMap(([day, n]) => n.map((visitors, i) => ({ day, slot: slots[i], visitors })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const title = "Library visitors by weekday and time of day";
// A heatmap's bands touch (padding 0) and need no gridlines: `gap` spaces the cells.
const heat = { data: "visits", x: "slot", y: "day", yType: "band", color: "visitors", colorType: "sequential", padding: 0, grid: false, title } as const;

export default doc({
  title,
  description: "A week of library visitors as a heatmap of weekday by time of day, then with wider gaps, then morphing into grouped bars.",
  size: [640, 320],
  data: { visits: data.values(rows, { key: ["day", "slot"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...heat, children: [cell()] }, at("default")),
      plot({ ...heat, children: [cell({ gap: 6 })] }, at("gap")),
      plot({ data: "visits", x: "slot", y: "visitors", color: "day", legend: true, title, children: [grouped()] }, at("grouped")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "cell()", text: "Each cell coloured by its visitors, on the plot's sequential colour scale." }),
      step("gap", { title: "gap: 6", text: "More space between the cells." }),
      step("grouped", { title: "grouped()", text: "Cells and grouped bars share their keys: each cell becomes its bar." }),
    ],
  }),
});
