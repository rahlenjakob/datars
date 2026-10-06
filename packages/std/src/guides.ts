// Guides and annotations: legend, title, rule, span, annotate.

import { e, group, recipe, repeat, shape, geom, t, text, Prop, Template } from "@datars/sdk";

export interface LegendParams { scale: string; title: string }

export const legend = recipe<LegendParams>({
  id: "@datars/std/legend",
  doc: "A legend for a colour scale: a swatch and a label per entry, flowing in rows (spaced by the labels' widths, wrapping at the box edge), under an optional title.",
  params: { scale: t.string("color"), title: t.string(undefined, "A title above the entries (and the legend's accessible name).") },
  tokens: ["ink", "ink-2", "size.label"],
  expand(p) {
    const items = repeat({ legend: p.scale }, group({
      semantics: { role: "legend-item", label: e("key.name(d.label)") },
      children: [
        shape(geom.rect({ x: 0, y: 2, w: 10, h: 10, r: 2 }), { fill: e("d.ink") }),
        text(e("key.name(d.label)"), [15, 11], { style: { size: "$size.label", ink: "$ink-2" } }),
      ],
    }));
    const semantics = { role: "legend" as const, label: p.title ?? "Legend" };
    if (!p.title) return group({ key: `legend-${p.scale}`, semantics, layout: { type: "flow", gap: 14 }, children: [items] });
    // Titled: the title on a line of its own, the entries flowing under it.
    return group({
      key: `legend-${p.scale}`,
      semantics,
      layout: { type: "rows", gap: 4 },
      children: [
        text(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "top", maxWidth: e("box.w") } }),
        group({ key: "items", size: { h: "auto" }, layout: { type: "flow", gap: 14 }, children: [items] }),
      ],
    });
  },
});

export interface TitleParams { text: string; subtitle: string; source: string }

export const title = recipe<TitleParams>({
  id: "@datars/std/title",
  doc: "A title (and optional subtitle and source line), each wrapped to the box: on a phone a long line takes two instead of running off.",
  params: { text: t.string(""), subtitle: t.string(), source: t.string() },
  expand(p) {
    return group({
      key: "title",
      // As tall as its lines: in a document's rows, the plot below gets the rest.
      size: { h: "auto" },
      layout: { type: "rows", gap: 4 },
      children: [
        // Wrapped to the box: a long title on a phone takes two lines instead of running off.
        text(p.text, [0, 0], { size: { h: "auto" }, style: { font: "font.title", size: "$size.title", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: p.text } }),
        p.subtitle ? text(p.subtitle, [0, 0], { size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }) : null,
        p.source ? text(p.source, [0, 0], { size: { h: "auto" }, style: { size: "$size.small", ink: "$muted", baseline: "top", maxWidth: e("box.w") } }) : null,
      ],
    });
  },
});

export interface RuleParams { axis: "x" | "y"; value: Prop; label: string; ink: string; dashed: boolean }

export const rule = recipe<RuleParams>({
  id: "@datars/std/rule",
  doc: "A reference line at a value on the x or y scale (a target, an average, 'today').",
  params: {
    axis: t.oneOf(["x", "y"] as const, "y"),
    value: t.prop("Where: a number, a category, or an expression (`e(\"table.mean('t', 'v')\")`, a signal) — the line moves when it changes."),
    label: t.string(), ink: t.ink("$ink-2"), dashed: t.bool(true),
  },
  expand(p) {
    const pos = e(`scale.${p.axis}(${valueSource(p.value)}) + scale.${p.axis}.bandwidth() / 2`);
    const g = p.axis === "y" ? geom.segment({ x1: 0, y1: pos, x2: e("box.w"), y2: pos }) : geom.segment({ x1: pos, y1: 0, x2: pos, y2: e("box.h") });
    // An x line's label goes right of the line, unless it would run out of the box there: then
    // left of it (like `annotate`), so a line near the right edge of a narrow plot keeps it inside.
    const flip = p.label ? `((${pos.expr}) + 4 + measure(${JSON.stringify(p.label)}, token("size.label")) > box.w)` : "false";
    return group({
      key: `rule-${p.axis}-${valueName(p.value)}`,
      // Unlabelled, it reads as its value (an expression's, as it is now).
      semantics: { role: "annotation", label: p.label ?? (isExprValue(p.value) ? e(`${JSON.stringify(`${p.axis} = `)} + ${valueSource(p.value)}`) : `${p.axis} = ${valueName(p.value)}`) },
      children: [
        shape(g, { stroke: { paint: p.ink, width: 1, dash: p.dashed ? [4, 3] : undefined } }),
        p.label
          ? p.axis === "y"
            ? text(p.label, [e("box.w"), e(`(${pos.expr}) - 4`)], { style: { size: "$size.label", ink: p.ink, align: "end" } })
            : text(p.label, [e(`${flip} ? (${pos.expr}) - 4 : (${pos.expr}) + 4`), 10], { style: { size: "$size.label", ink: p.ink, align: e(`${flip} ? "end" : "start"`) } })
          : null,
      ],
    });
  },
});

