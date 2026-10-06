// Comparisons and distributions — the charts a newsroom reaches for daily: before and after (slope,
// dumbbell), ranks over time (bump), values on a stem (lollipop), two sides of a population
// (pyramid), shares of shares (marimekko); and how values spread (histogram, boxplot, violin,
// ridgeline, errorBars, stackedArea, connectedScatter).
//
// Marks that live inside a `plot` read its scales and inherit its `data`, `x`, `y` and `color`; the
// plot asks `comparisonFrame` what they need from it (a value axis spanning bins or intervals, room
// for labels at both ends, no forced zero). `bump`, `pyramid` and `marimekko` are charts of their
// own, with their own scales.

import { e, group, instances, op, recipe, repeat, shape, geom, t, text, Cx, Prop, RecipeDef, ScaleDecl, Template } from "@datars/sdk";
import { axis } from "./axis.js";
import { grid } from "./grid.js";
import { legend } from "./guides.js";

const isBand = (type?: string) => type === "band" || type === "point";
/** A recipe's name, whatever its package (an ejected copy is still what it was). */
const nameOf = (id: unknown) => String(id ?? "").split("/").pop();
const colorOr = (color: string | undefined, fallback: Prop): Prop => (color ? e(`scale.color(d.${color})`) : fallback);
/** A category's centre on a band (or point) scale, or a value's position on a continuous one. */
function at(scale: string, field: string, type?: string): string {
  return isBand(type) ? `scale.${scale}(d.${field}) + scale.${scale}.bandwidth() / 2` : `scale.${scale}(d.${field})`;
}
// Named types, not inline ones, in helpers' signatures: `datars eject` copies a helper by its first
// brace block.
type Affix = { format?: string; prefix?: string; suffix?: string };
type DensityInput = { data: string; bandwidth: number; steps: number };
type DensityGrid = { trim?: boolean; extend?: number };
type DensityTables = { kde: string; stats: string };

/** Expression source for a number written with the format and its affixes (the sign first: −$3.2M). */
function written(v: string, p: Affix): string {
  const n = `format(abs(${v}), ${JSON.stringify(p.format ?? ",.1~f")})`;
  return `(${v} < 0 ? "−" : "") + ${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}`;
}
/** Is the row's series one of `names`? (An expression; true for every row when none are given.) */
function among(field: string, names: unknown): string {
  const list = Array.isArray(names) ? names : names === undefined || names === null || names === "" ? [] : [names];
  return list.length ? `(${list.map((n) => `d.${field} == ${JSON.stringify(String(n))}`).join(" || ")})` : "true";
}
/** Params with their declared defaults filled in (what the recipe itself will see). */
function withDefaults<P>(def: RecipeDef<P>, params: Record<string, unknown>): P {
  const out: Record<string, unknown> = { ...params };
  for (const [k, spec] of Object.entries(def.params as Record<string, { default?: unknown }>)) if (out[k] === undefined && spec.default !== undefined) out[k] = spec.default;
  return out as P;
}

// ---- lollipop -----------------------------------------------------------------------------------

export interface LollipopParams { data: string; x: string; y: string; color: string; xType: string; yType: string; r: number; fill: Prop; stroke: Prop; labels: boolean; format: string; prefix: string; suffix: string; label: Prop }

export const lollipop = recipe<LollipopParams>({
  id: "@datars/std/lollipop",
  doc: "A lollipop per category: a thin stem from zero to the value, a dot at its end — a bar chart with less ink, for many categories or values far from zero. Horizontal when y is the band axis.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("band", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("linear", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    r: t.number(5, "The dot's radius (px)."),
    fill: t.prop("The dot's ink (default: the colour scale, else $mark)."),
    stroke: t.prop("The stem's ink (default: the dot's)."),
    labels: t.bool(false, "Value labels past the dots (on the other side where the plot has no room)."),
    format: t.string(",.1~f", "Number format of the labels (and the tooltip)."),
    prefix: t.string(undefined, "Before each number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
    label: t.prop("Dot label (tooltip, accessible name): an expression over the row; default `name: value`."),
  },
  tokens: ["mark", "ink-2", "paper", "size.label"],
  expand(p) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const fill = p.fill ?? colorOr(p.color, "$mark");
    const c = at(cs, cat, "band"), v = `scale.${vs}(d.${val})`, zero = `scale.${vs}(0)`;
    const pt = (a: string, b: string): [Prop, Prop] => (horizontal ? [e(b), e(a)] : [e(a), e(b)]);
    const stem = horizontal ? geom.segment({ x1: e(zero), y1: e(c), x2: e(v), y2: e(c) }) : geom.segment({ x1: e(c), y1: e(zero), x2: e(c), y2: e(v) });
    const num = written(`d.${val}`, p);
    const lw = `measure(${num}, token("size.label"))`;
    // Past the dot, away from zero — unless that would leave the plot area: then on the stem's side.
    const out = `(d.${val} >= 0)`;
    const past = horizontal ? `(${out} ? ${v} + ${p.r + 4} + ${lw} <= box.w : ${v} - ${p.r + 4} - ${lw} >= 0)` : `(${out} ? ${v} - ${p.r + 4} - token("size.label") >= 0 : ${v} + ${p.r + 4} + token("size.label") <= box.h)`;
    const ahead = `(${out} == ${past})`; // true: the label sits on the high side of the dot
    const labelAt = horizontal
      ? pt(c, `${ahead} ? ${v} + ${p.r + 4} : ${v} - ${p.r + 4}`)
      : pt(c, `${ahead} ? ${v} - ${p.r + 4} : ${v} + ${p.r + 4}`);
    const style = horizontal
      ? { size: "$size.label", ink: "$ink-2", align: e(`${ahead} ? "start" : "end"`), baseline: "middle", contain: true }
      : { size: "$size.label", ink: "$ink-2", align: "middle", baseline: e(`${ahead} ? "alphabetic" : "top"`), contain: true };
    return group({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        group({ key: "stems", children: [repeat(p.data, shape(stem, { stroke: { paint: p.stroke ?? fill, width: 2, cap: "round" }, semantics: { role: "decoration" } }))] }),
        // The dots are the data: keyed by the row directly, like bars, so the two morph.
        repeat(p.data, shape(geom.circle({ cx: pt(c, v)[0], cy: pt(c, v)[1], r: p.r }), {
          fill,
          semantics: { role: "datum", label: p.label ?? e(`key.name(d.${cat}) + ": " + ${num}`), value: e(`d.${val}`) },
          pickable: true,
        })),
        p.labels ? group({ key: "labels", children: [repeat(p.data, text(e(num), labelAt, { halo: ["$paper", 2], style }))] }) : null,
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0 } }],
});

// ---- dumbbell -----------------------------------------------------------------------------------

export interface DumbbellParams { data: string; x: string; y: string; color: string; xType: string; yType: string; r: number; stroke: Prop; labels: boolean; format: string; prefix: string; suffix: string; label: Prop }

