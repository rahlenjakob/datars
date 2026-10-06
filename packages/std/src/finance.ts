// Financial charts: candlesticks and OHLC bars, volume, moving averages, volatility bands, indexed
// comparisons, drawdowns and sparklines. The marks read the enclosing plot's scales like bar and
// line do. On a band x scale over dates (`xType: "band"`) trading days sit side by side — no
// room for weekends and holidays — and the axis labels the first session of each month (year,
// quarter…). Prices that rise draw in `$up`, falling ones in `$down` (theme tokens: `$positive`
// and `$negative` unless a theme swaps them for markets that read red as up).
//
// Indicators (moving averages, bands, drawdowns) can compute over a longer history than the rows
// on show (`source`), so the first visible day already has a full window, and the chart zooms
// from a year to a month without the averages starting over.

import { e, geom, group, op, recipe, repeat, shape, t, text, Cx, Op, Prop, Recipe, Template } from "@datars/sdk";
import { area, line } from "./marks.js";
import { rule } from "./guides.js";

const isBand = (t?: string) => t === "band" || t === "point";
/** The centre of a row's slot on x. */
const xMid = (x: string, xType: string) => (isBand(xType) ? `scale.x(d.${x}) + scale.x.bandwidth() / 2` : `scale.x(d.${x})`);
/** An ink chosen per row by direction (close at or above open: up). */
const byDirection = (o: string, c: string, up: string, down: string) => e(`d.${c} >= d.${o} ? ${JSON.stringify(up)} : ${JSON.stringify(down)}`);
/** A field name safe in expressions for a number (2.5 → 2_5). */
const tag = (v: number) => String(v).replace(/[^0-9A-Za-z]/g, "_");
/** A recipe's name, whatever its package: an ejected `@local/candlestick/candlestick` is still a candlestick. */
const nameOf = (id: unknown) => String(id ?? "").split("/").pop();

// A named type, not an inline one: `datars eject` copies helpers by their first brace block.
type SlotParams = { data: string; xType: string; width: number };

/** A candle's (or bar's) width: the band (or `width` × band) on a band scale; else `width` px, or
 * 70 % of the average spacing of the rows. */
function slotWidth(p: SlotParams): string {
  if (isBand(p.xType)) return p.width ? `scale.x.bandwidth() * ${p.width}` : "scale.x.bandwidth()";
  return p.width ? String(p.width) : `max(1, box.w / max(1, table.count(${JSON.stringify(p.data)})) * 0.7)`;
}

type HistoryParams = { data: string; source: string; x: string; series: string };

/** The rows an indicator draws: `ops` over `source` (a longer history) when given, then only the
 * rows also in `data` (by x, and series) — or over `data` itself. */
function historyOps(p: HistoryParams, ops: Op[]): [string, Op[]] {
  if (!p.source || p.source === p.data) return [p.data, ops];
  return [p.source, [...ops, op.join(p.data, p.series ? [p.series, p.x] : [p.x], "inner")]];
}

type Ohlc = { open: string; high: string; low: string; close: string; y: string; x: string; format: string };

/** A candle's or bar's label (tooltip, accessible name): date, open, high, low, close, and the
 * change from the previous close when there is one. */
function ohlcLabel(p: Ohlc): Prop {
  const c = p.close ?? p.y;
  const f = (v: string) => `format(d.${v}, ${JSON.stringify(p.format)})`;
  const change = `(d.prev_close > 0 ? " (" + format(d.${c} / d.prev_close - 1, "+.2%") + ")" : "")`;
  return e(`scale.x.label(d.${p.x}) + " · O " + ${f(p.open)} + " H " + ${f(p.high)} + " L " + ${f(p.low)} + " C " + ${f(c)} + ${change}`);
}

// ---- candlestick --------------------------------------------------------------------------------

export interface CandlestickParams {
  data: string; x: string; y: string; xType: string; open: string; high: string; low: string; close: string;
  up: string; down: string; hollow: boolean; width: number; format: string; label: Prop;
}

