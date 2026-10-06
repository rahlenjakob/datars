import { test } from "node:test";
import assert from "node:assert/strict";
import { doc, data, story, step, e } from "@datars/sdk";
import { plot, bar, pie, axis, line, grouped, stacked, map, symbols, area as stdArea, geoLines, track, card, rule, span, annotate, waterfall, hemicycle, candlestick, volume, movingAverage, bollinger, indexed, drawdown, sparkline, legend, title, treemap, calendar, swarm, dotDensity } from "../dist/index.js";

test("std functions build `use` nodes; __expand returns templates", () => {
  const scene = plot({ data: "votes", x: "party", y: "share", title: "Vote share", children: [bar({ labels: true })] });
  assert.equal(scene.kind, "use");
  assert.equal(scene.recipe, "@datars/std/plot");
  const d = doc({ title: "t", size: [640, 400], data: { votes: data.values({ party: ["S", "SD"], share: [30.3, 20.5] }, { key: "party" }) }, scene, program: story({ steps: [step("bars")] }) });
  assert.equal(d.datars, 1);
  assert.deepEqual(d.size, { width: 640, height: 400 });
  const out = plot.__expand(scene.params, { size: [640, 400], sizeClass: "wide" });
  assert.equal(out.template.kind, "group");
  assert.equal(out.template.scales.x.type, "band");
  assert.deepEqual(out.template.scales.y.range, { box: "plot-area", axis: "-y" });
  const json = JSON.stringify(out.template);
  assert.ok(json.includes('"recipe":"@datars/std/bar"'), "children use nodes inherit");
  assert.ok(json.includes('"data":"votes"'));
});

test("bar expands to a repeat with keyed shapes and expressions", () => {
  const out = bar.__expand({ data: "votes", x: "party", y: "share", xType: "band", yType: "linear", labels: true }, {});
  const rep = out.template.children[0];
  assert.equal(rep.kind, "repeat");
  assert.equal(rep.from, "votes");
  assert.equal(rep.template.kind, "shape", "datum shapes are keyed by the datum (directly under the repeat)");
  assert.ok(JSON.stringify(rep).includes("scale.x(d.party)"));
  assert.ok(JSON.stringify(out.template.children[1]).includes('"number"'), "value labels count, in a sibling repeat");
});

test("pie registers a derived table through cx.table", () => {
  const out = pie.__expand({ data: "votes", value: "share", category: "party" }, {});
  const names = Object.keys(out.tables);
  assert.equal(names.length, 1);
  assert.equal(out.tables[names[0]].ops[0].op, "pie");
});

test("recipes describe themselves (for docs, inspectors, agents)", () => {
  const d = axis.describe();
  assert.equal(d.id, "@datars/std/axis");
  assert.ok(d.params.orient.values.includes("bottom"));
});

// ---- what the coverage corpus needs (docs/15) ------------------------------------------------------

test("grouped dodges series inside each band, keyed series@x", () => {
  const out = grouped.__expand({ data: "t", x: "year", y: "v", series: "party" }, {});
  assert.deepEqual(out.template.scales.dodge.range, [0, "=scale.x.bandwidth()"]);
  const rep = out.template.children[0];
  assert.deepEqual(rep.template.key, [{ expr: "d.party" }, { expr: "d.year" }], "a two-part key: series, then x");
  assert.ok(JSON.stringify(rep).includes("scale.dodge(d.party)"));
});

test("stacked goes horizontal when y is the band axis, and takes a segment key", () => {
  const out = stacked.__expand({ data: "t", x: "value", y: "whole", series: "party", xType: "linear", yType: "band", offset: "expand", segmentKey: { expr: "d.party" } }, {});
  const stack = Object.values(out.tables)[0].ops[0];
  assert.equal(stack.x, "whole", "stacks along the band field");
  assert.equal(stack.value, "value");
  const rep = out.template.children[0];
  assert.equal(rep.template.geom.x.expr, "scale.x(d.y0)");
  assert.equal(rep.template.key.expr, "d.party");
});

test("plot: piecewise colour stops, a clipped plot area, room for end labels", () => {
  const out = plot.__expand({ data: "t", x: "k", y: "v", color: "v", colorType: "piecewise", stops: "#fff 0 · #000 10", clip: true, children: [bar({})] }, {});
  assert.equal(out.template.scales.color.type, "piecewise");
  assert.equal(out.template.scales.color.stops, "#fff 0 · #000 10");
  assert.ok(JSON.stringify(out.template).includes('"clip":"box"'), "the plot area clips");
  // Labelled lines: the area can't clip (labels sit past its edge), so the marks clip themselves.
  const lines = JSON.stringify(plot.__expand({ data: "t", x: "k", y: "v", color: "s", clip: true, children: [line({ labels: true })] }, {}).template);
  assert.ok(lines.includes('"key":"end-labels"'), "room at the right");
  assert.ok(!lines.includes('"clip":"box"') && lines.includes('"clip":true'), "clip handed to the line");
});

test("line labels sit at each series' last point, spread apart", () => {
  const out = line.__expand({ data: "t", x: "year", y: "v", color: "party", labels: true, clip: true }, {});
  const [lines, labels] = out.template.children;
  assert.equal(out.template.clip, undefined, "the labels sit past the plot area's edge, unclipped…");
  assert.equal(lines.template.clip, "box", "…while each line clips itself");
  assert.equal(labels.key, "end-labels");
  const ops = Object.values(out.tables)[0].ops;
  assert.deepEqual(ops.map((o) => o.op), ["aggregate", "spread"]);
  assert.deepEqual(ops[0].ops.map((o) => o.op), ["last", "last"]);
  assert.equal(labels.children[0].template.at[1].expr, "d.label_y");
});

test("map with a camera: layers move in a view, the legend stays put; selection recedes by colour", () => {
  const camera = { fit: { keys: ["SWE"] }, padding: 16 };
  const out = map.__expand({ source: "world", data: "v", key: "id", value: "share", camera, legend: true, selected: "focus", under: "land" }, {});
  const [view, legend] = out.template.children;
  assert.equal(view.kind, "view");
  assert.deepEqual(view.camera, camera);
  assert.equal(view.coord.type, "geo", "the view carries the map's coordinates (geo.x in camera boxes)");
  assert.equal(view.children[0].key, "under");
  assert.equal(legend.key, "legend");
  assert.ok(JSON.stringify(view).includes("focus.has(d.id) ?"), "unselected regions change colour");
  const plain = map.__expand({ source: "world" }, {});
  assert.equal(plain.template.coord.type, "geo", "without a camera the map group keeps its coordinates");
});

test("map base layers sit beneath the regions and move with the camera; regions can be left out", () => {
  const base = [{ kind: "use", recipe: "@datars/std/basemap", params: { source: "tiles", part: "base" } }];
  const camera = { fit: {}, padding: 0 };
  const [view] = map.__expand({ source: "world", camera, base }, {}).template.children;
  assert.deepEqual(view.children.map((c) => c.key), ["base", "regions"]);
  assert.equal(view.children[0].children[0].recipe, "@datars/std/basemap");
  const beat = map.__expand({ source: "world", camera, base, regions: false }, {}).template.children[0];
  assert.deepEqual(beat.children.map((c) => c.key), ["base"], "a camera beat over the basemap");
});

test("pie labels keep inside the box: radius fitted to measured labels, a legend when too narrow", () => {
  const out = pie.__expand({ data: "votes", value: "share", category: "party", labels: true }, {});
  const slices = Object.values(out.tables)[0].ops;
  assert.equal(slices[1].op, "derive");
  assert.ok(slices[1].expr.expr.startsWith("measure(key.name(d.party)"), "label widths measured by the engine");
  const json = JSON.stringify(out.template);
  assert.ok(json.includes('table.max('), "the radius leaves room for the widest label");
  const legend = out.template.children.find((c) => c.key === "legend");
  assert.ok(legend && legend.when.expr.includes("< min(box.w, box.h) * 0.2"), "legend fallback when labels don't fit");
  const plain = pie.__expand({ data: "votes", value: "share", category: "party", labels: false }, {});
  assert.ok(!JSON.stringify(plain.template).includes('"key":"legend"'));
});

test("plot brush binds the brush intent on the plot area, along the category/time axis", () => {
  const out = plot.__expand({ data: "t", x: "year", y: "v", xType: "linear", brush: "range", children: [] }, {});
  const json = JSON.stringify(out.template);
  assert.ok(json.includes('"on":{"brush":{"brush":"range","axis":"x"}}'), json);
  const h = JSON.stringify(plot.__expand({ data: "t", x: "v", y: "k", xType: "linear", yType: "band", brush: "sel", children: [] }, {}).template);
  assert.ok(h.includes('"axis":"y"'), "horizontal bars brush along y");
});

test("plot brush shades the brushed range; areas fill to the plot edge when zero is off the axis", () => {
  const out = plot.__expand({ data: "prices", x: "day", y: "price", xType: "linear", brush: "range", children: [] }, {});
  const json = JSON.stringify(out.template);
  assert.ok(json.includes('"key":"brush"') && json.includes("range.active"), "an extent shown only while brushed");
  assert.ok(json.includes("scale.x(range.lo)"), "along the brushed axis");
  const area = JSON.stringify(stdArea.__expand({ data: "prices", x: "day", y: "price" }, {}).template);
  assert.ok(area.includes("max(0, min(box.h, scale.y(0)))"), "the baseline is clamped into the plot");
});

test("geoLines with data draws a line per row, as data, coloured on value stops", () => {
  const plain = JSON.stringify(geoLines.__expand({ source: "roads" }, {}).template);
  assert.ok(plain.includes('"role":"decoration"') && plain.includes('"from":"roads"'), "without data: every feature, as scenery");
  const t = geoLines.__expand({ source: "routes", data: "vals", key: "label", value: "value", stops: "#aaa 0 · #000 10" }, {}).template;
  const json = JSON.stringify(t);
  assert.equal(t.scales.stroke.type, "piecewise");
  assert.ok(json.includes('"from":"vals"') && json.includes("scale.stroke(d.value)") && json.includes('"role":"datum"'), json);
});

test("symbols without a value field are all one size", () => {
  const out = symbols.__expand({ source: "cities", data: "cities", key: "id", r: 3 }, {});
  assert.equal(out.template.scales, undefined);
  assert.equal(out.template.children[0].r, 3);
});

