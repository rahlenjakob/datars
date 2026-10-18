// Chart recipes beyond the basic marks — each is a composition of primitives plus a layout
// algorithm run by the engine (datars-algo) as a table op. Covers every chart type in the coverage
// corpus (docs/15-chart-coverage.md).

import { e, group, instances, op, recipe, repeat, shape, geom, t, text, Prop, ScaleDecl, Template } from "@datars/sdk";
import { fld } from "./field.js";
import { legend } from "./guides.js";

/** A number label: counting (`number`) when plain; with a prefix or suffix, formatted text. */
// A named type, not an inline one: `datars eject` copies helpers by their first brace block.
type Affix = { format: string; prefix?: string; suffix?: string };
function affixed(p: Affix, value: string, at: [Prop, Prop], opts: Record<string, unknown>) {
  if (!p.prefix && !p.suffix) return text("", at, { ...opts, number: { value: e(value), format: p.format } });
  // The sign leads the prefix: −$38.6M, not $−38.6M.
  const v = `(${value})`;
  return text(e(`(${v} < 0 ? "−" : "") + ${JSON.stringify(p.prefix ?? "")} + format(abs(${v}), ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}`), at, opts);
}

const lbl = (cat: string, val: string, fmt: string) => e(`\`\${key.name(${fld(cat)})}: \${format(${fld(val)}, ${JSON.stringify(fmt)})}\``);

// ---- cell (heatmap) -----------------------------------------------------------------------------

export interface CellParams { data: string; x: string; y: string; color: string; gap: number; format: string; label: Prop }

export const cell = recipe<CellParams>({
  id: "@datars/std/cell",
  doc: "Heatmap cells: a rectangle per (x, y) on two band scales, coloured by the colour scale. Keys (x, y) match grouped bars, so the two morph.",
  params: { data: t.table(), x: t.field(), y: t.field(), color: t.field("The value field (on a sequential/diverging colour scale)."), gap: t.number(1), format: t.string(".1~f"), label: t.prop("Cell label (tooltip, accessible name); default `y, x: value`.") },
  expand(p) {
    return group({
      key: "marks",
      children: [repeat(p.data, shape(geom.rect({ x: e(`scale.x(${fld(p.x)}) + ${p.gap / 2}`), y: e(`scale.y(${fld(p.y)}) + ${p.gap / 2}`), w: e(`scale.x.bandwidth() - ${p.gap}`), h: e(`scale.y.bandwidth() - ${p.gap}`) }), {
        key: [e(`${fld(p.y)}`), e(`${fld(p.x)}`)],
        fill: e(`scale.color(${fld(p.color)})`),
        semantics: { role: "datum", label: p.label ?? e(`\`\${${fld(p.y)}}, \${${fld(p.x)}}: \${format(${fld(p.color)}, ${JSON.stringify(p.format)})}\``) },
        pickable: true,
      }))],
    });
  },
});

// ---- stripes (warming stripes) -------------------------------------------------------------------

export interface StripesParams { data: string; x: string; value: string; mid: number; label: Prop }

export const stripes = recipe<StripesParams>({
  id: "@datars/std/stripes",
  doc: "Colour stripes: one full-height band per ordered value, on a diverging scale around `mid`.",
  params: { data: t.table(), x: t.field(), value: t.field(), mid: t.number(0), label: t.prop("Stripe label (tooltip, accessible name); default `x: value`.") },
  expand(p) {
    return group({
      key: "marks",
      scales: {
        sx: { type: "band", domain: { data: p.data, field: p.x }, range: "width", padding: 0 },
        c: { type: "diverging", domain: { data: p.data, field: p.value }, range: "$diverging", mid: p.mid },
      },
      children: [repeat(p.data, shape(geom.rect({ x: e(`scale.sx(${fld(p.x)})`), y: 0, w: e("scale.sx.bandwidth() + 0.5"), h: e("box.h") }), {
        fill: e(`scale.c(${fld(p.value)})`), semantics: { role: "datum", label: p.label ?? e(`\`\${${fld(p.x)}}: \${${fld(p.value)}}\``) }, pickable: true,
      }))],
    });
  },
});

/** Template-literal text for a formatted number with its prefix and suffix (sign first: −$3.2M). */
function affixedLabel(v: string, p: { format?: string; prefix?: string; suffix?: string }): string {
  const f = JSON.stringify(p.format ?? ",.1~f");
  return `\${${v} < 0 ? "−" : ""}${p.prefix ?? ""}\${format(abs(${v}), ${f})}${p.suffix ?? ""}`;
}

// ---- stacked bars (and 100 %) -----------------------------------------------------------------------

export interface StackedParams { data: string; x: string; y: string; color: string; series: string; offset: "zero" | "expand"; xType: string; yType: string; segmentKey: Prop; labels: boolean; format: string; prefix: string; suffix: string }