export interface SpanParams { axis: "x" | "y"; from: Prop; to: Prop; label: string; ink: string; opacity: number }

export const span = recipe<SpanParams>({
  id: "@datars/std/span",
  doc: "A shaded band between two values on the x or y scale (a recession, a target range).",
  params: {
    axis: t.oneOf(["x", "y"] as const, "x"),
    from: t.prop("Where the band starts: a number, a category, or an expression."), to: t.prop("Where it ends (a category: the end of its band)."),
    label: t.string(), ink: t.ink("$ink"), opacity: t.number(0.07),
  },
  expand(p) {
    const a = `scale.${p.axis}(${valueSource(p.from)})`;
    const b = `scale.${p.axis}(${valueSource(p.to)}) + scale.${p.axis}.bandwidth()`;
    const g = p.axis === "x"
      ? geom.rect({ x: e(`min(${a}, ${b})`), y: 0, w: e(`abs(${b} - (${a}))`), h: e("box.h") })
      : geom.rect({ x: 0, y: e(`min(${a}, ${b})`), w: e("box.w"), h: e(`abs(${b} - (${a}))`) });
    return group({
      key: `span-${p.axis}-${valueName(p.from)}`,
      z: -1,
      semantics: { role: "annotation", label: p.label ?? "" },
      children: [
        shape(g, { fill: p.ink, opacity: p.opacity }),
        p.label ? text(p.label, p.axis === "x" ? [e(`min(${a}, ${b}) + 4`), 12] : [4, e(`min(${a}, ${b}) + 12`)], { style: { size: "$size.label", ink: "$muted" } }) : null,
      ],
    });
  },
});

/** A value on a scale as expression source: a number, a category (a string), or an expression
 * (`e(…)`, or an "=…" string in hand-written JSON). */
function valueSource(v: Prop): string {
  if (typeof v === "number") return String(v);
  if (v && typeof v === "object" && "expr" in v) return `(${(v as { expr: string }).expr})`;
  if (typeof v === "string") return v.startsWith("=") ? `(${v.slice(1)})` : JSON.stringify(v);
  return "0";
}

const isExprValue = (v: Prop) => (!!v && typeof v === "object" && "expr" in v) || (typeof v === "string" && v.startsWith("="));

/** A value as it reads in a key or a label: the number, the category or the expression's source. */
function valueName(v: Prop): string {
  if (v && typeof v === "object" && "expr" in v) return String((v as { expr: string }).expr);
  if (typeof v === "string" && v.startsWith("=")) return v.slice(1);
  return String(v);
}

export interface AnnotateParams { x: Prop; y: Prop; text: string; dx: number; dy: number; connector: "line" | "elbow" | "curve" | "none"; head: boolean; dot: boolean; width: number; ink: string; pin: boolean }