export const dumbbell = recipe<DumbbellParams>({
  id: "@datars/std/dumbbell",
  doc: "Two values per category joined by a bar: a dot per row (coloured by the plot's colour field — the year, the group), a connector from each category's lowest to highest. Rows are long: one per (category, value). Horizontal when y is the band axis.",
  params: {
    data: t.table("One row per dot: (category, colour field, value)."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Which dot is which (a year, a group): the colour scale."),
    xType: t.string("linear", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("band", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    r: t.number(5, "Dot radius (px)."),
    stroke: t.prop("The connector's ink (default $rule)."),
    labels: t.bool(false, "Each category's lowest value before its low dot and highest after its high dot."),
    format: t.string(",.1~f", "Number format of labels and tooltips."),
    prefix: t.string(undefined, "Before each number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
    label: t.prop("Dot label (tooltip, accessible name): an expression over the row; default `category, colour: value`."),
  },
  tokens: ["mark", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const ends = cx.table("dumbbell", p.data, op.aggregate([cat], { lo: ["min", val], hi: ["max", val], first: ["first", val], last: ["last", val] }));
    const c = at(cs, cat, "band");
    const pos = (v: string): [Prop, Prop] => (horizontal ? [e(`scale.x(${v})`), e(c)] : [e(c), e(`scale.y(${v})`)]);
    const [a, b] = [pos("d.lo"), pos("d.hi")];
    const dotLabel = p.color && p.color !== cat ? e(`key.name(d.${cat}) + ", " + key.name(d.${p.color}) + ": " + ${written(`d.${val}`, p)}`) : e(`key.name(d.${cat}) + ": " + ${written(`d.${val}`, p)}`);
    // Labels outside the pair: before the low dot, after the high one (on a -y axis, "after" is up).
    const gap = p.r + 4;
    const lo = horizontal ? [e(`scale.x(d.lo) - ${gap}`), e(c)] : [e(c), e(`scale.y(d.lo) + ${gap}`)];
    const hi = horizontal ? [e(`scale.x(d.hi) + ${gap}`), e(c)] : [e(c), e(`scale.y(d.hi) - ${gap}`)];
    const st = (side: "lo" | "hi") => (horizontal
      ? { size: "$size.label", ink: "$ink-2", align: side === "lo" ? "end" : "start", baseline: "middle", contain: true }
      : { size: "$size.label", ink: "$ink-2", align: "middle", baseline: side === "lo" ? "top" : "alphabetic", contain: true });
    return group({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        group({ key: "connectors", children: [repeat(ends, shape(geom.segment({ x1: a[0], y1: a[1], x2: b[0], y2: b[1] }), {
          stroke: { paint: p.stroke ?? "$rule", width: 3, cap: "round" },
          semantics: { role: "datum", label: e(`key.name(d.${cat}) + ": " + ${written("d.first", p)} + " → " + ${written("d.last", p)}`), value: e("d.last - d.first") },
          pickable: true,
        }))] }),
        repeat(p.data, shape(geom.circle({ cx: pos(`d.${val}`)[0], cy: pos(`d.${val}`)[1], r: p.r }), {
          fill: colorOr(p.color, "$mark"),
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: p.label ?? dotLabel, value: e(`d.${val}`) },
          pickable: true,
        })),
        p.labels ? group({ key: "labels", children: [repeat(ends, group({ children: [
          // Only where it fits inside the plot area (past its edge it would run into the axis).
          text(e(written("d.lo", p)), lo as [Prop, Prop], { key: "lo", when: e(horizontal ? `scale.x(d.lo) - ${gap} - measure(${written("d.lo", p)}, token("size.label")) >= 0` : `scale.y(d.lo) + ${gap} + token("size.label") <= box.h`), style: st("lo") }),
          text(e(written("d.hi", p)), hi as [Prop, Prop], { key: "hi", when: e("d.hi != d.lo"), style: st("hi") }),
        ] }))] }) : null,
      ],
    });
  },
});

// ---- slope --------------------------------------------------------------------------------------

export interface SlopeParams { data: string; x: string; y: string; color: string; xType: string; series: string; labels: "both" | "start" | "end" | "none"; values: boolean; format: string; prefix: string; suffix: string; stroke: Prop; width: number; r: number; highlight: unknown; label: Prop }

/** A slope chart's table: one row per series with its first and last `x` and value (`start`, `end`). */
function slopeTable(cx: Cx, p: SlopeParams, name = "slope"): string {
  const series = p.series ?? p.color;
  return cx.table(name, p.data, op.sort(p.x), op.aggregate([series], { x0: ["first", p.x], x1: ["last", p.x], start: ["first", p.y], end: ["last", p.y] }));
}
function slopeText(p: SlopeParams, side: "start" | "end"): string {
  const series = p.series ?? p.color;
  const name = `key.name(d.${series})`;
  if (!p.values) return name;
  return side === "start" ? `${name} + "  " + ${written("d.start", p)}` : `${written("d.end", p)} + "  " + ${name}`;
}
/** A width in px, or an expression for one (a layout size). */
type Room = number | { expr: string };
type Rooms = { start?: Room; end?: Room };

/** Room a slope's labels need beside the plot area: as wide as the widest (measured), per side. */
function slopeRoom(cx: Cx, p: SlopeParams): Rooms {
  if (p.labels === "none") return {};
  const tbl = cx.table("slope-widths", slopeTable(cx, p, "slope-rows"), op.derive("w0", e(`measure(${slopeText(p, "start")}, token("size.label"))`)), op.derive("w1", e(`measure(${slopeText(p, "end")}, token("size.label"))`)));
  const room = (w: string) => e(`min(box.w * 0.34, table.max(${JSON.stringify(tbl)}, "${w}") + 14)`);
  return { start: p.labels === "end" ? undefined : room("w0"), end: p.labels === "start" ? undefined : room("w1") };
}

export const slope = recipe<SlopeParams>({
  id: "@datars/std/slope",
  doc: "A slope chart: each series' first and last value (two years, before and after) joined by a line, named at both ends — labels pushed apart so none overlap, the plot leaving them room. Rows are long: (series, x, value) on a point x scale.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("The time (or before/after) field, on a point scale."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t.string("point", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"),
    series: t.field("One line per value of this field (default: the colour field)."),
    labels: t.oneOf(["both", "start", "end", "none"] as const, "both", "Where each line is named."),
    values: t.bool(true, "Each label carries its value (`Norway  78.7` · `83.2  Norway`)."),
    format: t.string(",.1~f", "Number format of values in labels and tooltips."),
    prefix: t.string(undefined, "Before each number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
    stroke: t.prop("Line ink or expression — over the row `d.start` and `d.end` (a series' first and last values) and the series field: `e('d.end > d.start ? \"$positive\" : \"$negative\"')`. Default: the colour scale, else $mark."),
    width: t.number(2, "Line width (px)."),
    r: t.number(4, "Dot radius at each end (px); 0 for none."),
    highlight: t.json("Series to pick out (a name or a list): they keep their ink and labels, the others turn grey."),
    label: t.prop("Line label (tooltip, accessible name); default `name: start → end`."),
  },
  tokens: ["mark", "muted", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const pairs = slopeTable(cx, p);
    const on = among(series, p.highlight);
    const hl = p.highlight !== undefined && p.highlight !== null;
    const base = p.stroke ?? (p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    // Picked-out series keep their ink; the rest recede to a faded grey.
    const ink: Prop = hl ? e(`${on} ? ${typeof base === "string" ? JSON.stringify(base) : (base as { expr: string }).expr} : "$rule"`) : base;
    const fade = hl ? e(`${on} ? 1 : 0.5`) : undefined;
    const x0 = at("x", "x0", p.xType), x1 = at("x", "x1", p.xType);
    const gap = 'token("size.label") + 3';
    const placed = cx.table("slope-labels", pairs,
      op.spread({ position: e("scale.y(d.start)"), gap: e(gap), min: 0, max: e("box.h"), as: "ly0" }),
      op.spread({ position: e("scale.y(d.end)"), gap: e(gap), min: 0, max: e("box.h"), as: "ly1" }));
    const labelInk = hl ? e(`${on} ? "$ink-2" : "$muted"`) : "$ink-2";
    const side = (s: "start" | "end") => text(e(slopeText(p, s)), [e(s === "start" ? `${x0} - ${p.r + 6}` : `${x1} + ${p.r + 6}`), e(s === "start" ? "d.ly0" : "d.ly1")], {
      style: { size: "$size.label", ink: labelInk, weight: hl ? e(`${on} ? 600 : 400`) : undefined, align: s === "start" ? "end" : "start", baseline: "middle" },
    });
    const dot = (xe: string, v: string, key: string) => shape(geom.circle({ cx: e(xe), cy: e(`scale.y(${v})`), r: p.r }), { key, fill: ink, opacity: fade, semantics: { role: "decoration" } });
    return group({
      key: "slopes",
      semantics: { role: "series", label: `${p.y} by ${series}` },
      children: [
        // The lines are the data: one per series, keyed by it.
        repeat(pairs, shape(geom.segment({ x1: e(x0), y1: e("scale.y(d.start)"), x2: e(x1), y2: e("scale.y(d.end)") }), {
          stroke: { paint: ink, width: hl ? e(`${on} ? ${p.width} : ${Math.max(1, p.width - 0.5)}`) : p.width, cap: "round" },
          opacity: fade,
          semantics: { role: "datum", label: p.label ?? e(`key.name(d.${series}) + ": " + ${written("d.start", p)} + " → " + ${written("d.end", p)}`), value: e("d.end - d.start") },
          pickable: true,
        })),
        p.r > 0 ? group({ key: "dots", children: [repeat(pairs, group({ children: [dot(x0, "d.start", "start"), dot(x1, "d.end", "end")] }))] }) : null,
        p.labels === "both" || p.labels === "start" ? group({ key: "start-labels", children: [repeat(placed, side("start"))] }) : null,
        p.labels === "both" || p.labels === "end" ? group({ key: "end-labels", children: [repeat(placed, side("end"))] }) : null,
      ],
    });
  },
  motion: [{ select: { kind: "segment" }, enter: { trim: 0 } }],
});

// ---- bump ---------------------------------------------------------------------------------------

export interface BumpParams { data: string; x: string; y: string; series: string; rank: boolean; reverse: boolean; title: string; subtitle: string; highlight: unknown; format: string; r: number; width: number; labelSpace: number }

export const bump = recipe<BumpParams>({
  id: "@datars/std/bump",
  doc: "A bump chart: each series' rank at every time, 1 at the top, joined by lines and named at both ends. Ranks come from the values at each time (largest first) or from a rank field; rows are long (series, time, value).",
  params: {
    data: t.table("One row per (series, time)."), x: t.field("The time field (in order: it's sorted)."), y: t.field("The value ranked at each time (or the rank itself, with `rank`)."),
    series: t.field("One line per value of this field."),
    rank: t.bool(false, "`y` already holds ranks (1 = top)."),
    reverse: t.bool(false, "The smallest value ranks first (times, prices)."),
    title: t.string(undefined, "A title above the chart."), subtitle: t.string(undefined, "A line under the title."),
    highlight: t.json("Series to pick out (a name or a list): they keep their colour and draw on top, the others turn grey."),
    format: t.string(",.4~g", "Number format of values in tooltips."),
    r: t.number(4.5, "Dot radius at each time (px)."),
    width: t.number(3, "Line width (px)."),
    labelSpace: t.number(140, "Most room (px) the names take at each side."),
  },
  tokens: ["ink", "ink-2", "muted", "grid", "rule", "paper", "size.label", "size.title", "font.title"],
  expand(p, cx) {
    const s = p.series;
    const ranked = cx.table("bump", p.data,
      p.rank ? op.derive("__rank", e(`d.${p.y}`)) : op.window("rank", p.y, "__rank", { partition: [p.x], order: p.reverse ? p.y : `-${p.y}` }),
      op.derive("__lo", e("d.__rank - 0.5")), op.derive("__hi", e("d.__rank + 0.5")), op.sort(p.x));
    const on = among(s, p.highlight);
    const hl = p.highlight !== undefined && p.highlight !== null;
    // Highlighted series last, so they draw on top of the grey ones.
    const drawn = hl ? cx.table("bump-drawn", ranked, op.derive("__on", e(`${on} ? 1 : 0`)), op.sort("__on", p.x)) : ranked;
    const ends = cx.table("bump-ends", drawn, op.aggregate([s], { x0: ["first", p.x], r0: ["first", "__rank"], x1: ["last", p.x], r1: ["last", "__rank"] }), op.derive("w", e(`measure(key.name(d.${s}), token("size.label"), 600)`)));
    const room = e(`min(${p.labelSpace}, table.max(${JSON.stringify(ends)}, "w") + 12)`);
    const ink = hl ? e(`${on} ? scale.color(d.${s}) : "$rule"`) : e(`scale.color(d.${s})`);
    const fade = hl ? e(`${on} ? 1 : 0.45`) : undefined;
    const X = `scale.x(d.${p.x})`, Y = "scale.y(d.__rank)";
    const value = p.rank ? "" : ` + " (" + format(d.${p.y}, ${JSON.stringify(p.format)}) + ")"`;
    const pointLabel = e(`key.name(d.${s}) + ", " + key.name(d.${p.x}) + ": rank " + d.__rank${value}`);
    const scales: Record<string, ScaleDecl> = {
      x: { type: "point", domain: { data: ranked, field: p.x }, range: { box: "bump-area", axis: "x" }, padding: 0 },
      y: { type: "linear", domain: { data: ranked, fields: ["__lo", "__hi"] }, range: { box: "bump-area", axis: "y" }, zero: false, nice: false },
      color: { type: "categorical", domain: { data: p.data, field: s }, range: "$categorical" },
    };
    const name = (end: "start" | "end") => text(e(`key.name(d.${s})`), [e(end === "start" ? `scale.x(d.x0) - ${p.r + 6}` : `scale.x(d.x1) + ${p.r + 6}`), e(end === "start" ? "scale.y(d.r0)" : "scale.y(d.r1)")], {
      style: { size: "$size.label", weight: 600, ink: hl ? e(`${on} ? scale.color(d.${s}) : "$muted"`) : e(`scale.color(d.${s})`), align: end === "start" ? "end" : "start", baseline: "middle" },
    });
    return group({
      key: "bump",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `Rank by ${p.y}` },
      children: [
        p.title ? text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }) : null,
        group({
          key: "body",
          layout: { type: "columns", gap: 0 },
          children: [
            group({ key: "start-room", size: { w: room } }),
            group({
              key: "center",
              layout: { type: "rows", gap: 8 },
              children: [
                group({
                  id: "bump-area",
                  key: "area",
                  children: [
                    grid({ scale: "x", orient: "vertical" }),
                    repeat({ groups: drawn, by: s }, group({
                      children: [
                        shape(geom.polyline({ from: "@group", x: e(X), y: e(Y), curve: "monotone-x" }), {
                          key: "line",
                          opacity: fade,
                          stroke: { paint: ink, width: hl ? e(`${on} ? ${p.width} : ${Math.max(1, p.width - 1.5)}`) : p.width, join: "round", cap: "round" },
                          semantics: { role: "series", label: e(`key.name(d.${s})`) },
                        }),
                        instances({ key: "points", from: "@group", x: e(X), y: e(Y), r: hl ? e(`${on} ? ${p.r} : ${Math.max(2, p.r - 1.5)}`) : p.r, fill: ink, opacity: fade, stroke: { paint: "$paper", width: 1.5 }, instanceKey: e(`d.${p.x}`), label: pointLabel }),
                      ],
                    })),
                    group({ key: "start-labels", children: [repeat(ends, name("start"))] }),
                    group({ key: "end-labels", children: [repeat(ends, name("end"))] }),
                  ],
                }),
                axis({ scale: "x", orient: "bottom", type: "point", data: ranked, field: p.x, line: false }, { size: { h: "auto" } }),
              ],
            }),
            group({ key: "end-room", size: { w: room } }),
          ],
        }),
      ],
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }],
});