export const candlestick = recipe<CandlestickParams>({
  id: "@datars/std/candlestick",
  doc: "Candlesticks: per row a wick from low to high and a body from open to close, in `$up` when the close is at or above the open and `$down` below. Use in a plot with `xType: \"band\"` over dates for trading days without gaps; the plot fits its value axis to the lows and highs. Hovering a candle reads its date, OHLC and change.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date (or time) field."), y: t.field("The plot's value field: the close, unless `close` is given."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    open: t.string("open", "Opening price field."), high: t.string("high", "High field."), low: t.string("low", "Low field."),
    close: t.field("Closing price field (default: the plot's `y`, else `close`)."),
    up: t.ink("$up", "Ink of rising candles."), down: t.ink("$down", "Ink of falling candles."),
    hollow: t.bool(false, "Hollow candles: rising bodies outlined, falling ones filled."),
    width: t.number(0, "Body width: a fraction of the band on a band scale (0 = the band, whose padding leaves the gaps), px on a continuous scale (0 = 70% of the spacing)."),
    format: t.string(",.2f", "Price format in labels."),
    label: t.prop("Candle label (tooltip, accessible name); default date · O H L C (change)."),
  },
  tokens: ["up", "down", "paper"],
  expand(p, cx) {
    const c = p.close ?? p.y ?? "close";
    const [o, h, l] = [p.open, p.high, p.low];
    // The previous close, for the change a tooltip reads.
    const tbl = cx.table("candles", p.data, op.window("lag", c, "prev_close", { order: p.x }));
    const mid = xMid(p.x, p.xType);
    const w = slotWidth(p);
    const ink = byDirection(o, c, p.up, p.down);
    const wick = shape(geom.segment({ x1: e(mid), y1: e(`scale.y(d.${h})`), x2: e(mid), y2: e(`scale.y(d.${l})`) }), { stroke: { paint: ink, width: 1 }, semantics: { role: "decoration" } });
    const body = shape(geom.rect({
      x: e(`${mid} - (${w}) / 2`),
      y: e(`min(scale.y(d.${o}), scale.y(d.${c}))`),
      w: e(w),
      // A doji (open = close) still shows as a line.
      h: e(`max(1, abs(scale.y(d.${o}) - scale.y(d.${c})))`),
    }), {
      fill: p.hollow ? e(`d.${c} >= d.${o} ? "$paper" : ${JSON.stringify(p.down)}`) : ink,
      stroke: p.hollow ? { paint: ink, width: 1 } : undefined,
      semantics: { role: "datum", label: p.label ?? ohlcLabel({ ...p, close: c }), value: e(`d.${c}`) },
      pickable: true,
    });
    // Bodies are keyed by the datum (a std convention) so a candle morphs to the same day's
    // candle in another range; wicks are keyed the same under their own group.
    return group({
      key: "candles",
      semantics: { role: "series", label: `${c} candles` },
      children: [group({ key: "wicks", children: [repeat(tbl, wick)] }), group({ key: "bodies", children: [repeat(tbl, body)] })],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }],
});

// ---- ohlc ---------------------------------------------------------------------------------------

export interface OhlcParams { data: string; x: string; y: string; xType: string; open: string; high: string; low: string; close: string; up: string; down: string; colored: boolean; width: number; format: string; label: Prop }

export const ohlc = recipe<OhlcParams>({
  id: "@datars/std/ohlc",
  doc: "OHLC bars: per row a vertical line from low to high with the open ticked to the left and the close to the right, in `$up`/`$down` by direction (or ink). Same fields and plot fitting as `candlestick`.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date (or time) field."), y: t.field("The plot's value field: the close, unless `close` is given."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    open: t.string("open", "Opening price field."), high: t.string("high", "High field."), low: t.string("low", "Low field."),
    close: t.field("Closing price field (default: the plot's `y`, else `close`)."),
    up: t.ink("$up", "Ink of rising bars."), down: t.ink("$down", "Ink of falling bars."),
    colored: t.bool(true, "Colour by direction; off: every bar in `$ink`."),
    width: t.number(0, "Tick span: a fraction of the band (0 = the band), px on a continuous scale."),
    format: t.string(",.2f", "Price format in labels."),
    label: t.prop("Bar label; default date · O H L C (change)."),
  },
  tokens: ["up", "down", "ink"],
  expand(p, cx) {
    const c = p.close ?? p.y ?? "close";
    const [o, h, l] = [p.open, p.high, p.low];
    const tbl = cx.table("bars", p.data, op.window("lag", c, "prev_close", { order: p.x }));
    const mid = xMid(p.x, p.xType);
    const half = `(${slotWidth(p)}) / 2`;
    const y = (f: string) => `scale.y(d.${f})`;
    const path = `\`M \${${mid}} \${${y(h)}} L \${${mid}} \${${y(l)}} M \${${mid} - ${half}} \${${y(o)}} L \${${mid}} \${${y(o)}} M \${${mid}} \${${y(c)}} L \${${mid} + ${half}} \${${y(c)}}\``;
    return group({
      key: "ohlc",
      semantics: { role: "series", label: `${c} bars` },
      children: [repeat(tbl, shape(geom.path(e(path)), {
        stroke: { paint: p.colored ? byDirection(o, c, p.up, p.down) : "$ink", width: e(`max(1, min(2, ${half} / 2))`), cap: "butt" },
        semantics: { role: "datum", label: p.label ?? ohlcLabel({ ...p, close: c }), value: e(`d.${c}`) },
        pickable: true,
      }))],
    });
  },
});