test("a track's head is its path's own end, so it rides the tip as the path grows", () => {
  const out = track.__expand({ data: "t", time: "t", lon: "lon", lat: "lat", until: { expr: "time" } }, {});
  const [path] = out.template.children;
  assert.equal(path.key, "path");
  assert.deepEqual(path.markers, { end: { type: "dot", r: 5 } });
  assert.ok(!out.template.children.some((c) => c.kind === "instances"), "no separate head to fly in a straight line");
  const bare = track.__expand({ data: "t", time: "t", lon: "lon", lat: "lat", head: false }, {});
  assert.equal(bare.template.children[0].markers, undefined);
});

// ---- what articles built with std needed ------------------------------------------------------------

test("card: dodging tries every anchor, asked-for first; not dodging, its anchor is its only place", () => {
  const exp = (params) => card.__expand({ text: e("narration.text"), ...params }, {}).template;
  assert.deepEqual(exp({ at: "left" }).dodge, ["left", "top-right", "top-left", "bottom-right", "bottom-left", "right", "top", "bottom"]);
  assert.deepEqual(exp({ at: "left", dodge: false }).dodge, ["left"], "one anchor: fixed (the engine never bands it)");
  assert.deepEqual(exp({ dodge: false }).dodge, ["top-right"], "auto, not dodging: the first preference");
  // An expression anchor, evaluated per state (auto reads as the first preference).
  const moving = exp({ at: e("corner"), dodge: false }).dodge;
  assert.equal(moving.length, 1);
  assert.ok(moving[0].expr.includes('(corner) == "auto" ? "top-right" : (corner)'), moving[0].expr);
  assert.equal(exp({ at: e("corner") }).dodge.length, 9, "the expression first, then every anchor");
  assert.deepEqual(exp({ margin: [12, 12, 30, 12] }).layout.padding, [12, 12, 30, 12], "a margin per side");
});

test("card: an empty kicker or title is not drawn (no empty line at the top)", () => {
  const box = card.__expand({ kicker: e("narration.title"), title: "Always", text: e("narration.text") }, {}).template.children[0];
  const [kicker, title, body] = box.children;
  assert.equal(kicker.when.expr, '(narration.title) != ""');
  assert.equal(title.when, undefined, "a literal title always has text");
  assert.equal(body.when, undefined, "the body hides the whole card instead");
  const json = card.__expand({ kicker: "=narration.title", text: "=narration.text" }, {}).template;
  assert.equal(json.when.expr, '(narration.text) != ""', "'=…' strings are expressions too (hand-written JSON)");
  assert.equal(json.children[0].children[0].when.expr, '(narration.title) != ""');
});

test("axis: band names wrap, turn 45° or thin by their measured widths; one line's height up the side", () => {
  const out = axis.__expand({ scale: "x", orient: "bottom", data: "votes", field: "party" }, {});
  const [name] = Object.keys(out.tables);
  const [names] = Object.values(out.tables);
  assert.equal(names.from, "votes");
  assert.deepEqual(names.ops.map((o) => o.op), ["derive", "derive"]);
  assert.ok(names.ops[0].expr.expr.startsWith("measure(key.name(d.party)"), "whole names measured by the engine");
  assert.ok(names.ops[1].expr.expr.startsWith("measure.word(key.name(d.party)"), "and their longest words");
  const tick = (t) => t.children.find((c) => c.kind === "repeat").template;
  const [, , flat, turned] = tick(out.template).children;
  // Flat labels wrap in as many bands as their longest word needs; turned ones when that's more
  // than half again the bands a turned line needs.
  assert.ok(flat.when.expr.includes(`ceil((table.max(${JSON.stringify(name)}, "ww") + 4) / scale.x.step())`), flat.when.expr);
  assert.ok(flat.style.max_width.expr.includes("* scale.x.step() - 4"), "wrapped in the bands it may use");
  assert.equal(turned.rotate, -45);
  assert.equal(turned.style.align, "end", "a turned name ends at its tick");
  assert.ok(turned.when.expr.includes("* 1.5 <"), turned.when.expr);
  const side = axis.__expand({ scale: "y", orient: "left", data: "votes", field: "party" }, {});
  assert.deepEqual(side.tables, {}, "a vertical axis needs heights, not widths");
  assert.ok(tick(side.template).children[2].when.expr.includes('ceil((token("size.label") + 1) / scale.y.step())'));
  assert.equal(tick(side.template).children[3], undefined, "names up the side never turn");
  // The plot hands its band x axis the names; from a facet's shared domain when there is one.
  const xAxis = (params) => {
    let found;
    const walk = (n) => { if (n && typeof n === "object") { if (n.recipe === "@datars/std/axis" && n.params.scale === "x") found = n.params; Object.values(n).forEach(walk); } };
    walk(plot.__expand({ data: "v", x: "k", y: "n", children: [], ...params }, {}).template);
    return [found.data, found.field];
  };
  assert.deepEqual(xAxis({}), ["v", "k"]);
  assert.deepEqual(xAxis({ data: "@group", xDomain: { data: "all", field: "k" } }), ["all", "k"]);
  assert.deepEqual(xAxis({ xType: "linear" }), [undefined, undefined], "no names to measure on a linear axis");
});

test("plot: per-axis formats and tick counts; a whole-number format draws whole-number ticks only", () => {
  const axes = [];
  const walk = (n) => { if (n && typeof n === "object") { if (n.recipe === "@datars/std/axis") axes.push(n.params); Object.values(n).forEach(walk); } };
  walk(plot.__expand({ data: "r", x: "t", y: "v", xType: "linear", xFormat: "d", xTicks: 5, yFormat: ".1f", format: ".0f", children: [] }, {}).template);
  const [y, x] = axes;
  assert.deepEqual([x.format, x.ticks, y.format, y.ticks], ["d", 5, ".1f", undefined], "each axis its own format (yFormat over format) and count");
  const whole = (format) => JSON.stringify(axis.__expand({ scale: "x", orient: "bottom", format }, {}).template).includes("abs(d.value - round(d.value))");
  assert.deepEqual(["d", ",d", ".0f", ",.0~f", ".1f", ".0%", "$,.2s"].map(whole), [true, true, true, true, false, false, false]);
});

test("grouped and stacked: value labels on request, shown only where they fit", () => {
  const g = grouped.__expand({ data: "t", x: "k", y: "v", series: "s", labels: true, format: ".1f", suffix: " %" }, {}).template;
  const gl = g.children.find((c) => c.key === "labels");
  assert.ok(gl.children[0].template.when.expr.includes("<= scale.dodge.step() - 2"), "fits its bar's slot");
  assert.ok(JSON.stringify(gl).includes('\\" %\\"'), "with its suffix");
  assert.equal(grouped.__expand({ data: "t", x: "k", y: "v", series: "s" }, {}).template.children.length, 1, "no labels unless asked");
  const s = stacked.__expand({ data: "t", x: "v", y: "k", series: "s", xType: "linear", yType: "band", labels: true, format: ".0f" }, {}).template;
  const sl = s.children.find((c) => c.key === "labels").children[0].template;
  assert.equal(sl.at[0].expr, "(scale.x(d.y0) + scale.x(d.y1)) / 2", "in the middle of its segment");
  assert.ok(sl.when.expr.startsWith("abs(scale.x(d.y1) - scale.x(d.y0)) >= measure("), "as long as the label needs");
  assert.equal(sl.number.format, ".0f", "a number that counts");
});

test("rule: an x line's label turns to the left of the line where it would leave the box", () => {
  const [, label] = rule.__expand({ axis: "x", value: 175, label: "175" }, {}).template.children;
  assert.ok(label.style.align.expr.includes('measure("175", token("size.label")) > box.w'), label.style.align.expr);
  assert.ok(label.at[0].expr.includes("- 4 :"));
});

test("axis: a horizontal numeric axis asks for as many ticks as its measured labels fit; the grid takes the same count", () => {
  const out = axis.__expand({ scale: "x", orient: "bottom", type: "linear", format: ",.0f", suffix: " kr" }, {});
  const repeat = out.template.children.find((c) => c.kind === "repeat");
  const count = repeat.from.count.expr;
  assert.ok(count.startsWith("max(2, min(10, round((scale.x.max() - scale.x.min()) / 64), floor("), count);
  assert.ok(count.includes('format(scale.x.invert(scale.x.max()), ",.0f") + " kr"'), "the domain's end as the axis writes it");
  assert.ok(repeat.template.children[1].when.expr.includes("d.count - 1"), "labels leave out every k-th tick should the scale make more");
  assert.equal(axis.__expand({ scale: "x", orient: "bottom", type: "time" }, {}).template.children.find((c) => c.kind === "repeat").from.count, undefined, "time: the scale's own");
  assert.equal(axis.__expand({ scale: "y", orient: "left", type: "linear" }, {}).template.children.find((c) => c.kind === "repeat").from.count, undefined, "up the side: by length");
  assert.equal(axis.__expand({ scale: "x", orient: "bottom", type: "linear", ticks: 4 }, {}).template.children.find((c) => c.kind === "repeat").from.count, 4, "asked for");
  // Horizontal bars: the vertical gridlines take the value axis's count.
  const found = [];
  const walk = (n) => { if (n && typeof n === "object") { if (n.recipe === "@datars/std/grid") found.push(n.params); Object.values(n).forEach(walk); } };
  walk(plot.__expand({ data: "v", x: "n", y: "k", xType: "linear", yType: "band", children: [] }, {}).template);
  assert.ok(found[0].count.expr.includes("floor("), JSON.stringify(found[0]));
});

test("plot: lines labelled at their ends get the room their names need, up to labelSpace", () => {
  const out = plot.__expand({ data: "v", x: "t", y: "n", color: "s", xType: "linear", labelSpace: 150, children: [line({ labels: true })] }, {});
  const [series] = Object.entries(out.tables).find(([k]) => k.includes("series-names"));
  let room;
  const walk = (n) => { if (n && typeof n === "object") { if (n.key === "end-labels") room = n.size.w; Object.values(n).forEach(walk); } };
  walk(out.template);
  assert.equal(room.expr, `min(150, table.max(${JSON.stringify(series)}, "w") + 12)`);
});

test("card: never wider than its box leaves; its lines wrap to the card", () => {
  const box = card.__expand({ text: "Hello", width: 300, margin: [12, 10, 12, 14] }, {}).template.children[0];
  assert.equal(box.size.w.expr, "min(300, box.w - 24)");
  assert.equal(box.children[0].style.max_width.expr, "box.w");
});