export const stacked = recipe<StackedParams>({
  id: "@datars/std/stacked",
  doc: "Stacked bars: a segment per series in each category (offset `expand` for 100 %); horizontal when y is the band axis. Keys (series, x), shared with grouped bars and heatmap cells.",
  params: {
    data: t.table(), x: t.field(), y: t.field(), color: t.field(), series: t.field(), offset: t.oneOf(["zero", "expand"] as const, "zero"), xType: t.string("band"), yType: t.string("linear"),
    segmentKey: t.prop("Segment key (default the composite `(series, category)`, so segments merge into their series' bar); e.g. `d.party` so one 100 % bar's segments pair with that party's bar or slice."),
    labels: t.bool(false, "Value labels inside the segments (they count when values change), where they fit."),
    format: t.string(",.1~f", "Number format of each segment's label (its value label, hover, screen readers)."),
    prefix: t.string(undefined, "Before the number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr')."),
  },
  tokens: ["size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const horizontal = (p.yType === "band" || p.yType === "point") && p.xType !== "band" && p.xType !== "point";
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    const tbl = cx.table("stack", p.data, op.stack({ x: cat, series, value: val, offset: p.offset, as: ["y0", "y1"] }));
    const g = horizontal
      ? geom.rect({ x: e("scale.x(d.y0)"), y: e(`scale.y(${fld(cat)})`), w: e("scale.x(d.y1) - scale.x(d.y0)"), h: e("scale.y.bandwidth()") })
      : geom.rect({ x: e(`scale.x(${fld(cat)})`), y: e("scale.y(d.y1)"), w: e("scale.x.bandwidth()"), h: e("scale.y(d.y0) - scale.y(d.y1)") });
    // A value label in the middle of each segment, in the ink that reads on it — only where it
    // fits (a sliver of a party's seats keeps its number to the tooltip).
    const lw = `measure(${JSON.stringify(p.prefix ?? "")} + format(${fld(val)}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}, token("size.label"))`;
    const fits = horizontal
      ? `abs(scale.x(d.y1) - scale.x(d.y0)) >= ${lw} + 8 && scale.y.bandwidth() >= token("size.label") + 4`
      : `scale.x.bandwidth() >= ${lw} + 8 && abs(scale.y(d.y0) - scale.y(d.y1)) >= token("size.label") + 4`;
    const at: [Prop, Prop] = horizontal
      ? [e("(scale.x(d.y0) + scale.x(d.y1)) / 2"), e(`scale.y(${fld(cat)}) + scale.y.bandwidth() / 2`)]
      : [e(`scale.x(${fld(cat)}) + scale.x.bandwidth() / 2`), e("(scale.y(d.y0) + scale.y(d.y1)) / 2")];
    const label = affixed(p, `${fld(val)}`, at, { when: e(fits), style: { size: "$size.label", ink: e(`"on(" + scale.color(${fld(series)}) + ")"`), align: "middle", baseline: "middle" } });
    return group({
      key: "marks",
      children: [
        repeat(tbl, shape(g, {
          key: p.segmentKey ?? [e(`${fld(series)}`), e(`${fld(cat)}`)],
          fill: e(`scale.color(${fld(series)})`),
          stroke: { paint: "$paper", width: 1 }, // (a line between touching pieces: told apart however close their colours, WCAG 1.4.11)
          semantics: { role: "datum", label: e(`\`\${key.name(${fld(series)})}, \${${fld(cat)}}: ${affixedLabel(`${fld(val)}`, p)}\``), value: e(`${fld(val)}`) },
          pickable: true,
        })),
        p.labels ? group({ key: "labels", children: [repeat(tbl, label)] }) : null,
      ],
    });
  },
});

// ---- grouped bars ---------------------------------------------------------------------------------

export interface GroupedParams { data: string; x: string; y: string; color: string; series: string; xType: string; yType: string; gap: number; labels: boolean; format: string; prefix: string; suffix: string }