// ---- pyramid ------------------------------------------------------------------------------------

export interface PyramidParams { data: string; y: string; side: string; value: string; left: string; title: string; subtitle: string; format: string; labels: boolean; prefix: string; suffix: string; padding: number }

export const pyramid = recipe<PyramidParams>({
  id: "@datars/std/pyramid",
  doc: "A population pyramid: a band per group (age, youngest at the bottom), one side's bars growing left and the other's right from a column of group names, both on one scale so the sides compare. Rows are long (group, side, value).",
  params: {
    data: t.table("One row per (group, side)."), y: t.field("The groups up the middle (ages), in data order from the bottom."),
    side: t.field("The field with two values, one per side (sex)."), value: t.field("The numbers (each row's count, sales, share)."),
    left: t.string(undefined, "The value of `side` drawn on the left (default: the first in the data)."),
    title: t.string(undefined, "A title above the chart."), subtitle: t.string(undefined, "A line under the title."),
    format: t.string(undefined, "Number format of the axes, labels and tooltips ('.1%', ',.0f')."),
    labels: t.bool(false, "Value labels at the bars' ends."),
    prefix: t.string(undefined, "Before each number."), suffix: t.string(undefined, "After it (' %', 'k')."),
    padding: t.number(0.12, "Space between the bars (band fraction)."),
  },
  tokens: ["ink", "ink-2", "muted", "size.label", "size.title", "font.title", "categorical"],
  expand(p, cx) {
    const rows = cx.table("pyramid", p.data, op.window("first", p.side, "__first"));
    const isLeft = p.left !== undefined ? `d.${p.side} == ${JSON.stringify(p.left)}` : `d.${p.side} == d.__first`;
    const leftRows = cx.table("pyramid-left", rows, op.filter(e(isLeft)));
    const rightRows = cx.table("pyramid-right", rows, op.filter(e(`!(${isLeft})`)));
    const sides = cx.table("pyramid-sides", rows, op.aggregate([p.side], { __first: ["first", "__first"] }));
    const groups = cx.table("pyramid-groups", p.data, op.aggregate([p.y], { n: ["count"] }), op.derive("w", e(`measure(key.name(d.${p.y}), token("size.label"))`)));
    const gutter = `(table.max(${JSON.stringify(groups)}, "w") + 18)`;
    const fmt = { format: p.format ?? ",.4~g", prefix: p.prefix, suffix: p.suffix };
    const valueScale = (box: string, axisDir: string): ScaleDecl => ({ type: "linear", domain: { data: p.data, field: p.value }, range: { box, axis: axisDir }, zero: true, nice: true });
    const scales: Record<string, ScaleDecl> = {
      y: { type: "band", domain: { data: p.data, field: p.y }, range: { box: "pyramid-left", axis: "-y" }, padding: p.padding },
      xl: valueScale("pyramid-left", "-x"),
      xr: valueScale("pyramid-right", "x"),
      color: { type: "categorical", domain: { data: p.data, field: p.side }, range: "$categorical" },
    };
    const bar = (s: "xl" | "xr") => shape(geom.rect({
      x: e(s === "xl" ? `scale.xl(d.${p.value})` : "0"), y: e(`scale.y(d.${p.y})`),
      w: e(s === "xl" ? `scale.xl(0) - scale.xl(d.${p.value})` : `scale.xr(d.${p.value})`), h: e("scale.y.bandwidth()"),
    }), {
      fill: e(`scale.color(d.${p.side})`),
      semantics: { role: "datum", label: e(`key.name(d.${p.side}) + ", " + key.name(d.${p.y}) + ": " + ${written(`d.${p.value}`, fmt)}`), value: e(`d.${p.value}`) },
      pickable: true,
    });
    const valueLabel = (s: "xl" | "xr") => text(e(written(`d.${p.value}`, fmt)), [e(s === "xl" ? `scale.xl(d.${p.value}) - 4` : `scale.xr(d.${p.value}) + 4`), e("scale.y(d." + p.y + ") + scale.y.bandwidth() / 2")], {
      when: e('scale.y.bandwidth() >= token("size.label") - 2'),
      style: { size: "$size.label", ink: "$ink-2", align: s === "xl" ? "end" : "start", baseline: "middle", contain: true },
    });
    const head = (left: boolean) => group({ key: "head", size: { h: "auto" }, children: [repeat(sides, text(e(`key.name(d.${p.side})`), [left ? e("box.w") : 0, 0], {
      when: e(left ? isLeft : `!(${isLeft})`),
      style: { size: "$size.label", weight: 600, ink: e(`scale.color(d.${p.side})`), align: left ? "end" : "start", baseline: "top" },
    }))] });
    const pane = (s: "xl" | "xr") => group({
      key: s === "xl" ? "left" : "right",
      layout: { type: "rows", gap: 6 },
      children: [
        head(s === "xl"),
        group({
          id: s === "xl" ? "pyramid-left" : "pyramid-right",
          key: "area",
          children: [
            grid({ scale: s, orient: "vertical" }),
            group({ key: "bars", children: [repeat(s === "xl" ? leftRows : rightRows, bar(s))] }),
            p.labels ? group({ key: "labels", children: [repeat(s === "xl" ? leftRows : rightRows, valueLabel(s))] }) : null,
            // The group names, centred in the gutter between the two sides.
            s === "xl" ? group({ key: "groups", children: [repeat(groups, text(e(`key.name(d.${p.y})`), [e(`box.w + ${gutter} / 2`), e(`scale.y(d.${p.y}) + scale.y.bandwidth() / 2`)], {
              when: e('scale.y.bandwidth() >= token("size.label") * 0.8'),
              style: { size: "$size.label", ink: "$muted", align: "middle", baseline: "middle" },
            }))] }) : null,
          ],
        }),
        axis({ scale: s, orient: "bottom", type: "linear", format: p.format, prefix: p.prefix, suffix: p.suffix }, { size: { h: "auto" } }),
      ],
    });
    return group({
      key: "pyramid",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `${p.value} by ${p.y} and ${p.side}` },
      children: [
        p.title ? text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }) : null,
        group({ key: "body", layout: { type: "columns", gap: 0 }, children: [pane("xl"), group({ key: "gutter", size: { w: e(gutter) } }), pane("xr")] }),
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0 } }],
});

