// plot: a cartesian frame — scales, layout (title, axes measured by the engine, plot area), and
// marks as children. Marks inherit `data`, `x`, `y`, `color` unless they set their own.

import { brush, e, geom, group, op, recipe, shape, t, text, Template, ScaleDecl } from "@datars/sdk";
import { axis, tickCount } from "./axis.js";
import { grid } from "./grid.js";
import { legend } from "./guides.js";
import { financeDomain, financeKey } from "./finance.js";
import { comparisonFrame } from "./comparisons.js";

type ScaleType = "band" | "point" | "linear" | "log" | "sqrt" | "time";

export interface PlotParams {
  data: string;
  x: string;
  y: string;
  color: string;
  xType: ScaleType;
  yType: ScaleType;
  colorType: "categorical" | "sequential" | "diverging" | "piecewise";
  stops: string;
  xDomain: unknown;
  yDomain: unknown;
  zero: boolean;
  nice: boolean;
  padding: number;
  title: string;
  subtitle: string;
  axes: "both" | "x" | "y" | "none";
  grid: boolean;
  legend: boolean;
  xLabel: string;
  yLabel: string;
  format: string;
  xFormat: string;
  yFormat: string;
  xTicks: number;
  yTicks: number;
  clip: boolean;
  labelSpace: number;
  brush: string;
  prefix: string;
  suffix: string;
  right: unknown;
  lower: unknown;
  children: Template[];
}

/** A second value axis on the right (scale `y2`): a line of another measure over bars, a
 * Pareto's cumulative share. Marks draw on it with `yScale: "y2"`. */
export interface RightAxis { y?: string; data?: string; domain?: unknown; label?: string; format?: string; prefix?: string; suffix?: string; zero?: boolean }

/** A pane under the plot area (scale `lower`): its own value axis on the same x — volume under
 * prices. `height`: a fraction of the plot (≤ 1) or px. Marks go there with `yScale: "lower"`. */
export interface LowerPane { y?: string; data?: string; height?: number; format?: string; label?: string; domain?: unknown }

/** A recipe's name, whatever its package (an ejected copy is still what it was). */
const nameOf = (id: unknown) => String(id ?? "").split("/").pop();
/** Space between the plot area and a lower pane (plus the rows' gaps): room for both axes' end labels. */
const PANE_GAP = 10;
const isLower = (c: Template) => c.kind === "use" && ((c.params as { yScale?: string } | undefined)?.yScale ?? (nameOf(c.recipe) === "volume" ? "lower" : "y")) === "lower";
/** Does a child label its lines at their ends (just past the plot area, which then leaves room)? */
function labelsEnds(c: Template, color: string | undefined): boolean {
  const q = (c.params ?? {}) as Record<string, unknown>;
  if (c.kind !== "use") return false;
  if (nameOf(c.recipe) === "line") return !!q.labels;
  if (nameOf(c.recipe) === "indexed") return q.labels !== false;
  if (nameOf(c.recipe) === "drawdown") return q.labels !== false && !!(q.series ?? color);
  return false;
}

/** A `when` (an expression, `=source`, or a literal) as expression source. */
function whenSource(w: unknown): string {
  if (w && typeof w === "object" && "expr" in w) return String((w as Record<string, unknown>).expr);
  if (typeof w === "string") return w.startsWith("=") ? w.slice(1) : JSON.stringify(w);
  return JSON.stringify(w ?? true);
}

// `zero` applies to the value axis only (bars and areas need their baseline); the other linear
// axis (years, x of a scatter) fits the data.
function scaleDecl(type: ScaleType, data: string, field: string, axisName: "x" | "-y" | "y", p: PlotParams, domain: unknown, valueAxis: boolean): ScaleDecl {
  const categorical = type === "band" || type === "point";
  return {
    type,
    domain: (domain as never) ?? { data, field },
    range: { box: "plot-area", axis: axisName },
    padding: categorical ? p.padding : undefined,
    zero: valueAxis && !categorical && type !== "log" && type !== "time" ? p.zero : undefined,
    nice: !categorical ? p.nice : undefined,
  };
}