// ---- volume -------------------------------------------------------------------------------------

export interface VolumeParams { data: string; x: string; xType: string; volume: string; open: string; close: string; y: string; up: string; down: string; colored: boolean; opacity: number; yScale: string; format: string; width: number }

export const volume = recipe<VolumeParams>({
  id: "@datars/std/volume",
  doc: "Volume bars under a price chart: in a plot, they go to a pane below the price area that shares its x axis and has its own value axis (the plot's `lower` pane, made for them when not given), coloured like their day's candle.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date field."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."), y: t.field("The plot's value field: the close, unless `close` is given."),
    volume: t.string("volume", "Volume field."),
    open: t.string("open", "Opening price field (for the colour)."), close: t.field("Closing price field (default: the plot's `y` — unless that is the volume — else `close`)."),
    up: t.ink("$up", "Ink on rising days."), down: t.ink("$down", "Ink on falling days."),
    colored: t.bool(true, "Colour by the day's direction; off: `$muted`."),
    opacity: t.number(0.55, "Bar opacity (volume recedes behind price)."),
    yScale: t.string("lower", "The value scale: `lower` (the plot's lower pane), or `y` for a volume chart of its own."),
    format: t.string(",.0f", "Volume format in labels."),
    width: t.number(0, "Bar width: a fraction of the band (0 = the band), px on a continuous scale."),
  },
  tokens: ["up", "down", "muted"],
  expand(p) {
    // The plot's `y` is the close — unless it's the volume itself (a volume chart of its own,
    // `yScale: "y"`): then the close is the `close` field, not the bar's own height.
    const c = p.close ?? (p.y && p.y !== p.volume ? p.y : "close");
    const s = p.yScale || "lower";
    const w = slotWidth(p);
    const v = `d.${p.volume}`;
    return group({
      key: "volume",
      semantics: { role: "series", label: p.volume },
      children: [repeat(p.data, shape(geom.rect({
        x: e(`${xMid(p.x, p.xType)} - (${w}) / 2`),
        y: e(`min(scale.${s}(0), scale.${s}(${v}))`),
        w: e(w),
        h: e(`abs(scale.${s}(0) - scale.${s}(${v}))`),
      }), {
        fill: p.colored ? byDirection(p.open, c, p.up, p.down) : "$muted",
        opacity: p.opacity,
        semantics: { role: "datum", label: e(`scale.x.label(d.${p.x}) + " · volume " + format(${v}, ${JSON.stringify(p.format)})`), value: e(v) },
        pickable: true,
      }))],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }],
});

// ---- moving average -----------------------------------------------------------------------------

export interface MovingAverageParams { data: string; x: string; y: string; xType: string; series: string; color: string; window: number; kind: "sma" | "ema"; source: string; full: boolean; stroke: Prop; width: number; label: string }

type MaParams = { y: string; window: number; kind: string; series: string; x: string; full: boolean };

/** A moving average's column name and ops. */
function maOps(p: MaParams): [string, Op[]] {
  const as = `${p.kind === "ema" ? "ema" : "sma"}_${tag(p.window)}`;
  const fn = p.kind === "ema" ? "ema" : "rolling_mean";
  return [as, [op.window(fn, p.y, as, { partition: p.series ? [p.series] : undefined, order: p.x, k: p.window, min: p.full ? p.window : undefined })]];
}