export const grouped = recipe<GroupedParams>({
  id: "@datars/std/grouped",
  doc: "Grouped (dodged) bars: a group per x, a bar per series side by side. Keys (series, x), shared with stacked segments and heatmap cells, so the three morph.",
  params: { data: t.table(), x: t.field(), y: t.field(), color: t.field(), series: t.field("The series within each group (default: the colour field)."), xType: t.string("band"), yType: t.string("linear"), gap: t.number(0.08, "Padding between bars of a group (band fraction)."),
    labels: t.bool(false, "Value labels on the bars (they count when values change), where they fit a bar's width."), format: t.string(",.1~f", "Number format of each bar's label."),
    prefix: t.string(undefined, "Before each bar's number in its label ('$')."), suffix: t.string(undefined, "After it ('M', ' kr').") },
  tokens: ["ink-2", "size.label", "radius.bar"],
  expand(p) {
    const series = p.series ?? p.color;
    const x = `scale.x(${fld(p.x)}) + scale.dodge(${fld(series)})`, v = `${fld(p.y)}`;
    // Value labels as on `bar`: past the bar's end, or just inside it (in the ink that reads on
    // the bar) when the plot has no room above — and only where one fits its bar's slot, so
    // neighbours' labels never touch.
    const lw = `measure(${JSON.stringify(p.prefix ?? "")} + format(${v}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}, token("size.label"))`;
    const end = `min(scale.y(0), scale.y(${v}))`;
    const inside = `(${end} - 4 - token("size.label") < -2 && abs(scale.y(${v}) - scale.y(0)) >= token("size.label") + 8)`;
    const label = affixed(p, v, [e(`${x} + scale.dodge.bandwidth() / 2`), e(`${inside} ? ${end} + 4 : ${end} - 4`)], {
      when: e(`${lw} <= scale.dodge.step() - 2`),
      style: { size: "$size.label", ink: e(`${inside} ? "on(" + scale.color(${fld(series)}) + ")" : "$ink-2"`), align: "middle", baseline: e(`${inside} ? "top" : "alphabetic"`) },
    });
    return group({
      key: "marks",
      // An inner band scale over the series, spanning one outer band.
      scales: { dodge: { type: "band", domain: { data: p.data, field: series }, range: [0, "=scale.x.bandwidth()"], padding: p.gap } },
      children: [
        repeat(p.data, shape(geom.rect({ x: e(x), y: e(`min(scale.y(0), scale.y(${v}))`), w: e("scale.dodge.bandwidth()"), h: e(`abs(scale.y(0) - scale.y(${v}))`), r: "$radius.bar" }), {
          key: [e(`${fld(series)}`), e(`${fld(p.x)}`)],
          fill: e(`scale.color(${fld(series)})`),
          stroke: { paint: "$paper", width: 1 }, // (a line between touching pieces: told apart however close their colours, WCAG 1.4.11)
          semantics: { role: "datum", label: e(`\`\${key.name(${fld(series)})}, \${${fld(p.x)}}: ${affixedLabel(v, p)}\``), value: e(v) },
          pickable: true,
        })),
        p.labels ? group({ key: "labels", children: [repeat(p.data, label)] }) : null,
      ],
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }],
});

// ---- treemap --------------------------------------------------------------------------------------

export interface TreemapParams { data: string; value: string; category: string; color: string; colorType: "categorical" | "piecewise"; stops: string; labels: boolean; format: string }

export const treemap = recipe<TreemapParams>({
  id: "@datars/std/treemap",
  doc: "A squarified treemap: area ∝ value, one rectangle per category, labels where they fit. Coloured by category, or by a value on piecewise stops.",
  params: {
    data: t.table(), value: t.field(), category: t.field(),
    color: t.field("Colour by this field (default: the category)."),
    colorType: t.oneOf(["categorical", "piecewise"] as const, "categorical"),
    stops: t.string(undefined, "Piecewise colour stops: '#bae6fd 7 · #0b1f3a 14.2'."),
    labels: t.bool(true), format: t.string(".1~f"),
  },
  expand(p, cx) {
    const tbl = cx.table("treemap", p.data, op.treemap({ value: p.value, width: e("box.w"), height: e("box.h"), as: ["x0", "y0", "x1", "y1"] }));
    const by = p.color ?? p.category;
    const color: ScaleDecl = p.colorType === "piecewise"
      ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: by } }
      : { type: "categorical", domain: { data: p.data, field: by }, range: "$categorical" };
    return group({
      key: "marks",
      scales: { color },
      children: [
        repeat(tbl, shape(geom.rect({ x: e("d.x0"), y: e("d.y0"), w: e("d.x1 - d.x0"), h: e("d.y1 - d.y0") }), { fill: e(`scale.color(${fld(by)})`), stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "datum", label: lbl(p.category, p.value, p.format) }, pickable: true })),
        // Each label in the ink that reads on its rectangle's fill (dark on a light one).
        // (Only in a rectangle with room for a few letters and a line of them, in the label's
        // size: a reader's bigger text leaves out labels in more of the small ones.)
        p.labels ? group({ key: "labels", children: [repeat(tbl, text(e(`key.name(${fld(p.category)})`), [e("d.x0 + 6"), e('d.y0 + 4 + token("size.label") * 12 / 11')], { when: e('d.x1 - d.x0 > token("size.label") * 4 && d.y1 - d.y0 > token("size.label") * 2'), style: { size: "$size.label", weight: 600, ink: e(`"on(" + scale.color(${fld(by)}) + ")"`), maxWidth: e("d.x1 - d.x0 - 10") } }))] }) : null,
      ],
    });
  },
});

// ---- waffle ---------------------------------------------------------------------------------------

export interface WaffleParams { data: string; value: string; category: string; columns: number; rows: number; gap: number }

export const waffle = recipe<WaffleParams>({
  id: "@datars/std/waffle",
  doc: "One square per unit (e.g. per percentage point). Units are keyed (category, Unit i), so bars split into squares and back.",
  params: { data: t.table(), value: t.field(), category: t.field(), columns: t.number(10), rows: t.number(10), gap: t.number(2) },
  expand(p, cx) {
    const u = cx.table("waffle", p.data, op.units({ value: p.value }), op.waffle({ columns: p.columns, rows: p.rows, width: e("min(box.w, box.h)"), height: e("min(box.w, box.h)"), gap: p.gap }));
    return group({
      key: "waffle",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      children: [instances({ key: "marks", from: u, proto: "rect", x: e("d.x + (box.w - min(box.w, box.h)) / 2"), y: e("d.y + (box.h - min(box.w, box.h)) / 2"), w: e("d.w"), h: e("d.h"), fill: e(`scale.color(${fld(p.category)})`), label: e(`key.name(${fld(p.category)})`) })],
    });
  },
});

// ---- pictogram (isotype) ---------------------------------------------------------------------------

export interface PictogramParams { data: string; value: string; category: string; icon: string; icons: Record<string, string>; per: number; layout: "rows" | "grid"; columns: number; rows: number; color: boolean; ink: string; format: string; unit: string; gap: number; legend: boolean; note: boolean }

/** An expression picking the icon for a category: the map's entry, else `fallback`. */
function iconFor(field: string, icons: Record<string, string>, fallback: string): string {
  return Object.entries(icons).reduceRight((rest, [k, v]) => `(${field} == ${JSON.stringify(k)} ? ${JSON.stringify(v)} : ${rest})`, JSON.stringify(fallback));
}

