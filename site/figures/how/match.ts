// Under the hood, §5 (motion): who becomes whom. Keyed bars change between two scenes: a key on
// both sides is a pair and moves, a key only on the new side enters from a ghost (a bar grows
// from its axis), a key only on the old side exits into one. Then one bar's key becomes the
// parent of unit keys ("D", 0…5): the bar splits into its pieces, and stepping back merges them.
// Nothing here names a matcher: pairs by path, and splits by identity, are the defaults.
import { data, doc, e, geom, group, motion, op, repeat, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { box, label, line, narration, PHONE, SIZE, WIDE } from "./_kit";

const K = ["A", "B", "C", "D", "E"];
// Heights before and after, and each key's slot (left to right) in each scene; -1: not there.
const V0 = [5, 8, 4, 6, 0];
const V1 = [0, 6, 7, 6, 3];
const S0 = [0, 1, 2, 3, -1];
const S1 = [-1, 2, 1, 3, 4];

const MAXV = 9;
const slotX = (slot: string) => `(${slot}) * box.w / 5 + box.w / 10`;
const barW = "min(56, box.w / 5 - 18)";
const base = "box.h - 26";
const unitH = `(${base} - 16) / ${MAXV}`; // px per unit of value

/** The bars: every key present in this phase except a key that has split. */
const bars = (): Template => group({
  key: "bars",
  children: [
    line("axis", 0, e(base), e("box.w"), e(base), { ink: "$rule" }),
    // Each bar keyed by its row's key itself (the key a split compares with its units' keys).
    repeat("shown", shape(geom.rect({ x: e(`${slotX("d.slot")} - ${barW} / 2`), y: e(`${base} - d.v * ${unitH}`), w: e(barW), h: e(`d.v * ${unitH}`), r: 2 }), {
      key: e("d.k"), fill: e("'$categorical[' + d.i + ']'"), semantics: { role: "datum", label: e("d.k + ': ' + d.v") },
    })),
    repeat("shown", label(e("d.k + '-key'"), e("'(\"' + d.k + '\",)'"), [e(slotX("d.slot")), e(`${base} + 8`)], { size: SIZE.small, ink: "$ink-2", align: "middle", baseline: "top" })),
    // D's units: a 2 × 3 block of squares where the bar stood, each keyed ("D", unit).
    repeat("units", shape(geom.rect({
      x: e(`${slotX("3")} - ${barW} / 2 + (d.__unit % 2) * (${barW} / 2 + 1)`),
      y: e(`${base} - (floor(d.__unit / 2) + 1) * (${barW} / 2 + 1) + 1`),
      w: e(`${barW} / 2 - 1`), h: e(`${barW} / 2 - 1`), r: 2,
    }), { fill: "$categorical[3]", semantics: { role: "datum", label: e("'D, unit ' + d.__unit") } })),
    label("units-key", '("D", 0…5)', [e(slotX("3")), e(`${base} + 8`)], { size: SIZE.small, ink: "$ink-2", align: "middle", baseline: "top" }, { when: e("phase == 2") }),
    // What happened to each key, over where it happened.
    group({ key: "notes", children: [
      // Where A stood: an outline of the bar that left.
      shape(geom.rect({ x: e(`${slotX("0")} - ${barW} / 2`), y: e(`${base} - 5 * ${unitH}`), w: e(barW), h: e(`5 * ${unitH}`), r: 2 }), { key: "ghost", when: e("phase == 1"), stroke: { paint: "$rule", width: 1, dash: [4, 3] }, semantics: { role: "decoration" } }),
      label("exit", "exit", [e(slotX("0")), e(`${base} - 5 * ${unitH} - 14`)], { size: SIZE.small, ink: "$accent", align: "middle", strong: true }, { when: e("phase == 1") }),
      label("update", "pairs: move", [e(slotX("2")), 12], { size: SIZE.small, ink: "$accent", align: "middle", strong: true }, { when: e("phase == 1") }),
      line("update-rule", e(`${slotX("1")} - ${barW} / 2`), 24, e(`${slotX("3")} + ${barW} / 2`), 24, { ink: "$accent", when: e("phase == 1") }),
      label("enter", "enter", [e(slotX("4")), e(`${base} - 3 * ${unitH} - 14`)], { size: SIZE.small, ink: "$accent", align: "middle", strong: true }, { when: e("phase == 1") }),
      label("split", "split", [e(slotX("3")), e(`${base} - 3 * (${barW} / 2 + 1) - 14`)], { size: SIZE.small, ink: "$accent", align: "middle", strong: true }, { when: e("phase == 2") }),
    ] }),
  ],
});

const ROWS = [
  ["", "", ""],
  ['("A",)', "→ nothing", "exit"],
  ['("B",)', '→ ("B",)', "pair"],
  ['("C",)', '→ ("C",)', "pair"],
  ['("D",)', '→ ("D",)', "pair"],
  ["nothing", '→ ("E",)', "enter"],
];
const SPLIT = [
  ["", "", ""],
  ['("B",)', '→ ("B",)', "pair"],
  ['("C",)', '→ ("C",)', "pair"],
  ['("D",)', '→ ("D", 0…5)', "split"],
  ['("E",)', '→ ("E",)', "pair"],
  ["", "", ""],
];

/** The correspondence the planner builds, as a list. */
const table = (): Template => group({
  key: "corr",
  children: [
    label("head", "CORRESPONDENCE", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    box("card", 0, 20, e("box.w"), e("box.h - 20")),
    label("empty", "planned when the scene changes", [14, 44], { size: SIZE.small, ink: "$muted" }, { when: e("phase == 0") }),
    ...[1, 2, 3, 4, 5].map((i) => group({ key: `r${i}`, when: e(`phase > 0 && (phase == 1 ? ${JSON.stringify(ROWS[i][0])} : ${JSON.stringify(SPLIT[i][0])}) != ""`), children: [
      label("from", e(`phase == 1 ? ${JSON.stringify(ROWS[i][0])} : ${JSON.stringify(SPLIT[i][0])}`), [14, 22 + i * 24], { size: SIZE.small, ink: "$ink" }),
      label("to", e(`phase == 1 ? ${JSON.stringify(ROWS[i][1])} : ${JSON.stringify(SPLIT[i][1])}`), [70, 22 + i * 24], { size: SIZE.small, ink: "$ink" }),
      label("what", e(`phase == 1 ? ${JSON.stringify(ROWS[i][2])} : ${JSON.stringify(SPLIT[i][2])}`), [e("box.w - 14"), 22 + i * 24], { size: SIZE.small, ink: e(`(phase == 1 ? ${JSON.stringify(ROWS[i][2])} : ${JSON.stringify(SPLIT[i][2])}) == "pair" ? "$muted" : "$accent"`), align: "end", strong: true }),
    ] })),
  ],
});

const wide = group({
  key: "layout",
  when: e(WIDE),
  layout: { type: "rows", gap: 12, padding: [14, 18, 10, 18] },
  children: [
    group({ key: "content", layout: { type: "columns", gap: 28 }, children: [bars(), group({ key: "side", size: { w: 220 }, children: [table()] })] }),
    narration(2),
  ],
});
const phone = group({
  key: "layout",
  when: e(PHONE),
  layout: { type: "rows", gap: 12, padding: [12, 12, 8, 12] },
  children: [bars(), group({ key: "side", size: { h: 170 }, children: [table()] }), narration(3)],
});

export default doc({
  id: "how-match",
  title: "Pairs, enters, exits and splits",
  description: "Keyed bars between two scenes: B, C and D pair and move, A exits, E enters; then D's bar splits into six unit squares keyed (\"D\", 0…5).",
  size: [680, 300],
  data: { items: data.values({ k: K, i: [0, 1, 2, 3, 4], v0: V0, v1: V1, s0: S0, s1: S1 }, { key: "k" }) },
  signals: { phase: signal.num(0) },
  tables: {
    shown: { from: "items", ops: [
      op.filter(e("phase == 0 ? d.s0 >= 0 : phase == 1 ? d.s1 >= 0 : d.s1 >= 0 && d.k != 'D'")),
      op.derive("slot", e("phase == 0 ? d.s0 : d.s1")),
      op.derive("v", e("phase == 0 ? d.v0 : d.v1")),
    ] },
    units: { from: "items", ops: [op.filter(e("phase == 2 && d.k == 'D'")), op.units({ value: "v1" })] },
  },
  scene: group({ key: "root", children: [wide, phone] }),
  motion: motion(
    { duration: 1.1 },
    { select: { role: "datum" }, enter: { scale: 0, origin: "bottom" }, exit: { scale: 0, origin: "bottom" } },
  ),
  program: story({ steps: [
    step("before", { set: { phase: 0 }, text: "The scene on screen: four bars, each keyed by its row." }),
    step("after", { set: { phase: 1 }, text: "A new scene. Keys on both sides pair and move; E is new and grows from its axis; A is gone and shrinks into it." }),
    step("split", { set: { phase: 2 }, text: "D's key is now the parent of six unit keys: its bar splits into pieces that fly to their places. Back, they merge." }),
  ] }),
});