export const movingAverage = recipe<MovingAverageParams>({
  id: "@datars/std/movingAverage",
  doc: "A moving average of the plot's `y` (a closing price) as a line: simple (the mean of the last `window` rows) or exponential (span `window`). Computed over `source` — the whole history — when given, so the first day on show already has its full window. Shown in the plot's indicator key.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date field."), y: t.field("The averaged field (default: the plot's `y`)."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t.field("One average per series (tickers)."), color: t.field("The plot's colour field (per-series inks)."),
    window: t.number(20, "Rows per window (trading days on a band scale): 20 ≈ a month, 50, 200."),
    kind: t.oneOf(["sma", "ema"] as const, "sma", "Simple or exponential."),
    source: t.table("A longer history of the same rows to compute over (the chart shows only the rows of `data`)."),
    full: t.bool(true, "Only full windows: no line until `window` rows (else the first rows average what there is)."),
    stroke: t.prop("Line ink (default: $accent, or the colour scale per series)."),
    width: t.number(1.5, "Stroke width."),
    label: t.string(undefined, "Name in the indicator key and to screen readers (default `SMA 20`)."),
  },
  tokens: ["accent"],
  expand(p, cx) {
    const [as, ops] = maOps(p);
    const [from, all] = historyOps(p, ops);
    const tbl = cx.table("ma", from, ...all);
    const series = p.series ?? undefined;
    const stroke = p.stroke ?? (series && p.color ? undefined : "$accent");
    return group({
      key: `ma-${as}`,
      semantics: { role: "series", label: p.label ?? `${p.kind.toUpperCase()} ${p.window}` },
      children: [line({ data: tbl, x: p.x, y: as, xType: p.xType, series, color: series ? p.color : undefined, stroke, width: p.width, curve: "linear" })],
    });
  },
});

// ---- bollinger bands ----------------------------------------------------------------------------

export interface BollingerParams { data: string; x: string; y: string; xType: string; series: string; window: number; k: number; source: string; full: boolean; fill: string; opacity: number; stroke: string; width: number; middle: boolean; label: string }

type BandParams = { y: string; window: number; k: number; series: string; x: string; full: boolean };

/** Bollinger band columns [middle, upper, lower] and their ops. */
function bandOps(p: BandParams): [string[], Op[]] {
  const id = `${tag(p.window)}_${tag(p.k)}`;
  const [mid, sd, hi, lo] = [`bb_mid_${id}`, `bb_sd_${id}`, `bb_hi_${id}`, `bb_lo_${id}`];
  const o = { partition: p.series ? [p.series] : undefined, order: p.x, k: p.window, min: p.full ? p.window : undefined };
  return [[mid, hi, lo], [
    op.window("rolling_mean", p.y, mid, o),
    op.window("rolling_std", p.y, sd, o),
    op.derive(hi, e(`d.${mid} + ${p.k} * d.${sd}`)),
    op.derive(lo, e(`d.${mid} - ${p.k} * d.${sd}`)),
  ]];
}

export const bollinger = recipe<BollingerParams>({
  id: "@datars/std/bollinger",
  doc: "Bollinger bands: the moving average of `y` over `window` rows with a shaded band `k` standard deviations (population σ) above and below it — where the price is stretched, and how volatile it is. Computes over `source` when given, like `movingAverage`; the plot fits its value axis to the band.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date field."), y: t.field("The field (default: the plot's `y`)."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."), series: t.field("One band per series."),
    window: t.number(20, "Rows per window."), k: t.number(2, "Band half-width in standard deviations."),
    source: t.table("A longer history to compute over (see movingAverage)."),
    full: t.bool(true, "Only full windows."),
    fill: t.ink("$accent", "Band ink."), opacity: t.number(0.1, "Band opacity."),
    stroke: t.ink("$accent", "Ink of the band's edges and middle line."), width: t.number(1, "Edge stroke width."),
    middle: t.bool(true, "Draw the middle (moving average) line."),
    label: t.string(undefined, "Name in the indicator key (default `BB 20, 2`)."),
  },
  tokens: ["accent"],
  expand(p, cx) {
    const [[mid, hi, lo], ops] = bandOps(p);
    const [from, all] = historyOps(p, ops);
    const tbl = cx.table("bands", from, ...all);
    const edge = (key: string, y: string, opacity: number) => group({ key, opacity, children: [line({ data: tbl, x: p.x, y, xType: p.xType, series: p.series, stroke: p.stroke, width: p.width, curve: "linear" })] });
    return group({
      key: `bollinger-${tag(p.window)}-${tag(p.k)}`,
      semantics: { role: "series", label: p.label ?? `BB ${p.window}, ${p.k}` },
      children: [
        group({ key: "band", children: [area({ data: tbl, x: p.x, y: hi, y0: lo, xType: p.xType, series: p.series, fill: p.fill, opacity: p.opacity, curve: "linear" })] }),
        edge("upper", hi, 0.7),
        edge("lower", lo, 0.7),
        p.middle ? edge("middle", mid, 0.45) : null,
      ],
    });
  },
});

