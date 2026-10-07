// The time half of "Distributions and time": a city's bike counters. Every day of two years as a
// calendar cell, the three counters' months as stacked layers and as shares, each month's
// difference from the year's average as a stripe, and riders against temperature, month by month.
// The stripes and the scatter's points are keyed by month, so each stripe becomes its point.
// (Made-up numbers from a seeded formula, with a weekly and a seasonal rhythm.)
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { plot, calendar, stackedArea, stripes, connectedScatter, title as heading } from "@datars/std";

const u = (i: number, k: number) => Math.abs(Math.sin(i * 12.9898 + k * 78.233) * 43758.5453) % 1;
const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const temp = [-1, 0, 3, 8, 13, 17, 19, 18, 14, 9, 4, 1];
const days = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
// How much each month's weather lets people ride (rain, wind), beyond its temperature.
const weather = [0.8, 0.9, 1.0, 1.15, 1.1, 0.95, 0.85, 1.05, 1.1, 0.85, 0.8, 0.9];
// The counters: the bridge is commuters all year; the park fills up in summer.
const sites: [string, (m: number) => number][] = [["Bridge", () => 1.0], ["Harbour", () => 0.55], ["Park", (m) => 0.15 + 0.06 * temp[m]]];
const date: string[] = [], riders: number[] = [];
const monthly: { site: string; month: string; riders: number }[] = [];
let d = 0;
for (const year of [2023, 2024]) {
  days.forEach((n0, m) => {
    const n = m === 1 && year === 2024 ? 29 : n0;
    const tot = sites.map(() => 0);
    for (let k = 1; k <= n; k++, d++) {
      const weekday = (d + 6) % 7; // 2023-01-01 was a Sunday: 0 = Monday
      const day = (900 + 160 * temp[m]) * weather[m] + (weekday >= 5 ? -500 : 150) + 400 * (u(d, 1) - 0.5);
      sites.forEach(([, w], s) => { tot[s] += Math.max(50, day * w(m)); });
      date.push(`${year}-${String(m + 1).padStart(2, "0")}-${String(k).padStart(2, "0")}`);
      riders.push(Math.round(Math.max(120, day * 1.9)));
    }
    if (year === 2024) sites.forEach(([site], s) => monthly.push({ site, month: months[m], riders: Math.round(tot[s] / 1000) }));
  });
}
const total = months.map((name) => monthly.filter((r) => r.month === name).reduce((s, r) => s + r.riders, 0));
const mean = total.reduce((s, x) => s + x, 0) / 12;

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const layers = { data: "monthly", x: "month", y: "riders", xType: "point", color: "site", legend: true } as const;

export default doc({
  title: "Two years of a city's bike counters",
  description: "Two years of daily riders as a calendar, three counters' monthly riders in 2024 as stacked areas and as shares, each month's difference from the average as stripes, then riders against temperature month by month.",
  size: [640, 400],
  data: {
    daily: data.values({ date, riders }, { key: "date", types: { date: "date" } }),
    monthly: data.values(monthly, { key: ["site", "month"] }),
    months: data.values({ month: months, riders: total, vsAverage: total.map((t) => Math.round(t - mean)), temp }, { key: "month" }),
  },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      group({ key: "chart", when: e('state == "calendar"'), layout: { type: "rows", gap: 10 }, children: [
        heading({ text: "Riders a day, 2023 and 2024" }, { size: { h: "auto" } }),
        calendar({ data: "daily", date: "date", value: "riders" }),
      ] }),
      plot({ ...layers, title: "Riders a month, by counter (thousands)", children: [stackedArea()] }, at("layers")),
      plot({ ...layers, title: "Each counter's share of the month", children: [stackedArea({ offset: "expand" })] }, at("shares")),
      group({ key: "chart", when: e('state == "stripes"'), layout: { type: "rows", gap: 10 }, children: [
        heading({ text: "Each month against the year's average", subtitle: "Below in blue, above in red (thousands of riders)" }, { size: { h: "auto" } }),
        stripes({ data: "months", x: "month", value: "vsAverage" }),
      ] }),
      plot({ data: "months", x: "temp", y: "riders", xType: "linear", xLabel: "Average temperature (°C)", yLabel: "Riders (thousands)", title: "Riders against temperature, month by month",
        children: [connectedScatter({ order: "month", text: "month", labels: "all" })] }, at("scatter")),
    ],
  }),
  program: story({
    steps: [
      step("calendar", { title: "calendar()", text: "Every day a cell, a row per year: weekends and winter are quieter." }),
      step("layers", { title: "stackedArea()", text: "Three counters' months, stacked." }),
      step("shares", { title: "stackedArea({ offset: \"expand\" })", text: "The same layers as shares of each month." }),
      step("stripes", { title: "stripes()", text: "Each month's difference from the average, as a colour." }),
      step("scatter", { title: "connectedScatter()", text: "Each stripe becomes its month's point: riders follow the temperature." }),
    ],
  }),
});