export const annotate = recipe<AnnotateParams>({
  id: "@datars/std/annotate",
  doc: "A callout: text offset from a point (in plot coordinates via scales, or px), with a connector and a dot. On a line's point it moves with the line in transitions. Keyed (`annotate({…}, { key: \"note\" })`), one note carries across steps: from point to point along the line it sits on.",
  params: {
    x: t.prop("x position (expression, e.g. scale.x('SD'))"), y: t.prop(), text: t.string(""),
    dx: t.number(40), dy: t.number(-36), connector: t.oneOf(["line", "elbow", "curve", "none"] as const, "line"), head: t.bool(false, "An arrowhead at the point."), dot: t.bool(true), width: t.number(160), ink: t.ink("$ink"),
    pin: t.bool(false, "Pinned at the point: dot, connector and text keep screen size under cameras (map callouts)."),
  },
  tokens: ["ink", "paper", "size.label"],
  expand(p) {
    if (p.pin) {
      // The callout in screen px around its anchor; only the anchor moves with cameras.
      const conn = p.connector === "none" ? null : p.connector === "line" && !p.head ? shape(geom.segment({ x1: 0, y1: 0, x2: p.dx, y2: p.dy + (p.dy < 0 ? 4 : -12) }), { key: "connector", stroke: { paint: p.ink, width: 1 } }) : connector(p, "0", "0", String(p.dx), String(p.dy + (p.dy < 0 ? 4 : -12)));
      return group({
        key: `note-${p.text.slice(0, 24)}`,
        pin: true,
        transform: { translate: [p.x as never, p.y as never] },
        semantics: { role: "annotation", label: p.text },
        children: [
          conn,
          p.dot ? shape(geom.circle({ cx: 0, cy: 0, r: 3 }), { key: "dot", fill: p.ink }) : null,
          text(p.text, [p.dx, p.dy], { key: "text", style: { size: "$size.label", weight: 600, ink: p.ink, maxWidth: p.width, align: p.dx < 0 ? "end" : "start", baseline: p.dy < 0 ? "bottom" : "top" }, halo: ["$paper", 3] }),
        ],
      });
    }
    const x = p.x as never, y = p.y as never;
    // The text goes to the side `dx` asks for, unless it would run out of the box there: then it
    // mirrors to the other side of its point (a callout on the last point of a line stays inside).
    const w = `min(measure(${JSON.stringify(p.text)}, token("size.label"), 600), ${p.width})`;
    const flip = p.dx >= 0 ? `((${exprOf(x)}) + ${p.dx} + ${w} > box.w + 4)` : `((${exprOf(x)}) + ${p.dx} - ${w} < -4)`;
    const dxs = `(${flip} ? ${-p.dx} : ${p.dx})`;
    // Likewise up and down: text above a point near the top of the plot (a peak) would run into
    // the title, so it goes below the point instead — and text below one near the bottom, above.
    const size = 'token("size.label")';
    const h = `(ceil(measure(${JSON.stringify(p.text)}, ${size}, 600) / ${p.width}) * ${size} * 1.25)`;
    const flipY = p.dy < 0 ? `((${exprOf(y)}) + ${p.dy} - ${h} < -2)` : `((${exprOf(y)}) + ${p.dy} + ${h} > box.h + 2)`;
    const dys = `(${flipY} ? ${-p.dy} : ${p.dy})`;
    const tx = e(`(${exprOf(x)}) + ${dxs}`), ty = e(`(${exprOf(y)}) + ${dys}`);
    // The connector stops just short of the text: under it when the text is above, over it when below.
    const end = `(${exprOf(y)}) + ${dys} + (${flipY} ? ${p.dy < 0 ? -4 : 4} : ${p.dy < 0 ? 4 : -12})`;
    const conn = p.connector === "none" ? null
      : p.connector === "line" && !p.head
        ? shape(geom.segment({ x1: x, y1: y, x2: tx, y2: e(end) }), { key: "connector", stroke: { paint: p.ink, width: 1 } })
        : connector(p, exprOf(x), exprOf(y), `(${exprOf(x)}) + ${dxs}`, end);
    const align = p.dx < 0 ? e(`${flip} ? "start" : "end"`) : e(`${flip} ? "end" : "start"`);
    const baseline = p.dy < 0 ? e(`${flipY} ? "top" : "bottom"`) : e(`${flipY} ? "bottom" : "top"`);
    return group({
      key: `note-${p.text.slice(0, 24)}`,
      semantics: { role: "annotation", label: p.text },
      children: [
        conn,
        p.dot ? shape(geom.circle({ cx: x, cy: y, r: 3 }), { key: "dot", fill: p.ink }) : null,
        text(p.text, [tx, ty], { key: "text", style: { size: "$size.label", weight: 600, ink: p.ink, maxWidth: p.width, align, contain: true, baseline }, halo: ["$paper", 3] }),
      ],
    });
  },
});

/** The connector from the point (ax, ay) to the text (tx, ty) as a path: straight, elbow (along
 * y first, then x) or a curve — with an arrowhead at the point when asked. */
function connector(p: AnnotateParams, ax: string, ay: string, tx: string, ty: string): Template {
  const d = p.connector === "elbow"
    ? `\`M \${${ax}} \${${ay}} L \${${ax}} \${${ty}} L \${${tx}} \${${ty}}\``
    : p.connector === "curve"
      ? `\`M \${${ax}} \${${ay}} Q \${${ax}} \${${ty}} \${${tx}} \${${ty}}\``
      : `\`M \${${ax}} \${${ay}} L \${${tx}} \${${ty}}\``;
  return shape(geom.path(e(d)), { key: "connector", stroke: { paint: p.ink, width: 1 }, markers: p.head ? { start: { type: "arrow", size: 6 } } : undefined });
}

function exprOf(v: unknown): string {
  if (typeof v === "number") return String(v);
  if (v && typeof v === "object" && "expr" in (v as object)) return (v as { expr: string }).expr;
  if (typeof v === "string") return v.startsWith("=") ? v.slice(1) : v;
  return "0";
}

// ---- card -----------------------------------------------------------------------------------------

export interface CardParams { text: Prop; title: Prop; kicker: Prop; at: Prop; width: number; margin: number | [number, number, number, number]; dodge: boolean }

const CARD_AT = ["auto", "top-left", "top", "top-right", "left", "center", "right", "bottom-left", "bottom", "bottom-right"] as const;
// Where `auto` looks, in order of preference (the reading order: a note starts top-left).
const CARD_AUTO = ["top-right", "top-left", "bottom-right", "bottom-left", "right", "left", "top", "bottom"];

