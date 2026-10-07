// A recipe from scratch: a row of dots per category, a dot per unit. The units are keyed (category,
// unit i) the way std's waffle and hemicycle key theirs, so a category's dots gather into its bar,
// slice or tile in another chart, and split out of it again.
import { e, group, instances, op, recipe, repeat, text, t } from "@datars/sdk";

interface IsotypeParams { data: string; category: string; value: string; size: number; gap: number }

export const isotype = recipe<IsotypeParams>({
  id: "@local/isotype/isotype",
  doc: "A row of dots per category, one dot per unit of its value.",
  params: {
    data: t.table(),
    category: t.field("The category column: a row of dots each."),
    value: t.field("Whole units: a dot each."),
    size: t.number(12, "Dot diameter (px), smaller when the longest row wouldn't fit."),
    gap: t.number(3, "Space between dots (px)."),
  },
  expand(p, cx) {
    // One row per unit, keyed (category, unit), with the unit's number in `i`.
    const units = cx.table("units", p.data, op.units({ value: p.value, as: "i" }));
    const longest = `table.max(${JSON.stringify(p.data)}, ${JSON.stringify(p.value)})`;
    // Room for the names on the left and a number after the longest row.
    const step = `min(${p.size + p.gap}, (box.w - 150) / max(1, ${longest}))`;
    const y = `scale.row(d.${p.category}) + scale.row.bandwidth() / 2`;
    return group({
      key: "isotype",
      scales: { row: { type: "band", domain: { data: p.data, field: p.category }, range: "height", padding: 0.2 } },
      children: [
        repeat(p.data, text(e(`d.${p.category}`), [100, e(y)], {
          key: e(`"name-" + d.${p.category}`),
          style: { size: "$size.label", ink: "$ink-2", align: "end", baseline: "middle" },
        })),
        // Instanced marks, like std's units: one draw for every dot, however many.
        instances({
          key: "dots",
          from: units,
          x: e(`112 + (d.i + 0.5) * ${step}`),
          y: e(y),
          r: e(`(${step}) * ${p.size / (p.size + p.gap)} / 2`),
          fill: e(`scale.color(d.${p.category})`),
          label: e(`d.${p.category}`),
        }),
      ],
    });
  },
});
