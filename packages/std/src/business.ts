// What businesses and newsrooms put on dashboards and in stories: KPIs (a big number and its
// change), bullet graphs, gauges and progress toward a goal; plans on a time axis (gantt) and
// events along one (timeline); a spider chart for many dimensions; a data table drawn by the
// engine; and tile-grid maps. Recipes describe, the engine computes: anything data-sized is a
// table op (row positions, lane packing, grid layouts), the marks only read the columns.

import { e, geom, group, instances, op, recipe, repeat, shape, t, text, Op, Prop, ScaleDecl, Template } from "@datars/sdk";
import { axis } from "./axis.js";
import { grid } from "./grid.js";
import { legend } from "./guides.js";
import { sparkline } from "./finance.js";

/** A string as an expression literal. */
const q = (s: string) => JSON.stringify(s);
/** A field of the row as expression source (`d.sales`, `d["net sales"]`). */
const fld = (f: string) => (/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(f) ? `d.${f}` : `d[${JSON.stringify(f)}]`);

/** A value — a number, an expression (`e(…)`) or, in hand-written JSON, an "=…" string — as
 * expression source; a plain string is a literal (a date on a time scale, a category). */
function vsrc(v: unknown, fallback = "null"): string {
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "null";
  if (typeof v === "boolean") return String(v);
  if (v && typeof v === "object" && "expr" in (v as object)) return `(${(v as { expr: string }).expr})`;
  if (typeof v === "string") return v.startsWith("=") ? `(${v.slice(1)})` : JSON.stringify(v);
  return fallback;
}

/** A number as text with its prefix and suffix, the sign first (−$3.2M, not $−3.2M). */
function affixed(value: string, format: string, prefix?: string, suffix?: string): string {
  if (!prefix && !suffix) return `format(${value}, ${q(format)})`;
  return `((${value}) < 0 ? "−" : "") + ${q(prefix ?? "")} + format(abs(${value}), ${q(format)}) + ${q(suffix ?? "")}`;
}

/** A change as text, always signed (+4.1%, −120, 0): `format` without its own sign. */
function signed(value: string, format: string, prefix?: string, suffix?: string): string {
  const f = format.replace(/^\+/, "");
  return `((${value}) > 0 ? "+" : (${value}) < 0 ? "−" : "") + ${q(prefix ?? "")} + format(abs(${value}), ${q(f)}) + ${q(suffix ?? "")}`;
}

// ---- kpi ------------------------------------------------------------------------------------------

export interface KpiParams {
  label: string; value: Prop; compare: Prop; data: string; x: string; y: string;
  format: string; prefix: string; suffix: string; delta: "percent" | "absolute" | "none"; deltaFormat: string;
  compareLabel: string; better: "up" | "down" | "neither"; spark: boolean; align: "start" | "center"; size: number;
}

export const kpi = recipe<KpiParams>({
  id: "@datars/std/kpi",
  doc: "A key figure: its label, the value as a big number (counting when it changes), its change against a comparison — an arrow and the delta in `$positive` or `$negative` by whether the change is good (`better`) — and, from a table, a sparkline of how it got there. The value and comparison are numbers or expressions, or the last and previous rows of `y` in `data`. Screen readers and tooltips read label, value and change in one sentence.",
  params: {
    label: t.string("", "What the number is (sentence case: `Revenue`)."),
    value: t.prop("The value: a number or an expression (`e(\"table.sum('orders', 'total')\")`, a signal). Default: the last `y` in `data`."),
    compare: t.prop("What the value is compared with (last month, a target): a number or an expression. Default: the row before the last in `data`. None: no change line."),
    data: t.table("Rows over time (optional): the value is the last row's `y`, the comparison the one before, and a sparkline shows them all."),
    x: t.field("The rows' order (a date): sorts them and runs the sparkline."), y: t.field("The measured field in `data`."),
    format: t.string(",.0f", "Number format of the value (d3-format: `$,.0f`, `.3~s` for 12.9k, `.1%`)."),
    prefix: t.string(undefined, "Before the number (`€`)."), suffix: t.string(undefined, "After it (` kr`, `M`)."),
    delta: t.oneOf(["percent", "absolute", "none"] as const, "percent", "How the change reads: relative (+4.1%), in the value's units (+120), or not at all."),
    deltaFormat: t.string(undefined, "Format of the change (default `.1%`, or the value's format for `absolute`); it's always signed."),
    compareLabel: t.string(undefined, "What the change is against, after it (`vs last month`)."),
    better: t.oneOf(["up", "down", "neither"] as const, "up", "Which way is good: `down` for costs and waiting times (a fall in `$positive`); `neither` keeps the change neutral."),
    spark: t.bool(true, "A sparkline of `y` under the change (with `data` and `x`)."),
    align: t.oneOf(["start", "center"] as const, "start", "Left-aligned (a dashboard tile) or centred."),
    size: t.number(0, "The number's font size in px (0: from the box, 22–60)."),
  },
  tokens: ["ink", "ink-2", "muted", "positive", "negative", "mark", "size.body"],
  expand(p, cx) {
    let value = vsrc(p.value, "");
    let compare = vsrc(p.compare, "");
    let rows: string | undefined;
    if (p.data && p.y) {
      rows = cx.table("kpi", p.data, ...(p.x ? [op.sort(p.x)] : []), op.window("lag", p.y, "__prev"));
      if (!value) value = `table.last(${q(rows)}, ${q(p.y)})`;
      if (!compare) compare = `table.last(${q(rows)}, "__prev")`;
    }
    const v = `(${value || "null"})`;
    const c = `(${compare || "null"})`;
    const hasDelta = compare !== "" && p.delta !== "none";
    const diff = `(${v} - ${c})`;
    const change = p.delta === "absolute" ? diff : `(${c} != 0 ? ${diff} / abs(${c}) : null)`;
    const dfmt = p.deltaFormat ?? (p.delta === "absolute" ? p.format : ".1%");
    // The change counts too (a signed format: `+.1%`), unless a prefix or suffix makes it text.
    const deltaFmt = `+${dfmt.replace(/^\+/, "")}`;
    const deltaCounts = !(p.delta === "absolute" && (p.prefix || p.suffix));
    const deltaText = deltaCounts ? `format(${change}, ${q(deltaFmt)})` : signed(change, dfmt, p.prefix, p.suffix);
    const good = p.better === "down" ? `${diff} < 0` : `${diff} > 0`;
    const ink = p.better === "neither" ? `"$ink-2"` : `(${diff} == 0 ? "$muted" : ${good} ? "$positive" : "$negative")`;
    const valueText = affixed(v, p.format, p.prefix, p.suffix);
    const word = `(${diff} > 0 ? "up " : ${diff} < 0 ? "down " : "unchanged")`;
    const amount = p.delta === "absolute" ? affixed(`abs(${diff})`, dfmt.replace(/^\+/, ""), p.prefix, p.suffix) : `format(abs(${change}), ${q(dfmt.replace(/^\+/, ""))})`;
    const said = `${q(p.label ? `${p.label}: ` : "")} + ${valueText}` + (hasDelta ? ` + (${c} == null || ${change} == null ? "" : ", " + ${word} + (${diff} != 0 ? ${amount} : "") + ${q(p.compareLabel ? ` ${p.compareLabel}` : "")})` : "");
    // The number's size: given, or from the box (smaller when a sparkline shares it).
    const spark = !!(p.spark && rows && p.x);
    const S = p.size ? String(p.size) : `clamp(min(box.h * ${spark ? 0.26 : 0.36}, box.w * 0.17), 22, 60)`;
    const center = p.align === "center";
    // The value line: prefix, the counting number, suffix — measured, so they sit side by side.
    const vs = "(box.h / 1.2)";
    const pw = p.prefix ? `measure(${q(p.prefix)}, ${vs}, 600)` : "0";
    const nw = `measure(format(${v}, ${q(p.format)}), ${vs}, 600)`;
    const sw = p.suffix ? `measure(${q(p.suffix)}, ${vs}, 600)` : "0";
    const x0 = center ? `((box.w - ${pw} - ${nw} - ${sw}) / 2)` : "0";
    const big = { size: e(vs), weight: 600, ink: "$ink", baseline: "top" } as const;
    const valueRow = group({
      key: "value-row",
      size: { h: e(`${S} * 1.2`) },
      children: [
        p.prefix ? text(p.prefix, [e(x0), 0], { key: "prefix", style: big }) : null,
        text("", [e(`${x0} + ${pw}`), 0], { key: "value", number: { value: e(v), format: p.format }, style: big, semantics: { role: "datum", label: e(said), value: e(v) }, pickable: true }),
        p.suffix ? text(p.suffix, [e(`${x0} + ${pw} + ${nw}`), 0], { key: "suffix", style: big }) : null,
      ],
    });
    // The change: an arrow (up or down, the same three points so it flips), the signed delta in
    // the ink of good or bad, and what it's against.
    const body = 'token("size.body")';
    const dw = `measure(${deltaText}, ${body}, 600)`;
    const lw = p.compareLabel ? `measure(${q(p.compareLabel)}, ${body})` : "0";
    const dx0 = center ? `((box.w - 13 - ${dw} - ${p.compareLabel ? `5 - ${lw}` : "0"}) / 2)` : "0";
    const mid = "box.h / 2";
    const tri = `${diff} >= 0 ? \`M \${${dx0}} \${${mid} + 4} L \${${dx0} + 4.5} \${${mid} - 4} L \${${dx0} + 9} \${${mid} + 4} Z\` : \`M \${${dx0}} \${${mid} - 4} L \${${dx0} + 9} \${${mid} - 4} L \${${dx0} + 4.5} \${${mid} + 4} Z\``;
    const deltaRow = hasDelta ? group({
      key: "delta-row",
      size: { h: e(`${body} * 1.5`) },
      when: e(`${c} != null && ${change} != null`),
      children: [
        shape(geom.path(e(tri)), { key: "arrow", when: e(`${diff} != 0`), fill: e(ink), semantics: { role: "decoration" } }),
        // Keyed by its format: the change counts from one value to the next, but a change of
        // format (a share to an amount) is a new line, not a count between the two.
        text(deltaCounts ? "" : e(deltaText), [e(`${dx0} + 13`), e(mid)], { key: `delta ${deltaFmt}`, number: deltaCounts ? { value: e(change), format: deltaFmt } : undefined, style: { size: "$size.body", weight: 600, ink: e(ink), baseline: "middle" } }),
        p.compareLabel ? text(p.compareLabel, [e(`${dx0} + 18 + ${dw}`), e(mid)], { key: "against", style: { size: "$size.body", ink: "$muted", baseline: "middle" } }) : null,
      ],
    }) : null;
    return group({
      key: "kpi",
      layout: { type: "rows", gap: 4 },
      semantics: { role: "group", label: p.label || "Key figure" },
      children: [
        p.label ? text(p.label, [center ? e("box.w / 2") : 0, 0], { key: "label", size: { h: "auto" }, style: { size: "$size.body", weight: 600, ink: "$ink-2", baseline: "top", align: center ? "middle" : "start", maxWidth: e("box.w") } }) : null,
        valueRow,
        deltaRow,
        spark && rows ? group({ key: "trend", size: { h: "fill" }, children: [sparkline({ data: rows, x: p.x, y: p.y, trend: false, stroke: "$mark", format: p.format, name: p.label || p.y })] }) : null,
      ],
    });
  },
});