test("annotate: text that would leave the plot above (or below) its point goes to the other side", () => {
  const up = annotate.__expand({ x: e("scale.x(2022)"), y: e("scale.y(7.7)"), text: "2022 spike", dx: -40, dy: -24 }, {}).template;
  const text = up.children.find((c) => c.key === "text");
  assert.ok(text.at[1].expr.includes("scale.y(7.7)) + -24 - (ceil(measure("), text.at[1].expr);
  assert.ok(text.style.baseline.expr.endsWith('? "top" : "bottom"'), text.style.baseline.expr);
  const down = annotate.__expand({ x: 10, y: 20, text: "low", dy: 30 }, {}).template.children.find((c) => c.key === "text");
  assert.ok(down.at[1].expr.includes("> box.h + 2"), down.at[1].expr);
});

test("waterfall: a label that would run into the title goes inside its bar", () => {
  const [, label] = waterfall.__expand({ data: "b", x: "item", y: "delta", total: "total" }, {}).template.children[0].template.children;
  assert.ok(label.at[1].expr.includes('- 4 - token("size.label") < -2'), label.at[1].expr);
  assert.ok(label.style.ink.expr.includes('"on(" +'), "in the ink that reads on the bar");
});

test("hemicycle: at the bottom of a box its radius fills, centred in a taller one", () => {
  const out = hemicycle.__expand({ data: "s", value: "seats", category: "party" }, {});
  const seats = Object.values(out.tables)[0].ops.find((o) => o.op === "parliament");
  assert.equal(seats.cy.expr, "min(box.h - 10, (box.h + min(box.w / 2 - 8, box.h - 18)) / 2 + 4)");
});

test("map and pie labels keep off each other (declutter)", () => {
  const m = map.__expand({ source: "c", data: "d", key: "id", value: "v", labels: true }, {}).template;
  assert.equal(m.children.find((c) => c.key === "labels").declutter, true);
  const p = pie.__expand({ data: "d", value: "v", category: "c" }, {}).template;
  assert.equal(p.children.find((c) => c.key === "labels").declutter, true);
});

// ---- financial charts -------------------------------------------------------------------------------

test("candlestick: bodies and wicks keyed by the datum, the previous close for the tooltip's change", () => {
  const out = candlestick.__expand({ data: "px", x: "date", y: "close", xType: "band" }, {});
  const [wicks, bodies] = out.template.children;
  assert.deepEqual([wicks.key, bodies.key], ["wicks", "bodies"]);
  assert.equal(bodies.children[0].template.kind, "shape", "a body is the datum shape, directly under its repeat");
  assert.equal(bodies.children[0].template.semantics.role, "datum");
  const lag = Object.values(out.tables)[0].ops[0];
  assert.deepEqual([lag.fn, lag.field, lag.order], ["lag", "close", "date"]);
  const json = JSON.stringify(bodies);
  assert.ok(json.includes('d.close >= d.open ? \\"$up\\" : \\"$down\\"'), json);
  assert.ok(json.includes("max(1, abs(scale.y(d.open) - scale.y(d.close)))"), "a doji is still a line");
});

test("plot: finance marks fit the value axis, drop zero, and volume gets a pane of its own", () => {
  const out = plot.__expand({ data: "shown", x: "date", y: "close", xType: "band", children: [candlestick({}), movingAverage({ window: 20, source: "all" }), volume({})] }, {});
  const t = out.template;
  assert.equal(t.scales.y.zero, false, "prices leave out zero");
  assert.deepEqual(t.scales.y.domain.fields, ["low", "high", "sma_20", "close"]);
  const axisTable = out.tables[t.scales.y.domain.data];
  assert.equal(axisTable.from, "all", "averages over the whole history…");
  assert.deepEqual(axisTable.ops.map((o) => o.op), ["window", "join"], "…cut to the rows on show");
  assert.deepEqual(t.scales.lower.range, { box: "lower-area", axis: "-y" });
  assert.equal(t.scales.lower.domain.field, "volume");
  const json = JSON.stringify(t);
  const lowerPane = json.slice(json.indexOf('"id":"lower-area"'));
  assert.ok(lowerPane.includes('"recipe":"@datars/std/volume"'), "volume draws in the lower pane");
  assert.ok(json.includes('"key":"axes-left"') && json.includes('"scale":"lower"'), "with its axis under the price axis");
  assert.ok(json.includes('"key":"indicators"') && json.includes('"SMA 20"'), "the average is named in the key");
  // On a log axis prices fit their range instead of whole decades.
  assert.equal(plot.__expand({ data: "p", x: "date", y: "close", xType: "band", yType: "log", children: [candlestick({})] }, {}).template.scales.y.nice, false);
  // Ejected copies are still what they were: matched by name, not package.
  const ejected = plot.__expand({ data: "p", x: "date", y: "close", xType: "band", children: [{ kind: "use", recipe: "@local/candlestick/candlestick", params: {} }] }, {}).template;
  assert.deepEqual(ejected.scales.y.domain.fields, ["low", "high", "close"]);
  // Without finance marks nothing changes: zero stays in, one field.
  const plain = plot.__expand({ data: "t", x: "k", y: "v", children: [bar({})] }, {}).template;
  assert.equal(plain.scales.y.zero, true);
  assert.deepEqual(plain.scales.y.domain, { data: "t", field: "v" });
});

test("plot: a hidden indicator claims nothing; indexed lines replace prices on the axis", () => {
  const out = plot.__expand({ data: "p", x: "date", y: "close", xType: "band", children: [bollinger({ window: 10 }, { when: { expr: 'view == "bands"' } })] }, {});
  const ops = out.tables[out.template.scales.y.domain.data].ops;
  const shown = ops.filter((o) => o.op === "derive" && o.as.includes("_shown_"));
  assert.equal(shown.length, 2);
  assert.equal(shown[0].expr.expr, '(view == "bands") ? d.bb_hi_10_2 : null');
  assert.ok(JSON.stringify(out.template).includes('"when":{"expr":"(view == \\"bands\\")"}'), "the key row hides with it");
  const idx = plot.__expand({ data: "p", x: "date", y: "close", color: "t", xType: "band", children: [indexed({})] }, {}).template;
  assert.deepEqual(idx.scales.y.domain.fields, ["indexed"], "the index, not the prices");
  assert.ok(JSON.stringify(idx).includes('"key":"end-labels"'), "room for the end labels");
});

test("indicators: moving averages and bands over a history, rebasing and drawdowns per series", () => {
  const ma = movingAverage.__expand({ data: "shown", x: "date", y: "close", window: 50, kind: "ema", source: "all" }, {});
  const [w, j] = Object.values(ma.tables)[0].ops;
  assert.deepEqual([w.fn, w.k, w.min, j.op, j.with, j.kind], ["ema", 50, 50, "join", "shown", "inner"]);
  const bb = Object.values(bollinger.__expand({ data: "p", x: "date", y: "close", window: 20, k: 2.5 }, {}).tables)[0].ops;
  assert.deepEqual(bb.map((o) => o.fn ?? o.as), ["rolling_mean", "rolling_std", "bb_hi_20_2_5", "bb_lo_20_2_5"]);
  const idx = Object.values(indexed.__expand({ data: "p", x: "date", y: "close", series: "t" }, {}).tables)[0].ops;
  assert.deepEqual(idx[0].partition, ["t"]);
  assert.equal(idx[1].expr.expr, "d.close / d.index_first * 100");
  const dd = Object.values(drawdown.__expand({ data: "p", x: "date", y: "close" }, {}).tables)[0].ops;
  assert.equal(dd[0].fn, "cummax");
});

test("sparkline: its own scales over the box, coloured by the change", () => {
  const out = sparkline.__expand({ data: "@group", x: "date", y: "close" }, {});
  assert.deepEqual(Object.keys(out.template.scales), ["sx", "sy"]);
  assert.equal(out.template.scales.sy.zero, false);
  const json = JSON.stringify(out.template);
  assert.ok(json.includes('group.last(\\"close\\") >= group.first(\\"close\\") ? \\"$up\\" : \\"$down\\"'), json);
  assert.deepEqual(Object.keys(out.tables), [], "no derived tables: it works on @group");
});

// ---- what the std reference figures found ------------------------------------------------------------

/** Every `use` of `recipe` in a template tree, as its params. */
function uses(tree, recipe) {
  const found = [];
  const walk = (n) => { if (n && typeof n === "object") { if (n.recipe === `@datars/std/${recipe}`) found.push(n.params); Object.values(n).forEach(walk); } };
  walk(tree);
  return found;
}

/** The first node with `key` in a template tree. */
function byKey(tree, key) {
  let found;
  const walk = (n) => { if (!found && n && typeof n === "object") { if (n.key === key) found = n; else Object.values(n).forEach(walk); } };
  walk(tree);
  return found;
}

test("plot: yLabel (and the right axis's label) in a row above the plot area, not up the side", () => {
  const t = plot.__expand({ data: "t", x: "k", y: "v", title: "T", yLabel: "People", right: { y: "s", label: "Share" }, children: [bar({})] }, {}).template;
  const titles = byKey(t, "axis-titles");
  assert.deepEqual(titles.children.map((c) => [c.key, c.text, c.style.align]), [["y", "People", undefined], ["y2", "Share", "end"]]);
  assert.equal(t.children.indexOf(titles), t.children.findIndex((c) => c.key === "body") - 1, "right above the body, under the title");
  assert.deepEqual(uses(t, "axis").map((a) => a.label), [undefined, undefined, undefined], "no axis draws a vertical title itself");
  const hidden = plot.__expand({ data: "t", x: "k", y: "v", yLabel: "People", axes: "x", children: [] }, {}).template;
  assert.equal(byKey(hidden, "axis-titles"), undefined, "no y axis, no y title");
  // A vertical axis on its own draws its title above its top end, clear of the top label.
  const label = axis.__expand({ scale: "y", orient: "left", label: "People" }, {}).template.children.find((c) => c.key === "label");
  assert.equal(label.at[1].expr, "min(scale.y.min(), scale.y.max()) - 8");
  assert.equal(label.style.baseline, "bottom");
});

test("legend: a title above the entries; untitled, the entries flow as before", () => {
  const t = legend.__expand({ scale: "color", title: "Station" }, {}).template;
  assert.equal(t.layout.type, "rows");
  const [head, items] = t.children;
  assert.equal(head.text, "Station");
  assert.equal(items.key, "items");
  assert.equal(items.layout.type, "flow");
  assert.equal(t.semantics.label, "Station");
  const plain = legend.__expand({ scale: "color" }, {}).template;
  assert.equal(plain.layout.type, "flow");
  assert.equal(plain.children[0].kind, "repeat");
});

