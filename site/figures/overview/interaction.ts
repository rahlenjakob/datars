// Interactive: three lines and a keyset signal the lines read (`selected`): the series it holds
// stay strong, the others recede. The page sets it (`setSignal`), as a legend click or a linked
// view would; hover works as everywhere. Illustrative figures.
import { data, doc, group, interactive, signal } from "@datars/sdk";
import { line, plot } from "@datars/std";
import { PAD, SIZE } from "./_kit";

const city = ["Oslo", "Lisbon", "Cairo"], base = [6, 17, 22], amp = [9, 6, 7];
const rows = city.flatMap((c, ci) => Array.from({ length: 12 }, (_, m) => ({ city: c, month: m + 1, temp: Math.round(base[ci] - amp[ci] * Math.cos(((m + 0.5) / 12) * 2 * Math.PI)) })));

export default doc({
  id: "overview-interaction",
  title: "Monthly temperatures",
  description: "Average monthly temperatures in Oslo, Lisbon and Cairo as three lines.",
  size: SIZE,
  data: { temps: data.values(rows, { key: ["city", "month"] }) },
  signals: { pick: signal.keyset([]) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ data: "temps", x: "month", y: "temp", xType: "linear", color: "city", labelSpace: 52, children: [line({ labels: true, selected: "pick" })] }, { key: "chart" }),
  ] }),
  program: interactive(),
});
