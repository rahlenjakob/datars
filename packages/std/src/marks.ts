// Marks: bar, line, area, point, dot — the grammar-of-graphics basics. Each reads the enclosing
// plot's scales (`x`, `y`, `color`) and inherits `data`/`x`/`y`/`color` from the plot.

import { e, group, instances, op, recipe, repeat, shape, geom, t, text, Prop } from "@datars/sdk";

// A number label: counting (`number`) when plain; with a prefix or suffix, formatted text with the
// sign leading (−$38.6M). Kept local (as in charts.ts) so an ejected recipe stays self-contained.
// A named type, not an inline one: `datars eject` copies helpers by their first brace block.
type Affix = { format: string; prefix?: string; suffix?: string };
function affixed(p: Affix, value: string, at: [Prop, Prop], opts: Record<string, unknown>) {
  if (!p.prefix && !p.suffix) return text("", at, { ...opts, number: { value: e(value), format: p.format } });
  const v = `(${value})`;
  return text(e(`(${v} < 0 ? "−" : "") + ${JSON.stringify(p.prefix ?? "")} + format(abs(${v}), ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}`), at, opts);
}

const colorOr = (color: string | undefined, fallback: string) => (color ? e(`scale.color(d.${color})`) : fallback);
/** An expression for the ink that reads on `fill` (a literal/token ink or an ink expression). */
const onInkExpr = (fill: Prop) => (typeof fill === "string" ? JSON.stringify(`on(${fill})`) : fill && typeof fill === "object" && "expr" in fill ? `"on(" + (${(fill as { expr: string }).expr}) + ")"` : `"$ink"`);
const isBand = (t?: string) => t === "band" || t === "point";

// ---- bar --------------------------------------------------------------------------------------

export interface BarParams {
  data: string; x: string; y: string; color: string; xType: string; yType: string;
  fill: Prop; labels: boolean; format: string; radius: number; selected: string; label: Prop; chapter: string; prefix: string; suffix: string;
}

