// axis: ticks, labels and a domain line for a scale. Left/bottom/right/top. Its size is measured by
// the engine, so the plot area gets exactly the space the labels leave.

import { e, group, op, recipe, repeat, shape, geom, t, text } from "@datars/sdk";

export interface AxisParams { scale: string; orient: "left" | "bottom" | "right" | "top"; ticks: number; format: string; label: string; line: boolean; prefix: string; suffix: string; data: string; field: string; type: string }

export const axis = recipe<AxisParams>({
  id: "@datars/std/axis",
  doc: "An axis for a scale: tick labels, tick marks and a domain line. Labels never pile up: a horizontal numeric axis asks for as many ticks as its measured labels fit; category names wrap in their band, turn 45° when a word is wider than the band, and leave out every k-th name only when even that doesn't fit.",
  params: {
    scale: t.string("y", "The scale to draw."),
    orient: t.oneOf(["left", "bottom", "right", "top"] as const, "left"),
    ticks: t.number(0, "Tick count hint (0 = automatic: from the axis length, and across, from how many of its measured labels fit)."),
    format: t.string(undefined, "Number format for labels (d3-format)."),
    label: t.string(undefined, "Axis title: under the end of a horizontal axis; above the top of a vertical one, over its labels (a plot draws `yLabel` above its top-left corner instead, in a row of its own)."),
    line: t.bool(true, "Draw the domain line."),
    prefix: t.string(undefined, "Before each number label ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
    data: t.table("The table a band scale's categories come from (with `field`): their names are measured, so a horizontal axis wraps, turns or leaves out labels only as they need."),
    field: t.field("The categories' field in `data`."),
    type: t.string(undefined, "The scale's type (`linear`, `log`, `time`, …; the plot passes it). A horizontal numeric axis fits its tick count to its labels' measured width."),
  },
  tokens: ["muted", "rule", "size.label"],
  expand(p, cx) {
    const s = p.scale;
    const S = `scale.${s}`;
    const center = `d.pos + ${S}.bandwidth() / 2`;
    const affix = (n: string) => (p.prefix || p.suffix ? `${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}` : n);
    const num = p.format ? `format(d.value, ${JSON.stringify(p.format)})` : "d.label";
    const plain = affix(num);
    // Categories (band and point scales) read as their display names (key metadata), like the
    // marks' own labels, wrapped to their band on a horizontal axis. Dates on a band scale
    // (trading days) come as calendar ticks the engine already spaced: they label like time.
    const band = `d.kind == "category"`;
    const label = p.format ? plain : `${band} ? key.name(d.label) : d.label`;
    const vertical = p.orient === "left" || p.orient === "right";
    const sign = p.orient === "left" || p.orient === "top" ? -1 : 1;
    const size = `token("size.label")`;
    const len = `(${S}.max() - ${S}.min())`;
    // A horizontal numeric axis: as many ticks as labels fit, each as wide as the widest the axis
    // writes (its domain's ends, formatted as the ticks are) plus air — five across a desktop
    // chart's 600 px of years, three across a phone's 250 px of "25,000". Should the scale still
    // make more (nice steps), labels leave out every k-th tick so none touch.
    const numeric = fitsTicks(p);
    const widest = widestTick(p);
    const from = { ticks: s, count: tickCount(p) };
    const spaced = numeric ? `d.index % max(1, ceil((${widest} + 24) * max(1, d.count - 1) / max(${len}, 1))) == 0` : "true";
    // A whole-number format ('d', '.0f') would write a tick between whole numbers as its
    // neighbour (2021.5 → "2022"): on a continuous scale only whole-number ticks are drawn, so
    // decimal years on a linear axis read as years, each once.
    const whole = p.format && wholeNumbers(p.format) ? e(`${band} || abs(d.value - round(d.value)) < 0.000001`) : undefined;
    const tickMark = vertical
      ? shape(geom.segment({ x1: 0, y1: e(center), x2: sign * 5, y2: e(center) }), { stroke: { paint: "$rule", width: 1 } })
      : shape(geom.segment({ x1: e(center), y1: 0, x2: e(center), y2: sign * 5 }), { stroke: { paint: "$rule", width: 1 } });
    // Band and point scales label every category. Up and down a label needs one line (packed
    // tighter — 15 bars on a phone — every k-th one). Across, the names are measured: each wraps
    // in its band when its longest word fits there; when a word doesn't (party names on a phone)
    // the labels turn 45° and need only a line's height of room each; and only when even that
    // doesn't fit (70 years along a heatmap) every k-th one shows, flat or turned, whichever shows
    // more — flat when it's close, since flat reads better. Names unknown (a facet's panel): every
    // k-th, wrapped at 36 px.
    const names = !vertical && p.data && p.field
      ? cx.table("names", p.data, op.derive("w", e(`measure(key.name(d.${p.field}), ${size})`)), op.derive("ww", e(`measure.word(key.name(d.${p.field}), ${size})`)))
      : null;
    const step = `${S}.step()`;
    const W = names ? `table.max(${JSON.stringify(names)}, "w")` : "36";
    const WW = names ? `table.max(${JSON.stringify(names)}, "ww")` : "36";
    const kFlat = names ? `max(1, ceil((${WW} + 4) / ${step}))` : `max(1, ceil(40 / ${step}))`;
    const kTurned = `max(1, ceil((${size} + 3) * 1.42 / ${step}))`;
    // (Short names — years, codes — read better flat and thinned than turned.)
    const turned = names && sign > 0 ? `(${WW} > 3.5 * ${size} && ${kFlat} > 1 && ${kTurned} * 1.5 < ${kFlat})` : "false";
    const k = vertical ? `max(1, ceil((${size} + 1) / ${step}))` : `(${turned} ? ${kTurned} : ${kFlat})`;
    const thin = `d.index % ${k} == 0`;
    const flatWidth = names ? `${kFlat} * ${step} - 4` : `max(${step} - 4, 36)`;
    // Continuous scales: the tick value as it always was; categories: the variant above. A log
    // axis over a decade or two ticks every 1…9 × 10ⁿ: it labels the 1s, 2s and 5s.
    const lead = "round(abs(d.value) / pow(10, floor(log10(abs(d.value)))))";
    const shown = `!(${band}) && (d.kind != "log" || d.major || ${lead} == 2 || ${lead} == 5)`;
    const tickLabel = vertical
      ? text(e(plain), [sign * 8, e(center)], { when: e(shown), style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "middle" } })
      : text(e(plain), [e(center), sign < 0 ? -8 : 8], { when: e(`${shown} && ${spaced}`), style: { size: "$size.label", ink: "$muted", align: "middle", contain: true, baseline: sign < 0 ? "bottom" : "top" } });
    const catLabel = vertical
      ? text(e(label), [sign * 8, e(center)], { when: e(`${band} && ${thin}`), style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "middle" } })
      : text(e(label), [e(center), sign < 0 ? -8 : 8], { when: e(`${band} && ${thin} && !${turned}`), style: { size: "$size.label", ink: "$muted", align: "middle", contain: true, baseline: sign < 0 ? "bottom" : "top", maxWidth: e(flatWidth) } });
    // Turned: the name ends at its tick and runs down to the left, on one line.
    const turnedLabel = !vertical && names && sign > 0
      ? text(e(label), [e(center), 8], { key: "turned", when: e(`${band} && ${thin} && ${turned}`), rotate: -45, style: { size: "$size.label", ink: "$muted", align: "end", contain: true, baseline: "middle" } })
      : null;
    const domainLine = vertical
      ? shape(geom.segment({ x1: 0, y1: e(`${S}.min()`), x2: 0, y2: e(`${S}.max()`) }), { key: "domain", stroke: { paint: "$rule", width: 1 } })
      : shape(geom.segment({ x1: e(`${S}.min()`), y1: 0, x2: e(`${S}.max()`), y2: 0 }), { key: "domain", stroke: { paint: "$rule", width: 1 } });
    // Below turned names, the axis title goes under their slant. Up the side, it stands above the
    // axis's top end, over the labels (clear of the top one, so it isn't decluttered away).
    const titleY = turnedLabel ? e(`${turned} ? 12 + (${W} + ${size}) * 0.71 : 30`) : 30;
    const top = `min(${S}.min(), ${S}.max()) - 8`;
    return group({
      key: `axis-${s}`,
      // Whatever the rules above miss — a last label nudged in from the edge onto its neighbour, a
      // first turned name pushed right — the later label gives way.
      declutter: true,
      semantics: { role: "axis", label: p.label ?? `${s} axis` },
      children: [
        p.line ? domainLine : null,
        repeat(from, group({ when: whole, semantics: { role: "tick", label: e(label) }, children: [tickMark, tickLabel, catLabel, turnedLabel] })),
        p.label
          ? vertical
            ? text(p.label, [sign * 8, e(top)], { key: "label", style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "bottom" } })
            : text(p.label, [e(`${S}.max()`), titleY], { key: "label", style: { size: "$size.label", ink: "$muted", align: "end", baseline: "top" } })
          : null,
      ],
    });
  },
});