test("line: toggling labels keeps each line's key path, so the lines morph", () => {
  const path = (tpl) => [tpl.key, tpl.children[0].kind, tpl.children[0].template.children[0].key];
  const off = line.__expand({ data: "t", x: "m", y: "c", color: "city" }, {}).template;
  const on = line.__expand({ data: "t", x: "m", y: "c", color: "city", labels: true }, {}).template;
  assert.deepEqual(path(on), path(off));
  assert.deepEqual(path(off), ["lines", "repeat", "line"]);
});

test("plot: a 100 % stacked child fits the value axis to 0–1 (as percentages); a waterfall's, to its running totals", () => {
  const v = plot.__expand({ data: "t", x: "k", y: "v", color: "s", children: [stacked({ offset: "expand" })] }, {}).template;
  assert.deepEqual(v.scales.y.domain, [0, 1]);
  assert.equal(uses(v, "axis")[0].format, ".0%");
  const h = plot.__expand({ data: "t", x: "v", y: "k", xType: "linear", yType: "band", color: "s", format: ".1f", children: [stacked({ offset: "expand" })] }, {}).template;
  assert.deepEqual(h.scales.x.domain, [0, 1]);
  assert.equal(uses(h, "axis").find((a) => a.scale === "x").format, ".1f", "a format given wins");
  const w = plot.__expand({ data: "b", x: "item", y: "change", children: [waterfall({ total: "total" })] }, {});
  assert.deepEqual(w.template.scales.y.domain.fields, ["start", "end"]);
  const ops = w.tables[w.template.scales.y.domain.data].ops;
  assert.deepEqual([ops[0].op, ops[0].value, ops[0].total], ["waterfall", "change", "total"]);
  assert.deepEqual(plot.__expand({ data: "b", x: "item", y: "change", yDomain: [0, 9], children: [waterfall({})] }, {}).template.scales.y.domain, [0, 9], "an explicit domain wins");
});

test("rule and span: values given as expressions (or '=…' strings) are expressions", () => {
  const r = rule.__expand({ axis: "y", value: e("avg") }, {}).template;
  assert.equal(r.children[0].geom.y1.expr, "scale.y((avg)) + scale.y.bandwidth() / 2");
  assert.equal(r.key, "rule-y-avg");
  assert.ok(r.semantics.label.expr.includes("(avg)"), "it reads as its value");
  assert.ok(rule.__expand({ value: "=goal * 2" }, {}).template.children[0].geom.y1.expr.startsWith("scale.y((goal * 2))"));
  assert.ok(rule.__expand({ axis: "x", value: "SD" }, {}).template.children[0].geom.x1.expr.startsWith('scale.x("SD")'), "a string is a category");
  const sp = span.__expand({ from: e("lo"), to: "=hi" }, {}).template;
  assert.ok(sp.children[0].geom.x.expr.includes("scale.x((lo))") && sp.children[0].geom.x.expr.includes("scale.x((hi))"), JSON.stringify(sp.children[0].geom));
  assert.equal(sp.key, "span-x-lo");
});

test("title: the subtitle and source wrap to the box like the title", () => {
  const [t, sub, src] = title.__expand({ text: "T", subtitle: "S", source: "Src" }, {}).template.children;
  for (const n of [t, sub, src]) assert.equal(n.style.max_width.expr, "box.w");
  const p = plot.__expand({ data: "t", x: "k", y: "v", title: "T", subtitle: "S", children: [] }, {}).template;
  assert.equal(p.children.find((c) => c.key === "subtitle").style.max_width.expr, "box.w");
});

test("treemap: each label in the ink that reads on its fill", () => {
  const t = treemap.__expand({ data: "s", value: "v", category: "c" }, {}).template;
  assert.equal(t.children[1].children[0].template.style.ink.expr, '"on(" + scale.color(d.c) + ")"');
});

test("calendar: years stack, a row each, labelled in the room at the left", () => {
  const out = calendar.__expand({ data: "d", date: "date", value: "n" }, {});
  const [days, years] = out.template.children;
  assert.equal(days.y.expr, "d.cy + d.panel * 8 * d.cell");
  assert.ok(days.x.expr.startsWith('d.cx + measure("0000"'), days.x.expr);
  assert.equal(years.children[0].template.text.expr, 'formatDate(d.first, "%Y")');
  const [yearsTable, cells] = Object.values(out.tables);
  assert.deepEqual(yearsTable.ops.map((o) => o.op), ["calendar", "aggregate"]);
  assert.ok(cells.ops[0].cell.expr.includes("box.h / (8 *"), "the cells fit every year's row into the box");
  assert.equal(Object.values(calendar.__expand({ data: "d", date: "date", value: "n", cell: 8 }, {}).tables)[1].ops[0].cell.expr, "8");
});

test("plot: without a y, a unit y scale and no y axis or gridlines (a one-dimensional swarm)", () => {
  const t = plot.__expand({ data: "t", x: "v", xType: "linear", children: [swarm({})] }, {}).template;
  assert.deepEqual(t.scales.y.domain, [0, 1]);
  assert.deepEqual(uses(t, "axis").map((a) => a.scale), ["x"]);
  assert.deepEqual(uses(t, "grid"), []);
  assert.deepEqual(plot.__expand({ data: "t", x: "k", y: "v", children: [] }, {}).template.scales.y.domain, { data: "t", field: "v" }, "with a y, unchanged");
});

test("dotDensity: the key column is the op's feature ids; dots keyed (region, i) by the op", () => {
  const out = dotDensity.__expand({ source: "world", data: "forest", key: "id", value: "km2", per: 500 }, {});
  const op = Object.values(out.tables)[0].ops[0];
  assert.deepEqual([op.op, op.key, op.geo], ["scatter-in", "id", "world"]);
  assert.equal(out.template.instanceKey, undefined, "the table's (region, dot) key identifies each dot");
  assert.equal(out.template.label.expr, "key.name(d.id)");
});

test("volume: its own chart (yScale y) colours by the close, not by the volume it stands on", () => {
  const fill = (params) => JSON.stringify(volume.__expand({ data: "p", x: "date", ...params }, {}).template.children[0].template.fill);
  assert.ok(fill({ y: "volume", yScale: "y" }).includes("d.close >= d.open"), fill({ y: "volume", yScale: "y" }));
  assert.ok(fill({ y: "close" }).includes("d.close >= d.open"));
  assert.ok(fill({ y: "last" }).includes("d.last >= d.open"), "the plot's y is the close in a price chart");
  assert.ok(fill({ y: "volume", yScale: "y", close: "c" }).includes("d.c >= d.open"));
});

// ---- big data: hexbin, heatmap2d, contours, manyLines ------------------------------------------------

import { hexbin as stdHexbin, heatmap2d, contours as stdContours, manyLines } from "../dist/index.js";

/** Every node of a template tree (children, repeat templates). */
function nodes(t, out = []) {
  if (!t || typeof t !== "object") return out;
  out.push(t);
  for (const c of t.children ?? []) nodes(c, out);
  if (t.template) nodes(t.template, out);
  return out;
}

test("hexbin: a lattice from the plot area's size, binned in a pure table, keyed by lattice", () => {
  const out = stdHexbin.__expand({ data: "pts", x: "x", y: "y", radius: 10, extent: [0, 0, 40, 100] }, { size: [520, 300] });
  const [bins, px] = Object.values(out.tables);
  const op = bins.ops[0];
  assert.deepEqual([op.op, op.x, op.y, op.fn], ["hexbin", "x", "y", "count"]);
  assert.equal(op.columns, Math.round(520 / (10 * Math.sqrt(3))), "hexagons of the radius across the plot area");
  assert.equal(op.aspect, 0.58, "the plot area's height ÷ width, rounded (a pixel off reuses the table)");
  assert.deepEqual(op.extent, [0, 0, 40, 100]);
  assert.equal(bins.ops[1].expr.expr, "sign(d.value) * sqrt(abs(d.value))", "counts on a sqrt ramp by default");
  assert.ok(!JSON.stringify(bins).includes("scale."), "the binned table reads only rows: kept across resolves");
  assert.equal(px.from, Object.keys(out.tables)[0], "the per-resolve table derives from the bins");
  assert.ok(JSON.stringify(px).includes("scale.x(d.x)"));
  const all = nodes(out.template);
  const hexes = all.find((n) => typeof n.key === "string" && n.key.startsWith("hexagons-"));
  assert.equal(hexes.key, `hexagons-${op.columns}`, "another lattice crossfades as a layer");
  assert.equal(hexes.children[0].template.geom.type, "path");
  const pick = all.find((n) => n.kind === "instances");
  assert.equal(pick.instance_key.expr, "d.hex");
  assert.ok(pick.label.expr.includes("rows near"), pick.label.expr);
  assert.ok(all.some((n) => n.semantics?.role === "legend" && n.semantics.label === "Rows per hexagon"), "a colour key");
  assert.ok(all.every((n) => !JSON.stringify(n.fill ?? "").includes("#")), "theme inks only");
  // An aggregate: mean by default, on a linear ramp, and the key says what it is.
  const mean = stdHexbin.__expand({ data: "pts", x: "x", y: "y", value: "fare", unit: "trips" }, { size: [520, 300] });
  const mop = Object.values(mean.tables)[0];
  assert.deepEqual([mop.ops[0].fn, mop.ops[0].value, mop.ops[1].expr.expr], ["mean", "fare", "d.value"]);
  assert.ok(nodes(mean.template).some((n) => n.semantics?.label === "Mean fare"));
  // Sized, with one ink and no key.
  const sized = nodes(stdHexbin.__expand({ data: "pts", x: "x", y: "y", size: true, fill: "$accent", legend: "none" }, { size: [520, 300] }).template);
  assert.ok(!sized.some((n) => n.semantics?.role === "legend"));
  assert.ok(JSON.stringify(Object.values(stdHexbin.__expand({ data: "pts", x: "x", y: "y", size: true }, { size: [520, 300] }).tables)[1]).includes("table.max("), "area ∝ value");
});

test("heatmap2d: cells from the plot area and `cell`, drawn as keyed rect instances", () => {
  const out = heatmap2d.__expand({ data: "pts", x: "x", y: "y", cell: 5, unit: "pickups" }, { size: [500, 300] });
  const bins = Object.values(out.tables)[0];
  assert.deepEqual([bins.ops[0].op, bins.ops[0].columns, bins.ops[0].rows], ["bin2d", 100, 60]);
  const cells = nodes(out.template).find((n) => n.kind === "instances");
  assert.equal(cells.proto, "rect");
  assert.equal(cells.key, "cells-100x60", "keyed by the grid");
  assert.equal(cells.instance_key.expr, "d.cell");
  assert.ok(cells.fill.expr.startsWith("scale.heat("));
  assert.equal(out.template.scales.heat.type, "sequential");
  assert.ok(nodes(out.template).some((n) => n.semantics?.label === "Pickups per cell"));
  const fixed = Object.values(heatmap2d.__expand({ data: "pts", x: "x", y: "y", columns: 30, rows: 20 }, { size: [500, 300] }).tables)[0];
  assert.deepEqual([fixed.ops[0].columns, fixed.ops[0].rows], [30, 20], "an explicit grid wins");
});