export const pictogram = recipe<PictogramParams>({
  id: "@datars/std/pictogram",
  doc: "A pictogram (isotype): one icon per unit — per person, per thousand tonnes (`per`) — in a row per category with its name and value (`rows`), or filling a grid like a waffle (`grid`). Icons come from the default set (`user`, `car`, `tree-pine`) or a theme token, one for all or one per category (`icons`); every unit is an instance of one icon, so thousands draw as one node. Units are keyed (category, Unit i) as waffle's are, so a bar chart splits into its icons and back.",
  params: {
    data: t.table(), value: t.field(), category: t.field(),
    icon: t.string("user", "The icon every unit draws (a default-set name, `lucide:…`, `$icon.…`, a `.svg` file)."),
    icons: t.json("An icon per category (`{ \"Cars\": \"car\", \"Bikes\": \"bike\" }`); others draw `icon`.") as never,
    per: t.number(1, "What one icon stands for (values are rounded to whole icons)."),
    layout: t.oneOf(["rows", "grid"] as const, "rows", "`rows`: a row per category, labelled; `grid`: every unit in one grid (`columns` × `rows`), coloured by category."),
    columns: t.number(10, "`grid`: icons across."), rows: t.number(10, "`grid`: icons down."),
    color: t.bool(true, "Colour by category (`$categorical`); off, every icon in `ink`."),
    ink: t.ink("$mark", "The icons' ink without `color`."),
    format: t.string(",.0f", "The value's number format (rows)."), unit: t.string(undefined, "After the value (` people`)."),
    gap: t.number(0.18, "Space between icons, as a share of their size."),
    legend: t.bool(true, "`grid`: a legend of the categories (with their icons) below."),
    note: t.bool(true, "`rows` with `per`: say what one icon stands for under the rows (off when the page says it)."),
  },
  tokens: ["mark", "ink", "ink-2", "muted", "size.label", "categorical"],
  expand(p, cx) {
    const per = p.per > 0 ? p.per : 1;
    const icons = p.icons && typeof p.icons === "object" ? p.icons : {};
    const u = cx.table("pictogram", p.data, op.derive("__icons", e(`round(${fld(p.value)} / ${per})`)), op.units({ value: "__icons" }));
    const fill = p.color ? e(`scale.color(${fld(p.category)})`) : p.ink;
    const color: Record<string, ScaleDecl> = p.color ? { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } } : {};
    // Every distinct icon is one instances node over its categories' units.
    const kinds = [...new Set([p.icon, ...Object.values(icons)])];
    const nodes = (x: string, y: string, r: string): Template[] => kinds.map((ic, i) => {
      const mine = Object.keys(icons).filter((k) => icons[k] === ic);
      const others = Object.keys(icons).filter((k) => icons[k] !== ic);
      const keep = ic === p.icon ? others.map((k) => `${fld(p.category)} != ${JSON.stringify(k)}`).join(" && ") : mine.map((k) => `${fld(p.category)} == ${JSON.stringify(k)}`).join(" || ");
      const from = keep ? cx.table(`pictogram-${i}`, u, op.filter(e(keep))) : u;
      return instances({ key: `icons-${i}`, from, icon: ic, x: e(x), y: e(y), r: e(r), fill });
    });
    const said = () => e(`key.name(${fld(p.category)}) + ": " + format(${fld(p.value)}, ${JSON.stringify(p.format)})${p.unit ? ` + ${JSON.stringify(p.unit)}` : ""}${per !== 1 ? ` + " (one icon: " + format(${per}, ",~g") + ")"` : ""}`);
    if (p.layout === "grid") {
      const g = cx.table("pictogram-grid", u, op.waffle({ columns: p.columns, rows: p.rows, width: e("min(box.w, box.h)"), height: e("min(box.w, box.h)"), gap: 0 }));
      const off = "(box.w - min(box.w, box.h)) / 2";
      const k = (1 - Math.min(0.6, Math.max(0, p.gap))) / 2;
      const gridNodes = kinds.map((ic, i) => {
        const mine = Object.keys(icons).filter((key) => icons[key] === ic);
        const others = Object.keys(icons).filter((key) => icons[key] !== ic);
        const keep = ic === p.icon ? others.map((key) => `${fld(p.category)} != ${JSON.stringify(key)}`).join(" && ") : mine.map((key) => `${fld(p.category)} == ${JSON.stringify(key)}`).join(" || ");
        const from = keep ? cx.table(`pictogram-grid-${i}`, g, op.filter(e(keep))) : g;
        return instances({ key: `icons-${i}`, from, icon: ic, x: e(`d.x + d.w / 2 + ${off}`), y: e("d.y + d.h / 2 + (box.h - min(box.w, box.h)) / 2"), r: e(`min(d.w, d.h) * ${k.toFixed(3)}`), fill });
      });
      const many = Object.keys(icons).length > 0;
      return group({
        key: "pictogram",
        scales: color,
        semantics: { role: "group", label: "Pictogram" },
        layout: { type: "rows", gap: 10 },
        children: [
          group({ key: "grid", size: { h: "fill" }, children: [
            ...gridNodes,
            // What each category stands for, read once (not unit by unit).
            repeat(p.data, group({ key: e(fld(p.category)), semantics: { role: "series", label: said(), value: e(fld(p.value)) }, children: [] })),
          ] }),
          p.legend !== false && p.color ? legend(many ? { scale: "color", icons } : { scale: "color", icon: p.icon }, { size: { h: "auto" } }) : null,
        ],
      });
    }
    // Rows: names on the left, the icons, the value after the last one. Icons wrap to more lines
    // in a row rather than shrink past legibility (a phone's column); rows are as tall as their
    // lines.
    const labelW = "min(150, max(64, box.w * 0.24))";
    const valueW = 'token("size.label") * 4';
    const n = `max(1, table.count(${JSON.stringify(p.data)}))`;
    const most = `max(1, table.max(${JSON.stringify(u)}, "__unit") + 1)`;
    const avail = `max(20, box.w - ${labelW} - ${valueW} - 14)`;
    // The icons as big as the box allows: on one line per row, or wrapped to two when that makes
    // them much bigger (a narrow, tall box: a phone). Every row is as tall as the longest one's lines.
    // (The note under the rows takes its own lines: at the reader's text size, not a fixed 28 px. In
    // a box too short for rows and note at a reader's big text it gives way: each row says it too.)
    const noted = per !== 1 && p.note !== false;
    const room = `(box.h - ${noted ? '(token("size.small") * 2 + 8)' : 0})`;
    const sized = (k: number) => `min(${avail} / ceil(${most} / ${k}), 40, ${room} / (${n} * ${(k * 1.15 + 0.4).toFixed(2)}))`;
    // (Two lines only for icons half as big again: rows of uneven length read best on one.)
    const lines = `(${sized(2)} > ${sized(1)} * 1.5 ? 2 : 1)`;
    const best = `(${lines} == 2 ? ${sized(2)} : ${sized(1)})`;
    const perLine = `max(1, ceil(${most} / ${lines}))`;
    const step = `max(4, ${best})`;
    const rowH = `max(${lines} * ${step} * 1.15, token("size.label") * 2) + ${step} * 0.4`;
    const r = `${step} * ${((1 - Math.min(0.6, Math.max(0, p.gap))) / 2).toFixed(3)}`;
    // Rows and note centred in a box taller than they need (a figure sized for its other states),
    // as a gauge centres its dial: no block of empty space under them.
    const off = `max(0, (box.h - ${n} * (${rowH}) - ${noted ? '(token("size.small") * 2 + 8)' : 0}) / 2)`;
    const top = `scale.row(${fld(p.category)}) + ${step} * 0.2`;
    const line = `floor(d.__unit / ${perLine})`;
    const x = `${labelW} + 8 + (d.__unit - ${line} * ${perLine} + 0.5) * ${step}`;
    const y = `${top} + (${line} + 0.5) * ${step} * 1.15`;
    // The category's own count of icons: its last line, and how far along it the value goes.
    const own = `max(1, round(${fld(p.value)} / ${per}))`;
    const last = `floor((${own} - 1) / ${perLine})`;
    return group({
      key: "pictogram",
      scales: { ...color, row: { type: "band", domain: { data: p.data, field: p.category }, range: [`=${off}`, `=${off} + ${n} * (${rowH})`], padding: 0 } },
      semantics: { role: "group", label: "Pictogram" },
      children: [
        ...nodes(x, y, r),
        repeat(p.data, group({
          key: e(fld(p.category)),
          semantics: { role: "series", label: said(), value: e(fld(p.value)) },
          children: [
            text(e(`key.name(${fld(p.category)})`), [e(labelW), e(`${top} + ${step} * 0.575`)], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink", align: "end", baseline: "middle", maxWidth: e(labelW) } }),
            text(e(`format(${fld(p.value)}, ${JSON.stringify(p.format)})${p.unit ? ` + ${JSON.stringify(p.unit)}` : ""}`), [e(`${labelW} + 14 + (${own} - ${last} * ${perLine}) * ${step}`), e(`${top} + (${last} + 0.5) * ${step} * 1.15`)], { key: "value", style: { size: "$size.label", ink: "$ink-2", baseline: "middle" } }),
          ],
        })),
        noted ? text(e(`"One icon: " + format(${per}, ",~g")${p.unit ? ` + ${JSON.stringify(p.unit)}` : ""}`), [e(labelW + " + 8"), e('scale.row.max() + token("size.small") * 0.8 + 8')], { key: "per", when: e('scale.row.max() + token("size.small") * 1.4 + 8 <= box.h'), style: { size: "$size.small", ink: "$muted", baseline: "middle" } }) : null,
      ],
    });
  },
});