/** The brushed range, shaded under the marks (continuous scales; hidden while nothing is brushed). */
function brushExtent(signal: string, axis: "x" | "y"): Template {
  const [lo, hi] = [`scale.${axis}(${signal}.lo)`, `scale.${axis}(${signal}.hi)`];
  const g = axis === "x"
    ? geom.rect({ x: e(`min(${lo}, ${hi})`), y: 0, w: e(`abs(${hi} - ${lo})`), h: e("box.h") })
    : geom.rect({ x: 0, y: e(`min(${lo}, ${hi})`), w: e("box.w"), h: e(`abs(${hi} - ${lo})`) });
  return shape(g, { key: "brush", when: e(`${signal}.active && ${signal}.hi > ${signal}.lo`), fill: "$accent", opacity: 0.14, semantics: { role: "decoration", label: "" } });
}

export const plot = recipe<PlotParams>({
  id: "@datars/std/plot",
  doc: "A cartesian frame: x/y (and colour) scales from data, axes and gridlines sized by the engine, a title, and marks as children.",
  params: {
    data: t.table("The table marks read by default."),
    x: t.field("Field on the horizontal axis."),
    y: t.field("Field on the vertical axis (none: marks that place themselves across, like a one-dimensional `swarm`, with no y axis)."),
    color: t.field("Field mapped to colour (categorical by default)."),
    xType: t.oneOf(["band", "point", "linear", "log", "sqrt", "time"] as const, "band"),
    yType: t.oneOf(["band", "point", "linear", "log", "sqrt", "time"] as const, "linear"),
    colorType: t.oneOf(["categorical", "sequential", "diverging", "piecewise"] as const, "categorical"),
    stops: t.string(undefined, "Piecewise colour stops (colorType piecewise): '#bae6fd 7 · #0b1f3a 14.2'."),
    xDomain: t.json("Explicit x domain."),
    yDomain: t.json("Explicit y domain (default: the data's; a `stacked` child's totals, 0–1 stretched to 100 %; a `waterfall` child's running totals)."),
    zero: t.bool(undefined, "Include zero in the value axis's domain (default: yes; prices under candles, averages, bands and indexed lines fit their range instead)."),
    nice: t.bool(true, "Round linear domains to nice numbers."),
    padding: t.number(0.22, "Band padding."),
    title: t.string(undefined, "Title above the plot."),
    subtitle: t.string(undefined),
    axes: t.oneOf(["both", "x", "y", "none"] as const, "both"),
    grid: t.bool(true, "Gridlines along the value axis."),
    legend: t.bool(false, "A colour legend below the plot."),
    xLabel: t.string(undefined, "Title of the x axis, under its right end."),
    yLabel: t.string(undefined, "Title of the y axis, above its top-left corner (in a row of its own, between the title and the plot)."),
    format: t.string(undefined, "Number format for value-axis labels (a 100 % `stacked` child: '.0%')."),
    xFormat: t.string(undefined, "Number format for the x axis's labels, whichever axis holds the values: 'd' reads decimal years on a linear axis as whole years (a whole-number format labels whole-number ticks only)."),
    yFormat: t.string(undefined, "Number format for the y axis's labels (overrides `format` there)."),
    xTicks: t.number(0, "Tick count hint for the x axis (0 = from its length)."),
    yTicks: t.number(0, "Tick count hint for the y axis (0 = from its length)."),
    clip: t.bool(false, "Clip marks to the plot area (for explicit domains narrower than the data: a zoom)."),
    labelSpace: t.number(110, "Room at the right (px) when a line child labels its ends."),
    prefix: t.string(undefined, "Before each value-axis number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
    brush: t.string(undefined, "A signal to brush into: dragging across the plot selects a range of the category/time axis (see `brush()` and `brushed()`)."),
    right: t.json("A second value axis on the right, scale `y2`: {y, data?, domain?, label?, format?, prefix?, suffix?}. Marks use it with `yScale: \"y2\"`."),
    lower: t.json("A pane under the plot area on the same x with its own value axis, scale `lower`: {y, data?, height? (fraction or px, 0.22), format?, label?}. Marks go there with `yScale: \"lower\"`; a `volume` child makes one when it isn't given."),
    children: t.children("Marks and annotations."),
  },
  tokens: ["ink", "muted", "grid", "rule", "size.title", "size.label"],
  expand(p, cx) {
    const yCategorical = p.yType === "band" || p.yType === "point";
    const children = p.children ?? [];
    // Finance marks fit the value axis to what they draw — candles' lows and highs, band edges,
    // an index instead of prices — and prices leave out zero.
    const fin = yCategorical ? undefined : financeDomain(children, { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType }, cx);
    // Distributions, stacks and before-and-after marks: what they need of the axes (comparisons.ts).
    const cmp = comparisonFrame(children, { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType, yType: p.yType }, cx);
    const pz = { ...p, zero: p.zero ?? fin?.zero ?? cmp.zero ?? true };
    const stackedChild = children.find((c) => c.kind === "use" && nameOf(c.recipe) === "stacked");
    const waterfallChild = children.find((c) => c.kind === "use" && nameOf(c.recipe) === "waterfall");
    // No y at all (a one-dimensional swarm along x): a unit y scale for marks that place
    // themselves, and neither a y axis nor gridlines across it.
    const noY = !p.y && !yCategorical && p.yDomain === undefined && !fin && !stackedChild && !waterfallChild && !cmp.y;
    const scales: Record<string, ScaleDecl> = {
      x: scaleDecl(p.xType, p.data, p.x, "x", pz, p.xDomain ?? cmp.x, yCategorical),
      // Prices on a log axis fit their range instead of widening to whole decades.
      y: scaleDecl(p.yType, p.data, p.y, yCategorical ? "y" : "-y", fin && p.yType === "log" ? { ...pz, nice: false } : pz, p.yDomain ?? fin?.domain ?? cmp.y ?? (noY ? [0, 1] : undefined), !yCategorical),
    };
    const valueName = yCategorical ? "x" : "y";
    const valueGiven = yCategorical ? p.xDomain : p.yDomain;
    // Stacked bars reach the stack totals, not the largest single value: the value axis spans
    // the stacked table (the same op the child runs, so the same table). Stretched to 100 %
    // (`offset: "expand"`), it spans 0–1 (and reads as percentages unless a format is given).
    const sp = stackedChild?.params as { offset?: string; series?: string; color?: string; data?: string; x?: string; y?: string } | undefined;
    const expand = sp?.offset === "expand";
    if (sp && !valueGiven) {
      const series = sp.series ?? sp.color ?? p.color;
      const [x, y] = [sp.x ?? p.x, sp.y ?? p.y];
      const cat = yCategorical ? y : x;
      const val = yCategorical ? x : y;
      const domain = expand ? [0, 1] : { data: cx.table("stack", sp.data ?? p.data, op.stack({ x: cat, series, value: val, offset: "zero", as: ["y0", "y1"] })), field: "y1" };
      scales[valueName] = { ...scales[valueName], domain: domain as never };
    }
    // A waterfall's bars reach its running totals (start and end of every bar), not the changes.
    const wp = waterfallChild?.params as { data?: string; y?: string; total?: string } | undefined;
    if (wp && !valueGiven) {
      const tbl = cx.table("waterfall", wp.data ?? p.data, op.waterfall({ value: wp.y ?? p.y, total: wp.total }));
      scales[valueName] = { ...scales[valueName], domain: { data: tbl, fields: ["start", "end"] } };
    }
    const percent = (expand || cmp.percent) && !p.format ? ".0%" : undefined;
    const right = p.right as RightAxis | undefined;
    if (right) {
      scales.y2 = {
        type: "linear",
        domain: (right.domain as never) ?? { data: right.data ?? p.data, field: right.y ?? p.y },
        range: { box: "plot-area", axis: "-y" },
        zero: right.zero ?? true,
        nice: true,
      };
    }
    if (p.color) {
      scales.color = p.colorType === "piecewise"
        ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.color } }
        : { type: p.colorType, domain: { data: p.data, field: p.color }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" };
    }
    // A lower pane (volume under prices): its own value scale over a box under the plot area.
    const volumeChild = children.find((c) => nameOf(c.recipe) === "volume" && isLower(c));
    const vp = volumeChild?.params as { volume?: string; data?: string } | undefined;
    const lower = yCategorical ? undefined : ((p.lower as LowerPane | undefined) ?? (volumeChild ? { y: vp?.volume ?? "volume", data: vp?.data } : undefined));
    if (lower) {
      scales.lower = { type: "linear", domain: (lower.domain as never) ?? { data: lower.data ?? p.data, field: lower.y ?? p.y }, range: { box: "lower-area", axis: "-y" }, zero: true, nice: true };
    }
    const endLabels = children.some((c) => labelsEnds(c, p.color)) || cmp.end !== undefined;
    // Clipping the whole plot area would cut off labels drawn past its edge: with labelled lines
    // the marks clip themselves instead.
    const clipArea = p.clip && !endLabels;
    const inherit = { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType, yType: p.yType, clip: p.clip && endLabels ? true : undefined };
    const marks = children.map((c) => (c.kind === "use" ? { ...c, params: { ...inherit, ...(c.params as object) } } : c));
    const valueAxis = yCategorical ? "x" : "y";
    // A band x axis measures its categories' names, to leave labels out only when they'd collide:
    // from the explicit domain's table when it names one (a facet's shared domain), else the plot's.
    const xd = p.xDomain && typeof p.xDomain === "object" && !Array.isArray(p.xDomain) ? (p.xDomain as { data?: string; field?: string }) : undefined;
    const names = { data: xd?.data ?? p.data, field: xd?.field ?? p.x };
    const xNames = (p.xType === "band" || p.xType === "point") && names.data && names.data !== "@group" ? names : {};
    const showX = p.axes === "both" || p.axes === "x";
    const showY = (p.axes === "both" || p.axes === "y") && !noY;
    const yAxis = (opts?: Record<string, unknown>) => axis({ scale: "y", orient: "left", type: p.yType, format: p.yFormat ?? (valueAxis === "y" ? p.format ?? percent : undefined), ticks: p.yTicks || undefined, prefix: valueAxis === "y" ? p.prefix : undefined, suffix: valueAxis === "y" ? p.suffix : undefined }, opts);
    // The lower pane's axis sits under the price axis, in one column as wide as the wider of
    // the two, so both panes start at the same x.
    const leftAxes = !showY ? null : !lower ? yAxis({ size: { w: "auto" } }) : group({ key: "axes-left", size: { w: "auto" }, children: [
      yAxis(),
      group({ key: "lower-axis", transform: { translate: [0, e(`scale.y.max() + ${PANE_GAP + 12}`)] }, children: [axis({ scale: "lower", orient: "left", ticks: 2, format: lower.format ?? ".2~s" })] }),
    ] });
    const lowerPane = lower ? group({
      id: "lower-area",
      key: "lower",
      size: { h: (lower.height ?? 0.22) <= 1 ? `${Math.round((lower.height ?? 0.22) * 100)}%` : (lower.height as number) },
      clip: clipArea ? "box" : undefined,
      children: [
        grid({ scale: "lower", orient: "horizontal", ticks: 2 }),
        lower.label ? text(lower.label, [4, 2], { key: "pane-label", style: { size: "$size.small", ink: "$muted", baseline: "top" }, halo: ["$paper", 2] }) : null,
        ...marks.filter(isLower),
      ],
    }) : null;
    // The indicators on the chart (moving averages, bands), named in a key above it.
    const keys = financeKey(children);
    const keyRow = keys.length ? group({ key: "indicators", size: { h: "auto" }, layout: { type: "flow", gap: 14 }, when: keys.every((k) => k.when !== undefined) ? e(keys.map((k) => `(${whenSource(k.when)})`).join(" || ")) : undefined, children: keys }) : null;
    const xAxis = { scale: "x", orient: "bottom" as const, type: p.xType, format: p.xFormat ?? (valueAxis === "x" ? p.format ?? percent : undefined), ticks: p.xTicks || undefined, prefix: valueAxis === "x" ? p.prefix : undefined, suffix: valueAxis === "x" ? p.suffix : undefined };
    // Gridlines at the value axis's ticks: across, as many as its labels fit (the axis's count).
    const gridCount = valueAxis === "x" ? tickCount(xAxis) : p.yTicks || undefined;
    // Room at the right for lines labelled at their ends: as wide as the widest series name needs,
    // up to `labelSpace` (on a phone the plot keeps the rest).
    const labelled = endLabels ? (p.children ?? []).find((c) => c.kind === "use" && c.recipe === "@datars/std/line" && (c.params as { labels?: boolean }).labels) : undefined;
    const lp = (labelled?.params ?? {}) as { data?: string; series?: string; color?: string };
    const series = lp.series ?? lp.color ?? p.color;
    const seriesNames = labelled && series ? cx.table("series-names", lp.data ?? p.data, op.aggregate([series], { n: ["count"] }), op.derive("w", e(`measure(key.name(d.${series}), token("size.label"))`))) : undefined;
    const labelRoom = cmp.end ?? (seriesNames ? e(`min(${p.labelSpace}, table.max(${JSON.stringify(seriesNames)}, "w") + 12)`) : p.labelSpace);
    // The vertical axes' titles, in a row of their own above the plot area: the left one starts
    // where the y axis's labels do, the right one ends where the right axis's do. (Up the side, a
    // title would need the room its labels leave; above, it can never run into the title.)
    const yTitle = showY && p.yLabel ? p.yLabel : undefined;
    const axisTitles = yTitle || right?.label ? group({ key: "axis-titles", size: { h: "auto" }, children: [
      yTitle ? text(yTitle, [0, 0], { key: "y", style: { size: "$size.label", ink: "$muted", baseline: "top" } }) : null,
      right?.label ? text(right.label, [e("box.w"), 0], { key: "y2", style: { size: "$size.label", ink: "$muted", baseline: "top", align: "end" } }) : null,
    ] }) : null;
    return group({
      key: "plot",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? "" },
      children: [
        p.title ? text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }) : null,
        keyRow,
        axisTitles,
        group({
          key: "body",
          layout: { type: "columns", gap: 6 },
          children: [
            leftAxes,
            // Room at the left for marks labelled at their starts (a slope chart's first values).
            cmp.start !== undefined ? group({ key: "start-labels", size: { w: cmp.start } }) : null,
            group({
              key: "center",
              layout: { type: "rows", gap: 6 },
              children: [
                group({
                  id: "plot-area",
                  key: "area",
                  clip: clipArea ? "box" : undefined,
                  on: p.brush ? { brush: brush(p.brush, valueAxis === "y" ? "x" : "y") } : undefined,
                  children: [p.grid && !noY ? grid({ scale: valueAxis, orient: valueAxis === "y" ? "horizontal" : "vertical", count: gridCount }) : null, p.brush ? brushExtent(p.brush, valueAxis === "y" ? "x" : "y") : null, ...(lower ? marks.filter((c) => !isLower(c)) : marks)],
                }),
                lower ? group({ key: "pane-gap", size: { h: PANE_GAP } }) : null,
                lowerPane,
                showX ? axis({ ...xAxis, label: p.xLabel, ...xNames }, { size: { h: "auto" } }) : null,
              ],
            }),
            right ? axis({ scale: "y2", orient: "right", format: right.format, prefix: right.prefix, suffix: right.suffix }, { key: "axis-y2", size: { w: "auto" } }) : null,
            // Room at the right for lines labelled at their ends (they draw just past the plot area).
            endLabels ? group({ key: "end-labels", size: { w: labelRoom } }) : null,
          ],
        }),
        p.legend && p.color ? legend({ scale: "color" }, { size: { h: "auto" } }) : null,
      ],
    });
    void cx;
  },
});
