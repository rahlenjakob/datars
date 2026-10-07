// Ejected from @datars/std (marks.ts) by `datars eject std/bar`. It's yours now: edit freely.
// Documents that use it embed this code as a package (id `@local/bar/bar`).

import { e, group, instances, op, recipe, repeat, shape, geom, t, text, Prop } from "@datars/sdk";

export interface BarParams {
  data: string; x: string; y: string; color: string; xType: string; yType: string;
  fill: Prop; labels: boolean; format: string; radius: number; selected: string; label: Prop; chapter: string; prefix: string; suffix: string;
}

type Affix = { format: string; prefix?: string; suffix?: string };
function affixed(p: Affix, value: string, at: [Prop, Prop], opts: Record<string, unknown>) {
  if (!p.prefix && !p.suffix) return text("", at, { ...opts, number: { value: e(value), format: p.format } });
  const v = `(${value})`;
  return text(e(`(${v} < 0 ? "−" : "") + ${JSON.stringify(p.prefix ?? "")} + format(abs(${v}), ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}`), at, opts);
}
const colorOr = (color: string | undefined, fallback: string) => (color ? e(`scale.color(d.${color})`) : fallback);
const onInkExpr = (fill: Prop) => (typeof fill === "string" ? JSON.stringify(`on(${fill})`) : fill && typeof fill === "object" && "expr" in fill ? `"on(" + (${(fill as { expr: string }).expr}) + ")"` : `"$ink"`);
const isBand = (t?: string) => t === "band" || t === "point";

export const bar = recipe<BarParams>({
  id: "@local/bar/bar",
  doc: "Bars from a category field on a band scale to a value on a linear scale (vertical or horizontal, decided by which axis is the band). Negative values hang below zero.",
  params: {
    data: t.table(), x: t.field(), y: t.field(), color: t.field(), xType: t.string("band"), yType: t.string("linear"),
    fill: t.prop("Fill ink or expression (default: the colour scale, else $mark)."),
    labels: t.bool(false, "Value labels on the bars (they count when values change)."),
    format: t.string(",.1~f", "Label number format."),
    radius: t.number(0, "Corner radius (0: the theme's `radius.bar`)."),
    selected: t.string(undefined, "A keyset signal: selected bars stay strong, others recede."),
    label: t.prop("Bar label (tooltip, accessible name): an expression over the row; default `name: value`."),
    chapter: t.string(undefined, "Clicking a bar enters this program chapter with the bar's category (drill-down)."),
    prefix: t.string(undefined, "Before each value label's number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
  },
  tokens: ["mark", "ink", "muted", "dim", "size.label", "radius.bar"],
  expand(p) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const fill = p.fill ?? colorOr(p.color, "$mark");
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    const opacity = p.selected ? e(`${p.selected}.isEmpty() || ${p.selected}.has(d.${cat}) ? 1 : token("dim")`) : undefined;
    const g = horizontal
      ? geom.rect({ x: e(`min(scale.x(0), scale.x(d.${val}))`), y: e(`scale.y(d.${cat})`), w: e(`abs(scale.x(d.${val}) - scale.x(0))`), h: e("scale.y.bandwidth()"), r: p.radius || "$radius.bar" })
      : geom.rect({ x: e(`scale.x(d.${cat})`), y: e(`min(scale.y(0), scale.y(d.${val}))`), w: e("scale.x.bandwidth()"), h: e(`abs(scale.y(0) - scale.y(d.${val}))`), r: p.radius || "$radius.bar" });
    // Value labels only where they fit: as wide as the bar (vertical), as tall as the band (horizontal).
    const text_ = `(${JSON.stringify(p.prefix ?? "")} + format(d.${val}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")})`;
    const lw = `measure(${text_}, token("size.label"))`;
    const fits = horizontal
      ? e('scale.y.bandwidth() >= token("size.label") - 2')
      : e(`${lw} <= scale.x.bandwidth() + 6`);
    // Past the bar's end when there's room in the plot area; else just inside the end, in the ink
    // that reads on the bar — so the longest bar's label never leaves the canvas and the tallest
    // bar's never runs into the title.
    const end = horizontal ? `max(scale.x(0), scale.x(d.${val}))` : `min(scale.y(0), scale.y(d.${val}))`;
    const length = horizontal ? `abs(scale.x(d.${val}) - scale.x(0))` : `abs(scale.y(d.${val}) - scale.y(0))`;
    const inside = horizontal
      ? `(${end} + 4 + ${lw} > box.w && ${length} >= ${lw} + 8)`
      : `(${end} - 4 - token("size.label") < -2 && ${length} >= token("size.label") + 8)`;
    const ink = e(`${inside} ? ${onInkExpr(fill)} : "$ink-2"`);
    const label = horizontal
      ? affixed(p, `d.${val}`, [e(`${inside} ? ${end} - 4 : ${end} + 4`), e(`scale.y(d.${cat}) + scale.y.bandwidth() / 2`)], { when: fits, style: { size: "$size.label", ink, contain: true, align: e(`${inside} ? "end" : "start"`), baseline: "middle" } })
      : affixed(p, `d.${val}`, [e(`scale.x(d.${cat}) + scale.x.bandwidth() / 2`), e(`${inside} ? ${end} + 4 : ${end} - 4`)], { when: fits, style: { size: "$size.label", ink, contain: true, align: "middle", baseline: e(`${inside} ? "top" : "alphabetic"`) } });
    return group({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        // Primary datum shapes are keyed by the datum key directly (a std convention), so a bar
        // pairs with the same datum's slice, cell or region in another recipe.
        repeat(p.data, shape(g, {
          fill,
          opacity,
          semantics: { role: "datum", label: p.label ?? e(`\`\${key.name(d.${cat})}: \${format(d.${val}, ${JSON.stringify(p.format)})}\``), value: e(`d.${val}`) },
          anchors: [{ name: "top", at: horizontal ? [e(`scale.x(d.${val})`), e(`scale.y(d.${cat}) + scale.y.bandwidth()/2`)] : [e(`scale.x(d.${cat}) + scale.x.bandwidth()/2`), e(`scale.y(d.${val})`)] }],
          on: p.chapter ? { activate: { chapter: p.chapter, key: e(`d.${cat}`) } } : p.selected ? { activate: { toggle: p.selected, value: e(`d.${cat}`) } } : undefined,
          pickable: true,
        })),
        p.labels ? group({ key: "labels", children: [repeat(p.data, label)] }) : null,
      ],
    });
  },
  // Entering bars grow from their base along the value axis.
  motion: (p: BarParams) => [{ select: { role: "datum" }, enter: { scale: 0, origin: isBand(p.yType) && !isBand(p.xType) ? "left" : "bottom" } }],
});
