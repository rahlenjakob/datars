// Under the hood, §6 (point pyramids): 4,096 generated rows, each with a priority (a seeded hash of
// its index). Level ℓ holds the fraction budget · 4^ℓ / n of all rows (datars_algo::pyramid::fraction:
// here 1/64, 1/16, 1/4, 1), and a row belongs to the shallowest level its priority falls under —
// so every level is a uniform sample of the data. A frame draws levels coarse to fine while the
// rows in view, times the next level's growth (×4), fit its budget (crates/datars-engine/src/points/
// fill.rs): zoomed out it stops early, zoomed in it goes on to every row. Every count on the
// figure is computed by the engine from the rows; the budget is figure-sized (the engine's default
// is 150,000 rows a frame).
import { data, doc, e, geom, group, instances, motion, op, shape, signal, step, story, view, type Rule, type Template } from "@datars/sdk";
import { label, narration, PHONE, SIZE } from "./_kit";

const N = 4096;
const BUDGET = 500; // rows a frame may draw, in this figure
const W = 960, H = 640;
const inView = "d.x >= bx0 && d.x <= bx1 && d.y >= by0 && d.y <= by1";
const count = (l: number) => `table.count("v${l}")`;
// Levels drawn: while what's drawn, ×4, still fits the budget, the next level joins.
const DRAWN = `(${count(0)} * 4 <= ${BUDGET} ? (${count(1)} * 4 <= ${BUDGET} ? (${count(2)} * 4 <= ${BUDGET} ? 3 : 2) : 1) : 0)`;
const COLORS = ["$categorical[0]", "$categorical[2]", "$categorical[3]", "$categorical[1]"];