// ---- hemicycle (parliament) ---------------------------------------------------------------------------

export interface HemicycleParams { data: string; value: string; category: string; rows: number; total: boolean; align: "middle" | "bottom" }

export const hemicycle = recipe<HemicycleParams>({
  id: "@datars/std/hemicycle",
  doc: "A parliament: one dot per seat on concentric arcs, grouped by party left to right. Seats are keyed (party, Unit i).",
  params: { data: t.table(), value: t.field("Seats per party."), category: t.field(), rows: t.number(0, "Rows of seats (0 = automatic). More rows than automatic move inward to keep their arcs; past about √(seats / 2) they spread thin."), total: t.bool(true),
    align: t.oneOf(["middle", "bottom"] as const, "middle", "In a box taller than it needs (a phone): centred up and down, or at the bottom (with a line of text right under it).") },
  expand(p, cx) {
    // At the bottom of a box its radius fills; in a taller one (a phone), centred up and down.
    const cy = p.align === "bottom" ? "box.h - 10" : "min(box.h - 10, (box.h + min(box.w / 2 - 8, box.h - 18)) / 2 + 4)";
    const seats = cx.table("seats", p.data, op.units({ value: p.value }), op.parliament({ cx: e("box.w / 2"), cy: e(cy), r0: e("min(box.w / 2, box.h) * 0.36"), r1: e("min(box.w / 2 - 8, box.h - 18)"), rows: p.rows || undefined }));
    return group({
      key: "hemicycle",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      children: [
        // Few seats are big: lift them by what their radius needs beyond the 10 px under the baseline.
        instances({ key: "seats", from: seats, x: e("d.x"), y: e("d.y - max(0, d.r - 10)"), r: e("d.r"), fill: e(`scale.color(${fld(p.category)})`), label: e(`key.name(${fld(p.category)})`) }),
        p.total ? text("", [e("box.w / 2"), e(`${cy} - 4`)], { key: "total", number: { value: e(`sum(${JSON.stringify(p.data)}, ${JSON.stringify(p.value)})`), format: ",.0f" }, style: { font: "font.title", size: "$size.title", align: "middle" } }) : null,
      ],
    });
  },
});

