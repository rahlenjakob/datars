// Big data: millions of rows drawn as what they add up to — a density map of hexagons or grid
// cells, the contours of a crowd, thousands of series at once. The engine bins the rows in one
// pass (table ops in Rust that read only the rows, so they're computed once per data and kept
// across states, layout passes and hovers); a mark is drawn per bin, not per row, and a frame
// costs the same for a hundred thousand rows as for ten million. Each recipe is a mark inside a
// `plot` (it reads the plot's x and y scales) and picks its lattice from the plot area it's drawn
// in, so cells stay a few px wide on a phone and on a wide screen.

import { e, geom, group, instances, op, recipe, repeat, shape, t, text, Prop, Template } from "@datars/sdk";

type Agg = "count" | "sum" | "mean" | "min" | "max";
type Ramp = "linear" | "sqrt" | "log";
type KeyAt = "top-right" | "top-left" | "bottom-right" | "bottom-left" | "none";

const q = JSON.stringify;

/** How a bin's value maps onto the colour ramp: `sqrt` keeps sparse bins visible beside dense
 * ones, `log` spreads counts that span orders of magnitude (both keep the sign). */
function shade(ramp: Ramp, v: string): string {
  if (ramp === "sqrt") return `sign(${v}) * sqrt(abs(${v}))`;
  if (ramp === "log") return `sign(${v}) * log10(1 + abs(${v}))`;
  return v;
}

/** The lattice's aspect (plot-area height ÷ width), rounded so a layout pass a pixel off reuses
 * the same binned table instead of binning the rows again. */
const aspectOf = (size: [number, number]) => Math.max(0.05, Math.round((size[1] / Math.max(1, size[0])) * 50) / 50);

/** The key's title: "Trips per hexagon", or what an aggregate is ("Mean fare"). */
const keyTitle = (value: string | undefined, fn: Agg, unit: string, per: string) =>
  (!value || fn === "count" ? `${unit} per ${per}` : `${fn} ${value}`).replace(/^./, (c) => c.toUpperCase());
/** A tooltip: the aggregate (if any), then how many rows, then where. */
const binTip = (value: string | undefined, fn: Agg, unit: string, format: string, where: string) =>
  e(`${fn === "count" || !value ? "" : `format(d.value, ${q(format)}) + ${q(` ${fn} ${value} · `)} + `}format(d.count, ",") + ${q(` ${unit} ${where} `)} + ${xLabel} + ", " + ${yLabel}`);

/** A small colour key for a binned table's `shade` ramp: its lowest and highest `value` at the
 * ends, in a corner of the plot area. (A fixed corner: a density map covers most of the area, so
 * there is rarely a free one to dodge to.) */
function rampKey(scale: string, tbl: string, title: string, format: string, at: KeyAt): Template | null {
  if (at === "none") return null;
  const [lo, hi] = [`table.min(${q(tbl)}, "shade")`, `table.max(${q(tbl)}, "shade")`];
  const N = 6, W = 14, H = 8;
  const small = { size: "$size.small", ink: "$ink-2", baseline: "top" as const };
  const box = group({
    key: "box",
    size: { w: N * W, h: "auto" },
    layout: { type: "rows", gap: 3 },
    backdrop: { fill: "$paper", radius: 3, padding: 5 },
    semantics: { role: "legend", label: title },
    children: [
      text(title, [0, 0], { key: "title", size: { h: "auto" }, style: small }),
      group({ key: "ramp", size: { h: H }, children: [
        repeat({ count: N }, shape(geom.rect({ x: e(`d.index * ${W}`), y: 0, w: W, h: H }), { fill: e(`scale.${scale}(${lo} + (${hi} - ${lo}) * d.index / ${N - 1})`) })),
      ] }),
      group({ key: "ends", size: { h: "auto" }, children: [
        text(e(`format(table.min(${q(tbl)}, "value"), ${q(format)})`), [0, 0], { key: "lo", style: { ...small, ink: "$muted" } }),
        text(e(`format(table.max(${q(tbl)}, "value"), ${q(format)})`), [N * W, 0], { key: "hi", style: { ...small, ink: "$muted", align: "end" } }),
      ] }),
    ],
  });
  return group({ key: "key", layout: { type: "stack", padding: 6, align: "start" }, dodge: [at], children: [box] });
}