const screen = (): Template => group({
  key: "screen",
  children: [
    label("head", "THE VIEW", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    view({
      key: "view",
      transform: { translate: [0, 20] },
      size: { w: e("box.w"), h: e("box.w / 1.5") },
      camera: { fit: { bbox: [e("bx0"), e("by0"), e("bx1"), e("by1")] }, padding: 0 },
      children: [
        shape(geom.rect({ x: 0, y: 0, w: W, h: H }), { key: "plane", fill: "$surface", semantics: { role: "decoration" } }),
        instances({
          key: "rows", from: "drawn", x: e("d.x"), y: e("d.y"), r: 2.2, screenSize: true, instanceKey: e("d.i"),
          fill: e(`d.level == 0 ? "${COLORS[0]}" : d.level == 1 ? "${COLORS[1]}" : d.level == 2 ? "${COLORS[2]}" : "${COLORS[3]}"`),
        }),
      ],
    }),
  ],
});

/** Per level: its fraction of all rows, and how many of its rows (and the levels above) are in view. */
const side = (): Template => group({
  key: "side",
  children: [
    label("head", "LEVELS", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    label("h-frac", "of all rows", [e("box.w - 70"), 30], { size: SIZE.small, ink: "$muted", align: "end" }),
    label("h-view", "in view, 0–ℓ", [e("box.w"), 30], { size: SIZE.small, ink: "$muted", align: "end" }),
    ...[0, 1, 2, 3].map((l) => group({ key: `l${l}`, opacity: e(`${DRAWN} >= ${l} ? 1 : 0.4`), children: [
      shape(geom.circle({ cx: 5, cy: 54 + l * 24, r: 5 }), { key: "dot", fill: COLORS[l], semantics: { role: "decoration" } }),
      label("name", `level ${l}`, [16, 54 + l * 24], { strong: true }),
      label("frac", ["1/64", "1/16", "1/4", "all"][l], [e("box.w - 70"), 54 + l * 24], { ink: "$ink-2", align: "end" }),
      label("n", e(`format(${count(l)}, ",.0f")`), [e("box.w"), 54 + l * 24], { ink: "$ink-2", align: "end" }),
    ] })),
    shape(geom.segment({ x1: 0, y1: 150, x2: e("box.w"), y2: 150 }), { key: "rule", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
    label("budget", `frame budget: ${BUDGET} rows`, [0, 166], { ink: "$ink-2" }),
    label("draws", e(`'draws ' + (${DRAWN} == 0 ? 'level 0' : 'levels 0–' + ${DRAWN}) + ': ' + format(${DRAWN} == 0 ? ${count(0)} : ${DRAWN} == 1 ? ${count(1)} : ${DRAWN} == 2 ? ${count(2)} : ${count(3)}, ',.0f') + ' rows'`), [0, 188], { strong: true }),
  ],
});

const layout = (phone: boolean) => group({
  key: phone ? "phone" : "wide",
  when: e(phone ? PHONE : `!(${PHONE})`),
  layout: { type: "rows", gap: 14, padding: phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: [
    group({ key: "main", layout: phone ? { type: "rows", gap: 16 } : { type: "columns", gap: 32 }, children: [
      group({ key: "screen-at", size: phone ? { h: e("box.w / 1.5 + 20") } : { w: e("min(box.w - 290, (box.h - 24) * 1.5)") }, children: [screen()] }),
      group({ key: "side-at", size: phone ? { h: 200 } : undefined, children: [side()] }),
    ] }),
    narration(2, 3),
  ],
});

// New detail fades in once the camera is there (0.35 s, as ARRIVE_S); leaving detail fades out.
const rules: Rule[] = [{ duration: 1.2, easing: "cubic-in-out" }];
for (const l of ["wide", "phone"]) {
  rules.push({ select: { key: `root/${l}/main/screen-at/screen/view/rows` }, delay: 1.2, duration: 0.35, enter: { opacity: 0 }, exit: { opacity: 0 } });
  rules.push({ when: { to: "whole" }, select: { key: `root/${l}/main/screen-at/screen/view/rows` }, delay: 0, duration: 0.35, enter: { opacity: 0 }, exit: { opacity: 0 } });
}
const box = (b: number[]) => ({ bx0: b[0], by0: b[1], bx1: b[2], by1: b[3] });
const WHOLE = [0, 0, W, H];
const NEAR = [150, 150, 450, 350];
const NEARER = [270, 230, 330, 270];

export default doc({
  id: "how-points",
  title: "A point pyramid",
  description: "4,096 generated rows in four levels, each a uniform sample of the data: the whole view draws the two coarsest levels, zoomed in the frame draws three, zoomed further every row in view.",
  size: [680, 340],
  data: {
    pts: data.generate(N, {
      i: e("d.i"),
      c: e("rand(d.i, 1)"),
      x: e("clamp(d.c < 0.5 ? 300 + randn(d.i, 2) * 50 : d.c < 0.82 ? 660 + randn(d.i, 3) * 90 : rand(d.i, 4) * 960, 4, 956)"),
      y: e("clamp(d.c < 0.5 ? 250 + randn(d.i, 5) * 45 : d.c < 0.82 ? 420 + randn(d.i, 6) * 70 : rand(d.i, 7) * 640, 4, 636)"),
      // A row's priority, and the shallowest level whose fraction (64 · 4^ℓ / 4096) it falls under.
      p: e("rand(d.i, 8)"),
      level: e("d.p < 1 / 64 ? 0 : d.p < 1 / 16 ? 1 : d.p < 1 / 4 ? 2 : 3"),
    }, { key: "i", keep: ["i", "x", "y", "level"] }),
  },
  signals: { ...Object.fromEntries(Object.entries(box(WHOLE)).map(([k, v]) => [k, signal.num(v)])) },
  tables: {
    v0: { from: "pts", ops: [op.filter(e(`d.level <= 0 && ${inView}`))] },
    v1: { from: "pts", ops: [op.filter(e(`d.level <= 1 && ${inView}`))] },
    v2: { from: "pts", ops: [op.filter(e(`d.level <= 2 && ${inView}`))] },
    v3: { from: "pts", ops: [op.filter(e(`d.level <= 3 && ${inView}`))] },
    drawn: { from: "pts", ops: [op.filter(e(`d.level <= ${DRAWN}`))] },
  },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  motion: motion(...rules),
  program: story({ steps: [
    step("whole", { set: box(WHOLE), text: "The whole view: level 1's rows fit the budget, level 2's would not (×4), so the frame draws levels 0 and 1." }),
    step("closer", { set: box(NEAR), text: "Zoomed in, fewer rows are in view: level 2 fits too, and its rows fade in. Dense stays dense, sparse stays sparse." }),
    step("closest", { set: box(NEARER), text: "Closer still: every row in view fits, so all four levels draw. Nothing is re-resolved; tiles are built once and kept." }),
  ] }),
});
