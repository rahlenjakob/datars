// src/plot.ts
import { brush, e as e7, geom as geom7, group as group7, op as op5, recipe as recipe7, shape as shape7, t as t7, text as text6 } from "@datars/sdk";

// src/axis.ts
import { e, group, op, recipe, repeat, shape, geom, t, text } from "@datars/sdk";
var axis = recipe({
  id: "@datars/std/axis",
  doc: "An axis for a scale: tick labels, tick marks and a domain line. Labels never pile up: a horizontal numeric axis asks for as many ticks as its measured labels fit; category names wrap in their band, turn 45\xB0 when a word is wider than the band, and leave out every k-th name only when even that doesn't fit.",
  params: {
    scale: t.string("y", "The scale to draw."),
    orient: t.oneOf(["left", "bottom", "right", "top"], "left"),
    ticks: t.number(0, "Tick count hint (0 = automatic: from the axis length, and across, from how many of its measured labels fit)."),
    format: t.string(void 0, "Number format for labels (d3-format)."),
    label: t.string(void 0, "Axis title: under the end of a horizontal axis; above the top of a vertical one, over its labels (a plot draws `yLabel` above its top-left corner instead, in a row of its own)."),
    line: t.bool(true, "Draw the domain line."),
    prefix: t.string(void 0, "Before each number label ('$')."),
    suffix: t.string(void 0, "After it ('M', ' kr')."),
    data: t.table("The table a band scale's categories come from (with `field`): their names are measured, so a horizontal axis wraps, turns or leaves out labels only as they need."),
    field: t.field("The categories' field in `data`."),
    type: t.string(void 0, "The scale's type (`linear`, `log`, `time`, \u2026; the plot passes it). A horizontal numeric axis fits its tick count to its labels' measured width.")
  },
  tokens: ["muted", "rule", "size.label"],
  expand(p, cx) {
    const s = p.scale;
    const S = `scale.${s}`;
    const center = `d.pos + ${S}.bandwidth() / 2`;
    const affix = (n) => p.prefix || p.suffix ? `${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}` : n;
    const num = p.format ? `format(d.value, ${JSON.stringify(p.format)})` : "d.label";
    const plain = affix(num);
    const band = `d.kind == "category"`;
    const label = p.format ? plain : `${band} ? key.name(d.label) : d.label`;
    const vertical = p.orient === "left" || p.orient === "right";
    const sign = p.orient === "left" || p.orient === "top" ? -1 : 1;
    const size = `token("size.label")`;
    const len = `(${S}.max() - ${S}.min())`;
    const numeric = fitsTicks(p);
    const widest = widestTick(p);
    const from = { ticks: s, count: tickCount(p) };
    const spaced = numeric ? `d.index % max(1, ceil((${widest} + 24) * max(1, d.count - 1) / max(${len}, 1))) == 0` : "true";
    const whole = p.format && wholeNumbers(p.format) ? e(`${band} || abs(d.value - round(d.value)) < 0.000001`) : void 0;
    const tickMark = vertical ? shape(geom.segment({ x1: 0, y1: e(center), x2: sign * 5, y2: e(center) }), { stroke: { paint: "$rule", width: 1 } }) : shape(geom.segment({ x1: e(center), y1: 0, x2: e(center), y2: sign * 5 }), { stroke: { paint: "$rule", width: 1 } });
    const names = !vertical && p.data && p.field ? cx.table("names", p.data, op.derive("w", e(`measure(key.name(d.${p.field}), ${size})`)), op.derive("ww", e(`measure.word(key.name(d.${p.field}), ${size})`))) : null;
    const step = `${S}.step()`;
    const W = names ? `table.max(${JSON.stringify(names)}, "w")` : "36";
    const WW = names ? `table.max(${JSON.stringify(names)}, "ww")` : "36";
    const kFlat = names ? `max(1, ceil((${WW} + 4) / ${step}))` : `max(1, ceil(40 / ${step}))`;
    const kTurned = `max(1, ceil((${size} + 3) * 1.42 / ${step}))`;
    const turned = names && sign > 0 ? `(${WW} > 3.5 * ${size} && ${kFlat} > 1 && ${kTurned} * 1.5 < ${kFlat})` : "false";
    const k = vertical ? `max(1, ceil((${size} + 1) / ${step}))` : `(${turned} ? ${kTurned} : ${kFlat})`;
    const thin = `d.index % ${k} == 0`;
    const flatWidth = names ? `${kFlat} * ${step} - 4` : `max(${step} - 4, 36)`;
    const lead = "round(abs(d.value) / pow(10, floor(log10(abs(d.value)))))";
    const shown = `!(${band}) && (d.kind != "log" || d.major || ${lead} == 2 || ${lead} == 5)`;
    const tickLabel = vertical ? text(e(plain), [sign * 8, e(center)], { when: e(shown), style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "middle" } }) : text(e(plain), [e(center), sign < 0 ? -8 : 8], { when: e(`${shown} && ${spaced}`), style: { size: "$size.label", ink: "$muted", align: "middle", contain: true, baseline: sign < 0 ? "bottom" : "top" } });
    const catLabel = vertical ? text(e(label), [sign * 8, e(center)], { when: e(`${band} && ${thin}`), style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "middle" } }) : text(e(label), [e(center), sign < 0 ? -8 : 8], { when: e(`${band} && ${thin} && !${turned}`), style: { size: "$size.label", ink: "$muted", align: "middle", contain: true, baseline: sign < 0 ? "bottom" : "top", maxWidth: e(flatWidth) } });
    const turnedLabel = !vertical && names && sign > 0 ? text(e(label), [e(center), 8], { key: "turned", when: e(`${band} && ${thin} && ${turned}`), rotate: -45, style: { size: "$size.label", ink: "$muted", align: "end", contain: true, baseline: "middle" } }) : null;
    const domainLine = vertical ? shape(geom.segment({ x1: 0, y1: e(`${S}.min()`), x2: 0, y2: e(`${S}.max()`) }), { key: "domain", stroke: { paint: "$rule", width: 1 } }) : shape(geom.segment({ x1: e(`${S}.min()`), y1: 0, x2: e(`${S}.max()`), y2: 0 }), { key: "domain", stroke: { paint: "$rule", width: 1 } });
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
        p.label ? vertical ? text(p.label, [sign * 8, e(top)], { key: "label", style: { size: "$size.label", ink: "$muted", align: sign < 0 ? "end" : "start", baseline: "bottom" } }) : text(p.label, [e(`${S}.max()`), titleY], { key: "label", style: { size: "$size.label", ink: "$muted", align: "end", baseline: "top" } }) : null
      ]
    });
  }
});
function fitsTicks(p) {
  return (p.orient === "bottom" || p.orient === "top") && !!p.type && !["band", "point", "time"].includes(p.type);
}
function widestTick(p) {
  const S = `scale.${p.scale}`, size = `token("size.label")`;
  const written2 = (v) => {
    const n = p.format ? `format(${v}, ${JSON.stringify(p.format)})` : `${S}.label(${v})`;
    return p.prefix || p.suffix ? `${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}` : n;
  };
  return `max(measure(${written2(`${S}.invert(${S}.min())`)}, ${size}), measure(${written2(`${S}.invert(${S}.max())`)}, ${size}))`;
}
function tickCount(p) {
  if (p.ticks) return p.ticks;
  if (!fitsTicks(p)) return void 0;
  const len = `(scale.${p.scale}.max() - scale.${p.scale}.min())`;
  return e(`max(2, min(10, round(${len} / 64), floor(${len} / (${widestTick(p)} + 30))))`);
}
function wholeNumbers(spec) {
  const m = /(?:\.(\d+))?~?([a-z%])?$/i.exec(spec);
  return !!m && (m[2] === "d" || m[2] === "f" && m[1] === "0");
}

// src/grid.ts
import { e as e2, group as group2, recipe as recipe2, repeat as repeat2, shape as shape2, geom as geom2, t as t2 } from "@datars/sdk";
var grid = recipe2({
  id: "@datars/std/grid",
  doc: "Gridlines at a scale's ticks across the plot area.",
  params: {
    scale: t2.string("y"),
    orient: t2.oneOf(["horizontal", "vertical"], "horizontal"),
    ticks: t2.number(0),
    count: t2.prop("A tick count as an expression \u2014 the axis's own (a plot passes it), so every gridline meets a tick.")
  },
  tokens: ["grid", "stroke.grid"],
  expand(p) {
    const pos = `d.pos + scale.${p.scale}.bandwidth() / 2`;
    const line2 = p.orient === "horizontal" ? geom2.segment({ x1: 0, y1: e2(pos), x2: e2("box.w"), y2: e2(pos) }) : geom2.segment({ x1: e2(pos), y1: 0, x2: e2(pos), y2: e2("box.h") });
    return group2({
      key: `grid-${p.scale}`,
      z: -1,
      semantics: { role: "decoration" },
      children: [repeat2({ ticks: p.scale, count: p.count ?? (p.ticks || void 0) }, shape2(line2, { stroke: { paint: "$grid", width: "$stroke.grid" }, semantics: { role: "decoration" } }))]
    });
  }
});

// src/guides.ts
import { e as e3, group as group3, recipe as recipe3, repeat as repeat3, shape as shape3, geom as geom3, t as t3, text as text2 } from "@datars/sdk";
var legend = recipe3({
  id: "@datars/std/legend",
  doc: "A legend for a colour scale: a swatch and a label per entry, flowing in rows (spaced by the labels' widths, wrapping at the box edge), under an optional title.",
  params: { scale: t3.string("color"), title: t3.string(void 0, "A title above the entries (and the legend's accessible name).") },
  tokens: ["ink", "ink-2", "size.label"],
  expand(p) {
    const items = repeat3({ legend: p.scale }, group3({
      semantics: { role: "legend-item", label: e3("key.name(d.label)") },
      children: [
        shape3(geom3.rect({ x: 0, y: 2, w: 10, h: 10, r: 2 }), { fill: e3("d.ink") }),
        text2(e3("key.name(d.label)"), [15, 11], { style: { size: "$size.label", ink: "$ink-2" } })
      ]
    }));
    const semantics = { role: "legend", label: p.title ?? "Legend" };
    if (!p.title) return group3({ key: `legend-${p.scale}`, semantics, layout: { type: "flow", gap: 14 }, children: [items] });
    return group3({
      key: `legend-${p.scale}`,
      semantics,
      layout: { type: "rows", gap: 4 },
      children: [
        text2(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "top", maxWidth: e3("box.w") } }),
        group3({ key: "items", size: { h: "auto" }, layout: { type: "flow", gap: 14 }, children: [items] })
      ]
    });
  }
});
var title = recipe3({
  id: "@datars/std/title",
  doc: "A title (and optional subtitle and source line), each wrapped to the box: on a phone a long line takes two instead of running off.",
  params: { text: t3.string(""), subtitle: t3.string(), source: t3.string() },
  expand(p) {
    return group3({
      key: "title",
      // As tall as its lines: in a document's rows, the plot below gets the rest.
      size: { h: "auto" },
      layout: { type: "rows", gap: 4 },
      children: [
        // Wrapped to the box: a long title on a phone takes two lines instead of running off.
        text2(p.text, [0, 0], { size: { h: "auto" }, style: { font: "font.title", size: "$size.title", baseline: "top", maxWidth: e3("box.w") }, semantics: { role: "title", label: p.text } }),
        p.subtitle ? text2(p.subtitle, [0, 0], { size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e3("box.w") } }) : null,
        p.source ? text2(p.source, [0, 0], { size: { h: "auto" }, style: { size: "$size.small", ink: "$muted", baseline: "top", maxWidth: e3("box.w") } }) : null
      ]
    });
  }
});
var rule = recipe3({
  id: "@datars/std/rule",
  doc: "A reference line at a value on the x or y scale (a target, an average, 'today').",
  params: {
    axis: t3.oneOf(["x", "y"], "y"),
    value: t3.prop("Where: a number, a category, or an expression (`e(\"table.mean('t', 'v')\")`, a signal) \u2014 the line moves when it changes."),
    label: t3.string(),
    ink: t3.ink("$ink-2"),
    dashed: t3.bool(true)
  },
  expand(p) {
    const pos = e3(`scale.${p.axis}(${valueSource(p.value)}) + scale.${p.axis}.bandwidth() / 2`);
    const g = p.axis === "y" ? geom3.segment({ x1: 0, y1: pos, x2: e3("box.w"), y2: pos }) : geom3.segment({ x1: pos, y1: 0, x2: pos, y2: e3("box.h") });
    const flip = p.label ? `((${pos.expr}) + 4 + measure(${JSON.stringify(p.label)}, token("size.label")) > box.w)` : "false";
    return group3({
      key: `rule-${p.axis}-${valueName(p.value)}`,
      // Unlabelled, it reads as its value (an expression's, as it is now).
      semantics: { role: "annotation", label: p.label ?? (isExprValue(p.value) ? e3(`${JSON.stringify(`${p.axis} = `)} + ${valueSource(p.value)}`) : `${p.axis} = ${valueName(p.value)}`) },
      children: [
        shape3(g, { stroke: { paint: p.ink, width: 1, dash: p.dashed ? [4, 3] : void 0 } }),
        p.label ? p.axis === "y" ? text2(p.label, [e3("box.w"), e3(`(${pos.expr}) - 4`)], { style: { size: "$size.label", ink: p.ink, align: "end" } }) : text2(p.label, [e3(`${flip} ? (${pos.expr}) - 4 : (${pos.expr}) + 4`), 10], { style: { size: "$size.label", ink: p.ink, align: e3(`${flip} ? "end" : "start"`) } }) : null
      ]
    });
  }
});
var span = recipe3({
  id: "@datars/std/span",
  doc: "A shaded band between two values on the x or y scale (a recession, a target range).",
  params: {
    axis: t3.oneOf(["x", "y"], "x"),
    from: t3.prop("Where the band starts: a number, a category, or an expression."),
    to: t3.prop("Where it ends (a category: the end of its band)."),
    label: t3.string(),
    ink: t3.ink("$ink"),
    opacity: t3.number(0.07)
  },
  expand(p) {
    const a = `scale.${p.axis}(${valueSource(p.from)})`;
    const b = `scale.${p.axis}(${valueSource(p.to)}) + scale.${p.axis}.bandwidth()`;
    const g = p.axis === "x" ? geom3.rect({ x: e3(`min(${a}, ${b})`), y: 0, w: e3(`abs(${b} - (${a}))`), h: e3("box.h") }) : geom3.rect({ x: 0, y: e3(`min(${a}, ${b})`), w: e3("box.w"), h: e3(`abs(${b} - (${a}))`) });
    return group3({
      key: `span-${p.axis}-${valueName(p.from)}`,
      z: -1,
      semantics: { role: "annotation", label: p.label ?? "" },
      children: [
        shape3(g, { fill: p.ink, opacity: p.opacity }),
        p.label ? text2(p.label, p.axis === "x" ? [e3(`min(${a}, ${b}) + 4`), 12] : [4, e3(`min(${a}, ${b}) + 12`)], { style: { size: "$size.label", ink: "$muted" } }) : null
      ]
    });
  }
});
function valueSource(v) {
  if (typeof v === "number") return String(v);
  if (v && typeof v === "object" && "expr" in v) return `(${v.expr})`;
  if (typeof v === "string") return v.startsWith("=") ? `(${v.slice(1)})` : JSON.stringify(v);
  return "0";
}
var isExprValue = (v) => !!v && typeof v === "object" && "expr" in v || typeof v === "string" && v.startsWith("=");
function valueName(v) {
  if (v && typeof v === "object" && "expr" in v) return String(v.expr);
  if (typeof v === "string" && v.startsWith("=")) return v.slice(1);
  return String(v);
}
var annotate = recipe3({
  id: "@datars/std/annotate",
  doc: 'A callout: text offset from a point (in plot coordinates via scales, or px), with a connector and a dot. On a line\'s point it moves with the line in transitions. Keyed (`annotate({\u2026}, { key: "note" })`), one note carries across steps: from point to point along the line it sits on.',
  params: {
    x: t3.prop("x position (expression, e.g. scale.x('SD'))"),
    y: t3.prop(),
    text: t3.string(""),
    dx: t3.number(40),
    dy: t3.number(-36),
    connector: t3.oneOf(["line", "elbow", "curve", "none"], "line"),
    head: t3.bool(false, "An arrowhead at the point."),
    dot: t3.bool(true),
    width: t3.number(160),
    ink: t3.ink("$ink"),
    pin: t3.bool(false, "Pinned at the point: dot, connector and text keep screen size under cameras (map callouts).")
  },
  tokens: ["ink", "paper", "size.label"],
  expand(p) {
    if (p.pin) {
      const conn2 = p.connector === "none" ? null : p.connector === "line" && !p.head ? shape3(geom3.segment({ x1: 0, y1: 0, x2: p.dx, y2: p.dy + (p.dy < 0 ? 4 : -12) }), { key: "connector", stroke: { paint: p.ink, width: 1 } }) : connector(p, "0", "0", String(p.dx), String(p.dy + (p.dy < 0 ? 4 : -12)));
      return group3({
        key: `note-${p.text.slice(0, 24)}`,
        pin: true,
        transform: { translate: [p.x, p.y] },
        semantics: { role: "annotation", label: p.text },
        children: [
          conn2,
          p.dot ? shape3(geom3.circle({ cx: 0, cy: 0, r: 3 }), { key: "dot", fill: p.ink }) : null,
          text2(p.text, [p.dx, p.dy], { key: "text", style: { size: "$size.label", weight: 600, ink: p.ink, maxWidth: p.width, align: p.dx < 0 ? "end" : "start", baseline: p.dy < 0 ? "bottom" : "top" }, halo: ["$paper", 3] })
        ]
      });
    }
    const x = p.x, y = p.y;
    const w = `min(measure(${JSON.stringify(p.text)}, token("size.label"), 600), ${p.width})`;
    const flip = p.dx >= 0 ? `((${exprOf(x)}) + ${p.dx} + ${w} > box.w + 4)` : `((${exprOf(x)}) + ${p.dx} - ${w} < -4)`;
    const dxs = `(${flip} ? ${-p.dx} : ${p.dx})`;
    const size = 'token("size.label")';
    const h = `(ceil(measure(${JSON.stringify(p.text)}, ${size}, 600) / ${p.width}) * ${size} * 1.25)`;
    const flipY = p.dy < 0 ? `((${exprOf(y)}) + ${p.dy} - ${h} < -2)` : `((${exprOf(y)}) + ${p.dy} + ${h} > box.h + 2)`;
    const dys = `(${flipY} ? ${-p.dy} : ${p.dy})`;
    const tx = e3(`(${exprOf(x)}) + ${dxs}`), ty = e3(`(${exprOf(y)}) + ${dys}`);
    const end = `(${exprOf(y)}) + ${dys} + (${flipY} ? ${p.dy < 0 ? -4 : 4} : ${p.dy < 0 ? 4 : -12})`;
    const conn = p.connector === "none" ? null : p.connector === "line" && !p.head ? shape3(geom3.segment({ x1: x, y1: y, x2: tx, y2: e3(end) }), { key: "connector", stroke: { paint: p.ink, width: 1 } }) : connector(p, exprOf(x), exprOf(y), `(${exprOf(x)}) + ${dxs}`, end);
    const align = p.dx < 0 ? e3(`${flip} ? "start" : "end"`) : e3(`${flip} ? "end" : "start"`);
    const baseline = p.dy < 0 ? e3(`${flipY} ? "top" : "bottom"`) : e3(`${flipY} ? "bottom" : "top"`);
    return group3({
      key: `note-${p.text.slice(0, 24)}`,
      semantics: { role: "annotation", label: p.text },
      children: [
        conn,
        p.dot ? shape3(geom3.circle({ cx: x, cy: y, r: 3 }), { key: "dot", fill: p.ink }) : null,
        text2(p.text, [tx, ty], { key: "text", style: { size: "$size.label", weight: 600, ink: p.ink, maxWidth: p.width, align, contain: true, baseline }, halo: ["$paper", 3] })
      ]
    });
  }
});
function connector(p, ax, ay, tx, ty) {
  const d = p.connector === "elbow" ? `\`M \${${ax}} \${${ay}} L \${${ax}} \${${ty}} L \${${tx}} \${${ty}}\`` : p.connector === "curve" ? `\`M \${${ax}} \${${ay}} Q \${${ax}} \${${ty}} \${${tx}} \${${ty}}\`` : `\`M \${${ax}} \${${ay}} L \${${tx}} \${${ty}}\``;
  return shape3(geom3.path(e3(d)), { key: "connector", stroke: { paint: p.ink, width: 1 }, markers: p.head ? { start: { type: "arrow", size: 6 } } : void 0 });
}
function exprOf(v) {
  if (typeof v === "number") return String(v);
  if (v && typeof v === "object" && "expr" in v) return v.expr;
  if (typeof v === "string") return v.startsWith("=") ? v.slice(1) : v;
  return "0";
}
var CARD_AT = ["auto", "top-left", "top", "top-right", "left", "center", "right", "bottom-left", "bottom", "bottom-right"];
var CARD_AUTO = ["top-right", "top-left", "bottom-right", "bottom-left", "right", "left", "top", "bottom"];
var card = recipe3({
  id: "@datars/std/card",
  doc: "A text card over the chart: an optional kicker and title over wrapped body text, on the theme's card surface, anchored to a corner or edge of its box. Drawn by the engine (so it's in video, PNG and PDF too); empty text hides it, and a card whose text changes between states morphs.",
  params: {
    text: t3.prop("The body text (a string, or an expression over signals: one card narrating every state)."),
    title: t3.prop("A bold line above the text (empty: no line)."),
    kicker: t3.prop("A small accent line above the title (empty: no line)."),
    at: t3.oneOf(CARD_AT, "auto", "Where the card sits in its box \u2014 or an expression over signals, for a card that moves between states. With `dodge`, `auto` (and any anchor, as a first choice) moves it to where it covers the least data and text."),
    width: t3.number(260, "Card width (px); text wraps inside."),
    margin: t3.number(12, "Space between the card and its box's edges (px, or [top, right, bottom, left])."),
    dodge: t3.bool(true, "Move out of the way of data and text (taking a band below the chart when nowhere is free). Off: the card stays at `at` (`auto`: top-right), over whatever is there \u2014 a card over a map, where land covers everything.")
  },
  tokens: ["card", "card-ink", "card-ink-2", "card-line", "radius.card", "accent", "size.body", "size.small"],
  expand(params) {
    const norm = (v) => typeof v === "string" && v.startsWith("=") ? e3(v.slice(1)) : v;
    const p = { ...params, text: norm(params.text), title: norm(params.title), kicker: norm(params.kicker), at: norm(params.at) };
    const pad = 14;
    const m = Array.isArray(p.margin) ? p.margin : [p.margin, p.margin, p.margin, p.margin];
    const width = e3(`min(${p.width}, box.w - ${m[1] + m[3]})`);
    const isExpr = (v) => typeof v === "object" && v !== null && "expr" in v;
    const exprOf2 = (v) => isExpr(v) ? v.expr : JSON.stringify(v ?? "");
    const hidden = isExpr(p.text) ? e3(`(${exprOf2(p.text)}) != ""`) : void 0;
    const line2 = (key, content, style, optional = false) => text2(content, [0, 0], { key, when: optional && isExpr(content) ? e3(`(${exprOf2(content)}) != ""`) : void 0, size: { h: "auto" }, style: { baseline: "top", maxWidth: e3("box.w"), ...style } });
    const boxKey = isExpr(p.text) ? e3(`"box:" + (${exprOf2(p.text)})`) : "box";
    const box = group3({
      key: boxKey,
      size: { w: width, h: "auto" },
      layout: { type: "rows", gap: 6, padding: pad },
      backdrop: { fill: "$card", stroke: { paint: "$card-line", width: 1 }, radius: e3('token("radius.card")'), padding: pad, fit: "width" },
      semantics: { role: "annotation", label: p.text },
      children: [
        p.kicker ? line2("kicker", p.kicker, { size: "$size.small", weight: 700, ink: "$accent" }, true) : null,
        p.title ? line2("title", p.title, { size: e3('token("size.body") + 2'), weight: 600, ink: "$card-ink" }, true) : null,
        line2("text", p.text, { size: "$size.body", ink: "$card-ink-2" })
      ]
    });
    const at2 = isExpr(p.at) ? e3(`(${p.at.expr}) == "auto" ? ${JSON.stringify(CARD_AUTO[0])} : (${p.at.expr})`) : p.at === "auto" ? CARD_AUTO[0] : String(p.at);
    const dodge = !p.dodge ? [at2] : p.at === "auto" ? CARD_AUTO : [at2, ...CARD_AUTO.filter((a) => a !== p.at)];
    return group3({ key: "card", when: hidden, layout: { type: "stack", padding: p.margin, align: "start" }, dodge, children: [box] });
  }
});

// src/finance.ts
import { e as e5, geom as geom5, group as group5, op as op3, recipe as recipe5, repeat as repeat5, shape as shape5, t as t5, text as text4 } from "@datars/sdk";

// src/marks.ts
import { e as e4, group as group4, instances, op as op2, recipe as recipe4, repeat as repeat4, shape as shape4, geom as geom4, t as t4, text as text3 } from "@datars/sdk";
function affixed(p, value, at2, opts) {
  if (!p.prefix && !p.suffix) return text3("", at2, { ...opts, number: { value: e4(value), format: p.format } });
  const v = `(${value})`;
  return text3(e4(`(${v} < 0 ? "\u2212" : "") + ${JSON.stringify(p.prefix ?? "")} + format(abs(${v}), ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}`), at2, opts);
}
var colorOr = (color, fallback) => color ? e4(`scale.color(d.${color})`) : fallback;
var onInkExpr = (fill) => typeof fill === "string" ? JSON.stringify(`on(${fill})`) : fill && typeof fill === "object" && "expr" in fill ? `"on(" + (${fill.expr}) + ")"` : `"$ink"`;
var isBand = (t17) => t17 === "band" || t17 === "point";
var bar = recipe4({
  id: "@datars/std/bar",
  doc: "Bars from a category field on a band scale to a value on a linear scale (vertical or horizontal, decided by which axis is the band). Negative values hang below zero.",
  params: {
    data: t4.table(),
    x: t4.field(),
    y: t4.field(),
    color: t4.field(),
    xType: t4.string("band"),
    yType: t4.string("linear"),
    fill: t4.prop("Fill ink or expression (default: the colour scale, else $mark)."),
    labels: t4.bool(false, "Value labels on the bars (they count when values change)."),
    format: t4.string(",.1~f", "Label number format."),
    radius: t4.number(0, "Corner radius (0: the theme's `radius.bar`)."),
    selected: t4.string(void 0, "A keyset signal: selected bars stay strong, others recede."),
    label: t4.prop("Bar label (tooltip, accessible name): an expression over the row; default `name: value`."),
    chapter: t4.string(void 0, "Clicking a bar enters this program chapter with the bar's category (drill-down)."),
    prefix: t4.string(void 0, "Before each value label's number ('$')."),
    suffix: t4.string(void 0, "After it ('M', ' kr').")
  },
  tokens: ["mark", "ink", "muted", "dim", "size.label", "radius.bar"],
  expand(p) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const fill = p.fill ?? colorOr(p.color, "$mark");
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    const opacity = p.selected ? e4(`${p.selected}.isEmpty() || ${p.selected}.has(d.${cat}) ? 1 : token("dim")`) : void 0;
    const g = horizontal ? geom4.rect({ x: e4(`min(scale.x(0), scale.x(d.${val}))`), y: e4(`scale.y(d.${cat})`), w: e4(`abs(scale.x(d.${val}) - scale.x(0))`), h: e4("scale.y.bandwidth()"), r: p.radius || "$radius.bar" }) : geom4.rect({ x: e4(`scale.x(d.${cat})`), y: e4(`min(scale.y(0), scale.y(d.${val}))`), w: e4("scale.x.bandwidth()"), h: e4(`abs(scale.y(0) - scale.y(d.${val}))`), r: p.radius || "$radius.bar" });
    const text_ = `(${JSON.stringify(p.prefix ?? "")} + format(d.${val}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")})`;
    const lw = `measure(${text_}, token("size.label"))`;
    const fits = horizontal ? e4('scale.y.bandwidth() >= token("size.label") - 2') : e4(`${lw} <= scale.x.bandwidth() + 6`);
    const end = horizontal ? `max(scale.x(0), scale.x(d.${val}))` : `min(scale.y(0), scale.y(d.${val}))`;
    const length = horizontal ? `abs(scale.x(d.${val}) - scale.x(0))` : `abs(scale.y(d.${val}) - scale.y(0))`;
    const inside = horizontal ? `(${end} + 4 + ${lw} > box.w && ${length} >= ${lw} + 8)` : `(${end} - 4 - token("size.label") < -2 && ${length} >= token("size.label") + 8)`;
    const ink = e4(`${inside} ? ${onInkExpr(fill)} : "$ink-2"`);
    const label = horizontal ? affixed(p, `d.${val}`, [e4(`${inside} ? ${end} - 4 : ${end} + 4`), e4(`scale.y(d.${cat}) + scale.y.bandwidth() / 2`)], { when: fits, style: { size: "$size.label", ink, contain: true, align: e4(`${inside} ? "end" : "start"`), baseline: "middle" } }) : affixed(p, `d.${val}`, [e4(`scale.x(d.${cat}) + scale.x.bandwidth() / 2`), e4(`${inside} ? ${end} + 4 : ${end} - 4`)], { when: fits, style: { size: "$size.label", ink, contain: true, align: "middle", baseline: e4(`${inside} ? "top" : "alphabetic"`) } });
    return group4({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        // Primary datum shapes are keyed by the datum key directly (a std convention), so a bar
        // pairs with the same datum's slice, cell or region in another recipe.
        repeat4(p.data, shape4(g, {
          fill,
          opacity,
          semantics: { role: "datum", label: p.label ?? e4(`\`\${key.name(d.${cat})}: \${format(d.${val}, ${JSON.stringify(p.format)})}\``), value: e4(`d.${val}`) },
          anchors: [{ name: "top", at: horizontal ? [e4(`scale.x(d.${val})`), e4(`scale.y(d.${cat}) + scale.y.bandwidth()/2`)] : [e4(`scale.x(d.${cat}) + scale.x.bandwidth()/2`), e4(`scale.y(d.${val})`)] }],
          on: p.chapter ? { activate: { chapter: p.chapter, key: e4(`d.${cat}`) } } : p.selected ? { activate: { toggle: p.selected, value: e4(`d.${cat}`) } } : void 0,
          pickable: true
        })),
        p.labels ? group4({ key: "labels", children: [repeat4(p.data, label)] }) : null
      ]
    });
  },
  // Entering bars grow from their base along the value axis.
  motion: (p) => [{ select: { role: "datum" }, enter: { scale: 0, origin: isBand(p.yType) && !isBand(p.xType) ? "left" : "bottom" } }]
});
var line = recipe4({
  id: "@datars/std/line",
  doc: "One line per series (the colour field, or `series`), through x/y. Draws on as it enters.",
  params: {
    data: t4.table(),
    x: t4.field(),
    y: t4.field(),
    color: t4.field(),
    xType: t4.string("linear"),
    series: t4.field("Group rows into lines by this field (default: the colour field)."),
    curve: t4.oneOf(["linear", "monotone-x", "catmull-rom", "step", "step-before", "step-after"], "monotone-x"),
    width: t4.number(0, "Stroke width (0 = the theme's stroke.line)."),
    points: t4.bool(false, "Dots at the data points."),
    stroke: t4.prop("Stroke ink or expression."),
    labels: t4.bool(false, "Label each line at its last point (labels pushed apart so they never overlap)."),
    clip: t4.bool(false, "Clip the lines to the plot area (the end labels stay outside it)."),
    selected: t4.string(void 0, "A keyset signal: selected series stay strong, the others recede."),
    yScale: t4.string("y", "The value scale: `y2` for a plot's right axis.")
  },
  tokens: ["mark", "stroke.line", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const ink = p.stroke ?? (series && p.color ? e4(`scale.color(d.${p.color})`) : "$mark");
    const ys = p.yScale || "y";
    const lineShape = shape4(geom4.polyline({ from: "@group", x: e4(xExpr), y: e4(`scale.${ys}(d.${p.y})`), curve: p.curve }), {
      key: "line",
      opacity: p.selected && series ? e4(`${p.selected}.isEmpty() || ${p.selected}.has(d.${series}) ? 1 : token("dim")`) : void 0,
      stroke: { paint: ink, width: p.width || "$stroke.line", join: "round", cap: "round" },
      semantics: { role: "series", label: series ? e4(`key.name(d.${series})`) : p.y }
    });
    const xLabel2 = isBand(p.xType) ? `key.name(d.${p.x})` : `scale.x.label(d.${p.x})`;
    const dotLabel = series ? `\`\${key.name(d.${series})} \xB7 \${${xLabel2}}: \${format(d.${p.y}, ",.2~f")}\`` : `\`\${${xLabel2}}: \${format(d.${p.y}, ",.2~f")}\``;
    const pts = instances({ key: "points", from: "@group", x: e4(xExpr), y: e4(`scale.${ys}(d.${p.y})`), r: p.points ? 3 : 0, fill: p.points ? ink : "transparent", instanceKey: e4(`d.${p.x}`), label: e4(dotLabel), hit: "line" });
    const labelled = !!(p.labels && series);
    const one = group4({ clip: p.clip && labelled ? "box" : void 0, children: [lineShape, pts] });
    const each = series ? repeat4({ groups: p.data, by: series }, one) : repeat4({ groups: p.data, by: p.y === "__none" ? p.x : "__all" }, one);
    if (!labelled) return group4({ key: "lines", clip: p.clip ? "box" : void 0, children: [each] });
    const ends = cx.table(
      "ends",
      p.data,
      op2.aggregate([series], { x_end: ["last", p.x], y_end: ["last", p.y] }),
      op2.spread({ position: e4("scale.y(d.y_end)"), gap: e4('token("size.label") + 3'), min: 0, max: e4("box.h"), as: "label_y" })
    );
    const xEnd = isBand(p.xType) ? "scale.x(d.x_end) + scale.x.bandwidth() / 2" : "scale.x(d.x_end)";
    const labelInk = p.stroke ?? (p.color ? e4(`scale.color(d.${series})`) : "$mark");
    return group4({ key: "lines", children: [
      each,
      group4({ key: "end-labels", children: [repeat4(ends, text3(e4(`key.name(d.${series})`), [e4(`min(${xEnd}, box.w) + 6`), e4("d.label_y")], { style: { size: "$size.label", ink: labelInk, baseline: "middle", contain: true } }))] })
    ] });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }]
});
var area = recipe4({
  id: "@datars/std/area",
  doc: "Filled areas from a baseline (0, or `y0`) to y \u2014 one per series. For stacked areas, give a stacked table (op.stack) and y0/y fields.",
  params: {
    data: t4.table(),
    x: t4.field(),
    y: t4.field(),
    color: t4.field(),
    xType: t4.string("linear"),
    series: t4.field(),
    y0: t4.field("Lower bound field (default: zero)."),
    curve: t4.string("monotone-x"),
    opacity: t4.number(0.85),
    fill: t4.prop(),
    clip: t4.bool(false, "Clip to the plot area.")
  },
  tokens: ["mark"],
  expand(p) {
    const series = p.series ?? p.color;
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const fill = p.fill ?? (series && p.color ? e4(`scale.color(d.${p.color})`) : "$mark");
    const band = shape4(geom4.area({ from: "@group", x: e4(xExpr), y0: e4(p.y0 ? `scale.y(d.${p.y0})` : "max(0, min(box.h, scale.y(0)))"), y1: e4(`scale.y(d.${p.y})`), curve: p.curve }), {
      key: "area",
      fill,
      opacity: p.opacity,
      semantics: { role: "series", label: series ? e4(`key.name(d.${series})`) : p.y }
    });
    return group4({ key: "areas", clip: p.clip ? "box" : void 0, children: [repeat4({ groups: p.data, by: series ?? "__all" }, group4({ children: [band] }))] });
  }
});
var point = recipe4({
  id: "@datars/std/point",
  doc: "A dot per row (instanced: scales to 10\u2075\u201310\u2076 points).",
  params: {
    data: t4.table(),
    x: t4.field(),
    y: t4.field(),
    color: t4.field(),
    xType: t4.string("linear"),
    yType: t4.string("linear"),
    r: t4.prop("Radius (px) or expression."),
    symbol: t4.string("circle"),
    fill: t4.prop(),
    opacity: t4.prop(),
    label: t4.prop("Accessible label per point."),
    clip: t4.bool(false, "Clip to the plot area.")
  },
  tokens: ["mark", "point.radius"],
  expand(p) {
    const xExpr = isBand(p.xType) ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const yExpr = isBand(p.yType) ? `scale.y(d.${p.y}) + scale.y.bandwidth() / 2` : `scale.y(d.${p.y})`;
    return instances({
      key: "points",
      clip: p.clip ? "box" : void 0,
      from: p.data,
      proto: p.symbol,
      x: e4(xExpr),
      y: e4(yExpr),
      r: p.r ?? "$point.radius",
      fill: p.fill ?? colorOr(p.color, "$mark"),
      opacity: p.opacity,
      label: p.label ?? e4(`\`\${d.${p.x}}, \${d.${p.y}}\``),
      semantics: { role: "series", label: `${p.y} against ${p.x}` }
    });
  }
});
var dot = recipe4({
  id: "@datars/std/dot",
  doc: "A dot per category at its value (a dot plot).",
  params: { data: t4.table(), x: t4.field(), y: t4.field(), color: t4.field(), xType: t4.string("band"), yType: t4.string("linear"), r: t4.number(6), fill: t4.prop() },
  expand(p) {
    const horizontal = isBand(p.yType) && !isBand(p.xType);
    const cx = horizontal ? e4(`scale.x(d.${p.x})`) : e4(`scale.x(d.${p.x}) + scale.x.bandwidth() / 2`);
    const cy = horizontal ? e4(`scale.y(d.${p.y}) + scale.y.bandwidth() / 2`) : e4(`scale.y(d.${p.y})`);
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    return group4({
      key: "marks",
      children: [repeat4(p.data, shape4(geom4.circle({ cx, cy, r: p.r }), { fill: p.fill ?? colorOr(p.color, "$mark"), semantics: { role: "datum", label: e4(`\`\${key.name(d.${cat})}: \${d.${val}}\``) }, pickable: true }))]
    });
  }
});
var pareto = recipe4({
  id: "@datars/std/pareto",
  doc: "The running share of the total, from the largest category down, as a line with points on the plot's right axis (`right: { domain: [0, 1], format: '.0%' }`) \u2014 over bars sorted descending, a Pareto chart.",
  params: { data: t4.table(), x: t4.field(), y: t4.field(), xType: t4.string("band"), stroke: t4.prop("Line ink (default: the theme's secondary mark).") },
  tokens: ["ink-2"],
  expand(p, cx) {
    const tbl = cx.table("pareto", p.data, op2.window("share_of_total", p.y, "share"), op2.window("cumsum", "share", "cum_share", { order: `-${p.y}` }));
    return group4({ key: "pareto", children: [line({ data: tbl, x: p.x, y: "cum_share", xType: p.xType, curve: "linear", points: true, stroke: p.stroke ?? "$ink-2", yScale: "y2", width: 2 })] });
  }
});

// src/finance.ts
var isBand2 = (t17) => t17 === "band" || t17 === "point";
var xMid = (x, xType) => isBand2(xType) ? `scale.x(d.${x}) + scale.x.bandwidth() / 2` : `scale.x(d.${x})`;
var byDirection = (o, c, up, down) => e5(`d.${c} >= d.${o} ? ${JSON.stringify(up)} : ${JSON.stringify(down)}`);
var tag = (v) => String(v).replace(/[^0-9A-Za-z]/g, "_");
var nameOf = (id) => String(id ?? "").split("/").pop();
function slotWidth(p) {
  if (isBand2(p.xType)) return p.width ? `scale.x.bandwidth() * ${p.width}` : "scale.x.bandwidth()";
  return p.width ? String(p.width) : `max(1, box.w / max(1, table.count(${JSON.stringify(p.data)})) * 0.7)`;
}
function historyOps(p, ops) {
  if (!p.source || p.source === p.data) return [p.data, ops];
  return [p.source, [...ops, op3.join(p.data, p.series ? [p.series, p.x] : [p.x], "inner")]];
}
function ohlcLabel(p) {
  const c = p.close ?? p.y;
  const f = (v) => `format(d.${v}, ${JSON.stringify(p.format)})`;
  const change = `(d.prev_close > 0 ? " (" + format(d.${c} / d.prev_close - 1, "+.2%") + ")" : "")`;
  return e5(`scale.x.label(d.${p.x}) + " \xB7 O " + ${f(p.open)} + " H " + ${f(p.high)} + " L " + ${f(p.low)} + " C " + ${f(c)} + ${change}`);
}
var candlestick = recipe5({
  id: "@datars/std/candlestick",
  doc: 'Candlesticks: per row a wick from low to high and a body from open to close, in `$up` when the close is at or above the open and `$down` below. Use in a plot with `xType: "band"` over dates for trading days without gaps; the plot fits its value axis to the lows and highs. Hovering a candle reads its date, OHLC and change.',
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date (or time) field."),
    y: t5.field("The plot's value field: the close, unless `close` is given."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    open: t5.string("open", "Opening price field."),
    high: t5.string("high", "High field."),
    low: t5.string("low", "Low field."),
    close: t5.field("Closing price field (default: the plot's `y`, else `close`)."),
    up: t5.ink("$up", "Ink of rising candles."),
    down: t5.ink("$down", "Ink of falling candles."),
    hollow: t5.bool(false, "Hollow candles: rising bodies outlined, falling ones filled."),
    width: t5.number(0, "Body width: a fraction of the band on a band scale (0 = the band, whose padding leaves the gaps), px on a continuous scale (0 = 70% of the spacing)."),
    format: t5.string(",.2f", "Price format in labels."),
    label: t5.prop("Candle label (tooltip, accessible name); default date \xB7 O H L C (change).")
  },
  tokens: ["up", "down", "paper"],
  expand(p, cx) {
    const c = p.close ?? p.y ?? "close";
    const [o, h, l] = [p.open, p.high, p.low];
    const tbl = cx.table("candles", p.data, op3.window("lag", c, "prev_close", { order: p.x }));
    const mid = xMid(p.x, p.xType);
    const w = slotWidth(p);
    const ink = byDirection(o, c, p.up, p.down);
    const wick = shape5(geom5.segment({ x1: e5(mid), y1: e5(`scale.y(d.${h})`), x2: e5(mid), y2: e5(`scale.y(d.${l})`) }), { stroke: { paint: ink, width: 1 }, semantics: { role: "decoration" } });
    const body = shape5(geom5.rect({
      x: e5(`${mid} - (${w}) / 2`),
      y: e5(`min(scale.y(d.${o}), scale.y(d.${c}))`),
      w: e5(w),
      // A doji (open = close) still shows as a line.
      h: e5(`max(1, abs(scale.y(d.${o}) - scale.y(d.${c})))`)
    }), {
      fill: p.hollow ? e5(`d.${c} >= d.${o} ? "$paper" : ${JSON.stringify(p.down)}`) : ink,
      stroke: p.hollow ? { paint: ink, width: 1 } : void 0,
      semantics: { role: "datum", label: p.label ?? ohlcLabel({ ...p, close: c }), value: e5(`d.${c}`) },
      pickable: true
    });
    return group5({
      key: "candles",
      semantics: { role: "series", label: `${c} candles` },
      children: [group5({ key: "wicks", children: [repeat5(tbl, wick)] }), group5({ key: "bodies", children: [repeat5(tbl, body)] })]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }]
});
var ohlc = recipe5({
  id: "@datars/std/ohlc",
  doc: "OHLC bars: per row a vertical line from low to high with the open ticked to the left and the close to the right, in `$up`/`$down` by direction (or ink). Same fields and plot fitting as `candlestick`.",
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date (or time) field."),
    y: t5.field("The plot's value field: the close, unless `close` is given."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    open: t5.string("open", "Opening price field."),
    high: t5.string("high", "High field."),
    low: t5.string("low", "Low field."),
    close: t5.field("Closing price field (default: the plot's `y`, else `close`)."),
    up: t5.ink("$up", "Ink of rising bars."),
    down: t5.ink("$down", "Ink of falling bars."),
    colored: t5.bool(true, "Colour by direction; off: every bar in `$ink`."),
    width: t5.number(0, "Tick span: a fraction of the band (0 = the band), px on a continuous scale."),
    format: t5.string(",.2f", "Price format in labels."),
    label: t5.prop("Bar label; default date \xB7 O H L C (change).")
  },
  tokens: ["up", "down", "ink"],
  expand(p, cx) {
    const c = p.close ?? p.y ?? "close";
    const [o, h, l] = [p.open, p.high, p.low];
    const tbl = cx.table("bars", p.data, op3.window("lag", c, "prev_close", { order: p.x }));
    const mid = xMid(p.x, p.xType);
    const half = `(${slotWidth(p)}) / 2`;
    const y = (f) => `scale.y(d.${f})`;
    const path = `\`M \${${mid}} \${${y(h)}} L \${${mid}} \${${y(l)}} M \${${mid} - ${half}} \${${y(o)}} L \${${mid}} \${${y(o)}} M \${${mid}} \${${y(c)}} L \${${mid} + ${half}} \${${y(c)}}\``;
    return group5({
      key: "ohlc",
      semantics: { role: "series", label: `${c} bars` },
      children: [repeat5(tbl, shape5(geom5.path(e5(path)), {
        stroke: { paint: p.colored ? byDirection(o, c, p.up, p.down) : "$ink", width: e5(`max(1, min(2, ${half} / 2))`), cap: "butt" },
        semantics: { role: "datum", label: p.label ?? ohlcLabel({ ...p, close: c }), value: e5(`d.${c}`) },
        pickable: true
      }))]
    });
  }
});
var volume = recipe5({
  id: "@datars/std/volume",
  doc: "Volume bars under a price chart: in a plot, they go to a pane below the price area that shares its x axis and has its own value axis (the plot's `lower` pane, made for them when not given), coloured like their day's candle.",
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date field."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    y: t5.field("The plot's value field: the close, unless `close` is given."),
    volume: t5.string("volume", "Volume field."),
    open: t5.string("open", "Opening price field (for the colour)."),
    close: t5.field("Closing price field (default: the plot's `y` \u2014 unless that is the volume \u2014 else `close`)."),
    up: t5.ink("$up", "Ink on rising days."),
    down: t5.ink("$down", "Ink on falling days."),
    colored: t5.bool(true, "Colour by the day's direction; off: `$muted`."),
    opacity: t5.number(0.55, "Bar opacity (volume recedes behind price)."),
    yScale: t5.string("lower", "The value scale: `lower` (the plot's lower pane), or `y` for a volume chart of its own."),
    format: t5.string(",.0f", "Volume format in labels."),
    width: t5.number(0, "Bar width: a fraction of the band (0 = the band), px on a continuous scale.")
  },
  tokens: ["up", "down", "muted"],
  expand(p) {
    const c = p.close ?? (p.y && p.y !== p.volume ? p.y : "close");
    const s = p.yScale || "lower";
    const w = slotWidth(p);
    const v = `d.${p.volume}`;
    return group5({
      key: "volume",
      semantics: { role: "series", label: p.volume },
      children: [repeat5(p.data, shape5(geom5.rect({
        x: e5(`${xMid(p.x, p.xType)} - (${w}) / 2`),
        y: e5(`min(scale.${s}(0), scale.${s}(${v}))`),
        w: e5(w),
        h: e5(`abs(scale.${s}(0) - scale.${s}(${v}))`)
      }), {
        fill: p.colored ? byDirection(p.open, c, p.up, p.down) : "$muted",
        opacity: p.opacity,
        semantics: { role: "datum", label: e5(`scale.x.label(d.${p.x}) + " \xB7 volume " + format(${v}, ${JSON.stringify(p.format)})`), value: e5(v) },
        pickable: true
      }))]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }]
});
function maOps(p) {
  const as = `${p.kind === "ema" ? "ema" : "sma"}_${tag(p.window)}`;
  const fn = p.kind === "ema" ? "ema" : "rolling_mean";
  return [as, [op3.window(fn, p.y, as, { partition: p.series ? [p.series] : void 0, order: p.x, k: p.window, min: p.full ? p.window : void 0 })]];
}
var movingAverage = recipe5({
  id: "@datars/std/movingAverage",
  doc: "A moving average of the plot's `y` (a closing price) as a line: simple (the mean of the last `window` rows) or exponential (span `window`). Computed over `source` \u2014 the whole history \u2014 when given, so the first day on show already has its full window. Shown in the plot's indicator key.",
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date field."),
    y: t5.field("The averaged field (default: the plot's `y`)."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t5.field("One average per series (tickers)."),
    color: t5.field("The plot's colour field (per-series inks)."),
    window: t5.number(20, "Rows per window (trading days on a band scale): 20 \u2248 a month, 50, 200."),
    kind: t5.oneOf(["sma", "ema"], "sma", "Simple or exponential."),
    source: t5.table("A longer history of the same rows to compute over (the chart shows only the rows of `data`)."),
    full: t5.bool(true, "Only full windows: no line until `window` rows (else the first rows average what there is)."),
    stroke: t5.prop("Line ink (default: $accent, or the colour scale per series)."),
    width: t5.number(1.5, "Stroke width."),
    label: t5.string(void 0, "Name in the indicator key and to screen readers (default `SMA 20`).")
  },
  tokens: ["accent"],
  expand(p, cx) {
    const [as, ops] = maOps(p);
    const [from, all] = historyOps(p, ops);
    const tbl = cx.table("ma", from, ...all);
    const series = p.series ?? void 0;
    const stroke = p.stroke ?? (series && p.color ? void 0 : "$accent");
    return group5({
      key: `ma-${as}`,
      semantics: { role: "series", label: p.label ?? `${p.kind.toUpperCase()} ${p.window}` },
      children: [line({ data: tbl, x: p.x, y: as, xType: p.xType, series, color: series ? p.color : void 0, stroke, width: p.width, curve: "linear" })]
    });
  }
});
function bandOps(p) {
  const id = `${tag(p.window)}_${tag(p.k)}`;
  const [mid, sd, hi, lo] = [`bb_mid_${id}`, `bb_sd_${id}`, `bb_hi_${id}`, `bb_lo_${id}`];
  const o = { partition: p.series ? [p.series] : void 0, order: p.x, k: p.window, min: p.full ? p.window : void 0 };
  return [[mid, hi, lo], [
    op3.window("rolling_mean", p.y, mid, o),
    op3.window("rolling_std", p.y, sd, o),
    op3.derive(hi, e5(`d.${mid} + ${p.k} * d.${sd}`)),
    op3.derive(lo, e5(`d.${mid} - ${p.k} * d.${sd}`))
  ]];
}
var bollinger = recipe5({
  id: "@datars/std/bollinger",
  doc: "Bollinger bands: the moving average of `y` over `window` rows with a shaded band `k` standard deviations (population \u03C3) above and below it \u2014 where the price is stretched, and how volatile it is. Computes over `source` when given, like `movingAverage`; the plot fits its value axis to the band.",
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date field."),
    y: t5.field("The field (default: the plot's `y`)."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t5.field("One band per series."),
    window: t5.number(20, "Rows per window."),
    k: t5.number(2, "Band half-width in standard deviations."),
    source: t5.table("A longer history to compute over (see movingAverage)."),
    full: t5.bool(true, "Only full windows."),
    fill: t5.ink("$accent", "Band ink."),
    opacity: t5.number(0.1, "Band opacity."),
    stroke: t5.ink("$accent", "Ink of the band's edges and middle line."),
    width: t5.number(1, "Edge stroke width."),
    middle: t5.bool(true, "Draw the middle (moving average) line."),
    label: t5.string(void 0, "Name in the indicator key (default `BB 20, 2`).")
  },
  tokens: ["accent"],
  expand(p, cx) {
    const [[mid, hi, lo], ops] = bandOps(p);
    const [from, all] = historyOps(p, ops);
    const tbl = cx.table("bands", from, ...all);
    const edge = (key, y, opacity) => group5({ key, opacity, children: [line({ data: tbl, x: p.x, y, xType: p.xType, series: p.series, stroke: p.stroke, width: p.width, curve: "linear" })] });
    return group5({
      key: `bollinger-${tag(p.window)}-${tag(p.k)}`,
      semantics: { role: "series", label: p.label ?? `BB ${p.window}, ${p.k}` },
      children: [
        group5({ key: "band", children: [area({ data: tbl, x: p.x, y: hi, y0: lo, xType: p.xType, series: p.series, fill: p.fill, opacity: p.opacity, curve: "linear" })] }),
        edge("upper", hi, 0.7),
        edge("lower", lo, 0.7),
        p.middle ? edge("middle", mid, 0.45) : null
      ]
    });
  }
});
function indexOps(p) {
  const as = p.percent ? "change" : "indexed";
  return [as, [
    op3.window("first", p.y, "index_first", { partition: p.series ? [p.series] : void 0, order: p.x }),
    op3.derive(as, e5(p.percent ? `d.${p.y} / d.index_first - 1` : `d.${p.y} / d.index_first * ${p.base}`))
  ]];
}
var indexed = recipe5({
  id: "@datars/std/indexed",
  doc: 'Series rebased to their first visible row \u2014 100 (or 0 %) \u2014 so instruments at any price compare as performance: one line per series with a baseline, each labelled at its end with its change. Filter the rows to a range and every line starts from the base again. Use with `format: "+.0%"` on the plot for `percent`.',
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date field."),
    y: t5.field("The price field (default: the plot's `y`)."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t5.field("One line per series (default: the colour field)."),
    color: t5.field("The plot's colour field (line and label inks)."),
    base: t5.number(100, "The value every series starts at."),
    percent: t5.bool(false, "Change from the start (0 = no change, 0.1 = +10 %) instead of an index."),
    labels: t5.bool(true, "Label each line's end: series and change (pushed apart so they never overlap)."),
    baseline: t5.bool(true, "A rule at the base."),
    stroke: t5.prop("Line ink (default: the colour scale, else $mark)."),
    width: t5.number(0, "Stroke width (0 = the theme's)."),
    format: t5.string("+.1%", "Format of the change in end labels.")
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
    return group5({ key: "indexed", children: [baseline, lines, labels] });
  }
});
function endLabels(cx, p) {
  const ends = cx.table(
    "ends",
    p.table,
    op3.aggregate(p.series ? [p.series] : [], { x_end: ["last", p.x], y_end: ["last", p.field] }),
    op3.spread({ position: e5("scale.y(d.y_end)"), gap: e5('token("size.label") + 3'), min: 0, max: e5("box.h"), as: "label_y" })
  );
  const xEnd = isBand2(p.xType) ? "scale.x(d.x_end) + scale.x.bandwidth() / 2" : "scale.x(d.x_end)";
  const name = p.series ? `key.name(d.${p.series}) + " " + ` : "";
  const ink = p.ink ?? (p.series && p.color ? e5(`scale.color(d.${p.series})`) : "$mark");
  return group5({ key: "end-labels", children: [repeat5(ends, text4(e5(`${name}${p.value}`), [e5(`min(${xEnd}, box.w) + 6`), e5("d.label_y")], { style: { size: "$size.label", ink, baseline: "middle", contain: true } }))] });
}
function drawdownOps(p) {
  return ["drawdown", [
    op3.window("cummax", p.y, "peak", { partition: p.series ? [p.series] : void 0, order: p.x }),
    op3.derive("drawdown", e5(`d.peak > 0 ? d.${p.y} / d.peak - 1 : 0`))
  ]];
}
var drawdown = recipe5({
  id: "@datars/std/drawdown",
  doc: 'Drawdown: how far each series sits below its running peak (0 at a new high, \u221225 % a quarter below it), as an area hanging from zero \u2014 the pain chart next to a performance chart. With `source`, peaks count from the start of the whole history instead of the first visible row. Use with `format: ".0%"` on the plot.',
  params: {
    data: t5.table("The rows to draw (default: the plot's)."),
    x: t5.field("The date field."),
    y: t5.field("The price field (default: the plot's `y`)."),
    xType: t5.string("band", "`band`: trading days side by side, no weekend gaps (the plot's); `time`/`linear`: rows by value."),
    series: t5.field("One drawdown per series (default: the colour field)."),
    color: t5.field("The plot's colour field (line and label inks)."),
    source: t5.table("A longer history: peaks before the rows on show count too."),
    fill: t5.ink("$down", "Area ink (one series)."),
    opacity: t5.number(0.22, "Area opacity."),
    stroke: t5.prop("Line ink (default: $down, or the colour scale per series)."),
    width: t5.number(1.5, "Stroke width."),
    area: t5.bool(true, "Shade from zero (one series; several draw lines only)."),
    labels: t5.bool(true, "Label each series' end with its name and current drawdown (with a series; pushed apart)."),
    format: t5.string("+.1%", "Format of the drawdown in end labels.")
  },
  tokens: ["down", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const [as, ops] = drawdownOps({ ...p, series });
    const [from, all] = historyOps({ ...p, series }, ops);
    const tbl = cx.table("drawdown", from, ...all);
    const shade2 = p.area && !series ? area({ data: tbl, x: p.x, y: as, xType: p.xType, fill: p.fill, opacity: p.opacity, curve: "linear" }) : null;
    const stroke = p.stroke ?? (series && p.color ? void 0 : p.fill);
    return group5({ key: "drawdown", semantics: { role: "series", label: "drawdown" }, children: [
      shade2 ? group5({ key: "shade", children: [shade2] }) : null,
      line({ data: tbl, x: p.x, y: as, xType: p.xType, series, color: series ? p.color : void 0, stroke, width: p.width, curve: "linear" }),
      p.labels && series ? endLabels(cx, { table: tbl, x: p.x, xType: p.xType, series, color: p.color, field: as, value: `format(d.y_end, ${JSON.stringify(p.format)})`, ink: p.stroke }) : null
    ] });
  }
});
var sparkline = recipe5({
  id: "@datars/std/sparkline",
  doc: "A sparkline: a word-sized line of `y` over `x` that fills its box (a table cell, a card) \u2014 no axes, the last value marked with a dot, coloured `$up` or `$down` by whether it ended above where it started. Works on any table, `@group` too (one per ticker in a repeat).",
  params: {
    data: t5.table("The rows (a named table, or `@group` inside a repeat)."),
    x: t5.field("The date (or order) field."),
    y: t5.field("The value field."),
    xType: t5.oneOf(["linear", "time", "point"], "point", "x scale: `point` spaces rows evenly (trading days), `time` by date."),
    trend: t5.bool(true, "Colour by the change over the line ($up / $down); off: $mark."),
    stroke: t5.prop("Line ink (overrides the trend colour)."),
    fill: t5.bool(true, "A soft area under the line."),
    dot: t5.bool(true, "A dot at the last value."),
    width: t5.number(1.5, "Stroke width."),
    format: t5.string(",.2f", "Value format in the label."),
    name: t5.string(void 0, "What the line is (its accessible label starts with it).")
  },
  tokens: ["up", "down", "mark"],
  expand(p) {
    const [first, last] = [`group.first(${JSON.stringify(p.y)})`, `group.last(${JSON.stringify(p.y)})`];
    const ink = p.stroke ?? (p.trend ? e5(`${last} >= ${first} ? "$up" : "$down"`) : "$mark");
    const x = e5(`scale.sx(d.${p.x})`), y = e5(`scale.sy(d.${p.y})`);
    const label = e5(`${JSON.stringify(p.name ? `${p.name}: ` : "")} + format(${last}, ${JSON.stringify(p.format)}) + " (" + format(${last} / ${first} - 1, "+.1%") + ")"`);
    const pad = p.dot ? 3 : 1;
    return group5({
      key: "sparkline",
      // Narrower than this, a line says nothing (and its range would turn inside out).
      when: e5("box.w >= 24 && box.h >= 8"),
      // Rows in order, first to last: the last one sits at the right end of the x range.
      scales: {
        sx: { type: p.xType, domain: { data: p.data, field: p.x }, range: [pad, `=box.w - ${pad}`], padding: 0, nice: false },
        sy: { type: "linear", domain: { data: p.data, field: p.y }, range: [`=box.h - ${pad}`, pad], zero: false, nice: false }
      },
      children: [repeat5({ groups: p.data, by: "__all" }, group5({ children: [
        p.fill ? shape5(geom5.area({ from: "@group", x, y0: e5("box.h"), y1: y, curve: "linear" }), { key: "area", fill: ink, opacity: 0.12, semantics: { role: "decoration" } }) : null,
        shape5(geom5.polyline({ from: "@group", x, y, curve: "linear" }), { key: "line", stroke: { paint: ink, width: p.width, join: "round", cap: "round" }, semantics: { role: "series", label } }),
        p.dot ? shape5(geom5.circle({ cx: e5("scale.sx.max()"), cy: e5(`scale.sy(${last})`), r: 2.5 }), { key: "dot", fill: ink, semantics: { role: "decoration" } }) : null
      ] }))]
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }]
});
function whenExpr(w) {
  if (w && typeof w === "object" && "expr" in w) return String(w.expr);
  if (typeof w === "string") return w.startsWith("=") ? w.slice(1) : JSON.stringify(w);
  return JSON.stringify(w ?? true);
}
function claimingDefs(id) {
  const all = [candlestick, ohlc, movingAverage, bollinger, indexed, drawdown];
  return all.find((r) => nameOf(r.id) === nameOf(id))?.def.params;
}
function paramsOf(c, inherit, specs) {
  const out = { ...inherit, ...c.params };
  for (const [k, s] of Object.entries(specs)) if (out[k] === void 0 && s.default !== void 0) out[k] = s.default;
  return out;
}
function claimOf(c, inherit) {
  const specs = c.kind === "use" ? claimingDefs(c.recipe) : void 0;
  if (!specs) return null;
  const q3 = paramsOf(c, inherit, specs);
  const data = q3.data, y = q3.y ?? "close";
  const series = q3.series ?? (nameOf(c.recipe) === "indexed" || nameOf(c.recipe) === "drawdown" ? q3.color : void 0);
  const hist = (cols, ops, zero, replace = false) => {
    const from = q3.source && q3.source !== data ? q3.source : data;
    return { from, ops, late: [], fields: cols, zero, replace, series };
  };
  switch (nameOf(c.recipe)) {
    case "candlestick":
    case "ohlc":
      return { from: data, ops: [], late: [], fields: [q3.low, q3.high], zero: false, replace: false };
    case "movingAverage": {
      const [as, ops] = maOps({ ...q3, y, series });
      return hist([as], ops, false);
    }
    case "bollinger": {
      const [[, hi, lo], ops] = bandOps({ ...q3, y, series });
      return hist([hi, lo], ops, false);
    }
    case "indexed": {
      const [as, ops] = indexOps({ ...q3, y, series });
      return { from: data, ops: [], late: ops, fields: [as], zero: false, replace: true, series };
    }
    case "drawdown": {
      const [as, ops] = drawdownOps({ ...q3, y, series });
      return hist([as], ops, true, true);
    }
  }
  return null;
}
function financeDomain(children, inherit, cx) {
  const claims = children.map((c) => {
    const k = claimOf(c, inherit);
    return k && c.when !== void 0 ? { ...k, when: whenExpr(c.when) } : k;
  }).filter((c) => c !== null);
  if (!claims.length || !inherit.data) return void 0;
  const data = inherit.data;
  const history = claims.find((c) => c.from !== data)?.from;
  const seen = /* @__PURE__ */ new Set();
  const uniq = (ops) => ops.filter((o) => {
    const k = JSON.stringify(o);
    return seen.has(k) ? false : (seen.add(k), true);
  });
  const early = uniq(claims.filter((c) => c.from === (history ?? data)).flatMap((c) => c.ops));
  const series = claims.find((c) => c.series)?.series;
  const cut = history ? [op3.join(data, series ? [series, inherit.x] : [inherit.x], "inner")] : [];
  const late = uniq(claims.flatMap((c) => c.from === data && history ? [...c.ops, ...c.late] : c.late));
  const replace = claims.some((c) => c.replace);
  const shown = [];
  const claimed = claims.flatMap((c, i) => c.fields.map((f) => {
    if (!c.when) return f;
    shown.push(op3.derive(`${f}_shown_${i}`, e5(`(${c.when}) ? d.${f} : null`)));
    return `${f}_shown_${i}`;
  }));
  const fields = [.../* @__PURE__ */ new Set([...claimed, ...replace || !inherit.y ? [] : [inherit.y]])];
  const tbl = cx.table("value-axis", history ?? data, ...early, ...cut, ...late, ...shown);
  return { domain: { data: tbl, fields }, zero: claims.some((c) => c.zero) };
}
function financeKey(children) {
  const out = [];
  for (const c of children) {
    if (c.kind !== "use") continue;
    const q3 = c.params ?? {};
    let label, swatch;
    if (nameOf(c.recipe) === "movingAverage") {
      const kind = String(q3.kind ?? "sma").toUpperCase(), w = q3.window ?? 20;
      label = q3.label ?? `${kind} ${w}`;
      swatch = shape5(geom5.segment({ x1: 0, y1: 7, x2: 14, y2: 7 }), { stroke: { paint: q3.stroke ?? "$accent", width: 2, cap: "round" } });
    } else if (nameOf(c.recipe) === "bollinger") {
      const w = q3.window ?? 20, k = q3.k ?? 2;
      label = q3.label ?? `BB ${w}, ${k}`;
      swatch = shape5(geom5.rect({ x: 0, y: 2, w: 14, h: 10, r: 2 }), { fill: q3.fill ?? "$accent", opacity: 0.35, stroke: { paint: q3.stroke ?? "$accent", width: 1 } });
    } else continue;
    out.push(group5({ key: `key-${label}`, when: c.when, semantics: { role: "legend-item", label }, children: [swatch, text4(label, [19, 11], { style: { size: "$size.label", ink: "$ink-2" } })] }));
  }
  return out;
}

// src/comparisons.ts
import { e as e6, group as group6, instances as instances2, op as op4, recipe as recipe6, repeat as repeat6, shape as shape6, geom as geom6, t as t6, text as text5 } from "@datars/sdk";
var isBand3 = (type) => type === "band" || type === "point";
var nameOf2 = (id) => String(id ?? "").split("/").pop();
var colorOr2 = (color, fallback) => color ? e6(`scale.color(d.${color})`) : fallback;
function at(scale, field, type) {
  return isBand3(type) ? `scale.${scale}(d.${field}) + scale.${scale}.bandwidth() / 2` : `scale.${scale}(d.${field})`;
}
function written(v, p) {
  const n = `format(abs(${v}), ${JSON.stringify(p.format ?? ",.1~f")})`;
  return `(${v} < 0 ? "\u2212" : "") + ${JSON.stringify(p.prefix ?? "")} + ${n} + ${JSON.stringify(p.suffix ?? "")}`;
}
function among(field, names) {
  const list = Array.isArray(names) ? names : names === void 0 || names === null || names === "" ? [] : [names];
  return list.length ? `(${list.map((n) => `d.${field} == ${JSON.stringify(String(n))}`).join(" || ")})` : "true";
}
function withDefaults(def, params) {
  const out = { ...params };
  for (const [k, spec] of Object.entries(def.params)) if (out[k] === void 0 && spec.default !== void 0) out[k] = spec.default;
  return out;
}
var lollipop = recipe6({
  id: "@datars/std/lollipop",
  doc: "A lollipop per category: a thin stem from zero to the value, a dot at its end \u2014 a bar chart with less ink, for many categories or values far from zero. Horizontal when y is the band axis.",
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("band", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("linear", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    r: t6.number(5, "The dot's radius (px)."),
    fill: t6.prop("The dot's ink (default: the colour scale, else $mark)."),
    stroke: t6.prop("The stem's ink (default: the dot's)."),
    labels: t6.bool(false, "Value labels past the dots (on the other side where the plot has no room)."),
    format: t6.string(",.1~f", "Number format of the labels (and the tooltip)."),
    prefix: t6.string(void 0, "Before each number ('$')."),
    suffix: t6.string(void 0, "After it ('M', ' kr')."),
    label: t6.prop("Dot label (tooltip, accessible name): an expression over the row; default `name: value`.")
  },
  tokens: ["mark", "ink-2", "paper", "size.label"],
  expand(p) {
    const horizontal = isBand3(p.yType) && !isBand3(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const fill = p.fill ?? colorOr2(p.color, "$mark");
    const c = at(cs, cat, "band"), v = `scale.${vs}(d.${val})`, zero = `scale.${vs}(0)`;
    const pt = (a, b) => horizontal ? [e6(b), e6(a)] : [e6(a), e6(b)];
    const stem = horizontal ? geom6.segment({ x1: e6(zero), y1: e6(c), x2: e6(v), y2: e6(c) }) : geom6.segment({ x1: e6(c), y1: e6(zero), x2: e6(c), y2: e6(v) });
    const num = written(`d.${val}`, p);
    const lw = `measure(${num}, token("size.label"))`;
    const out = `(d.${val} >= 0)`;
    const past = horizontal ? `(${out} ? ${v} + ${p.r + 4} + ${lw} <= box.w : ${v} - ${p.r + 4} - ${lw} >= 0)` : `(${out} ? ${v} - ${p.r + 4} - token("size.label") >= 0 : ${v} + ${p.r + 4} + token("size.label") <= box.h)`;
    const ahead = `(${out} == ${past})`;
    const labelAt = horizontal ? pt(c, `${ahead} ? ${v} + ${p.r + 4} : ${v} - ${p.r + 4}`) : pt(c, `${ahead} ? ${v} - ${p.r + 4} : ${v} + ${p.r + 4}`);
    const style = horizontal ? { size: "$size.label", ink: "$ink-2", align: e6(`${ahead} ? "start" : "end"`), baseline: "middle", contain: true } : { size: "$size.label", ink: "$ink-2", align: "middle", baseline: e6(`${ahead} ? "alphabetic" : "top"`), contain: true };
    return group6({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        group6({ key: "stems", children: [repeat6(p.data, shape6(stem, { stroke: { paint: p.stroke ?? fill, width: 2, cap: "round" }, semantics: { role: "decoration" } }))] }),
        // The dots are the data: keyed by the row directly, like bars, so the two morph.
        repeat6(p.data, shape6(geom6.circle({ cx: pt(c, v)[0], cy: pt(c, v)[1], r: p.r }), {
          fill,
          semantics: { role: "datum", label: p.label ?? e6(`key.name(d.${cat}) + ": " + ${num}`), value: e6(`d.${val}`) },
          pickable: true
        })),
        p.labels ? group6({ key: "labels", children: [repeat6(p.data, text5(e6(num), labelAt, { halo: ["$paper", 2], style }))] }) : null
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0 } }]
});
var dumbbell = recipe6({
  id: "@datars/std/dumbbell",
  doc: "Two values per category joined by a bar: a dot per row (coloured by the plot's colour field \u2014 the year, the group), a connector from each category's lowest to highest. Rows are long: one per (category, value). Horizontal when y is the band axis.",
  params: {
    data: t6.table("One row per dot: (category, colour field, value)."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Which dot is which (a year, a group): the colour scale."),
    xType: t6.string("linear", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("band", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    r: t6.number(5, "Dot radius (px)."),
    stroke: t6.prop("The connector's ink (default $rule)."),
    labels: t6.bool(false, "Each category's lowest value before its low dot and highest after its high dot."),
    format: t6.string(",.1~f", "Number format of labels and tooltips."),
    prefix: t6.string(void 0, "Before each number ('$')."),
    suffix: t6.string(void 0, "After it ('M', ' kr')."),
    label: t6.prop("Dot label (tooltip, accessible name): an expression over the row; default `category, colour: value`.")
  },
  tokens: ["mark", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const horizontal = isBand3(p.yType) && !isBand3(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const ends = cx.table("dumbbell", p.data, op4.aggregate([cat], { lo: ["min", val], hi: ["max", val], first: ["first", val], last: ["last", val] }));
    const c = at(cs, cat, "band");
    const pos = (v) => horizontal ? [e6(`scale.x(${v})`), e6(c)] : [e6(c), e6(`scale.y(${v})`)];
    const [a, b] = [pos("d.lo"), pos("d.hi")];
    const dotLabel = p.color && p.color !== cat ? e6(`key.name(d.${cat}) + ", " + key.name(d.${p.color}) + ": " + ${written(`d.${val}`, p)}`) : e6(`key.name(d.${cat}) + ": " + ${written(`d.${val}`, p)}`);
    const gap = p.r + 4;
    const lo = horizontal ? [e6(`scale.x(d.lo) - ${gap}`), e6(c)] : [e6(c), e6(`scale.y(d.lo) + ${gap}`)];
    const hi = horizontal ? [e6(`scale.x(d.hi) + ${gap}`), e6(c)] : [e6(c), e6(`scale.y(d.hi) - ${gap}`)];
    const st = (side) => horizontal ? { size: "$size.label", ink: "$ink-2", align: side === "lo" ? "end" : "start", baseline: "middle", contain: true } : { size: "$size.label", ink: "$ink-2", align: "middle", baseline: side === "lo" ? "top" : "alphabetic", contain: true };
    return group6({
      key: "marks",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        group6({ key: "connectors", children: [repeat6(ends, shape6(geom6.segment({ x1: a[0], y1: a[1], x2: b[0], y2: b[1] }), {
          stroke: { paint: p.stroke ?? "$rule", width: 3, cap: "round" },
          semantics: { role: "datum", label: e6(`key.name(d.${cat}) + ": " + ${written("d.first", p)} + " \u2192 " + ${written("d.last", p)}`), value: e6("d.last - d.first") },
          pickable: true
        }))] }),
        repeat6(p.data, shape6(geom6.circle({ cx: pos(`d.${val}`)[0], cy: pos(`d.${val}`)[1], r: p.r }), {
          fill: colorOr2(p.color, "$mark"),
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: p.label ?? dotLabel, value: e6(`d.${val}`) },
          pickable: true
        })),
        p.labels ? group6({ key: "labels", children: [repeat6(ends, group6({ children: [
          // Only where it fits inside the plot area (past its edge it would run into the axis).
          text5(e6(written("d.lo", p)), lo, { key: "lo", when: e6(horizontal ? `scale.x(d.lo) - ${gap} - measure(${written("d.lo", p)}, token("size.label")) >= 0` : `scale.y(d.lo) + ${gap} + token("size.label") <= box.h`), style: st("lo") }),
          text5(e6(written("d.hi", p)), hi, { key: "hi", when: e6("d.hi != d.lo"), style: st("hi") })
        ] }))] }) : null
      ]
    });
  }
});
function slopeTable(cx, p, name = "slope") {
  const series = p.series ?? p.color;
  return cx.table(name, p.data, op4.sort(p.x), op4.aggregate([series], { x0: ["first", p.x], x1: ["last", p.x], start: ["first", p.y], end: ["last", p.y] }));
}
function slopeText(p, side) {
  const series = p.series ?? p.color;
  const name = `key.name(d.${series})`;
  if (!p.values) return name;
  return side === "start" ? `${name} + "  " + ${written("d.start", p)}` : `${written("d.end", p)} + "  " + ${name}`;
}
function slopeRoom(cx, p) {
  if (p.labels === "none") return {};
  const tbl = cx.table("slope-widths", slopeTable(cx, p, "slope-rows"), op4.derive("w0", e6(`measure(${slopeText(p, "start")}, token("size.label"))`)), op4.derive("w1", e6(`measure(${slopeText(p, "end")}, token("size.label"))`)));
  const room = (w) => e6(`min(box.w * 0.34, table.max(${JSON.stringify(tbl)}, "${w}") + 14)`);
  return { start: p.labels === "end" ? void 0 : room("w0"), end: p.labels === "start" ? void 0 : room("w1") };
}
var slope = recipe6({
  id: "@datars/std/slope",
  doc: "A slope chart: each series' first and last value (two years, before and after) joined by a line, named at both ends \u2014 labels pushed apart so none overlap, the plot leaving them room. Rows are long: (series, x, value) on a point x scale.",
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("The time (or before/after) field, on a point scale."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("point", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    series: t6.field("One line per value of this field (default: the colour field)."),
    labels: t6.oneOf(["both", "start", "end", "none"], "both", "Where each line is named."),
    values: t6.bool(true, "Each label carries its value (`Norway  78.7` \xB7 `83.2  Norway`)."),
    format: t6.string(",.1~f", "Number format of values in labels and tooltips."),
    prefix: t6.string(void 0, "Before each number ('$')."),
    suffix: t6.string(void 0, "After it ('M', ' kr')."),
    stroke: t6.prop('Line ink or expression \u2014 over the row `d.start` and `d.end` (a series\' first and last values) and the series field: `e(\'d.end > d.start ? "$positive" : "$negative"\')`. Default: the colour scale, else $mark.'),
    width: t6.number(2, "Line width (px)."),
    r: t6.number(4, "Dot radius at each end (px); 0 for none."),
    highlight: t6.json("Series to pick out (a name or a list): they keep their ink and labels, the others turn grey."),
    label: t6.prop("Line label (tooltip, accessible name); default `name: start \u2192 end`.")
  },
  tokens: ["mark", "muted", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const pairs = slopeTable(cx, p);
    const on = among(series, p.highlight);
    const hl = p.highlight !== void 0 && p.highlight !== null;
    const base = p.stroke ?? (p.color ? e6(`scale.color(d.${p.color})`) : "$mark");
    const ink = hl ? e6(`${on} ? ${typeof base === "string" ? JSON.stringify(base) : base.expr} : "$rule"`) : base;
    const fade = hl ? e6(`${on} ? 1 : 0.5`) : void 0;
    const x0 = at("x", "x0", p.xType), x1 = at("x", "x1", p.xType);
    const gap = 'token("size.label") + 3';
    const placed = cx.table(
      "slope-labels",
      pairs,
      op4.spread({ position: e6("scale.y(d.start)"), gap: e6(gap), min: 0, max: e6("box.h"), as: "ly0" }),
      op4.spread({ position: e6("scale.y(d.end)"), gap: e6(gap), min: 0, max: e6("box.h"), as: "ly1" })
    );
    const labelInk = hl ? e6(`${on} ? "$ink-2" : "$muted"`) : "$ink-2";
    const side = (s) => text5(e6(slopeText(p, s)), [e6(s === "start" ? `${x0} - ${p.r + 6}` : `${x1} + ${p.r + 6}`), e6(s === "start" ? "d.ly0" : "d.ly1")], {
      style: { size: "$size.label", ink: labelInk, weight: hl ? e6(`${on} ? 600 : 400`) : void 0, align: s === "start" ? "end" : "start", baseline: "middle" }
    });
    const dot2 = (xe, v, key) => shape6(geom6.circle({ cx: e6(xe), cy: e6(`scale.y(${v})`), r: p.r }), { key, fill: ink, opacity: fade, semantics: { role: "decoration" } });
    return group6({
      key: "slopes",
      semantics: { role: "series", label: `${p.y} by ${series}` },
      children: [
        // The lines are the data: one per series, keyed by it.
        repeat6(pairs, shape6(geom6.segment({ x1: e6(x0), y1: e6("scale.y(d.start)"), x2: e6(x1), y2: e6("scale.y(d.end)") }), {
          stroke: { paint: ink, width: hl ? e6(`${on} ? ${p.width} : ${Math.max(1, p.width - 0.5)}`) : p.width, cap: "round" },
          opacity: fade,
          semantics: { role: "datum", label: p.label ?? e6(`key.name(d.${series}) + ": " + ${written("d.start", p)} + " \u2192 " + ${written("d.end", p)}`), value: e6("d.end - d.start") },
          pickable: true
        })),
        p.r > 0 ? group6({ key: "dots", children: [repeat6(pairs, group6({ children: [dot2(x0, "d.start", "start"), dot2(x1, "d.end", "end")] }))] }) : null,
        p.labels === "both" || p.labels === "start" ? group6({ key: "start-labels", children: [repeat6(placed, side("start"))] }) : null,
        p.labels === "both" || p.labels === "end" ? group6({ key: "end-labels", children: [repeat6(placed, side("end"))] }) : null
      ]
    });
  },
  motion: [{ select: { kind: "segment" }, enter: { trim: 0 } }]
});
var bump = recipe6({
  id: "@datars/std/bump",
  doc: "A bump chart: each series' rank at every time, 1 at the top, joined by lines and named at both ends. Ranks come from the values at each time (largest first) or from a rank field; rows are long (series, time, value).",
  params: {
    data: t6.table("One row per (series, time)."),
    x: t6.field("The time field (in order: it's sorted)."),
    y: t6.field("The value ranked at each time (or the rank itself, with `rank`)."),
    series: t6.field("One line per value of this field."),
    rank: t6.bool(false, "`y` already holds ranks (1 = top)."),
    reverse: t6.bool(false, "The smallest value ranks first (times, prices)."),
    title: t6.string(void 0, "A title above the chart."),
    subtitle: t6.string(void 0, "A line under the title."),
    highlight: t6.json("Series to pick out (a name or a list): they keep their colour and draw on top, the others turn grey."),
    format: t6.string(",.4~g", "Number format of values in tooltips."),
    r: t6.number(4.5, "Dot radius at each time (px)."),
    width: t6.number(3, "Line width (px)."),
    labelSpace: t6.number(140, "Most room (px) the names take at each side.")
  },
  tokens: ["ink", "ink-2", "muted", "grid", "rule", "paper", "size.label", "size.title", "font.title"],
  expand(p, cx) {
    const s = p.series;
    const ranked = cx.table(
      "bump",
      p.data,
      p.rank ? op4.derive("__rank", e6(`d.${p.y}`)) : op4.window("rank", p.y, "__rank", { partition: [p.x], order: p.reverse ? p.y : `-${p.y}` }),
      op4.derive("__lo", e6("d.__rank - 0.5")),
      op4.derive("__hi", e6("d.__rank + 0.5")),
      op4.sort(p.x)
    );
    const on = among(s, p.highlight);
    const hl = p.highlight !== void 0 && p.highlight !== null;
    const drawn = hl ? cx.table("bump-drawn", ranked, op4.derive("__on", e6(`${on} ? 1 : 0`)), op4.sort("__on", p.x)) : ranked;
    const ends = cx.table("bump-ends", drawn, op4.aggregate([s], { x0: ["first", p.x], r0: ["first", "__rank"], x1: ["last", p.x], r1: ["last", "__rank"] }), op4.derive("w", e6(`measure(key.name(d.${s}), token("size.label"), 600)`)));
    const room = e6(`min(${p.labelSpace}, table.max(${JSON.stringify(ends)}, "w") + 12)`);
    const ink = hl ? e6(`${on} ? scale.color(d.${s}) : "$rule"`) : e6(`scale.color(d.${s})`);
    const fade = hl ? e6(`${on} ? 1 : 0.45`) : void 0;
    const X = `scale.x(d.${p.x})`, Y = "scale.y(d.__rank)";
    const value = p.rank ? "" : ` + " (" + format(d.${p.y}, ${JSON.stringify(p.format)}) + ")"`;
    const pointLabel = e6(`key.name(d.${s}) + ", " + key.name(d.${p.x}) + ": rank " + d.__rank${value}`);
    const scales = {
      x: { type: "point", domain: { data: ranked, field: p.x }, range: { box: "bump-area", axis: "x" }, padding: 0 },
      y: { type: "linear", domain: { data: ranked, fields: ["__lo", "__hi"] }, range: { box: "bump-area", axis: "y" }, zero: false, nice: false },
      color: { type: "categorical", domain: { data: p.data, field: s }, range: "$categorical" }
    };
    const name = (end) => text5(e6(`key.name(d.${s})`), [e6(end === "start" ? `scale.x(d.x0) - ${p.r + 6}` : `scale.x(d.x1) + ${p.r + 6}`), e6(end === "start" ? "scale.y(d.r0)" : "scale.y(d.r1)")], {
      style: { size: "$size.label", weight: 600, ink: hl ? e6(`${on} ? scale.color(d.${s}) : "$muted"`) : e6(`scale.color(d.${s})`), align: end === "start" ? "end" : "start", baseline: "middle" }
    });
    return group6({
      key: "bump",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `Rank by ${p.y}` },
      children: [
        p.title ? text5(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e6("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text5(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e6("box.w") } }) : null,
        group6({
          key: "body",
          layout: { type: "columns", gap: 0 },
          children: [
            group6({ key: "start-room", size: { w: room } }),
            group6({
              key: "center",
              layout: { type: "rows", gap: 8 },
              children: [
                group6({
                  id: "bump-area",
                  key: "area",
                  children: [
                    grid({ scale: "x", orient: "vertical" }),
                    repeat6({ groups: drawn, by: s }, group6({
                      children: [
                        shape6(geom6.polyline({ from: "@group", x: e6(X), y: e6(Y), curve: "monotone-x" }), {
                          key: "line",
                          opacity: fade,
                          stroke: { paint: ink, width: hl ? e6(`${on} ? ${p.width} : ${Math.max(1, p.width - 1.5)}`) : p.width, join: "round", cap: "round" },
                          semantics: { role: "series", label: e6(`key.name(d.${s})`) }
                        }),
                        instances2({ key: "points", from: "@group", x: e6(X), y: e6(Y), r: hl ? e6(`${on} ? ${p.r} : ${Math.max(2, p.r - 1.5)}`) : p.r, fill: ink, opacity: fade, stroke: { paint: "$paper", width: 1.5 }, instanceKey: e6(`d.${p.x}`), label: pointLabel })
                      ]
                    })),
                    group6({ key: "start-labels", children: [repeat6(ends, name("start"))] }),
                    group6({ key: "end-labels", children: [repeat6(ends, name("end"))] })
                  ]
                }),
                axis({ scale: "x", orient: "bottom", type: "point", data: ranked, field: p.x, line: false }, { size: { h: "auto" } })
              ]
            }),
            group6({ key: "end-room", size: { w: room } })
          ]
        })
      ]
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }]
});
var pyramid = recipe6({
  id: "@datars/std/pyramid",
  doc: "A population pyramid: a band per group (age, youngest at the bottom), one side's bars growing left and the other's right from a column of group names, both on one scale so the sides compare. Rows are long (group, side, value).",
  params: {
    data: t6.table("One row per (group, side)."),
    y: t6.field("The groups up the middle (ages), in data order from the bottom."),
    side: t6.field("The field with two values, one per side (sex)."),
    value: t6.field("The numbers (each row's count, sales, share)."),
    left: t6.string(void 0, "The value of `side` drawn on the left (default: the first in the data)."),
    title: t6.string(void 0, "A title above the chart."),
    subtitle: t6.string(void 0, "A line under the title."),
    format: t6.string(void 0, "Number format of the axes, labels and tooltips ('.1%', ',.0f')."),
    labels: t6.bool(false, "Value labels at the bars' ends."),
    prefix: t6.string(void 0, "Before each number."),
    suffix: t6.string(void 0, "After it (' %', 'k')."),
    padding: t6.number(0.12, "Space between the bars (band fraction).")
  },
  tokens: ["ink", "ink-2", "muted", "size.label", "size.title", "font.title", "categorical"],
  expand(p, cx) {
    const rows = cx.table("pyramid", p.data, op4.window("first", p.side, "__first"));
    const isLeft = p.left !== void 0 ? `d.${p.side} == ${JSON.stringify(p.left)}` : `d.${p.side} == d.__first`;
    const leftRows = cx.table("pyramid-left", rows, op4.filter(e6(isLeft)));
    const rightRows = cx.table("pyramid-right", rows, op4.filter(e6(`!(${isLeft})`)));
    const sides = cx.table("pyramid-sides", rows, op4.aggregate([p.side], { __first: ["first", "__first"] }));
    const groups = cx.table("pyramid-groups", p.data, op4.aggregate([p.y], { n: ["count"] }), op4.derive("w", e6(`measure(key.name(d.${p.y}), token("size.label"))`)));
    const gutter = `(table.max(${JSON.stringify(groups)}, "w") + 18)`;
    const fmt = { format: p.format ?? ",.4~g", prefix: p.prefix, suffix: p.suffix };
    const valueScale = (box, axisDir) => ({ type: "linear", domain: { data: p.data, field: p.value }, range: { box, axis: axisDir }, zero: true, nice: true });
    const scales = {
      y: { type: "band", domain: { data: p.data, field: p.y }, range: { box: "pyramid-left", axis: "-y" }, padding: p.padding },
      xl: valueScale("pyramid-left", "-x"),
      xr: valueScale("pyramid-right", "x"),
      color: { type: "categorical", domain: { data: p.data, field: p.side }, range: "$categorical" }
    };
    const bar2 = (s) => shape6(geom6.rect({
      x: e6(s === "xl" ? `scale.xl(d.${p.value})` : "0"),
      y: e6(`scale.y(d.${p.y})`),
      w: e6(s === "xl" ? `scale.xl(0) - scale.xl(d.${p.value})` : `scale.xr(d.${p.value})`),
      h: e6("scale.y.bandwidth()")
    }), {
      fill: e6(`scale.color(d.${p.side})`),
      semantics: { role: "datum", label: e6(`key.name(d.${p.side}) + ", " + key.name(d.${p.y}) + ": " + ${written(`d.${p.value}`, fmt)}`), value: e6(`d.${p.value}`) },
      pickable: true
    });
    const valueLabel = (s) => text5(e6(written(`d.${p.value}`, fmt)), [e6(s === "xl" ? `scale.xl(d.${p.value}) - 4` : `scale.xr(d.${p.value}) + 4`), e6("scale.y(d." + p.y + ") + scale.y.bandwidth() / 2")], {
      when: e6('scale.y.bandwidth() >= token("size.label") - 2'),
      style: { size: "$size.label", ink: "$ink-2", align: s === "xl" ? "end" : "start", baseline: "middle", contain: true }
    });
    const head = (left) => group6({ key: "head", size: { h: "auto" }, children: [repeat6(sides, text5(e6(`key.name(d.${p.side})`), [left ? e6("box.w") : 0, 0], {
      when: e6(left ? isLeft : `!(${isLeft})`),
      style: { size: "$size.label", weight: 600, ink: e6(`scale.color(d.${p.side})`), align: left ? "end" : "start", baseline: "top" }
    }))] });
    const pane = (s) => group6({
      key: s === "xl" ? "left" : "right",
      layout: { type: "rows", gap: 6 },
      children: [
        head(s === "xl"),
        group6({
          id: s === "xl" ? "pyramid-left" : "pyramid-right",
          key: "area",
          children: [
            grid({ scale: s, orient: "vertical" }),
            group6({ key: "bars", children: [repeat6(s === "xl" ? leftRows : rightRows, bar2(s))] }),
            p.labels ? group6({ key: "labels", children: [repeat6(s === "xl" ? leftRows : rightRows, valueLabel(s))] }) : null,
            // The group names, centred in the gutter between the two sides.
            s === "xl" ? group6({ key: "groups", children: [repeat6(groups, text5(e6(`key.name(d.${p.y})`), [e6(`box.w + ${gutter} / 2`), e6(`scale.y(d.${p.y}) + scale.y.bandwidth() / 2`)], {
              when: e6('scale.y.bandwidth() >= token("size.label") * 0.8'),
              style: { size: "$size.label", ink: "$muted", align: "middle", baseline: "middle" }
            }))] }) : null
          ]
        }),
        axis({ scale: s, orient: "bottom", type: "linear", format: p.format, prefix: p.prefix, suffix: p.suffix }, { size: { h: "auto" } })
      ]
    });
    return group6({
      key: "pyramid",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `${p.value} by ${p.y} and ${p.side}` },
      children: [
        p.title ? text5(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e6("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text5(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e6("box.w") } }) : null,
        group6({ key: "body", layout: { type: "columns", gap: 0 }, children: [pane("xl"), group6({ key: "gutter", size: { w: e6(gutter) } }), pane("xr")] })
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0 } }]
});
var marimekko = recipe6({
  id: "@datars/std/marimekko",
  doc: "A marimekko (mosaic): a column per category as wide as its share of the total, split into segments as tall as each series' share within it \u2014 so every area is that pair's share of everything. Segments are keyed (series, column), like stacked bars.",
  params: {
    data: t6.table("One row per (column, series)."),
    x: t6.field("The columns (markets, regions)."),
    series: t6.field("The segments within each column (and the colours)."),
    value: t6.field("The numbers (each row's count, sales, share)."),
    title: t6.string(void 0, "A title above the chart."),
    subtitle: t6.string(void 0, "A line under the title."),
    format: t6.string(",.4~g", "Number format of values in tooltips."),
    labels: t6.bool(true, "Series names and shares inside the segments that fit them."),
    legend: t6.bool(true, "A colour legend below."),
    gap: t6.number(2, "Gap between segments (px).")
  },
  tokens: ["ink", "ink-2", "muted", "size.label", "size.small", "size.title", "font.title", "categorical"],
  expand(p, cx) {
    const cols = cx.table("marimekko-columns", p.data, op4.aggregate([p.x], { __total: ["sum", p.value] }), op4.window("share_of_total", "__total", "__share"), op4.window("cumsum", "__share", "__x1"), op4.derive("__x0", e6("d.__x1 - d.__share")));
    const cells = cx.table("marimekko", p.data, op4.stack({ x: p.x, series: p.series, value: p.value, offset: "expand", as: ["y0", "y1"] }), op4.join(cols, p.x));
    const g = p.gap / 2;
    const [x0, x1, y0, y1] = ["scale.mx(d.__x0)", "scale.mx(d.__x1)", "scale.my(d.y0)", "scale.my(d.y1)"];
    const w = `(${x1} - ${x0} - ${p.gap})`, h = `(${y0} - ${y1} - ${p.gap})`;
    const name = `key.name(d.${p.series})`;
    const share = `format(d.y1 - d.y0, ".0%")`;
    const fits = `${w} >= max(measure(${name}, token("size.label"), 600), measure(${share}, token("size.label"))) + 10 && ${h} >= 2 * token("size.label") + 10`;
    const onFill = e6(`"on(" + scale.color(d.${p.series}) + ")"`);
    const colName = `key.name(d.${p.x})`;
    const colW = `(scale.mx(d.__x1) - scale.mx(d.__x0) - ${p.gap})`;
    return group6({
      key: "marimekko",
      scales: {
        mx: { type: "linear", domain: [0, 1], range: { box: "marimekko-area", axis: "x" } },
        my: { type: "linear", domain: [0, 1], range: { box: "marimekko-area", axis: "-y" } },
        color: { type: "categorical", domain: { data: p.data, field: p.series }, range: "$categorical" }
      },
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? `${p.value} by ${p.x} and ${p.series}` },
      children: [
        p.title ? text5(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e6("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text5(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e6("box.w") } }) : null,
        group6({
          key: "body",
          layout: { type: "columns", gap: 6 },
          children: [
            axis({ scale: "my", orient: "left", format: ".0%", line: false }, { size: { w: "auto" } }),
            group6({
              key: "center",
              layout: { type: "rows", gap: 6 },
              children: [
                group6({
                  id: "marimekko-area",
                  key: "area",
                  children: [
                    repeat6(cells, shape6(geom6.rect({ x: e6(`${x0} + ${g}`), y: e6(`${y1} + ${g}`), w: e6(`max(0, ${w})`), h: e6(`max(0, ${h})`) }), {
                      key: [e6(`d.${p.series}`), e6(`d.${p.x}`)],
                      fill: e6(`scale.color(d.${p.series})`),
                      semantics: { role: "datum", label: e6(`${name} + " in " + ${colName} + ": " + format(d.${p.value}, ${JSON.stringify(p.format)}) + " (" + ${share} + " of " + ${colName} + ", " + format((d.y1 - d.y0) * d.__share, ".1%") + " of all)"`), value: e6(`d.${p.value}`) },
                      pickable: true
                    })),
                    p.labels ? group6({ key: "labels", children: [repeat6(cells, group6({ when: e6(fits), children: [
                      text5(e6(name), [e6(`${x0} + ${g} + 6`), e6(`${y1} + ${g} + 5`)], { key: "name", style: { size: "$size.label", weight: 600, ink: onFill, baseline: "top" } }),
                      text5(e6(share), [e6(`${x0} + ${g} + 6`), e6(`${y1} + ${g} + 7 + token("size.label")`)], { key: "share", style: { size: "$size.label", ink: onFill, baseline: "top" } })
                    ] }))] }) : null
                  ]
                }),
                // Each column named under it, with its share of the total — where it has the width.
                group6({ key: "columns", size: { h: "auto" }, children: [repeat6(cols, group6({ when: e6(`${colW} >= 28`), children: [
                  text5(e6(colName), [e6("(scale.mx(d.__x0) + scale.mx(d.__x1)) / 2"), 0], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e6(colW) } }),
                  text5(e6('format(d.__share, ".0%")'), [e6("(scale.mx(d.__x0) + scale.mx(d.__x1)) / 2"), e6('token("size.label") + 5')], { key: "share", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } })
                ] }))] })
              ]
            })
          ]
        }),
        p.legend ? legend({ scale: "color" }, { size: { h: "auto" } }) : null
      ]
    });
  }
});
function histogramTable(cx, p) {
  const color = p.color && p.color !== p.x ? p.color : void 0;
  const y = p.density ? "density" : "count";
  return cx.table(
    "histogram",
    p.data,
    op4.filter(e6(`d.${p.x} == d.${p.x}`)),
    // no nulls (NaN is not itself)
    op4.bin(p.x, "bin", p.step > 0 ? { step: p.step } : { count: p.bins }),
    op4.aggregate(color ? ["bin", "bin_end", color] : ["bin", "bin_end"], { count: ["count"] }),
    op4.window("share_of_total", "count", "__share"),
    op4.derive("density", e6("d.__share / (d.bin_end - d.bin)")),
    ...color ? [op4.stack({ x: "bin", series: color, value: y, as: ["y0", "y1"] })] : [op4.derive("y0", 0), op4.derive("y1", e6(`d.${y}`))]
  );
}
var histogram = recipe6({
  id: "@datars/std/histogram",
  doc: "A histogram: the rows of `x` counted into bins of equal width (about `bins` of them at a round width, or exactly `step` wide), a bar per bin from its start to its end. `density` shows the share per unit instead, so the bars' area is 1; a colour field stacks the bins. The plot's axes fit the bins.",
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("The numeric field to bin."),
    color: t6.field("Stack each bin by this field."),
    xType: t6.string("linear", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    bins: t6.number(10, "About this many bins, at a round width (1, 2, 5 \xD7 10\u207F)."),
    step: t6.number(0, "Bins exactly this wide (anchored at 0), instead of `bins`."),
    density: t6.bool(false, "Bar heights as density (share of rows per unit of x) instead of counts: the bars' total area is 1, whatever the bin width."),
    gap: t6.number(1, "Gap between bars (px)."),
    fill: t6.prop("Bar ink (default: the colour scale, else $mark)."),
    format: t6.string(",.6~g", "Number format of the bin edges in tooltips."),
    label: t6.prop("Bar label (tooltip, accessible name); default `start\u2013end: count`.")
  },
  tokens: ["mark"],
  expand(p, cx) {
    const tbl = histogramTable(cx, p);
    const color = p.color && p.color !== p.x ? p.color : void 0;
    const f = JSON.stringify(p.format);
    const what = p.density ? `format(d.density, ".3~g") + " per unit (" + format(d.__share, ".1%") + ")"` : `format(d.count, ",d")`;
    return group6({
      key: "bins",
      semantics: { role: "series", label: `${p.x}, binned` },
      children: [repeat6(tbl, shape6(geom6.rect({ x: e6(`scale.x(d.bin) + ${p.gap / 2}`), y: e6("scale.y(d.y1)"), w: e6(`max(0.5, scale.x(d.bin_end) - scale.x(d.bin) - ${p.gap})`), h: e6("max(0, scale.y(d.y0) - scale.y(d.y1))") }), {
        key: color ? [e6(`d.${color}`), e6("d.bin")] : e6("d.bin"),
        fill: p.fill ?? colorOr2(color, "$mark"),
        semantics: { role: "datum", label: p.label ?? e6(`${color ? `key.name(d.${color}) + ", " + ` : ""}format(d.bin, ${f}) + "\u2013" + format(d.bin_end, ${f}) + ": " + ${what}`), value: e6(p.density ? "d.density" : "d.count") },
        pickable: true
      }))]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }]
});
var boxplot = recipe6({
  id: "@datars/std/boxplot",
  doc: "A box plot per category: the box spans the middle half (first to third quartile) with a line at the median; whiskers reach the furthest values within 1.5 \xD7 the box's height (`whisker`) of it, and values beyond are drawn as outliers. Horizontal when y is the band axis.",
  params: {
    data: t6.table("One row per observation."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("band", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("linear", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    whisker: t6.number(1.5, "How far whiskers reach, in interquartile ranges beyond the box (Tukey's 1.5)."),
    width: t6.number(0.7, "Box width as a fraction of the band (at most 72 px)."),
    outliers: t6.bool(true, "Draw the values beyond the whiskers."),
    fill: t6.prop("Box ink (default: the colour scale, else $mark)."),
    format: t6.string(",.1~f", "Number format in tooltips."),
    label: t6.prop("Box label (tooltip, accessible name); default the five numbers and the count.")
  },
  tokens: ["mark", "ink-2", "paper"],
  expand(p, cx) {
    const horizontal = isBand3(p.yType) && !isBand3(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const color = p.color && p.color !== cat ? p.color : void 0;
    const stats = cx.table("box-stats", p.data, op4.aggregate([cat], { __q1: ["q25", val], __median: ["median", val], __q3: ["q75", val], __n: ["count", val] }));
    const k = p.whisker;
    const rows = cx.table("box-rows", p.data, op4.join(stats, cat), op4.derive("__out", e6(`d.${val} < d.__q1 - ${k} * (d.__q3 - d.__q1) || d.${val} > d.__q3 + ${k} * (d.__q3 - d.__q1)`)));
    const box = cx.table("box", rows, op4.filter(e6("!d.__out")), op4.aggregate([cat], { lo: ["min", val], hi: ["max", val], q1: ["first", "__q1"], median: ["first", "__median"], q3: ["first", "__q3"], n: ["first", "__n"], ...color ? { [color]: ["first", color] } : {} }));
    const outliers = cx.table("box-outliers", rows, op4.filter(e6("d.__out")));
    const fill = p.fill ?? (p.color ? e6(`scale.color(d.${p.color})`) : "$mark");
    const c = at(cs, cat, "band");
    const half = `min(scale.${cs}.bandwidth() * ${p.width}, 72) / 2`;
    const V = (v) => `scale.${vs}(${v})`;
    const P = (along, across) => horizontal ? [e6(along), e6(across)] : [e6(across), e6(along)];
    const seg = (a0, c0, a1, c1) => {
      const [x1, y1] = P(a0, c0), [x2, y2] = P(a1, c1);
      return geom6.segment({ x1, y1, x2, y2 });
    };
    const rect = horizontal ? geom6.rect({ x: e6(V("d.q1")), y: e6(`${c} - ${half}`), w: e6(`${V("d.q3")} - ${V("d.q1")}`), h: e6(`2 * ${half}`) }) : geom6.rect({ x: e6(`${c} - ${half}`), y: e6(V("d.q3")), w: e6(`2 * ${half}`), h: e6(`${V("d.q1")} - ${V("d.q3")}`) });
    const f = (v) => `format(${v}, ${JSON.stringify(p.format)})`;
    const summary = e6(`key.name(d.${cat}) + ": median " + ${f("d.median")} + ", middle half " + ${f("d.q1")} + "\u2013" + ${f("d.q3")} + ", whiskers " + ${f("d.lo")} + "\u2013" + ${f("d.hi")} + " (n = " + format(d.n, ",d") + ")"`);
    const whiskerInk = "$ink-2";
    return group6({
      key: "boxes",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        repeat6(box, group6({ children: [
          shape6(seg(V("d.lo"), c, V("d.q1"), c), { key: "whisker-lo", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape6(seg(V("d.q3"), c, V("d.hi"), c), { key: "whisker-hi", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape6(seg(V("d.lo"), `${c} - ${half} / 2`, V("d.lo"), `${c} + ${half} / 2`), { key: "cap-lo", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape6(seg(V("d.hi"), `${c} - ${half} / 2`, V("d.hi"), `${c} + ${half} / 2`), { key: "cap-hi", stroke: { paint: whiskerInk, width: 1.2 }, semantics: { role: "decoration" } }),
          shape6(rect, { key: "box", fill, semantics: { role: "datum", label: p.label ?? summary, value: e6("d.median") }, pickable: true }),
          shape6(seg(V("d.median"), `${c} - ${half}`, V("d.median"), `${c} + ${half}`), { key: "median", stroke: { paint: "$paper", width: 2 }, semantics: { role: "decoration" } })
        ] })),
        // Outliers are few: a hollow circle each, ringed in its box's ink.
        p.outliers ? group6({ key: "outliers", children: [repeat6(outliers, shape6(geom6.circle({ cx: P(V(`d.${val}`), c)[0], cy: P(V(`d.${val}`), c)[1], r: 3 }), {
          fill: "$paper",
          stroke: { paint: fill, width: 1.3 },
          semantics: { role: "datum", label: e6(`key.name(d.${cat}) + ": " + ${f(`d.${val}`)} + " (beyond the whiskers)"`), value: e6(`d.${val}`) },
          pickable: true
        }))] }) : null
      ]
    });
  }
});
function densityTable(cx, name, p, cat, val, color, grid2) {
  const stats = cx.table(`${name}-stats`, p.data, op4.aggregate([cat], { __q1: ["q25", val], __median: ["median", val], __q3: ["q75", val], __n: ["count", val] }));
  const kde = cx.table(
    name,
    p.data,
    op4.kde({ field: val, groupby: color ? [cat, color] : [cat], bandwidth: p.bandwidth || void 0, steps: p.steps, ...grid2 }),
    op4.join(stats, cat),
    op4.derive("__g", e6(color ? `d.${cat} + " | " + d.${color}` : `d.${cat}`))
  );
  return { kde, stats };
}
var violin = recipe6({
  id: "@datars/std/violin",
  doc: "A violin per category: the values' kernel density (Gaussian, Silverman's bandwidth unless given) mirrored around the category's centre, over the range of the data \u2014 its shape shows where values bunch, where a box plot shows five numbers. A bar marks the middle half and a dot the median. Horizontal when y is the band axis.",
  params: {
    data: t6.table("One row per observation."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("band", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("linear", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    bandwidth: t6.number(0, "Kernel bandwidth in value units (0: Silverman's rule, per category). Smaller follows the data more closely; larger smooths."),
    steps: t6.number(80, "Points along each outline."),
    width: t6.number(0.9, "The widest violin's width as a fraction of the band."),
    normalize: t6.oneOf(["area", "width"], "area", "`area`: every violin encloses the same area, so a narrow spread is tall and wide; `width`: every violin is as wide as the band at its peak."),
    quartiles: t6.bool(true, "A bar over the middle half and a dot at the median."),
    fill: t6.prop("Ink (default: the colour scale, else $mark)."),
    opacity: t6.number(0.85, "Fill opacity."),
    format: t6.string(",.1~f", "Number format in tooltips.")
  },
  tokens: ["mark", "ink", "paper"],
  expand(p, cx) {
    const horizontal = isBand3(p.yType) && !isBand3(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const color = p.color && p.color !== cat ? p.color : void 0;
    const { kde, stats } = densityTable(cx, "violin", p, cat, val, color, { trim: true });
    const c = at(cs, cat, "band");
    const peak = p.normalize === "width" ? 'group.max("density")' : `table.max(${JSON.stringify(kde)}, "density")`;
    const half = `d.density / max(${peak}, 1e-300) * scale.${cs}.bandwidth() * ${p.width} / 2`;
    const fill = p.fill ?? (p.color ? e6(`scale.color(d.${p.color})`) : "$mark");
    const f = (v) => `format(${v}, ${JSON.stringify(p.format)})`;
    const label = e6(`key.name(d.${cat}) + ": median " + ${f("d.__median")} + ", middle half " + ${f("d.__q1")} + "\u2013" + ${f("d.__q3")} + " (n = " + format(d.__n, ",d") + ")"`);
    const outline = horizontal ? shape6(geom6.area({ from: "@group", x: e6(`scale.x(d.value)`), y0: e6(`${c} - ${half}`), y1: e6(`${c} + ${half}`), curve: "monotone-x" }), { key: "violin", fill, opacity: p.opacity, semantics: { role: "datum", label, value: e6("d.__median") }, pickable: true }) : shape6(geom6.area({ from: "@group", x: e6(`-scale.y(d.value)`), y0: e6(`${c} - ${half}`), y1: e6(`${c} + ${half}`), curve: "monotone-x" }), { key: "violin", transform: { rotate: -90 }, fill, opacity: p.opacity, semantics: { role: "datum", label, value: e6("d.__median") }, pickable: true });
    const V = (v) => `scale.${vs}(${v})`;
    const iqr = horizontal ? geom6.rect({ x: e6(V("d.__q1")), y: e6(`${c} - 1.5`), w: e6(`${V("d.__q3")} - ${V("d.__q1")}`), h: 3 }) : geom6.rect({ x: e6(`${c} - 1.5`), y: e6(V("d.__q3")), w: 3, h: e6(`${V("d.__q1")} - ${V("d.__q3")}`) });
    const med = horizontal ? geom6.circle({ cx: e6(V("d.__median")), cy: e6(c), r: 2.5 }) : geom6.circle({ cx: e6(c), cy: e6(V("d.__median")), r: 2.5 });
    return group6({
      key: "violins",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [
        repeat6({ groups: kde, by: "__g" }, group6({ children: [outline] })),
        p.quartiles ? group6({ key: "quartiles", children: [repeat6(stats, group6({ children: [
          shape6(iqr, { key: "iqr", fill: "$ink", opacity: 0.75, semantics: { role: "decoration" } }),
          shape6(med, { key: "median", fill: "$paper", semantics: { role: "decoration" } })
        ] }))] }) : null
      ]
    });
  }
});
function ridgelineTable(cx, p) {
  const color = p.color && p.color !== p.y ? p.color : void 0;
  return densityTable(cx, "ridgeline", p, p.y, p.x, color, { extend: 3 });
}
var ridgeline = recipe6({
  id: "@datars/std/ridgeline",
  doc: "Ridgelines: a kernel density curve per category (a row of the plot's band y axis) over a shared value axis, rising from the row's baseline into the rows above \u2014 many distributions (months, years, groups) compared in little space. Densities share one scale; the tallest ridge rises `overlap` rows, less if the top row would leave the plot.",
  params: {
    data: t6.table("One row per observation."),
    x: t6.field("The values (a linear x)."),
    y: t6.field("The categories (a band y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("linear", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("band", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    bandwidth: t6.number(0, "Kernel bandwidth in value units (0: Silverman's rule, per category)."),
    steps: t6.number(96, "Points along each curve."),
    overlap: t6.number(1.6, "How many rows the tallest ridge rises (1: it just meets the row above)."),
    fill: t6.prop("Ink (default: the colour scale, else $mark)."),
    opacity: t6.number(0.85, "Fill opacity."),
    format: t6.string(",.1~f", "Number format in tooltips.")
  },
  tokens: ["mark", "paper"],
  expand(p, cx) {
    const cat = p.y, val = p.x;
    const { kde } = ridgelineTable(cx, p);
    const peaks = cx.table("ridgeline-peaks", kde, op4.aggregate([cat], { m: ["max", "density"] }));
    const k = `min(${p.overlap} * scale.y.step() / max(table.max(${JSON.stringify(kde)}, "density"), 1e-300), (scale.y.step() + scale.y.bandwidth()) / 2 / max(table.first(${JSON.stringify(peaks)}, "m"), 1e-300))`;
    const base = `scale.y(d.${cat}) + scale.y.bandwidth()`;
    const fill = p.fill ?? (p.color ? e6(`scale.color(d.${p.color})`) : "$mark");
    const f = (v) => `format(${v}, ${JSON.stringify(p.format)})`;
    return group6({
      key: "ridges",
      semantics: { role: "series", label: `${val} by ${cat}` },
      children: [repeat6({ groups: kde, by: "__g" }, group6({ children: [
        shape6(geom6.area({ from: "@group", x: e6("scale.x(d.value)"), y0: e6(base), y1: e6(`${base} - d.density * ${k}`), curve: "monotone-x" }), {
          key: "ridge",
          fill,
          opacity: p.opacity,
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: e6(`key.name(d.${cat}) + ": median " + ${f("d.__median")} + ", middle half " + ${f("d.__q1")} + "\u2013" + ${f("d.__q3")} + " (n = " + format(d.__n, ",d") + ")"`), value: e6("d.__median") },
          pickable: true
        })
      ] }))]
    });
  }
});
function errorTable(cx, p) {
  const val = isBand3(p.yType) && !isBand3(p.xType) ? p.x : p.y;
  const lo = p.lo ? `d.${p.lo}` : p.error ? `d.${val} - d.${p.error}` : `d.${val}`;
  const hi = p.hi ? `d.${p.hi}` : p.error ? `d.${val} + d.${p.error}` : `d.${val}`;
  return cx.table("error-bars", p.data, op4.derive("__lo", e6(lo)), op4.derive("__hi", e6(hi)));
}
var errorBars = recipe6({
  id: "@datars/std/errorBars",
  doc: "Error bars: each row's interval as a line with caps \u2014 from `lo` to `hi` columns, or the value \xB1 `error` \u2014 around a dot at the value (a poll's margin, a confidence interval). Over bars, points or alone; the plot's value axis reaches the intervals' ends. Horizontal when y is the band axis.",
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("band", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("linear", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    lo: t6.field("The interval's low end (with `hi`)."),
    hi: t6.field("Its high end."),
    error: t6.field("A \xB1 half-width instead of `lo`/`hi` (a margin of error)."),
    cap: t6.number(8, "Cap width (px); 0 for none."),
    r: t6.number(4, "The dot's radius (px); 0 for none (over bars)."),
    stroke: t6.prop("Ink of the interval and dot (default: the colour scale, else $ink-2)."),
    width: t6.number(1.5, "Line width (px)."),
    format: t6.string(",.1~f", "Number format in tooltips."),
    label: t6.prop("Label (tooltip, accessible name); default `name: value (lo\u2013hi)`.")
  },
  tokens: ["ink-2", "paper"],
  expand(p, cx) {
    const horizontal = isBand3(p.yType) && !isBand3(p.xType);
    const [cat, val] = horizontal ? [p.y, p.x] : [p.x, p.y];
    const [cs, vs] = horizontal ? ["y", "x"] : ["x", "y"];
    const tbl = errorTable(cx, p);
    const c = at(cs, cat, horizontal ? p.yType : p.xType);
    const V = (v) => `scale.${vs}(${v})`;
    const P = (along, across) => horizontal ? [e6(along), e6(across)] : [e6(across), e6(along)];
    const seg = (a0, c0, a1, c1) => {
      const [x1, y1] = P(a0, c0), [x2, y2] = P(a1, c1);
      return geom6.segment({ x1, y1, x2, y2 });
    };
    const ink = p.stroke ?? colorOr2(p.color, "$ink-2");
    const stroke = { paint: ink, width: p.width, cap: "butt" };
    const f = (v) => `format(${v}, ${JSON.stringify(p.format)})`;
    const name = isBand3(horizontal ? p.yType : p.xType) ? `key.name(d.${cat})` : `scale.${cs}.label(d.${cat})`;
    const dot2 = P(V(`d.${val}`), c);
    return group6({
      key: "error-bars",
      semantics: { role: "series", label: `${val} with intervals` },
      children: [repeat6(tbl, group6({ children: [
        shape6(seg(V("d.__lo"), c, V("d.__hi"), c), { key: "interval", stroke, semantics: { role: "datum", label: p.label ?? e6(`${name} + ": " + ${f(`d.${val}`)} + " (" + ${f("d.__lo")} + "\u2013" + ${f("d.__hi")} + ")"`), value: e6(`d.${val}`) }, pickable: true }),
        p.cap > 0 ? shape6(seg(V("d.__lo"), `${c} - ${p.cap / 2}`, V("d.__lo"), `${c} + ${p.cap / 2}`), { key: "cap-lo", stroke, semantics: { role: "decoration" } }) : null,
        p.cap > 0 ? shape6(seg(V("d.__hi"), `${c} - ${p.cap / 2}`, V("d.__hi"), `${c} + ${p.cap / 2}`), { key: "cap-hi", stroke, semantics: { role: "decoration" } }) : null,
        p.r > 0 ? shape6(geom6.circle({ cx: dot2[0], cy: dot2[1], r: p.r }), { key: "dot", fill: ink, stroke: { paint: "$paper", width: 1 }, semantics: { role: "decoration" } }) : null
      ] }))]
    });
  }
});
function stackedAreaTable(cx, p) {
  const series = p.series ?? p.color;
  return cx.table("stacked-area", p.data, ...isBand3(p.xType) ? [] : [op4.sort(p.x), op4.derive("__x", e6(`d.${p.x}`))], op4.stack({ x: p.x, series, value: p.y, offset: p.offset, order: p.order, as: ["y0", "y1"] }));
}
var stackedArea = recipe6({
  id: "@datars/std/stackedArea",
  doc: 'Stacked areas: each series a layer on the ones below it, through x \u2014 totals and their parts over time. `offset: "expand"` stretches every x to 100 %; `"wiggle"` (with `order: "inside-out"`) makes a streamgraph, layers flowing around a moving centre; `"silhouette"` centres them. The plot\'s value axis fits the stack.',
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("linear", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    series: t6.field("One layer per value of this field (default: the colour field)."),
    offset: t6.oneOf(["zero", "expand", "silhouette", "wiggle"], "zero", "The baseline: zero, stretched to 100 %, centred, or a streamgraph's wiggle."),
    order: t6.oneOf(["input", "reverse", "ascending", "descending", "inside-out"], "input", "Which layers sit at the baseline: as in the data, by total, or the biggest in the middle (streamgraphs)."),
    curve: t6.string("monotone-x", "How layers' edges pass through the points: `monotone-x` (smooth, no overshoot), `linear`, `step`, \u2026"),
    opacity: t6.number(0.9, "Fill opacity."),
    labels: t6.bool(false, "Each layer named inside, where it is thickest (when a name fits)."),
    format: t6.string(",.4~g", "Number format of values in tooltips.")
  },
  tokens: ["mark", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const tbl = stackedAreaTable(cx, p);
    const X = at("x", p.x, p.xType);
    const fill = p.color ? e6(`scale.color(d.${p.color})`) : "$mark";
    const xLabel2 = isBand3(p.xType) ? `key.name(d.${p.x})` : `scale.x.label(d.${p.x})`;
    const value = p.offset === "expand" ? `format(d.y1 - d.y0, ".1%")` : `format(d.${p.y}, ${JSON.stringify(p.format)})`;
    const thick = cx.table("stacked-area-labels", tbl, op4.derive("__t", e6("d.y1 - d.y0")), op4.window("rank", "__t", "__r", { partition: [series] }), op4.filter(e6("d.__r == 1")), op4.aggregate([series], { x: ["first", p.x], y0: ["first", "y0"], y1: ["first", "y1"] }));
    const xt = isBand3(p.xType) ? "scale.x(d.x) + scale.x.bandwidth() / 2" : "scale.x(d.x)";
    const name = `key.name(d.${series})`;
    const lw = `measure(${name}, token("size.label"), 600)`;
    const T = JSON.stringify(tbl);
    const [xlo, xhi] = isBand3(p.xType) ? ["scale.x.min()", "scale.x.max()"] : [`scale.x(table.min(${T}, "__x"))`, `scale.x(table.max(${T}, "__x"))`];
    return group6({
      key: "areas",
      semantics: { role: "series", label: `${p.y} by ${series}, stacked` },
      children: [
        repeat6({ groups: tbl, by: series }, group6({ children: [
          shape6(geom6.area({ from: "@group", x: e6(X), y0: e6("scale.y(d.y0)"), y1: e6("scale.y(d.y1)"), curve: p.curve }), {
            key: "area",
            fill,
            opacity: p.opacity,
            semantics: { role: "series", label: e6(name) }
          }),
          // Hover anywhere along a layer: its value at the nearest x.
          instances2({ key: "points", from: "@group", x: e6(X), y: e6("(scale.y(d.y0) + scale.y(d.y1)) / 2"), r: 0, fill: "transparent", instanceKey: e6(`d.${p.x}`), label: e6(`${name} + " \xB7 " + ${xLabel2} + ": " + ${value}`), hit: "line" })
        ] })),
        // (Kept over the layers: one thickest at the first or last x is named just inside it.)
        p.labels ? group6({ key: "labels", children: [repeat6(thick, text5(e6(name), [e6(`clamp(${xt}, ${xlo} + ${lw} / 2 + 4, ${xhi} - ${lw} / 2 - 4)`), e6("(scale.y(d.y0) + scale.y(d.y1)) / 2")], {
          when: e6('abs(scale.y(d.y0) - scale.y(d.y1)) >= token("size.label") + 6'),
          style: { size: "$size.label", weight: 600, ink: p.color ? e6(`"on(" + scale.color(d.${p.color}) + ")"`) : "$accent-ink", align: "middle", baseline: "middle", contain: true }
        }))] }) : null
      ]
    });
  }
});
var connectedScatter = recipe6({
  id: "@datars/std/connectedScatter",
  doc: "A connected scatter plot: points at (x, y) joined in the order of a third field (usually time), an arrow at the end \u2014 how two measures moved together. Labelled at the ends (and every n-th point), labels kept off each other.",
  params: {
    data: t6.table("The rows (inside a plot: its data)."),
    x: t6.field("Field on the x axis (inside a plot: its x)."),
    y: t6.field("Field on the y axis (inside a plot: its y)."),
    color: t6.field("Field on the colour scale (inside a plot: its colour)."),
    xType: t6.string("linear", "The x scale's type (the plot passes it): `band`/`point` for categories, `linear`, `time`, \u2026"),
    yType: t6.string("linear", "The y scale's type (the plot passes it); a band y lays the chart on its side."),
    order: t6.field("The field the points are joined in (a year, a date); default: row order."),
    series: t6.field("One path per value of this field (default: the colour field)."),
    text: t6.field("Point labels (default: the order field)."),
    labels: t6.oneOf(["ends", "all", "none"], "ends", "Which points are labelled: each path's first and last, all, or none."),
    every: t6.number(0, 'Also label every n-th point (with `labels: "ends"`).'),
    curve: t6.oneOf(["linear", "catmull-rom"], "linear", "How the points are joined: straight (honest about where the data is), or smoothed through every point."),
    r: t6.number(3.5, "Dot radius (px)."),
    arrow: t6.bool(true, "An arrowhead at each path's end: which way it went."),
    stroke: t6.prop("Line and dot ink (default: the colour scale, else $mark)."),
    format: t6.string(",.3~g", "Number format in tooltips.")
  },
  tokens: ["mark", "ink-2", "paper", "size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const ord = p.order;
    const tbl = cx.table("connected", p.data, ...ord ? [op4.sort(ord)] : []);
    const X = at("x", p.x, p.xType), Y = at("y", p.y, p.yType);
    const ink = p.stroke ?? colorOr2(p.color, "$mark");
    const lbl2 = p.text ?? ord;
    const f = (v) => `format(${v}, ${JSON.stringify(p.format)})`;
    const pointLabel = e6(`${series ? `key.name(d.${series}) + " \xB7 " + ` : ""}${lbl2 ? `d.${lbl2} + ": " + ` : ""}${f(`d.${p.x}`)} + ", " + ${f(`d.${p.y}`)}`);
    const part = series ? [series] : [];
    const numbered = lbl2 && p.labels !== "none" ? cx.table(
      "connected-labels",
      tbl,
      op4.derive("__one", 1),
      op4.window("cumsum", "__one", "__i", { partition: part }),
      op4.window("last", "__i", "__n", { partition: part }),
      op4.filter(e6(p.labels === "all" ? "true" : `d.__i == 1 || d.__i == d.__n${p.every > 0 ? ` || (d.__i - 1) % ${p.every} == 0` : ""}`))
    ) : null;
    const path = group6({ children: [
      shape6(geom6.polyline({ from: "@group", x: e6(X), y: e6(Y), curve: p.curve }), {
        key: "line",
        stroke: { paint: ink, width: 1.5, join: "round", cap: "round" },
        markers: p.arrow ? { end: { type: "arrow", size: 7 } } : void 0,
        semantics: { role: "series", label: series ? e6(`key.name(d.${series})`) : `${p.y} against ${p.x}` }
      }),
      instances2({ key: "points", from: "@group", x: e6(X), y: e6(Y), r: p.r, fill: ink, stroke: { paint: "$paper", width: 1 }, instanceKey: ord ? e6(`d.${ord}`) : void 0, label: pointLabel })
    ] });
    return group6({
      key: "connected",
      semantics: { role: "series", label: `${p.y} against ${p.x}${ord ? `, by ${ord}` : ""}` },
      children: [
        series ? repeat6({ groups: tbl, by: series }, path) : repeat6({ groups: tbl, by: "__all" }, path),
        // Beside its point; a label that would land on another tries the other side, else stays out.
        numbered ? group6({ key: "labels", declutter: true, children: [repeat6(numbered, text5(e6(`d.${lbl2}`), [e6(X), e6(Y)], {
          offset: [p.r + 4, -(p.r + 3)],
          halo: ["$paper", 2],
          style: { size: "$size.label", ink: "$ink-2", align: "start", baseline: "alphabetic" }
        }))] }) : null
      ]
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { trim: 0 } }]
});
var NO_ZERO = /* @__PURE__ */ new Set(["boxplot", "violin", "ridgeline", "dumbbell", "slope", "connectedScatter"]);
function comparisonFrame(children, inherit, cx) {
  const out = {};
  for (const c of children) {
    if (c.kind !== "use") continue;
    const n = nameOf2(c.recipe);
    const raw = { ...inherit, ...c.params ?? {} };
    if (n && NO_ZERO.has(n)) out.zero ?? (out.zero = false);
    if (n === "histogram") {
      const q3 = withDefaults(histogram.def, raw);
      const tbl = histogramTable(cx, q3);
      out.x ?? (out.x = { data: tbl, fields: ["bin", "bin_end"] });
      out.y ?? (out.y = { data: tbl, fields: ["y0", "y1"] });
    } else if (n === "stackedArea") {
      const q3 = withDefaults(stackedArea.def, raw);
      out.y ?? (out.y = q3.offset === "expand" ? [0, 1] : { data: stackedAreaTable(cx, q3), fields: ["y0", "y1"] });
      if (q3.offset === "expand") out.percent = true;
    } else if (n === "errorBars") {
      const q3 = withDefaults(errorBars.def, raw);
      const horizontal = isBand3(q3.yType) && !isBand3(q3.xType);
      const domain = { data: errorTable(cx, q3), fields: ["__lo", "__hi", horizontal ? q3.x : q3.y] };
      if (horizontal) out.x ?? (out.x = domain);
      else out.y ?? (out.y = domain);
    } else if (n === "ridgeline") {
      out.x ?? (out.x = { data: ridgelineTable(cx, withDefaults(ridgeline.def, raw)).kde, field: "value" });
    } else if (n === "slope") {
      const room = slopeRoom(cx, withDefaults(slope.def, raw));
      out.start ?? (out.start = room.start);
      out.end ?? (out.end = room.end);
    }
  }
  return out;
}

// src/plot.ts
var nameOf3 = (id) => String(id ?? "").split("/").pop();
var PANE_GAP = 10;
var isLower = (c) => c.kind === "use" && (c.params?.yScale ?? (nameOf3(c.recipe) === "volume" ? "lower" : "y")) === "lower";
function labelsEnds(c, color) {
  const q3 = c.params ?? {};
  if (c.kind !== "use") return false;
  if (nameOf3(c.recipe) === "line") return !!q3.labels;
  if (nameOf3(c.recipe) === "indexed") return q3.labels !== false;
  if (nameOf3(c.recipe) === "drawdown") return q3.labels !== false && !!(q3.series ?? color);
  return false;
}
function whenSource(w) {
  if (w && typeof w === "object" && "expr" in w) return String(w.expr);
  if (typeof w === "string") return w.startsWith("=") ? w.slice(1) : JSON.stringify(w);
  return JSON.stringify(w ?? true);
}
function scaleDecl(type, data, field, axisName, p, domain, valueAxis) {
  const categorical = type === "band" || type === "point";
  return {
    type,
    domain: domain ?? { data, field },
    range: { box: "plot-area", axis: axisName },
    padding: categorical ? p.padding : void 0,
    zero: valueAxis && !categorical && type !== "log" && type !== "time" ? p.zero : void 0,
    nice: !categorical ? p.nice : void 0
  };
}
function brushExtent(signal, axis2) {
  const [lo, hi] = [`scale.${axis2}(${signal}.lo)`, `scale.${axis2}(${signal}.hi)`];
  const g = axis2 === "x" ? geom7.rect({ x: e7(`min(${lo}, ${hi})`), y: 0, w: e7(`abs(${hi} - ${lo})`), h: e7("box.h") }) : geom7.rect({ x: 0, y: e7(`min(${lo}, ${hi})`), w: e7("box.w"), h: e7(`abs(${hi} - ${lo})`) });
  return shape7(g, { key: "brush", when: e7(`${signal}.active && ${signal}.hi > ${signal}.lo`), fill: "$accent", opacity: 0.14, semantics: { role: "decoration", label: "" } });
}
var plot = recipe7({
  id: "@datars/std/plot",
  doc: "A cartesian frame: x/y (and colour) scales from data, axes and gridlines sized by the engine, a title, and marks as children.",
  params: {
    data: t7.table("The table marks read by default."),
    x: t7.field("Field on the horizontal axis."),
    y: t7.field("Field on the vertical axis (none: marks that place themselves across, like a one-dimensional `swarm`, with no y axis)."),
    color: t7.field("Field mapped to colour (categorical by default)."),
    xType: t7.oneOf(["band", "point", "linear", "log", "sqrt", "time"], "band"),
    yType: t7.oneOf(["band", "point", "linear", "log", "sqrt", "time"], "linear"),
    colorType: t7.oneOf(["categorical", "sequential", "diverging", "piecewise"], "categorical"),
    stops: t7.string(void 0, "Piecewise colour stops (colorType piecewise): '#bae6fd 7 \xB7 #0b1f3a 14.2'."),
    xDomain: t7.json("Explicit x domain."),
    yDomain: t7.json("Explicit y domain (default: the data's; a `stacked` child's totals, 0\u20131 stretched to 100 %; a `waterfall` child's running totals)."),
    zero: t7.bool(void 0, "Include zero in the value axis's domain (default: yes; prices under candles, averages, bands and indexed lines fit their range instead)."),
    nice: t7.bool(true, "Round linear domains to nice numbers."),
    padding: t7.number(0.22, "Band padding."),
    title: t7.string(void 0, "Title above the plot."),
    subtitle: t7.string(void 0),
    axes: t7.oneOf(["both", "x", "y", "none"], "both"),
    grid: t7.bool(true, "Gridlines along the value axis."),
    legend: t7.bool(false, "A colour legend below the plot."),
    xLabel: t7.string(void 0, "Title of the x axis, under its right end."),
    yLabel: t7.string(void 0, "Title of the y axis, above its top-left corner (in a row of its own, between the title and the plot)."),
    format: t7.string(void 0, "Number format for value-axis labels (a 100 % `stacked` child: '.0%')."),
    xFormat: t7.string(void 0, "Number format for the x axis's labels, whichever axis holds the values: 'd' reads decimal years on a linear axis as whole years (a whole-number format labels whole-number ticks only)."),
    yFormat: t7.string(void 0, "Number format for the y axis's labels (overrides `format` there)."),
    xTicks: t7.number(0, "Tick count hint for the x axis (0 = from its length)."),
    yTicks: t7.number(0, "Tick count hint for the y axis (0 = from its length)."),
    clip: t7.bool(false, "Clip marks to the plot area (for explicit domains narrower than the data: a zoom)."),
    labelSpace: t7.number(110, "Room at the right (px) when a line child labels its ends."),
    prefix: t7.string(void 0, "Before each value-axis number ('$')."),
    suffix: t7.string(void 0, "After it ('M', ' kr')."),
    brush: t7.string(void 0, "A signal to brush into: dragging across the plot selects a range of the category/time axis (see `brush()` and `brushed()`)."),
    right: t7.json('A second value axis on the right, scale `y2`: {y, data?, domain?, label?, format?, prefix?, suffix?}. Marks use it with `yScale: "y2"`.'),
    lower: t7.json('A pane under the plot area on the same x with its own value axis, scale `lower`: {y, data?, height? (fraction or px, 0.22), format?, label?}. Marks go there with `yScale: "lower"`; a `volume` child makes one when it isn\'t given.'),
    children: t7.children("Marks and annotations.")
  },
  tokens: ["ink", "muted", "grid", "rule", "size.title", "size.label"],
  expand(p, cx) {
    const yCategorical = p.yType === "band" || p.yType === "point";
    const children = p.children ?? [];
    const fin = yCategorical ? void 0 : financeDomain(children, { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType }, cx);
    const cmp = comparisonFrame(children, { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType, yType: p.yType }, cx);
    const pz = { ...p, zero: p.zero ?? fin?.zero ?? cmp.zero ?? true };
    const stackedChild = children.find((c) => c.kind === "use" && nameOf3(c.recipe) === "stacked");
    const waterfallChild = children.find((c) => c.kind === "use" && nameOf3(c.recipe) === "waterfall");
    const noY = !p.y && !yCategorical && p.yDomain === void 0 && !fin && !stackedChild && !waterfallChild && !cmp.y;
    const scales = {
      x: scaleDecl(p.xType, p.data, p.x, "x", pz, p.xDomain ?? cmp.x, yCategorical),
      // Prices on a log axis fit their range instead of widening to whole decades.
      y: scaleDecl(p.yType, p.data, p.y, yCategorical ? "y" : "-y", fin && p.yType === "log" ? { ...pz, nice: false } : pz, p.yDomain ?? fin?.domain ?? cmp.y ?? (noY ? [0, 1] : void 0), !yCategorical)
    };
    const valueName2 = yCategorical ? "x" : "y";
    const valueGiven = yCategorical ? p.xDomain : p.yDomain;
    const sp = stackedChild?.params;
    const expand = sp?.offset === "expand";
    if (sp && !valueGiven) {
      const series2 = sp.series ?? sp.color ?? p.color;
      const [x, y] = [sp.x ?? p.x, sp.y ?? p.y];
      const cat = yCategorical ? y : x;
      const val = yCategorical ? x : y;
      const domain = expand ? [0, 1] : { data: cx.table("stack", sp.data ?? p.data, op5.stack({ x: cat, series: series2, value: val, offset: "zero", as: ["y0", "y1"] })), field: "y1" };
      scales[valueName2] = { ...scales[valueName2], domain };
    }
    const wp = waterfallChild?.params;
    if (wp && !valueGiven) {
      const tbl = cx.table("waterfall", wp.data ?? p.data, op5.waterfall({ value: wp.y ?? p.y, total: wp.total }));
      scales[valueName2] = { ...scales[valueName2], domain: { data: tbl, fields: ["start", "end"] } };
    }
    const percent = (expand || cmp.percent) && !p.format ? ".0%" : void 0;
    const right = p.right;
    if (right) {
      scales.y2 = {
        type: "linear",
        domain: right.domain ?? { data: right.data ?? p.data, field: right.y ?? p.y },
        range: { box: "plot-area", axis: "-y" },
        zero: right.zero ?? true,
        nice: true
      };
    }
    if (p.color) {
      scales.color = p.colorType === "piecewise" ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.color } } : { type: p.colorType, domain: { data: p.data, field: p.color }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" };
    }
    const volumeChild = children.find((c) => nameOf3(c.recipe) === "volume" && isLower(c));
    const vp = volumeChild?.params;
    const lower = yCategorical ? void 0 : p.lower ?? (volumeChild ? { y: vp?.volume ?? "volume", data: vp?.data } : void 0);
    if (lower) {
      scales.lower = { type: "linear", domain: lower.domain ?? { data: lower.data ?? p.data, field: lower.y ?? p.y }, range: { box: "lower-area", axis: "-y" }, zero: true, nice: true };
    }
    const endLabels2 = children.some((c) => labelsEnds(c, p.color)) || cmp.end !== void 0;
    const clipArea = p.clip && !endLabels2;
    const inherit = { data: p.data, x: p.x, y: p.y, color: p.color, xType: p.xType, yType: p.yType, clip: p.clip && endLabels2 ? true : void 0 };
    const marks = children.map((c) => c.kind === "use" ? { ...c, params: { ...inherit, ...c.params } } : c);
    const valueAxis = yCategorical ? "x" : "y";
    const xd = p.xDomain && typeof p.xDomain === "object" && !Array.isArray(p.xDomain) ? p.xDomain : void 0;
    const names = { data: xd?.data ?? p.data, field: xd?.field ?? p.x };
    const xNames = (p.xType === "band" || p.xType === "point") && names.data && names.data !== "@group" ? names : {};
    const showX = p.axes === "both" || p.axes === "x";
    const showY = (p.axes === "both" || p.axes === "y") && !noY;
    const yAxis = (opts) => axis({ scale: "y", orient: "left", type: p.yType, format: p.yFormat ?? (valueAxis === "y" ? p.format ?? percent : void 0), ticks: p.yTicks || void 0, prefix: valueAxis === "y" ? p.prefix : void 0, suffix: valueAxis === "y" ? p.suffix : void 0 }, opts);
    const leftAxes = !showY ? null : !lower ? yAxis({ size: { w: "auto" } }) : group7({ key: "axes-left", size: { w: "auto" }, children: [
      yAxis(),
      group7({ key: "lower-axis", transform: { translate: [0, e7(`scale.y.max() + ${PANE_GAP + 12}`)] }, children: [axis({ scale: "lower", orient: "left", ticks: 2, format: lower.format ?? ".2~s" })] })
    ] });
    const lowerPane = lower ? group7({
      id: "lower-area",
      key: "lower",
      size: { h: (lower.height ?? 0.22) <= 1 ? `${Math.round((lower.height ?? 0.22) * 100)}%` : lower.height },
      clip: clipArea ? "box" : void 0,
      children: [
        grid({ scale: "lower", orient: "horizontal", ticks: 2 }),
        lower.label ? text6(lower.label, [4, 2], { key: "pane-label", style: { size: "$size.small", ink: "$muted", baseline: "top" }, halo: ["$paper", 2] }) : null,
        ...marks.filter(isLower)
      ]
    }) : null;
    const keys = financeKey(children);
    const keyRow = keys.length ? group7({ key: "indicators", size: { h: "auto" }, layout: { type: "flow", gap: 14 }, when: keys.every((k) => k.when !== void 0) ? e7(keys.map((k) => `(${whenSource(k.when)})`).join(" || ")) : void 0, children: keys }) : null;
    const xAxis = { scale: "x", orient: "bottom", type: p.xType, format: p.xFormat ?? (valueAxis === "x" ? p.format ?? percent : void 0), ticks: p.xTicks || void 0, prefix: valueAxis === "x" ? p.prefix : void 0, suffix: valueAxis === "x" ? p.suffix : void 0 };
    const gridCount = valueAxis === "x" ? tickCount(xAxis) : p.yTicks || void 0;
    const labelled = endLabels2 ? (p.children ?? []).find((c) => c.kind === "use" && c.recipe === "@datars/std/line" && c.params.labels) : void 0;
    const lp = labelled?.params ?? {};
    const series = lp.series ?? lp.color ?? p.color;
    const seriesNames = labelled && series ? cx.table("series-names", lp.data ?? p.data, op5.aggregate([series], { n: ["count"] }), op5.derive("w", e7(`measure(key.name(d.${series}), token("size.label"))`))) : void 0;
    const labelRoom = cmp.end ?? (seriesNames ? e7(`min(${p.labelSpace}, table.max(${JSON.stringify(seriesNames)}, "w") + 12)`) : p.labelSpace);
    const yTitle = showY && p.yLabel ? p.yLabel : void 0;
    const axisTitles = yTitle || right?.label ? group7({ key: "axis-titles", size: { h: "auto" }, children: [
      yTitle ? text6(yTitle, [0, 0], { key: "y", style: { size: "$size.label", ink: "$muted", baseline: "top" } }) : null,
      right?.label ? text6(right.label, [e7("box.w"), 0], { key: "y2", style: { size: "$size.label", ink: "$muted", baseline: "top", align: "end" } }) : null
    ] }) : null;
    return group7({
      key: "plot",
      scales,
      layout: { type: "rows", gap: 6 },
      semantics: { role: "group", label: p.title ?? "" },
      children: [
        p.title ? text6(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e7("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        p.subtitle ? text6(p.subtitle, [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e7("box.w") } }) : null,
        keyRow,
        axisTitles,
        group7({
          key: "body",
          layout: { type: "columns", gap: 6 },
          children: [
            leftAxes,
            // Room at the left for marks labelled at their starts (a slope chart's first values).
            cmp.start !== void 0 ? group7({ key: "start-labels", size: { w: cmp.start } }) : null,
            group7({
              key: "center",
              layout: { type: "rows", gap: 6 },
              children: [
                group7({
                  id: "plot-area",
                  key: "area",
                  clip: clipArea ? "box" : void 0,
                  on: p.brush ? { brush: brush(p.brush, valueAxis === "y" ? "x" : "y") } : void 0,
                  children: [p.grid && !noY ? grid({ scale: valueAxis, orient: valueAxis === "y" ? "horizontal" : "vertical", count: gridCount }) : null, p.brush ? brushExtent(p.brush, valueAxis === "y" ? "x" : "y") : null, ...lower ? marks.filter((c) => !isLower(c)) : marks]
                }),
                lower ? group7({ key: "pane-gap", size: { h: PANE_GAP } }) : null,
                lowerPane,
                showX ? axis({ ...xAxis, label: p.xLabel, ...xNames }, { size: { h: "auto" } }) : null
              ]
            }),
            right ? axis({ scale: "y2", orient: "right", format: right.format, prefix: right.prefix, suffix: right.suffix }, { key: "axis-y2", size: { w: "auto" } }) : null,
            // Room at the right for lines labelled at their ends (they draw just past the plot area).
            endLabels2 ? group7({ key: "end-labels", size: { w: labelRoom } }) : null
          ]
        }),
        p.legend && p.color ? legend({ scale: "color" }, { size: { h: "auto" } }) : null
      ]
    });
    void cx;
  }
});

// src/pie.ts
import { e as e8, group as group8, op as op6, recipe as recipe8, repeat as repeat7, shape as shape8, geom as geom8, t as t8, text as text7 } from "@datars/sdk";
var pie = recipe8({
  id: "@datars/std/pie",
  doc: "Parts of a whole as slices (set `inner` > 0 for a donut). Colours from the categorical palette by category.",
  params: {
    data: t8.table(),
    value: t8.field(),
    category: t8.field(),
    inner: t8.number(0, "Inner radius as a fraction of the outer (0 = pie, 0.6 = donut)."),
    pad: t8.number(6e-3, "Pad angle between slices (radians)."),
    sort: t8.oneOf(["none", "asc", "desc"], "none"),
    labels: t8.bool(true),
    format: t8.string(".1~f"),
    total: t8.bool(false, "Show the total in the middle (donuts).")
  },
  tokens: ["categorical", "paper", "ink", "muted", "size.label", "size.title"],
  expand(p, cx) {
    const slices = cx.table(
      "slices",
      p.data,
      op6.pie({ value: p.value, sort: p.sort, pad: p.pad, as: ["a0", "a1"] }),
      op6.derive("lw", e8(`measure(key.name(d.${p.category}), token("size.label"))`))
    );
    const S = JSON.stringify(slices);
    const base = "(min(box.w, box.h) / 2 - 36)";
    const fit = `(box.w / 2 - table.max(${S}, "lw") - 26)`;
    const colW = `(table.max(${S}, "lw") + 26)`;
    const cols = `max(1, floor(box.w / ${colW}))`;
    const legendH = `(ceil(table.count(${S}) / ${cols}) * 16 + 10)`;
    const narrow = p.labels ? `(${fit} < min(box.w, box.h) * 0.2)` : "false";
    const R = p.labels ? `(${narrow} ? min(box.w, box.h - ${legendH}) / 2 - 4 : min(${base}, ${fit}))` : base;
    const CY = p.labels ? `(${narrow} ? (box.h - ${legendH}) / 2 : box.h / 2)` : "box.h / 2";
    const mid = "(d.a0 + d.a1) / 2";
    return group8({
      key: "marks",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      semantics: { role: "series", label: `${p.value} by ${p.category}` },
      children: [
        repeat7(slices, shape8(geom8.arc({ cx: e8("box.w / 2"), cy: e8(CY), r0: e8(`(${R}) * ${p.inner}`), r1: e8(R), a0: e8("d.a0"), a1: e8("d.a1") }), {
          fill: e8(`scale.color(d.${p.category})`),
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: e8(`\`\${key.name(d.${p.category})}: \${format(d.${p.value}, ${JSON.stringify(p.format)})}\``), value: e8(`d.${p.value}`) },
          pickable: true
        })),
        p.labels ? group8({ key: "labels", declutter: true, children: [repeat7(slices, text7(e8(`key.name(d.${p.category})`), [e8(`box.w / 2 + (${R} + 14) * sin(${mid})`), e8(`${CY} - (${R} + 14) * cos(${mid})`)], {
          when: e8(`!${narrow} && d.a1 - d.a0 > 0.12`),
          style: { size: "$size.label", ink: "$ink-2", align: e8(`sin(${mid}) >= 0 ? "start" : "end"`), baseline: "middle" }
        }))] }) : null,
        p.labels ? group8({ key: "legend", when: e8(narrow), semantics: { role: "legend", label: "Legend" }, children: [repeat7({ legend: "color" }, group8({
          transform: { translate: [e8(`(d.index % ${cols}) * ${colW} + (box.w - min(table.count(${S}), ${cols}) * ${colW}) / 2`), e8(`box.h - ${legendH} + 10 + floor(d.index / ${cols}) * 16`)] },
          semantics: { role: "legend-item", label: e8("key.name(d.label)") },
          children: [
            shape8(geom8.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e8("d.ink") }),
            text7(e8("key.name(d.label)"), [14, 9], { style: { size: "$size.label", ink: "$ink-2" } })
          ]
        }))] }) : null,
        p.total && p.inner > 0 ? text7("", [e8("box.w / 2"), e8(CY)], { key: "total", number: { value: e8(`sum(${JSON.stringify(p.data)}, ${JSON.stringify(p.value)})`), format: p.format }, style: { font: "font.title", size: "$size.title", align: "middle", baseline: "middle" } }) : null
      ]
    });
  }
});
var donut = (params, opts) => pie({ inner: 0.58, ...params }, opts);

// src/charts.ts
import { e as e9, group as group9, instances as instances3, op as op7, recipe as recipe9, repeat as repeat8, shape as shape9, geom as geom9, t as t9, text as text8 } from "@datars/sdk";
function affixed2(p, value, at2, opts) {
  if (!p.prefix && !p.suffix) return text8("", at2, { ...opts, number: { value: e9(value), format: p.format } });
  const v = `(${value})`;
  return text8(e9(`(${v} < 0 ? "\u2212" : "") + ${JSON.stringify(p.prefix ?? "")} + format(abs(${v}), ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}`), at2, opts);
}
var lbl = (cat, val, fmt) => e9(`\`\${key.name(d.${cat})}: \${format(d.${val}, ${JSON.stringify(fmt)})}\``);
var cell = recipe9({
  id: "@datars/std/cell",
  doc: "Heatmap cells: a rectangle per (x, y) on two band scales, coloured by the colour scale. Keys (x, y) match grouped bars, so the two morph.",
  params: { data: t9.table(), x: t9.field(), y: t9.field(), color: t9.field("The value field (on a sequential/diverging colour scale)."), gap: t9.number(1), format: t9.string(".1~f"), label: t9.prop("Cell label (tooltip, accessible name); default `y, x: value`.") },
  expand(p) {
    return group9({
      key: "marks",
      children: [repeat8(p.data, shape9(geom9.rect({ x: e9(`scale.x(d.${p.x}) + ${p.gap / 2}`), y: e9(`scale.y(d.${p.y}) + ${p.gap / 2}`), w: e9(`scale.x.bandwidth() - ${p.gap}`), h: e9(`scale.y.bandwidth() - ${p.gap}`) }), {
        key: [e9(`d.${p.y}`), e9(`d.${p.x}`)],
        fill: e9(`scale.color(d.${p.color})`),
        semantics: { role: "datum", label: p.label ?? e9(`\`\${d.${p.y}}, \${d.${p.x}}: \${format(d.${p.color}, ${JSON.stringify(p.format)})}\``) },
        pickable: true
      }))]
    });
  }
});
var stripes = recipe9({
  id: "@datars/std/stripes",
  doc: "Colour stripes: one full-height band per ordered value, on a diverging scale around `mid`.",
  params: { data: t9.table(), x: t9.field(), value: t9.field(), mid: t9.number(0), label: t9.prop("Stripe label (tooltip, accessible name); default `x: value`.") },
  expand(p) {
    return group9({
      key: "marks",
      scales: {
        sx: { type: "band", domain: { data: p.data, field: p.x }, range: "width", padding: 0 },
        c: { type: "diverging", domain: { data: p.data, field: p.value }, range: "$diverging", mid: p.mid }
      },
      children: [repeat8(p.data, shape9(geom9.rect({ x: e9(`scale.sx(d.${p.x})`), y: 0, w: e9("scale.sx.bandwidth() + 0.5"), h: e9("box.h") }), {
        fill: e9(`scale.c(d.${p.value})`),
        semantics: { role: "datum", label: p.label ?? e9(`\`\${d.${p.x}}: \${d.${p.value}}\``) },
        pickable: true
      }))]
    });
  }
});
function affixedLabel(v, p) {
  const f = JSON.stringify(p.format ?? ",.1~f");
  return `\${${v} < 0 ? "\u2212" : ""}${p.prefix ?? ""}\${format(abs(${v}), ${f})}${p.suffix ?? ""}`;
}
var stacked = recipe9({
  id: "@datars/std/stacked",
  doc: "Stacked bars: a segment per series in each category (offset `expand` for 100 %); horizontal when y is the band axis. Keys (series, x), shared with grouped bars and heatmap cells.",
  params: {
    data: t9.table(),
    x: t9.field(),
    y: t9.field(),
    color: t9.field(),
    series: t9.field(),
    offset: t9.oneOf(["zero", "expand"], "zero"),
    xType: t9.string("band"),
    yType: t9.string("linear"),
    segmentKey: t9.prop("Segment key (default the composite `(series, category)`, so segments merge into their series' bar); e.g. `d.party` so one 100 % bar's segments pair with that party's bar or slice."),
    labels: t9.bool(false, "Value labels inside the segments (they count when values change), where they fit."),
    format: t9.string(",.1~f", "Number format of each segment's label (its value label, hover, screen readers)."),
    prefix: t9.string(void 0, "Before the number ('$')."),
    suffix: t9.string(void 0, "After it ('M', ' kr').")
  },
  tokens: ["size.label"],
  expand(p, cx) {
    const series = p.series ?? p.color;
    const horizontal = (p.yType === "band" || p.yType === "point") && p.xType !== "band" && p.xType !== "point";
    const cat = horizontal ? p.y : p.x;
    const val = horizontal ? p.x : p.y;
    const tbl = cx.table("stack", p.data, op7.stack({ x: cat, series, value: val, offset: p.offset, as: ["y0", "y1"] }));
    const g = horizontal ? geom9.rect({ x: e9("scale.x(d.y0)"), y: e9(`scale.y(d.${cat})`), w: e9("scale.x(d.y1) - scale.x(d.y0)"), h: e9("scale.y.bandwidth()") }) : geom9.rect({ x: e9(`scale.x(d.${cat})`), y: e9("scale.y(d.y1)"), w: e9("scale.x.bandwidth()"), h: e9("scale.y(d.y0) - scale.y(d.y1)") });
    const lw = `measure(${JSON.stringify(p.prefix ?? "")} + format(d.${val}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}, token("size.label"))`;
    const fits = horizontal ? `abs(scale.x(d.y1) - scale.x(d.y0)) >= ${lw} + 8 && scale.y.bandwidth() >= token("size.label") + 4` : `scale.x.bandwidth() >= ${lw} + 8 && abs(scale.y(d.y0) - scale.y(d.y1)) >= token("size.label") + 4`;
    const at2 = horizontal ? [e9("(scale.x(d.y0) + scale.x(d.y1)) / 2"), e9(`scale.y(d.${cat}) + scale.y.bandwidth() / 2`)] : [e9(`scale.x(d.${cat}) + scale.x.bandwidth() / 2`), e9("(scale.y(d.y0) + scale.y(d.y1)) / 2")];
    const label = affixed2(p, `d.${val}`, at2, { when: e9(fits), style: { size: "$size.label", ink: e9(`"on(" + scale.color(d.${series}) + ")"`), align: "middle", baseline: "middle" } });
    return group9({
      key: "marks",
      children: [
        repeat8(tbl, shape9(g, {
          key: p.segmentKey ?? [e9(`d.${series}`), e9(`d.${cat}`)],
          fill: e9(`scale.color(d.${series})`),
          semantics: { role: "datum", label: e9(`\`\${key.name(d.${series})}, \${d.${cat}}: ${affixedLabel(`d.${val}`, p)}\``), value: e9(`d.${val}`) },
          pickable: true
        })),
        p.labels ? group9({ key: "labels", children: [repeat8(tbl, label)] }) : null
      ]
    });
  }
});
var grouped = recipe9({
  id: "@datars/std/grouped",
  doc: "Grouped (dodged) bars: a group per x, a bar per series side by side. Keys (series, x), shared with stacked segments and heatmap cells, so the three morph.",
  params: {
    data: t9.table(),
    x: t9.field(),
    y: t9.field(),
    color: t9.field(),
    series: t9.field("The series within each group (default: the colour field)."),
    xType: t9.string("band"),
    yType: t9.string("linear"),
    gap: t9.number(0.08, "Padding between bars of a group (band fraction)."),
    labels: t9.bool(false, "Value labels on the bars (they count when values change), where they fit a bar's width."),
    format: t9.string(",.1~f", "Number format of each bar's label."),
    prefix: t9.string(void 0, "Before each bar's number in its label ('$')."),
    suffix: t9.string(void 0, "After it ('M', ' kr').")
  },
  tokens: ["ink-2", "size.label", "radius.bar"],
  expand(p) {
    const series = p.series ?? p.color;
    const x = `scale.x(d.${p.x}) + scale.dodge(d.${series})`, v = `d.${p.y}`;
    const lw = `measure(${JSON.stringify(p.prefix ?? "")} + format(${v}, ${JSON.stringify(p.format)}) + ${JSON.stringify(p.suffix ?? "")}, token("size.label"))`;
    const end = `min(scale.y(0), scale.y(${v}))`;
    const inside = `(${end} - 4 - token("size.label") < -2 && abs(scale.y(${v}) - scale.y(0)) >= token("size.label") + 8)`;
    const label = affixed2(p, v, [e9(`${x} + scale.dodge.bandwidth() / 2`), e9(`${inside} ? ${end} + 4 : ${end} - 4`)], {
      when: e9(`${lw} <= scale.dodge.step() - 2`),
      style: { size: "$size.label", ink: e9(`${inside} ? "on(" + scale.color(d.${series}) + ")" : "$ink-2"`), align: "middle", baseline: e9(`${inside} ? "top" : "alphabetic"`) }
    });
    return group9({
      key: "marks",
      // An inner band scale over the series, spanning one outer band.
      scales: { dodge: { type: "band", domain: { data: p.data, field: series }, range: [0, "=scale.x.bandwidth()"], padding: p.gap } },
      children: [
        repeat8(p.data, shape9(geom9.rect({ x: e9(x), y: e9(`min(scale.y(0), scale.y(${v}))`), w: e9("scale.dodge.bandwidth()"), h: e9(`abs(scale.y(0) - scale.y(${v}))`), r: "$radius.bar" }), {
          key: [e9(`d.${series}`), e9(`d.${p.x}`)],
          fill: e9(`scale.color(d.${series})`),
          semantics: { role: "datum", label: e9(`\`\${key.name(d.${series})}, \${d.${p.x}}: ${affixedLabel(v, p)}\``), value: e9(v) },
          pickable: true
        })),
        p.labels ? group9({ key: "labels", children: [repeat8(p.data, label)] }) : null
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "bottom" } }]
});
var treemap = recipe9({
  id: "@datars/std/treemap",
  doc: "A squarified treemap: area \u221D value, one rectangle per category, labels where they fit. Coloured by category, or by a value on piecewise stops.",
  params: {
    data: t9.table(),
    value: t9.field(),
    category: t9.field(),
    color: t9.field("Colour by this field (default: the category)."),
    colorType: t9.oneOf(["categorical", "piecewise"], "categorical"),
    stops: t9.string(void 0, "Piecewise colour stops: '#bae6fd 7 \xB7 #0b1f3a 14.2'."),
    labels: t9.bool(true),
    format: t9.string(".1~f")
  },
  expand(p, cx) {
    const tbl = cx.table("treemap", p.data, op7.treemap({ value: p.value, width: e9("box.w"), height: e9("box.h"), as: ["x0", "y0", "x1", "y1"] }));
    const by = p.color ?? p.category;
    const color = p.colorType === "piecewise" ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: by } } : { type: "categorical", domain: { data: p.data, field: by }, range: "$categorical" };
    return group9({
      key: "marks",
      scales: { color },
      children: [
        repeat8(tbl, shape9(geom9.rect({ x: e9("d.x0"), y: e9("d.y0"), w: e9("d.x1 - d.x0"), h: e9("d.y1 - d.y0") }), { fill: e9(`scale.color(d.${by})`), stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "datum", label: lbl(p.category, p.value, p.format) }, pickable: true })),
        // Each label in the ink that reads on its rectangle's fill (dark on a light one).
        p.labels ? group9({ key: "labels", children: [repeat8(tbl, text8(e9(`key.name(d.${p.category})`), [e9("d.x0 + 6"), e9("d.y0 + 16")], { when: e9("d.x1 - d.x0 > 44 && d.y1 - d.y0 > 22"), style: { size: "$size.label", weight: 600, ink: e9(`"on(" + scale.color(d.${by}) + ")"`), maxWidth: e9("d.x1 - d.x0 - 10") } }))] }) : null
      ]
    });
  }
});
var waffle = recipe9({
  id: "@datars/std/waffle",
  doc: "One square per unit (e.g. per percentage point). Units are keyed (category, Unit i), so bars split into squares and back.",
  params: { data: t9.table(), value: t9.field(), category: t9.field(), columns: t9.number(10), rows: t9.number(10), gap: t9.number(2) },
  expand(p, cx) {
    const u = cx.table("waffle", p.data, op7.units({ value: p.value }), op7.waffle({ columns: p.columns, rows: p.rows, width: e9("min(box.w, box.h)"), height: e9("min(box.w, box.h)"), gap: p.gap }));
    return group9({
      key: "waffle",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      children: [instances3({ key: "marks", from: u, proto: "rect", x: e9("d.x + (box.w - min(box.w, box.h)) / 2"), y: e9("d.y + (box.h - min(box.w, box.h)) / 2"), w: e9("d.w"), h: e9("d.h"), fill: e9(`scale.color(d.${p.category})`), label: e9(`key.name(d.${p.category})`) })]
    });
  }
});
var hemicycle = recipe9({
  id: "@datars/std/hemicycle",
  doc: "A parliament: one dot per seat on concentric arcs, grouped by party left to right. Seats are keyed (party, Unit i).",
  params: {
    data: t9.table(),
    value: t9.field("Seats per party."),
    category: t9.field(),
    rows: t9.number(0, "Rows of seats (0 = automatic). More rows than automatic move inward to keep their arcs; past about \u221A(seats / 2) they spread thin."),
    total: t9.bool(true),
    align: t9.oneOf(["middle", "bottom"], "middle", "In a box taller than it needs (a phone): centred up and down, or at the bottom (with a line of text right under it).")
  },
  expand(p, cx) {
    const cy = p.align === "bottom" ? "box.h - 10" : "min(box.h - 10, (box.h + min(box.w / 2 - 8, box.h - 18)) / 2 + 4)";
    const seats = cx.table("seats", p.data, op7.units({ value: p.value }), op7.parliament({ cx: e9("box.w / 2"), cy: e9(cy), r0: e9("min(box.w / 2, box.h) * 0.36"), r1: e9("min(box.w / 2 - 8, box.h - 18)"), rows: p.rows || void 0 }));
    return group9({
      key: "hemicycle",
      scales: { color: { type: "categorical", domain: { data: p.data, field: p.category }, range: "$categorical" } },
      children: [
        // Few seats are big: lift them by what their radius needs beyond the 10 px under the baseline.
        instances3({ key: "seats", from: seats, x: e9("d.x"), y: e9("d.y - max(0, d.r - 10)"), r: e9("d.r"), fill: e9(`scale.color(d.${p.category})`), label: e9(`key.name(d.${p.category})`) }),
        p.total ? text8("", [e9("box.w / 2"), e9(`${cy} - 4`)], { key: "total", number: { value: e9(`sum(${JSON.stringify(p.data)}, ${JSON.stringify(p.value)})`), format: ",.0f" }, style: { font: "font.title", size: "$size.title", align: "middle" } }) : null
      ]
    });
  }
});
var swarm = recipe9({
  id: "@datars/std/swarm",
  doc: "A beeswarm: every row a dot along one value axis, packed so none overlap (windowed neighbour search; 20k dots in about a second). In a plot it needs only `x`: without a `y`, the plot draws no y axis.",
  params: { data: t9.table(), x: t9.field(), color: t9.field(), r: t9.number(3.5), xType: t9.string("linear") },
  expand(p, cx) {
    const tbl = cx.table("swarm", p.data, op7.beeswarm({ position: e9(`scale.x(d.${p.x})`), radius: p.r + 0.5, as: "offset" }));
    return instances3({ key: "swarm", from: tbl, x: e9(`scale.x(d.${p.x})`), y: e9("box.h / 2 + d.offset"), r: p.r, fill: p.color ? e9(`scale.color(d.${p.color})`) : "$mark", label: e9(`\`\${d.${p.x}}\``) });
  }
});
var sankey = recipe9({
  id: "@datars/std/sankey",
  doc: "Flows between nodes: nodes in columns by depth, ribbons as wide as their flow. Links keep their row key (bars of the same rows morph into ribbons).",
  params: {
    data: t9.table("One row per link."),
    source: t9.field(),
    target: t9.field(),
    value: t9.field(),
    nodeWidth: t9.number(14),
    nodePadding: t9.number(12),
    label: t9.prop("Link label (tooltip, accessible name); default `source \u2192 target: value`."),
    selected: t9.string(void 0, "A keyset signal of node names: links touching them stay strong, the others recede.")
  },
  expand(p, cx) {
    const common = { links: p.data, source: p.source, target: p.target, value: p.value, width: e9("box.w"), height: e9("box.h"), nodeWidth: p.nodeWidth, nodePadding: p.nodePadding };
    const nodes = cx.table("nodes", p.data, op7.sankeyNodes(common));
    const links = cx.table("links", p.data, op7.sankeyLinks(common));
    return group9({
      key: "sankey",
      scales: { color: { type: "categorical", domain: { data: nodes, field: "name" }, range: "$categorical" } },
      children: [
        group9({ key: "links", children: [repeat8(links, shape9(geom9.path(e9("d.path")), { fill: e9(`scale.color(d.${p.source})`), opacity: p.selected ? e9(`${p.selected}.isEmpty() || ${p.selected}.has(d.${p.source}) || ${p.selected}.has(d.${p.target}) ? 0.42 : 0.12`) : 0.42, semantics: { role: "datum", label: p.label ?? e9(`\`\${d.${p.source}} \u2192 \${d.${p.target}}: \${d.${p.value}}\``) }, pickable: true }))] }),
        group9({ key: "nodes", children: [repeat8(nodes, group9({ children: [
          shape9(geom9.rect({ x: e9("d.x0"), y: e9("d.y0"), w: e9("d.x1 - d.x0"), h: e9("d.y1 - d.y0") }), { key: "node", fill: e9("scale.color(d.name)"), semantics: { role: "datum", label: e9("`${d.name}: ${d.value}`") } }),
          text8(e9("d.name"), [e9("d.x0 < box.w / 2 ? d.x1 + 6 : d.x0 - 6"), e9("(d.y0 + d.y1) / 2")], { key: "label", style: { size: "$size.label", align: e9('d.x0 < box.w / 2 ? "start" : "end"'), baseline: "middle" } })
        ] }))] })
      ]
    });
  }
});
var waterfall = recipe9({
  id: "@datars/std/waterfall",
  doc: "A waterfall/bridge: each change floats from the running total; rows marked as totals (a boolean field, or keys starting with '=') are bars from zero.",
  params: { data: t9.table(), x: t9.field(), y: t9.field(), total: t9.field(), xType: t9.string("band"), yType: t9.string("linear"), format: t9.string(",.0f"), prefix: t9.string(void 0, "Before each label's number ('$')."), suffix: t9.string(void 0, "After it ('M', ' kr').") },
  expand(p, cx) {
    const tbl = cx.table("waterfall", p.data, op7.waterfall({ value: p.y, total: p.total }));
    const fill = `(d.is_total || d.start == 0 ? "$ink-2" : d.${p.y} >= 0 ? "$positive" : "$negative")`;
    const top = "min(scale.y(d.start), scale.y(d.end))";
    const inside = `(${top} - 4 - token("size.label") < -2 && abs(scale.y(d.start) - scale.y(d.end)) >= token("size.label") + 8)`;
    return group9({
      key: "waterfall",
      children: [repeat8(tbl, group9({ children: [
        shape9(geom9.rect({ x: e9(`scale.x(d.${p.x})`), y: e9("min(scale.y(d.start), scale.y(d.end))"), w: e9("scale.x.bandwidth()"), h: e9("abs(scale.y(d.start) - scale.y(d.end))") }), {
          key: "bar",
          fill: e9(fill),
          semantics: { role: "datum", label: lbl(p.x, p.y, p.format) },
          pickable: true
        }),
        affixed2(p, `d.is_total ? d.end : d.${p.y}`, [e9(`scale.x(d.${p.x}) + scale.x.bandwidth() / 2`), e9(`${inside} ? ${top} + 4 : ${top} - 4`)], { key: "label", style: { size: "$size.label", ink: e9(`${inside} ? "on(" + ${fill} + ")" : "$ink-2"`), align: "middle", baseline: e9(`${inside} ? "top" : "alphabetic"`) } })
      ] }))]
    });
  }
});
var funnel = recipe9({
  id: "@datars/std/funnel",
  doc: "A funnel: stage names in a measured column, centred bars in stage order joined by tapering connectors, each labelled with the conversion from the stage before.",
  params: {
    data: t9.table(),
    stage: t9.field(),
    value: t9.field(),
    format: t9.string(",.0f"),
    title: t9.string(void 0, "A title above the funnel."),
    fill: t9.prop("The bars' colour (default $mark); labels inside take whichever of ink and paper reads on it."),
    conversion: t9.bool(true, "Connectors between stages, labelled with the share that carries on (\u2193 44 %).")
  },
  tokens: ["mark", "ink", "ink-2", "muted", "paper", "size.label", "font.title", "size.title"],
  expand(p, cx) {
    const tbl = cx.table("funnel", p.data, op7.window("lead", p.value, "next"));
    const v = `d.${p.value}`;
    const fill = p.fill ?? "$mark";
    const fillInk = typeof fill === "string" ? JSON.stringify(`on(${fill})`) : `"$accent-ink"`;
    const value = `format(${v}, ${JSON.stringify(p.format)})`;
    const half = `scale.fw(${v}) / 2`;
    const fits = `measure(${value}, token("size.label"), 600) + 14 <= scale.fw(${v})`;
    const top = `scale.fy(d.${p.stage})`, bottom = `${top} + scale.fy.bandwidth()`, gapMid = `${bottom} + (scale.fy.step() - scale.fy.bandwidth()) / 2`;
    const nextHalf = `scale.fw(d.next) / 2`;
    const connector2 = `\`M \${box.w / 2 - ${half}} \${${bottom}} L \${box.w / 2 + ${half}} \${${bottom}} L \${box.w / 2 + ${nextHalf}} \${${bottom} + scale.fy.step() - scale.fy.bandwidth()} L \${box.w / 2 - ${nextHalf}} \${${bottom} + scale.fy.step() - scale.fy.bandwidth()} Z\``;
    return group9({
      key: "funnel",
      scales: {
        fy: { type: "band", domain: { data: p.data, field: p.stage }, range: { box: "funnel-area", axis: "y" }, padding: p.conversion ? 0.42 : 0.18 },
        fw: { type: "linear", domain: { data: p.data, field: p.value }, range: { box: "funnel-area", axis: "x" }, zero: true }
      },
      layout: { type: "rows", gap: 10 },
      semantics: { role: "group", label: p.title ?? "Funnel" },
      children: [
        p.title ? text8(p.title, [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e9("box.w") }, semantics: { role: "title", label: p.title } }) : null,
        group9({
          key: "body",
          layout: { type: "columns", gap: 14 },
          children: [
            // Stage names, as wide as the widest (measured by layout), so none spills or overlaps.
            group9({ key: "stages", size: { w: "auto" }, children: [repeat8(tbl, text8(e9(`key.name(d.${p.stage})`), [0, e9(`${top} + scale.fy.bandwidth() / 2`)], {
              key: "stage",
              style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle" },
              semantics: { role: "decoration" }
            }))] }),
            group9({ id: "funnel-area", key: "area", children: [
              p.conversion ? repeat8(tbl, shape9(geom9.path(e9(connector2)), { key: "connector", when: e9("d.next != null"), fill, opacity: 0.2, semantics: { role: "decoration" } })) : null,
              repeat8(tbl, group9({ children: [
                shape9(geom9.rect({ x: e9(`box.w / 2 - ${half}`), y: e9(top), w: e9(`scale.fw(${v})`), h: e9("scale.fy.bandwidth()"), r: 2 }), { key: "bar", fill, semantics: { role: "datum", label: lbl(p.stage, p.value, p.format) }, pickable: true }),
                text8(e9(value), [e9(`${fits} ? box.w / 2 : box.w / 2 + ${half} + 6`), e9(`${top} + scale.fy.bandwidth() / 2`)], {
                  key: "value",
                  style: { size: "$size.label", weight: 600, ink: e9(`${fits} ? ${fillInk} : "$ink"`), align: e9(`${fits} ? "middle" : "start"`), baseline: "middle" }
                }),
                p.conversion ? text8(e9(`\`\u2193 \${format(d.next / ${v}, ".0%")}\``), [e9(`box.w / 2 + max(${half}, ${nextHalf}) + 6`), e9(gapMid)], {
                  key: "conversion",
                  when: e9(`d.next != null && ${v} > 0`),
                  style: { size: "$size.small", ink: "$muted", baseline: "middle" }
                }) : null
              ] }))
            ] }),
            // Room right of the widest bar for its conversion label.
            p.conversion ? group9({ key: "gutter", size: { w: 44 } }) : null
          ]
        })
      ]
    });
  }
});
var calendar = recipe9({
  id: "@datars/std/calendar",
  doc: "A calendar heatmap: a week-column \xD7 weekday-row grid per year, coloured by value. Several years stack, a row each, labelled at the left.",
  params: { data: t9.table(), date: t9.field("The date field."), value: t9.field(), cell: t9.number(0, "Cell size in px (0 = fit the box: 53 weeks across, every year's row down)."), label: t9.prop("Day label (tooltip, accessible name); default `date: value`.") },
  tokens: ["muted", "size.label", "sequential"],
  expand(p, cx) {
    const years = cx.table("years", p.data, op7.calendar({ date: p.date, cell: 1 }), op7.aggregate(["panel"], { first: ["min", p.date] }));
    const rows = `(table.max(${JSON.stringify(years)}, "panel") + 1)`;
    const gutter = 'measure("0000", token("size.label")) + 8';
    const cell2 = p.cell ? String(p.cell) : `max(1, min((box.w - ${gutter}) / 53, box.h > 0 ? box.h / (8 * ${rows} - 1) : box.w))`;
    const tbl = cx.table("calendar", p.data, op7.calendar({ date: p.date, cell: e9(cell2) }));
    return group9({
      key: "calendar",
      scales: { c: { type: "sequential", domain: { data: p.data, field: p.value }, range: "$sequential" } },
      children: [
        instances3({ key: "days", from: tbl, proto: "rect", x: e9(`d.cx + ${gutter}`), y: e9("d.cy + d.panel * 8 * d.cell"), w: e9("d.cell - 1"), h: e9("d.cell - 1"), fill: e9(`scale.c(d.${p.value})`), label: p.label ?? e9(`\`\${formatDate(d.${p.date}, "%-d %b %Y")}: \${d.${p.value}}\``) }),
        group9({ key: "years", children: [repeat8(years, text8(e9('formatDate(d.first, "%Y")'), [0, e9(`d.panel * 8 * (${cell2})`)], { style: { size: "$size.label", ink: "$muted", baseline: "top" } }))] })
      ]
    });
  }
});
var facet = recipe9({
  id: "@datars/std/facet",
  doc: "Small multiples: the given chart once per value of `by`, in a grid; inside, the chart's `data` is that group's rows (\"@group\"), on scales shared across panels unless `shared: false`.",
  params: {
    data: t9.table(),
    by: t9.field(),
    columns: t9.number(3),
    chart: t9.json("A chart (e.g. plot(...)) to repeat; its data is replaced by each group."),
    gap: t9.number(16),
    shared: t9.bool(true, "Every panel on the same x and y scales (the whole table's extent), so panels compare at a glance; off: each fits its own rows.")
  },
  expand(p) {
    const params = p.chart && p.chart.kind === "use" ? p.chart.params : null;
    const share = (axis2) => p.shared && params && typeof params[axis2] === "string" ? { [`${axis2}Domain`]: params[`${axis2}Domain`] ?? { data: p.data, field: params[axis2] } } : {};
    const inner = params ? { ...p.chart, params: { ...params, ...share("x"), ...share("y"), data: "@group", title: void 0 } } : p.chart;
    return group9({
      key: "facet",
      layout: { type: "grid", columns: p.columns, gap: p.gap },
      children: [repeat8({ groups: p.data, by: p.by }, group9({
        layout: { type: "rows", gap: 4 },
        children: [text8(e9(`key.name(d.${p.by})`), [0, 0], { size: { h: "auto" }, style: { weight: 600, size: "$size.body", ink: "$ink", baseline: "top" } }), inner]
      }))]
    });
  }
});

// src/map.ts
import { e as e10, group as group10, instances as instances4, op as op8, recipe as recipe10, repeat as repeat9, shape as shape10, geom as geom10, t as t10, text as text9, view } from "@datars/sdk";
var map = recipe10({
  id: "@datars/std/map",
  doc: "A map: regions of a geo source, coloured by a data table joined on a key (choropleth), with neutral backdrop land, over which children (symbols, points, lines, annotations) draw in the same projection.",
  params: {
    source: t10.table("A geo source (GeoJSON, TopoJSON or an atlas like `countries`)."),
    data: t10.table("Values per region (optional)."),
    key: t10.field("The data column holding region ids."),
    value: t10.field("The value to colour by."),
    colorType: t10.oneOf(["sequential", "diverging", "categorical", "piecewise"], "sequential"),
    stops: t10.string(void 0, "Piecewise colour stops: '#22c55e 2 \xB7 #f5a524 4 \xB7 #f97362 6.5'."),
    projection: t10.string("equal-earth", "equal-earth, mercator, web-mercator, natural-earth, albers-usa, orthographic, sweref99tm, planar, \u2026"),
    fit: t10.json("What the projection fits: {keys: [...]}, {bbox: [lon0, lat0, lon1, lat1]} or the whole source (default). A fit clips the map to its box."),
    camera: t10.json("A camera over the map: {fit: {keys: [...]} | {bbox: [x0, y0, x1, y1]}, padding} \u2014 bbox corners may be expressions like geo.x(lon, lat). Cameras fly between states; the projection stays."),
    padding: t10.number(8),
    backdrop: t10.bool(true, "Draw regions without data too (in $map.no-data); off: only regions with data."),
    sea: t10.bool(false, "Fill the map's box with $map.water first (the sea around atlas regions, in themes that colour it)."),
    under: t10.table("Another geo source drawn beneath as neutral land (countries around admin-1 regions)."),
    stroke: t10.bool(true, "Borders between regions."),
    legend: t10.bool(false, "A colour ramp (or swatches, for categorical colour) in the lower-left corner."),
    labels: t10.bool(false),
    format: t10.string(".1~f"),
    selected: t10.string(void 0, "A keyset signal: selected regions stay strong, the others recede."),
    label: t10.prop("Region label (tooltip, accessible name): an expression over the region row; default `name: value`."),
    chapter: t10.string(void 0, "Clicking a region enters this program chapter with the region's id (drill-down)."),
    regions: t10.bool(true, "Draw the source's regions; off: only the layers (a camera beat over a basemap)."),
    base: t10.children("Layers beneath everything, moving with the camera: a basemap (`basemap({ part: 'base' })`)."),
    fillOpacity: t10.number(void 0, "Region opacity; default 0.85 over a `base` basemap (its streets and water show through), else 1."),
    children: t10.children("Layers drawn on top: symbols, geoPoints, geoLines, annotations.")
  },
  tokens: ["map.land", "map.no-data", "map.border", "map.water", "sequential", "diverging", "categorical"],
  expand(p, cx) {
    const scales = {};
    let regions = p.source;
    if (p.data && p.key) {
      const vals = cx.table("values", p.data, op8.derive("id", e10(`d.${p.key}`)));
      regions = cx.table("regions", p.source, op8.join(vals, "id", p.backdrop ? "left" : "inner"));
      scales.color = p.colorType === "piecewise" ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } } : { type: p.colorType, domain: { data: p.data, field: p.colorType === "categorical" ? p.key : p.value }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" };
    }
    const color = p.data ? `d.${p.value} == null ? "$map.no-data" : scale.color(d.${p.colorType === "categorical" ? p.key : p.value})` : `"$map.land"`;
    const fill = p.selected ? e10(`${p.selected}.isEmpty() || ${p.selected}.has(d.id) ? (${color}) : "$map.no-data"`) : p.data ? e10(color) : "$map.land";
    const label = p.label ?? (p.data ? e10(`\`\${d.name ?? d.id}: \${d.${p.value} == null ? "no data" : format(d.${p.value}, ${JSON.stringify(p.format)})}\``) : e10("d.name ?? d.id"));
    const coord = { type: "geo", projection: p.projection, fit: p.fit ?? { source: p.source }, padding: p.padding };
    const layers = [
      p.base?.length ? group10({ key: "base", children: p.base }) : null,
      // Neutral land from another source beneath (decoration: it never pairs with data regions).
      p.under ? group10({ key: "under", children: [repeat9(p.under, shape10(geom10.feature(p.under, e10("d.id")), {
        fill: "$map.land",
        stroke: p.stroke ? { paint: "$map.border", width: 0.5, nonScaling: true } : void 0,
        semantics: { role: "decoration" }
      }))] }) : null,
      p.regions === false ? null : group10({ key: "regions", opacity: (p.fillOpacity ?? (p.base?.length ? 0.85 : 1)) < 1 ? p.fillOpacity ?? 0.85 : void 0, children: [repeat9(regions, shape10(geom10.feature(p.source, e10("d.id")), {
        fill,
        stroke: p.stroke ? { paint: "$map.border", width: 0.5, nonScaling: true } : void 0,
        semantics: { role: "region", label, value: p.data ? e10(`d.${p.value}`) : void 0 },
        on: p.chapter ? { activate: { chapter: p.chapter, key: e10("d.id") } } : void 0,
        pickable: true
      }))] }),
      // Region names where they fit: small regions crowding together (a phone) keep only the first.
      p.labels && p.regions !== false ? group10({ key: "labels", declutter: true, children: [repeat9(regions, text9(e10("d.name ?? d.id"), [e10(`geo.cx(${JSON.stringify(p.source)}, d.id)`), e10(`geo.cy(${JSON.stringify(p.source)}, d.id)`)], { style: { size: "$size.small", ink: "$map.label", align: "middle", baseline: "middle" }, halo: ["$map.label-halo", 2] }))] }) : null,
      ...p.children ?? []
    ];
    const legend2 = p.legend && p.data ? colorLegend(p, cx) : null;
    const sea = p.sea ? shape10(geom10.rect({ x: 0, y: 0, w: e10("box.w"), h: e10("box.h") }), { key: "sea", fill: "$map.water", semantics: { role: "decoration" } }) : null;
    if (p.camera) {
      return group10({
        key: "map",
        scales,
        semantics: { role: "group", label: "Map" },
        children: [sea, view({ key: "view", camera: p.camera, coord, children: layers }), legend2]
      });
    }
    return group10({ key: "map", scales, coord, clip: p.fit ? "box" : void 0, semantics: { role: "group", label: "Map" }, children: [sea, ...layers, legend2] });
  }
});
function colorLegend(p, cx) {
  if (p.colorType === "categorical") {
    return group10({ key: "legend", transform: { translate: [8, e10("box.h - 18")] }, semantics: { role: "legend", label: "Legend" }, layout: { type: "flow", gap: 14 }, children: [
      repeat9({ legend: "color" }, group10({ semantics: { role: "legend-item", label: e10("key.name(d.label)") }, children: [
        shape10(geom10.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e10("d.ink") }),
        text9(e10("key.name(d.label)"), [15, 9], { style: { size: "$size.label", ink: "$ink-2" } })
      ] }))
    ] });
  }
  const pinned = p.colorType === "piecewise" && p.stops ? p.stops.split(/[\s·,;]+/).filter((s) => s !== "" && !Number.isNaN(Number(s))).map(Number) : [];
  const extent = pinned.length >= 2 ? cx.table("extent", p.data, op8.aggregate([], { n: ["count"] }), op8.derive("lo", pinned[0]), op8.derive("hi", pinned[pinned.length - 1])) : cx.table("extent", p.data, op8.aggregate([], { lo: ["min", p.value], hi: ["max", p.value] }));
  const steps = 6, w = 22;
  return group10({ key: "legend", transform: { translate: [8, e10("box.h - 30")] }, semantics: { role: "legend", label: "Colour scale" }, children: [
    repeat9(extent, group10({ key: "ramp", children: [
      ...Array.from({ length: steps }, (_, i) => shape10(geom10.rect({ x: i * w, y: 0, w, h: 9 }), { key: `step-${i}`, fill: e10(`scale.color(d.lo + (d.hi - d.lo) * ${i / (steps - 1)})`), semantics: { role: "decoration" } })),
      text9("", [0, 22], { key: "lo", number: { value: e10("d.lo"), format: p.format }, style: { size: "$size.small", ink: "$ink-2" }, halo: ["$paper", 2] }),
      text9("", [steps * w, 22], { key: "hi", number: { value: e10("d.hi"), format: p.format }, style: { size: "$size.small", ink: "$ink-2", align: "end" }, halo: ["$paper", 2] })
    ] }))
  ] });
}
var symbols = recipe10({
  id: "@datars/std/symbols",
  doc: "Proportional symbols: a circle per region (or point feature) at its visual centre, area \u221D value \u2014 or all one size (`r`) without a value field.",
  params: {
    source: t10.table(),
    data: t10.table(),
    key: t10.field(),
    value: t10.field("Size by this field (omit for same-size dots)."),
    max: t10.number(28, "Largest radius (px)."),
    r: t10.number(4, "Radius without a value field (px)."),
    fill: t10.prop(),
    stops: t10.string(void 0, "Colour by value on piecewise stops ('#bae6fd 7 \xB7 #0b1f3a 14.2') instead of one fill."),
    format: t10.string(",.0f"),
    label: t10.prop("Label per symbol (tooltip, accessible name); default `name: value`.")
  },
  expand(p) {
    const name = `key.name(d.${p.key})`;
    const scales = {};
    if (p.value) scales.size = { type: "sqrt", domain: { data: p.data, field: p.value }, range: [0, p.max], zero: true };
    if (p.value && p.stops) scales.fill = { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } };
    const fill = p.value && p.stops ? e10(`scale.fill(d.${p.value})`) : p.fill ?? "$accent@0.7";
    return group10({
      key: "symbols",
      scales: p.value ? scales : void 0,
      children: [instances4({
        key: "circles",
        from: p.data,
        instanceKey: e10(`d.${p.key}`),
        x: e10(`geo.cx(${JSON.stringify(p.source)}, d.${p.key})`),
        y: e10(`geo.cy(${JSON.stringify(p.source)}, d.${p.key})`),
        r: p.value ? e10(`scale.size(d.${p.value})`) : p.r,
        fill,
        stroke: { paint: "$paper", width: 0.75 },
        screenSize: true,
        label: p.label ?? (p.value ? e10(`\`\${${name}}: \${format(d.${p.value}, ${JSON.stringify(p.format)})}\``) : e10(name))
      })]
    });
  }
});
var geoPoints = recipe10({
  id: "@datars/std/geoPoints",
  doc: "Points at lon/lat (cities, events, stations), in the enclosing map's projection, in $map.marker ringed by $map.label-halo so they read on any land.",
  params: { data: t10.table(), lon: t10.field(), lat: t10.field(), r: t10.prop(), fill: t10.prop(), label: t10.prop(), key: t10.field() },
  tokens: ["map.marker", "map.label-halo"],
  expand(p) {
    return instances4({
      key: "points",
      from: p.data,
      instanceKey: p.key ? e10(`d.${p.key}`) : void 0,
      x: e10(`geo.x(d.${p.lon}, d.${p.lat})`),
      y: e10(`geo.y(d.${p.lon}, d.${p.lat})`),
      r: p.r ?? 3,
      fill: p.fill ?? "$map.marker",
      stroke: { paint: "$map.label-halo", width: 1 },
      screenSize: true,
      label: p.label
    });
  }
});
var geoLines = recipe10({
  id: "@datars/std/geoLines",
  doc: "Line features of a geo source (roads, borders, courses, routes) as non-scaling strokes. With `data` and `key`, one line per row (the feature with that id) \u2014 data, not decoration \u2014 coloured by `value` on piecewise `stops`.",
  params: {
    source: t10.table(),
    ink: t10.ink("$map.border"),
    width: t10.number(1),
    dash: t10.bool(false),
    data: t10.table(),
    key: t10.field("The data field holding each row's feature id."),
    value: t10.field("Colour lines by this field (with `stops`)."),
    stops: t10.string(void 0, "Piecewise colour stops for `value` (`#a 0 \xB7 #b 10`)."),
    format: t10.string(",.1~f")
  },
  expand(p) {
    const stroke = (paint) => ({ paint, width: p.width, nonScaling: true, dash: p.dash ? [4, 3] : void 0 });
    if (!p.data || !p.key) {
      return group10({ key: `lines-${p.source}`, children: [repeat9(p.source, shape10(geom10.feature(p.source, e10("d.id")), { stroke: stroke(p.ink), semantics: { role: "decoration" } }))] });
    }
    const coloured = !!(p.value && p.stops);
    const label = p.value ? e10(`key.name(d.${p.key}) + ": " + format(d.${p.value}, ${JSON.stringify(p.format)})`) : e10(`key.name(d.${p.key})`);
    return group10({
      key: `lines-${p.source}`,
      scales: coloured ? { stroke: { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } } } : void 0,
      children: [repeat9(p.data, shape10(geom10.feature(p.source, e10(`d.${p.key}`)), {
        key: e10(`d.${p.key}`),
        stroke: stroke(coloured ? e10(`scale.stroke(d.${p.value})`) : p.ink),
        semantics: { role: "datum", label },
        pickable: true
      }))]
    });
  }
});
var route = recipe10({
  id: "@datars/std/route",
  doc: "Great-circle routes between two lon/lat points per row.",
  params: { data: t10.table(), lon0: t10.field(), lat0: t10.field(), lon1: t10.field(), lat1: t10.field(), ink: t10.ink("$accent@0.6"), width: t10.prop() },
  expand(p) {
    return group10({ key: "routes", children: [repeat9(p.data, shape10(geom10.path(e10(`geo.geodesic(d.${p.lon0}, d.${p.lat0}, d.${p.lon1}, d.${p.lat1})`)), { stroke: { paint: p.ink, width: p.width ?? 1.5, nonScaling: true, cap: "round" } }))] });
  }
});
var track = recipe10({
  id: "@datars/std/track",
  doc: "A path over time (a storm, a flight): drawn up to `until` (a data-time signal) with a moving head.",
  params: {
    data: t10.table(),
    time: t10.field(),
    lon: t10.field(),
    lat: t10.field(),
    until: t10.prop("Data time to draw up to (e.g. the `year` signal)."),
    ink: t10.ink("$negative"),
    head: t10.bool(true),
    future: t10.bool(false, "Draw the whole path faintly underneath (where it will go).")
  },
  expand(p, cx) {
    const until = typeof p.until === "number" ? String(p.until) : p.until && typeof p.until === "object" && "expr" in p.until ? p.until.expr : "1e18";
    const drawn = cx.table("drawn", p.data, op8.filter(e10(`d.${p.time} <= (${until})`)));
    return group10({
      key: "track",
      children: [
        p.future ? shape10(geom10.polyline({ from: p.data, x: e10(`geo.x(d.${p.lon}, d.${p.lat})`), y: e10(`geo.y(d.${p.lon}, d.${p.lat})`), curve: "catmull-rom" }), { key: "future", stroke: { paint: p.ink, width: 1.5, nonScaling: true, cap: "round", join: "round", dash: [4, 4] }, opacity: 0.35 }) : null,
        // The head is the path's own end: when the clock moves between steps the path grows along
        // itself (a line that runs on is trimmed, not bent) and the head rides its tip.
        shape10(geom10.polyline({ from: drawn, x: e10(`geo.x(d.${p.lon}, d.${p.lat})`), y: e10(`geo.y(d.${p.lon}, d.${p.lat})`), curve: "catmull-rom" }), {
          key: "path",
          stroke: { paint: p.ink, width: 2.5, nonScaling: true, cap: "round", join: "round" },
          markers: p.head ? { end: { type: "dot", r: 5 } } : void 0
        })
      ]
    });
  }
});
var dotDensity = recipe10({
  id: "@datars/std/dotDensity",
  doc: "Dot density: one dot per `per` units of a region's value, scattered evenly inside it (seeded, deterministic). Dots are keyed (region, i): in another state a region's dots pair only with its own, and the region's key splits into them.",
  params: {
    source: t10.table("The geo source whose features the dots fill."),
    data: t10.table("Values per region."),
    key: t10.field("The data column holding region ids (default: the table's key)."),
    value: t10.field(),
    per: t10.number(1e3, "Units per dot."),
    r: t10.number(1.2, "Dot radius (px)."),
    fill: t10.prop(),
    seed: t10.number(1)
  },
  expand(p, cx) {
    const dots = cx.table("dots", p.data, op8.scatterIn({ geo: p.source, key: p.key, count: e10(`floor(d.${p.value} / ${p.per})`), seed: p.seed }));
    const label = p.key ? e10(`key.name(d.${p.key})`) : void 0;
    return instances4({ key: "dots", from: dots, x: e10("geo.x(d.lon, d.lat)"), y: e10("geo.y(d.lon, d.lat)"), r: p.r, fill: p.fill ?? "$ink@0.7", screenSize: true, label });
  }
});

// src/controls.ts
import { e as e11, group as group11, pick, recipe as recipe11, scrub, shape as shape11, geom as geom11, t as t11, text as text10 } from "@datars/sdk";
var SNAPPY = [{ duration: 0.18, easing: "cubic-out" }];
var WASH = (strength = 0.06) => e11(`hover() ? ${strength} : 0`);
var slider = recipe11({
  id: "@datars/std/slider",
  doc: "A slider for a numeric signal: press or drag to set it (snapped to `step`). Shows the label and the current value.",
  params: {
    signal: t11.string(void 0, "The numeric signal it sets."),
    min: t11.number(0),
    max: t11.number(100),
    step: t11.number(0, "Snap step (0 = continuous)."),
    label: t11.string(""),
    format: t11.string(",.0f", "Value format.")
  },
  tokens: ["accent", "rule", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const v = p.signal;
    const x = e11(`scale.x(${v})`);
    const y = 30;
    return group11({
      key: "slider",
      size: { h: 44 },
      scales: { x: { type: "linear", domain: [p.min, p.max], range: [10, "=box.w - 10"] } },
      on: { drag: scrub(v, { step: p.step }) },
      pickable: true,
      // On the whole control (label, value, track): native bridges frame the adjustable element
      // by it, and touch exploration finds it anywhere on the slider.
      semantics: { role: "control", label: e11(`\`${p.label}: \${format(${v}, ${JSON.stringify(p.format)})}\``) },
      children: [
        text10(p.label, [0, 0], { key: "label", style: { size: "$size.label", ink: "$ink-2", baseline: "top" } }),
        text10(e11(`format(${v}, ${JSON.stringify(p.format)})`), [e11("box.w"), 0], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", align: "end", baseline: "top" } }),
        shape11(geom11.segment({ x1: 10, y1: y, x2: e11("box.w - 10"), y2: y }), { key: "track", stroke: { paint: "$rule", width: 4, cap: "round" } }),
        shape11(geom11.segment({ x1: 10, y1: y, x2: x, y2: y }), { key: "fill", stroke: { paint: "$accent", width: 4, cap: "round" } }),
        shape11(geom11.circle({ cx: x, cy: y, r: 15 }), { key: "halo", fill: "$accent", opacity: WASH(0.18) }),
        shape11(geom11.circle({ cx: x, cy: y, r: 8 }), { key: "thumb", fill: "$accent", stroke: { paint: "$paper", width: 2 } })
      ]
    });
  }
});
var lit = (v) => JSON.stringify(v);
var say = (v, labels, i) => labels?.[i] !== void 0 ? lit(String(labels[i])) : typeof v === "string" ? `key.name(${lit(v)})` : lit(String(v));
var current = (sig, opts, labels) => `(${opts.map((v, i) => `${sig} == ${lit(v)} ? ${say(v, labels, i)} : `).join("")}\`\${${sig}}\`)`;
var LABEL = { size: "$size.label", ink: "$ink-2", baseline: "top" };
var segmented = recipe11({
  id: "@datars/std/segmented",
  doc: "One of a few options side by side (a segmented control): click one to set the signal to it; the highlight slides to it. For more options than fit, use `select`.",
  params: {
    signal: t11.string(void 0, "The signal it sets."),
    options: t11.json("The values to choose from (strings or numbers)."),
    labels: t11.json("What each option says (default: its key's name from the document's keys, else the value)."),
    label: t11.string("", "A label above the options.")
  },
  tokens: ["accent", "surface", "rule", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = p.options ?? [];
    const labels = p.labels;
    const sig = p.signal;
    const n = Math.max(1, opts.length);
    const top = p.label ? 20 : 0;
    const h = 32;
    const at2 = opts.map((v, i) => `${sig} == ${lit(v)} ? ${i} : `).join("") + "-1";
    const w = `box.w / ${n}`;
    return group11({
      key: "segmented",
      size: { h: top + h },
      children: [
        p.label ? text10(p.label, [0, 0], { key: "label", style: LABEL }) : null,
        shape11(geom11.rect({ x: 0, y: top, w: e11("box.w"), h, r: 8 }), { key: "track", fill: "$surface", stroke: { paint: "$rule", width: 1 } }),
        // One highlight that slides to the chosen option.
        shape11(geom11.rect({ x: e11(`max(0, ${at2}) * ${w} + 2`), y: top + 2, w: e11(`${w} - 4`), h: h - 4, r: 6 }), { key: "thumb", fill: "$accent", opacity: e11(`(${at2}) >= 0 ? 1 : 0`) }),
        ...opts.map((v, i) => group11({
          key: `option:${String(v)}`,
          pickable: true,
          on: { activate: { set: sig, value: v } },
          semantics: { role: "control", label: e11(`${p.label ? `${lit(p.label + ": ")} + ` : ""}${say(v, labels, i)} + (${sig} == ${lit(v)} ? ", selected" : "")`) },
          children: [
            shape11(geom11.rect({ x: e11(`${i} * ${w}`), y: top, w: e11(w), h }), { key: "hit", pickable: true, fill: "transparent" }),
            shape11(geom11.rect({ x: e11(`${i} * ${w} + 2`), y: top + 2, w: e11(`${w} - 4`), h: h - 4, r: 6 }), { key: "hover", fill: "$ink", opacity: e11(`hover() && ${sig} != ${lit(v)} ? 0.07 : 0`) }),
            text10(e11(say(v, labels, i)), [e11(`(${i} + 0.5) * ${w}`), top + h / 2], { key: "text", style: { size: "$size.label", weight: 600, ink: e11(`${sig} == ${lit(v)} ? "on($accent)" : "$ink"`), align: "middle", baseline: "middle", maxWidth: e11(`${w} - 8`) } })
          ]
        }))
      ]
    });
  }
});
var select = recipe11({
  id: "@datars/std/select",
  doc: "A dropdown for one of many options: click to open the list over the chart, click an option to set the signal to it; a click anywhere else closes it. The list floats above the whole scene. On phones and tablets the platform's own picker opens instead (web, iOS, Android).",
  params: {
    signal: t11.string(void 0, "The signal it sets."),
    options: t11.json("The values to choose from (strings or numbers)."),
    labels: t11.json("What each option says (default: its key's name from the document's keys, else the value)."),
    label: t11.string("", "A label above the box."),
    open: t11.oneOf(["down", "up"], "down", "Which way the list opens (up for a select near the bottom of the chart)."),
    rows: t11.number(6, "At most this many options in a column: a longer list opens in columns side by side, so it fits a small chart.")
  },
  tokens: ["surface", "paper", "rule", "accent", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = p.options ?? [];
    const labels = p.labels;
    const sig = p.signal;
    const state = `${sig}.open`;
    const isOpen = `${state} == \`o:\${${sig}}\``;
    const top = p.label ? 20 : 0;
    const h = 34;
    const row = 28;
    const cols = Math.max(1, Math.ceil(opts.length / Math.max(1, p.rows || 6)));
    const perCol = Math.max(1, Math.ceil(opts.length / cols));
    const colW = `max(box.w / ${cols}, 110)`;
    const listH = perCol * row + 8;
    const listY = p.open === "up" ? top - listH - 4 : top + h + 4;
    const chevron = (up) => up ? [top + h / 2 + 3, top + h / 2 - 2] : [top + h / 2 - 2, top + h / 2 + 3];
    const [cy0, cy1] = chevron(false), [uy0, uy1] = chevron(true);
    return group11({
      key: "select",
      size: { h: top + h },
      children: [
        p.label ? text10(p.label, [0, 0], { key: "label", style: LABEL }) : null,
        group11({
          key: "box",
          pickable: true,
          // A click opens the drawn list; a host with a picker of its own (a phone's) offers that
          // instead, from `pick`.
          on: { activate: { set: state, value: e11(`${isOpen} ? "" : \`o:\${${sig}}\``) }, pick: pick(sig, opts, opts.map((v, i) => e11(say(v, labels, i)))) },
          semantics: { role: "control", label: e11(`${p.label ? `${lit(p.label + ": ")} + ` : ""}${current(sig, opts, labels)}`) },
          children: [
            shape11(geom11.rect({ x: 0, y: top, w: e11("box.w"), h, r: 8 }), { key: "frame", pickable: true, fill: "$surface", stroke: { paint: e11(`${isOpen} ? "$accent" : hover() ? "$ink-2" : "$rule"`), width: e11(`${isOpen} ? 1.5 : 1`) } }),
            shape11(geom11.rect({ x: 1, y: top + 1, w: e11("box.w - 2"), h: h - 2, r: 7 }), { key: "hover", fill: "$ink", opacity: e11(`hover() && !(${isOpen}) ? 0.04 : 0`) }),
            text10(e11(current(sig, opts, labels)), [12, top + h / 2], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle", maxWidth: e11("box.w - 40") } }),
            // A chevron that points up while the list is open.
            shape11(geom11.segment({ x1: e11("box.w - 22"), y1: e11(`${isOpen} ? ${uy0} : ${cy0}`), x2: e11("box.w - 17"), y2: e11(`${isOpen} ? ${uy1} : ${cy1}`) }), { key: "chevron-a", stroke: { paint: "$ink-2", width: 1.5, cap: "round" } }),
            shape11(geom11.segment({ x1: e11("box.w - 17"), y1: e11(`${isOpen} ? ${uy1} : ${cy1}`), x2: e11("box.w - 12"), y2: e11(`${isOpen} ? ${uy0} : ${cy0}`) }), { key: "chevron-b", stroke: { paint: "$ink-2", width: 1.5, cap: "round" } })
          ]
        }),
        // While open: a veil over everything (a click on it closes the list), and the list above it.
        shape11(geom11.rect({ x: -1e5, y: -1e5, w: 2e5, h: 2e5 }), { key: "veil", when: e11(isOpen), z: 1e3, pickable: true, fill: "transparent", on: { activate: { set: state, value: "" } } }),
        group11({
          key: "list",
          when: e11(isOpen),
          z: 1001,
          children: [
            shape11(geom11.rect({ x: 0, y: listY, w: e11(`${colW} * ${cols}`), h: listH, r: 8 }), { key: "panel", fill: "$paper", stroke: { paint: "$rule", width: 1 } }),
            ...opts.map((v, i) => group11({
              key: `option:${String(v)}`,
              pickable: true,
              on: { activate: { set: sig, value: v } },
              semantics: { role: "control", label: e11(`${say(v, labels, i)} + (${sig} == ${lit(v)} ? ", selected" : "")`) },
              children: (() => {
                const x = `4 + ${Math.floor(i / perCol)} * ${colW}`;
                const y = listY + 4 + i % perCol * row;
                return [
                  // The chosen option has no target: a click on it falls through to the veil and closes.
                  shape11(geom11.rect({ x: e11(x), y, w: e11(`${colW} - 8`), h: row, r: 6 }), { key: "hit", when: e11(`${sig} != ${lit(v)}`), pickable: true, fill: "transparent" }),
                  shape11(geom11.rect({ x: e11(x), y, w: e11(`${colW} - 8`), h: row, r: 6 }), { key: "chosen", fill: "$accent", opacity: e11(`${sig} == ${lit(v)} ? 0.14 : 0`) }),
                  shape11(geom11.rect({ x: e11(x), y, w: e11(`${colW} - 8`), h: row, r: 6 }), { key: "hover", fill: "$ink", opacity: e11(`hover() && ${sig} != ${lit(v)} ? 0.07 : 0`) }),
                  text10(e11(say(v, labels, i)), [e11(`${x} + 10`), y + row / 2], { key: "text", style: { size: "$size.label", weight: e11(`${sig} == ${lit(v)} ? 600 : 400`), ink: "$ink", baseline: "middle", maxWidth: e11(`${colW} - 28`) } })
                ];
              })()
            }))
          ]
        })
      ]
    });
  }
});
var toggle = recipe11({
  id: "@datars/std/toggle",
  doc: "A switch for a boolean signal: click to flip it. Shows its label beside the switch.",
  params: { signal: t11.string(void 0, "The boolean signal it flips."), label: t11.string("") },
  tokens: ["accent", "rule", "paper", "ink", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const sig = p.signal;
    return group11({
      key: "toggle",
      size: { h: 28 },
      pickable: true,
      on: { activate: { set: sig, value: e11(`!${sig}`) } },
      semantics: { role: "control", label: e11(`${lit((p.label || sig) + ": ")} + (${sig} ? "on" : "off")`) },
      children: [
        shape11(geom11.rect({ x: 0, y: 0, w: e11("box.w"), h: 28 }), { key: "hit", pickable: true, fill: "transparent" }),
        shape11(geom11.rect({ x: 0, y: 3, w: 40, h: 22, r: 11 }), { key: "track", fill: e11(`${sig} ? "$accent" : "$rule"`) }),
        shape11(geom11.circle({ cx: e11(`${sig} ? 29 : 11`), cy: 14, r: 13 }), { key: "halo", fill: "$ink", opacity: WASH(0.1) }),
        shape11(geom11.circle({ cx: e11(`${sig} ? 29 : 11`), cy: 14, r: 8 }), { key: "knob", fill: "$paper" }),
        p.label ? text10(p.label, [52, 14], { key: "label", style: { size: "$size.label", ink: "$ink", baseline: "middle" } }) : null
      ]
    });
  }
});
var checklist = recipe11({
  id: "@datars/std/checklist",
  doc: "Checkboxes for a key set: click an option to add it to the signal or take it out (series to show, regions to compare). Options wrap to the width.",
  params: {
    signal: t11.string(void 0, "The key-set signal it toggles keys in."),
    options: t11.json("The keys to offer."),
    labels: t11.json("What each option says (default: its key's name from the document's keys, else the key)."),
    label: t11.string("", "A label above the options.")
  },
  tokens: ["accent", "ink", "ink-2", "paper", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = p.options ?? [];
    const labels = p.labels;
    const sig = p.signal;
    const on = (v) => `${sig}.has(${lit(String(v))})`;
    return group11({
      key: "checklist",
      size: { h: "auto" },
      layout: { type: "rows", gap: 6 },
      children: [
        p.label ? text10(p.label, [0, 0], { key: "label", size: { h: "auto" }, style: LABEL }) : null,
        group11({
          key: "options",
          size: { h: "auto" },
          layout: { type: "flow", gap: 14 },
          children: opts.map((v, i) => group11({
            key: `option:${String(v)}`,
            pickable: true,
            on: { activate: { toggle: sig, value: String(v) } },
            semantics: { role: "control", label: e11(`${say(v, labels, i)} + (${on(v)} ? ", checked" : ", not checked")`) },
            children: [
              // The box and the gap up to the label are one target (the label is another).
              shape11(geom11.rect({ x: -3, y: -3, w: 29, h: 24 }), { key: "hit", pickable: true, fill: "transparent" }),
              shape11(geom11.rect({ x: -5, y: -5, w: 28, h: 28, r: 7 }), { key: "hover", fill: "$ink", opacity: WASH(0.07) }),
              shape11(geom11.rect({ x: 0, y: 0, w: 18, h: 18, r: 4 }), { key: "box", pickable: true, fill: e11(`${on(v)} ? "$accent" : "$paper"`), stroke: { paint: e11(`${on(v)} ? "$accent" : hover() ? "$ink" : "$ink-2"`), width: 1.5 } }),
              shape11(geom11.segment({ x1: 4.5, y1: 9.5, x2: 7.8, y2: 12.8 }), { key: "check-a", opacity: e11(`${on(v)} ? 1 : 0`), stroke: { paint: "on($accent)", width: 2, cap: "round" } }),
              shape11(geom11.segment({ x1: 7.8, y1: 12.8, x2: 13.5, y2: 6 }), { key: "check-b", opacity: e11(`${on(v)} ? 1 : 0`), stroke: { paint: "on($accent)", width: 2, cap: "round" } }),
              text10(e11(say(v, labels, i)), [26, 9], { key: "text", pickable: true, style: { size: "$size.label", ink: "$ink", baseline: "middle" } })
            ]
          }))
        })
      ]
    });
  }
});
var range = recipe11({
  id: "@datars/std/range",
  doc: "A slider with two thumbs for a range: drag either end to set the `lo` or `hi` signal (snapped to `step`); the thumbs can't cross. Filter rows with `d.x >= lo && d.x <= hi`.",
  params: {
    lo: t11.string(void 0, "The numeric signal for the low end."),
    hi: t11.string(void 0, "The numeric signal for the high end."),
    min: t11.number(0),
    max: t11.number(100),
    step: t11.number(0, "Snap step (0 = continuous)."),
    label: t11.string(""),
    format: t11.string(",.0f", "Value format.")
  },
  tokens: ["accent", "rule", "paper", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const { lo, hi } = p;
    const f = lit(p.format);
    const y = 30;
    const x = (s) => `scale.x(${s})`;
    const half = `clamp((${x(hi)} - ${x(lo)}) / 2, 0, 16)`;
    const thumb = (key) => group11({
      key,
      pickable: true,
      on: { drag: scrub(key === "lo" ? lo : hi, { step: p.step, ...key === "lo" ? { max: e11(hi) } : { min: e11(lo) } }) },
      semantics: { role: "control", label: e11(`\`${p.label || "Range"} ${key === "lo" ? "from" : "to"}: \${format(${key === "lo" ? lo : hi}, ${f})}\``) },
      children: [
        key === "lo" ? shape11(geom11.rect({ x: e11(`${x(lo)} - 16`), y: 12, w: e11(`16 + ${half}`), h: 32 }), { key: "hit", fill: "transparent" }) : shape11(geom11.rect({ x: e11(`${x(hi)} - ${half}`), y: 12, w: e11(`16 + ${half}`), h: 32 }), { key: "hit", fill: "transparent" }),
        shape11(geom11.circle({ cx: e11(x(key === "lo" ? lo : hi)), cy: y, r: 15 }), { key: "halo", fill: "$accent", opacity: WASH(0.18) }),
        shape11(geom11.circle({ cx: e11(x(key === "lo" ? lo : hi)), cy: y, r: 8 }), { key: "thumb", fill: "$accent", stroke: { paint: "$paper", width: 2 } })
      ]
    });
    return group11({
      key: "range",
      size: { h: 44 },
      scales: { x: { type: "linear", domain: [p.min, p.max], range: [10, "=box.w - 10"] } },
      semantics: { role: "control", label: e11(`\`${p.label || "Range"}: \${format(${lo}, ${f})}\u2013\${format(${hi}, ${f})}\``) },
      children: [
        text10(p.label, [0, 0], { key: "label", style: LABEL }),
        text10(e11(`\`\${format(${lo}, ${f})} \u2013 \${format(${hi}, ${f})}\``), [e11("box.w"), 0], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", align: "end", baseline: "top" } }),
        shape11(geom11.segment({ x1: 10, y1: y, x2: e11("box.w - 10"), y2: y }), { key: "track", stroke: { paint: "$rule", width: 4, cap: "round" } }),
        shape11(geom11.segment({ x1: e11(x(lo)), y1: y, x2: e11(x(hi)), y2: y }), { key: "fill", stroke: { paint: "$accent", width: 4, cap: "round" } }),
        thumb("lo"),
        thumb("hi")
      ]
    });
  }
});
var button = recipe11({
  id: "@datars/std/button",
  doc: "A button: click to fire a program event (`next`, `prev`, `back`, `goto:<state>`) or set a signal to a value (a reset, a preset). As wide as its box.",
  params: {
    label: t11.string(""),
    event: t11.string(void 0, "A program event to fire."),
    set: t11.string(void 0, "A signal to set (with `value`)."),
    value: t11.json("The value `set` sets (a literal, or an expression)."),
    kind: t11.oneOf(["primary", "secondary"], "secondary")
  },
  tokens: ["accent", "surface", "rule", "ink", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const primary = p.kind === "primary";
    const action = p.event ? { event: p.event } : { set: p.set, value: p.value };
    return group11({
      key: "button",
      size: { h: 36 },
      pickable: true,
      on: { activate: action },
      semantics: { role: "control", label: p.label },
      children: [
        shape11(geom11.rect({ x: 0, y: 0, w: e11("box.w"), h: 36, r: 8 }), { key: "face", pickable: true, fill: primary ? "$accent" : "$surface", stroke: primary ? void 0 : { paint: e11(`hover() ? "$ink-2" : "$rule"`), width: 1 } }),
        shape11(geom11.rect({ x: 0, y: 0, w: e11("box.w"), h: 36, r: 8 }), { key: "hover", fill: "$ink", opacity: WASH(primary ? 0.12 : 0.05) }),
        text10(p.label, [e11("box.w / 2"), 18], { key: "text", style: { size: "$size.label", weight: 600, ink: primary ? "on($accent)" : "$ink", align: "middle", baseline: "middle", maxWidth: e11("box.w - 12") } })
      ]
    });
  }
});

// src/basemap.ts
import { e as e12, geom as geom12, group as group12, recipe as recipe12, shape as shape12, t as t12, text as text11, tiles } from "@datars/sdk";
var HIGHWAY = ["motorway", "trunk", "motorway_link", "trunk_link", "Major Highway"];
var MAJOR = ["primary", "secondary", "primary_link", "secondary_link", "Secondary Highway"];
var MINOR = ["tertiary", "tertiary_link", "residential", "unclassified", "living_street", "pedestrian", "Road"];
var roadClass = (kind, raw) => e12(`d.kind == ${JSON.stringify(kind)} || ${JSON.stringify(raw)}.includes(d.type)`);
function byZoom(stops, extra = 0) {
  let out = String(stops[0][1] + extra);
  for (const [z, w] of stops.slice(1)) out = `tile.zoom >= ${z} ? ${w + extra} : (${out})`;
  return `=${out}`;
}
var MINOR_W = [[0, 0.5], [12, 1.3], [14, 2.6]];
var MAJOR_W = [[0, 0.7], [9, 1.1], [11, 1.8], [13, 3]];
var HIGHWAY_W = [[0, 0.9], [8, 1.4], [10, 2.2], [12, 3.2], [14, 4.5]];
var basemap = recipe12({
  id: "@datars/std/basemap",
  doc: "A basemap from a vector-tile source (our own OSM / Natural Earth archives): sea, land, water, parks, buildings, roads by class, borders and place labels, in the enclosing map's projection, styled by the theme's map tokens. Put it first inside a view with a `geo` coordinate system; data layers go after it (or between `part: base` and `part: labels`).",
  params: {
    source: t12.table("A `tiles` source (data.tiles(url))."),
    part: t12.oneOf(["all", "base", "labels"], "all", "Split the map around data layers: `base` below them, `labels` above."),
    labels: t12.bool(true, "Place names (screen-space placement, no overlaps)."),
    roads: t12.bool(true),
    buildings: t12.bool(true),
    boundaries: t12.bool(true, "Country borders."),
    sea: t12.bool(true, "Fill the world behind the land with $map.water."),
    tileSize: t12.number(512, "On-screen tile size the zoom aims for (px). Smaller: more detail, more tiles."),
    layers: t12.json("Archive layer names, if they differ from the defaults: { land, water, lakes, parks, rivers, buildings, roads, boundaries, places }.")
  },
  tokens: ["map.water", "map.land", "map.park", "map.building", "map.road", "map.road-major", "map.border", "map.label", "map.label-halo"],
  expand(p) {
    const L = { land: "land", water: "water", lakes: "lakes", parks: "parks", rivers: "rivers", buildings: "buildings", roads: "roads", boundaries: "boundaries", places: "places", ...p.layers ?? {} };
    const fill = (layer, ink, o = {}) => ({ layer, template: shape12(geom12.feature(), { fill: ink }), ...o });
    const line2 = (layer, ink, width, o = {}) => {
      const { dash, ...rest } = o;
      return { layer, template: shape12(geom12.feature(), { stroke: { paint: ink, width, nonScaling: true, cap: "round", join: "round", dash } }), ...rest };
    };
    const base = p.part !== "labels";
    const labels = p.part !== "base" && p.labels;
    const layers = [];
    if (base) {
      layers.push(
        fill(L.land, "$map.land"),
        fill(L.parks, "$map.park"),
        fill(L.water, "$map.water", { filter: e12("d.$type == 'polygon'") }),
        fill(L.lakes, "$map.water"),
        line2(L.rivers, "$map.water", byZoom([[0, 0.6], [7, 1], [10, 1.6]]), { minzoom: 5 })
      );
      if (p.buildings) layers.push(fill(L.buildings, "$map.building", { minzoom: 12 }));
      if (p.roads) {
        const minor = roadClass("minor", MINOR);
        layers.push(
          // Street-level minor roads get a casing so white streets read on pale land.
          line2(L.roads, "$map.building", byZoom(MINOR_W, 1.6), { id: "roads-minor-casing", filter: minor, minzoom: 12 }),
          line2(L.roads, "$map.road", byZoom(MINOR_W), { id: "roads-minor", filter: minor, minzoom: 10 }),
          line2(L.roads, "$map.road-major", byZoom(MAJOR_W), { id: "roads-major", filter: roadClass("major", MAJOR), minzoom: 7 }),
          line2(L.roads, "$map.road-major", byZoom(HIGHWAY_W), { id: "roads-highway", filter: roadClass("highway", HIGHWAY), minzoom: 5 })
        );
      }
      if (p.boundaries) layers.push(line2(L.boundaries, "$map.border", byZoom([[0, 0.6], [4, 0.9], [8, 1.3]]), { dash: [3, 2] }));
    }
    if (labels) {
      layers.push({
        layer: L.places,
        labels: true,
        priority: e12("d.pop ?? (100 - (d.rank ?? 50))"),
        template: text11(e12("d.name"), [e12("d.$x"), e12("d.$y")], {
          key: e12("d.name"),
          style: { size: e12("d.rank != null && d.rank <= 2 ? 13 : 11.5"), weight: e12("d.rank != null && d.rank <= 2 ? 600 : 400"), ink: "$map.label", align: "middle", baseline: "middle" },
          halo: ["$map.label-halo", 2.5],
          semantics: { role: "label", label: e12("d.name") }
        })
      });
    }
    const children = [];
    if (base && p.sea) {
      children.push(shape12(geom12.rect({ x: e12("geo.x(-180, 85.0511)"), y: e12("geo.y(-180, 85.0511)"), w: e12("geo.x(180, -85.0511) - geo.x(-180, 85.0511)"), h: e12("geo.y(180, -85.0511) - geo.y(-180, 85.0511)") }), { key: "sea", fill: "$map.water" }));
    }
    children.push(tiles({ key: "tiles", source: p.source, tileSize: p.tileSize, layers }));
    return group12({ key: p.part === "labels" ? "basemap-labels" : "basemap", semantics: { role: "decoration" }, children });
  }
});
var attribution = recipe12({
  id: "@datars/std/attribution",
  doc: "The data credit a map owes (OpenStreetMap's licence requires it), in the bottom-right corner of its box, legible and linked to the licence page (hosts make it a real link; SVG/PDF keep it). Place it outside the map's view so it stays put while the camera moves.",
  // It sits on the map, so it takes the map's label colours (legible on a dark basemap too).
  params: {
    text: t12.string("\xA9 OpenStreetMap contributors, Natural Earth"),
    ink: t12.ink("$map.label"),
    link: t12.string("https://www.openstreetmap.org/copyright", "Where the credit links (OpenStreetMap's copyright page, as its guidelines ask).")
  },
  tokens: ["map.label", "map.label-halo"],
  expand(p) {
    return text11(p.text, [e12("box.w - 6"), e12("box.h - 5")], {
      key: "attribution",
      style: { size: 10.5, ink: p.ink, align: "end", baseline: "bottom" },
      halo: ["$map.label-halo", 2.5],
      semantics: { role: "annotation", label: p.text, link: p.link || void 0 }
    });
  }
});

// src/cloud.ts
import { e as e13, instances as instances5, recipe as recipe13, t as t13, view as view2 } from "@datars/sdk";
var cloud = recipe13({
  id: "@datars/std/cloud",
  doc: "A point cloud of any size (millions of rows): dots at their own x/y in an explorable view \u2014 a density-preserving sample at every zoom, every row up close, a tooltip on each.",
  params: {
    data: t13.table("A table, or a point archive (a `tiles` source the publish compiler wrote)."),
    x: t13.field(),
    y: t13.field(),
    r: t13.prop("Dot radius in screen px: a number, or an expression over the row."),
    fill: t13.prop("Dot ink, or an expression over the row (default $mark)."),
    opacity: t13.prop("Per-dot opacity: an expression over the row."),
    label: t13.prop("Tooltip and accessible name per row (default `x, y`)."),
    name: t13.string(void 0, "What the rows are, for the accessible description ('4,000,000 stars')."),
    bbox: t13.json("The extent to frame, [x0, y0, x1, y1] in the rows' units. Default: the rows' extent \u2014 give it for millions of rows (finding it reads them all) and for point archives."),
    padding: t13.number(12),
    explore: t13.string("cloud", "The signals the camera explores with (`<explore>.x`, `.y`, `.zoom`)."),
    maxZoom: t13.number(2048, "How far in the reader can zoom, \xD7 the whole."),
    points: t13.number(15e4, "The most dots a frame draws: a view holding more shows a uniform sample this size, one holding fewer shows every row."),
    budget: t13.number(2048, "Rows per tile of the index at average density (smaller: lighter tiles, more of them)."),
    glow: t13.prop("Radius (px) of a glow under the dots (a number or an expression over the row): a sample drawn large and faint, so density reads at a glance and, up close, stars bloom. None by default."),
    glowOpacity: t13.prop("The glow's opacity per dot: a number (default 0.05) or an expression over the row (weight bright rows)."),
    glowPoints: t13.number(6e3, "How many dots the glow draws at most."),
    children: t13.children("Drawn over the dots, in the same coordinates (annotations, marks).")
  },
  tokens: ["mark"],
  expand(p) {
    const q3 = JSON.stringify;
    const bbox = p.bbox ?? [
      e13(`table.min(${q3(p.data)}, ${q3(p.x)})`),
      e13(`table.min(${q3(p.data)}, ${q3(p.y)})`),
      e13(`table.max(${q3(p.data)}, ${q3(p.x)})`),
      e13(`table.max(${q3(p.data)}, ${q3(p.y)})`)
    ];
    return view2({
      key: "cloud",
      camera: { fit: { bbox }, padding: p.padding, explore: p.explore, maxZoom: p.maxZoom },
      children: [
        p.glow != null && p.glow !== 0 ? instances5({
          key: "glow",
          from: p.data,
          x: e13(`d.${p.x}`),
          y: e13(`d.${p.y}`),
          r: p.glow,
          fill: p.fill ?? "$mark",
          opacity: p.glowOpacity ?? 0.05,
          screenSize: true,
          lod: { points: p.glowPoints, budget: p.budget },
          semantics: { role: "decoration" }
        }) : null,
        instances5({
          key: "rows",
          from: p.data,
          x: e13(`d.${p.x}`),
          y: e13(`d.${p.y}`),
          r: p.r ?? 1.2,
          fill: p.fill ?? "$mark",
          opacity: p.opacity,
          label: p.label ?? e13(`\`\${d.${p.x}}, \${d.${p.y}}\``),
          screenSize: true,
          lod: { points: p.points, budget: p.budget },
          semantics: { role: "series", label: p.name ?? `${p.data}: ${p.y} against ${p.x}` }
        }),
        ...p.children ?? []
      ].filter(Boolean)
    });
  }
});

// src/bigdata.ts
import { e as e14, geom as geom13, group as group13, instances as instances6, op as op9, recipe as recipe14, repeat as repeat10, shape as shape13, t as t14, text as text12 } from "@datars/sdk";
var q = JSON.stringify;
function shade(ramp, v) {
  if (ramp === "sqrt") return `sign(${v}) * sqrt(abs(${v}))`;
  if (ramp === "log") return `sign(${v}) * log10(1 + abs(${v}))`;
  return v;
}
var aspectOf = (size) => Math.max(0.05, Math.round(size[1] / Math.max(1, size[0]) * 50) / 50);
var keyTitle = (value, fn, unit, per) => (!value || fn === "count" ? `${unit} per ${per}` : `${fn} ${value}`).replace(/^./, (c) => c.toUpperCase());
var binTip = (value, fn, unit, format, where) => e14(`${fn === "count" || !value ? "" : `format(d.value, ${q(format)}) + ${q(` ${fn} ${value} \xB7 `)} + `}format(d.count, ",") + ${q(` ${unit} ${where} `)} + ${xLabel} + ", " + ${yLabel}`);
function rampKey(scale, tbl, title2, format, at2) {
  if (at2 === "none") return null;
  const [lo, hi] = [`table.min(${q(tbl)}, "shade")`, `table.max(${q(tbl)}, "shade")`];
  const N = 6, W = 14, H = 8;
  const small = { size: "$size.small", ink: "$ink-2", baseline: "top" };
  const box = group13({
    key: "box",
    size: { w: N * W, h: "auto" },
    layout: { type: "rows", gap: 3 },
    backdrop: { fill: "$paper", radius: 3, padding: 5 },
    semantics: { role: "legend", label: title2 },
    children: [
      text12(title2, [0, 0], { key: "title", size: { h: "auto" }, style: small }),
      group13({ key: "ramp", size: { h: H }, children: [
        repeat10({ count: N }, shape13(geom13.rect({ x: e14(`d.index * ${W}`), y: 0, w: W, h: H }), { fill: e14(`scale.${scale}(${lo} + (${hi} - ${lo}) * d.index / ${N - 1})`) }))
      ] }),
      group13({ key: "ends", size: { h: "auto" }, children: [
        text12(e14(`format(table.min(${q(tbl)}, "value"), ${q(format)})`), [0, 0], { key: "lo", style: { ...small, ink: "$muted" } }),
        text12(e14(`format(table.max(${q(tbl)}, "value"), ${q(format)})`), [N * W, 0], { key: "hi", style: { ...small, ink: "$muted", align: "end" } })
      ] })
    ]
  });
  return group13({ key: "key", layout: { type: "stack", padding: 6, align: "start" }, dodge: [at2], children: [box] });
}
var xLabel = "scale.x.label(d.x)";
var yLabel = "scale.y.label(d.y)";
var hexbin = recipe14({
  id: "@datars/std/hexbin",
  doc: "Hexagonal bins: rows of any number counted into hexagons a few px across, each coloured by its count (or an aggregate of a field) \u2014 a density map of a million points, drawn as a few thousand hexagons. Put it in a plot.",
  params: {
    data: t14.table("The rows (any number: they are binned once per data, in the engine)."),
    x: t14.field("Field on the plot's x scale (numbers or dates)."),
    y: t14.field("Field on the plot's y scale."),
    radius: t14.number(8, "Hexagon radius in px: the lattice is laid over the plot area at this size, so hexagons stay this size on a phone."),
    value: t14.field("A field to aggregate per hexagon (default: count the rows)."),
    fn: t14.oneOf(["count", "sum", "mean", "min", "max"], void 0, "How `value` is aggregated (default: mean with a `value`, else count)."),
    ramp: t14.oneOf(["linear", "sqrt", "log"], void 0, "How the value maps onto the colour ramp (default: sqrt for counts \u2014 sparse hexagons stay visible \u2014 linear for aggregates)."),
    size: t14.bool(false, "Size each hexagon by its value too (area \u221D value), so density reads twice \u2014 or alone, with a single `fill`."),
    fill: t14.prop("One ink for every hexagon instead of the sequential ramp (a size-only map, with `size`)."),
    extent: t14.json("The extent to bin, [x0, y0, x1, y1] in the rows' units (default: the rows' own)."),
    gap: t14.number(0, "Space between neighbouring hexagons, px."),
    legend: t14.oneOf(["top-right", "top-left", "bottom-right", "bottom-left", "none"], "top-right", "Where the colour key goes: a corner of the plot area, or none."),
    format: t14.string(",.3~s", "Number format for the key and the tooltips."),
    label: t14.prop("Tooltip per hexagon: an expression over its row (`d.count`, `d.value`, `d.x`, `d.y`); default: how many rows, near where."),
    name: t14.string(void 0, "What the rows are, for the accessible description ('500,000 taxi trips')."),
    unit: t14.string("rows", "What one row is, in the plural, for the key and the tooltips ('trips': 'Trips per hexagon')."),
    aspect: t14.number(0, "The lattice's height \xF7 width on screen (0: the plot area's, measured when the recipe expands).")
  },
  tokens: ["sequential", "paper", "ink-2", "muted", "size.small"],
  expand(p, cx) {
    const r = Math.max(2, p.radius || 8);
    const columns = Math.max(1, Math.round(cx.size[0] / (r * Math.sqrt(3))));
    const fn = p.fn ?? (p.value ? "mean" : "count");
    const ramp = p.ramp ?? (p.value && fn !== "count" ? "linear" : "sqrt");
    const bins = cx.table(
      "hexbin",
      p.data,
      op9.hexbin({ x: p.x, y: p.y, columns, aspect: p.aspect || aspectOf(cx.size), extent: p.extent, value: p.value, fn }),
      op9.derive("shade", e14(shade(ramp, "d.value")))
    );
    const k = p.size ? `clamp(sqrt(max(d.value, 0) / max(table.max(${q(bins)}, "value"), 1e-12)), 0.18, 1)` : "1";
    const inset = p.gap ? `max(0, 1 - ${p.gap / 2} / max(d.hwp, 0.01))` : "(1 + 0.5 / max(d.hwp, 1))";
    const px = cx.table(
      "hexagons",
      bins,
      op9.derive("cx", e14("scale.x(d.x)")),
      op9.derive("cy", e14("scale.y(d.y)")),
      op9.derive("hwp", e14("abs(scale.x(d.x + d.hw) - scale.x(d.x))")),
      op9.derive("hhp", e14("abs(scale.y(d.y + d.hh) - scale.y(d.y))")),
      op9.derive("f", e14(`${k} * ${inset}`))
    );
    const corner = ([a, b]) => `(d.cx + ${a} * d.hwp * d.f) + "," + (d.cy + ${b} * d.hhp * d.f)`;
    const hexPath = `"M" + ${[[0, -1], [1, -0.5], [1, 0.5], [0, 1], [-1, 0.5], [-1, -0.5]].map((c) => corner(c)).join(' + "L" + ')} + "Z"`;
    const unit = p.unit || "rows";
    const tip = p.label ?? binTip(p.value, fn, unit, p.format, "near");
    return group13({
      key: "hexbin",
      semantics: { role: "series", label: e14(`${q(p.name ?? p.data)} + ": " + format(table.count(${q(bins)}), ",") + " hexagons, up to " + format(table.max(${q(bins)}, "count"), ",") + ${q(` ${unit} each`)}`) },
      scales: { hex: { type: "sequential", domain: { data: bins, field: "shade" }, range: "$sequential" } },
      children: [
        // Keyed by the lattice: hexagons of another size are other hexagons, so a change of size
        // crossfades the two layers whole (the same lattice morphs hexagon by hexagon).
        group13({ key: `hexagons-${columns}`, children: [repeat10(px, shape13(geom13.path(e14(hexPath)), { fill: p.fill ?? e14("scale.hex(d.shade)"), pickable: false }))] }),
        // Where the pointer finds a hexagon: an invisible disc as wide as each, with its tooltip
        // (instances, so the accessible description lists a sample, not every hexagon).
        instances6({ key: `pick-${columns}`, from: px, x: e14("d.cx"), y: e14("d.cy"), r: e14("d.hwp"), fill: "transparent", instanceKey: e14("d.hex"), label: tip }),
        p.fill === void 0 ? rampKey("hex", bins, keyTitle(p.value, fn, unit, "hexagon"), p.format, p.legend) : null
      ]
    });
  }
});
var heatmap2d = recipe14({
  id: "@datars/std/heatmap2d",
  doc: "A 2-D density raster: rows of any number counted into a regular grid of cells over the plot's x and y scales in one pass, each cell coloured by its count (or an aggregate of a field). Put it in a plot.",
  params: {
    data: t14.table("The rows (any number: counted once per data, in the engine)."),
    x: t14.field("Field on the plot's x scale (numbers or dates)."),
    y: t14.field("Field on the plot's y scale."),
    cell: t14.number(6, "Cell size in px: the grid is as many cells across and down as the plot area holds."),
    columns: t14.number(0, "Cells across (0: from `cell`)."),
    rows: t14.number(0, "Cells down (0: from `cell`)."),
    value: t14.field("A field to aggregate per cell (default: count the rows)."),
    fn: t14.oneOf(["count", "sum", "mean", "min", "max"], void 0, "How `value` is aggregated (default: mean with a `value`, else count)."),
    ramp: t14.oneOf(["linear", "sqrt", "log"], void 0, "How the value maps onto the colour ramp (default: sqrt for counts, linear for aggregates)."),
    extent: t14.json("The extent to grid, [x0, y0, x1, y1] in the rows' units (default: the rows' own)."),
    gap: t14.number(0, "Space between cells, px."),
    legend: t14.oneOf(["top-right", "top-left", "bottom-right", "bottom-left", "none"], "top-right", "Where the colour key goes: a corner of the plot area, or none."),
    format: t14.string(",.3~s", "Number format for the key and the tooltips."),
    label: t14.prop("Tooltip per cell: an expression over its row (`d.count`, `d.value`, `d.x0`\u2026`d.y1`); default: how many rows, where."),
    name: t14.string(void 0, "What the rows are, for the accessible description."),
    unit: t14.string("rows", "What one row is, in the plural, for the key and the tooltips ('pickups': 'Pickups per cell').")
  },
  tokens: ["sequential", "paper", "ink-2", "muted", "size.small"],
  expand(p, cx) {
    const cell2 = Math.max(1, p.cell || 6);
    const columns = p.columns || Math.max(1, Math.round(cx.size[0] / cell2));
    const rows = p.rows || Math.max(1, Math.round(cx.size[1] / cell2));
    const fn = p.fn ?? (p.value ? "mean" : "count");
    const ramp = p.ramp ?? (p.value && fn !== "count" ? "linear" : "sqrt");
    const bins = cx.table(
      "bin2d",
      p.data,
      op9.bin2d({ x: p.x, y: p.y, columns, rows, extent: p.extent, value: p.value, fn }),
      op9.derive("shade", e14(shade(ramp, "d.value")))
    );
    const unit = p.unit || "rows";
    const g = p.gap ?? 0;
    const grow = g > 0 ? -g : 0.25;
    const tip = p.label ?? binTip(p.value, fn, unit, p.format, "at");
    return group13({
      key: "heatmap2d",
      scales: { heat: { type: "sequential", domain: { data: bins, field: "shade" }, range: "$sequential" } },
      children: [
        // Keyed by the grid: another grid crossfades as a whole layer (not cell by cell); the same
        // grid morphs its cells' colours.
        instances6({
          key: `cells-${columns}x${rows}`,
          from: bins,
          proto: "rect",
          x: e14(`min(scale.x(d.x0), scale.x(d.x1)) - ${grow / 2}`),
          y: e14(`min(scale.y(d.y0), scale.y(d.y1)) - ${grow / 2}`),
          w: e14(`max(0.5, abs(scale.x(d.x1) - scale.x(d.x0)) + ${grow})`),
          h: e14(`max(0.5, abs(scale.y(d.y1) - scale.y(d.y0)) + ${grow})`),
          fill: e14("scale.heat(d.shade)"),
          instanceKey: e14("d.cell"),
          label: tip,
          semantics: { role: "series", label: e14(`${q(p.name ?? p.data)} + ": " + format(table.count(${q(bins)}), ",") + " cells of ${columns} \xD7 ${rows}, up to " + format(table.max(${q(bins)}, "count"), ",") + ${q(` ${unit} each`)}`) }
        }),
        rampKey("heat", bins, keyTitle(p.value, fn, unit, "cell"), p.format, p.legend)
      ]
    });
  }
});
var contours = recipe14({
  id: "@datars/std/contours",
  doc: "Density contours: the shape of a crowd of rows \u2014 a smoothed density of their points cut into nested levels (by default each holding a further share of the rows: the innermost line encloses the densest fifth), drawn as filled bands or lines. Put it in a plot.",
  params: {
    data: t14.table("The rows (any number: the density is computed once per data, in the engine)."),
    x: t14.field("Field on the plot's x scale."),
    y: t14.field("Field on the plot's y scale."),
    levels: t14.number(4, 'How many levels: with `by: "share"` they enclose 1/(levels+1), 2/(levels+1)\u2026 of the rows.'),
    shares: t14.json("Exact shares of the rows the levels enclose, e.g. [0.9, 0.5, 0.25] (overrides `levels`)."),
    by: t14.oneOf(["share", "density"], "share", "Levels by the share of rows inside them, or evenly spaced in density up to the peak."),
    bandwidth: t14.number(14, "Smoothing, px on screen: the standard deviation of the Gaussian kernel (larger: rounder, fewer islands)."),
    cell: t14.number(4, "Grid resolution, px: the density is sampled on cells this size over the plot area."),
    style: t14.oneOf(["bands", "lines", "both"], "bands", "Filled nested bands (darker inside), contour lines, or both."),
    stroke: t14.prop("Line ink (default: the ramp, darker inside)."),
    opacity: t14.number(0.9, "Band opacity."),
    dots: t14.number(0, "Also draw a seeded sample of this many rows as faint dots over the bands (0: none) \u2014 the crowd itself."),
    extent: t14.json("The extent of the density grid, [x0, y0, x1, y1] (default: the rows' own, widened by `pad`)."),
    pad: t14.number(0.05, "How far the default extent reaches past the rows, as a share of their range (room for the outer line to close)."),
    label: t14.prop("Tooltip and accessible name per level: an expression over its row (`d.share`, `d.density`, `d.level`); default: the share of rows inside."),
    name: t14.string(void 0, "What the rows are, for the accessible description."),
    unit: t14.string("rows", "What one row is, in the plural, for the tooltips ('eruptions': '50% of the eruptions inside').")
  },
  tokens: ["sequential", "ink", "size.small"],
  expand(p, cx) {
    const cell2 = Math.max(1, p.cell || 4);
    const columns = Math.max(2, Math.round(cx.size[0] / cell2));
    const rows = Math.max(2, Math.round(cx.size[1] / cell2));
    const shares = Array.isArray(p.shares) ? p.shares : void 0;
    const n = shares?.length ?? Math.max(1, Math.round(p.levels ?? 4));
    const levels = cx.table("contours", p.data, op9.contours({
      x: p.x,
      y: p.y,
      columns,
      rows,
      extent: p.extent,
      pad: p.pad,
      bandwidth: Math.max(0.5, (p.bandwidth ?? 14) / cell2),
      levels: n,
      shares,
      by: p.by
    }));
    const paths = cx.table("contour-paths", levels, op9.paths({ by: "level", ring: "ring", closed: true, x: e14("scale.x(d.x)"), y: e14("scale.y(d.y)") }));
    const unit = p.unit || "rows";
    const tip = p.label ?? e14(`d.share >= 0.995 ? ${q(`Almost all the ${unit} inside`)} : format(d.share, ".0%") + ${q(` of the ${unit} inside`)}`);
    const bands = p.style !== "lines";
    const lines = p.style !== "bands";
    const lev = { type: "sequential", domain: [-0.6, Math.max(1, n - 1)], range: "$sequential" };
    const dots = p.dots > 0 ? cx.table("contour-dots", p.data, op9.sample(Math.round(p.dots), 7)) : null;
    return group13({
      key: "contours",
      clip: "box",
      semantics: { role: "series", label: `${p.name ?? p.data}: density contours, ${n} levels` },
      scales: { lev },
      children: [
        bands ? group13({ key: "bands", children: [repeat10(paths, shape13(geom13.path(e14("d.path")), {
          fill: e14("scale.lev(d.level)"),
          opacity: p.opacity,
          semantics: { role: "region", label: tip },
          pickable: true
        }))] }) : null,
        dots ? instances6({ key: "dots", from: dots, x: e14(`scale.x(d.${p.x})`), y: e14(`scale.y(d.${p.y})`), r: 1.1, fill: "$ink", opacity: 0.28, semantics: { role: "decoration" } }) : null,
        lines ? group13({ key: "lines", children: [repeat10(paths, shape13(geom13.path(e14("d.path")), {
          stroke: { paint: p.stroke ?? (bands ? "$paper" : e14("scale.lev(d.level)")), width: bands ? 0.8 : e14("1 + d.level * 0.35"), join: "round" },
          semantics: bands ? { role: "decoration" } : { role: "region", label: tip },
          pickable: !bands
        }))] }) : null
      ]
    });
  }
});
var manyLines = recipe14({
  id: "@datars/std/manyLines",
  doc: "Thousands of time series at once: every series a faint path, so their bulk shows where most of them run, with the ones that matter highlighted in the accent colour and labelled. Hover a line for its series. Put it in a plot.",
  params: {
    data: t14.table("The rows: one per series and x (in x order within each series)."),
    x: t14.field("Field on the plot's x scale (numbers or dates)."),
    y: t14.field("Field on the plot's y scale."),
    series: t14.field("The field naming each row's series: one path per value."),
    xType: t14.string("linear", "The plot's x scale type (a band scale puts points mid-band)."),
    stroke: t14.prop("Ink of the series that aren't highlighted (default $muted: grey, so the highlighted ones stand out)."),
    opacity: t14.number(0, "Their opacity (0: from how many there are \u2014 fainter the more there are, so where most of them run darkens instead of filling solid)."),
    width: t14.number(1, "Their stroke width, px."),
    highlight: t14.prop("The series to bring forward: a value of `series`, a list of them, or an expression (a signal a control or story step sets). None by default."),
    highlightStroke: t14.prop("Ink of the highlighted series (default $accent)."),
    highlightWidth: t14.number(2.5, "Stroke width of the highlighted series, px."),
    labels: t14.bool(true, "Name each highlighted series at its last point (inside the plot's right edge, pushed apart so names never overlap)."),
    hover: t14.bool(true, "The series under the pointer comes forward in the highlight colour while hovered (`hover()`; never on touch screens, where a tap names it)."),
    label: t14.prop("Tooltip and accessible name per series: an expression over its first row (default: the series' name)."),
    name: t14.string(void 0, "What the series are, for the accessible description ('3,000 weather stations').")
  },
  tokens: ["muted", "accent", "paper", "size.label"],
  expand(p, cx) {
    const s = p.series;
    const band = p.xType === "band" || p.xType === "point";
    const x = band ? `scale.x(d.${p.x}) + scale.x.bandwidth() / 2` : `scale.x(d.${p.x})`;
    const lines = cx.table("lines", p.data, op9.paths({ by: s, x: e14(x), y: e14(`scale.y(d.${p.y})`) }));
    const count = `table.count(${q(lines)})`;
    const isExprObj = (v) => typeof v === "object" && v !== null && "expr" in v;
    const opacity = p.opacity ? String(p.opacity) : `clamp(3 / sqrt(max(${count}, 1)), 0.04, 0.7)`;
    const inkSrc = (v) => isExprObj(v) ? `(${v.expr})` : q(v);
    const rest = p.stroke ?? "$muted";
    const accent = p.highlightStroke ?? "$accent";
    const tip = p.label ?? e14(`key.name(d.${s})`);
    const hl = p.highlight;
    const hlTest = hl === void 0 || hl === null ? null : Array.isArray(hl) ? hl.map((v) => `d.${s} == ${q(v)}`).join(" || ") || "false" : isExprObj(hl) ? `d.${s} == (${hl.expr})` : `d.${s} == ${q(hl)}`;
    const picked = hlTest ? cx.table("highlight", lines, op9.filter(e14(hlTest))) : null;
    const names = picked && p.labels ? cx.table("names", picked, op9.spread({ position: e14("d.end_y"), gap: e14('token("size.label") + 3'), min: 0, max: e14("box.h"), as: "label_y" })) : null;
    const path = (width, paint, extra) => shape13(geom13.path(e14("d.path")), { stroke: { paint, width, join: "round", cap: "round" }, pickable: true, semantics: { role: "series", label: tip }, ...extra });
    return group13({
      key: "many-lines",
      semantics: { role: "group", label: e14(`${q(p.name ?? p.data)} + ": " + format(${count}, ",") + " series"`) },
      children: [
        // Hovered (with a mouse), a series takes the highlight's ink, width and full opacity.
        group13({ key: "all", children: [repeat10(lines, p.hover ? path(e14(`hover() ? ${p.highlightWidth || 2.5} : ${p.width || 1}`), e14(`hover() ? ${inkSrc(accent)} : ${inkSrc(rest)}`), { opacity: e14(`hover() ? 1 : ${opacity}`) }) : path(p.width || 1, rest, { opacity: e14(opacity) }))] }),
        picked ? group13({ key: "highlight", children: [repeat10(picked, path(p.highlightWidth || 2.5, accent, {}))] }) : null,
        names ? group13({ key: "labels", children: [repeat10(names, text12(e14(`key.name(d.${s})`), [e14("d.end_x - 6"), e14("d.label_y")], {
          style: { size: "$size.label", weight: 600, ink: accent, align: "end", baseline: "middle", contain: true },
          halo: ["$paper", 3]
        }))] }) : null
      ]
    });
  }
});

// src/networks.ts
import { e as e15, group as group14, op as op10, recipe as recipe15, repeat as repeat11, shape as shape14, geom as geom14, t as t15, text as text13 } from "@datars/sdk";
var J = JSON.stringify;
var col = (name) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? `d.${name}` : `d[${J(name)}]`;
function graphNames(p) {
  const id = p.id ?? "id";
  return { id, source: p.source ?? "source", target: p.target ?? "target", label: p.label ? col(p.label) : `key.name(${col(id)})` };
}
function selectedOpacity(sel, hit, on, off) {
  return sel ? e15(`${sel}.isEmpty() || (${hit}) ? ${on} : ${off}`) : on;
}
function orderOps(order) {
  if (order === "input") return [];
  if (order === "cluster") return [op10.sort("order")];
  if (order === "degree") return [op10.sort(["degree", "desc"], "order")];
  return [op10.sort(order, "order")];
}
var network = recipe15({
  id: "@datars/std/network",
  doc: "A node-link diagram laid out by a deterministic force simulation \u2014 seeded starts, a fixed number of steps, computed once per data and size, never per frame: links pull, nodes repel, never overlap and stay inside the box. Nodes sized by a field (default: their links), the largest named; nodes and links keyed, so a data change moves them.",
  params: {
    nodes: t15.table("One row per node."),
    links: t15.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t15.field("The nodes' id column, which links' source and target name (default `id`)."),
    source: t15.field("The links' source column (default `source`)."),
    target: t15.field("The links' target column (default `target`)."),
    weight: t15.field("Link weight: heavier links are thicker and pull harder (default: all equal)."),
    size: t15.field("Node size field (default `degree`: how many links a node has, or their summed weight)."),
    color: t15.field("Colour nodes by this field (categorical); `community` colours the clusters found in the links. Default: one colour."),
    label: t15.field("Node name column (default: the id, through the document's key names)."),
    minRadius: t15.number(3, "Radius of the smallest node (px)."),
    maxRadius: t15.number(14, "Radius of the largest node (px); areas between are \u221D size."),
    distance: t15.number(0, "Link rest length in px (0: from the box and the number of nodes)."),
    charge: t15.number(0, "Many-body strength: negative repels, positive attracts (0: from the link length)."),
    iterations: t15.number(300, "Simulation steps \u2014 a fixed number, so the layout is the same on every device."),
    seed: t15.number(1, "Seed of the starting positions: another seed, another (equally valid) layout."),
    labels: t15.number(8, "How many nodes carry a name label, largest first (0: none; every node names itself on hover). Labels that would collide are left out."),
    selected: t15.string(void 0, "A keyset signal of node ids: those nodes and their links stay strong, the rest recede."),
    format: t15.string(",.0f", "Number format of sizes and weights in tooltips.")
  },
  tokens: ["mark", "categorical", "paper", "rule", "grid", "ink", "muted", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op10.communities(links));
    const S = J(stats);
    const size = p.size ?? "degree";
    const count = `max(1, table.count(${S}))`;
    const radius = `${p.minRadius} + ${Math.max(0, p.maxRadius - p.minRadius)} * sqrt(max(0, ${col(size)}) / max(1e-9, table.max(${S}, ${J(size)})))`;
    const dist = p.distance ? String(p.distance) : `clamp(sqrt(box.w * box.h / ${count}) * 0.45, 12, 100)`;
    const charge = p.charge ? String(p.charge) : `-(${dist}) * (${dist}) / 20`;
    const nodes = cx.table(
      "nodes",
      stats,
      op10.derive("r", e15(radius)),
      op10.force({ ...links, radius: e15("d.r"), width: e15("box.w"), height: e15("box.h"), distance: e15(dist), charge: e15(charge), iterations: p.iterations, seed: p.seed }),
      op10.window("rank", size, "rank", { order: `-${size}` })
    );
    const ends = cx.table(
      "links",
      p.links,
      op10.lookup({ from: nodes, key: g.id, field: g.source, values: ["x", "y"], as: ["x1", "y1"] }),
      op10.lookup({ from: nodes, key: g.id, field: g.target, values: ["x", "y"], as: ["x2", "y2"] }),
      op10.filter(e15("isFinite(d.x1) && isFinite(d.x2)"))
    );
    const named = cx.table("named", nodes, op10.filter(e15(`d.rank <= ${Math.max(0, Math.floor(p.labels))}`)), op10.sort("rank"));
    const sel = p.selected;
    const strong = sel ? `(${sel}.isEmpty() || ${sel}.has(${col(g.id)}))` : "true";
    const colour = p.color ? `scale.color(${col(p.color)})` : `"$mark"`;
    const fill = sel ? e15(`${strong} ? ${colour} : "$grid"`) : p.color ? e15(colour) : "$mark";
    const w = p.weight ? `1 + 2.2 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))` : "1.2";
    const f = J(p.format);
    const nodeLabel = p.size ? `\`\${${g.label}}: \${format(${col(size)}, ${f})}\`` : p.weight ? `\`\${${g.label}}: \${format(d.degree, ${f})} ${p.weight}\`` : `\`\${${g.label}}: \${d.degree} link\${d.degree == 1 ? "" : "s"}\``;
    const linkLabel = `\`\${key.name(${col(g.source)})} \u2013 \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``;
    return group14({
      key: "network",
      scales: p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : void 0,
      semantics: { role: "group", label: "Network" },
      children: [
        group14({ key: "links", children: [repeat11(ends, shape14(geom14.segment({ x1: e15("d.x1"), y1: e15("d.y1"), x2: e15("d.x2"), y2: e15("d.y2") }), {
          key: [e15(col(g.source)), e15(col(g.target))],
          stroke: { paint: sel ? e15(`!${sel}.isEmpty() && (${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})) ? "$ink-2" : "$rule"`) : "$rule", width: e15(w), cap: "round" },
          opacity: selectedOpacity(sel, `${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})`, 0.6, 0.12),
          semantics: { role: "datum", label: e15(linkLabel) },
          pickable: true
        }))] }),
        group14({ key: "nodes", children: [repeat11(nodes, shape14(geom14.circle({ cx: e15("d.x"), cy: e15("d.y"), r: e15("d.r") }), {
          key: e15(col(g.id)),
          fill,
          stroke: { paint: "$paper", width: 1.5 },
          semantics: { role: "datum", label: e15(nodeLabel), value: e15(col(size)) },
          pickable: true
        }))] }),
        p.labels > 0 ? group14({ key: "labels", declutter: true, children: [repeat11(named, text13(e15(g.label), [e15("d.x + d.r + 3"), e15("d.y")], {
          key: e15(col(g.id)),
          style: { size: "$size.label", ink: sel ? e15(`${strong} ? "$ink" : "$muted"`) : "$ink", baseline: "middle", contain: true },
          halo: ["$paper", 3]
        }))] }) : null
      ]
    });
  }
});
var arcDiagram = recipe15({
  id: "@datars/std/arcDiagram",
  doc: "Nodes on a line and each link as an arc above it, as tall as it is long. Ordered so clusters sit together (or by degree, a field, or input order); names under the line, turned when they crowd. Links between nodes of one colour take it.",
  params: {
    nodes: t15.table("One row per node."),
    links: t15.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t15.field("The nodes' id column (default `id`)."),
    source: t15.field("The links' source column (default `source`)."),
    target: t15.field("The links' target column (default `target`)."),
    weight: t15.field("Link weight: heavier links are thicker (default: all equal)."),
    size: t15.field("Node size field (default `degree`, the node's links)."),
    color: t15.field("Colour nodes (and the arcs within one colour) by this field; `community` colours clusters. Default: one colour."),
    label: t15.field("Node name column (default: the id)."),
    order: t15.string("cluster", "Order along the line: `cluster` (communities together, best connected first), `degree`, `input`, or a column of the nodes table."),
    maxRadius: t15.number(7, "Radius of the largest node (px)."),
    labels: t15.bool(true, "Node names under the line (turned upright when they don't fit across)."),
    selected: t15.string(void 0, "A keyset signal of node ids: their arcs stay strong, the rest recede."),
    format: t15.string(",.0f", "Number format of sizes and weights in tooltips.")
  },
  tokens: ["mark", "categorical", "paper", "rule", "ink-2", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op10.communities(links));
    const nodes = cx.table("nodes", stats, ...orderOps(p.order ?? "cluster"), op10.derive("lw", e15(`measure(${g.label}, token("size.label"))`)));
    const N = J(nodes);
    const size = p.size ?? "degree";
    const withColor = p.color ? [op10.lookup({ from: stats, key: g.id, field: g.source, values: [p.color], as: ["c1"] }), op10.lookup({ from: stats, key: g.id, field: g.target, values: [p.color], as: ["c2"] })] : [];
    const arcs = cx.table("arcs", p.links, ...withColor, op10.filter(e15(`scale.nx(${col(g.source)}) != null && scale.nx(${col(g.target)}) != null`)));
    const R = Math.max(1, p.maxRadius);
    const turned = `(scale.nx.step() < table.max(${N}, "lw") + 6)`;
    const below = p.labels ? `(${turned} ? table.max(${N}, "lw") + 6 : token("size.label") + 6)` : "0";
    const half = "max(1, (scale.nx.max() - scale.nx.min()) / 2)";
    const under = `(${below} + ${R + 3})`;
    const base = `(box.h - ${under} - max(0, (box.h - ${under} - ${half} - ${R + 2}) / 2))`;
    const k = `min(1, (${base} - ${R + 2}) / ${half})`;
    const x1 = `scale.nx(${col(g.source)})`, x2 = `scale.nx(${col(g.target)})`;
    const c = `(abs(${x2} - ${x1}) / 2 * ${k} * 4 / 3)`;
    const path = `\`M \${${x1}} \${${base}} C \${${x1}} \${${base} - ${c}} \${${x2}} \${${base} - ${c}} \${${x2}} \${${base}}\``;
    const r = `(2.5 + ${R - 2.5} * sqrt(max(0, ${col(size)}) / max(1e-9, table.max(${N}, ${J(size)}))))`;
    const nodeFill = p.color ? e15(`scale.color(${col(p.color)})`) : "$mark";
    const arcInk = p.color ? e15(`d.c1 == d.c2 ? scale.color(d.c1) : "$rule"`) : "$mark";
    const w = p.weight ? `1 + 2.5 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))` : "1.4";
    const f = J(p.format);
    const sel = p.selected;
    return group14({
      key: "arc-diagram",
      scales: {
        nx: { type: "point", domain: { data: nodes, field: g.id }, range: "width", padding: 1 },
        ...p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : {}
      },
      semantics: { role: "group", label: "Arc diagram" },
      children: [
        group14({ key: "arcs", children: [repeat11(arcs, shape14(geom14.path(e15(path)), {
          key: [e15(col(g.source)), e15(col(g.target))],
          stroke: { paint: arcInk, width: e15(w), cap: "round" },
          opacity: selectedOpacity(sel, `${sel}.has(${col(g.source)}) || ${sel}.has(${col(g.target)})`, 0.55, 0.1),
          semantics: { role: "datum", label: e15(`\`\${key.name(${col(g.source)})} \u2013 \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``) },
          pickable: true
        }))] }),
        group14({ key: "nodes", children: [repeat11(nodes, shape14(geom14.circle({ cx: e15(`scale.nx(${col(g.id)})`), cy: e15(base), r: e15(r) }), {
          key: e15(col(g.id)),
          fill: nodeFill,
          stroke: { paint: "$paper", width: 1.5 },
          semantics: { role: "datum", label: e15(`\`\${${g.label}}: \${format(${col(size)}, ${f})}${p.size ? "" : " links"}\``), value: e15(col(size)) },
          pickable: true
        }))] }),
        p.labels ? group14({ key: "labels", children: [repeat11(nodes, text13(e15(g.label), [e15(`scale.nx(${col(g.id)})`), e15(`${base} + ${R + 5}`)], {
          key: e15(col(g.id)),
          rotate: e15(`${turned} ? -90 : 0`),
          style: { size: "$size.label", ink: "$ink-2", align: e15(`${turned} ? "end" : "middle"`), baseline: e15(`${turned} ? "middle" : "top"`), contain: true }
        }))] }) : null
      ]
    });
  }
});
var chord = recipe15({
  id: "@datars/std/chord",
  doc: "A chord diagram: groups around a circle, each as long as the flows touching it, and a ribbon per flow between its two groups, as wide as the flow at both ends. Ribbons sharing a group never cross; each takes its source's colour.",
  params: {
    data: t15.table("One row per flow: source, target and value."),
    source: t15.field("The flows' source column (default `source`)."),
    target: t15.field("The flows' target column (default `target`)."),
    value: t15.field("The flows' size (default: each flow counts 1)."),
    pad: t15.number(0.04, "Gap between groups (radians)."),
    thickness: t15.number(0.07, "The group arcs' thickness, as a share of the radius."),
    labels: t15.bool(true, "Group names outside the circle, where their arc has room."),
    format: t15.string(",.0f", "Number format of values in tooltips."),
    selected: t15.string(void 0, "A keyset signal of group names: flows touching them stay strong, the rest recede.")
  },
  tokens: ["categorical", "paper", "ink-2", "size.label"],
  expand(p, cx) {
    const src = p.source ?? "source", dst = p.target ?? "target";
    const common = { source: src, target: dst, value: p.value, pad: p.pad };
    const groups = cx.table("groups", p.data, op10.chordGroups(common), op10.derive("lw", e15(`measure(key.name(d.name), token("size.label"))`)));
    const G = J(groups);
    const outer = p.labels ? `max(12, min(box.h / 2 - token("size.label") - 4, box.w / 2 - table.max(${G}, "lw") - 16))` : "max(12, min(box.w, box.h) / 2 - 4)";
    const inner = `(${outer}) * ${1 - p.thickness}`;
    const ribbons = cx.table("ribbons", p.data, op10.chordRibbons({ ...common, cx: e15("box.w / 2"), cy: e15("box.h / 2"), r: e15(`${inner} - 1.5`) }));
    const mid = "(d.a0 + d.a1) / 2";
    const f = J(p.format);
    const sel = p.selected;
    const v = p.value ? col(p.value) : "1";
    return group14({
      key: "chord",
      scales: { color: { type: "categorical", domain: { data: groups, field: "name" }, range: "$categorical" } },
      semantics: { role: "group", label: "Chord diagram" },
      children: [
        group14({ key: "ribbons", children: [repeat11(ribbons, shape14(geom14.path(e15("d.path")), {
          key: [e15(col(src)), e15(col(dst))],
          fill: e15(`scale.color(${col(src)})`),
          opacity: selectedOpacity(sel, `${sel}.has(${col(src)}) || ${sel}.has(${col(dst)})`, 0.68, 0.12),
          stroke: { paint: "$paper", width: 0.5 },
          semantics: { role: "datum", label: e15(`\`\${key.name(${col(src)})} \u2192 \${key.name(${col(dst)})}: \${format(${v}, ${f})}\``), value: e15(v) },
          pickable: true
        }))] }),
        group14({ key: "groups", children: [repeat11(groups, shape14(geom14.arc({ cx: e15("box.w / 2"), cy: e15("box.h / 2"), r0: e15(inner), r1: e15(outer), a0: e15("d.a0"), a1: e15("d.a1") }), {
          key: e15("d.name"),
          fill: e15("scale.color(d.name)"),
          semantics: { role: "datum", label: e15(`\`\${key.name(d.name)}: \${format(d.value, ${f})}\``), value: e15("d.value") },
          pickable: true
        }))] }),
        p.labels ? group14({ key: "labels", declutter: true, children: [repeat11(groups, text13(e15("key.name(d.name)"), [e15(`box.w / 2 + (${outer} + 6) * sin(${mid})`), e15(`box.h / 2 - (${outer} + 6) * cos(${mid})`)], {
          key: e15("d.name"),
          when: e15(`(d.a1 - d.a0) * (${outer}) >= token("size.label") * 0.8`),
          style: { size: "$size.label", ink: "$ink-2", align: e15(`sin(${mid}) >= 0 ? "start" : "end"`), baseline: e15(`abs(cos(${mid})) > 0.94 ? (cos(${mid}) > 0 ? "alphabetic" : "hanging") : "middle"`) }
        }))] }) : null
      ]
    });
  }
});
var matrix = recipe15({
  id: "@datars/std/matrix",
  doc: "An adjacency matrix: a row and a column per node, a cell where two are linked \u2014 shaded by weight, coloured where both ends share a colour. Ordered so clusters show as blocks along the diagonal (or by degree, a field, or input order).",
  params: {
    nodes: t15.table("One row per node."),
    links: t15.table("One row per link: the ids of its two nodes (and optionally a weight)."),
    id: t15.field("The nodes' id column (default `id`)."),
    source: t15.field("The links' source column (default `source`)."),
    target: t15.field("The links' target column (default `target`)."),
    weight: t15.field("Link weight: heavier links are darker cells (default: all equal)."),
    color: t15.field("Colour cells whose two nodes share this field's value by it (`community`: the clusters); the others stay neutral. Default: one colour."),
    label: t15.field("Node name column (default: the id)."),
    order: t15.string("cluster", "Order of the rows and columns: `cluster` (communities together, best connected first), `degree`, `input`, or a column of the nodes table."),
    directed: t15.bool(false, "Links go one way: a link fills only its source's row. Otherwise each fills both cells, so the matrix is symmetric."),
    labels: t15.bool(true, "Node names left of the rows and above the columns, where the rows are tall enough."),
    format: t15.string(",.0f", "Number format of weights in tooltips.")
  },
  tokens: ["mark", "categorical", "muted", "surface", "ink-2", "size.label"],
  expand(p, cx) {
    const g = graphNames(p);
    const links = { links: p.links, source: g.source, target: g.target, id: g.id, weight: p.weight };
    const stats = cx.table("stats", p.nodes, op10.communities(links));
    const nodes = cx.table("nodes", stats, ...orderOps(p.order ?? "cluster"), op10.derive("lw", e15(`measure(${g.label}, token("size.label"))`)));
    const N = J(nodes);
    const withColor = p.color ? [op10.lookup({ from: stats, key: g.id, field: g.source, values: [p.color], as: ["c1"] }), op10.lookup({ from: stats, key: g.id, field: g.target, values: [p.color], as: ["c2"] })] : [];
    const cells = cx.table("cells", p.links, ...withColor, op10.filter(e15(`scale.m(${col(g.source)}) != null && scale.m(${col(g.target)}) != null`)));
    const gutter = p.labels ? `(box.h / max(1, table.count(${N})) >= 7 ? min(table.max(${N}, "lw"), box.w * 0.3) + 6 : 0)` : "0";
    const side = `max(0, min(box.w, box.h) - ${gutter})`;
    const x0 = `((box.w - ${side} - ${gutter}) / 2 + ${gutter})`;
    const fill = p.color ? e15(`d.c1 == d.c2 ? scale.color(d.c1) : "$muted"`) : "$mark";
    const shade2 = p.weight ? e15(`0.3 + 0.7 * sqrt(max(0, ${col(p.weight)}) / max(1e-9, table.max(${J(p.links)}, ${J(p.weight)})))`) : 1;
    const f = J(p.format);
    const label = `\`\${key.name(${col(g.source)})} \u2013 \${key.name(${col(g.target)})}${p.weight ? `: \${format(${col(p.weight)}, ${f})}` : ""}\``;
    const cell2 = (row, column, key) => shape14(geom14.rect({ x: e15(`scale.m(${column})`), y: e15(`scale.m(${row})`), w: e15("scale.m.bandwidth()"), h: e15("scale.m.bandwidth()"), r: e15("min(2, scale.m.bandwidth() / 5)") }), {
      key,
      fill,
      opacity: shade2,
      semantics: { role: "datum", label: e15(label), value: p.weight ? e15(col(p.weight)) : void 0 },
      pickable: true
    });
    const size = `min(token("size.label"), scale.m.bandwidth() * 0.92)`;
    const named = `scale.m.bandwidth() >= 6`;
    return group14({
      key: "matrix",
      transform: { translate: [e15(x0), e15(gutter)] },
      scales: {
        m: { type: "band", domain: { data: nodes, field: g.id }, range: [0, `=${side}`], padding: 0.08 },
        ...p.color ? { color: { type: "categorical", domain: { data: stats, field: p.color }, range: "$categorical" } } : {}
      },
      semantics: { role: "group", label: "Adjacency matrix" },
      children: [
        shape14(geom14.rect({ x: 0, y: 0, w: e15(side), h: e15(side), r: 3 }), { key: "ground", fill: "$surface", semantics: { role: "decoration" } }),
        group14({ key: "cells", children: [repeat11(cells, cell2(col(g.source), col(g.target), [e15(col(g.source)), e15(col(g.target))]))] }),
        p.directed ? null : group14({ key: "mirror", children: [repeat11(cells, cell2(col(g.target), col(g.source), [e15(col(g.target)), e15(col(g.source))]))] }),
        p.labels ? group14({ key: "rows", children: [repeat11(nodes, text13(e15(g.label), [-5, e15(`scale.m(${col(g.id)}) + scale.m.bandwidth() / 2`)], {
          key: e15(col(g.id)),
          when: e15(named),
          style: { size: e15(size), ink: "$ink-2", align: "end", baseline: "middle", maxWidth: e15(`${gutter} - 6`) }
        }))] }) : null,
        p.labels ? group14({ key: "columns", children: [repeat11(nodes, text13(e15(g.label), [e15(`scale.m(${col(g.id)}) + scale.m.bandwidth() / 2`), -5], {
          key: e15(col(g.id)),
          when: e15(named),
          rotate: -90,
          style: { size: e15(size), ink: "$ink-2", align: "start", baseline: "middle", maxWidth: e15(`${gutter} - 6`) }
        }))] }) : null
      ]
    });
  }
});
function treeParams(link) {
  return {
    data: t15.table("One row per node: its id and its parent's id (none for the root)."),
    id: t15.field("The nodes' id column (default `id`)."),
    parent: t15.field("The column holding each node's parent id (default `parent`); empty for the root."),
    label: t15.field("Node name column (default: the id)."),
    orientation: t15.oneOf(["horizontal", "vertical", "radial"], "horizontal", "Root at the left (names read across), at the top, or in the middle with the leaves around it."),
    color: t15.field("Colour nodes by this field (categorical). Default: branches dark, leaves light."),
    link: t15.oneOf(["curve", "elbow", "line"], link, "Links as smooth curves, right-angled elbows or straight lines."),
    r: t15.number(3.5, "Node radius (px)."),
    labels: t15.bool(true, "Node names: leaves outside, branches beside their node.")
  };
}
function hierarchyTree(p, cx, method) {
  const id = p.id ?? "id", parent = p.parent ?? "parent";
  const label = p.label ? col(p.label) : `key.name(${col(id)})`;
  const orient = p.orientation ?? "horizontal";
  const R = Math.max(0, p.r);
  const probe = cx.table(
    "probe",
    p.data,
    op10.tree({ id, parent, method, width: 1, height: 1 }),
    op10.derive("lw", e15(`measure(${label}, token("size.label"))`)),
    op10.derive("leafw", e15("d.leaf ? d.lw : 0")),
    op10.derive("rootw", e15("d.depth == 0 ? d.lw : 0")),
    op10.derive("isleaf", e15("d.leaf ? 1 : 0"))
  );
  const P = J(probe);
  const leafW = p.labels ? `table.max(${P}, "leafw")` : "0", rootW = p.labels ? `table.max(${P}, "rootw")` : "0";
  const gap = R + 4;
  const fs = `token("size.label")`;
  let width, height, X, Y, PX, PY;
  const crowded = `(box.w / max(1, table.sum(${P}, "isleaf")) < ${leafW} + 6)`;
  if (orient === "vertical") {
    const top = p.labels ? `(max(${R}, ${fs} / 2) + 3)` : `${R + 2}`;
    const bottom = p.labels ? `(${crowded} ? ${leafW} + ${gap} + 2 : ${fs} + ${gap} + 2)` : `${R + 2}`;
    width = `box.w - ${2 * R + 12}`;
    height = `box.h - ${top} - ${bottom}`;
    X = `(${R + 6} + d.x)`;
    Y = `(${top} + d.y)`;
    PX = `(${R + 6} + d.px)`;
    PY = `(${top} + d.py)`;
  } else if (orient === "radial") {
    const rad = `max(10, min(box.w, box.h) / 2 - ${leafW} - ${gap} - 2)`;
    width = String(2 * Math.PI);
    height = rad;
    X = "(box.w / 2 + d.y * sin(d.x))";
    Y = "(box.h / 2 - d.y * cos(d.x))";
    PX = "(box.w / 2 + d.py * sin(d.px))";
    PY = "(box.h / 2 - d.py * cos(d.px))";
  } else {
    const left = p.labels ? `(${rootW} + ${gap} + 2)` : `${R + 2}`;
    const right = p.labels ? `(${leafW} + ${gap} + 2)` : `${R + 2}`;
    const pad = `(${fs} / 2 + 2)`;
    width = `box.h - 2 * ${pad}`;
    height = `box.w - ${left} - ${right}`;
    X = `(${left} + d.y)`;
    Y = `(${pad} + d.x)`;
    PX = `(${left} + d.py)`;
    PY = `(${pad} + d.px)`;
  }
  const laid = cx.table("tree", p.data, op10.tree({ id, parent, label: p.label, method, width: e15(width), height: e15(height) }));
  const linksT = cx.table("links", laid, op10.filter(e15("isFinite(d.px)")));
  const link = p.link ?? (method === "cluster" ? "elbow" : "curve");
  const polar = (a, r) => [`box.w / 2 + (${r}) * sin(${a})`, `box.h / 2 - (${r}) * cos(${a})`];
  let path;
  if (link === "line") {
    path = `\`M \${${PX}} \${${PY}} L \${${X}} \${${Y}}\``;
  } else if (orient === "radial") {
    if (link === "curve") {
      const m = "(d.py + d.y) / 2";
      const [c1x, c1y] = polar("d.px", m), [c2x, c2y] = polar("d.x", m);
      path = `\`M \${${PX}} \${${PY}} C \${${c1x}} \${${c1y}} \${${c2x}} \${${c2y}} \${${X}} \${${Y}}\``;
    } else {
      const k = "(4 / 3 * tan((d.x - d.px) / 4) * d.py)";
      const [qx, qy] = polar("d.x", "d.py");
      path = `\`M \${${PX}} \${${PY}} C \${${PX} + ${k} * cos(d.px)} \${${PY} + ${k} * sin(d.px)} \${${qx} - ${k} * cos(d.x)} \${${qy} - ${k} * sin(d.x)} \${${qx}} \${${qy}} L \${${X}} \${${Y}}\``;
    }
  } else if (orient === "vertical") {
    path = link === "curve" ? `\`M \${${PX}} \${${PY}} C \${${PX}} \${(${PY} + ${Y}) / 2} \${${X}} \${(${PY} + ${Y}) / 2} \${${X}} \${${Y}}\`` : `\`M \${${PX}} \${${PY}} H \${${X}} V \${${Y}}\``;
  } else {
    path = link === "curve" ? `\`M \${${PX}} \${${PY}} C \${(${PX} + ${X}) / 2} \${${PY}} \${(${PX} + ${X}) / 2} \${${Y}} \${${X}} \${${Y}}\`` : `\`M \${${PX}} \${${PY}} V \${${Y}} H \${${X}}\``;
  }
  let leafAt, leafRotate, leafAlign, leafBaseline;
  let branchAt, branchAlign, branchBaseline;
  if (orient === "radial") {
    const rr = `(d.y + ${gap})`;
    leafAt = [e15(`box.w / 2 + ${rr} * sin(d.x)`), e15(`box.h / 2 - ${rr} * cos(d.x)`)];
    leafRotate = e15(`d.x * ${180 / Math.PI} + (d.x < ${Math.PI} ? -90 : 90)`);
    leafAlign = e15(`d.x < ${Math.PI} ? "start" : "end"`);
    leafBaseline = "middle";
    branchAt = [e15(X), e15(`${Y} - ${gap}`)];
    branchAlign = "middle";
    branchBaseline = "alphabetic";
  } else if (orient === "vertical") {
    leafAt = [e15(X), e15(`${Y} + ${gap}`)];
    leafRotate = e15(`${crowded} ? 90 : 0`);
    leafAlign = e15(`${crowded} ? "start" : "middle"`);
    leafBaseline = e15(`${crowded} ? "middle" : "top"`);
    branchAt = [e15(`${X} + ${gap}`), e15(Y)];
    branchAlign = "start";
    branchBaseline = "middle";
  } else {
    leafAt = [e15(`${X} + ${gap}`), e15(Y)];
    leafRotate = 0;
    leafAlign = "start";
    leafBaseline = "middle";
    branchAt = [e15(`${X} - ${gap}`), e15(Y)];
    branchAlign = "end";
    branchBaseline = "middle";
  }
  return group14({
    key: method === "cluster" ? "dendrogram" : "tree",
    scales: p.color ? { color: { type: "categorical", domain: { data: p.data, field: p.color }, range: "$categorical" } } : void 0,
    semantics: { role: "group", label: method === "cluster" ? "Dendrogram" : "Tree" },
    children: [
      group14({ key: "links", children: [repeat11(linksT, shape14(geom14.path(e15(path)), {
        key: e15(col(id)),
        stroke: { paint: "$rule", width: 1.2, join: "round" },
        opacity: 0.8,
        semantics: { role: "decoration" }
      }))] }),
      group14({ key: "nodes", children: [repeat11(laid, shape14(geom14.circle({ cx: e15(X), cy: e15(Y), r: R }), {
        key: e15(col(id)),
        fill: p.color ? e15(`scale.color(${col(p.color)})`) : e15(`d.leaf ? "$muted" : "$ink-2"`),
        stroke: { paint: "$paper", width: 1 },
        semantics: { role: "datum", label: e15("d.path") },
        pickable: true
      }))] }),
      p.labels ? group14({ key: "leaf-labels", children: [repeat11(laid, text13(e15(label), leafAt, {
        key: e15(col(id)),
        when: e15("d.leaf"),
        rotate: leafRotate,
        style: { size: "$size.label", ink: "$ink-2", align: leafAlign, baseline: leafBaseline, contain: true }
      }))] }) : null,
      p.labels ? group14({ key: "branch-labels", declutter: orient === "radial", children: [repeat11(laid, text13(e15(label), branchAt, {
        key: e15(col(id)),
        when: e15("!d.leaf"),
        style: { size: "$size.label", ink: "$ink", align: branchAlign, baseline: branchBaseline, contain: true },
        halo: ["$paper", 3]
      }))] }) : null
    ]
  });
}
var tree = recipe15({
  id: "@datars/std/tree",
  doc: "A tidy tree (Reingold\u2013Tilford) of a parent-child table: parents centred over their children, subtrees packed as close as they go, identical subtrees drawn alike. Horizontal, vertical or radial; nodes keyed by id, so the tree re-lays itself when rows change.",
  params: treeParams("curve"),
  tokens: ["rule", "ink", "ink-2", "muted", "paper", "size.label"],
  expand(p, cx) {
    return hierarchyTree(p, cx, "tidy");
  }
});
var dendrogram = recipe15({
  id: "@datars/std/dendrogram",
  doc: "A dendrogram (cluster layout) of a parent-child table: every leaf on one line, each parent centred over its children at its height, joined by elbows. Horizontal, vertical or radial; nodes keyed by id.",
  params: treeParams("elbow"),
  tokens: ["rule", "ink", "ink-2", "muted", "paper", "size.label"],
  expand(p, cx) {
    return hierarchyTree(p, cx, "cluster");
  }
});
function partitionParams() {
  return {
    data: t15.table("One row per node: its id and its parent's id (none for the root)."),
    id: t15.field("The nodes' id column (default `id`)."),
    parent: t15.field("The column holding each node's parent id (default `parent`); empty for the root."),
    value: t15.field("Leaf size (a parent is the sum of its leaves; values on parent rows are ignored). Default: every leaf counts 1."),
    label: t15.field("Node name column (default: the id)."),
    color: t15.field("Colour by this field (default `branch`: each top-level branch its own colour, lighter deeper down)."),
    sort: t15.bool(false, "Siblings largest first. Off, they keep row order \u2014 so when the values change, every piece grows or shrinks in place instead of changing places."),
    labels: t15.bool(true, "Names inside the pieces that have room for them."),
    format: t15.string(",.0f", "Number format of values in labels and tooltips.")
  };
}
function partitionInk(p, cx, laid, depth0) {
  const by = p.color ?? "branch";
  const drawn = cx.table("coloured", laid, op10.filter(e15(`d.depth >= ${depth0}`)));
  const fade = `max(0.35, 1 - 0.2 * (d.depth - ${depth0}))`;
  return {
    fill: e15(`scale.color(${col(by)})`),
    opacity: e15(fade),
    ink: e15(`${fade} > 0.75 ? "on(" + scale.color(${col(by)}) + ")" : "$ink"`),
    scale: { type: "categorical", domain: { data: drawn, field: by }, range: "$categorical" }
  };
}
var partitionLabel = (f) => e15(`\`\${d.path}: \${format(d.sum, ${f})} (\${format(d.share, ".0%")})\``);
var sunburst = recipe15({
  id: "@datars/std/sunburst",
  doc: "A sunburst: a hierarchy as rings, the root in the middle and each node an arc spanning its share of its parent. A single root becomes the hole, with the total in it; names follow the arcs where they fit.",
  params: { ...partitionParams(), total: t15.bool(true, "The root's name and total in the middle (with a single root).") },
  tokens: ["categorical", "paper", "ink", "ink-2", "size.label", "size.title", "font.title"],
  expand(p, cx) {
    const id = p.id ?? "id", parent = p.parent ?? "parent";
    const label = p.label ? col(p.label) : `key.name(${col(id)})`;
    const R = "(min(box.w, box.h) / 2 - 2)";
    const laid = cx.table(
      "partition",
      p.data,
      op10.partition({ id, parent, label: p.label, value: p.value, width: 2 * Math.PI, height: e15(R), sort: p.sort }),
      op10.derive("lw", e15(`measure(${label}, token("size.label"))`)),
      op10.derive("isroot", e15("d.depth == 0 ? 1 : 0"))
    );
    const L = J(laid);
    const single = `table.sum(${L}, "isroot") == 1`;
    const depth0 = `(${single} ? 1 : 0)`;
    const ink = partitionInk(p, cx, laid, depth0);
    const f = J(p.format);
    const a = "((d.x0 + d.x1) / 2)", rm = "((d.y0 + d.y1) / 2)";
    const along = `(d.lw + 8 <= (d.x1 - d.x0) * ${rm} && token("size.label") + 4 <= d.y1 - d.y0)`;
    const across = `(d.lw + 8 <= d.y1 - d.y0 && token("size.label") + 2 <= (d.x1 - d.x0) * ${rm})`;
    const deg = `${a} * ${180 / Math.PI}`;
    const rot = `${along} ? ${deg} - (${a} > ${Math.PI / 2} && ${a} < ${1.5 * Math.PI} ? 180 : 0) : ${deg} + (${a} < ${Math.PI} ? -90 : 90)`;
    return group14({
      key: "sunburst",
      scales: { color: ink.scale },
      semantics: { role: "group", label: "Sunburst" },
      children: [
        group14({ key: "arcs", children: [repeat11(laid, shape14(geom14.arc({ cx: e15("box.w / 2"), cy: e15("box.h / 2"), r0: e15("d.y0"), r1: e15("d.y1"), a0: e15("d.x0"), a1: e15("d.x1") }), {
          key: e15(col(id)),
          when: e15(`d.depth >= ${depth0}`),
          fill: ink.fill,
          opacity: ink.opacity,
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: partitionLabel(f), value: e15("d.sum") },
          pickable: true
        }))] }),
        p.labels ? group14({ key: "labels", children: [repeat11(laid, text13(e15(label), [e15(`box.w / 2 + ${rm} * sin(${a})`), e15(`box.h / 2 - ${rm} * cos(${a})`)], {
          key: e15(col(id)),
          when: e15(`d.depth >= ${depth0} && (${along} || ${across})`),
          rotate: e15(rot),
          style: { size: "$size.label", ink: ink.ink, align: "middle", baseline: "middle" }
        }))] }) : null,
        p.total ? group14({ key: "total", when: e15(single), children: [repeat11(cx.table("root", laid, op10.filter(e15("d.depth == 0"))), group14({ children: [
          text13(e15(label), [e15("box.w / 2"), e15("box.h / 2 - 4")], { key: "name", when: e15(`d.y1 >= 34 && d.lw + 8 <= 2 * d.y1`), style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "alphabetic" } }),
          text13("", [e15("box.w / 2"), e15("box.h / 2")], { key: "value", when: e15("d.y1 >= 22"), number: { value: e15("d.sum"), format: p.format }, style: { font: "font.title", size: e15(`min(token("size.title"), d.y1 * 0.5)`), ink: "$ink", align: "middle", baseline: e15(`d.y1 >= 34 && d.lw + 8 <= 2 * d.y1 ? "hanging" : "middle"`) } })
        ] }))] }) : null
      ]
    });
  }
});
var icicle = recipe15({
  id: "@datars/std/icicle",
  doc: "An icicle: a hierarchy as bands, the root first and each node spanning its share of its parent, its name and value inside where they fit. Horizontal (the root at the left) or vertical (on top).",
  params: { ...partitionParams(), orientation: t15.oneOf(["horizontal", "vertical"], "horizontal", "The root at the left, its children in the next column (names read across), or on top.") },
  tokens: ["categorical", "paper", "muted", "ink", "size.label", "size.small"],
  expand(p, cx) {
    const id = p.id ?? "id", parent = p.parent ?? "parent";
    const label = p.label ? col(p.label) : `key.name(${col(id)})`;
    const across = (p.orientation ?? "horizontal") === "horizontal";
    const f = J(p.format);
    const laid = cx.table(
      "partition",
      p.data,
      op10.partition({ id, parent, label: p.label, value: p.value, width: e15(across ? "box.h" : "box.w"), height: e15(across ? "box.w" : "box.h"), sort: p.sort }),
      op10.derive("lw", e15(`measure(${label}, token("size.label"), 600)`)),
      op10.derive("vw", e15(`measure(format(d.sum, ${f}), token("size.small"))`)),
      op10.derive("rootw", e15("d.depth == 0 ? max(d.lw, d.vw) : 0"))
    );
    const ink = partitionInk(p, cx, laid, "1");
    const L = J(laid);
    const H = across ? "box.w" : "box.h";
    const band = `(${H} / (table.max(${L}, "depth") + 1))`;
    const thin = across ? `min(${band}, table.max(${L}, "rootw") + 14)` : `min(${band}, token("size.label") + token("size.small") + 22)`;
    const depthAt = (v) => `(${v} <= ${band} ? ${v} * ${thin} / ${band} : ${thin} + (${v} - ${band}) * (${H} - ${thin}) / max(1, ${H} - ${band}))`;
    const [d0, d1] = [depthAt("d.y0"), depthAt("d.y1")];
    const [x, y, w, h] = across ? [d0, "d.x0", `(${d1} - ${d0})`, "(d.x1 - d.x0)"] : ["d.x0", d0, "(d.x1 - d.x0)", `(${d1} - ${d0})`];
    const fill = e15(`d.depth == 0 ? "$muted" : scale.color(${col(p.color ?? "branch")})`);
    const opacity = e15(`d.depth == 0 ? 0.3 : ${ink.opacity.expr}`);
    const textInk = e15(`d.depth == 0 ? "$ink" : ${ink.ink.expr}`);
    const nameFits = `${w} >= d.lw + 12 && ${h} >= token("size.label") + 8`;
    const valueFits = `${w} >= max(d.lw, d.vw) + 12 && ${h} >= token("size.label") + token("size.small") + 14`;
    return group14({
      key: "icicle",
      scales: { color: ink.scale },
      semantics: { role: "group", label: "Icicle" },
      children: [
        group14({ key: "cells", children: [repeat11(laid, shape14(geom14.rect({ x: e15(x), y: e15(y), w: e15(w), h: e15(h) }), {
          key: e15(col(id)),
          fill,
          opacity,
          stroke: { paint: "$paper", width: 1 },
          semantics: { role: "datum", label: partitionLabel(f), value: e15("d.sum") },
          pickable: true
        }))] }),
        p.labels ? group14({ key: "labels", children: [repeat11(laid, group14({ children: [
          text13(e15(label), [e15(`${x} + 6`), e15(`${y} + 6`)], { key: "name", when: e15(nameFits), style: { size: "$size.label", weight: 600, ink: textInk, baseline: "top" } }),
          text13(e15(`format(d.sum, ${f})`), [e15(`${x} + 6`), e15(`${y} + 8 + token("size.label")`)], { key: "value", when: e15(valueFits), style: { size: "$size.small", ink: textInk, baseline: "top" } })
        ] }))] }) : null
      ]
    });
  }
});

// src/business.ts
import { e as e16, geom as geom15, group as group15, instances as instances7, op as op11, recipe as recipe16, repeat as repeat12, shape as shape15, t as t16, text as text14 } from "@datars/sdk";
var q2 = (s) => JSON.stringify(s);
var fld = (f) => /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(f) ? `d.${f}` : `d[${JSON.stringify(f)}]`;
function vsrc(v, fallback = "null") {
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "null";
  if (typeof v === "boolean") return String(v);
  if (v && typeof v === "object" && "expr" in v) return `(${v.expr})`;
  if (typeof v === "string") return v.startsWith("=") ? `(${v.slice(1)})` : JSON.stringify(v);
  return fallback;
}
function affixed3(value, format, prefix, suffix) {
  if (!prefix && !suffix) return `format(${value}, ${q2(format)})`;
  return `((${value}) < 0 ? "\u2212" : "") + ${q2(prefix ?? "")} + format(abs(${value}), ${q2(format)}) + ${q2(suffix ?? "")}`;
}
function signed(value, format, prefix, suffix) {
  const f = format.replace(/^\+/, "");
  return `((${value}) > 0 ? "+" : (${value}) < 0 ? "\u2212" : "") + ${q2(prefix ?? "")} + format(abs(${value}), ${q2(f)}) + ${q2(suffix ?? "")}`;
}
var kpi = recipe16({
  id: "@datars/std/kpi",
  doc: "A key figure: its label, the value as a big number (counting when it changes), its change against a comparison \u2014 an arrow and the delta in `$positive` or `$negative` by whether the change is good (`better`) \u2014 and, from a table, a sparkline of how it got there. The value and comparison are numbers or expressions, or the last and previous rows of `y` in `data`. Screen readers and tooltips read label, value and change in one sentence.",
  params: {
    label: t16.string("", "What the number is (sentence case: `Revenue`)."),
    value: t16.prop("The value: a number or an expression (`e(\"table.sum('orders', 'total')\")`, a signal). Default: the last `y` in `data`."),
    compare: t16.prop("What the value is compared with (last month, a target): a number or an expression. Default: the row before the last in `data`. None: no change line."),
    data: t16.table("Rows over time (optional): the value is the last row's `y`, the comparison the one before, and a sparkline shows them all."),
    x: t16.field("The rows' order (a date): sorts them and runs the sparkline."),
    y: t16.field("The measured field in `data`."),
    format: t16.string(",.0f", "Number format of the value (d3-format: `$,.0f`, `.3~s` for 12.9k, `.1%`)."),
    prefix: t16.string(void 0, "Before the number (`\u20AC`)."),
    suffix: t16.string(void 0, "After it (` kr`, `M`)."),
    delta: t16.oneOf(["percent", "absolute", "none"], "percent", "How the change reads: relative (+4.1%), in the value's units (+120), or not at all."),
    deltaFormat: t16.string(void 0, "Format of the change (default `.1%`, or the value's format for `absolute`); it's always signed."),
    compareLabel: t16.string(void 0, "What the change is against, after it (`vs last month`)."),
    better: t16.oneOf(["up", "down", "neither"], "up", "Which way is good: `down` for costs and waiting times (a fall in `$positive`); `neither` keeps the change neutral."),
    spark: t16.bool(true, "A sparkline of `y` under the change (with `data` and `x`)."),
    align: t16.oneOf(["start", "center"], "start", "Left-aligned (a dashboard tile) or centred."),
    size: t16.number(0, "The number's font size in px (0: from the box, 22\u201360).")
  },
  tokens: ["ink", "ink-2", "muted", "positive", "negative", "mark", "size.body"],
  expand(p, cx) {
    let value = vsrc(p.value, "");
    let compare = vsrc(p.compare, "");
    let rows;
    if (p.data && p.y) {
      rows = cx.table("kpi", p.data, ...p.x ? [op11.sort(p.x)] : [], op11.window("lag", p.y, "__prev"));
      if (!value) value = `table.last(${q2(rows)}, ${q2(p.y)})`;
      if (!compare) compare = `table.last(${q2(rows)}, "__prev")`;
    }
    const v = `(${value || "null"})`;
    const c = `(${compare || "null"})`;
    const hasDelta = compare !== "" && p.delta !== "none";
    const diff = `(${v} - ${c})`;
    const change = p.delta === "absolute" ? diff : `(${c} != 0 ? ${diff} / abs(${c}) : null)`;
    const dfmt = p.deltaFormat ?? (p.delta === "absolute" ? p.format : ".1%");
    const deltaFmt = `+${dfmt.replace(/^\+/, "")}`;
    const deltaCounts = !(p.delta === "absolute" && (p.prefix || p.suffix));
    const deltaText = deltaCounts ? `format(${change}, ${q2(deltaFmt)})` : signed(change, dfmt, p.prefix, p.suffix);
    const good = p.better === "down" ? `${diff} < 0` : `${diff} > 0`;
    const ink = p.better === "neither" ? `"$ink-2"` : `(${diff} == 0 ? "$muted" : ${good} ? "$positive" : "$negative")`;
    const valueText = affixed3(v, p.format, p.prefix, p.suffix);
    const word = `(${diff} > 0 ? "up " : ${diff} < 0 ? "down " : "unchanged")`;
    const amount = p.delta === "absolute" ? affixed3(`abs(${diff})`, dfmt.replace(/^\+/, ""), p.prefix, p.suffix) : `format(abs(${change}), ${q2(dfmt.replace(/^\+/, ""))})`;
    const said = `${q2(p.label ? `${p.label}: ` : "")} + ${valueText}` + (hasDelta ? ` + (${c} == null || ${change} == null ? "" : ", " + ${word} + (${diff} != 0 ? ${amount} : "") + ${q2(p.compareLabel ? ` ${p.compareLabel}` : "")})` : "");
    const spark = !!(p.spark && rows && p.x);
    const S = p.size ? String(p.size) : `clamp(min(box.h * ${spark ? 0.26 : 0.36}, box.w * 0.17), 22, 60)`;
    const center = p.align === "center";
    const vs = "(box.h / 1.2)";
    const pw = p.prefix ? `measure(${q2(p.prefix)}, ${vs}, 600)` : "0";
    const nw = `measure(format(${v}, ${q2(p.format)}), ${vs}, 600)`;
    const sw = p.suffix ? `measure(${q2(p.suffix)}, ${vs}, 600)` : "0";
    const x0 = center ? `((box.w - ${pw} - ${nw} - ${sw}) / 2)` : "0";
    const big = { size: e16(vs), weight: 600, ink: "$ink", baseline: "top" };
    const valueRow = group15({
      key: "value-row",
      size: { h: e16(`${S} * 1.2`) },
      children: [
        p.prefix ? text14(p.prefix, [e16(x0), 0], { key: "prefix", style: big }) : null,
        text14("", [e16(`${x0} + ${pw}`), 0], { key: "value", number: { value: e16(v), format: p.format }, style: big, semantics: { role: "datum", label: e16(said), value: e16(v) }, pickable: true }),
        p.suffix ? text14(p.suffix, [e16(`${x0} + ${pw} + ${nw}`), 0], { key: "suffix", style: big }) : null
      ]
    });
    const body = 'token("size.body")';
    const dw = `measure(${deltaText}, ${body}, 600)`;
    const lw = p.compareLabel ? `measure(${q2(p.compareLabel)}, ${body})` : "0";
    const dx0 = center ? `((box.w - 13 - ${dw} - ${p.compareLabel ? `5 - ${lw}` : "0"}) / 2)` : "0";
    const mid = "box.h / 2";
    const tri = `${diff} >= 0 ? \`M \${${dx0}} \${${mid} + 4} L \${${dx0} + 4.5} \${${mid} - 4} L \${${dx0} + 9} \${${mid} + 4} Z\` : \`M \${${dx0}} \${${mid} - 4} L \${${dx0} + 9} \${${mid} - 4} L \${${dx0} + 4.5} \${${mid} + 4} Z\``;
    const deltaRow = hasDelta ? group15({
      key: "delta-row",
      size: { h: e16(`${body} * 1.5`) },
      when: e16(`${c} != null && ${change} != null`),
      children: [
        shape15(geom15.path(e16(tri)), { key: "arrow", when: e16(`${diff} != 0`), fill: e16(ink), semantics: { role: "decoration" } }),
        // Keyed by its format: the change counts from one value to the next, but a change of
        // format (a share to an amount) is a new line, not a count between the two.
        text14(deltaCounts ? "" : e16(deltaText), [e16(`${dx0} + 13`), e16(mid)], { key: `delta ${deltaFmt}`, number: deltaCounts ? { value: e16(change), format: deltaFmt } : void 0, style: { size: "$size.body", weight: 600, ink: e16(ink), baseline: "middle" } }),
        p.compareLabel ? text14(p.compareLabel, [e16(`${dx0} + 18 + ${dw}`), e16(mid)], { key: "against", style: { size: "$size.body", ink: "$muted", baseline: "middle" } }) : null
      ]
    }) : null;
    return group15({
      key: "kpi",
      layout: { type: "rows", gap: 4 },
      semantics: { role: "group", label: p.label || "Key figure" },
      children: [
        p.label ? text14(p.label, [center ? e16("box.w / 2") : 0, 0], { key: "label", size: { h: "auto" }, style: { size: "$size.body", weight: 600, ink: "$ink-2", baseline: "top", align: center ? "middle" : "start", maxWidth: e16("box.w") } }) : null,
        valueRow,
        deltaRow,
        spark && rows ? group15({ key: "trend", size: { h: "fill" }, children: [sparkline({ data: rows, x: p.x, y: p.y, trend: false, stroke: "$mark", format: p.format, name: p.label || p.y })] }) : null
      ]
    });
  }
});
var BAND_ALPHA = [0.3, 0.19, 0.11, 0.06, 0.035];
var bullet = recipe16({
  id: "@datars/std/bullet",
  doc: "Bullet graphs: per row the actual value as a bar over qualitative bands (poor, fair, good \u2014 darkest first) with the target as a tick across it, each on its own scale with its own ticks (or one scale for all with `shared`). A compact replacement for a gauge: many measures in the height of one.",
  params: {
    data: t16.table("One row per measure."),
    label: t16.field("The measure's name (and the rows' order)."),
    value: t16.field("The actual value."),
    target: t16.field("The target (optional): a tick across the bar."),
    bands: t16.json('The qualitative ranges\' upper bounds, ascending: field names (each row its own, `["poor", "fair", "good"]`) or numbers (the same for every row, `[150, 225, 300]`).'),
    sublabel: t16.field("A second, smaller line under the name (its unit: `US$, thousands`)."),
    max: t16.prop("Where the scale ends (a number): default each row's largest value, target or band (rounded up to a nice number without bands)."),
    shared: t16.bool(false, "One scale for every row (the largest of them all), ticked under the last: measures in the same unit compare across rows."),
    format: t16.string(",.4~r", "Tick and label number format."),
    ink: t16.ink("$mark", "The value bar's ink.")
  },
  tokens: ["mark", "ink", "ink-2", "muted", "size.body", "size.small"],
  expand(p, cx) {
    const bands = Array.isArray(p.bands) ? p.bands : [];
    const bandSrc = bands.map((b) => typeof b === "number" ? String(b) : `(${fld(String(b))} ?? 0)`);
    const v = fld(p.value), tg = p.target ? fld(p.target) : null;
    const parts = [`(${v} ?? 0)`, ...tg ? [`(${tg} ?? 0)`] : [], ...bandSrc];
    const fixed = p.max !== void 0 && p.max !== null ? vsrc(p.max) : null;
    const nice = !bands.length && !fixed;
    const ops = [
      op11.derive("__one", 1),
      op11.window("cumsum", "__one", "__pos"),
      op11.derive("__m", e16(fixed ?? `max(${parts.join(", ")}, 0)`))
    ];
    if (p.shared && !fixed) ops.push(op11.window("cummax", "__m", "__cm"), op11.window("last", "__cm", "__m2"), op11.derive("__m", e16("d.__m2")));
    ops.push(
      op11.derive("__p", e16("pow(10, floor(log10(max(d.__m, 0.000001) / 4)))")),
      op11.derive("__r", e16("d.__m / 4 / d.__p")),
      op11.derive("__step", e16("(d.__r <= 1 ? 1 : d.__r <= 2 ? 2 : d.__r <= 2.5 ? 2.5 : d.__r <= 5 ? 5 : 10) * d.__p")),
      op11.derive("__max", e16(nice ? "max(d.__step, ceil(d.__m / d.__step - 0.000001) * d.__step)" : "max(d.__m, 0.000001)")),
      op11.derive("__n", e16("floor(d.__max / d.__step + 0.000001) + 1")),
      op11.derive("__lw", e16(`max(measure(key.name(${fld(p.label)}), token("size.body"), 600), ${p.sublabel ? `measure(String(${fld(p.sublabel)} ?? ""), token("size.small"))` : "0"})`))
    );
    const T = cx.table("bullet", p.data, ...ops);
    const ticks = cx.table("bullet-ticks", T, op11.units({ value: "__n" }), ...p.shared ? [op11.filter(e16(`d.__pos == table.count(${q2(T)})`))] : []);
    const by = `scale.by(${fld(p.label)})`, bw = "scale.by.bandwidth()";
    const X = (x) => `clamp((${x}) / d.__max, 0, 1) * box.w`;
    const bandRects = (bands.length ? bandSrc : ["d.__max"]).map((b, i) => {
      const lo = i === 0 ? "0" : bandSrc[i - 1];
      return shape15(geom15.rect({ x: e16(X(lo)), y: e16(by), w: e16(`max(0, ${X(b)} - ${X(lo)})`), h: e16(bw) }), { key: `band-${i}`, fill: `$ink@${bands.length ? BAND_ALPHA[Math.min(i, BAND_ALPHA.length - 1)] : 0.07}`, semantics: { role: "decoration" } });
    });
    const name = `key.name(${fld(p.label)})`;
    const said = `${name} + ": " + format(${v}, ${q2(p.format)})` + (tg ? ` + (${tg} == null ? "" : " (target " + format(${tg}, ${q2(p.format)}) + ")")` : "");
    const lw = `min(box.w * 0.42, table.max(${q2(T)}, "__lw"))`;
    return group15({
      key: "bullet",
      scales: { by: { type: "band", domain: { data: T, field: p.label }, range: { box: "bullet-area", axis: "y" }, padding: 0.5 } },
      layout: { type: "columns", gap: 12 },
      semantics: { role: "group", label: "Bullet graph" },
      children: [
        // Names right-aligned against their bullets, as wide as the widest (up to 42 % of the box).
        group15({ key: "labels", size: { w: e16(lw) }, children: [repeat12(T, group15({ children: [
          text14(e16(name), [e16("box.w"), e16(p.sublabel ? `${by} + ${bw} / 2 - 1` : `${by} + ${bw} / 2`)], { key: "name", style: { size: "$size.body", weight: 600, ink: "$ink", align: "end", baseline: p.sublabel ? "bottom" : "middle" } }),
          p.sublabel ? text14(e16(`String(${fld(p.sublabel)} ?? "")`), [e16("box.w"), e16(`${by} + ${bw} / 2 + 1`)], { key: "sub", style: { size: "$size.small", ink: "$muted", align: "end", baseline: "top" } }) : null
        ] }))] }),
        group15({ id: "bullet-area", key: "area", children: [
          repeat12(T, group15({ children: [
            ...bandRects,
            shape15(geom15.rect({ x: 0, y: e16(`${by} + ${bw} / 3`), w: e16(X(`${v} ?? 0`)), h: e16(`${bw} / 3`) }), { key: "value", fill: p.ink, semantics: { role: "datum", label: e16(said), value: e16(v) }, pickable: true }),
            tg ? shape15(geom15.segment({ x1: e16(X(tg)), y1: e16(`${by} + ${bw} * 0.16`), x2: e16(X(tg)), y2: e16(`${by} + ${bw} * 0.84`) }), { key: "target", when: e16(`${tg} != null`), stroke: { paint: "$ink", width: 2.5 }, semantics: { role: "decoration" } }) : null
          ] })),
          group15({ key: "ticks", children: [repeat12(ticks, group15({ when: e16("d.__unit * d.__step <= d.__max * 1.000001"), children: [
            shape15(geom15.segment({ x1: e16(X("d.__unit * d.__step")), y1: e16(`${by} + ${bw}`), x2: e16(X("d.__unit * d.__step")), y2: e16(`${by} + ${bw} + 3`) }), { key: "tick", stroke: { paint: "$muted", width: 1 } }),
            text14(e16(`format(d.__unit * d.__step, ${q2(p.format)})`), [e16(X("d.__unit * d.__step")), e16(`${by} + ${bw} + 4`)], { key: "tick-label", style: { size: "$size.small", ink: "$muted", baseline: "top", align: e16(`d.__unit == 0 ? "start" : d.__unit * d.__step >= d.__max * 0.999 ? "end" : "middle"`) } })
          ] }))] })
        ] })
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "left" } }]
});
var gauge = recipe16({
  id: "@datars/std/gauge",
  doc: "A dial: an arc from `min` to `max` filled up to the value \u2014 or, with `bands`, coloured ranges and a needle \u2014 with the value in the middle, the ends labelled and the label under it. The needle turns (and the fill grows) when the value changes. For many measures at once, `bullet` says more in less room.",
  params: {
    value: t16.prop("The value: a number or an expression (a signal, `table.last(\u2026)`)."),
    min: t16.number(0, "The arc's start."),
    max: t16.number(100, "The arc's end."),
    label: t16.string(void 0, "What it measures, under the dial."),
    format: t16.string(",.0f", "Number format of the value and the ends."),
    prefix: t16.string(void 0, "Before the value (`$`)."),
    suffix: t16.string(void 0, "After it (`%`, ` km/h`)."),
    bands: t16.json("Upper bounds of coloured ranges along the arc, ascending (`[60, 85, 100]`); with bands the value shows as a needle."),
    inks: t16.json('The bands\' inks, in order (default: from light to dark `$ink`; status inks like `["$positive", "$highlight", "$negative"]` read as good to bad).'),
    needle: t16.bool(void 0, "A needle instead of a filled arc (default: with bands)."),
    arc: t16.number(240, "The dial's sweep in degrees (180: a half circle; up to 300)."),
    ink: t16.ink("$mark", "The fill's ink (without bands)."),
    thickness: t16.number(0.22, "The arc's thickness, as a share of its radius.")
  },
  tokens: ["mark", "ink", "ink-2", "muted", "paper", "size.label", "size.small"],
  expand(p) {
    const sweep = Math.min(300, Math.max(60, p.arc || 240)) * Math.PI / 180;
    const half = sweep / 2;
    const [lo, hi] = [p.min ?? 0, p.max ?? 100];
    const v = vsrc(p.value, "null");
    const frac = `clamp((${v} - ${lo}) / ${hi - lo || 1}, 0, 1)`;
    const ang = (f) => `(${-half} + (${f}) * ${sweep})`;
    const wUnits = sweep >= Math.PI ? 2 : 2 * Math.sin(half);
    const below = Math.max(0, -Math.cos(half));
    const ends = 'token("size.small") + 8';
    const needle = p.needle ?? (Array.isArray(p.bands) && p.bands.length > 0);
    const vTop = sweep > Math.PI ? 0.3 : 0.12, vS = needle ? 0.26 : 0.42;
    const extent = needle ? Math.max(below, vTop + vS * 1.1) : below;
    const lab = p.label ? 'token("size.body") * 1.3 + 4' : "0";
    const R = `max(8, min(box.w / ${wUnits.toFixed(4)}, (box.h - (${ends}) - (${lab})) / ${(1 + extent).toFixed(4)}) - 2)`;
    const cy = `((box.h - (${ends}) - (${lab}) - ${R} * ${(1 + extent).toFixed(4)}) / 2 + ${R})`;
    const cxs = "box.w / 2";
    const th = Math.min(0.6, Math.max(0.05, p.thickness ?? 0.22));
    const r0 = `${R} * ${1 - th}`;
    const bands = Array.isArray(p.bands) ? p.bands : [];
    const inks = Array.isArray(p.inks) ? p.inks : [];
    const arc = (key, f0, f1, fill, extra = {}) => shape15(geom15.arc({ cx: e16(cxs), cy: e16(cy), r0: e16(r0), r1: e16(R), a0: e16(ang(f0)), a1: e16(ang(f1)) }), { key, fill, semantics: { role: "decoration" }, ...extra });
    const bandArcs = bands.map((b, i) => {
      const f0 = i === 0 ? "0" : String((bands[i - 1] - lo) / (hi - lo || 1));
      const ink = inks[i] ?? `$ink@${[0.12, 0.22, 0.34, 0.46, 0.58][Math.min(i, 4)]}`;
      return arc(`band-${i}`, `clamp(${f0}, 0, 1)`, `clamp(${(b - lo) / (hi - lo || 1)}, 0, 1)`, ink);
    });
    const valueText = affixed3(v, p.format, p.prefix, p.suffix);
    const counting = !(p.prefix || p.suffix);
    const said = `${q2(p.label ? `${p.label}: ` : "")} + ${valueText} + ${q2(` (${lo} to ${hi})`)}`;
    const deg = `(${ang(frac)} * ${180 / Math.PI} / 2)`;
    const needleNode = group15({ key: "needle", transform: { translate: [e16(cxs), e16(cy)], rotate: e16(deg) }, children: [group15({ key: "turn", transform: { rotate: e16(deg) }, children: [
      shape15(geom15.path(e16(`\`M -3 0 L 0 \${-(${R}) * 0.92} L 3 0 Z\``)), { key: "hand", fill: "$ink", semantics: { role: "decoration" } }),
      shape15(geom15.circle({ cx: 0, cy: 0, r: 5 }), { key: "hub", fill: "$ink", stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "decoration" } })
    ] })] });
    const vSize = `clamp(min(${R} * ${vS}, ${needle ? `${R} * 1.3` : `${r0} * 1.5`} * 10 / max(1, measure(${counting ? `format(${v}, ${q2(p.format)})` : valueText}, 10, 600))), 11, 56)`;
    const vy = needle ? `${cy} + ${R} * ${vTop}` : `${cy} + ${sweep > Math.PI ? `(${vSize}) * 0.36` : `-(${vSize}) * 0.12`}`;
    const endAt = (f) => [e16(`${cxs} + (${R} * ${1 - th / 2}) * sin(${ang(String(f))})`), e16(`${cy} - (${R} * ${1 - th / 2}) * cos(${ang(String(f))}) + ${R} * ${th / 2} + 4`)];
    return group15({
      key: "gauge",
      semantics: { role: "group", label: p.label || "Gauge" },
      children: [
        group15({ key: "dial", children: [
          arc("track", "0", "1", bands.length ? "$ink@0.06" : "$ink@0.1"),
          ...bandArcs,
          needle ? null : arc("fill", "0", frac, p.ink, { semantics: { role: "datum", label: e16(said), value: e16(v) }, pickable: true }),
          needle ? needleNode : null,
          text14(counting ? "" : e16(valueText), [e16(cxs), e16(vy)], { key: "value", number: counting ? { value: e16(v), format: p.format } : void 0, style: { size: e16(vSize), weight: 600, ink: "$ink", align: "middle", baseline: needle ? "top" : "alphabetic" }, semantics: needle ? { role: "datum", label: e16(said), value: e16(v) } : void 0, pickable: needle || void 0 }),
          text14(e16(`format(${lo}, ${q2(p.format)})`), endAt(0), { key: "min", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } }),
          text14(e16(`format(${hi}, ${q2(p.format)})`), endAt(1), { key: "max", style: { size: "$size.small", ink: "$muted", align: "middle", baseline: "top" } })
        ] }),
        p.label ? text14(p.label, [e16("box.w / 2"), e16(`${cy} + max(${R} * ${extent.toFixed(4)} + 6, ${R} * ${(below + th / 2).toFixed(4)} + ${ends})`)], { key: "label", style: { size: "$size.body", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e16("box.w") } }) : null
      ]
    });
  }
});
var progress = recipe16({
  id: "@datars/std/progress",
  doc: "Progress toward a goal: a bar (label and share above it) or a ring (the share in the middle), filled to value \xF7 goal \u2014 past the goal it stays full and the share says how far past. The fill grows when the value changes.",
  params: {
    value: t16.prop("How far along: a number or an expression."),
    goal: t16.prop("The goal (default 1: the value is a share)."),
    label: t16.string(void 0, "What's progressing."),
    shape: t16.oneOf(["bar", "ring"], "bar", "A bar across the box, or a ring."),
    format: t16.string(",.0f", "Number format of the value and the goal (with `showGoal`)."),
    prefix: t16.string(void 0, "Before the value and goal (`$`)."),
    suffix: t16.string(void 0, "After them."),
    showGoal: t16.bool(false, "Say `value of goal` too (`7,200 of 10,000`)."),
    ink: t16.ink("$mark", "The fill's ink."),
    thickness: t16.number(0, "The bar's height or the ring's width in px (0: 8 for a bar, a fifth of the ring's radius).")
  },
  tokens: ["mark", "ink", "ink-2", "muted", "size.body", "size.label"],
  expand(p) {
    const v = vsrc(p.value, "0");
    const g = p.goal === void 0 || p.goal === null ? "1" : vsrc(p.goal, "1");
    const share = `((${g}) != 0 ? (${v}) / (${g}) : 0)`;
    const f = `clamp(${share}, 0, 1)`;
    const pct = `format(${share}, ".0%")`;
    const of = `${affixed3(v, p.format, p.prefix, p.suffix)} + " of " + ${affixed3(g, p.format, p.prefix, p.suffix)}`;
    const said = `${q2(p.label ? `${p.label}: ` : "")} + ${pct}` + (p.showGoal ? ` + " (" + ${of} + ")"` : "");
    const sem = { role: "datum", label: e16(said), value: e16(share) };
    if (p.shape === "ring") {
      const R = "max(6, min(box.w, box.h) / 2 - 2)";
      const w = p.thickness ? String(p.thickness) : `max(4, ${R} * 0.2)`;
      const ring = (key, a1, fill, extra = {}) => shape15(geom15.arc({ cx: e16("box.w / 2"), cy: e16("box.h / 2"), r0: e16(`${R} - ${w}`), r1: e16(R), a0: 0, a1: e16(a1) }), { key, fill, ...extra });
      return group15({ key: "progress", layout: { type: "rows", gap: 6 }, semantics: { role: "group", label: p.label || "Progress" }, children: [
        group15({ key: "ring", size: { h: "fill" }, children: [
          ring("track", String(2 * Math.PI), "$ink@0.1", { semantics: { role: "decoration" } }),
          ring("fill", `${f} * ${2 * Math.PI}`, p.ink, { semantics: sem, pickable: true }),
          text14("", [e16("box.w / 2"), e16("box.h / 2")], { key: "share", number: { value: e16(share), format: ".0%" }, style: { size: e16(`clamp((${R} - ${w}) * 0.46, 11, 34)`), weight: 600, ink: "$ink", align: "middle", baseline: "middle" } })
        ] }),
        p.label ? text14(p.label, [e16("box.w / 2"), 0], { key: "label", size: { h: "auto" }, style: { size: "$size.body", weight: 600, ink: "$ink-2", align: "middle", baseline: "top", maxWidth: e16("box.w") } }) : null,
        p.showGoal ? text14(e16(of), [e16("box.w / 2"), 0], { key: "goal", size: { h: "auto" }, style: { size: "$size.label", ink: "$muted", align: "middle", baseline: "top", maxWidth: e16("box.w") } }) : null
      ] });
    }
    const h = p.thickness || 8;
    return group15({ key: "progress", layout: { type: "rows", gap: 6 }, semantics: { role: "group", label: p.label || "Progress" }, children: [
      group15({ key: "head", size: { h: e16('token("size.body") * 1.3') }, children: [
        p.label ? text14(p.label, [0, e16("box.h / 2")], { key: "label", style: { size: "$size.body", weight: 600, ink: "$ink-2", baseline: "middle", maxWidth: e16("box.w * 0.7") } }) : null,
        text14("", [e16("box.w"), e16("box.h / 2")], { key: "share", number: { value: e16(share), format: ".0%" }, style: { size: "$size.body", weight: 600, ink: "$ink", align: "end", baseline: "middle" } })
      ] }),
      group15({ key: "bar", size: { h }, children: [
        shape15(geom15.rect({ x: 0, y: 0, w: e16("box.w"), h, r: h / 2 }), { key: "track", fill: "$ink@0.1", semantics: { role: "decoration" } }),
        shape15(geom15.rect({ x: 0, y: 0, w: e16(`max(${f} > 0 ? ${h} : 0, ${f} * box.w)`), h, r: h / 2 }), { key: "fill", fill: p.ink, semantics: sem, pickable: true })
      ] }),
      p.showGoal ? text14(e16(of), [0, 0], { key: "goal", size: { h: "auto" }, style: { size: "$size.label", ink: "$muted", baseline: "top" } }) : null
    ] });
  }
});
var radar = recipe16({
  id: "@datars/std/radar",
  doc: "A radar (spider) chart: one spoke per dimension, in the order they first appear, and each series a closed shape through its values, on one radial scale from zero with rings at even steps. Long rows (series, dimension, value); series are keyed, so a changed value reshapes its polygon and a new series grows in. Best for a few series over five to ten dimensions.",
  params: {
    data: t16.table("Long rows: one per series and dimension."),
    axis: t16.field("The dimension (a spoke each)."),
    value: t16.field("The value (distance from the centre)."),
    series: t16.field("One shape per series (optional), coloured by the categorical palette."),
    max: t16.prop("The scale's end (a number: `5` for ratings out of 5; default: the largest value, rounded up to a ring)."),
    levels: t16.number(4, "About how many rings, at nice steps from the centre to the rim, each labelled with its value."),
    format: t16.string(",.4~r", "Number format of the rings and the tooltips."),
    fill: t16.bool(true, "A soft fill inside each shape."),
    points: t16.bool(true, "A dot at each value (hover it for the value)."),
    legend: t16.bool(true, "Name the series below the chart (with `series`).")
  },
  tokens: ["grid", "ink-2", "muted", "mark", "categorical", "paper", "size.label", "size.small"],
  expand(p, cx) {
    const A = fld(p.axis), V = fld(p.value);
    const axes = cx.table("radar-axes", p.data, op11.aggregate([p.axis], { __rows: ["count"] }), op11.derive("__one", 1), op11.window("cumsum", "__one", "__ai"), op11.derive("__lw", e16(`measure(key.name(${A}), token("size.label"))`)));
    const levels = Math.max(1, Math.round(p.levels ?? 4));
    const fixed = p.max !== void 0 && p.max !== null ? vsrc(p.max) : null;
    const stats = cx.table(
      "radar-scale",
      p.data,
      op11.aggregate([], { __m: ["max", p.value] }),
      op11.derive("__one", 1),
      ...fixed ? [op11.derive("__m", e16(fixed))] : [],
      op11.derive("__p", e16(`pow(10, floor(log10(max(d.__m, 0.000001) / ${levels})))`)),
      op11.derive("__r", e16(`d.__m / ${levels} / d.__p`)),
      op11.derive("__step", e16("(d.__r <= 1 ? 1 : d.__r <= 2 ? 2 : d.__r <= 2.5 ? 2.5 : d.__r <= 5 ? 5 : 10) * d.__p")),
      op11.derive("__max", e16(fixed ? "max(d.__m, 0.000001)" : "max(d.__step, ceil(d.__m / d.__step - 0.000001) * d.__step)")),
      op11.derive("__n", e16("ceil(d.__max / d.__step - 0.000001)"))
    );
    const rings = cx.table("radar-rings", axes, op11.join(stats, "__one"), op11.units({ value: "__n" }), op11.derive("__f", e16("min(1, (d.__unit + 1) * d.__step / d.__max)")));
    const rows = cx.table("radar", p.data, op11.join(axes, p.axis), op11.sort("__ai"));
    const R = `max(10, min(box.h / 2 - token("size.label") - 8, box.w / 2 - table.max(${q2(axes)}, "__lw") - 12))`;
    const at2 = (r, a) => [e16(`box.w / 2 + (${r}) * sin(${a})`), e16(`box.h / 2 - (${r}) * cos(${a})`)];
    const ang = (x) => `scale.ang(${x})`;
    const xy = (r, a) => ({ x: e16(`box.w / 2 + (${r}) * sin(${a})`), y: e16(`box.h / 2 - (${r}) * cos(${a})`) });
    const series = p.series;
    const ink = series ? e16(`scale.color(${fld(series)})`) : "$mark";
    const name = series ? `key.name(${fld(series)}) + " \xB7 " + ` : "";
    const chart = group15({
      key: "radar",
      size: { h: "fill" },
      scales: {
        ang: { type: "band", domain: { data: axes, field: p.axis }, range: [0, 2 * Math.PI], padding: 0 },
        r: { type: "linear", domain: { data: stats, field: "__max" }, range: [0, `=${R}`], zero: true, nice: false }
      },
      children: [
        // Rings at even steps of the scale, each labelled on the first spoke.
        group15({ key: "rings", children: [repeat12({ groups: rings, by: "__unit" }, group15({ children: [
          shape15(geom15.polyline({ from: "@group", ...xy("d.__f * scale.r.max()", ang(A)), curve: "linear", closed: true }), { key: "ring", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
          text14(e16(`format(d.__f * scale.r.invert(scale.r.max()), ${q2(p.format)})`), [e16("box.w / 2 + 4"), e16("box.h / 2 - d.__f * scale.r.max() + 2")], { key: "ring-label", style: { size: "$size.small", ink: "$muted", baseline: "top" }, halo: ["$paper", 2] })
        ] }))] }),
        group15({ key: "spokes", children: [repeat12(axes, group15({ children: [
          shape15(geom15.segment({ x1: e16("box.w / 2"), y1: e16("box.h / 2"), x2: xy("scale.r.max()", ang(A)).x, y2: xy("scale.r.max()", ang(A)).y }), { key: "spoke", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
          text14(e16(`key.name(${A})`), at2("scale.r.max() + 8", ang(A)), { key: "name", style: { size: "$size.label", ink: "$ink-2", align: e16(`sin(${ang(A)}) > 0.2 ? "start" : sin(${ang(A)}) < -0.2 ? "end" : "middle"`), baseline: e16(`cos(${ang(A)}) > 0.3 ? "bottom" : cos(${ang(A)}) < -0.3 ? "top" : "middle"`) } })
        ] }))] }),
        group15({ key: "shapes", children: [repeat12({ groups: rows, by: series ?? "__all" }, group15({ semantics: { role: "series", label: series ? e16(`key.name(${fld(series)})`) : p.value }, children: [
          p.fill ? shape15(geom15.polyline({ from: "@group", ...xy(`scale.r(${V})`, ang(A)), curve: "linear", closed: true }), { key: "area", fill: ink, opacity: 0.14, semantics: { role: "decoration" } }) : null,
          shape15(geom15.polyline({ from: "@group", ...xy(`scale.r(${V})`, ang(A)), curve: "linear", closed: true }), { key: "outline", stroke: { paint: ink, width: 2, join: "round" }, semantics: { role: "decoration" } }),
          instances7({ key: "points", from: "@group", ...xy(`scale.r(${V})`, ang(A)), instanceKey: e16(A), r: p.points ? 3 : 0, fill: p.points ? ink : "transparent", stroke: p.points ? { paint: "$paper", width: 1 } : void 0, label: e16(`${name}key.name(${A}) + ": " + format(${V}, ${q2(p.format)})`) })
        ] }))] })
      ]
    });
    if (!series || !p.legend) return group15({ key: "radar-chart", layout: { type: "rows" }, semantics: { role: "group", label: "Radar chart" }, children: [chart] });
    return group15({
      key: "radar-chart",
      layout: { type: "rows", gap: 8 },
      scales: { color: { type: "categorical", domain: { data: p.data, field: series }, range: "$categorical" } },
      semantics: { role: "group", label: "Radar chart" },
      children: [chart, legend({ scale: "color" }, { size: { h: "auto" } })]
    });
  },
  motion: [{ select: { kind: "polyline" }, enter: { scale: 0, origin: "center" } }]
});
function cellText(c) {
  const f = fld(c.field ?? "");
  if (c.kind === "text") return `(${f} == null ? "" : key.name(String(${f})))`;
  return c.prefix || c.suffix ? `(${f} == null ? "\u2013" : ${affixed3(f, c.format ?? ",.4~r", c.prefix, c.suffix)})` : `format(${f}, ${q2(c.format ?? ",.4~r")})`;
}
var dataTable = recipe16({
  id: "@datars/std/dataTable",
  doc: "A data table drawn by the engine: column headers, text left and numbers right-aligned in their formats, optional inline bars, sparklines or coloured cells per column, striped rows, sorted by a column \u2014 or by whichever header the reader clicks (`sortable`). Rows are keyed, so a new sort slides them to their places and a filter lets rows leave and arrive. Columns are as wide as their widest cell; on a narrow screen `optional` columns step aside.",
  params: {
    data: t16.table("The rows."),
    columns: t16.json("The columns, in order: `{ field, label?, type?, format?, prefix?, suffix?, align?, width?, bar?, color?, stops?, spark?: { data, x, y }, optional? }`."),
    key: t16.field("The field that names a row (its key: rows morph by it). Needed for sparklines, which match their rows on it."),
    sort: t16.field("Sort the rows by this field (default: as they come)."),
    descending: t16.bool(true, "Numbers largest first (text always sorts A to Z)."),
    sortable: t16.string(void 0, 'A text signal holding the field to sort by: clicking a header sets it (declare it: `signal.str("revenue")`).'),
    maxRows: t16.number(0, "Show only the first rows after sorting (0: all)."),
    striped: t16.bool(true, "Shade every other row (else a hairline between rows)."),
    rowHeight: t16.number(0, "Row height in px (0: from the body text size).")
  },
  tokens: ["ink", "ink-2", "muted", "surface", "grid", "rule", "mark", "up", "down", "sequential", "diverging", "size.body", "size.label"],
  expand(p, cx) {
    const cols = (Array.isArray(p.columns) ? p.columns : []).map((c, i) => {
      const kind = c.spark ? "spark" : c.bar ? "bar" : c.color ? "color" : c.type === "number" || c.type !== "text" && c.format ? "number" : "text";
      return { ...c, i, kind, head: c.label ?? c.field ?? (c.spark ? c.spark.y : "") };
    });
    const G = 16, P = 8;
    const rowH = p.rowHeight ? String(p.rowHeight) : 'round(token("size.body") * 2.3)';
    const sortCols = cols.filter((c) => c.field && c.kind !== "spark");
    const dirOf = (c, field) => c && c.kind === "text" || !p.descending ? field : `-${field}`;
    const order = [op11.derive("__one", 1)];
    if (p.sortable) {
      sortCols.forEach((c) => order.push(op11.window("cumsum", "__one", `__p${c.i}`, { order: dirOf(c, c.field) })));
      const def = p.sort ? sortCols.find((c) => c.field === p.sort) : void 0;
      if (!def) order.push(op11.window("cumsum", "__one", "__pin"));
      order.push(op11.derive("__pos", e16(sortCols.map((c) => `${p.sortable} == ${q2(c.field)} ? d.__p${c.i} : `).join("") + (def ? `d.__p${def.i}` : "d.__pin"))));
    } else {
      order.push(op11.window("cumsum", "__one", "__pos", p.sort ? { order: dirOf(cols.find((c) => c.field === p.sort), p.sort) } : {}));
    }
    const widths = cols.filter((c) => c.kind !== "spark").map((c) => op11.derive(`__w${c.i}`, e16(`measure(${cellText(c)}, token("size.body"))`)));
    const T1 = cx.table("table", p.data, ...order, ...p.maxRows > 0 ? [op11.filter(e16(`d.__pos <= ${p.maxRows}`))] : [], ...widths);
    const T1q = q2(T1);
    const head = (c) => `measure(${q2(c.head)}, token("size.label"), 600) + ${p.sortable && c.kind !== "spark" ? 14 : 0}`;
    const nat = (c) => c.width ? String(c.width) : c.kind === "spark" ? "84" : c.kind === "bar" ? `max(${head(c)}, table.max(${T1q}, "__w${c.i}") + 6 + 56)` : `max(${head(c)}, table.max(${T1q}, "__w${c.i}") + ${c.kind === "color" ? 12 : 0})`;
    const flex = (c) => !c.width && (c.kind === "text" || c.kind === "bar");
    const n = cols.length;
    const all = `(${cols.map(nat).join(" + ") || "0"} + ${G * Math.max(0, n - 1)})`;
    const fits = `(${all} <= box.w - ${2 * P})`;
    const vis = (c) => c.optional ? fits : "true";
    const count = `(${cols.map((c) => `(${vis(c)} ? 1 : 0)`).join(" + ") || "1"})`;
    const room = `(box.w - ${2 * P} - ${G} * max(0, ${count} - 1))`;
    const anyFlex = cols.some(flex);
    const fixedSum = `(${cols.filter((c) => !flex(c)).map((c) => `(${vis(c)} ? ${nat(c)} : 0)`).join(" + ") || "0"})`;
    const flexCount = `max(1, ${cols.filter(flex).map((c) => `(${vis(c)} ? 1 : 0)`).join(" + ") || "0"})`;
    const base = (c) => c.kind === "bar" ? `(table.max(${T1q}, "__w${c.i}") + 46)` : `min(${nat(c)}, 64)`;
    const bases = `(${cols.filter(flex).map((c) => `(${vis(c)} ? ${base(c)} : 0)`).join(" + ") || "0"})`;
    const extra = `((${room} - ${fixedSum} - ${bases}) / ${flexCount})`;
    const share = `(${room} / max(1, ${count}))`;
    const width = (c) => flex(c) ? `max(${c.kind === "bar" ? `table.max(${T1q}, "__w${c.i}") + 30` : "36"}, ${base(c)} + ${extra})` : anyFlex || c.width ? nat(c) : `max(${nat(c)}, ${share})`;
    const T2 = cx.table("table-rows", T1, ...cols.flatMap((c) => [op11.derive(`__W${c.i}`, e16(width(c))), op11.derive(`__V${c.i}`, e16(`${vis(c)} ? 1 : 0`))]));
    const sparks = cols.filter((c) => c.spark);
    const spark = sparks[0]?.spark;
    const series = spark ? cx.table("table-series", spark.data, op11.derive("__sx", e16(fld(spark.x))), ...sparks.map((c) => op11.derive(`__sy${c.i}`, e16(fld(c.spark.y))))) : void 0;
    const rowsFrom = series && p.key ? cx.table("table-spark", T2, op11.join(series, p.key)) : T2;
    const T2q = q2(T2);
    const mid = `(${rowH}) / 2`;
    const at2 = (c, align) => [align === "end" ? e16("box.w") : align === "center" ? e16("box.w / 2") : 0, e16(mid)];
    const alignOf = (c) => c.align ?? (c.kind === "text" ? "start" : "end");
    const scales = {};
    const cell2 = (c) => {
      const f = fld(c.field ?? "");
      const align = alignOf(c);
      const size = { w: e16(`d.__W${c.i}`) };
      const when = c.optional ? e16(`d.__V${c.i} == 1`) : void 0;
      const body = { size: "$size.body", ink: "$ink", baseline: "middle", align };
      const num = (x, style) => c.prefix || c.suffix ? text14(e16(cellText(c)), x, { key: "text", style }) : text14("", x, { key: "text", number: { value: e16(f), format: c.format ?? ",.4~r" }, style });
      if (c.kind === "text") return group15({ key: `c${c.i}`, size, when, clip: "box", children: [text14(e16(cellText(c)), at2(c, align), { key: "text", style: body })] });
      if (c.kind === "number") return group15({ key: `c${c.i}`, size, when, children: [num(at2(c, align), body)] });
      if (c.kind === "color") {
        scales[`c${c.i}`] = c.stops ? { type: "piecewise", stops: c.stops, domain: { data: T1, field: c.field } } : c.color === "diverging" ? { type: "diverging", domain: { data: T1, field: c.field }, range: "$diverging", mid: 0 } : { type: "sequential", domain: { data: T1, field: c.field }, range: "$sequential" };
        const fill = `(${f} == null ? "transparent" : scale.c${c.i}(${f}))`;
        return group15({ key: `c${c.i}`, size, when, children: [
          shape15(geom15.rect({ x: -6, y: 2, w: e16("box.w + 12"), h: e16(`${rowH} - 4`), r: 2 }), { key: "fill", fill: e16(fill), semantics: { role: "decoration" } }),
          num([align === "end" ? e16("box.w") : align === "center" ? e16("box.w / 2") : 0, e16(mid)], { ...body, ink: e16(`"on(" + ${fill} + ")"`) })
        ] });
      }
      if (c.kind === "bar") {
        const nw = `table.max(${T1q}, "__w${c.i}")`;
        const [lo, hi] = [`min(0, table.min(${T1q}, ${q2(c.field ?? "")}))`, `max(0, table.max(${T1q}, ${q2(c.field ?? "")}))`];
        const X = (v) => `(${nw} + 6 + ((${v}) - ${lo}) / max(${hi} - ${lo}, 0.000001) * (box.w - ${nw} - 6))`;
        return group15({ key: `c${c.i}`, size, when, children: [
          num([e16(nw), e16(mid)], { ...body, align: "end" }),
          shape15(geom15.rect({ x: e16(`min(${X("0")}, ${X(`${f} ?? 0`)})`), y: e16(`(${rowH}) * 0.28`), w: e16(`abs(${X(`${f} ?? 0`)} - ${X("0")})`), h: e16(`(${rowH}) * 0.44`), r: 1 }), { key: "bar", fill: "$mark", semantics: { role: "decoration" } })
        ] });
      }
      const sy = `__sy${c.i}`;
      const [first, last] = [`group.first(${q2(sy)})`, `group.last(${q2(sy)})`];
      const ink = e16(`${last} >= ${first} ? "$up" : "$down"`);
      return group15({
        key: `c${c.i}`,
        size,
        when,
        scales: {
          sx: { type: "point", domain: { data: "@group", field: "__sx" }, range: [2, "=box.w - 4"], padding: 0, nice: false },
          sy: { type: "linear", domain: { data: "@group", field: sy }, range: [`=${rowH} - 6`, 6], zero: false, nice: false }
        },
        children: [
          shape15(geom15.polyline({ from: "@group", x: e16("scale.sx(d.__sx)"), y: e16(`scale.sy(d.${sy})`), curve: "linear" }), { key: "line", stroke: { paint: ink, width: 1.5, join: "round", cap: "round" }, semantics: { role: "decoration" } }),
          shape15(geom15.circle({ cx: e16("scale.sx.max()"), cy: e16(`scale.sy(${last})`), r: 2 }), { key: "dot", when: e16(`${last} != null`), fill: ink, semantics: { role: "decoration" } })
        ]
      });
    };
    const said = cols.filter((c) => c.kind !== "spark").map((c) => `${q2(`${c.head}: `)} + ${cellText(c)}`).join(` + ", " + `) || '""';
    const rowTemplate = group15({
      key: p.key ? e16(fld(p.key)) : void 0,
      transform: { translate: [0, e16(`(d.__pos - 1) * (${rowH})`)] },
      children: [
        shape15(geom15.rect({ x: 0, y: 0, w: e16("box.w"), h: e16(rowH) }), { key: "row", fill: e16('hover() ? "$ink@0.05" : "$ink@0"'), semantics: { role: "datum", label: e16(said) }, pickable: true }),
        group15({ key: "cells", layout: { type: "columns", gap: G, padding: [0, P] }, children: cols.map(cell2) })
      ]
    });
    const headerCell = (c) => {
      const align = alignOf(c);
      const sorted = p.sortable && c.field ? `${p.sortable} == ${q2(c.field)}` : p.sort && c.field === p.sort ? "true" : "false";
      const arrow = c.kind === "text" || !p.descending ? "\u2191" : "\u2193";
      const label = sorted === "false" ? q2(c.head) : `${q2(c.head)} + (${sorted} ? ${q2(align === "end" ? "" : ` ${arrow}`)} : "")`;
      const pre = align === "end" && sorted !== "false" ? `(${sorted} ? ${q2(`${arrow} `)} : "") + ` : "";
      return group15({
        key: `h${c.i}`,
        size: { w: e16(`table.first(${T2q}, "__W${c.i}")`) },
        when: c.optional ? e16(`table.first(${T2q}, "__V${c.i}") == 1`) : void 0,
        on: p.sortable && c.field && c.kind !== "spark" ? { activate: { set: p.sortable, value: c.field } } : void 0,
        pickable: p.sortable && c.field && c.kind !== "spark" ? true : void 0,
        semantics: p.sortable && c.field && c.kind !== "spark" ? { role: "control", label: `Sort by ${c.head}` } : void 0,
        children: [text14(e16(pre + label), [align === "end" ? e16("box.w") : align === "center" ? e16("box.w / 2") : 0, e16(mid)], { key: "text", style: { size: "$size.label", weight: 600, ink: e16(`${sorted} ? "$ink" : "$ink-2"`), baseline: "middle", align } })]
      });
    };
    const n_ = `table.count(${T2q})`;
    return group15({
      key: "table",
      scales,
      layout: { type: "rows" },
      semantics: { role: "group", label: "Table" },
      children: [
        group15({ key: "header", size: { h: e16(rowH) }, layout: { type: "columns", gap: G, padding: [0, P] }, children: cols.map(headerCell) }),
        group15({ key: "body", children: [
          shape15(geom15.segment({ x1: 0, y1: 0, x2: e16("box.w"), y2: 0 }), { key: "header-rule", stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
          group15({ key: "stripes", z: -1, children: [repeat12({ count: e16(n_) }, p.striped ? shape15(geom15.rect({ x: 0, y: e16(`d.index * (${rowH})`), w: e16("box.w"), h: e16(rowH) }), { when: e16("d.index % 2 == 1"), fill: "$surface", semantics: { role: "decoration" } }) : shape15(geom15.segment({ x1: 0, y1: e16(`(d.index + 1) * (${rowH})`), x2: e16("box.w"), y2: e16(`(d.index + 1) * (${rowH})`) }), { when: e16(`d.index < ${n_} - 1`), stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }))] }),
          group15({ key: "rows", children: [spark && p.key ? repeat12({ groups: rowsFrom, by: p.key }, rowTemplate) : repeat12(rowsFrom, rowTemplate)] })
        ] })
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { opacity: 0 }, exit: { opacity: 0 } }]
});
var gantt = recipe16({
  id: "@datars/std/gantt",
  doc: "A plan: one row per task, a bar from its start to its end on a time axis, tasks under their group's heading (with a thin bar spanning the group), milestones as diamonds, how far each task has got as a solid part of its bar, and a line at today. Rows keep the order they come in, groups the order they first appear; tasks are keyed by name, so a replanned task slides to its new dates.",
  params: {
    data: t16.table("One row per task or milestone."),
    task: t16.field("The task's name (its key: unique)."),
    start: t16.field("Start date."),
    end: t16.field("End date (none, or the start: a milestone)."),
    group: t16.field("A phase or team: tasks gather under its heading, coloured by it."),
    milestone: t16.field("A true/false field marking milestones (default: rows without an end, or ending when they start)."),
    progress: t16.field("How far along, 0\u20131: that share of the bar is solid, the rest pale."),
    color: t16.field("Colour bars by this field instead of the group (categorical)."),
    today: t16.prop('Where today is: a date (`"2026-03-16"`) or an expression; a line across the plan.'),
    todayLabel: t16.string("Today", "The today line's label."),
    format: t16.string("%-d %b", "Date format in tooltips (strftime).")
  },
  tokens: ["ink", "ink-2", "muted", "mark", "accent", "paper", "categorical", "grid", "size.body", "size.label", "size.small"],
  expand(p, cx) {
    const [S, E] = [fld(p.start), fld(p.end)];
    const ms = p.milestone ? `(${fld(p.milestone)} == true)` : `(${E} == null || ${E} == ${S})`;
    const base = cx.table("gantt-base", p.data, op11.derive("__s", e16(S)), op11.derive("__e", e16(`${E} ?? ${S}`)));
    const byGroup = !!p.group;
    const tasks = [
      op11.derive("__id", e16(`String(${fld(p.task)})`)),
      op11.derive("__kind", e16(`${ms} ? "milestone" : "task"`)),
      op11.derive("__one", 1),
      op11.window("cumsum", "__one", "__ti"),
      op11.derive("__ord", 1)
    ];
    let T;
    if (byGroup) {
      const groups = cx.table("gantt-groups", base, op11.aggregate([p.group], { __gs: ["min", "__s"], __ge: ["max", "__e"] }), op11.derive("__one", 1), op11.window("cumsum", "__one", "__go"));
      const heads = cx.table("gantt-heads", groups, op11.derive(p.task, e16(`String(${fld(p.group)})`)), op11.derive("__id", e16(`"group:" + String(${fld(p.group)})`)), op11.derive("__kind", "group"), op11.derive("__ord", 0), op11.derive("__ti", 0), op11.derive("__s", e16("d.__gs")), op11.derive("__e", e16("d.__ge")));
      T = cx.table("gantt", base, ...tasks, op11.join(groups, p.group), op11.union(heads), op11.sort("__go", "__ord", "__ti"));
    } else {
      T = cx.table("gantt", base, ...tasks);
    }
    const indent = byGroup ? 12 : 0;
    const Tl = cx.table("gantt-labels", T, op11.derive("__lw", e16(`d.__kind == "group" ? measure(String(${fld(p.task)}), token("size.body"), 600) : measure(String(${fld(p.task)}), token("size.label")) + ${indent}`)));
    const colorField = p.color ?? p.group;
    const y = "scale.y(d.__id)", bw = "scale.y.bandwidth()";
    const x0 = "scale.x(d.__s)", x1 = "scale.x(d.__e)";
    const fill = colorField ? e16(`scale.color(${fld(colorField)})`) : "$mark";
    const when = (fmt) => `formatDate(d.__s, ${q2(fmt)}) + (d.__kind == "milestone" ? "" : " \u2013 " + formatDate(d.__e, ${q2(fmt)}))`;
    const said = `String(${fld(p.task)}) + ": " + ${when(p.format)}` + (p.progress ? ` + (${fld(p.progress)} == null || d.__kind != "task" ? "" : " (" + format(${fld(p.progress)}, ".0%") + " done)")` : "");
    const pr = p.progress ? `clamp(${fld(p.progress)} ?? 0, 0, 1)` : null;
    const hh = `min(${bw} / 2, 8)`;
    const cyv = `${y} + ${bw} / 2`;
    const today = p.today !== void 0 && p.today !== null ? vsrc(p.today) : null;
    const tx = today ? `scale.x(${today})` : "0";
    const scales = {
      x: { type: "time", domain: { data: T, fields: ["__s", "__e"] }, range: { box: "gantt-area", axis: "x" }, nice: true },
      y: { type: "band", domain: { data: T, field: "__id" }, range: { box: "gantt-area", axis: "y" }, padding: 0.32 }
    };
    if (colorField) scales.color = { type: "categorical", domain: { data: p.data, field: colorField }, range: "$categorical" };
    return group15({
      key: "gantt",
      scales,
      layout: { type: "columns", gap: 12 },
      semantics: { role: "group", label: "Plan" },
      children: [
        // Names in a column as wide as the widest (up to a third of the box: longer ones clip).
        group15({ key: "labels", size: { w: e16(`min(box.w * 0.34, table.max(${q2(Tl)}, "__lw"))`) }, clip: "box", children: [repeat12(T, text14(e16(`String(${fld(p.task)})`), [e16(`d.__kind == "group" ? 0 : ${indent}`), e16(cyv)], {
          key: e16("d.__id"),
          style: { size: e16('d.__kind == "group" ? token("size.body") : token("size.label")'), weight: e16('d.__kind == "group" ? 600 : 400'), ink: e16('d.__kind == "group" ? "$ink" : "$ink-2"'), baseline: "middle" },
          semantics: { role: "decoration" }
        }))] }),
        group15({ key: "center", layout: { type: "rows", gap: 6 }, children: [
          group15({ id: "gantt-area", key: "area", children: [
            grid({ scale: "x", orient: "vertical" }),
            repeat12(T, group15({ key: e16("d.__id"), children: [
              shape15(geom15.rect({ x: e16(x0), y: e16(`${cyv} - 2`), w: e16(`max(1, ${x1} - ${x0})`), h: 4, r: 1 }), { key: "span", when: e16('d.__kind == "group"'), fill: "$ink-2", opacity: 0.55, semantics: { role: "decoration" } }),
              shape15(geom15.rect({ x: e16(x0), y: e16(y), w: e16(`max(2, ${x1} - ${x0})`), h: e16(bw), r: 3 }), { key: "bar", when: e16('d.__kind == "task"'), fill, opacity: pr ? 0.35 : void 0, semantics: { role: "datum", label: e16(said) }, pickable: true }),
              pr ? shape15(geom15.rect({ x: e16(x0), y: e16(y), w: e16(`max(0, ${x1} - ${x0}) * ${pr}`), h: e16(bw), r: 3 }), { key: "done", when: e16(`d.__kind == "task" && ${pr} > 0`), fill, semantics: { role: "decoration" } }) : null,
              shape15(geom15.path(e16(`\`M \${${x0}} \${${cyv} - ${hh}} L \${${x0} + ${hh}} \${${cyv}} L \${${x0}} \${${cyv} + ${hh}} L \${${x0} - ${hh}} \${${cyv}} Z\``)), { key: "diamond", when: e16('d.__kind == "milestone"'), fill: "$ink", stroke: { paint: "$paper", width: 1 }, semantics: { role: "datum", label: e16(said) }, pickable: true })
            ] })),
            today ? group15({ key: "today", when: e16(`${tx} >= 0 && ${tx} <= box.w`), semantics: { role: "annotation", label: `${p.todayLabel}` }, children: [
              shape15(geom15.segment({ x1: e16(tx), y1: 0, x2: e16(tx), y2: e16("box.h") }), { key: "line", stroke: { paint: "$accent", width: 1.5, dash: [4, 3] } }),
              p.todayLabel ? text14(p.todayLabel, [e16(`${tx} + 4 + measure(${q2(p.todayLabel)}, token("size.small"), 600) > box.w ? ${tx} - 4 : ${tx} + 4`), 0], { key: "label", style: { size: "$size.small", weight: 600, ink: "$accent", baseline: "top", align: e16(`${tx} + 4 + measure(${q2(p.todayLabel)}, token("size.small"), 600) > box.w ? "end" : "start"`) }, halo: ["$paper", 2] }) : null
            ] }) : null
          ] }),
          axis({ scale: "x", orient: "bottom", type: "time" }, { size: { h: "auto" } })
        ] })
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "left" } }]
});
var timeline = recipe16({
  id: "@datars/std/timeline",
  doc: "Events on a time axis: a dot per event and its date and label on a stem, packed into as few lanes above and below the line as keep every label clear of the others (an engine op, so it re-packs for a phone); labels that still don't fit the height are left to the dot's tooltip. Optional eras shade spans of the axis with their names, under it or over it, in rows: an era inside another stacks past it (phases, and the sprints in them), eras that overlap take rows of their own.",
  params: {
    data: t16.table("One row per event."),
    date: t16.field('When (a date, or a number such as a year with `xType: "linear"`).'),
    label: t16.field("What happened (keep it short: one line)."),
    xType: t16.oneOf(["time", "linear"], "time", "`time` for dates, `linear` for numbers (years)."),
    dateFormat: t16.string(void 0, "How the date reads above each label: strftime for dates (default `%Y`), d3-format for numbers (default `d`)."),
    color: t16.field("Colour the dots by this field (categorical)."),
    eras: t16.table("Spans along the axis (optional): one row each with a start, an end and a name."),
    eraStart: t16.string("start", "The eras' start field."),
    eraEnd: t16.string("end", "The eras' end field."),
    eraLabel: t16.string("label", "The eras' name field."),
    eraLevel: t16.string(void 0, "The eras' row field (optional): 0 next to the axis, 1 past it, and so on \u2014 quarters on one row and phases on the next, whatever they overlap. Default: packed, each era in the first row free over its span, so an era inside another sits a row past it."),
    eraColor: t16.string(void 0, "Colour the eras by this field (categorical: a phase, a status, a team). Default: each era by its name."),
    eraSide: t16.oneOf(["below", "above"], "below", "Which side of the axis the eras sit on; the labels on that side start past them.")
  },
  tokens: ["ink", "ink-2", "muted", "rule", "paper", "categorical", "size.body", "size.small"],
  expand(p, cx) {
    const D = fld(p.date);
    const time = p.xType !== "linear";
    const dateOf = (v) => time ? `formatDate(${v}, ${q2(p.dateFormat ?? "%Y")})` : `format(${v}, ${q2(p.dateFormat ?? "d")})`;
    const dateText = dateOf(D);
    const labelText = `String(${fld(p.label)} ?? "")`;
    const ext = [cx.table("timeline-extent", p.data, op11.aggregate([], { __t0: ["min", p.date], __t1: ["max", p.date] }))];
    if (p.eras) ext.push(cx.table("timeline-era-extent", p.eras, op11.aggregate([], { __t0: ["min", p.eraStart], __t1: ["max", p.eraEnd] })));
    const extent = ext.length > 1 ? cx.table("timeline-span", ext[0], op11.union(ext[1])) : ext[0];
    const body = 'token("size.body")', small = 'token("size.small")';
    const laneH = `(${small} * 1.3 + ${body} * 1.3 + 8)`;
    const stem = 12;
    const [es, ee] = [`scale.x(${fld(p.eraStart)})`, `scale.x(${fld(p.eraEnd)})`];
    const E = p.eras ? cx.table("timeline-eras", p.eras, ...p.eraLevel ? [op11.derive("__row", e16(`max(0, floor(${fld(p.eraLevel)} ?? 0))`))] : [op11.derive("__s", e16(es)), op11.derive("__e", e16(ee)), op11.sort(["__s", "asc"], ["__e", "desc"]), op11.lanes({ start: e16("d.__s"), end: e16("d.__e"), as: "__row" })]) : null;
    const eraH = 16, eraPitch = 19, eraOff = 5;
    const eraGap = E ? `(${eraOff - eraPitch + eraH + 1} + (max(0, table.max(${q2(E)}, "__row")) + 1) * ${eraPitch})` : "0";
    const over = p.eraSide === "above";
    const gapUp = E && over ? eraGap : "0", gapDown = E && !over ? eraGap : "0";
    const L = cx.table(
      "timeline",
      p.data,
      op11.derive("__x", e16(`scale.x(${D})`)),
      op11.derive("__w", e16(`max(measure(${labelText}, ${body}), measure(${dateText}, ${small})) + 4`)),
      op11.derive("__a", e16("d.__x + d.__w > box.w ? d.__x - d.__w : d.__x")),
      op11.lanes({ start: e16("d.__a"), end: e16("d.__a + d.__w"), gap: 10, max: e16(`max(1, floor((box.h - ${2 * stem + 4} - ${eraGap}) / ${laneH}))`) })
    );
    const Lq = q2(L);
    const maxLane = `max(0, table.max(${Lq}, "lane"))`;
    const above = `(floor(${maxLane} / 2) + 1)`, below = `floor((${maxLane} + 1) / 2)`;
    const axisY = `((box.h - (${above} + ${below}) * ${laneH} - ${2 * stem} - ${eraGap}) / 2 + ${above} * ${laneH} + ${stem} + ${gapUp})`;
    const up = "(d.lane % 2 == 0)", level = "floor(d.lane / 2)";
    const edge = `(${up} ? ${axisY} - ${gapUp} - ${stem} - ${level} * ${laneH} : ${axisY} + ${gapDown} + ${stem} + ${level} * ${laneH})`;
    const block = `(${laneH} - 8)`;
    const flip = "(d.__x + d.__w > box.w)";
    const tx = `(${flip} ? d.__x - 4 : d.__x + 4)`;
    const align = e16(`${flip} ? "end" : "start"`);
    const dot2 = p.color ? e16(`scale.color(${fld(p.color)})`) : "$ink";
    const scales = { x: { type: time ? "time" : "linear", domain: { data: extent, fields: ["__t0", "__t1"] }, range: "width", nice: false } };
    if (p.color) scales.color = { type: "categorical", domain: { data: p.data, field: p.color }, range: "$categorical" };
    if (p.eras) scales.era = { type: "categorical", domain: { data: p.eras, field: p.eraColor ?? p.eraLabel }, range: "$categorical" };
    const eraName = `String(${fld(p.eraLabel)} ?? "")`;
    const eraY = over ? `(${axisY} - ${eraOff + eraH} - d.__row * ${eraPitch})` : `(${axisY} + ${eraOff} + d.__row * ${eraPitch})`;
    const placed = e16("d.lane != null");
    return group15({
      key: "timeline",
      scales,
      semantics: { role: "group", label: "Timeline" },
      children: [
        shape15(geom15.segment({ x1: 0, y1: e16(axisY), x2: e16("box.w"), y2: e16(axisY) }), { key: "axis", stroke: { paint: "$rule", width: 1.5 }, semantics: { role: "decoration" } }),
        group15({ key: "stems", children: [repeat12(L, shape15(geom15.segment({ x1: e16("d.__x"), y1: e16(axisY), x2: e16("d.__x"), y2: e16(`${edge} + (${up} ? 2 : -2)`) }), { when: placed, stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }))] }),
        // Eras on paper over the stems: a stem runs from its dot behind any era in its way. An era
        // is hovered for its name and span; the name inside it only shows where it fits.
        E ? group15({ key: "eras", children: [repeat12(E, group15({ semantics: { role: "annotation", label: e16(`${eraName} + ": " + ${dateOf(fld(p.eraStart))} + " \u2013 " + ${dateOf(fld(p.eraEnd))}`) }, children: [
          shape15(geom15.rect({ x: e16(es), y: e16(eraY), w: e16(`max(1, ${ee} - ${es})`), h: eraH, r: 2 }), { key: "paper", fill: "$paper", semantics: { role: "decoration" } }),
          shape15(geom15.rect({ x: e16(es), y: e16(eraY), w: e16(`max(1, ${ee} - ${es})`), h: eraH, r: 2 }), { key: "span", fill: e16(`scale.era(${fld(p.eraColor ?? p.eraLabel)})`), opacity: e16("hover() ? 0.45 : 0.3"), pickable: true }),
          text14(e16(eraName), [e16(`${es} + 5`), e16(`${eraY} + ${eraH / 2}`)], { key: "name", when: e16(`measure(${eraName}, ${small}, 600) + 10 <= ${ee} - ${es}`), style: { size: "$size.small", weight: 600, ink: "$ink-2", baseline: "middle" } })
        ] }))] }) : null,
        group15({ key: "labels", children: [repeat12(L, group15({ when: placed, children: [
          shape15(geom15.rect({ x: e16(`${flip} ? d.__x - d.__w : d.__x + 1`), y: e16(`${up} ? ${edge} - ${block} : ${edge}`), w: e16("d.__w - 1"), h: e16(block) }), { key: "paper", fill: "$paper", semantics: { role: "decoration" } }),
          text14(e16(dateText), [e16(tx), e16(`${up} ? ${edge} - ${body} * 1.3 : ${edge}`)], { key: "date", style: { size: "$size.small", weight: 600, ink: "$muted", align, baseline: e16(`${up} ? "bottom" : "top"`) } }),
          text14(e16(labelText), [e16(tx), e16(`${up} ? ${edge} : ${edge} + ${small} * 1.3`)], { key: "label", style: { size: "$size.body", ink: "$ink", align, baseline: e16(`${up} ? "bottom" : "top"`) } })
        ] }))] }),
        group15({ key: "dots", children: [repeat12(L, shape15(geom15.circle({ cx: e16("d.__x"), cy: e16(axisY), r: 4.5 }), { fill: dot2, stroke: { paint: "$paper", width: 1.5 }, semantics: { role: "datum", label: e16(`${dateText} + ": " + ${labelText}`) }, pickable: true }))] })
      ]
    });
  },
  motion: [{ select: { role: "datum" }, enter: { scale: 0, origin: "center" } }]
});
var US_TILES = "AK 0 0 Alaska|ME 10 0 Maine|WI 5 1 Wisconsin|VT 9 1 Vermont|NH 10 1 New Hampshire|WA 0 2 Washington|ID 1 2 Idaho|MT 2 2 Montana|ND 3 2 North Dakota|MN 4 2 Minnesota|IL 5 2 Illinois|MI 6 2 Michigan|NY 8 2 New York|MA 9 2 Massachusetts|OR 0 3 Oregon|NV 1 3 Nevada|WY 2 3 Wyoming|SD 3 3 South Dakota|IA 4 3 Iowa|IN 5 3 Indiana|OH 6 3 Ohio|PA 7 3 Pennsylvania|NJ 8 3 New Jersey|CT 9 3 Connecticut|RI 10 3 Rhode Island|CA 0 4 California|UT 1 4 Utah|CO 2 4 Colorado|NE 3 4 Nebraska|MO 4 4 Missouri|KY 5 4 Kentucky|WV 6 4 West Virginia|VA 7 4 Virginia|MD 8 4 Maryland|DE 9 4 Delaware|AZ 1 5 Arizona|NM 2 5 New Mexico|KS 3 5 Kansas|AR 4 5 Arkansas|TN 5 5 Tennessee|NC 6 5 North Carolina|SC 7 5 South Carolina|DC 8 5 District of Columbia|OK 3 6 Oklahoma|LA 4 6 Louisiana|MS 5 6 Mississippi|AL 6 6 Alabama|GA 7 6 Georgia|HI 0 7 Hawaii|TX 3 7 Texas|FL 8 7 Florida";
var EUROPE_TILES = "ISL IS 0 0 Iceland|NOR NO 4 0 Norway|SWE SE 5 0 Sweden|FIN FI 6 0 Finland|IRL IE 1 1 Ireland|GBR GB 2 1 United Kingdom|EST EE 6 1 Estonia|NLD NL 3 2 Netherlands|DNK DK 4 2 Denmark|LVA LV 6 2 Latvia|BEL BE 3 3 Belgium|DEU DE 4 3 Germany|POL PL 5 3 Poland|LTU LT 6 3 Lithuania|BLR BY 7 3 Belarus|FRA FR 2 4 France|LUX LU 3 4 Luxembourg|CZE CZ 4 4 Czechia|SVK SK 5 4 Slovakia|UKR UA 6 4 Ukraine|PRT PT 0 5 Portugal|ESP ES 1 5 Spain|CHE CH 3 5 Switzerland|AUT AT 4 5 Austria|HUN HU 5 5 Hungary|ROU RO 6 5 Romania|MDA MD 7 5 Moldova|ITA IT 3 6 Italy|SVN SI 4 6 Slovenia|HRV HR 5 6 Croatia|SRB RS 6 6 Serbia|BGR BG 7 6 Bulgaria|MLT MT 3 7 Malta|BIH BA 5 7 Bosnia and Herzegovina|MNE ME 6 7 Montenegro|MKD MK 7 7 North Macedonia|TUR TR 8 7 T\xFCrkiye|ALB AL 6 8 Albania|GRC GR 7 8 Greece|CYP CY 8 8 Cyprus";
function tileLayout(name, codes) {
  const out = { id: [], abbr: [], name: [], col: [], row: [] };
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
var tileMap = recipe16({
  id: "@datars/std/tileMap",
  doc: "A tile-grid map (a grid cartogram): every state or country the same size \u2014 a square or a hexagon \u2014 in roughly its place, coloured by value and marked with its abbreviation, so small places count as much as big ones. Built-in layouts for US states (postal codes) and European countries (ISO codes), or a layout table of your own (`id`, `col`, `row`, optional `abbr` and `name`). Tiles without data stay neutral; tiles are keyed by id, so they morph to the same region on a map.",
  params: {
    data: t16.table("Values per place."),
    key: t16.field("The data column holding each place's id (a postal code, an ISO code, your layout's `id`)."),
    value: t16.field("The value to colour by."),
    layout: t16.string("us", "`us` (50 states and DC), `europe` (40 countries), or the name of a table with columns `id`, `col`, `row` (and optional `abbr`, `name`)."),
    codes: t16.oneOf(["alpha3", "alpha2"], "alpha3", "The `europe` layout's ids: ISO 3166 alpha-3 (`DEU`, as the countries atlas) or alpha-2 (`DE`)."),
    shape: t16.oneOf(["square", "hex"], "square", "Square tiles, or hexagons (odd rows shifted half a tile)."),
    colorType: t16.oneOf(["sequential", "diverging", "categorical", "piecewise"], "sequential"),
    stops: t16.string(void 0, "Piecewise colour stops: '#22c55e 2 \xB7 #f5a524 4 \xB7 #f97362 6.5'."),
    format: t16.string(",.1~f", "Number format in tooltips, values and the legend."),
    label: t16.prop("Tile label (tooltip, accessible name): an expression over the tile row; default `name: value`."),
    values: t16.bool(false, "The value under each abbreviation, where tiles are big enough."),
    legend: t16.bool(true, "A colour ramp (or swatches) under the grid."),
    gap: t16.number(2, "Space between tiles (px).")
  },
  tokens: ["map.no-data", "sequential", "diverging", "categorical", "ink-2", "paper", "size.small"],
  expand(p, cx) {
    const V = fld(p.value);
    const vals = cx.table("tile-values", p.data, op11.derive("id", e16(`String(${fld(p.key)})`)));
    const builtIn = p.layout === "us" || p.layout === "europe" || !p.layout;
    const tiles2 = builtIn ? cx.table("tiles", p.data, op11.values(tileLayout(p.layout || "us", p.codes), { key: "id" }), op11.join(vals, "id", "left")) : cx.table("tiles", p.layout, op11.derive("id", e16("String(d.id)")), op11.join(vals, "id", "left"));
    const Tq = q2(tiles2);
    const hex = p.shape === "hex";
    const legendH = p.legend ? 34 : 0;
    const C = `(table.max(${Tq}, "col") + 1)`, R = `(table.max(${Tq}, "row") + 1)`;
    const size = hex ? `min(box.w / ((${C} + 0.5) * 1.7320508), (box.h - ${legendH}) / ((${R} - 1) * 1.5 + 2))` : `min(box.w / ${C}, (box.h - ${legendH}) / ${R})`;
    const gw = hex ? `((${C} + 0.5) * 1.7320508 * d.__s)` : `(${C} * d.__s)`;
    const gh = hex ? `(((${R} - 1) * 1.5 + 2) * d.__s)` : `(${R} * d.__s)`;
    const geo = cx.table(
      "tile-grid",
      tiles2,
      op11.derive("__s", e16(size)),
      op11.derive("__ox", e16(`(box.w - ${gw}) / 2`)),
      op11.derive("__oy", e16(`(box.h - ${legendH} - ${gh}) / 2`)),
      op11.derive("__cx", e16(hex ? "d.__ox + (d.col + 0.5 + (d.row % 2) * 0.5) * 1.7320508 * d.__s" : "d.__ox + (d.col + 0.5) * d.__s")),
      op11.derive("__cy", e16(hex ? "d.__oy + d.__s + d.row * 1.5 * d.__s" : "d.__oy + (d.row + 0.5) * d.__s"))
    );
    const g = Math.max(0, p.gap ?? 2);
    const side = hex ? "(d.__s * 1.7320508)" : "d.__s";
    const color = p.colorType === "categorical" ? `scale.color(${V})` : `scale.color(${V})`;
    const fill = `(${V} == null ? "$map.no-data" : ${color})`;
    const rr = `max(1, d.__s - ${g / 1.7320508})`;
    const hexPath = `\`M \${d.__cx} \${d.__cy - ${rr}} L \${d.__cx + ${rr} * 0.8660254} \${d.__cy - ${rr} * 0.5} L \${d.__cx + ${rr} * 0.8660254} \${d.__cy + ${rr} * 0.5} L \${d.__cx} \${d.__cy + ${rr}} L \${d.__cx - ${rr} * 0.8660254} \${d.__cy + ${rr} * 0.5} L \${d.__cx - ${rr} * 0.8660254} \${d.__cy - ${rr} * 0.5} Z\``;
    const tileGeom = hex ? geom15.path(e16(hexPath)) : geom15.rect({ x: e16(`d.__cx - (d.__s - ${g}) / 2`), y: e16(`d.__cy - (d.__s - ${g}) / 2`), w: e16(`max(1, d.__s - ${g})`), h: e16(`max(1, d.__s - ${g})`), r: 2 });
    const name = "(d.name ?? d.abbr ?? d.id)";
    const label = p.label ?? e16(`${name} + ": " + (${V} == null ? "no data" : format(${V}, ${q2(p.format)}))`);
    const abbr = "String(d.abbr ?? d.id)";
    const fs = `clamp(${side} * 0.3, 8, 14)`;
    const withValue = p.values ? `(${side} >= 38 && ${V} != null)` : "false";
    const scales = {
      color: p.colorType === "piecewise" ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } } : { type: p.colorType, domain: { data: p.data, field: p.value }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" }
    };
    const pinned = p.colorType === "piecewise" && p.stops ? p.stops.split(/[\s·,;]+/).filter((s) => s !== "" && !Number.isNaN(Number(s))).map(Number) : [];
    const ox = `(box.w - ${gw.replace(/d\.__s/g, `(${size})`)}) / 2`;
    const gh0 = gh.replace(/d\.__s/g, `(${size})`);
    const by = `((box.h - ${legendH} - ${gh0}) / 2 + ${gh0})`;
    const ext = pinned.length >= 2 ? cx.table("tile-extent", p.data, op11.aggregate([], { n: ["count"] }), op11.derive("lo", pinned[0]), op11.derive("hi", pinned[pinned.length - 1]), op11.derive("__ox", e16(ox))) : cx.table("tile-extent", p.data, op11.aggregate([], { lo: ["min", p.value], hi: ["max", p.value] }), op11.derive("__ox", e16(ox)));
    const steps = 6, sw = 22;
    const legendNode = !p.legend ? null : p.colorType === "categorical" ? group15({ key: "legend", transform: { translate: [e16(`max(0, ${ox})`), e16(`${by} + 12`)] }, layout: { type: "flow", gap: 14 }, semantics: { role: "legend", label: "Legend" }, children: [
      repeat12({ legend: "color" }, group15({ semantics: { role: "legend-item", label: e16("key.name(d.label)") }, children: [
        shape15(geom15.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e16("d.ink") }),
        text14(e16("key.name(d.label)"), [15, 9], { style: { size: "$size.small", ink: "$ink-2" } })
      ] }))
    ] }) : group15({ key: "legend", semantics: { role: "legend", label: "Colour scale" }, children: [repeat12(ext, group15({ key: "ramp", transform: { translate: [e16("max(0, d.__ox)"), e16(`${by} + 10`)] }, children: [
      ...Array.from({ length: steps }, (_, i) => shape15(geom15.rect({ x: i * sw, y: 0, w: sw, h: 8 }), { key: `step-${i}`, fill: e16(`scale.color(d.lo + (d.hi - d.lo) * ${i / (steps - 1)})`), semantics: { role: "decoration" } })),
      text14(e16(`format(d.lo, ${q2(p.format)})`), [0, 20], { key: "lo", style: { size: "$size.small", ink: "$ink-2" } }),
      text14(e16(`format(d.hi, ${q2(p.format)})`), [steps * sw, 20], { key: "hi", style: { size: "$size.small", ink: "$ink-2", align: "end" } })
    ] }))] });
    return group15({
      key: "tile-map",
      scales,
      semantics: { role: "group", label: "Tile map" },
      children: [
        group15({ key: "tiles", children: [repeat12(geo, group15({ key: e16("d.id"), children: [
          shape15(tileGeom, { key: "tile", fill: e16(fill), semantics: { role: "region", label, value: e16(V) }, pickable: true }),
          text14(e16(abbr), [e16("d.__cx"), e16(`d.__cy - (${withValue} ? ${fs} * 0.45 : 0)`)], { key: "abbr", when: e16(`${side} >= 16`), style: { size: e16(fs), weight: 600, ink: e16(`"on(" + ${fill} + ")"`), align: "middle", baseline: "middle" } }),
          p.values ? text14(e16(`format(${V}, ${q2(p.format)})`), [e16("d.__cx"), e16(`d.__cy + ${fs} * 0.6`)], { key: "value", when: e16(withValue), style: { size: e16(`${fs} * 0.8`), ink: e16(`"on(" + ${fill} + ")"`), align: "middle", baseline: "middle" } }) : null
        ] }))] }),
        legendNode
      ]
    });
  }
});
export {
  annotate,
  arcDiagram,
  area,
  attribution,
  axis,
  bar,
  basemap,
  bollinger,
  boxplot,
  bullet,
  bump,
  button,
  calendar,
  candlestick,
  card,
  cell,
  checklist,
  chord,
  cloud,
  comparisonFrame,
  connectedScatter,
  contours,
  dataTable,
  dendrogram,
  donut,
  dot,
  dotDensity,
  drawdown,
  dumbbell,
  errorBars,
  facet,
  financeDomain,
  financeKey,
  funnel,
  gantt,
  gauge,
  geoLines,
  geoPoints,
  grid,
  grouped,
  heatmap2d,
  hemicycle,
  hexbin,
  histogram,
  icicle,
  indexed,
  kpi,
  legend,
  line,
  lollipop,
  manyLines,
  map,
  marimekko,
  matrix,
  movingAverage,
  network,
  ohlc,
  pareto,
  pie,
  plot,
  point,
  progress,
  pyramid,
  radar,
  range,
  ridgeline,
  route,
  rule,
  sankey,
  segmented,
  select,
  slider,
  slope,
  span,
  sparkline,
  stacked,
  stackedArea,
  stripes,
  sunburst,
  swarm,
  symbols,
  tickCount,
  tileMap,
  timeline,
  title,
  toggle,
  track,
  tree,
  treemap,
  violin,
  volume,
  waffle,
  waterfall
};