// ---- indexed (rebased comparison) ----------------------------------------------------------------

export interface IndexedParams { data: string; x: string; y: string; xType: string; series: string; color: string; base: number; percent: boolean; labels: boolean; baseline: boolean; stroke: Prop; width: number; format: string }

type IndexParams = { y: string; series: string; x: string; base: number; percent: boolean };

/** The rebased column's name and ops: each series divided by its first value in the rows. */
function indexOps(p: IndexParams): [string, Op[]] {
  const as = p.percent ? "change" : "indexed";
  return [as, [
    op.window("first", p.y, "index_first", { partition: p.series ? [p.series] : undefined, order: p.x }),
    op.derive(as, e(p.percent ? `d.${p.y} / d.index_first - 1` : `d.${p.y} / d.index_first * ${p.base}`)),
  ]];
}

export const indexed = recipe<IndexedParams>({
  id: "@datars/std/indexed",
  doc: "Series rebased to their first visible row — 100 (or 0 %) — so instruments at any price compare as performance: one line per series with a baseline, each labelled at its end with its change. Filter the rows to a range and every line starts from the base again. Use with `format: \"+.0%\"` on the plot for `percent`.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date field."), y: t.field("The price field (default: the plot's `y`)."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t.field("One line per series (default: the colour field)."), color: t.field("The plot's colour field (line and label inks)."),
    base: t.number(100, "The value every series starts at."),
    percent: t.bool(false, "Change from the start (0 = no change, 0.1 = +10 %) instead of an index."),
    labels: t.bool(true, "Label each line's end: series and change (pushed apart so they never overlap)."),
    baseline: t.bool(true, "A rule at the base."),
    stroke: t.prop("Line ink (default: the colour scale, else $mark)."),
    width: t.number(0, "Stroke width (0 = the theme's)."),
    format: t.string("+.1%", "Format of the change in end labels."),
  },
  tokens: ["mark", "ink-2", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const [as, ops] = indexOps({ ...p, series });
    const tbl = cx.table("indexed", p.data, ...ops);
    const lines = line({ data: tbl, x: p.x, y: as, xType: p.xType, series, color: p.color, stroke: p.stroke, width: p.width, curve: "linear" });
    const base = p.percent ? 0 : p.base;
    const baseline = p.baseline ? rule({ axis: "y", value: base, ink: "$ink-2", dashed: true }) : null;
    const change = p.percent ? "d.y_end" : `d.y_end / ${p.base} - 1`;
    const labels = p.labels ? endLabels(cx, { table: tbl, x: p.x, xType: p.xType, series, color: p.color, field: as, value: `format(${change}, ${JSON.stringify(p.format)})`, ink: p.stroke }) : null;
    return group({ key: "indexed", children: [baseline, lines, labels] });
  },
});

type EndLabelParams = { table: string; x: string; xType: string; series: string; color: string; field: string; value: string; ink: Prop };

/** A label at each series' last point — its name and `value` (an expression over `d.y_end`) —
 * pushed apart so none overlap, drawn just past the plot area (the plot leaves room). */
function endLabels(cx: Cx, p: EndLabelParams): Template {
  const ends = cx.table("ends", p.table, op.aggregate(p.series ? [p.series] : [], { x_end: ["last", p.x], y_end: ["last", p.field] }),
    op.spread({ position: e("scale.y(d.y_end)"), gap: e('token("size.label") + 3'), min: 0, max: e("box.h"), as: "label_y" }));
  const xEnd = isBand(p.xType) ? "scale.x(d.x_end) + scale.x.bandwidth() / 2" : "scale.x(d.x_end)";
  const name = p.series ? `key.name(d.${p.series}) + " " + ` : "";
  const ink = p.ink ?? (p.series && p.color ? e(`scale.color(d.${p.series})`) : "$mark");
  return group({ key: "end-labels", children: [repeat(ends, text(e(`${name}${p.value}`), [e(`min(${xEnd}, box.w) + 6`), e("d.label_y")], { style: { size: "$size.label", ink, baseline: "middle", contain: true } }))] });
}

// ---- drawdown -----------------------------------------------------------------------------------

export interface DrawdownParams { data: string; x: string; y: string; xType: string; series: string; color: string; source: string; fill: string; opacity: number; stroke: Prop; width: number; area: boolean; labels: boolean; format: string }

