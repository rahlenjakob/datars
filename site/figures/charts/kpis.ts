// The "KPIs and planning" family on one fictional tea shop: its key figures with sparklines, its
// regions against target, its service as dials, its goals as rings, the plan for its app and its
// history on a timeline. (Made-up numbers.)
import { doc, data, e, geom, group, shape, story, step, type Template } from "@datars/sdk";
import { kpi, bullet, gauge, progress, gantt, timeline, title as heading } from "@datars/std";

const months = ["2025-10", "2025-11", "2025-12", "2026-01", "2026-02", "2026-03", "2026-04", "2026-05", "2026-06", "2026-07", "2026-08", "2026-09"].map((m) => `${m}-01`);
const revenue = [38200, 41900, 55400, 36100, 37800, 40300, 42600, 44100, 43300, 45900, 47200, 51400];
const orders = [1010, 1105, 1460, 952, 990, 1046, 1098, 1132, 1117, 1164, 1190, 1284];
const returns = [0.052, 0.049, 0.061, 0.058, 0.055, 0.051, 0.05, 0.047, 0.049, 0.046, 0.048, 0.041];
const plan = [
  { task: "User interviews", phase: "Research", start: "2026-02-02", end: "2026-02-20", done: 1 },
  { task: "Sketches", phase: "Design", start: "2026-02-23", end: "2026-03-13", done: 1 },
  { task: "Prototype", phase: "Design", start: "2026-03-09", end: "2026-04-03", done: 0.7 },
  { task: "Design sign-off", phase: "Design", start: "2026-04-03", end: null, done: 0 },
  { task: "App build", phase: "Build", start: "2026-03-23", end: "2026-05-08", done: 0.3 },
  { task: "Beta test", phase: "Build", start: "2026-04-27", end: "2026-05-22", done: 0 },
  { task: "Launch day", phase: "Launch", start: "2026-06-02", end: null, done: 0 },
];
const history = [
  { year: 2009, event: "First shop, by the harbour" }, { year: 2012, event: "Second shop, Old Town" },
  { year: 2015, event: "Tea club starts" }, { year: 2017, event: "Own blends" }, { year: 2019, event: "Online shop" },
  { year: 2020, event: "Deliveries through lockdown" }, { year: 2023, event: "Roastery opens" }, { year: 2026, event: "The app" },
];

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
/** A dashboard tile: the card surface, the figure inside it. */
const tile = (key: string, inside: Template) => group({ key, children: [
  shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 8 }), { key: "card", fill: "$card", stroke: { paint: "$card-line", width: 1 } }),
  group({ key: "inside", layout: { type: "stack", padding: 14 }, children: [inside] }),
] });
const status = { bands: [70, 90, 100], inks: ["$negative", "$highlight", "$positive"] };

export default doc({
  title: "A tea shop, in numbers and plans",
  description: "A fictional tea shop's revenue, orders and returns as key figures; its regions' sales against target as bullet graphs; its service as three dials; its year's goals as rings; its app's launch plan as a Gantt chart; and its history on a timeline.",
  size: [640, 400],
  data: {
    shop: data.values({ month: months, revenue, orders, returns }, { key: "month", types: { month: "date" } }),
    regions: data.values({
      region: ["North", "Coast", "City", "Online"], sales: [182, 236, 412, 318], target: [200, 220, 450, 300],
      poor: [120, 150, 300, 200], fair: [170, 200, 400, 280], good: [240, 280, 520, 380],
    }, { key: "region" }),
    plan: data.values(plan, { key: "task", types: { start: "date", end: "date" } }),
    history: data.values(history, { key: "year" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      group({ key: "chart", when: e('state == "figures"'), layout: { type: "rows", gap: 12, padding: [40, 0, 40, 0] }, children: [
        group({ key: "tiles", layout: { type: "columns", gap: 12, wrap: 420 }, children: [
          tile("revenue", kpi({ label: "Revenue", data: "shop", x: "month", y: "revenue", format: "$,.0f", compareLabel: "vs August" })),
          tile("orders", kpi({ label: "Orders", data: "shop", x: "month", y: "orders", compareLabel: "vs August" })),
          tile("returns", kpi({ label: "Returns", data: "shop", x: "month", y: "returns", format: ".1%", better: "down", compareLabel: "vs August" })),
        ] }),
      ] }),
      group({ key: "chart", when: e('state == "bullet"'), layout: { type: "rows", gap: 12 }, children: [
        heading({ text: "Sales by region against target ($ thousands)" }, { size: { h: "auto" } }),
        bullet({ data: "regions", label: "region", value: "sales", target: "target", bands: ["poor", "fair", "good"] }),
      ] }),
      group({ key: "chart", when: e('state == "dials"'), layout: { type: "columns", gap: 16, wrap: 420, padding: [50, 0, 50, 0] }, children: [
        gauge({ value: 86, label: "Satisfaction", suffix: " / 100", ...status }, { key: "satisfaction" }),
        gauge({ value: 94, label: "Orders on time", suffix: "%", ...status }, { key: "on-time" }),
        gauge({ value: 64, label: "Stock available", suffix: "%", ...status }, { key: "stock" }),
      ] }),
      group({ key: "chart", when: e('state == "goals"'), layout: { type: "columns", gap: 16, padding: [50, 0, 50, 0] }, children: [
        progress({ label: "Sales", value: 412000, goal: 520000, prefix: "$", format: ",.0f", shape: "ring", showGoal: true }, { key: "sales" }),
        progress({ label: "Tea club members", value: 1840, goal: 2000, format: ",.0f", shape: "ring", showGoal: true }, { key: "members" }),
        progress({ label: "New shops opened", value: 1, goal: 3, format: ",.0f", shape: "ring", showGoal: true }, { key: "shops" }),
      ] }),
      gantt({ data: "plan", task: "task", start: "start", end: "end", group: "phase", progress: "done", today: "2026-03-16" }, at("plan")),
      timeline({ data: "history", date: "year", label: "event", xType: "linear" }, at("history")),
    ],
  }),
  program: story({
    steps: [
      step("figures", { title: "kpi()", text: "Each month against the one before, with a year's sparkline." }),
      step("bullet", { title: "bullet()", text: "Sales as bars, targets as ticks, poor-fair-good as bands." }),
      step("dials", { title: "gauge()", text: "Dials on red, amber and green bands." }),
      step("goals", { title: "progress()", text: "The year's goals as rings." }),
      step("plan", { title: "gantt()", text: "The app's plan: phases, progress and today." }),
      step("history", { title: "timeline()", text: "The shop's story, labels packed so none collide." }),
    ],
  }),
});
