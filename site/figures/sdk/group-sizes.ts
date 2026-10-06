// Children's `size` along a `columns` layout — fixed px, a percentage, `auto` (what the content
// measures) and `fill` shares — then `align` on the cross axis. Step to a narrower box: fixed and
// auto keep their width, the rest re-share what's left (each width counts as it changes).
import { doc, e, geom, group, shape, signal, step, story, text, type Size, type Template } from "@datars/sdk";

// An `auto` child is as wide as what it draws, so its rectangle can't follow its own box.
const box = (name: string, i: number, w: Size, content?: Template) => group({
  key: name,
  size: { w },
  children: [
    shape(geom.rect({ x: 0, y: 0, w: w === "auto" ? 64 : e("box.w"), h: e("box.h"), r: 3 }), { key: "box", fill: `$categorical[${i}]`, opacity: 0.85 }),
    content ?? text(name, [e("box.w / 2"), e("box.h / 2 - 7")], { key: "name", style: { font: "font.strong", size: "$size.label", ink: `on($categorical[${i}])`, align: "middle", baseline: "middle" } }),
    text("", [w === "auto" ? 32 : e("box.w / 2"), e("box.h / 2 + 9")], { key: "px", number: { value: e("box.w"), format: ".0f" }, style: { size: "$size.small", ink: `on($categorical[${i}])`, align: "middle", baseline: "middle" } }),
  ],
});

const alignDemo = (align: "start" | "center" | "end" | "stretch", i: number) => group({
  key: align,
  layout: { type: "rows", gap: 4 },
  children: [
    text(`align: "${align}"`, [0, 0], { key: "title", size: { h: 14 }, style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
    group({ key: "demo", children: [
      shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 3 }), { key: "bg", fill: "$surface", stroke: { paint: "$rule", width: 1, dash: [3, 3] } }),
      group({ key: "row", layout: { type: "columns", gap: 4, padding: 4, align }, children: [14, 26, 38].map((h, j) =>
        group({ key: `c${j}`, size: { h }, children: [shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 2 }), { key: "box", fill: `$categorical[${i}]`, opacity: 0.4 + j * 0.25 })] })) }),
    ] }),
  ],
});

export default doc({
  id: "sdk-group-sizes",
  title: "Sizes and alignment",
  description: "Five children of a columns layout sized 90 px, 22%, auto, fill and fill 2, in a wide box and then a narrow one; below, four rows of fixed-height children aligned start, center, end and stretch.",
  size: [640, 260],
  signals: { narrow: signal.bool(false) },
  scene: group({
    key: "root",
    // `align: "start"`: children with a width of their own keep it across the rows (the rest stretch).
    layout: { type: "rows", gap: 14, padding: [12, 12, 10, 12], align: "start" },
    children: [
      text("size: { w } in a columns layout", [0, 0], { key: "t1", size: { h: 16 }, style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } }),
      group({
        key: "sizes",
        size: { w: e("narrow ? box.w * 0.62 : box.w"), h: 64 },
        layout: { type: "columns", gap: 6 },
        children: [
          box("90", 0, 90),
          box('"22%"', 1, "22%"),
          box('"auto"', 2, "auto", text('"auto"', [32, e("box.h / 2 - 7")], { key: "name", style: { font: "font.strong", size: "$size.label", ink: "on($categorical[2])", align: "middle", baseline: "middle" } })),
          box('"fill"', 3, "fill"),
          box("{ fill: 2 }", 4, { fill: 2 }),
        ],
      }),
      text("layout.align: fixed heights, across", [0, 0], { key: "t2", size: { h: 16 }, style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } }),
      group({ key: "aligns", layout: { type: "columns", gap: 12, wrap: 420 }, children: (["start", "center", "end", "stretch"] as const).map(alignDemo) }),
    ],
  }),
  program: story({ steps: [step("wide", { set: { narrow: false } }), step("narrow", { set: { narrow: true } })] }),
});