type DrawdownOpsParams = { y: string; series: string; x: string };

/** Drawdown ops: value / running peak − 1 (0 at a new high, −0.25 a quarter below it). */
function drawdownOps(p: DrawdownOpsParams): [string, Op[]] {
  return ["drawdown", [
    op.window("cummax", p.y, "peak", { partition: p.series ? [p.series] : undefined, order: p.x }),
    op.derive("drawdown", e(`d.peak > 0 ? d.${p.y} / d.peak - 1 : 0`)),
  ]];
}

export const drawdown = recipe<DrawdownParams>({
  id: "@datars/std/drawdown",
  doc: "Drawdown: how far each series sits below its running peak (0 at a new high, −25 % a quarter below it), as an area hanging from zero — the pain chart next to a performance chart. With `source`, peaks count from the start of the whole history instead of the first visible row. Use with `format: \".0%\"` on the plot.",
  params: {
    data: t.table("The rows to draw (default: the plot's)."), x: t.field("The date field."), y: t.field("The price field (default: the plot's `y`)."), xType: t.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t.field("One drawdown per series (default: the colour field)."), color: t.field("The plot's colour field (line and label inks)."),
    source: t.table("A longer history: peaks before the rows on show count too."),
    fill: t.ink("$down", "Area ink (one series)."), opacity: t.number(0.22, "Area opacity."),
    stroke: t.prop("Line ink (default: $down, or the colour scale per series)."), width: t.number(1.5, "Stroke width."),
    area: t.bool(true, "Shade from zero (one series; several draw lines only)."),
    labels: t.bool(true, "Label each series' end with its name and current drawdown (with a series; pushed apart)."),
    format: t.string("+.1%", "Format of the drawdown in end labels."),
  },
  tokens: ["down", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const [as, ops] = drawdownOps({ ...p, series });
    const [from, all] = historyOps({ ...p, series }, ops);
    const tbl = cx.table("drawdown", from, ...all);
    const shade = p.area && !series ? area({ data: tbl, x: p.x, y: as, xType: p.xType, fill: p.fill, opacity: p.opacity, curve: "linear" }) : null;
    const stroke = p.stroke ?? (series && p.color ? undefined : p.fill);
    return group({ key: "drawdown", semantics: { role: "series", label: "drawdown" }, children: [
      shade ? group({ key: "shade", children: [shade] }) : null,
      line({ data: tbl, x: p.x, y: as, xType: p.xType, series, color: series ? p.color : undefined, stroke, width: p.width, curve: "linear" }),
      p.labels && series ? endLabels(cx, { table: tbl, x: p.x, xType: p.xType, series, color: p.color, field: as, value: `format(d.y_end, ${JSON.stringify(p.format)})`, ink: p.stroke }) : null,
    ] });
  },
});

// ---- sparkline ----------------------------------------------------------------------------------

export interface SparklineParams { data: string; x: string; y: string; xType: "linear" | "time" | "point"; trend: boolean; stroke: Prop; fill: boolean; dot: boolean; width: number; format: string; name: string }