// ---- swarm (beeswarm) -----------------------------------------------------------------------------------

export interface SwarmParams { data: string; x: string; color: string; r: number; xType: string }

export const swarm = recipe<SwarmParams>({
  id: "@datars/std/swarm",
  doc: "A beeswarm: every row a dot along one value axis, packed so none overlap (windowed neighbour search; 20k dots in about a second). In a plot it needs only `x`: without a `y`, the plot draws no y axis.",
  params: { data: t.table(), x: t.field(), color: t.field(), r: t.number(3.5), xType: t.string("linear") },
  expand(p, cx) {
    const tbl = cx.table("swarm", p.data, op.beeswarm({ position: e(`scale.x(${fld(p.x)})`), radius: p.r + 0.5, as: "offset" }));
    return instances({ key: "swarm", from: tbl, x: e(`scale.x(${fld(p.x)})`), y: e("box.h / 2 + d.offset"), r: p.r, fill: p.color ? e(`scale.color(${fld(p.color)})`) : "$mark", label: e(`\`\${${fld(p.x)}}\``) });
  },
});

// ---- sankey --------------------------------------------------------------------------------------------

export interface SankeyParams { data: string; source: string; target: string; value: string; nodeWidth: number; nodePadding: number; label: Prop; selected: string }

export const sankey = recipe<SankeyParams>({
  id: "@datars/std/sankey",
  doc: "Flows between nodes: nodes in columns by depth, ribbons as wide as their flow. Links keep their row key (bars of the same rows morph into ribbons).",
  params: { data: t.table("One row per link."), source: t.field(), target: t.field(), value: t.field(), nodeWidth: t.number(14), nodePadding: t.number(12), label: t.prop("Link label (tooltip, accessible name); default `source → target: value`."),
    selected: t.string(undefined, "A keyset signal of node names: links touching them stay strong, the others recede."),
  },
  expand(p, cx) {
    const common = { links: p.data, source: p.source, target: p.target, value: p.value, width: e("box.w"), height: e("box.h"), nodeWidth: p.nodeWidth, nodePadding: p.nodePadding };
    const nodes = cx.table("nodes", p.data, op.sankeyNodes(common));
    const links = cx.table("links", p.data, op.sankeyLinks(common));
    return group({
      key: "sankey",
      scales: { color: { type: "categorical", domain: { data: nodes, field: "name" }, range: "$categorical" } },
      children: [
        group({ key: "links", children: [repeat(links, shape(geom.path(e("d.path")), { fill: e(`scale.color(${fld(p.source)})`), opacity: p.selected ? e(`${p.selected}.isEmpty() || ${p.selected}.has(${fld(p.source)}) || ${p.selected}.has(${fld(p.target)}) ? 0.42 : 0.12`) : 0.42, semantics: { role: "datum", label: p.label ?? e(`\`\${${fld(p.source)}} → \${${fld(p.target)}}: \${${fld(p.value)}}\``) }, pickable: true }))] }),
        // Nodes closer than a line of their names (a short box, a reader's big text): a name that
        // would touch an earlier one is left out (its node still says it, to a pointer and a reader).
        group({ key: "nodes", declutter: true, children: [repeat(nodes, group({ children: [
          shape(geom.rect({ x: e("d.x0"), y: e("d.y0"), w: e("d.x1 - d.x0"), h: e("d.y1 - d.y0") }), { key: "node", fill: e("scale.color(d.name)"), semantics: { role: "datum", label: e("`${d.name}: ${d.value}`") } }),
          // Over the flows beside the node: a halo keeps the name readable across them.
          text(e("d.name"), [e("d.x0 < box.w / 2 ? d.x1 + 6 : d.x0 - 6"), e("(d.y0 + d.y1) / 2")], { key: "label", style: { size: "$size.label", align: e('d.x0 < box.w / 2 ? "start" : "end"'), baseline: "middle" }, halo: ["$paper", 2.5] }),
        ] }))] }),
      ],
    });
  },
});

// ---- waterfall ----------------------------------------------------------------------------------------

export interface WaterfallParams { data: string; x: string; y: string; total: string; xType: string; yType: string; format: string; prefix: string; suffix: string }