const xLabel = "scale.x.label(d.x)";
const yLabel = "scale.y.label(d.y)";

// ---- hexbin -------------------------------------------------------------------------------------

export interface HexbinParams {
  data: string; x: string; y: string; radius: number; value: string; fn: Agg; ramp: Ramp; size: boolean;
  fill: Prop; extent: unknown; gap: number; legend: KeyAt; format: string; label: Prop; name: string; unit: string; aspect: number;
}

export const hexbin = recipe<HexbinParams>({
  id: "@datars/std/hexbin",
  doc: "Hexagonal bins: rows of any number counted into hexagons a few px across, each coloured by its count (or an aggregate of a field) — a density map of a million points, drawn as a few thousand hexagons. Put it in a plot.",
  params: {
    data: t.table("The rows (any number: they are binned once per data, in the engine)."),
    x: t.field("Field on the plot's x scale (numbers or dates)."),
    y: t.field("Field on the plot's y scale."),
    radius: t.number(8, "Hexagon radius in px: the lattice is laid over the plot area at this size, so hexagons stay this size on a phone."),
    value: t.field("A field to aggregate per hexagon (default: count the rows)."),
    fn: t.oneOf(["count", "sum", "mean", "min", "max"] as const, undefined, "How `value` is aggregated (default: mean with a `value`, else count)."),
    ramp: t.oneOf(["linear", "sqrt", "log"] as const, undefined, "How the value maps onto the colour ramp (default: sqrt for counts — sparse hexagons stay visible — linear for aggregates)."),
    size: t.bool(false, "Size each hexagon by its value too (area ∝ value), so density reads twice — or alone, with a single `fill`."),
    fill: t.prop("One ink for every hexagon instead of the sequential ramp (a size-only map, with `size`)."),
    extent: t.json("The extent to bin, [x0, y0, x1, y1] in the rows' units (default: the rows' own)."),
    gap: t.number(0, "Space between neighbouring hexagons, px."),
    legend: t.oneOf(["top-right", "top-left", "bottom-right", "bottom-left", "none"] as const, "top-right", "Where the colour key goes: a corner of the plot area, or none."),
    format: t.string(",.3~s", "Number format for the key and the tooltips."),
    label: t.prop("Tooltip per hexagon: an expression over its row (`d.count`, `d.value`, `d.x`, `d.y`); default: how many rows, near where."),
    name: t.string(undefined, "What the rows are, for the accessible description ('500,000 taxi trips')."),
    unit: t.string("rows", "What one row is, in the plural, for the key and the tooltips ('trips': 'Trips per hexagon')."),
    aspect: t.number(0, "The lattice's height ÷ width on screen (0: the plot area's, measured when the recipe expands)."),
  },
  tokens: ["sequential", "paper", "ink-2", "muted", "size.small"],
  expand(p, cx) {
    const r = Math.max(2, p.radius || 8);
    const columns = Math.max(1, Math.round(cx.size[0] / (r * Math.sqrt(3))));
    const fn = (p.fn ?? (p.value ? "mean" : "count")) as Agg;
    const ramp = (p.ramp ?? (p.value && fn !== "count" ? "linear" : "sqrt")) as Ramp;
    // Pure: binned once per data. Then, per resolve, a few thousand hexagons through the scales.
    const bins = cx.table("hexbin", p.data,
      op.hexbin({ x: p.x, y: p.y, columns, aspect: p.aspect || aspectOf(cx.size), extent: p.extent as never, value: p.value, fn }),
      op.derive("shade", e(shade(ramp, "d.value"))));
    const k = p.size ? `clamp(sqrt(max(d.value, 0) / max(table.max(${q(bins)}, "value"), 1e-12)), 0.18, 1)` : "1";
    // With a gap, each hexagon shrinks by half of it; without, it grows by half a px so neighbours
    // overlap a hair and no antialiased seam shows the paper between them.
    const inset = p.gap ? `max(0, 1 - ${p.gap / 2} / max(d.hwp, 0.01))` : "(1 + 0.5 / max(d.hwp, 1))";
    const px = cx.table("hexagons", bins,
      op.derive("cx", e("scale.x(d.x)")), op.derive("cy", e("scale.y(d.y)")),
      op.derive("hwp", e("abs(scale.x(d.x + d.hw) - scale.x(d.x))")), op.derive("hhp", e("abs(scale.y(d.y + d.hh) - scale.y(d.y))")),
      op.derive("f", e(`${k} * ${inset}`)));
    // Corners of a pointy-top hexagon, clockwise from the top.
    const corner = ([a, b]: [number, number]) => `(d.cx + ${a} * d.hwp * d.f) + "," + (d.cy + ${b} * d.hhp * d.f)`;
    const hexPath = `"M" + ${[[0, -1], [1, -0.5], [1, 0.5], [0, 1], [-1, 0.5], [-1, -0.5]].map((c) => corner(c as [number, number])).join(' + "L" + ')} + "Z"`;
    const unit = p.unit || "rows";
    const tip = p.label ?? binTip(p.value, fn, unit, p.format, "near");
    return group({
      key: "hexbin",
      semantics: { role: "series", label: e(`${q(p.name ?? p.data)} + ": " + format(table.count(${q(bins)}), ",") + " hexagons, up to " + format(table.max(${q(bins)}, "count"), ",") + ${q(` ${unit} each`)}`) },
      scales: { hex: { type: "sequential", domain: { data: bins, field: "shade" }, range: "$sequential" } },
      children: [
        // Keyed by the lattice: hexagons of another size are other hexagons, so a change of size
        // crossfades the two layers whole (the same lattice morphs hexagon by hexagon).
        group({ key: `hexagons-${columns}`, children: [repeat(px, shape(geom.path(e(hexPath)), { fill: p.fill ?? e("scale.hex(d.shade)"), pickable: false }))] }),
        // Where the pointer finds a hexagon: an invisible disc as wide as each, with its tooltip
        // (instances, so the accessible description lists a sample, not every hexagon).
        instances({ key: `pick-${columns}`, from: px, x: e("d.cx"), y: e("d.cy"), r: e("d.hwp"), fill: "transparent", instanceKey: e("d.hex"), label: tip }),
        p.fill === undefined ? rampKey("hex", bins, keyTitle(p.value, fn, unit, "hexagon"), p.format, p.legend) : null,
      ],
    });
  },
});

