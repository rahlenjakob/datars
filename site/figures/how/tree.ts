// Under the hood, §1 (the document): the votes example's scene as a tree, three times — as the
// document holds it (recipe calls, `use` nodes), expanded into primitives (what the T2 variant
// ships), and resolved for the first step (the scene: `when` decided, repeats turned into keyed
// nodes). Rows that are the same node keep their place in the tree and move with it.
// The trees are transcribed from examples/votes/doc.json, its expanded variant (`datars publish`)
// and `datars inspect examples/votes/doc.json --state 0`.
import { doc, e, geom, group, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { narration, PHONE, SIZE, WIDE } from "./_kit";

type Row = { id: string; depth: number; kind: string; label: string; note?: string; short?: string };

const DOCUMENT: Row[] = [
  { id: "root", depth: 0, kind: "group", label: "root", note: "layout: stack", short: "stack" },
  { id: "bars", depth: 1, kind: "use", label: "std/plot", note: 'key "chart" · when shape == "bars"', short: 'when "bars"' },
  { id: "bars-mark", depth: 2, kind: "use", label: "std/bar", note: "labels: true" },
  { id: "ranked", depth: 1, kind: "use", label: "std/plot", note: 'key "chart" · when shape == "ranked"', short: 'when "ranked"' },
  { id: "ranked-mark", depth: 2, kind: "use", label: "std/bar", note: "labels: true" },
  { id: "pie", depth: 1, kind: "use", label: "std/pie", note: 'key "chart" · when shape == "pie"', short: 'when "pie"' },
  { id: "donut", depth: 1, kind: "use", label: "std/pie", note: 'key "chart" · when shape == "donut" · inner: 0.58', short: 'when "donut"' },
  { id: "caption", depth: 1, kind: "use", label: "std/card", note: "the step's narration" },
];

const EXPANDED: Row[] = [
  { id: "root", depth: 0, kind: "group", label: "root" },
  { id: "bars", depth: 1, kind: "group", label: "chart", note: 'when shape == "bars"', short: 'when "bars"' },
  { id: "title", depth: 2, kind: "text", label: "title", note: '"Vote share by party, 2022 (%)"' },
  { id: "body", depth: 2, kind: "group", label: "body" },
  { id: "axis-y", depth: 3, kind: "group", label: "axis-y", note: "repeat over the y scale's ticks", short: "× ticks" },
  { id: "center", depth: 3, kind: "group", label: "center" },
  { id: "area", depth: 4, kind: "group", label: "area" },
  { id: "grid-y", depth: 5, kind: "group", label: "grid-y", note: "repeat over ticks → line", short: "× ticks" },
  { id: "bars-mark", depth: 5, kind: "group", label: "marks", note: 'repeat "votes" → rect (std/bar\'s expansion)', short: "× votes" },
  { id: "labels", depth: 6, kind: "group", label: "labels", note: 'repeat "votes" → text', short: "× votes" },
  { id: "axis-x", depth: 4, kind: "group", label: "axis-x", note: "repeat over the parties → tick, name", short: "× ticks" },
  { id: "ranked", depth: 1, kind: "group", label: "chart", note: 'when shape == "ranked" · the same, over "ranked"', short: 'when "ranked"' },
  { id: "pie", depth: 1, kind: "group", label: "chart", note: 'when shape == "pie" · repeat a derived table → arc', short: 'when "pie"' },
  { id: "donut", depth: 1, kind: "group", label: "chart", note: 'when shape == "donut"', short: 'when "donut"' },
  { id: "caption", depth: 1, kind: "group", label: "caption", note: "when the step has narration" },
];

const RESOLVED: Row[] = [
  { id: "root", depth: 0, kind: "group", label: '("root",)' },
  { id: "bars", depth: 1, kind: "group", label: '("chart",)', note: 'the one chart whose "when" holds' },
  { id: "title", depth: 2, kind: "text", label: '("title",)', note: '"Vote share by party, 2022 (%)"' },
  { id: "body", depth: 2, kind: "group", label: '("body",)' },
  { id: "axis-y", depth: 3, kind: "group", label: '("axis-y",)', note: "ticks (0,) (5,) … (35,)" },
  { id: "center", depth: 3, kind: "group", label: '("center",)' },
  { id: "area", depth: 4, kind: "group", label: '("area",)' },
  { id: "grid-y", depth: 5, kind: "group", label: '("grid-y",)', note: "a line per tick" },
  { id: "bars-mark", depth: 5, kind: "group", label: '("marks",)', note: "a rect per row of votes", short: "8 rects" },
  { id: "S", depth: 6, kind: "rect", label: '("S",)', note: '"Social Democrats: 30.3"', short: "30.3" },
  { id: "SD", depth: 6, kind: "rect", label: '("SD",)', note: '"Sweden Democrats: 20.5"', short: "20.5" },
  { id: "more", depth: 6, kind: "rect", label: "… six more", note: "(\"M\",) (\"V\",) … (\"L\",)" },
  { id: "labels", depth: 6, kind: "group", label: '("labels",)', note: 'a text per row, keyed ("S",) …' },
  { id: "axis-x", depth: 4, kind: "group", label: '("axis-x",)', note: "a tick per party, keyed by it" },
  { id: "caption", depth: 1, kind: "group", label: '("caption",)', note: "the first step's narration card" },
];

const TREES = [DOCUMENT, EXPANDED, RESOLVED];
const RH = 19; // row height
const IND = 16; // indent per level
const TOP = 4;

/** `phase == 0 ? a : phase == 1 ? b : c` over the phases a value is given for. */
function per<T>(vals: (T | undefined)[], fmt: (v: T) => string, fallback: string): string {
  let out = fallback;
  for (let i = vals.length - 1; i >= 0; i--) {
    const v = vals[i];
    if (v !== undefined) out = `phase == ${i} ? ${fmt(v)} : ${out}`;
  }
  return out;
}
const q = (s: string) => JSON.stringify(s);
const CHIP: Record<string, number> = { use: 28, group: 40, text: 30, rect: 30 };
const INK: Record<string, string> = { use: "$accent", group: "$muted", text: "$ink-2", rect: "$ink-2" };

const ids = [...new Set(TREES.flatMap((t) => t.map((r) => r.id)))];

function node(id: string, phone: boolean): Template {
  const at = TREES.map((t) => {
    const i = t.findIndex((r) => r.id === id);
    if (i < 0) return undefined;
    const r = t[i];
    // The parent: the nearest row above with one level less.
    let p = i - 1;
    while (p >= 0 && t[p].depth >= r.depth) p--;
    return { r, y: TOP + i * RH, py: p >= 0 ? TOP + p * RH : 0, pd: p >= 0 ? t[p].depth : 0 };
  });
  const present = at.map((a, i) => (a ? `phase == ${i}` : null)).filter(Boolean).join(" || ");
  const last = at.filter(Boolean).pop()!;
  const num = (f: (a: NonNullable<(typeof at)[number]>) => number) => e(per(at.map((a) => (a ? f(a) : undefined)), String, String(f(last))));
  const str = (f: (a: NonNullable<(typeof at)[number]>) => string) => e(per(at.map((a) => (a ? f(a) : undefined)), q, q(f(last))));
  const x = (a: NonNullable<(typeof at)[number]>) => a.r.depth * IND;
  const mid = (a: NonNullable<(typeof at)[number]>) => a.y + RH / 2;
  const chipW = (a: NonNullable<(typeof at)[number]>) => CHIP[a.r.kind];
  // The elbow from the parent's chip down and across to this row (none for the root).
  const elbow = (a: NonNullable<(typeof at)[number]>) => a.r.depth === 0 ? "M 0 0 L 0 0 L 0 0" : `M ${a.pd * IND + 6} ${a.py + RH - 3} L ${a.pd * IND + 6} ${mid(a)} L ${x(a) - 3} ${mid(a)}`;
  const size = phone ? SIZE.small : SIZE.label;
  return group({
    key: id,
    when: e(present),
    children: [
      shape(geom.path(str(elbow)), { key: "elbow", stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
      shape(geom.rect({ x: num(x), y: num((a) => a.y + 3), w: num(chipW), h: RH - 6, r: 3 }), { key: "chip", fill: str((a) => `${INK[a.r.kind]}@0.12`), semantics: { role: "decoration" } }),
      text(str((a) => a.r.kind), [num((a) => x(a) + chipW(a) / 2), num(mid)], { key: "kind", style: { size: 10, ink: str((a) => INK[a.r.kind]), align: "middle", baseline: "middle", font: "font.strong" } }),
      text(str((a) => a.r.label), [num((a) => x(a) + chipW(a) + 6), num(mid)], { key: "label", style: { size, ink: "$ink", baseline: "middle", font: "font.strong" } }),
      phone
        ? text(str((a) => a.r.short ?? ""), [e(`box.w`), num(mid)], { key: "note", style: { size: SIZE.small, ink: "$muted", baseline: "middle", align: "end" } })
        : text(str((a) => a.r.note ?? ""), [300, num(mid)], { key: "note", style: { size: SIZE.small, ink: "$muted", baseline: "middle" } }),
    ],
  });
}

const layout = (phone: boolean) => group({
  key: "layout",
  when: e(phone ? PHONE : WIDE),
  layout: { type: "rows", gap: 10, padding: phone ? [10, 12, 8, 12] : [12, 18, 10, 18] },
  children: [
    group({ key: "tree", children: ids.map((id) => node(id, phone)) }),
    narration(phone ? 3 : 2),
  ],
});

export default doc({
  id: "how-tree",
  title: "The votes chart as a tree",
  description: "The example's scene three times: as the document holds it (recipe calls), expanded into primitives with repeats, and resolved for the bars step into keyed nodes.",
  size: [794, 372],
  signals: { phase: signal.num(0) },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  program: story({ steps: [
    step("document", { set: { phase: 0 }, text: "The document: a root group and seven recipe calls (use nodes) with their parameters, keys and conditions. Nothing to draw yet." }),
    step("expanded", { set: { phase: 1 }, text: "Expanded, at publish time or on the device: groups, texts and shapes, with repeats over tables and ticks." }),
    step("resolved", { set: { phase: 2 }, text: "Resolved for the bars step: one chart left, every repeat turned into keyed nodes with values." }),
  ] }),
});