export const waterfall = recipe<WaterfallParams>({
  id: "@datars/std/waterfall",
  doc: "A waterfall/bridge: each change floats from the running total; rows marked as totals (a boolean field, or keys starting with '=') are bars from zero.",
  params: { data: t.table(), x: t.field(), y: t.field(), total: t.field(), xType: t.string("band"), yType: t.string("linear"), format: t.string(",.0f"), prefix: t.string(undefined, "Before each label's number ('$')."), suffix: t.string(undefined, "After it ('M', ' kr').") },
  expand(p, cx) {
    const tbl = cx.table("waterfall", p.data, op.waterfall({ value: p.y, total: p.total }));
    const fill = `(d.is_total || d.start == 0 ? "$ink-2" : ${fld(p.y)} >= 0 ? "$positive" : "$negative")`;
    // Each label above its bar — or just inside the bar's top, in the ink that reads on it, when
    // the bar reaches the top of the plot (the label would run into the title).
    const top = "min(scale.y(d.start), scale.y(d.end))";
    const inside = `(${top} - 4 - token("size.label") < -2 && abs(scale.y(d.start) - scale.y(d.end)) >= token("size.label") + 8)`;
    return group({
      key: "waterfall",
      children: [repeat(tbl, group({ children: [
        shape(geom.rect({ x: e(`scale.x(${fld(p.x)})`), y: e("min(scale.y(d.start), scale.y(d.end))"), w: e("scale.x.bandwidth()"), h: e("abs(scale.y(d.start) - scale.y(d.end))") }), {
          key: "bar", fill: e(fill), semantics: { role: "datum", label: lbl(p.x, p.y, p.format) }, pickable: true,
        }),
        affixed(p, `d.is_total ? d.end : ${fld(p.y)}`, [e(`scale.x(${fld(p.x)}) + scale.x.bandwidth() / 2`), e(`${inside} ? ${top} + 4 : ${top} - 4`)], { key: "label", style: { size: "$size.label", ink: e(`${inside} ? "on(" + ${fill} + ")" : "$ink-2"`), align: "middle", baseline: e(`${inside} ? "top" : "alphabetic"`) } }),
      ] }))],
    });
  },
});

// ---- funnel -------------------------------------------------------------------------------------------

export interface FunnelParams { data: string; stage: string; value: string; format: string; title: string; fill: Prop; conversion: boolean }

export const funnel = recipe<FunnelParams>({
  id: "@datars/std/funnel",
  doc: "A funnel: stage names in a measured column, centred bars in stage order joined by tapering connectors, each labelled with the conversion from the stage before.",
  params: {
    data: t.table(), stage: t.field(), value: t.field(), format: t.string(",.0f"),
    title: t.string(undefined, "A title above the funnel."),
    fill: t.prop("The bars' colour (default $mark); labels inside take whichever of ink and paper reads on it."),
    conversion: t.bool(true, "Connectors between stages, labelled with the share that carries on (↓ 44 %)."),
  },
  tokens: ["mark", "ink", "ink-2", "muted", "paper", "size.label", "font.title", "size.title"],
  expand(p, cx) {
    const tbl = cx.table("funnel", p.data, op.window("lead", p.value, "next"));
    const v = `${fld(p.value)}`;
    const fill = p.fill ?? "$mark";
    const fillInk = typeof fill === "string" ? JSON.stringify(`on(${fill})`) : `"$accent-ink"`;
    const value = `format(${v}, ${JSON.stringify(p.format)})`;
    // Bars are centred in the area; a value label goes inside when it fits (measured), else just
    // right of the bar. Conversions sit in the gap below a bar, right of the connector's top edge.
    const half = `scale.fw(${v}) / 2`;
    const fits = `measure(${value}, token("size.label"), 600) + 14 <= scale.fw(${v})`;
    const top = `scale.fy(${fld(p.stage)})`, bottom = `${top} + scale.fy.bandwidth()`, gapMid = `${bottom} + (scale.fy.step() - scale.fy.bandwidth()) / 2`;
    const nextHalf = `scale.fw(d.next) / 2`;
    const connector = `\`M \${box.w / 2 - ${half}} \${${bottom}} L \${box.w / 2 + ${half}} \${${bottom}} L \${box.w / 2 + ${nextHalf}} \${${bottom} + scale.fy.step() - scale.fy.bandwidth()} L \${box.w / 2 - ${nextHalf}} \${${bottom} + scale.fy.step() - scale.fy.bandwidth()} Z\``;
    return group({
      key: "funnel",
      scales: {
        fy: { type: "band", domain: { data: p.data, field: p.stage }, range: { box: "funnel-area", axis: "y" }, padding: p.conversion ? 0.42 : 0.18 },
        fw: { type: "linear", domain: { data: p.data, field: p.value }, range: { box: "funnel-area", axis: "x" }, zero: true },
      },
      layout: { type: "rows", gap: 10 },
      semantics: { role: "group", label: p.title ?? "Funnel" },
      children: [
        p.title ? text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        group({
          key: "body",
          layout: { type: "columns", gap: 14 },
          children: [
            // Stage names, as wide as the widest (measured by layout), so none spills or overlaps.
            group({ key: "stages", size: { w: "auto" }, children: [repeat(tbl, text(e(`key.name(${fld(p.stage)})`), [0, e(`${top} + scale.fy.bandwidth() / 2`)], {
              key: "stage", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle" }, semantics: { role: "decoration" },
            }))] }),
            group({ id: "funnel-area", key: "area", children: [
              p.conversion ? repeat(tbl, shape(geom.path(e(connector)), { key: "connector", when: e("d.next != null"), fill, opacity: 0.2, semantics: { role: "decoration" } })) : null,
              repeat(tbl, group({ children: [
                shape(geom.rect({ x: e(`box.w / 2 - ${half}`), y: e(top), w: e(`scale.fw(${v})`), h: e("scale.fy.bandwidth()"), r: 2 }), { key: "bar", fill, semantics: { role: "datum", label: lbl(p.stage, p.value, p.format) }, pickable: true }),
                text(e(value), [e(`${fits} ? box.w / 2 : box.w / 2 + ${half} + 6`), e(`${top} + scale.fy.bandwidth() / 2`)], {
                  key: "value",
                  style: { size: "$size.label", weight: 600, ink: e(`${fits} ? ${fillInk} : "$ink"`), align: e(`${fits} ? "middle" : "start"`), baseline: "middle" },
                }),
                p.conversion ? text(e(`\`↓ \${format(d.next / ${v}, ".0%")}\``), [e(`box.w / 2 + max(${half}, ${nextHalf}) + 6`), e(gapMid)], {
                  key: "conversion", when: e(`d.next != null && ${v} > 0`),
                  style: { size: "$size.small", ink: "$muted", baseline: "middle" },
                }) : null,
              ] })),
            ] }),
            // Room right of the widest bar for its conversion label.
            p.conversion ? group({ key: "gutter", size: { w: 44 } }) : null,
          ],
        }),
      ],
    });
  },
});