// ---- heatmap2d ------------------------------------------------------------------------------------

export interface Heatmap2dParams {
  data: string; x: string; y: string; cell: number; columns: number; rows: number; value: string; fn: Agg; ramp: Ramp;
  extent: unknown; gap: number; legend: KeyAt; format: string; label: Prop; name: string; unit: string;
}

export const heatmap2d = recipe<Heatmap2dParams>({
  id: "@datars/std/heatmap2d",
  doc: "A 2-D density raster: rows of any number counted into a regular grid of cells over the plot's x and y scales in one pass, each cell coloured by its count (or an aggregate of a field). Put it in a plot.",
  params: {
    data: t.table("The rows (any number: counted once per data, in the engine)."),
    x: t.field("Field on the plot's x scale (numbers or dates)."),
    y: t.field("Field on the plot's y scale."),
    cell: t.number(6, "Cell size in px: the grid is as many cells across and down as the plot area holds."),
    columns: t.number(0, "Cells across (0: from `cell`)."),
    rows: t.number(0, "Cells down (0: from `cell`)."),
    value: t.field("A field to aggregate per cell (default: count the rows)."),
    fn: t.oneOf(["count", "sum", "mean", "min", "max"] as const, undefined, "How `value` is aggregated (default: mean with a `value`, else count)."),
    ramp: t.oneOf(["linear", "sqrt", "log"] as const, undefined, "How the value maps onto the colour ramp (default: sqrt for counts, linear for aggregates)."),
    extent: t.json("The extent to grid, [x0, y0, x1, y1] in the rows' units (default: the rows' own)."),
    gap: t.number(0, "Space between cells, px."),
    legend: t.oneOf(["top-right", "top-left", "bottom-right", "bottom-left", "none"] as const, "top-right", "Where the colour key goes: a corner of the plot area, or none."),
    format: t.string(",.3~s", "Number format for the key and the tooltips."),
    label: t.prop("Tooltip per cell: an expression over its row (`d.count`, `d.value`, `d.x0`…`d.y1`); default: how many rows, where."),
    name: t.string(undefined, "What the rows are, for the accessible description."),
    unit: t.string("rows", "What one row is, in the plural, for the key and the tooltips ('pickups': 'Pickups per cell')."),
  },
  tokens: ["sequential", "paper", "ink-2", "muted", "size.small"],
  expand(p, cx) {
    const cell = Math.max(1, p.cell || 6);
    const columns = p.columns || Math.max(1, Math.round(cx.size[0] / cell));
    const rows = p.rows || Math.max(1, Math.round(cx.size[1] / cell));
    const fn = (p.fn ?? (p.value ? "mean" : "count")) as Agg;
    const ramp = (p.ramp ?? (p.value && fn !== "count" ? "linear" : "sqrt")) as Ramp;
    const bins = cx.table("bin2d", p.data,
      op.bin2d({ x: p.x, y: p.y, columns, rows, extent: p.extent as never, value: p.value, fn }),
      op.derive("shade", e(shade(ramp, "d.value"))));
    const unit = p.unit || "rows";
    const g = p.gap ?? 0;
    // Cells meet edge to edge; a quarter px of overlap keeps antialiased seams from showing.
    const grow = g > 0 ? -g : 0.25;
    const tip = p.label ?? binTip(p.value, fn, unit, p.format, "at");
    return group({
      key: "heatmap2d",
      scales: { heat: { type: "sequential", domain: { data: bins, field: "shade" }, range: "$sequential" } },
      children: [
        // Keyed by the grid: another grid crossfades as a whole layer (not cell by cell); the same
        // grid morphs its cells' colours.
        instances({
          key: `cells-${columns}x${rows}`,
          from: bins,
          proto: "rect",
          x: e(`min(scale.x(d.x0), scale.x(d.x1)) - ${grow / 2}`),
          y: e(`min(scale.y(d.y0), scale.y(d.y1)) - ${grow / 2}`),
          w: e(`max(0.5, abs(scale.x(d.x1) - scale.x(d.x0)) + ${grow})`),
          h: e(`max(0.5, abs(scale.y(d.y1) - scale.y(d.y0)) + ${grow})`),
          fill: e("scale.heat(d.shade)"),
          instanceKey: e("d.cell"),
          label: tip,
          semantics: { role: "series", label: e(`${q(p.name ?? p.data)} + ": " + format(table.count(${q(bins)}), ",") + " cells of ${columns} × ${rows}, up to " + format(table.max(${q(bins)}, "count"), ",") + ${q(` ${unit} each`)}`) },
        }),
        rampKey("heat", bins, keyTitle(p.value, fn, unit, "cell"), p.format, p.legend),
      ],
    });
  },
});

