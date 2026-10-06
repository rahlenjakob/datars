// checklist: checkboxes for a key set — here which fruits the chart draws. Click one to take it
// out or put it back.
import { doc, data, e, group, op, signal, story, step } from "@datars/sdk";
import { plot, line, checklist } from "@datars/std";

const weeks = [1, 2, 3, 4, 5, 6, 7, 8];
const fruit = ["Apples", "Pears", "Plums", "Cherries"];
const kg = { Apples: [40, 42, 45, 43, 48, 50, 47, 52], Pears: [22, 24, 23, 27, 26, 29, 31, 30], Plums: [8, 12, 18, 25, 30, 27, 20, 14], Cherries: [0, 5, 14, 22, 18, 9, 3, 0] };

export default doc({
  title: "Fruit sold a week (kg)",
  description: "Checkboxes choose the fruits drawn; the lines come and go.",
  size: [640, 320],
  data: { s: data.values({ week: fruit.flatMap(() => weeks), fruit: fruit.flatMap((f) => weeks.map(() => f)), kg: fruit.flatMap((f) => kg[f as keyof typeof kg]) }, { key: ["fruit", "week"] }) },
  signals: { shown: signal.keyset(fruit) },
  tables: { picked: { from: "s", ops: [op.filter(e("shown.has(d.fruit)"))] } },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] }, children: [
    checklist({ signal: "shown", options: fruit }, { key: "shown" }),
    plot({ data: "picked", x: "week", y: "kg", color: "fruit", yDomain: [0, 60], children: [line({ labels: true })] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("all", { set: { shown: fruit }, title: "every key" }),
    step("stone-fruit", { set: { shown: ["Plums", "Cherries"] }, title: 'shown = ["Plums", "Cherries"]' }),
  ] }),
});