// ---- bullet ---------------------------------------------------------------------------------------

export interface BulletParams { data: string; label: string; value: string; target: string; bands: unknown; sublabel: string; max: Prop; shared: boolean; format: string; ink: string }

/** Band inks, darkest (the worst range) first: ink over paper, so they read in dark mode too. */
const BAND_ALPHA = [0.3, 0.19, 0.11, 0.06, 0.035];

export const bullet = recipe<BulletParams>({
  id: "@datars/std/bullet",
  doc: "Bullet graphs: per row the actual value as a bar over qualitative bands (poor, fair, good — darkest first) with the target as a tick across it, each on its own scale with its own ticks (or one scale for all with `shared`). A compact replacement for a gauge: many measures in the height of one.",
  params: {
    data: t.table("One row per measure."), label: t.field("The measure's name (and the rows' order)."),
    value: t.field("The actual value."), target: t.field("The target (optional): a tick across the bar."),
    bands: t.json("The qualitative ranges' upper bounds, ascending: field names (each row its own, `[\"poor\", \"fair\", \"good\"]`) or numbers (the same for every row, `[150, 225, 300]`)."),
    sublabel: t.field("A second, smaller line under the name (its unit: `US$, thousands`)."),
    max: t.prop("Where the scale ends (a number): default each row's largest value, target or band (rounded up to a nice number without bands)."),
    shared: t.bool(false, "One scale for every row (the largest of them all), ticked under the last: measures in the same unit compare across rows."),
    format: t.string(",.4~r", "Tick and label number format."),
    ink: t.ink("$mark", "The value bar's ink."),
  },
  tokens: ["mark", "ink", "ink-2", "muted", "size.body", "size.small"],
  expand(p, cx) {
    const bands = Array.isArray(p.bands) ? (p.bands as unknown[]) : [];
    const bandSrc = bands.map((b) => (typeof b === "number" ? String(b) : `(${fld(String(b))} ?? 0)`));
    const v = fld(p.value), tg = p.target ? fld(p.target) : null;
    const parts = [`(${v} ?? 0)`, ...(tg ? [`(${tg} ?? 0)`] : []), ...bandSrc];
    const fixed = p.max !== undefined && p.max !== null ? vsrc(p.max) : null;
    const nice = !bands.length && !fixed;
    // The scale's end and a nice tick step per row (or for all rows, `shared`), computed once as
    // columns; the ticks are rows of their own (`units`), so each row's axis reads its own numbers.
    const ops: Op[] = [
      op.derive("__one", 1),
      op.window("cumsum", "__one", "__pos"),
      op.derive("__m", e(fixed ?? `max(${parts.join(", ")}, 0)`)),
    ];
    if (p.shared && !fixed) ops.push(op.window("cummax", "__m", "__cm"), op.window("last", "__cm", "__m2"), op.derive("__m", e("d.__m2")));
    ops.push(
      op.derive("__p", e("pow(10, floor(log10(max(d.__m, 0.000001) / 4)))")),
      op.derive("__r", e("d.__m / 4 / d.__p")),
      op.derive("__step", e("(d.__r <= 1 ? 1 : d.__r <= 2 ? 2 : d.__r <= 2.5 ? 2.5 : d.__r <= 5 ? 5 : 10) * d.__p")),
      op.derive("__max", e(nice ? "max(d.__step, ceil(d.__m / d.__step - 0.000001) * d.__step)" : "max(d.__m, 0.000001)")),
      op.derive("__n", e("floor(d.__max / d.__step + 0.000001) + 1")),
      op.derive("__lw", e(`max(measure(key.name(${fld(p.label)}), token("size.body"), 600), ${p.sublabel ? `measure(String(${fld(p.sublabel)} ?? ""), token("size.small"))` : "0"})`)),
    );
    const T = cx.table("bullet", p.data, ...ops);
    const ticks = cx.table("bullet-ticks", T, op.units({ value: "__n" }), ...(p.shared ? [op.filter(e(`d.__pos == table.count(${q(T)})`))] : []));
    const by = `scale.by(${fld(p.label)})`, bw = "scale.by.bandwidth()";
    const X = (x: string) => `clamp((${x}) / d.__max, 0, 1) * box.w`;
    const bandRects = (bands.length ? bandSrc : ["d.__max"]).map((b, i) => {
      const lo = i === 0 ? "0" : bandSrc[i - 1];
      return shape(geom.rect({ x: e(X(lo)), y: e(by), w: e(`max(0, ${X(b)} - ${X(lo)})`), h: e(bw) }), { key: `band-${i}`, fill: `$ink@${bands.length ? BAND_ALPHA[Math.min(i, BAND_ALPHA.length - 1)] : 0.07}`, semantics: { role: "decoration" } });
    });
    const name = `key.name(${fld(p.label)})`;
    const said = `${name} + ": " + format(${v}, ${q(p.format)})` + (tg ? ` + (${tg} == null ? "" : " (target " + format(${tg}, ${q(p.format)}) + ")")` : "");
    const lw = `min(box.w * 0.42, table.max(${q(T)}, "__lw"))`;
    return group({
      key: "bullet",
      scales: { by: { type: "band", domain: { data: T, field: p.label }, range: { box: "bullet-area", axis: "y" }, padding: 0.5 } },
      layout: { type: "columns", gap: 12 },
      semantics: { role: "group", label: "Bullet graph" },
      children: [
        // Names right-aligned against their bullets, as wide as the widest (up to 42 % of the box).
        group({ key: "labels", size: { w: e(lw) }, children: [repeat(T, group({ children: [
          text(e(name), [e("box.w"), e(p.sublabel ? `${by} + ${bw} / 2 - 1` : `${by} + ${bw} / 2`)], { key: "name", style: { size: "$size.body", weight: 600, ink: "$ink", align: "end", baseline: p.sublabel ? "bottom" : "middle" } }),
          p.sublabel ? text(e(`String(${fld(p.sublabel)} ?? "")`), [e("box.w"), e(`${by} + ${bw} / 2 + 1`)], { key: "sub", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "top" } }) : null,
        ] }))] }),
        group({ id: "bullet-area", key: "area", children: [
          repeat(T, group({ children: [
            ...bandRects,
            shape(geom.rect({ x: 0, y: e(`${by} + ${bw} / 3`), w: e(X(`${v} ?? 0`)), h: e(`${bw} / 3`) }), { key: "value", fill: p.ink, semantics: { role: "datum", label: e(said), value: e(v) }, pickable: true }),
            tg ? shape(geom.segment({ x1: e(X(tg)), y1: e(`${by} + ${bw} * 0.16`), x2: e(X(tg)), y2: e(`${by} + ${bw} * 0.84`) }), { key: "target", when: e(`${tg} != null`), stroke: { paint: "$ink", width: 2.5 }, semantics: { role: "decoration" } }) : null,
          ] })),
          group({ key: "ticks", children: [repeat(ticks, group({ when: e("d.__unit * d.__step <= d.__max * 1.000001"), children: [
            shape(geom.segment({ x1: e(X("d.__unit * d.__step")), y1: e(`${by} + ${bw}`), x2: e(X("d.__unit * d.__step")), y2: e(`${by} + ${bw} + 3`) }), { key: "tick", stroke: { paint: "$muted", width: 1 } }),
            text(e(`format(d.__unit * d.__step, ${q(p.format)})`), [e(X("d.__unit * d.__step")), e(`${by} + ${bw} + 4`)], { key: "tick-label", style: { size: "$size.small", ink: "$muted", baseline: "top", align: e(`d.__unit == 0 ? "start" : d.__unit * d.__step >= d.__max * 0.999 ? "end" : "middle"`) } }),
          ] }))] }),
        ] }),
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "left" } }],
});

// ---- gauge ----------------------------------------------------------------------------------------

export interface GaugeParams { value: Prop; min: number; max: number; label: string; format: string; prefix: string; suffix: string; bands: number[]; inks: string[]; needle: boolean; arc: number; ink: string; thickness: number }