export const card = recipe<CardParams>({
  id: "@datars/std/card",
  doc: "A text card over the chart: an optional kicker and title over wrapped body text, on the theme's card surface, anchored to a corner or edge of its box. Drawn by the engine (so it's in video, PNG and PDF too); empty text hides it, and a card whose text changes between states morphs.",
  params: {
    text: t.prop("The body text (a string, or an expression over signals: one card narrating every state)."),
    title: t.prop("A bold line above the text (empty: no line)."), kicker: t.prop("A small accent line above the title (empty: no line)."),
    at: t.oneOf(CARD_AT, "auto", "Where the card sits in its box — or an expression over signals, for a card that moves between states. With `dodge`, `auto` (and any anchor, as a first choice) moves it to where it covers the least data and text."),
    width: t.number(260, "Card width (px); text wraps inside."), margin: t.number(12, "Space between the card and its box's edges (px, or [top, right, bottom, left])."),
    dodge: t.bool(true, "Move out of the way of data and text (taking a band below the chart when nowhere is free). Off: the card stays at `at` (`auto`: top-right), over whatever is there — a card over a map, where land covers everything."),
  },
  tokens: ["card", "card-ink", "card-ink-2", "card-line", "radius.card", "accent", "size.body", "size.small"],
  expand(params) {
    // Expressions as `{ expr }` (the SDK's `e()`) or, in hand-written JSON, as "=…" strings.
    const norm = (v: Prop): Prop => (typeof v === "string" && v.startsWith("=") ? e(v.slice(1)) : v);
    const p = { ...params, text: norm(params.text), title: norm(params.title), kicker: norm(params.kicker), at: norm(params.at) };
    const pad = 14;
    // As wide as asked, but never wider than the box leaves (a 300 px card in a 320 px phone chart).
    const m = Array.isArray(p.margin) ? p.margin : [p.margin, p.margin, p.margin, p.margin];
    const width = e(`min(${p.width}, box.w - ${m[1] + m[3]})`);
    const isExpr = (v: unknown): v is { expr: string } => typeof v === "object" && v !== null && "expr" in v;
    const exprOf = (v: Prop) => (isExpr(v) ? v.expr : JSON.stringify(v ?? ""));
    const hidden = isExpr(p.text) ? e(`(${exprOf(p.text)}) != ""`) : undefined;
    // A kicker or title that is empty in a state isn't drawn: an empty line would still stand at the
    // top of the card (the backdrop hugging it, the gap after it). It fades in when a state gives
    // it text.
    const line = (key: string, content: Prop, style: Record<string, unknown>, optional = false) =>
      text(content, [0, 0], { key, when: optional && isExpr(content) ? e(`(${exprOf(content)}) != ""`) : undefined, size: { h: "auto" }, style: { baseline: "top", maxWidth: e("box.w"), ...style } });
    // A card whose text changes is a new card: it fades out where it was and in where it lands,
    // instead of flying an empty box across the chart (the texts can't travel with it).
    const boxKey = isExpr(p.text) ? e(`"box:" + (${exprOf(p.text)})`) : "box";
    const box = group({
      key: boxKey,
      size: { w: width, h: "auto" },
      layout: { type: "rows", gap: 6, padding: pad },
      backdrop: { fill: "$card", stroke: { paint: "$card-line", width: 1 }, radius: e('token("radius.card")'), padding: pad, fit: "width" },
      semantics: { role: "annotation", label: p.text },
      children: [
        p.kicker ? line("kicker", p.kicker, { size: "$size.small", weight: 700, ink: "$accent" }, true) : null,
        p.title ? line("title", p.title, { size: e('token("size.body") + 2'), weight: 600, ink: "$card-ink" }, true) : null,
        line("text", p.text, { size: "$size.body", ink: "$card-ink-2" }),
      ],
    });
    // Placed after layout (the engine's `dodge`): the asked-for anchor first, then the others, so
    // the card never sits on the data it talks about. Not dodging, its anchor is its only place
    // (the engine keeps a single anchor fixed). An expression anchor is evaluated per state, so
    // the card moves between states (the engine tries each anchor once).
    const at = isExpr(p.at) ? e(`(${p.at.expr}) == "auto" ? ${JSON.stringify(CARD_AUTO[0])} : (${p.at.expr})`) : p.at === "auto" ? CARD_AUTO[0] : String(p.at);
    const dodge = !p.dodge ? [at] : p.at === "auto" ? CARD_AUTO : [at, ...CARD_AUTO.filter((a) => a !== p.at)];
    return group({ key: "card", when: hidden, layout: { type: "stack", padding: p.margin, align: "start" }, dodge, children: [box] });
  },
});