test("contours: a pure density table, then paths per level through the scales", () => {
  const out = stdContours.__expand({ data: "pts", x: "x", y: "y", bandwidth: 12, cell: 4 }, { size: [400, 240] });
  const [rings, paths] = Object.values(out.tables);
  const op = rings.ops[0];
  assert.deepEqual([op.op, op.columns, op.rows, op.bandwidth, op.levels], ["contours", 100, 60, 3, 4]);
  assert.ok(!JSON.stringify(rings).includes("scale."), "the density reads only rows: kept across resolves");
  assert.deepEqual([paths.ops[0].op, paths.ops[0].by, paths.ops[0].ring, paths.ops[0].closed], ["paths", "level", "ring", true]);
  const all = nodes(out.template);
  assert.equal(out.template.clip, "box");
  assert.ok(all.some((n) => n.semantics?.role === "region"), "each level is a region with its share");
  assert.ok(!all.some((n) => n.key === "lines"), "bands only by default");
  const both = stdContours.__expand({ data: "pts", x: "x", y: "y", style: "both", dots: 500, shares: [0.9, 0.5] }, { size: [400, 240] });
  assert.deepEqual(Object.values(both.tables)[0].ops[0].shares, [0.9, 0.5]);
  assert.ok(Object.values(both.tables).some((t) => t.ops[0].op === "sample" && t.ops[0].n === 500), "a seeded sample of the rows as dots");
  assert.ok(nodes(both.template).some((n) => n.key === "lines"));
});

test("manyLines: a path per series, the highlighted ones in the accent colour, named, hover", () => {
  const out = manyLines.__expand({ data: "w", x: "week", y: "temp", series: "station", highlight: ["S1", "S2"] }, {});
  const [lines, picked, names] = Object.values(out.tables);
  assert.deepEqual([lines.ops[0].op, lines.ops[0].by], ["paths", "station"]);
  assert.equal(picked.ops[0].expr.expr, 'd.station == "S1" || d.station == "S2"');
  assert.equal(names.ops[0].op, "spread", "names pushed apart");
  const all = nodes(out.template);
  const rest = all.find((n) => n.key === "all").children[0].template;
  assert.equal(rest.geom.d.expr, "d.path");
  assert.ok(rest.stroke.paint.expr.includes("hover()") && rest.stroke.paint.expr.includes('"$muted"'), "grey, the accent while hovered");
  assert.equal(rest.semantics.role, "series");
  const hl = all.find((n) => n.key === "highlight").children[0].template;
  assert.equal(hl.stroke.paint, "$accent");
  assert.ok(all.some((n) => n.key === "labels"));
  // A signal as the highlight; no hover; no highlight at all.
  const sig = manyLines.__expand({ data: "w", x: "week", y: "temp", series: "station", highlight: e("pick"), hover: false }, {});
  assert.equal(Object.values(sig.tables)[1].ops[0].expr.expr, "d.station == (pick)");
  assert.ok(!JSON.stringify(sig.template).includes("hover()"));
  const none = manyLines.__expand({ data: "w", x: "week", y: "temp", series: "station" }, {});
  assert.equal(Object.keys(none.tables).length, 1);
  assert.ok(!nodes(none.template).some((n) => n.key === "highlight"));
});

// ---- networks and hierarchies ------------------------------------------------------------------------

import { network, arcDiagram, chord, matrix, tree, dendrogram, sunburst, icicle } from "../dist/index.js";

/** Every node of a template (groups' children, repeats' templates). */
function netNodes(t, out = []) {
  if (!t || typeof t !== "object") return out;
  out.push(t);
  for (const c of t.children ?? []) netNodes(c, out);
  if (t.template) netNodes(t.template, out);
  return out;
}
const opsOf = (out) => Object.values(out.tables).flatMap((t) => t.ops.map((o) => o.op));