export const gauge = recipe<GaugeParams>({
  id: "@datars/std/gauge",
  doc: "A dial: an arc from `min` to `max` filled up to the value — or, with `bands`, coloured ranges and a needle — with the value in the middle, the ends labelled and the label under it. The needle turns (and the fill grows) when the value changes. For many measures at once, `bullet` says more in less room.",
  params: {
    value: t.prop("The value: a number or an expression (a signal, `table.last(…)`)."),
    min: t.number(0, "The arc's start."), max: t.number(100, "The arc's end."),
    label: t.string(undefined, "What it measures, under the dial."),
    format: t.string(",.0f", "Number format of the value and the ends."),
    prefix: t.string(undefined, "Before the value (`$`)."), suffix: t.string(undefined, "After it (`%`, ` km/h`)."),
    bands: t.json("Upper bounds of coloured ranges along the arc, ascending (`[60, 85, 100]`); with bands the value shows as a needle.") as never,
    inks: t.json("The bands' inks, in order (default: from light to dark `$ink`; status inks like `[\"$positive\", \"$highlight\", \"$negative\"]` read as good to bad).") as never,
    needle: t.bool(undefined, "A needle instead of a filled arc (default: with bands)."),
    arc: t.number(240, "The dial's sweep in degrees (180: a half circle; up to 300)."),
    ink: t.ink("$mark", "The fill's ink (without bands)."),
    thickness: t.number(0.22, "The arc's thickness, as a share of its radius."),
  },
  tokens: ["mark", "ink", "ink-2", "muted", "paper", "size.label", "size.small"],
  expand(p) {
    const sweep = Math.min(300, Math.max(60, p.arc || 240)) * Math.PI / 180;
    const half = sweep / 2;
    const [lo, hi] = [p.min ?? 0, p.max ?? 100];
    const v = vsrc(p.value, "null");
    const frac = `clamp((${v} - ${lo}) / ${hi - lo || 1}, 0, 1)`;
    const ang = (f: string) => `(${-half} + (${f}) * ${sweep})`;
    // The dial's extent in radii: across (a sweep past a half circle is a full width), and up and
    // down from the centre — to the top, and to the arc's ends when they hang below the centre.
    const wUnits = sweep >= Math.PI ? 2 : 2 * Math.sin(half);
    const below = Math.max(0, -Math.cos(half));
    const ends = 'token("size.small") + 8';
    const needle = p.needle ?? (Array.isArray(p.bands) && p.bands.length > 0);
    // With a needle the value reads under the hub (below the centre line of a half circle), so
    // the dial leaves room for it; filled, it sits in the arc's hole.
    const vTop = sweep > Math.PI ? 0.3 : 0.12, vS = needle ? 0.26 : 0.42;
    const extent = needle ? Math.max(below, vTop + vS * 1.1) : below;
    // The label's line under the dial; dial and label centred together in the box.
    const lab = p.label ? 'token("size.body") * 1.3 + 4' : "0";
    const R = `max(8, min(box.w / ${wUnits.toFixed(4)}, (box.h - (${ends}) - (${lab})) / ${(1 + extent).toFixed(4)}) - 2)`;
    const cy = `((box.h - (${ends}) - (${lab}) - ${R} * ${(1 + extent).toFixed(4)}) / 2 + ${R})`;
    const cxs = "box.w / 2";
    const th = Math.min(0.6, Math.max(0.05, p.thickness ?? 0.22));
    const r0 = `${R} * ${1 - th}`;
    const bands = Array.isArray(p.bands) ? p.bands : [];
    const inks = Array.isArray(p.inks) ? p.inks : [];
    const arc = (key: string, f0: string, f1: string, fill: string, extra: Record<string, unknown> = {}) =>
      shape(geom.arc({ cx: e(cxs), cy: e(cy), r0: e(r0), r1: e(R), a0: e(ang(f0)), a1: e(ang(f1)) }), { key, fill, semantics: { role: "decoration" }, ...extra });
    const bandArcs = bands.map((b, i) => {
      const f0 = i === 0 ? "0" : String((bands[i - 1] - lo) / (hi - lo || 1));
      const ink = inks[i] ?? `$ink@${[0.12, 0.22, 0.34, 0.46, 0.58][Math.min(i, 4)]}`;
      return arc(`band-${i}`, `clamp(${f0}, 0, 1)`, `clamp(${(b - lo) / (hi - lo || 1)}, 0, 1)`, ink);
    });
    const valueText = affixed(v, p.format, p.prefix, p.suffix);
    // A plain number counts when it changes; with a prefix or suffix it's formatted text.
    const counting = !(p.prefix || p.suffix);
    const said = `${q(p.label ? `${p.label}: ` : "")} + ${valueText} + ${q(` (${lo} to ${hi})`)}`;
    // The needle: a thin wedge from the hub to near the rim, turned in two halves (each under
    // 180°) so a large change swings the long way round the dial, as a needle does.
    const deg = `(${ang(frac)} * ${180 / Math.PI} / 2)`;
    const needleNode = group({ key: "needle", transform: { translate: [e(cxs), e(cy)], rotate: e(deg) }, children: [group({ key: "turn", transform: { rotate: e(deg) }, children: [
      shape(geom.path(e(`\`M -3 0 L 0 \${-(${R}) * 0.92} L 3 0 Z\``)), { key: "hand", fill: "$ink", semantics: { role: "decoration" } }),
      shape(geom.circle({ cx: 0, cy: 0, r: 5 }), { key: "hub", fill: "$ink", stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "decoration" } }),
    ] })] });
    // As big as the dial allows, but never wider than the hole (or the room under the hub).
    const vSize = `clamp(min(${R} * ${vS}, ${needle ? `${R} * 1.3` : `${r0} * 1.5`} * 10 / max(1, measure(${counting ? `format(${v}, ${q(p.format)})` : valueText}, 10, 600))), 11, 56)`;
    // The value sits in the middle of the dial; with a needle, under its hub.
    const vy = needle ? `${cy} + ${R} * ${vTop}` : `${cy} + ${sweep > Math.PI ? `(${vSize}) * 0.36` : `-(${vSize}) * 0.12`}`;
    const endAt = (f: number) => [e(`${cxs} + (${R} * ${1 - th / 2}) * sin(${ang(String(f))})`), e(`${cy} - (${R} * ${1 - th / 2}) * cos(${ang(String(f))}) + ${R} * ${th / 2} + 4`)] as [Prop, Prop];
    return group({
      key: "gauge",
      semantics: { role: "group", label: p.label || "Gauge" },
      children: [
        group({ key: "dial", children: [
          arc("track", "0", "1", bands.length ? "$ink@0.06" : "$ink@0.1"),
          ...bandArcs,
          needle ? null : arc("fill", "0", frac, p.ink, { semantics: { role: "datum", label: e(said), value: e(v) }, pickable: true }),
          needle ? needleNode : null,
          text(counting ? "" : e(valueText), [e(cxs), e(vy)], { key: "value", number: counting ? { value: e(v), format: p.format } : undefined, style: { size: e(vSize), weight: 600, ink: "$ink", align: "middle", baseline: needle ? "top" : "alphabetic" }, semantics: needle ? { role: "datum", label: e(said), value: e(v) } : undefined, pickable: needle || undefined }),
          text(e(`format(${lo}, ${q(p.format)})`), endAt(0), { key: "min", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
          text(e(`format(${hi}, ${q(p.format)})`), endAt(1), { key: "max", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
        ] }),
        p.label ? text(p.label, [e("box.w / 2"), e(`${cy} + max(${R} * ${extent.toFixed(4)} + 6, ${R} * ${(below + th / 2).toFixed(4)} + ${ends})`)], { key: "label", style: { size: "$size.body", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e("box.w") } }) : null,
      ],
    });
  },
});

// ---- progress -------------------------------------------------------------------------------------

export interface ProgressParams { value: Prop; goal: Prop; label: string; shape: "bar" | "ring"; format: string; prefix: string; suffix: string; showGoal: boolean; ink: string; thickness: number }

export const progress = recipe<ProgressParams>({
  id: "@datars/std/progress",
  doc: "Progress toward a goal: a bar (label and share above it) or a ring (the share in the middle), filled to value ÷ goal — past the goal it stays full and the share says how far past. The fill grows when the value changes.",
  params: {
    value: t.prop("How far along: a number or an expression."),
    goal: t.prop("The goal (default 1: the value is a share)."),
    label: t.string(undefined, "What's progressing."),
    shape: t.oneOf(["bar", "ring"] as const, "bar", "A bar across the box, or a ring."),
    format: t.string(",.0f", "Number format of the value and the goal (with `showGoal`)."),
    prefix: t.string(undefined, "Before the value and goal (`$`)."), suffix: t.string(undefined, "After them."),
    showGoal: t.bool(false, "Say `value of goal` too (`7,200 of 10,000`)."),
    ink: t.ink("$mark", "The fill's ink."),
    thickness: t.number(0, "The bar's height or the ring's width in px (0: 8 for a bar, a fifth of the ring's radius)."),
  },
  tokens: ["mark", "ink", "ink-2", "muted", "size.body", "size.label"],
  expand(p) {
    const v = vsrc(p.value, "0");
    const g = p.goal === undefined || p.goal === null ? "1" : vsrc(p.goal, "1");
    const share = `((${g}) != 0 ? (${v}) / (${g}) : 0)`;
    const f = `clamp(${share}, 0, 1)`;
    const pct = `format(${share}, ".0%")`;
    const of = `${affixed(v, p.format, p.prefix, p.suffix)} + " of " + ${affixed(g, p.format, p.prefix, p.suffix)}`;
    const said = `${q(p.label ? `${p.label}: ` : "")} + ${pct}` + (p.showGoal ? ` + " (" + ${of} + ")"` : "");
    const sem = { role: "datum" as const, label: e(said), value: e(share) };
    if (p.shape === "ring") {
      const R = "max(6, min(box.w, box.h) / 2 - 2)";
      const w = p.thickness ? String(p.thickness) : `max(4, ${R} * 0.2)`;
      const ring = (key: string, a1: string, fill: string, extra: Record<string, unknown> = {}) => shape(geom.arc({ cx: e("box.w / 2"), cy: e("box.h / 2"), r0: e(`${R} - ${w}`), r1: e(R), a0: 0, a1: e(a1) }), { key, fill, ...extra });
      return group({ key: "progress", layout: { type: "rows", gap: 6 }, semantics: { role: "group", label: p.label || "Progress" }, children: [
        group({ key: "ring", size: { h: "fill" }, children: [
          ring("track", String(2 * Math.PI), "$ink@0.1", { semantics: { role: "decoration" } }),
          ring("fill", `${f} * ${2 * Math.PI}`, p.ink, { semantics: sem, pickable: true }),
          text("", [e("box.w / 2"), e("box.h / 2")], { key: "share", number: { value: e(share), format: ".0%" }, style: { size: e(`clamp((${R} - ${w}) * 0.46, 11, 34)`), weight: 600, ink: "$ink", align: "middle", baseline: "middle" } }),
        ] }),
        p.label ? text(p.label, [e("box.w / 2"), 0], { key: "label", size: { h: "auto" }, style: { size: "$size.body", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e("box.w") } }) : null,
        p.showGoal ? text(e(of), [e("box.w / 2"), 0], { key: "goal", size: { h: "auto" }, style: { size: "$size.label", ink: "$muted", align: "middle", baseline: "top", maxWidth: e("box.w") } }) : null,
      ] });
    }
    const h = p.thickness || 8;
    return group({ key: "progress", layout: { type: "rows", gap: 6 }, semantics: { role: "group", label: p.label || "Progress" }, children: [
      group({ key: "head", size: { h: e('token("size.body") * 1.3') }, children: [
        p.label ? text(p.label, [0, e("box.h / 2")], { key: "label", style: { size: "$size.body", weight: 600, ink: "$ink-2", baseline: "middle", maxWidth: e("box.w * 0.7") } }) : null,
        text("", [e("box.w"), e("box.h / 2")], { key: "share", number: { value: e(share), format: ".0%" }, style: { size: "$size.body", weight: 600, ink: "$ink", align: "end", baseline: "middle" } }),
      ] }),
      group({ key: "bar", size: { h }, children: [
        shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h, r: h / 2 }), { key: "track", fill: "$ink@0.1", semantics: { role: "decoration" } }),
        shape(geom.rect({ x: 0, y: 0, w: e(`max(${f} > 0 ? ${h} : 0, ${f} * box.w)`), h, r: h / 2 }), { key: "fill", fill: p.ink, semantics: sem, pickable: true }),
      ] }),
      p.showGoal ? text(e(of), [0, 0], { key: "goal", size: { h: "auto" }, style: { size: "$size.label", ink: "$muted", baseline: "top" } }) : null,
    ] });
  },
});

// ---- radar ----------------------------------------------------------------------------------------

export interface RadarParams { data: string; axis: string; value: string; series: string; max: Prop; levels: number; format: string; fill: boolean; points: boolean; legend: boolean }

