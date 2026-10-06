// A dumbbell chart as a recipe: two values per row joined by a bar, rows sorted by the second.
// It derives a sorted table with each name's measured width (`cx.table`; `measure()` runs in the
// engine — a recipe never sees rows), measures room for its value labels (`cx.measure`),
// declares its own scales and reads them through `cx.scale`.
import { e, geom, group, op, recipe, repeat, shape, t, text } from "@datars/sdk";

export interface DumbbellParams {
  data: string; y: string; a: string; b: string; aInk: string; bInk: string; format: string;
}

export const dumbbell = recipe<DumbbellParams>({
  id: "@local/dumbbell/dumbbell",
  doc: "Two values per row, joined: where each row started (`a`) and ended (`b`), sorted by `b`.",
  params: {
    data: t.table("The rows."),
    y: t.field("The category column, one row each."),
    a: t.field("The first value."),
    b: t.field("The second value; rows are sorted by it."),
    aInk: t.ink("$muted", "The first value's dot."),
    bInk: t.ink("$accent", "The second value's dot."),
    format: t.string(",.0f", "Number format for the value labels."),
  },
  tokens: ["muted", "accent", "rule", "ink", "ink-2", "size.label"],
  expand(p, cx) {
    // Sorted by the second value, largest first, each row with its name's width.
    const rows = cx.table("sorted", p.data,
      op.sort([p.b, "desc"]),
      op.derive("w", e(`measure(d.${p.y}, token("size.label"))`)));
    // Room on the right for a value label, measured with the chart's own font.
    const right = cx.measure("0,000", { size: 11 }).w + 16;
    const x = cx.scale("x"), y = cx.scale("y");
    const A = cx.field(p.a), B = cx.field(p.b), name = cx.field(p.y);
    const mid = e(`${y(name).expr} + ${y.bandwidth().expr} / 2`);
    const label = (f: string) => e(`d.${p.y} + ": " + format(d.${f}, ${JSON.stringify(p.format)})`);
    const up = `d.${p.b} >= d.${p.a}`;
    return group({
      key: "dumbbell",
      scales: {
        x: {
          type: "linear", domain: { data: rows, fields: [p.a, p.b] }, nice: true, zero: false,
          range: [`=table.max(${JSON.stringify(rows)}, "w") + 14`, `=box.w - ${right}`],
        },
        y: { type: "band", domain: { data: rows, field: p.y }, range: "height", padding: 0.3 },
      },
      children: [
        repeat(rows, group({ key: name, children: [
          text(name, [0, mid], {
            key: "name", style: { size: "$size.label", ink: "$ink-2", baseline: "middle" },
          }),
          shape(geom.segment({ x1: x(A), y1: mid, x2: x(B), y2: mid }), {
            key: "bar", stroke: { paint: "$rule", width: 3, cap: "round" },
          }),
          shape(geom.circle({ cx: x(A), cy: mid, r: 5 }), {
            key: "a", fill: p.aInk, pickable: true, semantics: { role: "datum", label: label(p.a) },
          }),
          shape(geom.circle({ cx: x(B), cy: mid, r: 6 }), {
            key: "b", fill: p.bInk, pickable: true, semantics: { role: "datum", label: label(p.b) },
          }),
          text(e(`format(d.${p.b}, ${JSON.stringify(p.format)})`), [x(B), mid], {
            key: "value",
            offset: [e(`${up} ? 10 : -10`), 0],
            style: { size: "$size.label", ink: "$ink", baseline: "middle", align: e(`${up} ? "start" : "end"`) },
          }),
        ] })),
      ],
    });
  },
  // New rows' dots grow in from their centre.
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }],
});