// ---- marimekko ----------------------------------------------------------------------------------

export interface MarimekkoParams { data: string; x: string; series: string; value: string; title: string; subtitle: string; format: string; labels: boolean; legend: boolean; gap: number }

export const marimekko = recipe<MarimekkoParams>({
  id: "@datars/std/marimekko",
  doc: "A marimekko (mosaic): a column per category as wide as its share of the total, split into segments as tall as each series' share within it — so every area is that pair's share of everything. Segments are keyed (series, column), like stacked bars.",
  params: {
    data: t.table("One row per (column, series)."), x: t.field("The columns (markets, regions)."), series: t.field("The segments within each column (and the colours)."), value: t.field("The numbers (each row's count, sales, share)."),
    title: t.string(undefined, "A title above the chart."), subtitle: t.string(undefined, "A line under the title."),
    format: t.string(",.4~g", "Number format of values in tooltips."),
    labels: t.bool(true, "Series names and shares inside the segments that fit them."),
    legend: t.bool(true, "A colour legend below."),
    gap: t.number(2, "Gap between segments (px)."),
  },
  tokens: ["ink", "ink-2", "muted", "size.label", "size.small", "size.title", "font.title", "categorical"],
  expand(p, cx) {
    const cols = cx.table("marimekko-columns", p.data, op.aggregate([p.x], { __total: ["sum", p.value] }), op.window("share_of_total", "__total", "__share"), op.window("cumsum", "__share", "__x1"), op.derive("__x0", e("d.__x1 - d.__share")));
    const cells = cx.table("marimekko", p.data, op.stack({ x: p.x, series: p.series, value: p.value, offset: "expand", as: ["y0", "y1"] }), op.join(cols, p.x));
    const g = p.gap / 2;
    const [x0, x1, y0, y1] = ["scale.mx(d.__x0)", "scale.mx(d.__x1)", "scale.my(d.y0)", "scale.my(d.y1)"];
    const w = `(${x1} - ${x0} - ${p.gap})`, h = `(${y0} - ${y1} - ${p.gap})`;
    const name = `key.name(d.${p.series})`;
    const share = `format(d.y1 - d.y0, ".0%")`;
    const fits = `${w} >= max(measure(${name}, token("size.label"), 600), measure(${share}, token("size.label"))) + 10 && ${h} >= 2 * token("size.label") + 10`;
    const onFill = e(`"on(" + scale.color(d.${p.series}) + ")"`);
    const colName = `key.name(d.${p.x})`;
    const colW = `(scale.mx(d.__x1) - scale.mx(d.__x0) - ${p.gap})`;
    return group({
      key: "marimekko",
      scales: {
        mx: { type: "linear", domain: [0, 1], range: { box: "marimekko-area", axis: "x" } },
        my: { type: "linear", domain: [0, 1], range: { box: "marimekko-area", axis: "-y" } },
        color: { type: "categorical", domain: { data: p.data, field: p.series }, range: "$categorical" },
      },
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `${p.value} by ${p.x} and ${p.series}` },
      children: [
        p.title ? text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }) : null,
        group({
          key: "body",
          layout: { type: "columns", gap: 6 },
          children: [
            axis({ scale: "my", orient: "left", format: ".0%", line: false }, { size: { w: "auto" } }),
            group({
              key: "center",
              layout: { type: "rows", gap: 6 },
              children: [
                group({
                  id: "marimekko-area",
                  key: "area",
                  children: [
                    repeat(cells, shape(geom.rect({ x: e(`${x0} + ${g}`), y: e(`${y1} + ${g}`), w: e(`max(0, ${w})`), h: e(`max(0, ${h})`) }), {
                      key: [e(`d.${p.series}`), e(`d.${p.x}`)],
                      fill: e(`scale.color(d.${p.series})`),
                      semantics: { role: "datum", label: e(`${name} + " in " + ${colName} + ": " + format(d.${p.value}, ${JSON.stringify(p.format)}) + " (" + ${share} + " of " + ${colName} + ", " + format((d.y1 - d.y0) * d.__share, ".1%") + " of all)"`), value: e(`d.${p.value}`) },
                      pickable: true,
                    })),
                    p.labels ? group({ key: "labels", children: [repeat(cells, group({ when: e(fits), children: [
                      text(e(name), [e(`${x0} + ${g} + 6`), e(`${y1} + ${g} + 5`)], { key: "name", style: { size: "$size.label", weight: 600, ink: onFill, baseline: "top" } }),
                      text(e(share), [e(`${x0} + ${g} + 6`), e(`${y1} + ${g} + 7 + token("size.label")`)], { key: "share", style: { size: "$size.label", ink: onFill, baseline: "top" } }),
                    ] }))] }) : null,
                  ],
                }),
                // Each column named under it, with its share of the total — where it has the width.
                group({ key: "columns", size: { h: "auto" }, children: [repeat(cols, group({ when: e(`${colW} >= 28`), children: [
                  text(e(colName), [e("(scale.mx(d.__x0) + scale.mx(d.__x1)) / 2"), 0], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e(colW) } }),
                  text(e('format(d.__share, ".0%")'), [e("(scale.mx(d.__x0) + scale.mx(d.__x1)) / 2"), e('token("size.label") + 5')], { key: "share", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
                ] }))] }),
              ],
            }),
          ],
        }),
        p.legend ? legend({ scale: "color" }, { size: { h: "auto" } }) : null,
      ],
    });
  },
});