export const bar = recipe<BarParams>({
  id: "@datars/std/bar",
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

// ---- line ---------------------------------------------------------------------------------------

export interface LineParams { data: string; x: string; y: string; color: string; xType: string; series: string; curve: string; width: number; points: boolean; stroke: Prop; labels: boolean; clip: boolean; selected: string; yScale: string }

export const line = recipe<LineParams>({
  id: "@datars/std/line",
  doc: "One line per series (the colour field, or `series`), through x/y. Draws on as it enters.",
  params: {
    data: t.table(), x: t.field(), y: t.field(), color: t.field(), xType: t.string("linear"),
    series: t.field("Group rows into lines by this field (default: the colour field)."),
    curve: t.oneOf(["linear", "monotone-x", "catmull-rom", "step", "step-before", "step-after"] as const, "monotone-x"),
    width: t.number(0, "Stroke width (0 = the theme's stroke.line)."),
    points: t.bool(false, "Dots at the data points."),
    stroke: t.prop("Stroke ink or expression."),
    labels: t.bool(false, "Label each line at its last point (labels pushed apart so they never overlap)."),
    clip: t.bool(false, "Clip the lines to the plot area (the end labels stay outside it)."),
    selected: t.string(undefined, "A keyset signal: selected series stay strong, the others recede."),
    yScale: t.string("y", "The value scale: `y2` for a plot's right axis."),
  },
  tokens: ["mark", "stroke.line", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const ink = p.stroke ?? (series && p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    const ys = p.yScale || "y";
    const lineShape = shape(geom.polyline({ from: "@group", x: e(xExpr), y: e(`scale.${ys}(d.${p.y})`), curve: p.curve }), {
      key: "line",
      opacity: p.selected && series ? e(`${p.selected}.isEmpty() || ${p.selected}.has(d.${series}) ? 1 : token("dim")`) : undefined,
      stroke: { paint: ink, width: p.width || "$stroke.line", join: "round", cap: "round" },
      semantics: { role: "series", label: series ? e(`key.name(d.${series})`) : p.y },
    });
    // Each point says what it is: the series, where on x (as the axis writes it) and the value.
    const xLabel = isBand(p.xType) ? `key.name(d.${p.x})` : `scale.x.label(d.${p.x})`;
    const dotLabel = series ? `\`\${key.name(d.${series})} · \${${xLabel}}: \${format(d.${p.y}, ",.2~f")}\`` : `\`\${${xLabel}}: \${format(d.${p.y}, ",.2~f")}\``;
    // The points are where the pointer finds the line's values: hovered or tapped anywhere along
    // the line, the nearest point's series, x and value (`hit: "line"`). Dots with `points`,
    // otherwise drawn as nothing.
    const pts = instances({ key: "points", from: "@group", x: e(xExpr), y: e(`scale.${ys}(d.${p.y})`), r: p.points ? 3 : 0, fill: p.points ? ink : "transparent", instanceKey: e(`d.${p.x}`), label: e(dotLabel), hit: "line" });
    const labelled = !!(p.labels && series);
    // Clipped with end labels, each line clips itself (to the plot area, its box): the labels sit
    // just past the area's edge, beside the lines in the same group — so a line's key path is the
    // same with labels on or off, and toggling them morphs the lines instead of cross-fading them.
    const one = group({ clip: p.clip && labelled ? "box" : undefined, children: [lineShape, pts] });
    const each = series ? repeat({ groups: p.data, by: series }, one) : repeat({ groups: p.data, by: p.y === "__none" ? p.x : "__all" }, one);
    if (!labelled) return group({ key: "lines", clip: p.clip ? "box" : undefined, children: [each] });
    // Each series' last point, its label pushed apart from its neighbours' (1-D spread).
    const ends = cx.table("ends", p.data,
      op.aggregate([series], { x_end: ["last", p.x], y_end: ["last", p.y] }),
      op.spread({ position: e("scale.y(d.y_end)"), gap: e('token("size.label") + 3'), min: 0, max: e("box.h"), as: "label_y" }));
    const xEnd = isBand(p.xType) ? "scale.x(d.x_end) + scale.x.bandwidth() / 2" : "scale.x(d.x_end)";
    const labelInk = p.stroke ?? (p.color ? e(`scale.color(d.${series})`) : "$mark");
    return group({ key: "lines", children: [
      each,
      group({ key: "end-labels", children: [repeat(ends, text(e(`key.name(d.${series})`), [e(`min(${xEnd}, box.w) + 6`), e("d.label_y")], { style: { size: "$size.label", ink: labelInk, baseline: "middle", contain: true } }))] }),
    ] });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }],
});

// ---- area ---------------------------------------------------------------------------------------

export interface AreaParams { data: string; x: string; y: string; color: string; xType: string; series: string; y0: string; curve: string; opacity: number; fill: Prop; clip: boolean }

export const area = recipe<AreaParams>({
  id: "@datars/std/area",
  doc: "Filled areas from a baseline (0, or `y0`) to y — one per series. For stacked areas, give a stacked table (op.stack) and y0/y fields.",
  params: {
    data: t.table(), x: t.field(), y: t.field(), color: t.field(), xType: t.string("linear"), series: t.field(), y0: t.field("Lower bound field (default: zero)."),
    curve: t.string("monotone-x"), opacity: t.number(0.85), fill: t.prop(), clip: t.bool(false, "Clip to the plot area."),
  },
  tokens: ["mark"],
  expand(p) {
    const series = p.series ?? p.color;
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const fill = p.fill ?? (series && p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    const band = shape(geom.area({ from: "@group", x: e(xExpr), y0: e(p.y0 ? `scale.y(d.${p.y0})` : "max(0, min(box.h, scale.y(0)))"), y1: e(`scale.y(d.${p.y})`), curve: p.curve }), {
      key: "area", fill, opacity: p.opacity, semantics: { role: "series", label: series ? e(`key.name(d.${series})`) : p.y },
    });
    return group({ key: "areas", clip: p.clip ? "box" : undefined, children: [repeat({ groups: p.data, by: series ?? "__all" }, group({ children: [band] }))] });
  },
});

// ---- point (scatter) ----------------------------------------------------------------------------

export interface PointParams { data: string; x: string; y: string; color: string; xType: string; yType: string; r: Prop; symbol: string; fill: Prop; opacity: Prop; label: Prop; clip: boolean }

export const point = recipe<PointParams>({
  id: "@datars/std/point",
  doc: "A dot per row (instanced: scales to 10⁵–10⁶ points).",
  params: {
    data: t.table(), x: t.field(), y: t.field(), color: t.field(), xType: t.string("linear"), yType: t.string("linear"),
    r: t.prop("Radius (px) or expression."), symbol: t.string("circle"), fill: t.prop(), opacity: t.prop(), label: t.prop("Accessible label per point."),
    clip: t.bool(false, "Clip to the plot area."),
  },
  tokens: ["mark", "point.radius"],
  expand(p) {
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const yExpr = isBand(p.yType) ? `scale.y(d.${p.y}) + scale.y.bandwidth() / 2` : `scale.y(d.${p.y})`;
    return instances({
      key: "points",
      clip: p.clip ? "box" : undefined,
      from: p.data,
      proto: p.symbol as "circle",
      x: e(xExpr),
      y: e(yExpr),
      r: p.r ?? "$point.radius",
      fill: p.fill ?? colorOr(p.color, "$mark"),
      opacity: p.opacity,
      label: p.label ?? e(`\`\${d.${p.x}}, \${d.${p.y}}\``),
      semantics: { role: "series", label: `${p.y} against ${p.x}` },
    });
  },
});

// ---- dot (dot plot: one circle per category at its value) ----------------------------------------

export interface DotParams { data: string; x: string; y: string; color: string; xType: string; yType: string; r: number; fill: Prop }

export const dot = recipe<DotParams>({
  id: "@datars/std/dot",
  doc: "A dot per category at its value (a dot plot).",
  params: { data: t.table(), x: t.field(), y: t.field(), color: t.field(), xType: t.string("band"), yType: t.string("linear"), r: t.number(6), fill: t.prop() },
  expand(p) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const cx = horizontal ? e(`scale.x(d.${p.x})`) : e(`scale.x(d.${p.x}) + scale.x.bandwidth() / 2`);
    const cy = horizontal ? e(`scale.y(d.${p.y}) + scale.y.bandwidth() / 2`) : e(`scale.y(d.${p.y})`);
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    return group({
      key: "marks",
      children: [repeat(p.data, shape(geom.circle({ cx, cy, r: p.r }), { fill: p.fill ?? colorOr(p.color, "$mark"), semantics: { role: "datum", label: e(`\`\${key.name(d.${cat})}: \${d.${val}}\``) }, pickable: true }))],
    });
  },
});

// ---- Pareto line --------------------------------------------------------------------------------

export interface ParetoParams { data: string; x: string; y: string; xType: string; stroke: Prop }

export const pareto = recipe<ParetoParams>({
  id: "@datars/std/pareto",
  doc: "The running share of the total, from the largest category down, as a line with points on the plot's right axis (`right: { domain: [0, 1], format: '.0%' }`) — over bars sorted descending, a Pareto chart.",
  params: { data: t.table(), x: t.field(), y: t.field(), xType: t.string("band"), stroke: t.prop("Line ink (default: the theme's secondary mark).") },
  tokens: ["ink-2"],
  expand(p, cx) {
    const tbl = cx.table("pareto", p.data, op.window("share_of_total", p.y, "share"), op.window("cumsum", "share", "cum_share", { order: `-${p.y}` }));
    return group({ key: "pareto", children: [line({ data: tbl, x: p.x, y: "cum_share", xType: p.xType, curve: "linear", points: true, stroke: p.stroke ?? "$ink-2", yScale: "y2", width: 2 })] });
  },
});
