// Matchers: how the marks of one scene pair with the marks of the next. In each panel six keyed dots
// (A–F) move from a column on the left to a shuffled column on the right — and to a different parent
// group, so their key paths change. `by-path` finds no pairs (they crossfade), `by-key` pairs each
// dot with itself wherever it went, `nearest` pairs by position, ignoring keys (colours swap).
import { data, doc, e, geom, group, motion, repeat, shape, signal, step, story, text, type Matcher, type Rule } from "@datars/sdk";

const MATCHERS: [string, Matcher, string][] = [
  ["by-path", "by-path", "same key path: none here"],
  ["by-key", "by-key", "same key, wherever it is"],
  ["nearest", "nearest", "closest position, any key"],
];
const ORDER = ["D", "A", "F", "B", "E", "C"];

const y = (i: string) => e(`34 + (${i}) * (box.h - 72) / 5`);
const panel = ([name, , note]: [string, Matcher, string]) => group({
  key: name,
  children: [
    shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 6 }), { key: "bg", fill: "$surface" }),
    text(`matcher: "${name}"`, [8, 8], { key: "title", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } }),
    text(note, [8, e("box.h - 6")], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "bottom" } }),
    // The dots' parent changes key between the steps: "left" → "right". Each dot and each letter
    // is keyed by the row's own key (a matcher by key compares marks' own keys).
    group({ key: e('moved ? "right" : "left"'), children: [
      repeat("dots", shape(geom.circle({ cx: e("moved ? box.w * 0.72 : box.w * 0.28"), cy: y("moved ? d.slot : d.i"), r: 9 }), { key: e("d.k"), fill: e("'$categorical[' + d.i + ']'") })),
      repeat("dots", text(e("d.k"), [e("moved ? box.w * 0.72 : box.w * 0.28"), y("moved ? d.slot : d.i")], { key: e("d.k + '-name'"), style: { font: "font.strong", size: "$size.small", ink: e("'on($categorical[' + d.i + '])'"), align: "middle", baseline: "middle" } })),
    ] }),
  ],
});

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 12, padding: [10, 12, 10, 12] }, children: MATCHERS.map(panel) });

export default doc({
  id: "sdk-motion-matchers",
  title: "Matchers",
  description: "Three panels of six lettered dots moving to a shuffled column in a new parent group: by path they crossfade, by key each dot travels to its new place, by nearest position they slide straight across and swap colours.",
  size: [640, 260],
  data: { dots: data.values({ k: ["A", "B", "C", "D", "E", "F"], i: [0, 1, 2, 3, 4, 5], slot: ["A", "B", "C", "D", "E", "F"].map((k) => ORDER.indexOf(k)) }, { key: "k" }) },
  signals: { moved: signal.bool(false) },
  scene: group({ key: "figure", children: [grid(3, 'sizeClass != "phone"'), grid(1, 'sizeClass == "phone"')] }),
  motion: motion(
    { duration: 1.6, easing: "cubic-in-out" },
    ...MATCHERS.map(([name, m]): Rule => ({ select: { key: `figure/root/${name}` }, matcher: m })),
  ),
  program: story({ steps: [step("before", { set: { moved: false } }), step("after", { set: { moved: true } })] }),
});
