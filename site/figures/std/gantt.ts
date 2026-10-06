// gantt: a plan on a time axis — a bar per task, tasks under their phase's heading, milestones as
// diamonds, how far each task has got, and today. A fictional app's launch plan for spring.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { gantt } from "@datars/std";

const plan = [
  { task: "User interviews", phase: "Research", start: "2026-02-02", end: "2026-02-20", done: 1 },
  { task: "Competitor review", phase: "Research", start: "2026-02-09", end: "2026-02-27", done: 1 },
  { task: "Sketches", phase: "Design", start: "2026-02-23", end: "2026-03-13", done: 1 },
  { task: "Prototype", phase: "Design", start: "2026-03-09", end: "2026-04-03", done: 0.7 },
  { task: "Design sign-off", phase: "Design", start: "2026-04-03", end: null, done: 0 },
  { task: "App build", phase: "Build", start: "2026-03-23", end: "2026-05-08", done: 0.3 },
  { task: "Beta test", phase: "Build", start: "2026-04-27", end: "2026-05-22", done: 0 },
  { task: "Store listing", phase: "Launch", start: "2026-05-11", end: "2026-05-29", done: 0 },
  { task: "Launch day", phase: "Launch", start: "2026-06-02", end: null, done: 0 },
];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const fields = { data: "plan", task: "task", start: "start", end: "end" };

export default doc({
  title: "An app's launch plan",
  description: "Nine tasks from February to June as bars on a time axis, two of them milestones; then gathered under their phases, coloured by phase; then with how far each task has got and a line at today, 16 March.",
  size: [640, 340],
  data: { plan: data.values(plan, { key: "task", types: { start: "date", end: "date" } }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      gantt(fields, at("default")),
      gantt({ ...fields, group: "phase" }, at("group")),
      gantt({ ...fields, group: "phase", progress: "done", today: "2026-03-16" }, at("today")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "gantt({ task, start, end })", text: "A bar from each task's start to its end; tasks without an end are milestones." }),
      step("group", { title: "group: \"phase\"", text: "Tasks under their phase's heading, a thin bar spanning each phase, coloured by phase." }),
      step("today", { title: "progress: \"done\", today", text: "The solid part of a bar is done; the dashed line is today." }),
    ],
  }),
});
