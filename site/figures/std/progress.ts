// progress: how far toward a goal — a bar with the share above it, or a ring with the share in
// the middle. A fictional tea shop's goals for the year, three quarters of the way through it.
import { doc, e, group, story, step } from "@datars/sdk";
import { progress } from "@datars/std";

const goals = [
  { key: "sales", label: "Sales", value: 412000, goal: 520000, prefix: "$", format: ",.0f" },
  { key: "members", label: "Tea club members", value: 1840, goal: 2000, format: ",.0f" },
  { key: "shops", label: "New shops opened", value: 1, goal: 3, format: ",.0f" },
];
// The three goals, one set per step: as bars one above the other, or as rings side by side.
const set = (state: string, o: Record<string, unknown> = {}) => group({
  key: "goals", when: e(`state == "${state}"`),
  layout: o.shape === "ring" ? { type: "columns", gap: 16 } : { type: "rows", gap: 22 },
  children: goals.map(({ key, ...g }) => progress({ ...g, ...o }, { key, size: o.shape === "ring" ? undefined : { h: "auto" } })),
});

export default doc({
  title: "A tea shop's goals for the year",
  description: "Three goals — sales, tea club members and new shops — as progress bars with the share reached; then with the value and the goal under each; then as rings.",
  size: [640, 240],
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [20, 20, 12, 12] },
    children: [set("default"), set("goal", { showGoal: true }), set("ring", { shape: "ring", showGoal: true })],
  }),
  program: story({
    steps: [
      step("default", { title: "progress({ value, goal, label })", text: "A bar filled to value ÷ goal, the share at its right." }),
      step("goal", { title: "showGoal: true", text: "The value and the goal too: $412,000 of $520,000." }),
      step("ring", { title: "shape: \"ring\"", text: "Rings, the share in the middle." }),
    ],
  }),
});