test("network: communities, a force layout inside the box, link ends looked up; nodes and links keyed", () => {
  const out = network.__expand({ nodes: "people", links: "papers" }, {});
  const ops = opsOf(out);
  for (const op of ["communities", "force", "lookup", "window"]) assert.ok(ops.includes(op), op);
  const force = Object.values(out.tables).flatMap((t) => t.ops).find((o) => o.op === "force");
  assert.deepEqual([force.links, force.source, force.target, force.id, force.iterations, force.seed], ["papers", "source", "target", "id", 300, 1]);
  assert.equal(force.radius.expr, "d.r");
  assert.equal(force.width.expr, "box.w");
  const all = netNodes(out.template);
  const node = all.find((n) => n.kind === "shape" && n.geom.type === "circle");
  assert.equal(node.key.expr, "d.id");
  assert.equal(node.semantics.role, "datum");
  assert.ok(node.semantics.label.expr.includes("d.degree} link"), "sized by degree by default: the tooltip says so");
  const link = all.find((n) => n.kind === "shape" && n.geom.type === "segment");
  assert.deepEqual(link.key, [{ expr: "d.source" }, { expr: "d.target" }]);
  assert.ok(all.some((n) => n.key === "labels" && n.declutter), "labels for the largest nodes, decluttered");
  // Colour, weight and a selection; no labels.
  const more = network.__expand({ nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n", selected: "focus", labels: 0 }, {});
  assert.equal(more.template.scales.color.domain.field, "lab");
  assert.ok(JSON.stringify(more.template).includes("focus.has(d.name)"));
  assert.ok(!netNodes(more.template).some((n) => n.key === "labels"));
});

test("arcDiagram and matrix: nodes ordered by cluster (or degree, a field, input order)", () => {
  const out = arcDiagram.__expand({ nodes: "people", links: "papers" }, {});
  const nodes = Object.entries(out.tables).find(([k]) => k.includes(":nodes:"))[1];
  assert.deepEqual(nodes.ops[0], { op: "sort", by: [["order", "asc"]] }, "cluster order by default");
  assert.equal(out.template.scales.nx.type, "point");
  assert.ok(netNodes(out.template).some((n) => n.kind === "shape" && n.geom.type === "path"), "arcs as paths");
  const input = arcDiagram.__expand({ nodes: "people", links: "papers", order: "input" }, {});
  assert.equal(Object.entries(input.tables).find(([k]) => k.includes(":nodes:"))[1].ops[0].op, "derive", "input order: no sort");
  const byName = matrix.__expand({ nodes: "people", links: "papers", order: "name" }, {});
  assert.deepEqual(Object.entries(byName.tables).find(([k]) => k.includes(":nodes:"))[1].ops[0].by[0], ["name", "asc"]);
  const m = matrix.__expand({ nodes: "people", links: "papers" }, {});
  assert.equal(m.template.scales.m.type, "band");
  assert.ok(netNodes(m.template).some((n) => n.key === "mirror"), "undirected: both cells");
  assert.ok(!netNodes(matrix.__expand({ nodes: "people", links: "papers", directed: true }, {}).template).some((n) => n.key === "mirror"));
});

test("chord: groups and ribbons from one links table; ribbons keyed by their pair", () => {
  const out = chord.__expand({ data: "trade", source: "from", target: "to", value: "v" }, {});
  const ops = Object.values(out.tables).flatMap((t) => t.ops);
  assert.ok(ops.some((o) => o.op === "chord-groups" && o.pad === 0.04));
  const ribbons = ops.find((o) => o.op === "chord-ribbons");
  assert.ok(ribbons.r && ribbons.cx, "ribbon outlines around the box's centre");
  const all = netNodes(out.template);
  const arc = all.find((n) => n.kind === "shape" && n.geom.type === "arc");
  assert.equal(arc.key.expr, "d.name");
  const ribbon = all.find((n) => n.kind === "shape" && n.geom.type === "path");
  assert.deepEqual(ribbon.key, [{ expr: "d.from" }, { expr: "d.to" }]);
  assert.ok(ribbon.semantics.label.expr.includes("→"));
});

test("tree and dendrogram: the tidy and the cluster layout, in three orientations", () => {
  const out = tree.__expand({ data: "org", id: "team", parent: "boss" }, {});
  const laid = Object.values(out.tables).flatMap((t) => t.ops).filter((o) => o.op === "tree");
  assert.ok(laid.every((o) => o.method === "tidy" && o.parent === "boss" && o.id === "team"));
  const links = netNodes(out.template).find((n) => n.key === "links").children[0].template;
  assert.ok(links.geom.d.expr.includes(" C "), "curved links by default");
  const radial = tree.__expand({ data: "org", id: "team", parent: "boss", orientation: "radial" }, {});
  assert.ok(Object.values(radial.tables).flatMap((t) => t.ops).some((o) => o.op === "tree" && o.width.expr === String(2 * Math.PI)), "angle × radius");
  const den = dendrogram.__expand({ data: "org", id: "team", parent: "boss", orientation: "vertical" }, {});
  assert.ok(Object.values(den.tables).flatMap((t) => t.ops).filter((o) => o.op === "tree").every((o) => o.method === "cluster"));
  const elbow = netNodes(den.template).find((n) => n.key === "links").children[0].template;
  assert.ok(elbow.geom.d.expr.includes(" H ") && elbow.geom.d.expr.includes(" V "), "elbows by default");
  const nodes = netNodes(den.template).find((n) => n.key === "nodes").children[0].template;
  assert.equal(nodes.key.expr, "d.team");
  assert.equal(nodes.semantics.label.expr, "d.path", "a node names its path from the root");
});

test("sunburst and icicle: a partition in angle × radius, or across the box", () => {
  const sb = sunburst.__expand({ data: "disk", id: "folder", value: "gb" }, {});
  const part = Object.values(sb.tables).flatMap((t) => t.ops).find((o) => o.op === "partition");
  assert.deepEqual([part.width, part.value, part.sort], [2 * Math.PI, "gb", false]);
  const arc = netNodes(sb.template).find((n) => n.kind === "shape" && n.geom.type === "arc");
  assert.equal(arc.key.expr, "d.folder");
  assert.ok(arc.semantics.label.expr.includes("d.path"));
  assert.ok(netNodes(sb.template).some((n) => n.key === "total"), "the total in the hole");
  const ic = icicle.__expand({ data: "disk", id: "folder", value: "gb", sort: true }, {});
  const p2 = Object.values(ic.tables).flatMap((t) => t.ops).find((o) => o.op === "partition");
  assert.deepEqual([p2.width.expr, p2.height.expr, p2.sort], ["box.h", "box.w", true], "horizontal: breadth down the box, depth across");
  const v = icicle.__expand({ data: "disk", id: "folder", orientation: "vertical" }, {});
  assert.equal(Object.values(v.tables).flatMap((t) => t.ops).find((o) => o.op === "partition").width.expr, "box.w");
});

// ---- comparisons and distributions (comparisons.ts) -------------------------------------------

import { slope, dumbbell, bump, lollipop, pyramid, marimekko, histogram, boxplot, violin, ridgeline, errorBars, stackedArea, connectedScatter } from "../dist/index.js";

/** Every node of a template (children and repeat templates), depth first. */
function cmpNodes(t, out = []) {
  if (!t || typeof t !== "object") return out;
  out.push(t);
  for (const c of t.children ?? []) cmpNodes(c, out);
  if (t.template) cmpNodes(t.template, out);
  return out;
}
const cmpTables = (out) => Object.values(out.tables);

test("histogram: bins counted per bin, stacked by colour; the plot's axes fit the bins", () => {
  const out = histogram.__expand({ data: "t", x: "m", xType: "linear", step: 5 }, {});
  const ops = cmpTables(out)[0].ops;
  assert.deepEqual(ops.map((o) => o.op), ["filter", "bin", "aggregate", "window", "derive", "derive", "derive"]);
  assert.equal(ops[1].step, 5);
  const rep = out.template.children[0];
  assert.equal(rep.template.key.expr, "d.bin", "a bar per bin, keyed by where it starts");
  assert.ok(rep.template.semantics.label.expr.includes("d.bin_end"));
  const stackedBins = histogram.__expand({ data: "t", x: "m", color: "mode", bins: 20, density: true }, {});
  const sops = cmpTables(stackedBins)[0].ops;
  assert.equal(sops[1].count, 20);
  assert.deepEqual(sops.at(-1), { op: "stack", x: "bin", series: "mode", value: "density", as: ["y0", "y1"] });
  // In a plot without a y field: a count axis, both axes spanning the bins.
  const p = plot.__expand({ data: "t", x: "m", xType: "linear", children: [histogram({ step: 5 })] }, {});
  assert.deepEqual(p.template.scales.x.domain.fields, ["bin", "bin_end"]);
  assert.deepEqual(p.template.scales.y.domain.fields, ["y0", "y1"]);
  assert.ok(JSON.stringify(p.template).includes('"scale":"y","orient":"left"'), "a y axis");
});

test("boxplot: quartiles per category, whiskers inside 1.5 IQR, outliers beyond; horizontal on a band y", () => {
  const out = boxplot.__expand({ data: "t", x: "g", y: "v", xType: "band", yType: "linear" }, {});
  const [stats, rows, box, outliers] = cmpTables(out);
  assert.deepEqual(stats.ops[0].ops.map((o) => o.op), ["q25", "median", "q75", "count"]);
  assert.equal(rows.ops[0].op, "join");
  assert.ok(rows.ops[1].expr.expr.includes("1.5 * (d.__q3 - d.__q1)"));
  assert.equal(box.ops[0].expr.expr, "!d.__out");
  assert.equal(outliers.ops[0].expr.expr, "d.__out");
  const parts = cmpNodes(out.template).map((n) => n.key).filter((k) => typeof k === "string");
  for (const k of ["whisker-lo", "whisker-hi", "box", "median", "outliers"]) assert.ok(parts.includes(k), k);
  const h = boxplot.__expand({ data: "t", x: "v", y: "g", xType: "linear", yType: "band", whisker: 3 }, {});
  const hbox = cmpNodes(h.template).find((n) => n.key === "box");
  assert.equal(hbox.geom.x.expr, "scale.x(d.q1)", "along x");
  assert.ok(cmpTables(h)[1].ops[1].expr.expr.includes("3 * "));
  // A distribution's value axis needn't start at zero.
  assert.equal(plot.__expand({ data: "t", x: "g", y: "v", children: [boxplot({})] }, {}).template.scales.y.zero, false);
});

test("violin: a trimmed density per category (op.kde), turned upright; ridgeline: tails that taper, the plot reaching them", () => {
  const out = violin.__expand({ data: "t", x: "g", y: "v", xType: "band", yType: "linear", bandwidth: 2 }, {});
  const kde = cmpTables(out).find((t) => t.ops[0].op === "kde").ops[0];
  assert.deepEqual([kde.field, kde.groupby, kde.bandwidth, kde.trim], ["v", ["g"], 2, true]);
  const outline = cmpNodes(out.template).find((n) => n.key === "violin");
  assert.deepEqual(outline.transform, { rotate: -90 });
  assert.equal(outline.geom.x.expr, "-scale.y(d.value)");
  const flat = violin.__expand({ data: "t", x: "v", y: "g", xType: "linear", yType: "band", normalize: "width" }, {});
  const f = cmpNodes(flat.template).find((n) => n.key === "violin");
  assert.equal(f.transform, undefined);
  assert.ok(f.geom.y0.expr.includes('group.max("density")'), "each violin to its own peak");
  const ridges = ridgeline.__expand({ data: "t", x: "v", y: "month", xType: "linear", yType: "band" }, {});
  assert.equal(cmpTables(ridges).find((t) => t.ops[0].op === "kde").ops[0].extend, 3);
  const p = plot.__expand({ data: "t", x: "v", y: "month", xType: "linear", yType: "band", children: [ridgeline({})] }, {});
  assert.equal(p.template.scales.x.domain.field, "value");
  assert.equal(p.template.scales.x.zero, false);
});

test("slope: first and last value per series, labels spread at both ends, the plot leaving room", () => {
  const out = slope.__expand({ data: "v", x: "year", y: "share", color: "party", xType: "point", labels: "both", values: true, highlight: ["A"] }, {});
  const [pairs, placed] = cmpTables(out);
  assert.deepEqual(pairs.ops.map((o) => o.op), ["sort", "aggregate"]);
  assert.deepEqual(placed.ops.map((o) => [o.op, o.as]), [["spread", "ly0"], ["spread", "ly1"]]);
  const seg = out.template.children[0].template;
  assert.equal(seg.semantics.role, "datum");
  assert.ok(seg.stroke.paint.expr.includes('d.party == "A"') && seg.stroke.paint.expr.includes('"$rule"'), "the others grey");
  const p = plot.__expand({ data: "v", x: "year", y: "share", xType: "point", color: "party", children: [slope({})] }, {});
  const json = JSON.stringify(p.template);
  assert.ok(json.includes('"key":"start-labels"') && json.includes('"key":"end-labels"'), "room at both sides");
  const endOnly = JSON.stringify(plot.__expand({ data: "v", x: "year", y: "share", xType: "point", color: "party", children: [slope({ labels: "end" })] }, {}).template);
  assert.ok(!endOnly.includes('"key":"start-labels"') && endOnly.includes('"key":"end-labels"'));
});

test("dumbbell and lollipop: a pair per category joined; a stem and a dot, keyed by the row", () => {
  const d = dumbbell.__expand({ data: "t", x: "min", y: "district", color: "year", xType: "linear", yType: "band", labels: true }, {});
  assert.deepEqual(cmpTables(d)[0].ops[0].ops.map((o) => [o.as, o.op]), [["lo", "min"], ["hi", "max"], ["first", "first"], ["last", "last"]]);
  const all = cmpNodes(d.template);
  assert.ok(all.some((n) => n.key === "connectors") && all.some((n) => n.key === "lo") && all.some((n) => n.key === "hi"));
  const l = lollipop.__expand({ data: "t", x: "v", y: "town", xType: "linear", yType: "band", labels: true }, {});
  const dot = l.template.children[1].template;
  assert.equal(dot.geom.type, "circle");
  assert.equal(dot.key, undefined, "the dot is the datum, keyed by its row (like a bar)");
  assert.equal(dot.geom.cx.expr, "scale.x(d.v)");
});

test("bump: ranks per time from the values (largest first), highlighted series on top", () => {
  const out = bump.__expand({ data: "n", x: "year", y: "births", series: "name", highlight: ["Alma"] }, {});
  const [ranked, drawn] = cmpTables(out);
  assert.deepEqual(ranked.ops[0], { op: "window", fn: "rank", field: "births", as: "__rank", partition: ["year"], order: "-births" });
  assert.deepEqual(drawn.ops.at(-1).by, [["__on", "asc"], ["year", "asc"]], "picked-out series drawn last");
  assert.equal(out.template.scales.y.domain.fields.join(), "__lo,__hi");
  const rev = bump.__expand({ data: "n", x: "year", y: "time", series: "name", reverse: true }, {});
  assert.equal(cmpTables(rev)[0].ops[0].order, "time");
  const given = bump.__expand({ data: "n", x: "year", y: "pos", series: "name", rank: true }, {});
  assert.equal(cmpTables(given)[0].ops[0].op, "derive");
});

test("pyramid: two panes on one scale, the left one reversed; marimekko: columns as wide as their totals", () => {
  const out = pyramid.__expand({ data: "p", y: "age", side: "sex", value: "n" }, {});
  const s = out.template.scales;
  assert.deepEqual([s.xl.range.axis, s.xr.range.axis, s.y.range.axis], ["-x", "x", "-y"]);
  assert.deepEqual(s.xl.domain, s.xr.domain, "one scale for both sides");
  const left = cmpTables(out).find((t) => t.ops[0]?.op === "filter");
  assert.equal(left.ops[0].expr.expr, "d.sex == d.__first", "the first side in the data goes left");
  const swapped = pyramid.__expand({ data: "p", y: "age", side: "sex", value: "n", left: "Women" }, {});
  assert.ok(JSON.stringify(swapped.tables).includes('d.sex == \\"Women\\"'));
  const m = marimekko.__expand({ data: "b", x: "region", series: "type", value: "k" }, {});
  const [cols, cells] = cmpTables(m);
  assert.deepEqual(cols.ops.map((o) => o.fn ?? o.op), ["aggregate", "share_of_total", "cumsum", "derive"]);
  assert.deepEqual(cells.ops.map((o) => o.op), ["stack", "join"]);
  const rect = cmpNodes(m.template).find((n) => n.geom?.type === "rect" && Array.isArray(n.key));
  assert.deepEqual(rect.key, [{ expr: "d.type" }, { expr: "d.region" }], "keyed (series, column), as stacked bars are");
});

test("errorBars, stackedArea and connectedScatter: intervals, stacks and paths the plot's axes fit", () => {
  const eb = errorBars.__expand({ data: "p", x: "party", y: "share", xType: "band", yType: "linear", error: "moe" }, {});
  assert.deepEqual(cmpTables(eb)[0].ops.map((o) => o.expr.expr), ["d.share - d.moe", "d.share + d.moe"]);
  const pe = plot.__expand({ data: "p", x: "party", y: "share", children: [errorBars({ lo: "lo", hi: "hi" })] }, {});
  assert.deepEqual(pe.template.scales.y.domain.fields, ["__lo", "__hi", "share"]);
  const sa = stackedArea.__expand({ data: "e", x: "year", y: "twh", color: "src", xType: "linear", offset: "wiggle", order: "inside-out", labels: true }, {});
  const st = cmpTables(sa)[0].ops.find((o) => o.op === "stack");
  assert.deepEqual([st.offset, st.order], ["wiggle", "inside-out"]);
  const pa = plot.__expand({ data: "e", x: "year", y: "twh", xType: "linear", color: "src", children: [stackedArea({ offset: "expand" })] }, {});
  assert.deepEqual(pa.template.scales.y.domain, [0, 1]);
  assert.ok(JSON.stringify(pa.template).includes('"format":".0%"'), "a 100 % stack reads as percentages");
  const cs = connectedScatter.__expand({ data: "c", x: "u", y: "i", xType: "linear", yType: "linear", order: "year", every: 3 }, {});
  const [sorted, labelled] = cmpTables(cs);
  assert.deepEqual(sorted.ops, [{ op: "sort", by: [["year", "asc"]] }]);
  assert.ok(labelled.ops.at(-1).expr.expr.includes("(d.__i - 1) % 3 == 0"));
  const line = cmpNodes(cs.template).find((n) => n.key === "line");
  assert.deepEqual(line.markers, { end: { type: "arrow", size: 7 } });
  assert.ok(cmpNodes(cs.template).find((n) => n.key === "labels").declutter, "labels give way to each other");
});

// ---- KPIs, planning, tables, radar and tile maps (business.ts) ---------------------------------------
import { kpi, bullet, gauge, progress, gantt, timeline, radar, dataTable as stdTable, tileMap } from "../dist/index.js";

/** Every node of an expansion, depth first (repeat templates included). */
function bizNodes(t, out = []) {
  if (!t || typeof t !== "object") return out;
  out.push(t);
  for (const c of t.children ?? []) bizNodes(c, out);
  if (t.template) bizNodes(t.template, out);
  return out;
}
const bizOps = (out) => Object.values(out.tables).flatMap((t) => t.ops);
const bizFind = (out, key) => bizNodes(out.template).find((n) => n.key === key);

test("kpi: the last row against the one before, the change coloured by which way is good, said in one sentence", () => {
  const out = kpi.__expand({ label: "Revenue", data: "shop", x: "month", y: "revenue", format: "$,.0f", compareLabel: "vs August" }, {});
  const t = Object.values(out.tables)[0];
  assert.deepEqual(t.ops.map((o) => o.fn ?? o.op), ["sort", "lag"], "rows in order, each with the one before");
  const value = bizFind(out, "value");
  assert.equal(value.number.format, "$,.0f", "the value counts when it changes");
  assert.ok(value.semantics.label.expr.startsWith('"Revenue: " + '), "tooltips and screen readers read label, value…");
  assert.ok(value.semantics.label.expr.includes("vs August") && value.semantics.label.expr.includes('"up "'), "…and the change");
  const delta = bizFind(out, "delta +.1%");
  assert.equal(delta.number.format, "+.1%", "the change counts too, keyed by its format");
  assert.ok(delta.style.ink.expr.includes("> 0 ? \"$positive\" : \"$negative\""));
  assert.ok(bizFind(out, "trend"), "a sparkline from the rows");
  // Costs: a fall is good. Plain numbers: no table, no sparkline; a comparison line only with `compare`.
  const down = kpi.__expand({ label: "Returns", value: 0.04, compare: 0.05, better: "down", delta: "absolute", format: ".1%" }, {});
  assert.ok(bizFind(down, "delta +.1%").style.ink.expr.includes("< 0 ? \"$positive\""));
  assert.equal(Object.keys(down.tables).length, 0);
  assert.equal(bizFind(down, "trend"), undefined);
  assert.equal(bizFind(kpi.__expand({ value: 3 }, {}), "delta-row"), undefined, "nothing to compare: no change line");
});

test("bullet: a scale and nice ticks per row as columns, the ticks as rows of their own, bands darkest first", () => {
  const out = bullet.__expand({ data: "m", label: "name", value: "v", target: "t", bands: ["poor", "ok", "good"] }, {});
  const [rows, ticks] = Object.values(out.tables);
  assert.ok(rows.ops.some((o) => o.as === "__max") && rows.ops.some((o) => o.as === "__step"));
  assert.equal(ticks.ops[0].op, "units", "one row per tick");
  const bands = bizNodes(out.template).filter((n) => String(n.key).startsWith("band-"));
  assert.deepEqual(bands.map((b) => b.fill), ["$ink@0.3", "$ink@0.19", "$ink@0.11"]);
  assert.ok(bizFind(out, "target").geom.x1.expr.includes("d.t"));
  const shared = bullet.__expand({ data: "m", label: "name", value: "v", bands: [100, 200], shared: true }, {});
  assert.ok(bizOps(shared).some((o) => o.fn === "cummax"), "one scale for all: the largest of every row");
  assert.ok(bizOps(shared).some((o) => o.op === "filter"), "ticked under the last row only");
});

test("gauge: a filled arc, or bands and a needle turned in two halves", () => {
  const fill = gauge.__expand({ value: 64, label: "Stock" }, {});
  assert.ok(bizFind(fill, "fill"), "filled to the value");
  assert.equal(bizFind(fill, "needle"), undefined);
  const dial = gauge.__expand({ value: 86, bands: [70, 90, 100], inks: ["$negative", "$highlight", "$positive"] }, {});
  assert.deepEqual(bizNodes(dial.template).filter((n) => String(n.key).startsWith("band-")).map((n) => n.fill), ["$negative", "$highlight", "$positive"]);
  const needle = bizFind(dial, "needle");
  assert.equal(needle.transform.rotate.expr, bizFind(dial, "turn").transform.rotate.expr, "each half turns by half the angle");
  assert.equal(bizFind(dial, "value").semantics.role, "datum");
});

test("progress: a bar or a ring filled to value ÷ goal, never past full", () => {
  const bar = progress.__expand({ value: 7200, goal: 10000, label: "Raised", showGoal: true, prefix: "$" }, {});
  assert.ok(bizFind(bar, "fill").geom.w.expr.includes("clamp("));
  assert.ok(bizFind(bar, "goal").text.expr.includes('" of "'));
  const ring = progress.__expand({ value: 0.4, shape: "ring" }, {});
  assert.equal(bizFind(ring, "fill").geom.type, "arc");
  assert.equal(bizFind(ring, "share").number.format, ".0%");
});

test("gantt: groups as heading rows (union, then sorted), tasks keyed by name, milestones and today", () => {
  const out = gantt.__expand({ data: "plan", task: "task", start: "s", end: "e", group: "phase", progress: "done", today: "2026-03-16" }, {});
  const main = Object.values(out.tables).find((t) => t.ops.some((o) => o.op === "union"));
  assert.ok(main, "heading rows joined to the tasks");
  assert.deepEqual(main.ops.at(-1), { op: "sort", by: [["__go", "asc"], ["__ord", "asc"], ["__ti", "asc"]] }, "groups in order, heading first, tasks as they come");
  const row = bizNodes(out.template).find((n) => n.kind === "group" && n.key?.expr === "d.__id");
  assert.ok(row, "a row keyed by its task, so a replanned task slides");
  assert.ok(bizFind(out, "diamond") && bizFind(out, "done") && bizFind(out, "today"));
  assert.equal(out.template.scales.x.type, "time");
  assert.equal(out.template.scales.color.domain.field, "phase");
});

test("timeline: labels packed into lanes by the engine, eras under the axis", () => {
  const out = timeline.__expand({ data: "ev", date: "year", label: "what", xType: "linear", eras: "eras" }, {});
  const lanes = bizOps(out).find((o) => o.op === "lanes" && o.start.expr === "d.__a");
  assert.ok(lanes, "a lanes op over the labels");
  assert.ok(lanes.max.expr.includes("box.h"), "only as many lanes as the box holds");
  assert.ok(bizOps(out).some((o) => o.op === "union"), "the axis spans events and eras");
  assert.ok(bizFind(out, "eras") && bizFind(out, "stems") && bizFind(out, "labels") && bizFind(out, "dots"), "drawn in layers");
});

test("timeline: eras in rows — packed so an era inside another sits past it, or on a level of their own — under or over the axis", () => {
  const out = timeline.__expand({ data: "ev", date: "year", label: "what", xType: "linear", eras: "eras" }, {});
  const eras = Object.entries(out.tables).find(([, t]) => t.from === "eras" && t.ops.some((o) => o.op === "lanes"));
  assert.ok(eras, "the eras packed into rows by the engine");
  const [name, t] = eras;
  assert.deepEqual(t.ops.find((o) => o.op === "sort").by, [["__s", "asc"], ["__e", "desc"]], "the longer first where two start together: an era before the eras inside it");
  assert.equal(t.ops.find((o) => o.op === "lanes").as, "__row");
  const lanes = bizOps(out).find((o) => o.op === "lanes" && o.start.expr === "d.__a");
  assert.ok(lanes.max.expr.includes(`table.max(${JSON.stringify(name)}, "__row")`), "the rows take room from the labels");
  const span = bizNodes(bizFind(out, "eras")).find((n) => n.key === "span");
  assert.ok(span.pickable && span.opacity.expr.includes("hover()"), "an era hovers for its name and span");
  assert.ok(span.geom.y.expr.includes("+ 5 + d.__row * 19"), "rows go down from under the axis");
  const layers = out.template.children.map((c) => c.key);
  assert.ok(layers.indexOf("stems") < layers.indexOf("eras"), "a stem runs from its dot behind the eras");

  const over = timeline.__expand({ data: "ev", date: "year", label: "what", xType: "linear", eras: "eras", eraSide: "above", eraLevel: "level", eraColor: "kind" }, {});
  const levelled = Object.values(over.tables).find((t) => t.from === "eras" && t.ops.some((o) => o.as === "__row"));
  assert.ok(!levelled.ops.some((o) => o.op === "lanes"), "given levels, no packing");
  assert.ok(levelled.ops[0].expr.expr.includes("d.level"));
  const up = bizNodes(bizFind(over, "eras")).find((n) => n.key === "span");
  assert.ok(up.geom.y.expr.includes("- 21 - d.__row * 19"), "rows go up from over the axis");
  assert.equal(over.template.scales.era.domain.field, "kind", "coloured by a field of the eras");
});

test("radar: spokes in order, rings at nice steps as rows, one shape per series", () => {
  const out = radar.__expand({ data: "t", axis: "skill", value: "score", series: "who", levels: 5, max: 5 }, {});
  assert.ok(bizOps(out).some((o) => o.op === "units"), "rings: one row per spoke and ring");
  const chart = bizFind(out, "radar");
  assert.equal(chart.scales.ang.range[1], 2 * Math.PI);
  assert.equal(chart.scales.r.domain.field, "__max");
  const shapes = bizFind(out, "shapes");
  assert.deepEqual(shapes.children[0].from, { groups: Object.keys(out.tables).find((k) => k.startsWith("radar:radar:")), by: "who" });
  assert.equal(out.template.scales.color.type, "categorical");
});

test("table: rows keyed and placed by a sort, headers that sort, optional columns, sparklines matched on the key", () => {
  const columns = [
    { field: "shop" }, { field: "city", optional: true }, { field: "revenue", format: "$,.0f", bar: true },
    { field: "growth", format: "+.1%", color: "diverging" }, { label: "Trend", spark: { data: "monthly", x: "m", y: "revenue" } },
  ];
  const out = stdTable.__expand({ data: "shops", key: "shop", columns, sort: "revenue" }, {});
  const ops = bizOps(out);
  assert.ok(ops.some((o) => o.fn === "cumsum" && o.order === "-revenue" && o.as === "__pos"), "largest first");
  assert.ok(ops.some((o) => o.as === "__V1"), "the optional column's visibility, from the widths");
  assert.ok(ops.some((o) => o.op === "join" && o.on[0] === "shop"), "sparklines: the series joined on the key");
  assert.ok(ops.some((o) => o.as === "__sy4"), "…its y copied so the join can't rename it");
  const row = bizNodes(out.template).find((n) => n.key?.expr === "d.shop");
  assert.ok(row.transform.translate[1].expr.includes("d.__pos - 1"), "a row's place is its position: a new sort slides it");
  assert.equal(out.template.scales.c3.type, "diverging");
  const sortable = stdTable.__expand({ data: "shops", key: "shop", columns, sortable: "sortBy" }, {});
  const pos = bizOps(sortable).find((o) => o.as === "__pos").expr.expr;
  assert.ok(pos.includes('sortBy == "revenue" || sortBy == "-revenue"') && pos.includes('sortBy == "+revenue" ? d.__r2'), "a field sorts its own way, + and - either way");
  assert.ok(bizOps(sortable).some((o) => o.fn === "cumsum" && o.as === "__r2" && o.order === "revenue"), "numbers reversed: smallest first");
  assert.ok(bizOps(sortable).some((o) => o.fn === "cumsum" && o.as === "__r0" && o.order === "-shop"), "text reversed: Z to A");
  const head = bizFind(sortable, "h2");
  assert.equal(head.on.activate.set, "sortBy");
  assert.ok(head.on.activate.value.expr.endsWith('? "+revenue" : "revenue"'), "a click sorts by the column; another reverses it");
  // The whole cell takes the click (the pointer finds shapes, not groups): a pointer over it.
  const hit = head.children.find((n) => n.key === "hit");
  assert.ok(hit && hit.pickable && hit.fill === "transparent", "a hit area the pointer finds");
  assert.equal(head.semantics.role, "control");
  assert.ok(head.semantics.label.expr.includes("revenue, sorted ") && head.semantics.label.expr.includes("Sort by revenue"), "says how it sorts");
  // The label never changes (a changed text cross-fades: a blink); an arrow fades in and turns.
  const label = head.children.find((n) => n.key === "text");
  assert.equal(label.text, "revenue");
  assert.ok(label.style.ink.expr.includes("hover()"), "it lights up under the pointer");
  const arrow = head.children.find((n) => n.key === "arrow");
  assert.ok(arrow.opacity.expr.includes('sortBy == "revenue"') && arrow.transform.rotate.expr.endsWith("? 0 : 180"));
  assert.ok(!bizFind(sortable, "h4").on && !bizFind(sortable, "h4").children.some((n) => n.key === "hit"), "a sparkline column doesn't sort");
  // Not sortable: no hit area, no pointer; a fixed sort's arrow is just there.
  const fixed = bizFind(out, "h2");
  assert.ok(!fixed.on && !fixed.children.some((n) => n.key === "hit") && !fixed.semantics);
  assert.equal(fixed.children.find((n) => n.key === "arrow").opacity.expr, "true ? 1 : 0");
  assert.ok(!bizFind(out, "h0").children.some((n) => n.key === "arrow"));
});

test("tileMap: a built-in grid as its own rows (op.values), data joined on the id", () => {
  const out = tileMap.__expand({ data: "u", key: "state", value: "rate" }, {});
  const values = bizOps(out).find((o) => o.op === "values");
  assert.equal(values.values.id.length, 51, "fifty states and DC");
  assert.deepEqual(values.key, ["id"]);
  const at = values.values.id.indexOf("NY");
  assert.deepEqual([values.values.col[at], values.values.row[at], values.values.name[at]], [8, 2, "New York"]);
  assert.ok(bizOps(out).some((o) => o.op === "join" && o.kind === "left"), "places without data stay, neutral");
  const eu3 = bizOps(tileMap.__expand({ data: "c", key: "iso", value: "v", layout: "europe" }, {})).find((o) => o.op === "values");
  const eu2 = bizOps(tileMap.__expand({ data: "c", key: "iso", value: "v", layout: "europe", codes: "alpha2", shape: "hex" }, {})).find((o) => o.op === "values");
  assert.ok(eu3.values.id.includes("DEU") && eu2.values.id.includes("DE"));
  assert.equal(eu3.values.abbr[eu3.values.id.indexOf("DEU")], "DE", "tiles read the short code either way");
  const own = tileMap.__expand({ data: "c", key: "id", value: "v", layout: "myGrid" }, {});
  assert.ok(Object.values(own.tables).some((t) => t.from === "myGrid"), "or a layout table of your own");
});

// Real land borders, for checking that the built-in grids keep neighbours touching.
const US_BORDERS = "AL FL GA MS TN|AZ CA NM NV UT|AR LA MO MS OK TN TX|CA NV OR|CO KS NE NM OK UT WY|CT MA NY RI|DE MD NJ PA|DC MD VA|FL GA|GA NC SC TN|ID MT NV OR UT WA WY|IL IA IN KY MO WI|IN KY MI OH|IA MN MO NE SD WI|KS MO NE OK|KY MO OH TN VA WV|LA MS TX|ME NH|MD PA VA WV|MA NH NY RI VT|MI OH WI|MN ND SD WI|MS TN|MO NE OK TN|MT ND SD WY|NE SD WY|NV OR UT|NH VT|NJ NY PA|NM OK TX|NY PA VT|NC SC TN VA|ND SD|OH PA WV|OK TX|OR WA|PA WV|SD WY|TN VA|UT WY|VA WV";
const EU_BORDERS = "NOR SWE FIN|SWE FIN|IRL GBR|EST LVA|NLD BEL DEU|DNK DEU|LVA LTU BLR|BEL DEU LUX FRA|DEU LUX FRA CHE AUT CZE POL|POL CZE SVK UKR BLR LTU|LTU BLR|BLR UKR|FRA LUX CHE ITA ESP|CZE SVK AUT|SVK UKR HUN AUT|UKR HUN ROU MDA|PRT ESP|CHE AUT ITA|AUT HUN SVN ITA|HUN ROU SRB HRV SVN|ROU MDA BGR SRB|ITA SVN|SVN HRV|HRV SRB BIH MNE|SRB BGR MKD BIH MNE|BGR MKD GRC TUR|MNE BIH ALB|MKD GRC ALB|GRC ALB TUR";
const borders = (text) => text.split("|").flatMap((l) => { const [a, ...bs] = l.split(" "); return bs.map((b) => [a, b]); });
/** Does tile b touch tile a? Squares by an edge or a corner; hexagons by a side (odd rows half a
 * tile right, as the recipe draws them). */
function touching(a, b, hex) {
  const dc = b.col - a.col, dr = b.row - a.row;
  if (!hex) return Math.max(Math.abs(dc), Math.abs(dr)) === 1;
  if (dr === 0) return Math.abs(dc) === 1;
  if (Math.abs(dr) !== 1) return false;
  return a.row % 2 === 0 ? dc === -1 || dc === 0 : dc === 0 || dc === 1;
}

test("tileMap: in every built-in grid, square or hex, neighbours touch — no place cut off from all of them", () => {
  for (const [layout, text, codes] of [["us", US_BORDERS, undefined], ["europe", EU_BORDERS, "alpha3"]]) {
    for (const shape of ["square", "hex"]) {
      const v = bizOps(tileMap.__expand({ data: "d", key: "k", value: "v", layout, codes, shape }, {})).find((o) => o.op === "values").values;
      const tile = Object.fromEntries(v.id.map((id, i) => [id, { col: v.col[i], row: v.row[i] }]));
      const cells = new Set(v.id.map((id) => `${tile[id].col},${tile[id].row}`));
      assert.equal(cells.size, v.id.length, `${layout} ${shape}: one place per tile`);
      const pairs = borders(text);
      for (const [a, b] of pairs) assert.ok(tile[a] && tile[b], `${layout}: ${a}, ${b} in the grid`);
      const kept = pairs.filter(([a, b]) => touching(tile[a], tile[b], shape === "hex"));
      const ids = [...new Set(pairs.flat())];
      const cut = ids.filter((id) => !kept.some(([a, b]) => a === id || b === id));
      // Florida once floated free of Georgia and Alabama in the hex grid (the square grid's rows,
      // shifted): every place with a land border touches at least one neighbour.
      assert.deepEqual(cut, [], `${layout} ${shape}: cut off from every neighbour`);
      // And most borders hold: the floors sit just under each grid's own (a hexagon touches six
      // tiles, a square eight with its corners). The square grids' rows shifted kept 69% (US) and
      // 68% (Europe) as hexagons.
      const floor = { "us square": 0.85, "us hex": 0.8, "europe square": 0.88, "europe hex": 0.76 }[`${layout} ${shape}`];
      assert.ok(kept.length / pairs.length >= floor, `${layout} ${shape}: ${kept.length} of ${pairs.length} borders kept`);
    }
  }
});