type TickSpec = { scale: string; orient?: string; ticks?: number; type?: string; format?: string; prefix?: string; suffix?: string };

/** Does this axis fit its tick count to its labels (horizontal, numeric, no count asked for)? */
function fitsTicks(p: TickSpec): boolean {
  return (p.orient === "bottom" || p.orient === "top") && !!p.type && !["band", "point", "time"].includes(p.type);
}

/** The widest label a numeric axis writes: its domain's ends, formatted as its ticks are. */
function widestTick(p: TickSpec): string {
  const S = `scale.${p.scale}`, size = `token("size.label")`;
  const written = (v: string) => {
    const n = p.format ? `format(${v}, ${JSON.stringify(p.format)})` : `${S}.label(${v})`;
    return p.prefix || p.suffix ? `${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}` : n;
  };
  return `max(measure(${written(`${S}.invert(${S}.min())`)}, ${size}), measure(${written(`${S}.invert(${S}.max())`)}, ${size}))`;
}

/** An axis's tick count: the one asked for; across, for numbers, as many as labels fit (at most
 * one per 64 px, as the scale would pick); else the scale's own. Gridlines take the same count. */
export function tickCount(p: TickSpec): number | { expr: string } | undefined {
  if (p.ticks) return p.ticks;
  if (!fitsTicks(p)) return undefined;
  const len = `(scale.${p.scale}.max() - scale.${p.scale}.min())`;
  return e(`max(2, min(10, round(${len} / 64), floor(${len} / (${widestTick(p)} + 30))))`);
}

/** Does a d3-format spec always write whole numbers ('d', ',d', '.0f', ',.0~f')? */
function wholeNumbers(spec: string): boolean {
  const m = /(?:\.(\d+))?~?([a-z%])?$/i.exec(spec);
  return !!m && (m[2] === "d" || (m[2] === "f" && m[1] === "0"));
}
