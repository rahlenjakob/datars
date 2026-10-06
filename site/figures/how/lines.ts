// Under the hood, §5 (tracks): lines keep their points. A monthly series grows from six months to
// nine (the line runs on: the shared part moves with its points as the x scale rescales, the new
// part is drawn on like a pen, and each dot appears as the pen reaches it), then its values are
// revised (same points, so each travels straight up or down), then the window slides.
// Illustrative values, not data.
import { data, doc, e, group, op, signal, step, story } from "@datars/sdk";
import { line, plot } from "@datars/std";
import { narration } from "./_kit";

const month = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
const first = [42, 47, 45, 53, 58, 55, 61, 66, 63, 70, 74, 71];
const revised = [44, 45, 49, 51, 56, 59, 58, 64, 67, 68, 72, 75];

export default doc({
  id: "how-lines",
  title: "A line that runs on",
  description: "A monthly line with dots grows from six months to nine, is revised, then slides to a later window: points keep their identity, new ones are drawn on, revised ones move straight up or down.",
  size: [680, 300],
  data: { series: data.values({ month, first, revised }, { key: "month" }) },
  signals: { from: signal.num(1), to: signal.num(6), rev: signal.bool(false) },
  tables: { shown: { from: "series", ops: [op.filter(e("d.month >= from && d.month <= to")), op.derive("value", e("rev ? d.revised : d.first"))] } },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 12, padding: [14, 18, 10, 14] },
    children: [
      plot({ data: "shown", x: "month", y: "value", xType: "linear", xFormat: "d", zero: false, format: ".0f", title: "A monthly series (illustrative)", children: [line({ points: true, width: 2 })] }, { key: "plot" }),
      narration(2, 3),
    ],
  }),
  program: story({ steps: [
    step("six", { set: { from: 1, to: 6, rev: false }, text: "Six months, a dot on every point." }),
    step("nine", { set: { from: 1, to: 9, rev: false }, text: "Nine: the line runs on along itself, like a pen; its dots appear as the pen reaches them, while the rest move with the scale." }),
    step("revised", { set: { from: 1, to: 9, rev: true }, text: "Revised values: the same points, so each one travels straight up or down and its dot rides with it." }),
    step("later", { set: { from: 4, to: 12, rev: true }, text: "A later window: the points slide left, the first three leave through the edge and three new ones are drawn on." }),
  ] }),
});
