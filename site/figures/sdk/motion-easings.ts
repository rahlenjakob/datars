// Easing curves. Each panel plots one easing (progress against time) and, when you step, moves a dot
// along it: the dot sits in two nested groups, the outer moved across with a `linear` rule and the
// inner moved up (its `transform`) with a rule whose `easing` is the panel's — so the engine itself
// traces the curve, overshoots included.
// The curves are computed here with the engine's formulas (crates/datars-motion/src/easing.rs).
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, type Rule } from "@datars/sdk";

const EASINGS = ["linear", "cubic-in-out", "quad-out", "expo-in", "back-out", "elastic-out", "bounce-out", "spring(120, 10)", "steps(5)", "cubic-bezier(.2,.8,.2,1)"];

// ---- the curves -------------------------------------------------------------------------------
const TAU = Math.PI * 2;
const bounceOut = (x: number) => {
  const n = 7.5625, d = 2.75;
  if (x < 1 / d) return n * x * x;
  if (x < 2 / d) { const u = x - 1.5 / d; return n * u * u + 0.75; }
  if (x < 2.5 / d) { const u = x - 2.25 / d; return n * u * u + 0.9375; }
  const u = x - 2.625 / d; return n * u * u + 0.984375;
};
const easeIn: Record<string, (x: number) => number> = {
  quad: (x) => x * x, cubic: (x) => x * x * x,
  expo: (x) => (x <= 0 ? 0 : 2 ** (10 * x - 10)),
  back: (x) => 2.70158 * x * x * x - 1.70158 * x * x,
  elastic: (x) => (x <= 0 ? 0 : x >= 1 ? 1 : -(2 ** (10 * x - 10)) * Math.sin((x * 10 - 10.75) * (TAU / 3))),
  bounce: (x) => 1 - bounceOut(1 - x),
};
function spring(stiffness: number, damping: number) {
  const w0 = Math.sqrt(stiffness), zeta = damping / (2 * Math.sqrt(stiffness));
  const wd = w0 * Math.sqrt(1 - zeta * zeta), b = (zeta * w0) / wd;
  const settle = Math.log(Math.sqrt(1 + b * b) / 1e-3) / (zeta * w0);
  const x = (tau: number) => Math.exp(-zeta * w0 * tau) * (Math.cos(wd * tau) + b * Math.sin(wd * tau));
  const xs = x(settle);
  return (t: number) => 1 - (x(t * settle) - t * xs);
}
function bezier(x1: number, y1: number, x2: number, y2: number) {
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  const sx = (s: number) => ((ax * s + bx) * s + cx) * s, sy = (s: number) => ((ay * s + by) * s + cy) * s;
  return (t: number) => {
    let lo = 0, hi = 1, s = t;
    for (let i = 0; i < 60; i++) { if (sx(s) < t) lo = s; else hi = s; s = (lo + hi) / 2; }
    return sy(s);
  };
}
function curve(name: string): (t: number) => number {
  if (name === "linear") return (t) => t;
  if (name.startsWith("spring")) return spring(120, 10);
  if (name.startsWith("cubic-bezier")) return bezier(0.2, 0.8, 0.2, 1);
  const [fam, dir] = [name.split("-")[0], name.slice(name.indexOf("-") + 1)];
  const f = easeIn[fam];
  if (dir === "in") return f;
  if (dir === "out") return (t) => 1 - f(1 - t);
  return (t) => (t < 0.5 ? f(2 * t) / 2 : 1 - f(2 - 2 * t) / 2);
}

const name: string[] = [], t: number[] = [], v: number[] = [];
const push = (n: string, u: number, p: number) => { name.push(n); t.push(u); v.push(p); };
for (const n of EASINGS) {
  if (n.startsWith("steps")) {
    // A staircase, drawn exactly: flat for each fifth of the time, then a jump.
    for (let k = 0; k < 5; k++) for (const u of [k / 5, (k + 1) / 5]) push(n, u, k / 5);
    push(n, 1, 1);
    continue;
  }
  const f = curve(n);
  for (let i = 0; i <= 96; i++) push(n, i / 96, Math.round(f(i / 96) * 1e5) / 1e5);
}

// ---- the panels ---------------------------------------------------------------------------------
// Progress −0.08 … 1.42 fits the overshoots (elastic-out peaks near 1.37); the plot sits under the
// panel's title, clear of it.
const X0 = 6, Y0 = 24, LO = -0.08, HI = 1.42;
const px = (u: string) => `(${X0} + (${u}) * (box.w - ${2 * X0}))`;
const py = (p: string) => `(${Y0} + (1 - ((${p}) - (${LO})) / ${HI - LO}) * (box.h - ${Y0 + 6}))`;

const panel = group({
  key: e("d.name"),
  children: [
    text(e("d.name"), [0, 2], { key: "title", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } }),
    shape(geom.rect({ x: e(px("0")), y: e(py("1")), w: e(`box.w - ${2 * X0}`), h: e(`${py("0")} - ${py("1")}`) }), { key: "frame", fill: "$surface", stroke: { paint: "$grid", width: 1 } }),
    shape(geom.segment({ x1: e(px("0")), y1: e(py("0")), x2: e(px("1")), y2: e(py("1")) }), { key: "diagonal", stroke: { paint: "$rule", width: 1, dash: [2, 3] } }),
    shape(geom.polyline({ from: "@group", x: e(px("d.t")), y: e(py("d.v")) }), { key: "curve", stroke: { paint: "$accent", width: 2, join: "round" } }),
    group({ key: "tx", transform: { translate: [e(px("t")), 0] }, children: [
      group({ key: "ty", transform: { translate: [0, e(py("t"))] }, children: [
        shape(geom.circle({ cx: 0, cy: 0, r: 5 }), { key: "dot", fill: "$highlight", stroke: { paint: "$paper", width: 1.5 } }),
      ] }),
    ] }),
  ],
});

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 14, padding: [10, 12, 8, 12] }, children: [repeat({ groups: "curves", by: "name" }, panel)] });

export default doc({
  id: "sdk-motion-easings",
  title: "Easings",
  description: "Ten easing curves — linear, cubic in-out, quad out, expo in, back out, elastic out, bounce out, a spring, five steps and a cubic Bézier — each with a dot that follows it when the step changes.",
  size: [640, 360],
  // A point per row, keyed by its easing and its place along the curve (a staircase has two points
  // at each jump).
  data: { curves: data.values({ name, i: name.map((_, j) => j), t, v }, { key: ["name", "i"] }) },
  signals: { t: signal.num(0) },
  scene: group({ key: "figure", children: [grid(5, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
  motion: motion(
    { duration: 2.4, easing: "linear" },
    ...EASINGS.map((n): Rule => ({ select: { key: `figure/root/${n}/tx/ty` }, easing: n })),
  ),
  program: story({ steps: [step("start", { set: { t: 0 } }), step("end", { set: { t: 1 } })] }),
});