// ---- contours -----------------------------------------------------------------------------------

export interface ContoursParams {
  data: string; x: string; y: string; levels: number; shares: unknown; by: "share" | "density"; bandwidth: number; cell: number;
  style: "bands" | "lines" | "both"; stroke: Prop; opacity: number; dots: number; extent: unknown; pad: number; label: Prop; name: string; unit: string;
}

export const contours = recipe<ContoursParams>({
  id: "@datars/std/contours",
  doc: "Density contours: the shape of a crowd of rows — a smoothed density of their points cut into nested levels (by default each holding a further share of the rows: the innermost line encloses the densest fifth), drawn as filled bands or lines. Put it in a plot.",
  params: {
    data: t.table("The rows (any number: the density is computed once per data, in the engine)."),
    x: t.field("Field on the plot's x scale."),
    y: t.field("Field on the plot's y scale."),
    levels: t.number(4, "How many levels: with `by: \"share\"` they enclose 1/(levels+1), 2/(levels+1)… of the rows."),
    shares: t.json("Exact shares of the rows the levels enclose, e.g. [0.9, 0.5, 0.25] (overrides `levels`)."),
    by: t.oneOf(["share", "density"] as const, "share", "Levels by the share of rows inside them, or evenly spaced in density up to the peak."),
    bandwidth: t.number(14, "Smoothing, px on screen: the standard deviation of the Gaussian kernel (larger: rounder, fewer islands)."),
    cell: t.number(4, "Grid resolution, px: the density is sampled on cells this size over the plot area."),
    style: t.oneOf(["bands", "lines", "both"] as const, "bands", "Filled nested bands (darker inside), contour lines, or both."),
    stroke: t.prop("Line ink (default: the ramp, darker inside)."),
    opacity: t.number(0.9, "Band opacity."),
    dots: t.number(0, "Also draw a seeded sample of this many rows as faint dots over the bands (0: none) — the crowd itself."),
    extent: t.json("The extent of the density grid, [x0, y0, x1, y1] (default: the rows' own, widened by `pad`)."),
    pad: t.number(0.05, "How far the default extent reaches past the rows, as a share of their range (room for the outer line to close)."),
    label: t.prop("Tooltip and accessible name per level: an expression over its row (`d.share`, `d.density`, `d.level`); default: the share of rows inside."),
    name: t.string(undefined, "What the rows are, for the accessible description."),
    unit: t.string("rows", "What one row is, in the plural, for the tooltips ('eruptions': '50% of the eruptions inside')."),
  },
  tokens: ["sequential", "ink", "size.small"],
  expand(p, cx) {
    const cell = Math.max(1, p.cell || 4);
    const columns = Math.max(2, Math.round(cx.size[0] / cell));
    const rows = Math.max(2, Math.round(cx.size[1] / cell));
    const shares = Array.isArray(p.shares) ? (p.shares as number[]) : undefined;
    const n = shares?.length ?? Math.max(1, Math.round(p.levels ?? 4));
    const levels = cx.table("contours", p.data, op.contours({
      x: p.x, y: p.y, columns, rows, extent: p.extent as never, pad: p.pad, bandwidth: Math.max(0.5, (p.bandwidth ?? 14) / cell), levels: n, shares, by: p.by,
    }));
    const paths = cx.table("contour-paths", levels, op.paths({ by: "level", ring: "ring", closed: true, x: e("scale.x(d.x)"), y: e("scale.y(d.y)") }));
    const unit = p.unit || "rows";
    const tip = p.label ?? e(`d.share >= 0.995 ? ${q(`Almost all the ${unit} inside`)} : format(d.share, ".0%") + ${q(` of the ${unit} inside`)}`);
    const bands = p.style !== "lines";
    const lines = p.style !== "bands";
    // The ramp from a step above its lightest (the outer band reads against the paper) to darkest.
    const lev = { type: "sequential" as const, domain: [-0.6, Math.max(1, n - 1)], range: "$sequential" };
    const dots = p.dots > 0 ? cx.table("contour-dots", p.data, op.sample(Math.round(p.dots), 7)) : null;
    return group({
      key: "contours",
      clip: "box",
      semantics: { role: "series", label: `${p.name ?? p.data}: density contours, ${n} levels` },
      scales: { lev },
      children: [
        bands ? group({ key: "bands", children: [repeat(paths, shape(geom.path(e("d.path")), {
          fill: e("scale.lev(d.level)"), opacity: p.opacity, semantics: { role: "region", label: tip }, pickable: true,
        }))] }) : null,
        dots ? instances({ key: "dots", from: dots, x: e(`scale.x(d.${p.x})`), y: e(`scale.y(d.${p.y})`), r: 1.1, fill: "$ink", opacity: 0.28, semantics: { role: "decoration" } }) : null,
        lines ? group({ key: "lines", children: [repeat(paths, shape(geom.path(e("d.path")), {
          stroke: { paint: p.stroke ?? (bands ? "$paper" : e("scale.lev(d.level)")), width: bands ? 0.8 : e("1 + d.level * 0.35"), join: "round" },
          semantics: bands ? { role: "decoration" } : { role: "region", label: tip }, pickable: !bands,
        }))] }) : null,
      ],
    });
  },
});

