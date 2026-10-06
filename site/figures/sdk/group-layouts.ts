// `group({ layout })`: how a group places its children in its box. Each panel is one group (its
// box tinted) with padding 8 and gap 6; its children draw a rectangle over the box they're given.
import { doc, e, geom, group, shape, text, type Layout, type Template } from "@datars/sdk";

/** A child that shows the box the layout gave it. */
const child = (name: string, i: number) => group({
  key: name,
  children: [
    shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 3 }), { key: "box", fill: `$categorical[${i}]`, opacity: 0.85 }),
    text(name, [e("box.w / 2"), e("box.h / 2")], { key: "name", style: { font: "font.strong", size: "$size.label", ink: `on($categorical[${i}])`, align: "middle", baseline: "middle" } }),
  ],
});
/** A child with a size of its own (a flow measures what its children draw). */
const chip = (name: string, i: number, w: number) => group({
  key: name,
  children: [
    shape(geom.rect({ x: 0, y: 0, w, h: 22, r: 11 }), { key: "box", fill: `$categorical[${i}]`, opacity: 0.85 }),
    text(name, [w / 2, 11], { key: "name", style: { font: "font.strong", size: "$size.label", ink: `on($categorical[${i}])`, align: "middle", baseline: "middle" } }),
  ],
});

const panel = (name: string, layout: Layout, children: Template[], note: string) => group({
  key: name,
  layout: { type: "rows", gap: 6 },
  children: [
    text(`"${name}"`, [0, 0], { key: "title", size: { h: 16 }, style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } }),
    group({ key: "demo", children: [
      shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 4 }), { key: "box", fill: "$surface", stroke: { paint: "$rule", width: 1, dash: [3, 3] } }),
      group({ key: "group", layout: { gap: 6, padding: 8, ...layout }, children }),
    ] }),
    text(note, [0, 0], { key: "note", size: { h: 28 }, style: { size: "$size.small", ink: "$muted", baseline: "top", maxWidth: e("box.w") } }),
  ],
});

const panels = (): Template[] => [
  panel("stack", { type: "stack" }, [
    group({ key: "A", children: [shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 3 }), { key: "box", fill: "$categorical[0]", opacity: 0.85 })] }),
    group({ key: "B", children: [shape(geom.circle({ cx: e("box.w / 2"), cy: e("box.h / 2"), r: e("min(box.w, box.h) / 3") }), { key: "dot", fill: "$categorical[1]" })] }),
    text("C", [e("box.w / 2"), e("box.h / 2")], { key: "C", style: { font: "font.title", size: 17, ink: "on($categorical[1])", align: "middle", baseline: "middle" } }),
  ], "each child fills the box, on top of the last"),
  panel("rows", { type: "rows" }, ["A", "B", "C"].map(child), "one under the other"),
  panel("columns", { type: "columns" }, ["A", "B", "C"].map(child), "side by side"),
  panel("grid", { type: "grid", columns: 2 }, ["A", "B", "C", "D", "E"].map(child), "equal cells, columns: 2"),
  panel("flow", { type: "flow" }, ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"].map((n, i) => chip(n, i, 16 + n.length * 7)), "as wide as they draw, wrapping"),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 16, padding: [12, 12, 4, 12] }, children: panels() });

export default doc({
  id: "sdk-group-layouts",
  title: "Group layouts",
  description: "Five groups with padding 8 and gap 6, laid out as stack, rows, columns, a two-column grid and a wrapping flow.",
  size: [640, 260],
  scene: group({ key: "figure", children: [grid(5, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
});