// ---- calendar -----------------------------------------------------------------------------------------

export interface CalendarParams { data: string; date: string; value: string; cell: number; label: Prop }

export const calendar = recipe<CalendarParams>({
  id: "@datars/std/calendar",
  doc: "A calendar heatmap: a week-column × weekday-row grid per year, coloured by value. Several years stack, a row each, labelled at the left.",
  params: { data: t.table(), date: t.field("The date field."), value: t.field(), cell: t.number(0, "Cell size in px (0 = fit the box: 53 weeks across, every year's row down)."), label: t.prop("Day label (tooltip, accessible name); default `date: value`.") },
  tokens: ["muted", "size.label", "sequential"],
  expand(p, cx) {
    // The years (panels, from the first year in the rows), for their labels and to fit the cells.
    const years = cx.table("years", p.data, op.calendar({ date: p.date, cell: 1 }), op.aggregate(["panel"], { first: ["min", p.date] }));
    const rows = `(table.max(${JSON.stringify(years)}, "panel") + 1)`;
    // Room at the left for the year labels; a year's grid is 7 cells tall, and a cell apart.
    const gutter = 'measure("0000", token("size.label")) + 8';
    const cell = p.cell ? String(p.cell) : `max(1, min((box.w - ${gutter}) / 53, box.h > 0 ? box.h / (8 * ${rows} - 1) : box.w))`;
    const tbl = cx.table("calendar", p.data, op.calendar({ date: p.date, cell: e(cell) }));
    return group({
      key: "calendar",
      scales: { c: { type: "sequential", domain: { data: p.data, field: p.value }, range: "$sequential" } },
      children: [
        instances({ key: "days", from: tbl, proto: "rect", x: e(`d.cx + ${gutter}`), y: e("d.cy + d.panel * 8 * d.cell"), w: e("d.cell - 1"), h: e("d.cell - 1"), fill: e(`scale.c(${fld(p.value)})`), label: p.label ?? e(`\`\${formatDate(${fld(p.date)}, "%-d %b %Y")}: \${${fld(p.value)}}\``) }),
        group({ key: "years", children: [repeat(years, text(e('formatDate(d.first, "%Y")'), [0, e(`d.panel * 8 * (${cell})`)], { style: { size: "$size.label", ink: "$muted", baseline: "top" } }))] }),
      ],
    });
  },
});

// ---- facet (small multiples) ------------------------------------------------------------------------

export interface FacetParams { data: string; by: string; columns: number; chart: (Record<string, unknown> & { kind: string }) | null; gap: number; shared: boolean }

export const facet = recipe<FacetParams>({
  id: "@datars/std/facet",
  doc: "Small multiples: the given chart once per value of `by`, in a grid; inside, the chart's `data` is that group's rows (\"@group\"), on scales shared across panels unless `shared: false`.",
  params: {
    data: t.table(), by: t.field(), columns: t.number(3),
    chart: t.json("A chart (e.g. plot(...)) to repeat; its data is replaced by each group.") as never,
    gap: t.number(16),
    shared: t.bool(true, "Every panel on the same x and y scales (the whole table's extent), so panels compare at a glance; off: each fits its own rows."),
  },
  expand(p) {
    const params = p.chart && p.chart.kind === "use" ? (p.chart.params as Record<string, unknown>) : null;
    const share = (axis: "x" | "y") => (p.shared && params && typeof params[axis] === "string" ? { [`${axis}Domain`]: params[`${axis}Domain`] ?? { data: p.data, field: params[axis] } } : {});
    const inner = params ? { ...p.chart, params: { ...params, ...share("x"), ...share("y"), data: "@group", title: undefined } } : p.chart;
    // A grid layout: equal cells, as many rows as the groups need, each a label over the chart.
    return group({
      key: "facet",
      layout: { type: "grid", columns: p.columns, gap: p.gap },
      children: [repeat({ groups: p.data, by: p.by }, group({
        layout: { type: "rows", gap: 4 },
        children: [text(e(`key.name(${fld(p.by)})`), [0, 0], { size: { h: "auto" }, style: { weight: 600, size: "$size.body", ink: "$ink", baseline: "top" } }), inner as never],
      }))],
    });
  },
});

export type { Prop };
