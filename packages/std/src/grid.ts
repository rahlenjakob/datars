import { e, group, recipe, repeat, shape, geom, t, Prop } from "@datars/sdk";

export interface GridParams { scale: string; orient: "horizontal" | "vertical"; ticks: number; count: Prop }

export const grid = recipe<GridParams>({
  id: "@datars/std/grid",
  doc: "Gridlines at a scale's ticks across the plot area.",
  params: {
    scale: t.string("y"), orient: t.oneOf(["horizontal", "vertical"] as const, "horizontal"), ticks: t.number(0),
    count: t.prop("A tick count as an expression — the axis's own (a plot passes it), so every gridline meets a tick."),
  },
  tokens: ["grid", "stroke.grid"],
  expand(p) {
    const pos = `d.pos + scale.${p.scale}.bandwidth() / 2`;
    const line = p.orient === "horizontal"
      ? geom.segment({ x1: 0, y1: e(pos), x2: e("box.w"), y2: e(pos) })
      : geom.segment({ x1: e(pos), y1: 0, x2: e(pos), y2: e("box.h") });
    return group({
      key: `grid-${p.scale}`,
      z: -1,
      semantics: { role: "decoration" },
      children: [repeat({ ticks: p.scale, count: p.count ?? (p.ticks || undefined) }, shape(line, { stroke: { paint: "$grid", width: "$stroke.grid" }, semantics: { role: "decoration" } }))],
    });
  },
});
