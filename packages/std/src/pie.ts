// pie / donut: slices from the `pie` layout algorithm, labelled outside with leaders when they fit.

import { e, group, op, recipe, repeat, shape, geom, t, text } from "@datars/sdk";

export interface PieParams { data: string; value: string; category: string; inner: number; pad: number; sort: "none" | "asc" | "desc"; labels: boolean; format: string; total: boolean }

export const pie = recipe<PieParams>({
  id: "@datars/std/pie",
  doc: "Parts of a whole as slices (set `inner` > 0 for a donut). Colours from the categorical palette by category.",
  params: {
    data: t.table(), value: t.field(), category: t.field(),
    inner: t.number(0, "Inner radius as a fraction of the outer (0 = pie, 0.6 = donut)."),
    pad: t.number(0.006, "Pad angle between slices (radians)."),
    sort: t.oneOf(["none", "asc", "desc"] as const, "none"),
    labels: t.bool(true), format: t.string(".1~f"), total: t.bool(false, "Show the total in the middle (donuts)."),
  },
  tokens: ["categorical", "paper", "ink", "muted", "size.label", "size.title"],
  expand(p, cx) {
    // Each slice's label width, measured with the engine's shaper, so the radius leaves room for
    // the widest label beside the pie (labels never leave the box).
    const slices = cx.table("slices", p.data,
      op.pie({ value: p.value, sort: p.sort, pad: p.pad, as: ["a0", "a1"] }),
      op.derive("lw", e(`measure(key.name(d.${p.category}), token("size.label"))`)));
    const S = JSON.stringify(slices);
    const base = "(min(box.w, box.h) / 2 - 36)";
    const fit = `(box.w / 2 - table.max(${S}, "lw") - 26)`;
    // Too narrow for labels beside the pie (they'd leave it under a fifth of the box): a wrapped
    // legend below it instead.
    const colW = `(table.max(${S}, "lw") + 26)`;
    const cols = `max(1, floor(box.w / ${colW}))`;
    const legendH = `(ceil(table.count(${S}) / ${cols}) * 16 + 10)`;
    const narrow = p.labels ? `(${fit} < min(box.w, box.h) * 0.2)` : "false";
    const R = p.labels ? `(${narrow} ? min(box.w, box.h - ${legendH}) / 2 - 4 : min(${base}, ${fit}))` : base;
    const CY = p.labels ? `(${narrow} ? (box.h - ${legendH}) / 2 : box.h / 2)` : "box.h / 2";
    const mid = "(d.a0 + d.a1) / 2";
    return group({
      key: "marks",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      semantics: { role: "series", label: `${p.value} by ${p.category}` },
      children: [
        repeat(slices, shape(geom.arc({ cx: e("box.w / 2"), cy: e(CY), r0: e(`(${R}) * ${p.inner}`), r1: e(R), a0: e("d.a0"), a1: e("d.a1") }), {
          fill: e(`scale.color(d.${p.category})`),
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: e(`\`\${key.name(d.${p.category})}: \${format(d.${p.value}, ${JSON.stringify(p.format)})}\``), value: e(`d.${p.value}`) },
          pickable: true,
        })),
        p.labels
          ? group({ key: "labels", declutter: true, children: [repeat(slices, text(e(`key.name(d.${p.category})`), [e(`box.w / 2 + (${R} + 14) * sin(${mid})`), e(`${CY} - (${R} + 14) * cos(${mid})`)], {
              when: e(`!${narrow} && d.a1 - d.a0 > 0.12`),
              style: { size: "$size.label", ink: "$ink-2", align: e(`sin(${mid}) >= 0 ? "start" : "end"`), baseline: "middle" },
            }))] })
          : null,
        p.labels
          ? group({ key: "legend", when: e(narrow), semantics: { role: "legend", label: "Legend" }, children: [repeat({ legend: "color" }, group({
              transform: { translate: [e(`(d.index % ${cols}) * ${colW} + (box.w - min(table.count(${S}), ${cols}) * ${colW}) / 2`), e(`box.h - ${legendH} + 10 + floor(d.index / ${cols}) * 16`)] },
              semantics: { role: "legend-item", label: e("key.name(d.label)") },
              children: [
                shape(geom.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e("d.ink") }),
                text(e("key.name(d.label)"), [14, 9], { style: { size: "$size.label", ink: "$ink-2"} }),
              ],
            }))] })
          : null,
        p.total && p.inner > 0
          ? text("", [e("box.w / 2"), e(CY)], { key: "total", number: { value: e(`sum(${JSON.stringify(p.data)}, ${JSON.stringify(p.value)})`), format: p.format }, style: { font: "font.title", size: "$size.title", align: "middle", baseline: "middle" } })
          : null,
      ],
    });
  },
});

export const donut = (params: Partial<PieParams> & Record<string, unknown>, opts?: Record<string, unknown>) => pie({ inner: 0.58, ...params }, opts);
