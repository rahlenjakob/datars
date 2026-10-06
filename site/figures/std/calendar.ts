// calendar: a heatmap of days — a week-column × weekday-row grid per year, coloured by value on
// the theme's sequential ramp.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { calendar, title } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Cyclists counted on a town bridge each day of 2024: busier on weekdays and in summer.
const days = Array.from({ length: 366 }, (_, i) => new Date(Date.UTC(2024, 0, 1 + i)));
const date = days.map((d) => d.toISOString().slice(0, 10));
const riders = days.map((d, i) => Math.round((d.getUTCDay() % 6 === 0 ? 700 : 1500) * (1.1 - 0.55 * Math.cos((i / 365.25) * 2 * Math.PI)) * (0.85 + 0.3 * Math.abs(Math.sin(i * 7.3)))));

export default doc({
  title: "Cyclists crossing a town bridge",
  description: "A year of daily cycle counts as a calendar heatmap, then with smaller cells set in pixels.",
  size: [640, 180],
  data: { counts: data.values({ date, riders }, { key: "date" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 12, padding: [16, 20, 12, 12] },
    children: [
      title({ text: "Cyclists a day, 2024", subtitle: "One square per day, Monday on top." }, { size: { h: "auto" } }),
      group({ key: "body", children: [
        calendar({ data: "counts", date: "date", value: "riders" }, at("default")),
        calendar({ data: "counts", date: "date", value: "riders", cell: 8 }, at("cell")),
      ] }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "calendar()", text: "Cells sized to fill the width: one column per week, one row per weekday." }),
      step("cell", { title: "cell: 8", text: "A fixed cell size in pixels." }),
    ],
  }),
});