export const sparkline = recipe<SparklineParams>({
  id: "@datars/std/sparkline",
  doc: "A sparkline: a word-sized line of `y` over `x` that fills its box (a table cell, a card) — no axes, the last value marked with a dot, coloured `$up` or `$down` by whether it ended above where it started. Works on any table, `@group` too (one per ticker in a repeat).",
  params: {
    data: t.table("The rows (a named table, or `@group` inside a repeat)."), x: t.field("The date (or order) field."), y: t.field("The value field."),
    xType: t.oneOf(["linear", "time", "point"] as const, "point", "x scale: `point` spaces rows evenly (trading days), `time` by date."),
    trend: t.bool(true, "Colour by the change over the line ($up / $down); off: $mark."),
    stroke: t.prop("Line ink (overrides the trend colour)."),
    fill: t.bool(true, "A soft area under the line."),
    dot: t.bool(true, "A dot at the last value."),
    width: t.number(1.5, "Stroke width."),
    format: t.string(",.2f", "Value format in the label."),
    name: t.string(undefined, "What the line is (its accessible label starts with it)."),
  },
  tokens: ["up", "down", "mark"],
  expand(p) {
    const [first, last] = [`group.first(${JSON.stringify(p.y)})`, `group.last(${JSON.stringify(p.y)})`];
    const ink = p.stroke ?? (p.trend ? e(`${last} >= ${first} ? "$up" : "$down"`) : "$mark");
    const x = e(`scale.sx(d.${p.x})`), y = e(`scale.sy(d.${p.y})`);
    const label = e(`${JSON.stringify(p.name ? `${p.name}: ` : "")} + format(${last}, ${JSON.stringify(p.format)}) + " (" + format(${last} / ${first} - 1, "+.1%") + ")"`);
    const pad = p.dot ? 3 : 1;
    return group({
      key: "sparkline",
      // Narrower than this, a line says nothing (and its range would turn inside out).
      when: e("box.w >= 24 && box.h >= 8"),
      // Rows in order, first to last: the last one sits at the right end of the x range.
      scales: {
        sx: { type: p.xType, domain: { data: p.data, field: p.x }, range: [pad, `=box.w - ${pad}`], padding: 0, nice: false },
        sy: { type: "linear", domain: { data: p.data, field: p.y }, range: [`=box.h - ${pad}`, pad], zero: false, nice: false },
      },
      children: [repeat({ groups: p.data, by: "__all" }, group({ children: [
        p.fill ? shape(geom.area({ from: "@group", x, y0: e("box.h"), y1: y, curve: "linear" }), { key: "area", fill: ink, opacity: 0.12, semantics: { role: "decoration" } }) : null,
        shape(geom.polyline({ from: "@group", x, y, curve: "linear" }), { key: "line", stroke: { paint: ink, width: p.width, join: "round", cap: "round" }, semantics: { role: "series", label } }),
        p.dot ? shape(geom.circle({ cx: e("scale.sx.max()"), cy: e(`scale.sy(${last})`), r: 2.5 }), { key: "dot", fill: ink, semantics: { role: "decoration" } }) : null,
      ] }))],
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }],
});

// ---- what a plot needs from these -----------------------------------------------------------------

/** What an enclosing plot hands its marks. */
export interface Inherited { data?: string; x?: string; y?: string; color?: string; xType?: string }

/** The value-axis domain a plot's finance marks need (see `financeDomain`). */
export interface FinanceDomain { domain: { data: string; fields: string[] }; zero: boolean }

/** A finance mark's claim on the plot's value axis: fields of a table built from `from` by `ops`
 * (`late` ops run on the rows on show, after a history is cut to them). */
type Claim = { from: string; ops: Op[]; late: Op[]; fields: string[]; zero: boolean; replace: boolean; series?: string; when?: string };

/** A `when` (an expression, `=source`, or a literal) as expression source. */
function whenExpr(w: unknown): string {
  if (w && typeof w === "object" && "expr" in w) return String((w as Record<string, unknown>).expr);
  if (typeof w === "string") return w.startsWith("=") ? w.slice(1) : JSON.stringify(w);
  return JSON.stringify(w ?? true);
}

type Specs = Record<string, Record<string, unknown>>;

/** The param specs of the finance marks that claim a plot's value axis (by recipe id). */
function claimingDefs(id: unknown): Specs | undefined {
  const all = [candlestick, ohlc, movingAverage, bollinger, indexed, drawdown] as unknown as Recipe<unknown>[];
  return all.find((r) => nameOf(r.id) === nameOf(id))?.def.params as Specs | undefined;
}

/** A child's params as it will expand: the plot's, its own, then its defaults. */
function paramsOf(c: Template, inherit: Inherited, specs: Specs): Record<string, unknown> {
  const out: Record<string, unknown> = { ...inherit, ...(c.params as object) };
  for (const [k, s] of Object.entries(specs)) if (out[k] === undefined && s.default !== undefined) out[k] = s.default;
  return out;
}

function claimOf(c: Template, inherit: Inherited): Claim | null {
  const specs = c.kind === "use" ? claimingDefs(c.recipe) : undefined;
  if (!specs) return null;
  const q = paramsOf(c, inherit, specs) as Record<string, never>;
  const data = q.data as string, y = (q.y ?? "close") as string;
  const series = (q.series ?? (nameOf(c.recipe) === "indexed" || nameOf(c.recipe) === "drawdown" ? q.color : undefined)) as string | undefined;
  const hist = (cols: string[], ops: Op[], zero: boolean, replace = false): Claim => {
    const from = (q.source as string | undefined) && q.source !== data ? (q.source as string) : data;
    return { from, ops, late: [], fields: cols, zero, replace, series };
  };
  switch (nameOf(c.recipe)) {
    case "candlestick":
    case "ohlc":
      return { from: data, ops: [], late: [], fields: [q.low, q.high], zero: false, replace: false };
    case "movingAverage": {
      const [as, ops] = maOps({ ...q, y, series } as never);
      return hist([as], ops, false);
    }
    case "bollinger": {
      const [[, hi, lo], ops] = bandOps({ ...q, y, series } as never);
      return hist([hi, lo], ops, false);
    }
    case "indexed": {
      const [as, ops] = indexOps({ ...q, y, series } as never);
      return { from: data, ops: [], late: ops, fields: [as], zero: false, replace: true, series };
    }
    case "drawdown": {
      const [as, ops] = drawdownOps({ ...q, y, series } as never);
      return hist([as], ops, true, true);
    }
  }
  return null;
}

/** The value-axis domain a plot's finance marks need — candles reach their lows and highs, bands
 * their edges, indexed lines replace prices with the index — and whether it should include zero
 * (prices: no; drawdowns: yes). One derived table: the marks' ops over a common history (cut to
 * the rows on show), then the ops that work on those rows. Undefined without finance marks. */
export function financeDomain(children: Template[], inherit: Inherited, cx: Cx): FinanceDomain | undefined {
  const claims = children.map((c) => { const k = claimOf(c, inherit); return k && c.when !== undefined ? { ...k, when: whenExpr(c.when) } : k; }).filter((c): c is Claim => c !== null);
  if (!claims.length || !inherit.data) return undefined;
  const data = inherit.data;
  const history = claims.find((c) => c.from !== data)?.from;
  const seen = new Set<string>();
  const uniq = (ops: Op[]) => ops.filter((o) => { const k = JSON.stringify(o); return seen.has(k) ? false : (seen.add(k), true); });
  const early = uniq(claims.filter((c) => c.from === (history ?? data)).flatMap((c) => c.ops));
  const series = claims.find((c) => c.series)?.series;
  const cut = history ? [op.join(data, series ? [series, inherit.x as string] : [inherit.x as string], "inner")] : [];
  // Claims over the rows on show (candles, indexed lines) run after the cut.
  const late = uniq(claims.flatMap((c) => (c.from === data && history ? [...c.ops, ...c.late] : c.late)));
  const replace = claims.some((c) => c.replace);
  // A hidden mark (its `when` false: the bands before the story gets to them) claims nothing:
  // its fields read null while it's hidden.
  const shown: Op[] = [];
  const claimed = claims.flatMap((c, i) => c.fields.map((f) => {
    if (!c.when) return f;
    shown.push(op.derive(`${f}_shown_${i}`, e(`(${c.when}) ? d.${f} : null`)));
    return `${f}_shown_${i}`;
  }));
  const fields = [...new Set([...claimed, ...(replace || !inherit.y ? [] : [inherit.y])])];
  const tbl = cx.table("value-axis", history ?? data, ...early, ...cut, ...late, ...shown);
  return { domain: { data: tbl, fields }, zero: claims.some((c) => c.zero) };
}

/** An entry per indicator (moving averages, bands) for the plot's key: a swatch and its name,
 * shown when the indicator is. */
export function financeKey(children: Template[]): Template[] {
  const out: Template[] = [];
  for (const c of children) {
    if (c.kind !== "use") continue;
    const q = (c.params ?? {}) as Record<string, unknown>;
    let label: string, swatch: Template;
    if (nameOf(c.recipe) === "movingAverage") {
      const kind = String(q.kind ?? "sma").toUpperCase(), w = q.window ?? 20;
      label = (q.label as string) ?? `${kind} ${w}`;
      swatch = shape(geom.segment({ x1: 0, y1: 7, x2: 14, y2: 7 }), { stroke: { paint: (q.stroke as Prop) ?? "$accent", width: 2, cap: "round" } });
    } else if (nameOf(c.recipe) === "bollinger") {
      const w = q.window ?? 20, k = q.k ?? 2;
      label = (q.label as string) ?? `BB ${w}, ${k}`;
      swatch = shape(geom.rect({ x: 0, y: 2, w: 14, h: 10, r: 2 }), { fill: (q.fill as string) ?? "$accent", opacity: 0.35, stroke: { paint: (q.stroke as string) ?? "$accent", width: 1 } });
    } else continue;
    out.push(group({ key: `key-${label}`, when: c.when as Prop, semantics: { role: "legend-item", label }, children: [swatch, text(label, [19, 11], { style: { size: "$size.label", ink: "$ink-2" } })] }));
  }
  return out;
}