// ---- histogram ----------------------------------------------------------------------------------

export interface HistogramParams { data: string; x: string; color: string; xType: string; bins: number; step: number; density: boolean; gap: number; fill: Prop; format: string; label: Prop }

/** A histogram's table: a row per bin (per colour) with `bin`, `bin_end`, `count`, `density` and
 * the stacked extent `y0`–`y1` of whichever the bars show. */
function histogramTable(cx: Cx, p: HistogramParams): string {
  const color = p.color && p.color !== p.x ? p.color : undefined;
  const y = p.density ? "density" : "count";
  return cx.table("histogram", p.data,
    op.filter(e(`d.${p.x} == d.${p.x}`)), // no nulls (NaN is not itself)
    op.bin(p.x, "bin", p.step > 0 ? { step: p.step } : { count: p.bins }),
    op.aggregate(color ? ["bin", "bin_end", color] : ["bin", "bin_end"], { count: ["count"] }),
    op.window("share_of_total", "count", "__share"),
    op.derive("density", e("d.__share / (d.bin_end - d.bin)")),
    ...(color ? [op.stack({ x: "bin", series: color, value: y, as: ["y0", "y1"] })] : [op.derive("y0", 0), op.derive("y1", e(`d.${y}`))]));
}

export const histogram = recipe<HistogramParams>({
  id: "@datars/std/histogram",
  doc: "A histogram: the rows of `x` counted into bins of equal width (about `bins` of them at a round width, or exactly `step` wide), a bar per bin from its start to its end. `density` shows the share per unit instead, so the bars' area is 1; a colour field stacks the bins. The plot's axes fit the bins.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("The numeric field to bin."), color: t.field("Stack each bin by this field."), xType: t.string("linear", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"),
    bins: t.number(10, "About this many bins, at a round width (1, 2, 5 × 10ⁿ)."),
    step: t.number(0, "Bins exactly this wide (anchored at 0), instead of `bins`."),
    density: t.bool(false, "Bar heights as density (share of rows per unit of x) instead of counts: the bars' total area is 1, whatever the bin width."),
    gap: t.number(1, "Gap between bars (px)."),
    fill: t.prop("Bar ink (default: the colour scale, else $mark)."),
    format: t.string(",.6~g", "Number format of the bin edges in tooltips."),
    label: t.prop("Bar label (tooltip, accessible name); default `start–end: count`."),
  },
  tokens: ["mark"],
  expand(p, cx) {
    const tbl = histogramTable(cx, p);
    const color = p.color && p.color !== p.x ? p.color : undefined;
    const f = JSON.stringify(p.format);
    const what = p.density ? `format(d.density, ".3~g") + " per unit (" + format(d.__share, ".1%") + ")"` : `format(d.count, ",d")`;
    return group({
      key: "bins",
      semantics: { role: "series", label: `${p.x}, binned` },
      children: [repeat(tbl, shape(geom.rect({ x: e(`scale.x(d.bin) + ${p.gap / 2}`), y: e("scale.y(d.y1)"), w: e(`max(0.5, scale.x(d.bin_end) - scale.x(d.bin) - ${p.gap})`), h: e("max(0, scale.y(d.y0) - scale.y(d.y1))") }), {
        key: color ? [e(`d.${color}`), e("d.bin")] : e("d.bin"),
        fill: p.fill ?? colorOr(color, "$mark"),
        semantics: { role: "datum", label: p.label ?? e(`${color ? `key.name(d.${color}) + ", " + ` : ""}format(d.bin, ${f}) + "–" + format(d.bin_end, ${f}) + ": " + ${what}`), value: e(p.density ? "d.density" : "d.count") },
        pickable: true,
      }))],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }],
});

// ---- boxplot ------------------------------------------------------------------------------------

export interface BoxplotParams { data: string; x: string; y: string; color: string; xType: string; yType: string; whisker: number; width: number; outliers: boolean; fill: Prop; format: string; label: Prop }