export const radar = recipe<RadarParams>({
  id: "@datars/std/radar",
  doc: "A radar (spider) chart: one spoke per dimension, in the order they first appear, and each series a closed shape through its values, on one radial scale from zero with rings at even steps. Long rows (series, dimension, value); series are keyed, so a changed value reshapes its polygon and a new series grows in. Best for a few series over five to ten dimensions.",
  params: {
    data: t.table("Long rows: one per series and dimension."), axis: t.field("The dimension (a spoke each)."), value: t.field("The value (distance from the centre)."),
    series: t.field("One shape per series (optional), coloured by the categorical palette."),
    max: t.prop("The scale's end (a number: `5` for ratings out of 5; default: the largest value, rounded up to a ring)."),
    levels: t.number(4, "About how many rings, at nice steps from the centre to the rim, each labelled with its value."),
    format: t.string(",.4~r", "Number format of the rings and the tooltips."),
    fill: t.bool(true, "A soft fill inside each shape."),
    points: t.bool(true, "A dot at each value (hover it for the value)."),
    legend: t.bool(true, "Name the series below the chart (with `series`)."),
  },
  tokens: ["grid", "ink-2", "muted", "mark", "categorical", "paper", "size.label", "size.small"],
  expand(p, cx) {
    const A = fld(p.axis), V = fld(p.value);
    const axes = cx.table("radar-axes", p.data, op.aggregate([p.axis], { __rows: ["count"] }), op.derive("__one", 1), op.window("cumsum", "__one", "__ai"), op.derive("__lw", e(`measure(key.name(${A}), token("size.label"))`)));
    const levels = Math.max(1, Math.round(p.levels ?? 4));
    const fixed = p.max !== undefined && p.max !== null ? vsrc(p.max) : null;
    // The scale's end and a nice ring step (about `levels` rings), as a one-row table; the rings
    // are rows of their own — one per spoke and ring — each ring's polygon one group of them.
    const stats = cx.table("radar-scale", p.data, op.aggregate([], { __m: ["max", p.value] }), op.derive("__one", 1),
      ...(fixed ? [op.derive("__m", e(fixed))] : []),
      op.derive("__p", e(`pow(10, floor(log10(max(d.__m, 0.000001) / ${levels})))`)),
      op.derive("__r", e(`d.__m / ${levels} / d.__p`)),
      op.derive("__step", e("(d.__r <= 1 ? 1 : d.__r <= 2 ? 2 : d.__r <= 2.5 ? 2.5 : d.__r <= 5 ? 5 : 10) * d.__p")),
      op.derive("__max", e(fixed ? "max(d.__m, 0.000001)" : "max(d.__step, ceil(d.__m / d.__step - 0.000001) * d.__step)")),
      op.derive("__n", e("ceil(d.__max / d.__step - 0.000001)")));
    const rings = cx.table("radar-rings", axes, op.join(stats, "__one"), op.units({ value: "__n" }), op.derive("__f", e("min(1, (d.__unit + 1) * d.__step / d.__max)")));
    // Rows in spoke order (each series' shape goes round the spokes once).
    const rows = cx.table("radar", p.data, op.join(axes, p.axis), op.sort("__ai"));
    const R = `max(10, min(box.h / 2 - token("size.label") - 8, box.w / 2 - table.max(${q(axes)}, "__lw") - 12))`;
    const at = (r: string, a: string) => [e(`box.w / 2 + (${r}) * sin(${a})`), e(`box.h / 2 - (${r}) * cos(${a})`)] as [Prop, Prop];
    const ang = (x: string) => `scale.ang(${x})`;
    const xy = (r: string, a: string) => ({ x: e(`box.w / 2 + (${r}) * sin(${a})`), y: e(`box.h / 2 - (${r}) * cos(${a})`) });
    const series = p.series;
    const ink = series ? e(`scale.color(${fld(series)})`) : "$mark";
    const name = series ? `key.name(${fld(series)}) + " · " + ` : "";
    const chart = group({
      key: "radar",
      size: { h: "fill" },
      scales: {
        ang: { type: "band", domain: { data: axes, field: p.axis }, range: [0, 2 * Math.PI], padding: 0 },
        r: { type: "linear", domain: { data: stats, field: "__max" }, range: [0, `=${R}`], zero: true, nice: false },
      },
      children: [
        // Rings at even steps of the scale, each labelled on the first spoke.
        group({ key: "rings", children: [repeat({ groups: rings, by: "__unit" }, group({ children: [
          shape(geom.polyline({ from: "@group", ...xy("d.__f * scale.r.max()", ang(A)), curve: "linear", closed: true }), { key: "ring", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
          text(e(`format(d.__f * scale.r.invert(scale.r.max()), ${q(p.format)})`), [e("box.w / 2 + 4"), e("box.h / 2 - d.__f * scale.r.max() + 2")], { key: "ring-label", style: { size: "$size.small", ink: "$muted", baseline: "top" }, halo: ["$paper", 2] }),
        ] }))] }),
        group({ key: "spokes", children: [repeat(axes, group({ children: [
          shape(geom.segment({ x1: e("box.w / 2"), y1: e("box.h / 2"), x2: xy("scale.r.max()", ang(A)).x, y2: xy("scale.r.max()", ang(A)).y }), { key: "spoke", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
          text(e(`key.name(${A})`), at("scale.r.max() + 8", ang(A)), { key: "name", style: { size: "$size.label", ink: "$ink-2", align: e(`sin(${ang(A)}) > 0.2 ? "start" : sin(${ang(A)}) < -0.2 ? "end" : "middle"`), baseline: e(`cos(${ang(A)}) > 0.3 ? "bottom" : cos(${ang(A)}) < -0.3 ? "top" : "middle"`) } }),
        ] }))] }),
        group({ key: "shapes", children: [repeat({ groups: rows, by: series ?? "__all" }, group({ semantics: { role: "series", label: series ? e(`key.name(${fld(series)})`) : p.value }, children: [
          p.fill ? shape(geom.polyline({ from: "@group", ...xy(`scale.r(${V})`, ang(A)), curve: "linear", closed: true }), { key: "area", fill: ink, opacity: 0.14, semantics: { role: "decoration" } }) : null,
          shape(geom.polyline({ from: "@group", ...xy(`scale.r(${V})`, ang(A)), curve: "linear", closed: true }), { key: "outline", stroke: { paint: ink, width: 2, join: "round" }, semantics: { role: "decoration" } }),
          instances({ key: "points", from: "@group", ...xy(`scale.r(${V})`, ang(A)), instanceKey: e(A), r: p.points ? 3 : 0, fill: p.points ? ink : "transparent", stroke: p.points ? { paint: "$paper", width: 1 } : undefined, label: e(`${name}key.name(${A}) + ": " + format(${V}, ${q(p.format)})`) }),
        ] }))] }),
      ],
    });
    if (!series || !p.legend) return group({ key: "radar-chart", layout: { type: "rows" }, semantics: { role: "group", label: "Radar chart" }, children: [chart] });
    return group({
      key: "radar-chart",
      layout: { type: "rows", gap: 8 },
      scales: { color: { type: "categorical", domain: { data: p.data, field: series }, range: "$categorical" } },
      semantics: { role: "group", label: "Radar chart" },
      children: [chart, legend({ scale: "color" }, { size: { h: "auto" } })],
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { scale: 0, origin: "center" } }],
});

// ---- table ----------------------------------------------------------------------------------------

/** A column of a `table`. */
export interface TableColumn {
  /** The row's field it shows. */
  field?: string;
  /** The header (default: the field). */
  label?: string;
  /** `number` right-aligns and formats; default `number` with a format, bar or colour, else `text`. */
  type?: "text" | "number";
  /** d3-format of a number column (`,.0f`, `$,.2s`, `+.1%`). */
  format?: string; prefix?: string; suffix?: string;
  align?: "start" | "end" | "center";
  /** Width in px (default: as wide as the widest cell; text and bar columns share what's left). */
  width?: number;
  /** An inline bar after the number, from zero, on the column's range. */
  bar?: boolean;
  /** Cells coloured by value on the theme's sequential (or diverging, around 0) ramp. */
  color?: boolean | "sequential" | "diverging";
  /** Piecewise colour stops for coloured cells instead (`$negative -0.1 · $paper 0 · $positive 0.1`). */
  stops?: string;
  /** A sparkline of `y` over `x` from `data`, the rows matched on the table's `key` field. */
  spark?: { data: string; x: string; y: string };
  /** Left out when the table is too narrow for every column (a phone). */
  optional?: boolean;
}

export interface TableParams { data: string; columns: TableColumn[]; key: string; sort: string; descending: boolean; sortable: string; maxRows: number; striped: boolean; rowHeight: number }

type Col = TableColumn & { i: number; kind: "text" | "number" | "bar" | "color" | "spark"; head: string };

/** A cell's text as expression source: formatted numbers (a dash for none), names for text. */
function cellText(c: Col): string {
  const f = fld(c.field ?? "");
  if (c.kind === "text") return `(${f} == null ? "" : key.name(String(${f})))`;
  return c.prefix || c.suffix ? `(${f} == null ? "–" : ${affixed(f, c.format ?? ",.4~r", c.prefix, c.suffix)})` : `format(${f}, ${q(c.format ?? ",.4~r")})`;
}

export const dataTable = recipe<TableParams>({
  id: "@datars/std/dataTable",
  doc: "A data table drawn by the engine: column headers, text left and numbers right-aligned in their formats, optional inline bars, sparklines or coloured cells per column, striped rows, sorted by a column — or by whichever header the reader clicks (`sortable`). Rows are keyed, so a new sort slides them to their places and a filter lets rows leave and arrive. Columns are as wide as their widest cell; on a narrow screen `optional` columns step aside.",
  params: {
    data: t.table("The rows."),
    columns: t.json("The columns, in order: `{ field, label?, type?, format?, prefix?, suffix?, align?, width?, bar?, color?, stops?, spark?: { data, x, y }, optional? }`.") as never,
    key: t.field("The field that names a row (its key: rows morph by it). Needed for sparklines, which match their rows on it."),
    sort: t.field("Sort the rows by this field (default: as they come)."),
    descending: t.bool(true, "Numbers largest first (text always sorts A to Z)."),
    sortable: t.string(undefined, "A text signal holding the field to sort by: clicking a header sets it (declare it: `signal.str(\"revenue\")`)."),
    maxRows: t.number(0, "Show only the first rows after sorting (0: all)."),
    striped: t.bool(true, "Shade every other row (else a hairline between rows)."),
    rowHeight: t.number(0, "Row height in px (0: from the body text size)."),
  },
  tokens: ["ink", "ink-2", "muted", "surface", "grid", "rule", "mark", "up", "down", "sequential", "diverging", "size.body", "size.label"],
  expand(p, cx) {
    const cols: Col[] = (Array.isArray(p.columns) ? p.columns : []).map((c, i) => {
      const kind = c.spark ? "spark" : c.bar ? "bar" : c.color ? "color" : c.type === "number" || (c.type !== "text" && c.format) ? "number" : "text";
      return { ...c, i, kind, head: c.label ?? c.field ?? (c.spark ? c.spark.y : "") };
    });
    const G = 16, P = 8;
    const rowH = p.rowHeight ? String(p.rowHeight) : 'round(token("size.body") * 2.3)';
    const sortCols = cols.filter((c) => c.field && c.kind !== "spark");
    const dirOf = (c: Col | undefined, field: string) => (c && c.kind === "text") || !p.descending ? field : `-${field}`;
    // Row positions: sorted by a field, by whichever the `sortable` signal holds, or as they come.
    const order: Op[] = [op.derive("__one", 1)];
    if (p.sortable) {
      sortCols.forEach((c) => order.push(op.window("cumsum", "__one", `__p${c.i}`, { order: dirOf(c, c.field as string) })));
      const def = p.sort ? sortCols.find((c) => c.field === p.sort) : undefined;
      if (!def) order.push(op.window("cumsum", "__one", "__pin"));
      order.push(op.derive("__pos", e(sortCols.map((c) => `${p.sortable} == ${q(c.field as string)} ? d.__p${c.i} : `).join("") + (def ? `d.__p${def.i}` : "d.__pin"))));
    } else {
      order.push(op.window("cumsum", "__one", "__pos", p.sort ? { order: dirOf(cols.find((c) => c.field === p.sort), p.sort) } : {}));
    }
    // Each cell's measured width, for the columns' natural widths.
    const widths = cols.filter((c) => c.kind !== "spark").map((c) => op.derive(`__w${c.i}`, e(`measure(${cellText(c)}, token("size.body"))`)));
    const T1 = cx.table("table", p.data, ...order, ...(p.maxRows > 0 ? [op.filter(e(`d.__pos <= ${p.maxRows}`))] : []), ...widths);
    const T1q = q(T1);
    const head = (c: Col) => `measure(${q(c.head)}, token("size.label"), 600) + ${p.sortable && c.kind !== "spark" ? 14 : 0}`;
    const nat = (c: Col) => c.width ? String(c.width)
      : c.kind === "spark" ? "84"
        : c.kind === "bar" ? `max(${head(c)}, table.max(${T1q}, "__w${c.i}") + 6 + 56)`
          : `max(${head(c)}, table.max(${T1q}, "__w${c.i}") + ${c.kind === "color" ? 12 : 0})`;
    const flex = (c: Col) => !c.width && (c.kind === "text" || c.kind === "bar");
    const n = cols.length;
    const all = `(${cols.map(nat).join(" + ") || "0"} + ${G * Math.max(0, n - 1)})`;
    const fits = `(${all} <= box.w - ${2 * P})`;
    const vis = (c: Col) => (c.optional ? fits : "true");
    const count = `(${cols.map((c) => `(${vis(c)} ? 1 : 0)`).join(" + ") || "1"})`;
    const room = `(box.w - ${2 * P} - ${G} * max(0, ${count} - 1))`;
    const anyFlex = cols.some(flex);
    const fixedSum = `(${cols.filter((c) => !flex(c)).map((c) => `(${vis(c)} ? ${nat(c)} : 0)`).join(" + ") || "0"})`;
    const flexCount = `max(1, ${cols.filter(flex).map((c) => `(${vis(c)} ? 1 : 0)`).join(" + ") || "0"})`;
    // Text and bar columns share what the others leave: each starts from a base (a text column's
    // width up to 64 px, a bar column's numbers and a 40 px bar) and gets an equal part of the
    // rest — or gives one up when there's too little, down to 36 px of text (then it clips) or a
    // 24 px bar. With no text or bar column every column takes an equal share when it fits.
    const base = (c: Col) => (c.kind === "bar" ? `(table.max(${T1q}, "__w${c.i}") + 46)` : `min(${nat(c)}, 64)`);
    const bases = `(${cols.filter(flex).map((c) => `(${vis(c)} ? ${base(c)} : 0)`).join(" + ") || "0"})`;
    const extra = `((${room} - ${fixedSum} - ${bases}) / ${flexCount})`;
    const share = `(${room} / max(1, ${count}))`;
    const width = (c: Col) => (flex(c) ? `max(${c.kind === "bar" ? `table.max(${T1q}, "__w${c.i}") + 30` : "36"}, ${base(c)} + ${extra})` : anyFlex || c.width ? nat(c) : `max(${nat(c)}, ${share})`);
    const T2 = cx.table("table-rows", T1, ...cols.flatMap((c) => [op.derive(`__W${c.i}`, e(width(c))), op.derive(`__V${c.i}`, e(`${vis(c)} ? 1 : 0`))]));
    // Sparklines read one series table (the first spark column's), its x and each column's y
    // copied to columns of their own so the join can't rename them (the rows' own `revenue`).
    const sparks = cols.filter((c) => c.spark);
    const spark = sparks[0]?.spark;
    const series = spark ? cx.table("table-series", spark.data, op.derive("__sx", e(fld(spark.x))), ...sparks.map((c) => op.derive(`__sy${c.i}`, e(fld((c.spark as { y: string }).y))))) : undefined;
    const rowsFrom = series && p.key ? cx.table("table-spark", T2, op.join(series, p.key)) : T2;
    const T2q = q(T2);
    const mid = `(${rowH}) / 2`;
    const at = (c: Col, align: string) => [align === "end" ? e("box.w") : align === "center" ? e("box.w / 2") : 0, e(mid)] as [Prop, Prop];
    const alignOf = (c: Col) => c.align ?? (c.kind === "text" ? "start" : "end");
    const scales: Record<string, ScaleDecl> = {};
    const cell = (c: Col): Template => {
      const f = fld(c.field ?? "");
      const align = alignOf(c);
      const size = { w: e(`d.__W${c.i}`) };
      const when = c.optional ? e(`d.__V${c.i} == 1`) : undefined;
      const body = { size: "$size.body", ink: "$ink", baseline: "middle", align } as const;
      const num = (x: Prop[], style: Record<string, unknown>) => (c.prefix || c.suffix ? text(e(cellText(c)), x as [Prop, Prop], { key: "text", style }) : text("", x as [Prop, Prop], { key: "text", number: { value: e(f), format: c.format ?? ",.4~r" }, style }));
      if (c.kind === "text") return group({ key: `c${c.i}`, size, when, clip: "box", children: [text(e(cellText(c)), at(c, align), { key: "text", style: body })] });
      if (c.kind === "number") return group({ key: `c${c.i}`, size, when, children: [num(at(c, align), body)] });
      if (c.kind === "color") {
        scales[`c${c.i}`] = c.stops
          ? { type: "piecewise", stops: c.stops, domain: { data: T1, field: c.field } }
          : c.color === "diverging"
          ? { type: "diverging", domain: { data: T1, field: c.field }, range: "$diverging", mid: 0 }
          : { type: "sequential", domain: { data: T1, field: c.field }, range: "$sequential" };
        const fill = `(${f} == null ? "transparent" : scale.c${c.i}(${f}))`;
        return group({ key: `c${c.i}`, size, when, children: [
          shape(geom.rect({ x: -6, y: 2, w: e("box.w + 12"), h: e(`${rowH} - 4`), r: 2 }), { key: "fill", fill: e(fill), semantics: { role: "decoration" } }),
          num([align === "end" ? e("box.w") : align === "center" ? e("box.w / 2") : 0, e(mid)], { ...body, ink: e(`"on(" + ${fill} + ")"`) }),
        ] });
      }
      if (c.kind === "bar") {
        const nw = `table.max(${T1q}, "__w${c.i}")`;
        const [lo, hi] = [`min(0, table.min(${T1q}, ${q(c.field ?? "")}))`, `max(0, table.max(${T1q}, ${q(c.field ?? "")}))`];
        const X = (v: string) => `(${nw} + 6 + ((${v}) - ${lo}) / max(${hi} - ${lo}, 0.000001) * (box.w - ${nw} - 6))`;
        return group({ key: `c${c.i}`, size, when, children: [
          num([e(nw), e(mid)], { ...body, align: "end" }),
          shape(geom.rect({ x: e(`min(${X("0")}, ${X(`${f} ?? 0`)})`), y: e(`(${rowH}) * 0.28`), w: e(`abs(${X(`${f} ?? 0`)} - ${X("0")})`), h: e(`(${rowH}) * 0.44`), r: 1 }), { key: "bar", fill: "$mark", semantics: { role: "decoration" } }),
        ] });
      }
      // A sparkline over the row's own rows of the series table (`@group`).
      const sy = `__sy${c.i}`;
      const [first, last] = [`group.first(${q(sy)})`, `group.last(${q(sy)})`];
      const ink = e(`${last} >= ${first} ? "$up" : "$down"`);
      return group({
        key: `c${c.i}`, size, when,
        scales: {
          sx: { type: "point", domain: { data: "@group", field: "__sx" }, range: [2, "=box.w - 4"], padding: 0, nice: false },
          sy: { type: "linear", domain: { data: "@group", field: sy }, range: [`=${rowH} - 6`, 6], zero: false, nice: false },
        },
        children: [
          shape(geom.polyline({ from: "@group", x: e("scale.sx(d.__sx)"), y: e(`scale.sy(d.${sy})`), curve: "linear" }), { key: "line", stroke: { paint: ink, width: 1.5, join: "round", cap: "round" }, semantics: { role: "decoration" } }),
          shape(geom.circle({ cx: e("scale.sx.max()"), cy: e(`scale.sy(${last})`), r: 2 }), { key: "dot", when: e(`${last} != null`), fill: ink, semantics: { role: "decoration" } }),
        ],
      });
    };
    const said = cols.filter((c) => c.kind !== "spark").map((c) => `${q(`${c.head}: `)} + ${cellText(c)}`).join(` + ", " + `) || '""';
    // A row: the whole row is what the pointer finds (and washes under it), the cells in columns
    // over it. It moves as one when a sort gives it another place.
    const rowTemplate = group({
      key: p.key ? e(fld(p.key)) : undefined,
      transform: { translate: [0, e(`(d.__pos - 1) * (${rowH})`)] },
      children: [
        shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e(rowH) }), { key: "row", fill: e('hover() ? "$ink@0.05" : "$ink@0"'), semantics: { role: "datum", label: e(said) }, pickable: true }),
        group({ key: "cells", layout: { type: "columns", gap: G, padding: [0, P] }, children: cols.map(cell) }),
      ],
    });
    const headerCell = (c: Col) => {
      const align = alignOf(c);
      const sorted = p.sortable && c.field ? `${p.sortable} == ${q(c.field)}` : p.sort && c.field === p.sort ? "true" : "false";
      const arrow = c.kind === "text" || !p.descending ? "↑" : "↓";
      const label = sorted === "false" ? q(c.head) : `${q(c.head)} + (${sorted} ? ${q(align === "end" ? "" : ` ${arrow}`)} : "")`;
      const pre = align === "end" && sorted !== "false" ? `(${sorted} ? ${q(`${arrow} `)} : "") + ` : "";
      return group({
        key: `h${c.i}`,
        size: { w: e(`table.first(${T2q}, "__W${c.i}")`) },
        when: c.optional ? e(`table.first(${T2q}, "__V${c.i}") == 1`) : undefined,
        on: p.sortable && c.field && c.kind !== "spark" ? { activate: { set: p.sortable, value: c.field } } : undefined,
        pickable: p.sortable && c.field && c.kind !== "spark" ? true : undefined,
        semantics: p.sortable && c.field && c.kind !== "spark" ? { role: "control", label: `Sort by ${c.head}` } : undefined,
        children: [text(e(pre + label), [align === "end" ? e("box.w") : align === "center" ? e("box.w / 2") : 0, e(mid)], { key: "text", style: { size: "$size.label", weight: 600, ink: e(`${sorted} ? "$ink" : "$ink-2"`), baseline: "middle", align } })],
      });
    };
    const n_ = `table.count(${T2q})`;
    return group({
      key: "table",
      scales,
      layout: { type: "rows" },
      semantics: { role: "group", label: "Table" },
      children: [
        group({ key: "header", size: { h: e(rowH) }, layout: { type: "columns", gap: G, padding: [0, P] }, children: cols.map(headerCell) }),
        group({ key: "body", children: [
          shape(geom.segment({ x1: 0, y1: 0, x2: e("box.w"), y2: 0 }), { key: "header-rule", stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
          group({ key: "stripes", z: -1, children: [repeat({ count: e(n_) }, p.striped
            ? shape(geom.rect({ x: 0, y: e(`d.index * (${rowH})`), w: e("box.w"), h: e(rowH) }), { when: e("d.index % 2 == 1"), fill: "$surface", semantics: { role: "decoration" } })
            : shape(geom.segment({ x1: 0, y1: e(`(d.index + 1) * (${rowH})`), x2: e("box.w"), y2: e(`(d.index + 1) * (${rowH})`) }), { when: e(`d.index < ${n_} - 1`), stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }))] }),
          group({ key: "rows", children: [spark && p.key ? repeat({ groups: rowsFrom, by: p.key }, rowTemplate) : repeat(rowsFrom, rowTemplate)] }),
        ] }),
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { opacity: 0 }, exit: { opacity: 0 } }],
});

// ---- gantt ----------------------------------------------------------------------------------------

export interface GanttParams { data: string; task: string; start: string; end: string; group: string; milestone: string; progress: string; color: string; today: Prop; todayLabel: string; format: string }

export const gantt = recipe<GanttParams>({
  id: "@datars/std/gantt",
  doc: "A plan: one row per task, a bar from its start to its end on a time axis, tasks under their group's heading (with a thin bar spanning the group), milestones as diamonds, how far each task has got as a solid part of its bar, and a line at today. Rows keep the order they come in, groups the order they first appear; tasks are keyed by name, so a replanned task slides to its new dates.",
  params: {
    data: t.table("One row per task or milestone."), task: t.field("The task's name (its key: unique)."),
    start: t.field("Start date."), end: t.field("End date (none, or the start: a milestone)."),
    group: t.field("A phase or team: tasks gather under its heading, coloured by it."),
    milestone: t.field("A true/false field marking milestones (default: rows without an end, or ending when they start)."),
    progress: t.field("How far along, 0–1: that share of the bar is solid, the rest pale."),
    color: t.field("Colour bars by this field instead of the group (categorical)."),
    today: t.prop("Where today is: a date (`\"2026-03-16\"`) or an expression; a line across the plan."),
    todayLabel: t.string("Today", "The today line's label."),
    format: t.string("%-d %b", "Date format in tooltips (strftime)."),
  },
  tokens: ["ink", "ink-2", "muted", "mark", "accent", "paper", "categorical", "grid", "size.body", "size.label", "size.small"],
  expand(p, cx) {
    const [S, E] = [fld(p.start), fld(p.end)];
    const ms = p.milestone ? `(${fld(p.milestone)} == true)` : `(${E} == null || ${E} == ${S})`;
    const base = cx.table("gantt-base", p.data, op.derive("__s", e(S)), op.derive("__e", e(`${E} ?? ${S}`)));
    const byGroup = !!p.group;
    const tasks: Op[] = [
      op.derive("__id", e(`String(${fld(p.task)})`)), op.derive("__kind", e(`${ms} ? "milestone" : "task"`)),
      op.derive("__one", 1), op.window("cumsum", "__one", "__ti"), op.derive("__ord", 1),
    ];
    let T: string;
    if (byGroup) {
      // One heading row per group (first-appearance order), spanning its tasks, ahead of them.
      const groups = cx.table("gantt-groups", base, op.aggregate([p.group], { __gs: ["min", "__s"], __ge: ["max", "__e"] }), op.derive("__one", 1), op.window("cumsum", "__one", "__go"));
      const heads = cx.table("gantt-heads", groups, op.derive(p.task, e(`String(${fld(p.group)})`)), op.derive("__id", e(`"group:" + String(${fld(p.group)})`)), op.derive("__kind", "group"), op.derive("__ord", 0), op.derive("__ti", 0), op.derive("__s", e("d.__gs")), op.derive("__e", e("d.__ge")));
      T = cx.table("gantt", base, ...tasks, op.join(groups, p.group), op.union(heads), op.sort("__go", "__ord", "__ti"));
    } else {
      T = cx.table("gantt", base, ...tasks);
    }
    const indent = byGroup ? 12 : 0;
    const Tl = cx.table("gantt-labels", T, op.derive("__lw", e(`d.__kind == "group" ? measure(String(${fld(p.task)}), token("size.body"), 600) : measure(String(${fld(p.task)}), token("size.label")) + ${indent}`)));
    const colorField = p.color ?? p.group;
    const y = "scale.y(d.__id)", bw = "scale.y.bandwidth()";
    const x0 = "scale.x(d.__s)", x1 = "scale.x(d.__e)";
    const fill = colorField ? e(`scale.color(${fld(colorField)})`) : "$mark";
    const when = (fmt: string) => `formatDate(d.__s, ${q(fmt)}) + (d.__kind == "milestone" ? "" : " – " + formatDate(d.__e, ${q(fmt)}))`;
    const said = `String(${fld(p.task)}) + ": " + ${when(p.format)}` + (p.progress ? ` + (${fld(p.progress)} == null || d.__kind != "task" ? "" : " (" + format(${fld(p.progress)}, ".0%") + " done)")` : "");
    const pr = p.progress ? `clamp(${fld(p.progress)} ?? 0, 0, 1)` : null;
    const hh = `min(${bw} / 2, 8)`;
    const cyv = `${y} + ${bw} / 2`;
    const today = p.today !== undefined && p.today !== null ? vsrc(p.today) : null;
    const tx = today ? `scale.x(${today})` : "0";
    const scales: Record<string, ScaleDecl> = {
      x: { type: "time", domain: { data: T, fields: ["__s", "__e"] }, range: { box: "gantt-area", axis: "x" }, nice: true },
      y: { type: "band", domain: { data: T, field: "__id" }, range: { box: "gantt-area", axis: "y" }, padding: 0.32 },
    };
    if (colorField) scales.color = { type: "categorical", domain: { data: p.data, field: colorField }, range: "$categorical" };
    return group({
      key: "gantt",
      scales,
      layout: { type: "columns", gap: 12 },
      semantics: { role: "group", label: "Plan" },
      children: [
        // Names in a column as wide as the widest (up to a third of the box: longer ones clip).
        group({ key: "labels", size: { w: e(`min(box.w * 0.34, table.max(${q(Tl)}, "__lw"))`) }, clip: "box", children: [repeat(T, text(e(`String(${fld(p.task)})`), [e(`d.__kind == "group" ? 0 : ${indent}`), e(cyv)], {
          key: e("d.__id"),
          style: { size: e('d.__kind == "group" ? token("size.body") : token("size.label")'), weight: e('d.__kind == "group" ? 600 : 400'), ink: e('d.__kind == "group" ? "$ink" : "$ink-2"'), baseline: "middle" },
          semantics: { role: "decoration" },
        }))] }),
        group({ key: "center", layout: { type: "rows", gap: 6 }, children: [
          group({ id: "gantt-area", key: "area", children: [
            grid({ scale: "x", orient: "vertical" }),
            repeat(T, group({ key: e("d.__id"), children: [
              shape(geom.rect({ x: e(x0), y: e(`${cyv} - 2`), w: e(`max(1, ${x1} - ${x0})`), h: 4, r: 1 }), { key: "span", when: e('d.__kind == "group"'), fill: "$ink-2", opacity: 0.55, semantics: { role: "decoration" } }),
              shape(geom.rect({ x: e(x0), y: e(y), w: e(`max(2, ${x1} - ${x0})`), h: e(bw), r: 3 }), { key: "bar", when: e('d.__kind == "task"'), fill, opacity: pr ? 0.35 : undefined, semantics: { role: "datum", label: e(said) }, pickable: true }),
              pr ? shape(geom.rect({ x: e(x0), y: e(y), w: e(`max(0, ${x1} - ${x0}) * ${pr}`), h: e(bw), r: 3 }), { key: "done", when: e(`d.__kind == "task" && ${pr} > 0`), fill, semantics: { role: "decoration" } }) : null,
              shape(geom.path(e(`\`M \${${x0}} \${${cyv} - ${hh}} L \${${x0} + ${hh}} \${${cyv}} L \${${x0}} \${${cyv} + ${hh}} L \${${x0} - ${hh}} \${${cyv}} Z\``)), { key: "diamond", when: e('d.__kind == "milestone"'), fill: "$ink", stroke: { paint: "$paper", width: 1 }, semantics: { role: "datum", label: e(said) }, pickable: true }),
            ] })),
            today ? group({ key: "today", when: e(`${tx} >= 0 && ${tx} <= box.w`), semantics: { role: "annotation", label: `${p.todayLabel}` }, children: [
              shape(geom.segment({ x1: e(tx), y1: 0, x2: e(tx), y2: e("box.h") }), { key: "line", stroke: { paint: "$accent", width: 1.5, dash: [4, 3] } }),
              p.todayLabel ? text(p.todayLabel, [e(`${tx} + 4 + measure(${q(p.todayLabel)}, token("size.small"), 600) > box.w ? ${tx} - 4 : ${tx} + 4`), 0], { key: "label", style: { size: "$size.small", weight: 600, ink: "$accent", baseline: "top", align: e(`${tx} + 4 + measure(${q(p.todayLabel)}, token("size.small"), 600) > box.w ? "end" : "start"`) }, halo: ["$paper", 2] }) : null,
            ] }) : null,
          ] }),
          axis({ scale: "x", orient: "bottom", type: "time" }, { size: { h: "auto" } }),
        ] }),
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "left" } }],
});

// ---- timeline -------------------------------------------------------------------------------------

export interface TimelineParams { data: string; date: string; label: string; xType: "time" | "linear"; dateFormat: string; color: string; eras: string; eraStart: string; eraEnd: string; eraLabel: string; eraLevel: string; eraColor: string; eraSide: "below" | "above" }

export const timeline = recipe<TimelineParams>({
  id: "@datars/std/timeline",
  doc: "Events on a time axis: a dot per event and its date and label on a stem, packed into as few lanes above and below the line as keep every label clear of the others (an engine op, so it re-packs for a phone); labels that still don't fit the height are left to the dot's tooltip. Optional eras shade spans of the axis with their names, under it or over it, in rows: an era inside another stacks past it (phases, and the sprints in them), eras that overlap take rows of their own.",
  params: {
    data: t.table("One row per event."), date: t.field("When (a date, or a number such as a year with `xType: \"linear\"`)."), label: t.field("What happened (keep it short: one line)."),
    xType: t.oneOf(["time", "linear"] as const, "time", "`time` for dates, `linear` for numbers (years)."),
    dateFormat: t.string(undefined, "How the date reads above each label: strftime for dates (default `%Y`), d3-format for numbers (default `d`)."),
    color: t.field("Colour the dots by this field (categorical)."),
    eras: t.table("Spans along the axis (optional): one row each with a start, an end and a name."),
    eraStart: t.string("start", "The eras' start field."), eraEnd: t.string("end", "The eras' end field."), eraLabel: t.string("label", "The eras' name field."),
    eraLevel: t.string(undefined, "The eras' row field (optional): 0 next to the axis, 1 past it, and so on — quarters on one row and phases on the next, whatever they overlap. Default: packed, each era in the first row free over its span, so an era inside another sits a row past it."),
    eraColor: t.string(undefined, "Colour the eras by this field (categorical: a phase, a status, a team). Default: each era by its name."),
    eraSide: t.oneOf(["below", "above"] as const, "below", "Which side of the axis the eras sit on; the labels on that side start past them."),
  },
  tokens: ["ink", "ink-2", "muted", "rule", "paper", "categorical", "size.body", "size.small"],
  expand(p, cx) {
    const D = fld(p.date);
    const time = p.xType !== "linear";
    const dateOf = (v: string) => (time ? `formatDate(${v}, ${q(p.dateFormat ?? "%Y")})` : `format(${v}, ${q(p.dateFormat ?? "d")})`);
    const dateText = dateOf(D);
    const labelText = `String(${fld(p.label)} ?? "")`;
    // The axis spans every event and era.
    const ext = [cx.table("timeline-extent", p.data, op.aggregate([], { __t0: ["min", p.date], __t1: ["max", p.date] }))];
    if (p.eras) ext.push(cx.table("timeline-era-extent", p.eras, op.aggregate([], { __t0: ["min", p.eraStart], __t1: ["max", p.eraEnd] })));
    const extent = ext.length > 1 ? cx.table("timeline-span", ext[0], op.union(ext[1])) : ext[0];
    const body = 'token("size.body")', small = 'token("size.small")';
    const laneH = `(${small} * 1.3 + ${body} * 1.3 + 8)`;
    const stem = 12;
    const [es, ee] = [`scale.x(${fld(p.eraStart)})`, `scale.x(${fld(p.eraEnd)})`];
    // Eras are strips in rows beside the axis (clear of the dots), row 0 nearest it. Packed, an
    // era takes the first row free over its span, the longer first where two start together: an
    // era inside another is still going when the inner one starts, so the inner lands past it.
    const E = p.eras
      ? cx.table("timeline-eras", p.eras, ...(p.eraLevel
        ? [op.derive("__row", e(`max(0, floor(${fld(p.eraLevel)} ?? 0))`))]
        : [op.derive("__s", e(es)), op.derive("__e", e(ee)), op.sort(["__s", "asc"], ["__e", "desc"]), op.lanes({ start: e("d.__s"), end: e("d.__e"), as: "__row" })]))
      : null;
    const eraH = 16, eraPitch = 19, eraOff = 5;
    // The room the rows take beside the axis; the lanes on that side start past it.
    const eraGap = E ? `(${eraOff - eraPitch + eraH + 1} + (max(0, table.max(${q(E)}, "__row")) + 1) * ${eraPitch})` : "0";
    const over = p.eraSide === "above";
    const gapUp = E && over ? eraGap : "0", gapDown = E && !over ? eraGap : "0";
    // Labels as intervals from their stem (to its right, or to its left near the right edge),
    // packed into lanes that alternate above and below the axis — as many as the box holds.
    const L = cx.table("timeline", p.data,
      op.derive("__x", e(`scale.x(${D})`)),
      op.derive("__w", e(`max(measure(${labelText}, ${body}), measure(${dateText}, ${small})) + 4`)),
      op.derive("__a", e("d.__x + d.__w > box.w ? d.__x - d.__w : d.__x")),
      op.lanes({ start: e("d.__a"), end: e("d.__a + d.__w"), gap: 10, max: e(`max(1, floor((box.h - ${2 * stem + 4} - ${eraGap}) / ${laneH}))`) }));
    const Lq = q(L);
    const maxLane = `max(0, table.max(${Lq}, "lane"))`;
    const above = `(floor(${maxLane} / 2) + 1)`, below = `floor((${maxLane} + 1) / 2)`;
    const axisY = `((box.h - (${above} + ${below}) * ${laneH} - ${2 * stem} - ${eraGap}) / 2 + ${above} * ${laneH} + ${stem} + ${gapUp})`;
    const up = "(d.lane % 2 == 0)", level = "floor(d.lane / 2)";
    // The label block's edge nearest the axis, and its far edge.
    const edge = `(${up} ? ${axisY} - ${gapUp} - ${stem} - ${level} * ${laneH} : ${axisY} + ${gapDown} + ${stem} + ${level} * ${laneH})`;
    const block = `(${laneH} - 8)`;
    const flip = "(d.__x + d.__w > box.w)";
    const tx = `(${flip} ? d.__x - 4 : d.__x + 4)`;
    const align = e(`${flip} ? "end" : "start"`);
    const dot = p.color ? e(`scale.color(${fld(p.color)})`) : "$ink";
    const scales: Record<string, ScaleDecl> = { x: { type: time ? "time" : "linear", domain: { data: extent, fields: ["__t0", "__t1"] }, range: "width", nice: false } };
    if (p.color) scales.color = { type: "categorical", domain: { data: p.data, field: p.color }, range: "$categorical" };
    if (p.eras) scales.era = { type: "categorical", domain: { data: p.eras, field: p.eraColor ?? p.eraLabel }, range: "$categorical" };
    const eraName = `String(${fld(p.eraLabel)} ?? "")`;
    // An era's row: strips go down from under the axis, or up from over it.
    const eraY = over ? `(${axisY} - ${eraOff + eraH} - d.__row * ${eraPitch})` : `(${axisY} + ${eraOff} + d.__row * ${eraPitch})`;
    const placed = e("d.lane != null");
    // Drawn in layers — stems, then eras and labels on paper, then dots — so a stem that climbs
    // past an era or a lower label runs behind it, never through its text.
    return group({
      key: "timeline",
      scales,
      semantics: { role: "group", label: "Timeline" },
      children: [
        shape(geom.segment({ x1: 0, y1: e(axisY), x2: e("box.w"), y2: e(axisY) }), { key: "axis", stroke: { paint: "$rule", width: 1.5 }, semantics: { role: "decoration" } }),
        group({ key: "stems", children: [repeat(L, shape(geom.segment({ x1: e("d.__x"), y1: e(axisY), x2: e("d.__x"), y2: e(`${edge} + (${up} ? 2 : -2)`) }), { when: placed, stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }))] }),
        // Eras on paper over the stems: a stem runs from its dot behind any era in its way. An era
        // is hovered for its name and span; the name inside it only shows where it fits.
        E ? group({ key: "eras", children: [repeat(E, group({ semantics: { role: "annotation", label: e(`${eraName} + ": " + ${dateOf(fld(p.eraStart))} + " – " + ${dateOf(fld(p.eraEnd))}`) }, children: [
          shape(geom.rect({ x: e(es), y: e(eraY), w: e(`max(1, ${ee} - ${es})`), h: eraH, r: 2 }), { key: "paper", fill: "$paper", semantics: { role: "decoration" } }),
          shape(geom.rect({ x: e(es), y: e(eraY), w: e(`max(1, ${ee} - ${es})`), h: eraH, r: 2 }), { key: "span", fill: e(`scale.era(${fld(p.eraColor ?? p.eraLabel)})`), opacity: e("hover() ? 0.45 : 0.3"), pickable: true }),
          text(e(eraName), [e(`${es} + 5`), e(`${eraY} + ${eraH / 2}`)], { key: "name", when: e(`measure(${eraName}, ${small}, 600) + 10 <= ${ee} - ${es}`), style: { size: "$size.small", weight: 600, ink: "$ink-2", baseline: "middle" } }),
        ] }))] }) : null,
        group({ key: "labels", children: [repeat(L, group({ when: placed, children: [
          shape(geom.rect({ x: e(`${flip} ? d.__x - d.__w : d.__x + 1`), y: e(`${up} ? ${edge} - ${block} : ${edge}`), w: e("d.__w - 1"), h: e(block) }), { key: "paper", fill: "$paper", semantics: { role: "decoration" } }),
          text(e(dateText), [e(tx), e(`${up} ? ${edge} - ${body} * 1.3 : ${edge}`)], { key: "date", style: { size: "$size.small", weight: 600, ink: "$muted", align, baseline: e(`${up} ? "bottom" : "top"`) } }),
          text(e(labelText), [e(tx), e(`${up} ? ${edge} : ${edge} + ${small} * 1.3`)], { key: "label", style: { size: "$size.body", ink: "$ink", align, baseline: e(`${up} ? "bottom" : "top"`) } }),
        ] }))] }),
        group({ key: "dots", children: [repeat(L, shape(geom.circle({ cx: e("d.__x"), cy: e(axisY), r: 4.5 }), { fill: dot, stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "datum", label: e(`${dateText} + ": " + ${labelText}`) }, pickable: true }))] }),
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }],
});

// ---- tileMap --------------------------------------------------------------------------------------

/** US states and DC on an 11 × 8 grid: `code col row name`, `|`-separated. */
const US_TILES = "AK 0 0 Alaska|ME 10 0 Maine|WI 5 1 Wisconsin|VT 9 1 Vermont|NH 10 1 New Hampshire|WA 0 2 Washington|ID 1 2 Idaho|MT 2 2 Montana|ND 3 2 North Dakota|MN 4 2 Minnesota|IL 5 2 Illinois|MI 6 2 Michigan|NY 8 2 New York|MA 9 2 Massachusetts|OR 0 3 Oregon|NV 1 3 Nevada|WY 2 3 Wyoming|SD 3 3 South Dakota|IA 4 3 Iowa|IN 5 3 Indiana|OH 6 3 Ohio|PA 7 3 Pennsylvania|NJ 8 3 New Jersey|CT 9 3 Connecticut|RI 10 3 Rhode Island|CA 0 4 California|UT 1 4 Utah|CO 2 4 Colorado|NE 3 4 Nebraska|MO 4 4 Missouri|KY 5 4 Kentucky|WV 6 4 West Virginia|VA 7 4 Virginia|MD 8 4 Maryland|DE 9 4 Delaware|AZ 1 5 Arizona|NM 2 5 New Mexico|KS 3 5 Kansas|AR 4 5 Arkansas|TN 5 5 Tennessee|NC 6 5 North Carolina|SC 7 5 South Carolina|DC 8 5 District of Columbia|OK 3 6 Oklahoma|LA 4 6 Louisiana|MS 5 6 Mississippi|AL 6 6 Alabama|GA 7 6 Georgia|HI 0 7 Hawaii|TX 3 7 Texas|FL 8 7 Florida";
/** European countries on a 9 × 9 grid: `alpha-3 alpha-2 col row name`, `|`-separated. */
const EUROPE_TILES = "ISL IS 0 0 Iceland|NOR NO 4 0 Norway|SWE SE 5 0 Sweden|FIN FI 6 0 Finland|IRL IE 1 1 Ireland|GBR GB 2 1 United Kingdom|EST EE 6 1 Estonia|NLD NL 3 2 Netherlands|DNK DK 4 2 Denmark|LVA LV 6 2 Latvia|BEL BE 3 3 Belgium|DEU DE 4 3 Germany|POL PL 5 3 Poland|LTU LT 6 3 Lithuania|BLR BY 7 3 Belarus|FRA FR 2 4 France|LUX LU 3 4 Luxembourg|CZE CZ 4 4 Czechia|SVK SK 5 4 Slovakia|UKR UA 6 4 Ukraine|PRT PT 0 5 Portugal|ESP ES 1 5 Spain|CHE CH 3 5 Switzerland|AUT AT 4 5 Austria|HUN HU 5 5 Hungary|ROU RO 6 5 Romania|MDA MD 7 5 Moldova|ITA IT 3 6 Italy|SVN SI 4 6 Slovenia|HRV HR 5 6 Croatia|SRB RS 6 6 Serbia|BGR BG 7 6 Bulgaria|MLT MT 3 7 Malta|BIH BA 5 7 Bosnia and Herzegovina|MNE ME 6 7 Montenegro|MKD MK 7 7 North Macedonia|TUR TR 8 7 Türkiye|ALB AL 6 8 Albania|GRC GR 7 8 Greece|CYP CY 8 8 Cyprus";

/** A built-in grid layout as columns: `id` (the key data joins on), `abbr` (the tile's text), `name`, `col`, `row`. */
function tileLayout(name: string, codes: string): Record<string, unknown[]> {
  const out: Record<string, unknown[]> = { id: [], abbr: [], name: [], col: [], row: [] };
  const europe = name === "europe";
  for (const entry of (europe ? EUROPE_TILES : US_TILES).split("|")) {
    const parts = entry.split(" ");
    const [a3, a2] = europe ? [parts[0], parts[1]] : [parts[0], parts[0]];
    const rest = parts.slice(europe ? 2 : 1);
    out.id.push(europe && codes !== "alpha2" ? a3 : a2);
    out.abbr.push(a2);
    out.col.push(Number(rest[0]));
    out.row.push(Number(rest[1]));
    out.name.push(rest.slice(2).join(" "));
  }
  return out;
}

export interface TileMapParams { data: string; key: string; value: string; layout: string; codes: "alpha3" | "alpha2"; shape: "square" | "hex"; colorType: "sequential" | "diverging" | "categorical" | "piecewise"; stops: string; format: string; label: Prop; values: boolean; legend: boolean; gap: number }

export const tileMap = recipe<TileMapParams>({
  id: "@datars/std/tileMap",
  doc: "A tile-grid map (a grid cartogram): every state or country the same size — a square or a hexagon — in roughly its place, coloured by value and marked with its abbreviation, so small places count as much as big ones. Built-in layouts for US states (postal codes) and European countries (ISO codes), or a layout table of your own (`id`, `col`, `row`, optional `abbr` and `name`). Tiles without data stay neutral; tiles are keyed by id, so they morph to the same region on a map.",
  params: {
    data: t.table("Values per place."), key: t.field("The data column holding each place's id (a postal code, an ISO code, your layout's `id`)."), value: t.field("The value to colour by."),
    layout: t.string("us", "`us` (50 states and DC), `europe` (40 countries), or the name of a table with columns `id`, `col`, `row` (and optional `abbr`, `name`)."),
    codes: t.oneOf(["alpha3", "alpha2"] as const, "alpha3", "The `europe` layout's ids: ISO 3166 alpha-3 (`DEU`, as the countries atlas) or alpha-2 (`DE`)."),
    shape: t.oneOf(["square", "hex"] as const, "square", "Square tiles, or hexagons (odd rows shifted half a tile)."),
    colorType: t.oneOf(["sequential", "diverging", "categorical", "piecewise"] as const, "sequential"),
    stops: t.string(undefined, "Piecewise colour stops: '#22c55e 2 · #f5a524 4 · #f97362 6.5'."),
    format: t.string(",.1~f", "Number format in tooltips, values and the legend."),
    label: t.prop("Tile label (tooltip, accessible name): an expression over the tile row; default `name: value`."),
    values: t.bool(false, "The value under each abbreviation, where tiles are big enough."),
    legend: t.bool(true, "A colour ramp (or swatches) under the grid."),
    gap: t.number(2, "Space between tiles (px)."),
  },
  tokens: ["map.no-data", "sequential", "diverging", "categorical", "ink-2", "paper", "size.small"],
  expand(p, cx) {
    const V = fld(p.value);
    const vals = cx.table("tile-values", p.data, op.derive("id", e(`String(${fld(p.key)})`)));
    const builtIn = p.layout === "us" || p.layout === "europe" || !p.layout;
    const tiles = builtIn
      ? cx.table("tiles", p.data, op.values(tileLayout(p.layout || "us", p.codes), { key: "id" }), op.join(vals, "id", "left"))
      : cx.table("tiles", p.layout, op.derive("id", e("String(d.id)")), op.join(vals, "id", "left"));
    const Tq = q(tiles);
    const hex = p.shape === "hex";
    const legendH = p.legend ? 34 : 0;
    const C = `(table.max(${Tq}, "col") + 1)`, R = `(table.max(${Tq}, "row") + 1)`;
    // Tile size (a square's side, a hexagon's radius) to fit the box, and the grid centred in it.
    const size = hex ? `min(box.w / ((${C} + 0.5) * 1.7320508), (box.h - ${legendH}) / ((${R} - 1) * 1.5 + 2))` : `min(box.w / ${C}, (box.h - ${legendH}) / ${R})`;
    const gw = hex ? `((${C} + 0.5) * 1.7320508 * d.__s)` : `(${C} * d.__s)`;
    const gh = hex ? `(((${R} - 1) * 1.5 + 2) * d.__s)` : `(${R} * d.__s)`;
    const geo = cx.table("tile-grid", tiles,
      op.derive("__s", e(size)),
      op.derive("__ox", e(`(box.w - ${gw}) / 2`)), op.derive("__oy", e(`(box.h - ${legendH} - ${gh}) / 2`)),
      op.derive("__cx", e(hex ? "d.__ox + (d.col + 0.5 + (d.row % 2) * 0.5) * 1.7320508 * d.__s" : "d.__ox + (d.col + 0.5) * d.__s")),
      op.derive("__cy", e(hex ? "d.__oy + d.__s + d.row * 1.5 * d.__s" : "d.__oy + (d.row + 0.5) * d.__s")));
    const g = Math.max(0, p.gap ?? 2);
    const side = hex ? "(d.__s * 1.7320508)" : "d.__s";
    const color = p.colorType === "categorical" ? `scale.color(${V})` : `scale.color(${V})`;
    const fill = `(${V} == null ? "$map.no-data" : ${color})`;
    const rr = `max(1, d.__s - ${g / 1.7320508})`;
    const hexPath = `\`M \${d.__cx} \${d.__cy - ${rr}} L \${d.__cx + ${rr} * 0.8660254} \${d.__cy - ${rr} * 0.5} L \${d.__cx + ${rr} * 0.8660254} \${d.__cy + ${rr} * 0.5} L \${d.__cx} \${d.__cy + ${rr}} L \${d.__cx - ${rr} * 0.8660254} \${d.__cy + ${rr} * 0.5} L \${d.__cx - ${rr} * 0.8660254} \${d.__cy - ${rr} * 0.5} Z\``;
    const tileGeom = hex ? geom.path(e(hexPath)) : geom.rect({ x: e(`d.__cx - (d.__s - ${g}) / 2`), y: e(`d.__cy - (d.__s - ${g}) / 2`), w: e(`max(1, d.__s - ${g})`), h: e(`max(1, d.__s - ${g})`), r: 2 });
    const name = "(d.name ?? d.abbr ?? d.id)";
    const label = p.label ?? e(`${name} + ": " + (${V} == null ? "no data" : format(${V}, ${q(p.format)}))`);
    const abbr = "String(d.abbr ?? d.id)";
    const fs = `clamp(${side} * 0.3, 8, 14)`;
    const withValue = p.values ? `(${side} >= 38 && ${V} != null)` : "false";
    const scales: Record<string, ScaleDecl> = {
      color: p.colorType === "piecewise"
        ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } }
        : { type: p.colorType, domain: { data: p.data, field: p.value }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" },
    };
    // The legend under the grid, from its left edge: a stepped ramp with the extremes (the pinned
    // stops for piecewise colour), or swatches for categories.
    const pinned = p.colorType === "piecewise" && p.stops ? p.stops.split(/[\s·,;]+/).filter((s) => s !== "" && !Number.isNaN(Number(s))).map(Number) : [];
    const ox = `(box.w - ${gw.replace(/d\.__s/g, `(${size})`)}) / 2`;
    // Just under the grid (a phone's tall box leaves room below it).
    const gh0 = gh.replace(/d\.__s/g, `(${size})`);
    const by = `((box.h - ${legendH} - ${gh0}) / 2 + ${gh0})`;
    const ext = pinned.length >= 2
      ? cx.table("tile-extent", p.data, op.aggregate([], { n: ["count"] }), op.derive("lo", pinned[0]), op.derive("hi", pinned[pinned.length - 1]), op.derive("__ox", e(ox)))
      : cx.table("tile-extent", p.data, op.aggregate([], { lo: ["min", p.value], hi: ["max", p.value] }), op.derive("__ox", e(ox)));
    const steps = 6, sw = 22;
    const legendNode = !p.legend ? null : p.colorType === "categorical"
      ? group({ key: "legend", transform: { translate: [e(`max(0, ${ox})`), e(`${by} + 12`)] }, layout: { type: "flow", gap: 14 }, semantics: { role: "legend", label: "Legend" }, children: [
          repeat({ legend: "color" }, group({ semantics: { role: "legend-item", label: e("key.name(d.label)") }, children: [
            shape(geom.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e("d.ink") }),
            text(e("key.name(d.label)"), [15, 9], { style: { size: "$size.small", ink: "$ink-2" } }),
          ] })),
        ] })
      : group({ key: "legend", semantics: { role: "legend", label: "Colour scale" }, children: [repeat(ext, group({ key: "ramp", transform: { translate: [e("max(0, d.__ox)"), e(`${by} + 10`)] }, children: [
          ...Array.from({ length: steps }, (_, i) => shape(geom.rect({ x: i * sw, y: 0, w: sw, h: 8 }), { key: `step-${i}`, fill: e(`scale.color(d.lo + (d.hi - d.lo) * ${i / (steps - 1)})`), semantics: { role: "decoration" } })),
          text(e(`format(d.lo, ${q(p.format)})`), [0, 20], { key: "lo", style: { size: "$size.small", ink: "$ink-2" } }),
          text(e(`format(d.hi, ${q(p.format)})`), [steps * sw, 20], { key: "hi", style: { size: "$size.small", ink: "$ink-2", align: "end" } }),
        ] }))] });
    return group({
      key: "tile-map",
      scales,
      semantics: { role: "group", label: "Tile map" },
      children: [
        group({ key: "tiles", children: [repeat(geo, group({ key: e("d.id"), children: [
          shape(tileGeom, { key: "tile", fill: e(fill), semantics: { role: "region", label, value: e(V) }, pickable: true }),
          text(e(abbr), [e("d.__cx"), e(`d.__cy - (${withValue} ? ${fs} * 0.45 : 0)`)], { key: "abbr", when: e(`${side} >= 16`), style: { size: e(fs), weight: 600, ink: e(`"on(" + ${fill} + ")"`), align: "middle", baseline: "middle" } }),
          p.values ? text(e(`format(${V}, ${q(p.format)})`), [e("d.__cx"), e(`d.__cy + ${fs} * 0.6`)], { key: "value", when: e(withValue), style: { size: e(`${fs} * 0.8`), ink: e(`"on(" + ${fill} + ")"`), align: "middle", baseline: "middle" } }) : null,
        ] }))] }),
        legendNode,
      ],
    });
  },
});
