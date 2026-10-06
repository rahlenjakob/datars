// Exit and enter ghosts: the state a mark leaves to, and the state it appears from. Each panel's
// marks exist only in the first step; its motion rule gives them a different ghost, as both `exit`
// and `enter` — step forward to see them leave, back to see them arrive.
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, ghost, type Ghost, type Rule } from "@datars/sdk";

const GHOSTS: [string, string, Ghost][] = [
  ["fade", "ghost.fade()", ghost.fade()],
  ["grow", 'ghost.grow("bottom")', ghost.grow("bottom")],
  ["grow-center", 'ghost.grow("center")', ghost.grow("center")],
  ["slide", "ghost.slide(0, -24)", ghost.slide(0, -24)],
  ["mixed", '{ opacity: 0, scale: 0.4, origin: "center" }', { opacity: 0, scale: 0.4, origin: "center" }],
  ["trim", "{ trim: 0 }", { trim: 0 }],
];

const bars = repeat("bars", shape(geom.rect({ x: e("12 + d.i * (box.w - 24) / 4 + 3"), y: e("box.h - 26 - d.v * (box.h - 64)"), w: e("(box.w - 24) / 4 - 6"), h: e("d.v * (box.h - 64)"), r: 2 }), {
  key: e("d.i"), when: e("show"), fill: "$accent", semantics: { role: "datum", label: e("'bar ' + d.i") },
}));
const line = shape(geom.polyline({ from: "bars", x: e("16 + d.i * (box.w - 32) / 3"), y: e("box.h - 26 - d.v * (box.h - 64)"), curve: "monotone" }), { key: "line", when: e("show"), stroke: { paint: "$accent", width: 3, cap: "round" } });

const panel = ([key, label]: [string, string, Ghost]) => group({
  key,
  children: [
    shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 6 }), { key: "bg", fill: "$surface" }),
    text(label, [8, 8], { key: "label", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top", maxWidth: e("box.w - 16") } }),
    shape(geom.segment({ x1: 12, y1: e("box.h - 26"), x2: e("box.w - 12"), y2: e("box.h - 26") }), { key: "base", stroke: { paint: "$rule", width: 1 } }),
    key === "trim" ? line : bars,
  ],
});

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 10, padding: [10, 12, 10, 12] }, children: GHOSTS.map(panel) });

export default doc({
  id: "sdk-motion-ghosts",
  title: "Enter and exit ghosts",
  description: "Six panels whose marks leave and arrive in different ways: fading, growing from the bottom, growing from the centre, sliding, a mix of fading and growing, and a line drawing on and off.",
  size: [640, 260],
  data: { bars: data.values({ i: [0, 1, 2, 3], v: [0.55, 0.9, 0.4, 0.7] }, { key: "i" }) },
  signals: { show: signal.bool(true) },
  scene: group({ key: "figure", children: [grid(3, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
  motion: motion(
    { duration: 1.4, easing: "cubic-out" },
    ...GHOSTS.map(([key, , g]): Rule => ({ select: { key: `figure/root/${key}` }, enter: g, exit: g })),
  ),
  program: story({ steps: [step("shown", { set: { show: true } }), step("gone", { set: { show: false } })] }),
});