export const boxplot = recipe<BoxplotParams>({
  id: "@datars/std/boxplot",
  doc: "A box plot per category: the box spans the middle half (first to third quartile) with a line at the median; whiskers reach the furthest values within 1.5 × the box's height (`whisker`) of it, and values beyond are drawn as outliers. Horizontal when y is the band axis.",
  params: {
    data: t.table("One row per observation."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("band", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("linear", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    whisker: t.number(1.5, "How far whiskers reach, in interquartile ranges beyond the box (Tukey's 1.5)."),
    width: t.number(0.7, "Box width as a fraction of the band (at most 72 px)."),
    outliers: t.bool(true, "Draw the values beyond the whiskers."),
    fill: t.prop("Box ink (default: the colour scale, else $mark)."),
    format: t.string(",.1~f", "Number format in tooltips."),
    label: t.prop("Box label (tooltip, accessible name); default the five numbers and the count."),
  },
  tokens: ["mark", "ink-2", "paper"],
  expand(p, cx) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const color = p.color && p.color !== cat ? p.color : undefined;
    const stats = cx.table("box-stats", p.data, op.aggregate([cat], { __q1: ["q25", val], __median: ["median", val], __q3: ["q75", val], __n: ["count", val] }));
    const k = p.whisker;
    const rows = cx.table("box-rows", p.data, op.join(stats, cat), op.derive("__out", e(`d.${val} < d.__q1 - ${k} * (d.__q3 - d.__q1) || d.${val} > d.__q3 + ${k} * (d.__q3 - d.__q1)`)));
    const box = cx.table("box", rows, op.filter(e("!d.__out")), op.aggregate([cat], { lo: ["min", val], hi: ["max", val], q1: ["first", "__q1"], median: ["first", "__median"], q3: ["first", "__q3"], n: ["first", "__n"], ...(color ? { [color]: ["first", color] } : {}) }));
    const outliers = cx.table("box-outliers", rows, op.filter(e("d.__out")));
    const fill = p.fill ?? (p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    const c = at(cs, cat, "band");
    const half = `min(scale.${cs}.bandwidth() * ${p.width}, 72) / 2`;
    const V = (v: string) => `scale.${vs}(${v})`;
    // (along, across) → (x, y): along the value axis, across the band.
    const P = (along: string, across: string): [Prop, Prop] => (horizontal ? [e(along), e(across)] : [e(across), e(along)]);
    const seg = (a0: string, c0: string, a1: string, c1: string) => { const [x1, y1] = P(a0, c0), [x2, y2] = P(a1, c1); return geom.segment({ x1, y1, x2, y2 }); };
    const rect = horizontal
      ? geom.rect({ x: e(V("d.q1")), y: e(`${c} - ${half}`), w: e(`${V("d.q3")} - ${V("d.q1")}`), h: e(`2 * ${half}`) })
      : geom.rect({ x: e(`${c} - ${half}`), y: e(V("d.q3")), w: e(`2 * ${half}`), h: e(`${V("d.q1")} - ${V("d.q3")}`) });
    const f = (v: string) => `format(${v}, ${JSON.stringify(p.format)})`;
    const summary = e(`key.name(d.${cat}) + ": median " + ${f("d.median")} + ", middle half " + ${f("d.q1")} + "–" + ${f("d.q3")} + ", whiskers " + ${f("d.lo")} + "–" + ${f("d.hi")} + " (n = " + format(d.n, ",d") + ")"`);
    const whiskerInk = "$ink-2";
    return group({
      key: "boxes",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        repeat(box, group({ children: [
          shape(seg(V("d.lo"), c, V("d.q1"), c), { key: "whisker-lo", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape(seg(V("d.q3"), c, V("d.hi"), c), { key: "whisker-hi", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape(seg(V("d.lo"), `${c} - ${half} / 2`, V("d.lo"), `${c} + ${half} / 2`), { key: "cap-lo", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape(seg(V("d.hi"), `${c} - ${half} / 2`, V("d.hi"), `${c} + ${half} / 2`), { key: "cap-hi", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape(rect, { key: "box", fill, semantics: { role: "datum", label: p.label ?? summary, value: e("d.median") }, pickable: true }),
          shape(seg(V("d.median"), `${c} - ${half}`, V("d.median"), `${c} + ${half}`), { key: "median", stroke: { paint: "$paper", width: 2 }, semantics: { role: "decoration" } }),
        ] })),
        // Outliers are few: a hollow circle each, ringed in its box's ink.
        p.outliers ? group({ key: "outliers", children: [repeat(outliers, shape(geom.circle({ cx: P(V(`d.${val}`), c)[0], cy: P(V(`d.${val}`), c)[1], r: 3 }), {
          fill: "$paper", stroke: { paint: fill, width: 1.3 },
          semantics: { role: "datum", label: e(`key.name(d.${cat}) + ": " + ${f(`d.${val}`)} + " (beyond the whiskers)"`), value: e(`d.${val}`) },
          pickable: true,
        }))] }) : null,
      ],
    });
  },
});

// ---- violin -------------------------------------------------------------------------------------

export interface ViolinParams { data: string; x: string; y: string; color: string; xType: string; yType: string; bandwidth: number; steps: number; width: number; normalize: "area" | "width"; quartiles: boolean; fill: Prop; opacity: number; format: string }

/** A density table per category (op.kde) with each category's quartiles and count joined on. */
function densityTable(cx: Cx, name: string, p: DensityInput, cat: string, val: string, color: string | undefined, grid: DensityGrid): DensityTables {
  const stats = cx.table(`${name}-stats`, p.data, op.aggregate([cat], { __q1: ["q25", val], __median: ["median", val], __q3: ["q75", val], __n: ["count", val] }));
  // Split by a colour within a category: one curve per (category, colour), grouped by `__g`.
  const kde = cx.table(name, p.data, op.kde({ field: val, groupby: color ? [cat, color] : [cat], bandwidth: p.bandwidth || undefined, steps: p.steps, ...grid }), op.join(stats, cat),
    op.derive("__g", e(color ? `d.${cat} + " | " + d.${color}` : `d.${cat}`)));
  return { kde, stats };
}

export const violin = recipe<ViolinParams>({
  id: "@datars/std/violin",
  doc: "A violin per category: the values' kernel density (Gaussian, Silverman's bandwidth unless given) mirrored around the category's centre, over the range of the data — its shape shows where values bunch, where a box plot shows five numbers. A bar marks the middle half and a dot the median. Horizontal when y is the band axis.",
  params: {
    data: t.table("One row per observation."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("band", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("linear", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    bandwidth: t.number(0, "Kernel bandwidth in value units (0: Silverman's rule, per category). Smaller follows the data more closely; larger smooths."),
    steps: t.number(80, "Points along each outline."),
    width: t.number(0.9, "The widest violin's width as a fraction of the band."),
    normalize: t.oneOf(["area", "width"] as const, "area", "`area`: every violin encloses the same area, so a narrow spread is tall and wide; `width`: every violin is as wide as the band at its peak."),
    quartiles: t.bool(true, "A bar over the middle half and a dot at the median."),
    fill: t.prop("Ink (default: the colour scale, else $mark)."),
    opacity: t.number(0.85, "Fill opacity."),
    format: t.string(",.1~f", "Number format in tooltips."),
  },
  tokens: ["mark", "ink", "paper"],
  expand(p, cx) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const color = p.color && p.color !== cat ? p.color : undefined;
    const { kde, stats } = densityTable(cx, "violin", p, cat, val, color, { trim: true });
    const c = at(cs, cat, "band");
    const peak = p.normalize === "width" ? 'group.max("density")' : `table.max(${JSON.stringify(kde)}, "density")`;
    const half = `d.density / max(${peak}, 1e-300) * scale.${cs}.bandwidth() * ${p.width} / 2`;
    const fill = p.fill ?? (p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    const f = (v: string) => `format(${v}, ${JSON.stringify(p.format)})`;
    const label = e(`key.name(d.${cat}) + ": median " + ${f("d.__median")} + ", middle half " + ${f("d.__q1")} + "–" + ${f("d.__q3")} + " (n = " + format(d.__n, ",d") + ")"`);
    // Upright violins are areas along the value axis drawn on their side: turned a quarter (−90°),
    // an area's x runs up the screen, so x is minus the value's y and y0/y1 are the two sides.
    const outline = horizontal
      ? shape(geom.area({ from: "@group", x: e(`scale.x(d.value)`), y0: e(`${c} - ${half}`), y1: e(`${c} + ${half}`), curve: "monotone-x" }), { key: "violin", fill, opacity: p.opacity, semantics: { role: "datum", label, value: e("d.__median") }, pickable: true })
      : shape(geom.area({ from: "@group", x: e(`-scale.y(d.value)`), y0: e(`${c} - ${half}`), y1: e(`${c} + ${half}`), curve: "monotone-x" }), { key: "violin", transform: { rotate: -90 }, fill, opacity: p.opacity, semantics: { role: "datum", label, value: e("d.__median") }, pickable: true });
    const V = (v: string) => `scale.${vs}(${v})`;
    const iqr = horizontal
      ? geom.rect({ x: e(V("d.__q1")), y: e(`${c} - 1.5`), w: e(`${V("d.__q3")} - ${V("d.__q1")}`), h: 3 })
      : geom.rect({ x: e(`${c} - 1.5`), y: e(V("d.__q3")), w: 3, h: e(`${V("d.__q1")} - ${V("d.__q3")}`) });
    const med = horizontal ? geom.circle({ cx: e(V("d.__median")), cy: e(c), r: 2.5 }) : geom.circle({ cx: e(c), cy: e(V("d.__median")), r: 2.5 });
    return group({
      key: "violins",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        repeat({ groups: kde, by: "__g" }, group({ children: [outline] })),
        p.quartiles ? group({ key: "quartiles", children: [repeat(stats, group({ children: [
          shape(iqr, { key: "iqr", fill: "$ink", opacity: 0.75, semantics: { role: "decoration" } }),
          shape(med, { key: "median", fill: "$paper", semantics: { role: "decoration" } }),
        ] }))] }) : null,
      ],
    });
  },
});

// ---- ridgeline ----------------------------------------------------------------------------------

export interface RidgelineParams { data: string; x: string; y: string; color: string; xType: string; yType: string; bandwidth: number; steps: number; overlap: number; fill: Prop; opacity: number; format: string }

/** A ridgeline's curves: on one grid over the data, widened by three bandwidths so tails taper. */
function ridgelineTable(cx: Cx, p: RidgelineParams): DensityTables {
  const color = p.color && p.color !== p.y ? p.color : undefined;
  return densityTable(cx, "ridgeline", p, p.y, p.x, color, { extend: 3 });
}

export const ridgeline = recipe<RidgelineParams>({
  id: "@datars/std/ridgeline",
  doc: "Ridgelines: a kernel density curve per category (a row of the plot's band y axis) over a shared value axis, rising from the row's baseline into the rows above — many distributions (months, years, groups) compared in little space. Densities share one scale; the tallest ridge rises `overlap` rows, less if the top row would leave the plot.",
  params: {
    data: t.table("One row per observation."), x: t.field("The values (a linear x)."), y: t.field("The categories (a band y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("linear", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("band", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    bandwidth: t.number(0, "Kernel bandwidth in value units (0: Silverman's rule, per category)."),
    steps: t.number(96, "Points along each curve."),
    overlap: t.number(1.6, "How many rows the tallest ridge rises (1: it just meets the row above)."),
    fill: t.prop("Ink (default: the colour scale, else $mark)."),
    opacity: t.number(0.85, "Fill opacity."),
    format: t.string(",.1~f", "Number format in tooltips."),
  },
  tokens: ["mark", "paper"],
  expand(p, cx) {
    const cat = p.y, val = p.x;
    const { kde } = ridgelineTable(cx, p);
    const peaks = cx.table("ridgeline-peaks", kde, op.aggregate([cat], { m: ["max", "density"] }));
    // One px-per-density for every ridge: `overlap` rows for the tallest, unless the top row's
    // ridge would then rise past the top of the plot area (it has half a padding and a band).
    const k = `min(${p.overlap} * scale.y.step() / max(table.max(${JSON.stringify(kde)}, "density"), 1e-300), (scale.y.step() + scale.y.bandwidth()) / 2 / max(table.first(${JSON.stringify(peaks)}, "m"), 1e-300))`;
    const base = `scale.y(d.${cat}) + scale.y.bandwidth()`;
    const fill = p.fill ?? (p.color ? e(`scale.color(d.${p.color})`) : "$mark");
    const f = (v: string) => `format(${v}, ${JSON.stringify(p.format)})`;
    return group({
      key: "ridges",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [repeat({ groups: kde, by: "__g" }, group({ children: [
        shape(geom.area({ from: "@group", x: e("scale.x(d.value)"), y0: e(base), y1: e(`${base} - d.density * ${k}`), curve: "monotone-x" }), {
          key: "ridge", fill, opacity: p.opacity, stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: e(`key.name(d.${cat}) + ": median " + ${f("d.__median")} + ", middle half " + ${f("d.__q1")} + "–" + ${f("d.__q3")} + " (n = " + format(d.__n, ",d") + ")"`), value: e("d.__median") },
          pickable: true,
        }),
      ] }))],
    });
  },
});

// ---- errorBars ----------------------------------------------------------------------------------

export interface ErrorBarsParams { data: string; x: string; y: string; color: string; xType: string; yType: string; lo: string; hi: string; error: string; cap: number; r: number; stroke: Prop; width: number; format: string; label: Prop }

/** The rows with each interval's ends as `__lo` and `__hi`. */
function errorTable(cx: Cx, p: ErrorBarsParams): string {
  const val = isBand(p.yType) && !isBand(p.xType) ? p.x : p.y;
  const lo = p.lo ? `d.${p.lo}` : p.error ? `d.${val} - d.${p.error}` : `d.${val}`;
  const hi = p.hi ? `d.${p.hi}` : p.error ? `d.${val} + d.${p.error}` : `d.${val}`;
  return cx.table("error-bars", p.data, op.derive("__lo", e(lo)), op.derive("__hi", e(hi)));
}

export const errorBars = recipe<ErrorBarsParams>({
  id: "@datars/std/errorBars",
  doc: "Error bars: each row's interval as a line with caps — from `lo` to `hi` columns, or the value ± `error` — around a dot at the value (a poll's margin, a confidence interval). Over bars, points or alone; the plot's value axis reaches the intervals' ends. Horizontal when y is the band axis.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("band", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("linear", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    lo: t.field("The interval's low end (with `hi`)."), hi: t.field("Its high end."),
    error: t.field("A ± half-width instead of `lo`/`hi` (a margin of error)."),
    cap: t.number(8, "Cap width (px); 0 for none."),
    r: t.number(4, "The dot's radius (px); 0 for none (over bars)."),
    stroke: t.prop("Ink of the interval and dot (default: the colour scale, else $ink-2)."),
    width: t.number(1.5, "Line width (px)."),
    format: t.string(",.1~f", "Number format in tooltips."),
    label: t.prop("Label (tooltip, accessible name); default `name: value (lo–hi)`."),
  },
  tokens: ["ink-2", "paper"],
  expand(p, cx) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const tbl = errorTable(cx, p);
    const c = at(cs, cat, horizontal ? p.yType : p.xType);
    const V = (v: string) => `scale.${vs}(${v})`;
    const P = (along: string, across: string): [Prop, Prop] => (horizontal ? [e(along), e(across)] : [e(across), e(along)]);
    const seg = (a0: string, c0: string, a1: string, c1: string) => { const [x1, y1] = P(a0, c0), [x2, y2] = P(a1, c1); return geom.segment({ x1, y1, x2, y2 }); };
    const ink = p.stroke ?? colorOr(p.color, "$ink-2");
    const stroke = { paint: ink, width: p.width, cap: "butt" as const };
    const f = (v: string) => `format(${v}, ${JSON.stringify(p.format)})`;
    const name = isBand(horizontal ? p.yType : p.xType) ? `key.name(d.${cat})` : `scale.${cs}.label(d.${cat})`;
    const dot = P(V(`d.${val}`), c);
    return group({
      key: "error-bars",
      semantics: { role: "series", label: `${val} with intervals` },
      children: [repeat(tbl, group({ children: [
        shape(seg(V("d.__lo"), c, V("d.__hi"), c), { key: "interval", stroke, semantics: { role: "datum", label: p.label ?? e(`${name} + ": " + ${f(`d.${val}`)} + " (" + ${f("d.__lo")} + "–" + ${f("d.__hi")} + ")"`), value: e(`d.${val}`) }, pickable: true }),
        p.cap > 0 ? shape(seg(V("d.__lo"), `${c} - ${p.cap / 2}`, V("d.__lo"), `${c} + ${p.cap / 2}`), { key: "cap-lo", stroke, semantics: { role: "decoration" } }) : null,
        p.cap > 0 ? shape(seg(V("d.__hi"), `${c} - ${p.cap / 2}`, V("d.__hi"), `${c} + ${p.cap / 2}`), { key: "cap-hi", stroke, semantics: { role: "decoration" } }) : null,
        p.r > 0 ? shape(geom.circle({ cx: dot[0], cy: dot[1], r: p.r }), { key: "dot", fill: ink, stroke: { paint: "$paper", width: 1 }, semantics: { role: "decoration" } }) : null,
      ] }))],
    });
  },
});

// ---- stackedArea (and streamgraph) --------------------------------------------------------------

export interface StackedAreaParams { data: string; x: string; y: string; color: string; xType: string; series: string; offset: "zero" | "expand" | "silhouette" | "wiggle"; order: "input" | "reverse" | "ascending" | "descending" | "inside-out"; curve: string; opacity: number; labels: boolean; format: string }

function stackedAreaTable(cx: Cx, p: StackedAreaParams): string {
  const series = p.series ?? p.color;
  // (`__x`: x as a number — dates too — for the table's extent.)
  return cx.table("stacked-area", p.data, ...(isBand(p.xType) ? [] : [op.sort(p.x), op.derive("__x", e(`d.${p.x}`))]), op.stack({ x: p.x, series, value: p.y, offset: p.offset, order: p.order, as: ["y0", "y1"] }));
}

export const stackedArea = recipe<StackedAreaParams>({
  id: "@datars/std/stackedArea",
  doc: "Stacked areas: each series a layer on the ones below it, through x — totals and their parts over time. `offset: \"expand\"` stretches every x to 100 %; `\"wiggle\"` (with `order: \"inside-out\"`) makes a streamgraph, layers flowing around a moving centre; `\"silhouette\"` centres them. The plot's value axis fits the stack.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("linear", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"),
    series: t.field("One layer per value of this field (default: the colour field)."),
    offset: t.oneOf(["zero", "expand", "silhouette", "wiggle"] as const, "zero", "The baseline: zero, stretched to 100 %, centred, or a streamgraph's wiggle."),
    order: t.oneOf(["input", "reverse", "ascending", "descending", "inside-out"] as const, "input", "Which layers sit at the baseline: as in the data, by total, or the biggest in the middle (streamgraphs)."),
    curve: t.string("monotone-x", "How layers' edges pass through the points: `monotone-x` (smooth, no overshoot), `linear`, `step`, …"),
    opacity: t.number(0.9, "Fill opacity."),
    labels: t.bool(false, "Each layer named inside, where it is thickest (when a name fits)."),
    format: t.string(",.4~g", "Number format of values in tooltips."),
  },
  tokens: ["mark", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const tbl = stackedAreaTable(cx, p);
    const X = at("x", p.x, p.xType);
    const fill = p.color ? e(`scale.color(d.${p.color})`) : "$mark";
    const xLabel = isBand(p.xType) ? `key.name(d.${p.x})` : `scale.x.label(d.${p.x})`;
    const value = p.offset === "expand" ? `format(d.y1 - d.y0, ".1%")` : `format(d.${p.y}, ${JSON.stringify(p.format)})`;
    // Where each layer is thickest: its name goes there, if it fits.
    const thick = cx.table("stacked-area-labels", tbl, op.derive("__t", e("d.y1 - d.y0")), op.window("rank", "__t", "__r", { partition: [series] }), op.filter(e("d.__r == 1")), op.aggregate([series], { x: ["first", p.x], y0: ["first", "y0"], y1: ["first", "y1"] }));
    const xt = isBand(p.xType) ? "scale.x(d.x) + scale.x.bandwidth() / 2" : "scale.x(d.x)";
    const name = `key.name(d.${series})`;
    const lw = `measure(${name}, token("size.label"), 600)`;
    const T = JSON.stringify(tbl);
    const [xlo, xhi] = isBand(p.xType) ? ["scale.x.min()", "scale.x.max()"] : [`scale.x(table.min(${T}, "__x"))`, `scale.x(table.max(${T}, "__x"))`];
    return group({
      key: "areas",
      semantics: { role: "series", label: `${p.y} by ${series}, stacked` },
      children: [
        repeat({ groups: tbl, by: series }, group({ children: [
          shape(geom.area({ from: "@group", x: e(X), y0: e("scale.y(d.y0)"), y1: e("scale.y(d.y1)"), curve: p.curve }), {
            key: "area", fill, opacity: p.opacity, semantics: { role: "series", label: e(name) },
          }),
          // Hover anywhere along a layer: its value at the nearest x.
          instances({ key: "points", from: "@group", x: e(X), y: e("(scale.y(d.y0) + scale.y(d.y1)) / 2"), r: 0, fill: "transparent", instanceKey: e(`d.${p.x}`), label: e(`${name} + " · " + ${xLabel} + ": " + ${value}`), hit: "line" }),
        ] })),
        // (Kept over the layers: one thickest at the first or last x is named just inside it.)
        p.labels ? group({ key: "labels", children: [repeat(thick, text(e(name), [e(`clamp(${xt}, ${xlo} + ${lw} / 2 + 4, ${xhi} - ${lw} / 2 - 4)`), e("(scale.y(d.y0) + scale.y(d.y1)) / 2")], {
          when: e('abs(scale.y(d.y0) - scale.y(d.y1)) >= token("size.label") + 6'),
          style: { size: "$size.label", weight: 600, ink: p.color ? e(`"on(" + scale.color(d.${p.color}) + ")"`) : "$accent-ink", align: "middle", baseline: "middle", contain: true },
        }))] }) : null,
      ],
    });
  },
});

// ---- connectedScatter ---------------------------------------------------------------------------

export interface ConnectedScatterParams { data: string; x: string; y: string; color: string; xType: string; yType: string; order: string; series: string; text: string; labels: "ends" | "all" | "none"; every: number; curve: string; r: number; arrow: boolean; stroke: Prop; format: string }

export const connectedScatter = recipe<ConnectedScatterParams>({
  id: "@datars/std/connectedScatter",
  doc: "A connected scatter plot: points at (x, y) joined in the order of a third field (usually time), an arrow at the end — how two measures moved together. Labelled at the ends (and every n-th point), labels kept off each other.",
  params: {
    data: t.table("The rows (inside a plot: its data)."), x: t.field("Field on the x axis (inside a plot: its x)."), y: t.field("Field on the y axis (inside a plot: its y)."), color: t.field("Field on the colour scale (inside a plot: its colour)."), xType: t.string("linear", "The x scale\'s type (the plot passes it): `band`/`point` for categories, `linear`, `time`, …"), yType: t.string("linear", "The y scale\'s type (the plot passes it); a band y lays the chart on its side."),
    order: t.field("The field the points are joined in (a year, a date); default: row order."),
    series: t.field("One path per value of this field (default: the colour field)."),
    text: t.field("Point labels (default: the order field)."),
    labels: t.oneOf(["ends", "all", "none"] as const, "ends", "Which points are labelled: each path's first and last, all, or none."),
    every: t.number(0, "Also label every n-th point (with `labels: \"ends\"`)."),
    curve: t.oneOf(["linear", "catmull-rom"] as const, "linear", "How the points are joined: straight (honest about where the data is), or smoothed through every point."),
    r: t.number(3.5, "Dot radius (px)."),
    arrow: t.bool(true, "An arrowhead at each path's end: which way it went."),
    stroke: t.prop("Line and dot ink (default: the colour scale, else $mark)."),
    format: t.string(",.3~g", "Number format in tooltips."),
  },
  tokens: ["mark", "ink-2", "paper", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const ord = p.order;
    const tbl = cx.table("connected", p.data, ...(ord ? [op.sort(ord)] : []));
    const X = at("x", p.x, p.xType), Y = at("y", p.y, p.yType);
    const ink = p.stroke ?? colorOr(p.color, "$mark");
    const lbl = p.text ?? ord;
    const f = (v: string) => `format(${v}, ${JSON.stringify(p.format)})`;
    const pointLabel = e(`${series ? `key.name(d.${series}) + " · " + ` : ""}${lbl ? `d.${lbl} + ": " + ` : ""}${f(`d.${p.x}`)} + ", " + ${f(`d.${p.y}`)}`);
    // Which points get a label: numbered in order within each path, the first and the last (and
    // every n-th).
    const part = series ? [series] : [];
    const numbered = lbl && p.labels !== "none" ? cx.table("connected-labels", tbl,
      op.derive("__one", 1), op.window("cumsum", "__one", "__i", { partition: part }), op.window("last", "__i", "__n", { partition: part }),
      op.filter(e(p.labels === "all" ? "true" : `d.__i == 1 || d.__i == d.__n${p.every > 0 ? ` || (d.__i - 1) % ${p.every} == 0` : ""}`))) : null;
    const path = group({ children: [
      shape(geom.polyline({ from: "@group", x: e(X), y: e(Y), curve: p.curve }), {
        key: "line", stroke: { paint: ink, width: 1.5, join: "round", cap: "round" },
        markers: p.arrow ? { end: { type: "arrow", size: 7 } } : undefined,
        semantics: { role: "series", label: series ? e(`key.name(d.${series})`) : `${p.y} against ${p.x}` },
      }),
      instances({ key: "points", from: "@group", x: e(X), y: e(Y), r: p.r, fill: ink, stroke: { paint: "$paper", width: 1 }, instanceKey: ord ? e(`d.${ord}`) : undefined, label: pointLabel }),
    ] });
    return group({
      key: "connected",
      semantics: { role: "series", label: `${p.y} against ${p.x}${ord ? `, by ${ord}` : ""}` },
      children: [
        series ? repeat({ groups: tbl, by: series }, path) : repeat({ groups: tbl, by: "__all" }, path),
        // Beside its point; a label that would land on another tries the other side, else stays out.
        numbered ? group({ key: "labels", declutter: true, children: [repeat(numbered, text(e(`d.${lbl}`), [e(X), e(Y)], {
          offset: [p.r + 4, -(p.r + 3)],
          halo: ["$paper", 2],
          style: { size: "$size.label", ink: "$ink-2", align: "start", baseline: "alphabetic" },
        }))] }) : null,
      ],
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }],
});

// ---- what these marks need from a plot ----------------------------------------------------------

/** What an enclosing plot hands its marks (and reads here before laying out). */
export interface ComparisonInherit { data?: string; x?: string; y?: string; color?: string; xType?: string; yType?: string }

/** What the marks in this file need from the plot around them: a value (or x) domain that spans
 * their bins, stacks and intervals, no forced zero where a baseline means nothing (distributions,
 * before-and-after), percentages for a 100 % stack, and room at either side for labels. */
export interface ComparisonFrame { x?: unknown; y?: unknown; zero?: boolean; percent?: boolean; start?: Room; end?: Room }

const NO_ZERO = new Set(["boxplot", "violin", "ridgeline", "dumbbell", "slope", "connectedScatter"]);

export function comparisonFrame(children: Template[], inherit: ComparisonInherit, cx: Cx): ComparisonFrame {
  const out: ComparisonFrame = {};
  for (const c of children) {
    if (c.kind !== "use") continue;
    const n = nameOf(c.recipe);
    const raw = { ...inherit, ...((c.params ?? {}) as Record<string, unknown>) };
    if (n && NO_ZERO.has(n)) out.zero ??= false;
    if (n === "histogram") {
      const q = withDefaults(histogram.def, raw);
      const tbl = histogramTable(cx, q);
      out.x ??= { data: tbl, fields: ["bin", "bin_end"] };
      out.y ??= { data: tbl, fields: ["y0", "y1"] };
    } else if (n === "stackedArea") {
      const q = withDefaults(stackedArea.def, raw);
      out.y ??= q.offset === "expand" ? [0, 1] : { data: stackedAreaTable(cx, q), fields: ["y0", "y1"] };
      if (q.offset === "expand") out.percent = true;
    } else if (n === "errorBars") {
      const q = withDefaults(errorBars.def, raw);
      const horizontal = isBand(q.yType) && !isBand(q.xType);
      const domain = { data: errorTable(cx, q), fields: ["__lo", "__hi", horizontal ? q.x : q.y] };
      if (horizontal) out.x ??= domain;
      else out.y ??= domain;
    } else if (n === "ridgeline") {
      // The curves' tails run past the data: the value axis reaches them.
      out.x ??= { data: ridgelineTable(cx, withDefaults(ridgeline.def, raw)).kde, field: "value" };
    } else if (n === "slope") {
      const room = slopeRoom(cx, withDefaults(slope.def, raw));
      out.start ??= room.start;
      out.end ??= room.end;
    }
  }
  return out;
}