// ---- manyLines ----------------------------------------------------------------------------------

export interface ManyLinesParams {
  data: string; x: string; y: string; series: string; xType: string; stroke: Prop; opacity: number; width: number;
  highlight: Prop; highlightStroke: Prop; highlightWidth: number; labels: boolean; hover: boolean; label: Prop; name: string;
}

export const manyLines = recipe<ManyLinesParams>({
  id: "@datars/std/manyLines",
  doc: "Thousands of time series at once: every series a faint path, so their bulk shows where most of them run, with the ones that matter highlighted in the accent colour and labelled. Hover a line for its series. Put it in a plot.",
  params: {
    data: t.table("The rows: one per series and x (in x order within each series)."),
    x: t.field("Field on the plot's x scale (numbers or dates)."),
    y: t.field("Field on the plot's y scale."),
    series: t.field("The field naming each row's series: one path per value."),
    xType: t.string("linear", "The plot's x scale type (a band scale puts points mid-band)."),
    stroke: t.prop("Ink of the series that aren't highlighted (default $muted: grey, so the highlighted ones stand out)."),
    opacity: t.number(0, "Their opacity (0: from how many there are — fainter the more there are, so where most of them run darkens instead of filling solid)."),
    width: t.number(1, "Their stroke width, px."),
    highlight: t.prop("The series to bring forward: a value of `series`, a list of them, or an expression (a signal a control or story step sets). None by default."),
    highlightStroke: t.prop("Ink of the highlighted series (default $accent)."),
    highlightWidth: t.number(2.5, "Stroke width of the highlighted series, px."),
    labels: t.bool(true, "Name each highlighted series at its last point (inside the plot's right edge, pushed apart so names never overlap)."),
    hover: t.bool(true, "The series under the pointer comes forward in the highlight colour while hovered (`hover()`; never on touch screens, where a tap names it)."),
    label: t.prop("Tooltip and accessible name per series: an expression over its first row (default: the series' name)."),
    name: t.string(undefined, "What the series are, for the accessible description ('3,000 weather stations')."),
  },
  tokens: ["muted", "accent", "paper", "size.label"],
  expand(p, cx) {
    const s = p.series;
    const band = p.xType === "band" || p.xType === "point";
    const x = band ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    // One path per series, built per resolve from the rows through the scales (a path each, not a
    // mark per point: a frame draws a few hundred paths whatever the rows).
    const lines = cx.table("lines", p.data, op.paths({ by: s, x: e(x), y: e(`scale.y(d.${p.y})`) }));
    const count = `table.count(${q(lines)})`;
    const isExprObj = (v: unknown): v is { expr: string } => typeof v === "object" && v !== null && "expr" in v;
    const opacity = p.opacity ? String(p.opacity) : `clamp(3 / sqrt(max(${count}, 1)), 0.04, 0.7)`;
    const inkSrc = (v: Prop) => (isExprObj(v) ? `(${v.expr})` : q(v));
    const rest = p.stroke ?? "$muted";
    const accent = p.highlightStroke ?? "$accent";
    const tip = p.label ?? e(`key.name(d.${s})`);
    const hl = p.highlight;
    const hlTest = hl === undefined || hl === null ? null
      : Array.isArray(hl) ? hl.map((v) => `d.${s} == ${q(v)}`).join(" || ") || "false"
      : isExprObj(hl) ? `d.${s} == (${hl.expr})`
      : `d.${s} == ${q(hl)}`;
    const picked = hlTest ? cx.table("highlight", lines, op.filter(e(hlTest))) : null;
    // Their names by their last points, pushed apart vertically so two ending close never collide.
    const names = picked && p.labels ? cx.table("names", picked, op.spread({ position: e("d.end_y"), gap: e('token("size.label") + 3'), min: 0, max: e("box.h"), as: "label_y" })) : null;
    const path = (width: Prop, paint: Prop, extra: Record<string, unknown>) => shape(geom.path(e("d.path")), { stroke: { paint, width, join: "round", cap: "round" }, pickable: true, semantics: { role: "series", label: tip }, ...extra });
    return group({
      key: "many-lines",
      semantics: { role: "group", label: e(`${q(p.name ?? p.data)} + ": " + format(${count}, ",") + " series"`) },
      children: [
        // Hovered (with a mouse), a series takes the highlight's ink, width and full opacity.
        group({ key: "all", children: [repeat(lines, p.hover
          ? path(e(`hover() ? ${p.highlightWidth || 2.5} : ${p.width || 1}`), e(`hover() ? ${inkSrc(accent)} : ${inkSrc(rest)}`), { opacity: e(`hover() ? 1 : ${opacity}`) })
          : path(p.width || 1, rest, { opacity: e(opacity) }))] }),
        picked ? group({ key: "highlight", children: [repeat(picked, path(p.highlightWidth || 2.5, accent, {}))] }) : null,
        names ? group({ key: "labels", children: [repeat(names, text(e(`key.name(d.${s})`), [e("d.end_x - 6"), e("d.label_y")], {
          style: { size: "$size.label", weight: 600, ink: accent, align: "end", baseline: "middle", contain: true }, halo: ["$paper", 3],
        }))] }) : null,
      ],
    });
  },
});
