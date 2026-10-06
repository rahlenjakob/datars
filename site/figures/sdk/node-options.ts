// Options every node takes: `opacity` (and `isolate`), `clip`, `trim`, `transform` and `z`. Dashed
// outlines show what the option changed.
import { data, doc, e, geom, group, shape, text, type Template } from "@datars/sdk";

const W = "box.w", H = "box.h";
const title = (s: string) => text(s, [0, 0], { key: "title", style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } });
const note = (s: string) => text(s, [0, e(`${H} - 2`)], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "bottom" } });
const panel = (key: string, t: string, n: string, children: Template[]) => group({ key, children: [title(t), ...children, note(n)] });
const dashed = { paint: "$muted", width: 1, dash: [3, 3] };
const pair = [
  shape(geom.circle({ cx: e(`${W} / 2 - 16`), cy: e(`${H} / 2 + 2`), r: 26 }), { key: "a", fill: "$categorical[0]" }),
  shape(geom.circle({ cx: e(`${W} / 2 + 16`), cy: e(`${H} / 2 + 2`), r: 26 }), { key: "b", fill: "$categorical[0]" }),
];

const panels = (): Template[] => [
  panel("opacity", "opacity: 0.5", "each child fades: overlaps show", [group({ key: "g", opacity: 0.5, children: pair })]),
  panel("isolate", "opacity: 0.5, isolate", "the group fades as one layer", [group({ key: "g", opacity: 0.5, isolate: true, children: pair })]),
  panel("clip", "clip: [x, y, w, h]", "drawn only inside the rectangle", [
    // The inner group's box, and the circle it cuts.
    group({ key: "frame", children: [shape(geom.rect({ x: e(`${W} / 2 - 34`), y: e(`${H} / 2 - 26`), w: 68, h: 56 }), { key: "box", stroke: dashed })] }),
    shape(geom.circle({ cx: e(`${W} / 2 + 20`), cy: e(`${H} / 2 + 2`), r: 34 }), { key: "ghost", stroke: dashed }),
    group({ key: "g", clip: [e(`${W} / 2 - 34`), e(`${H} / 2 - 26`), 68, 56], children: [shape(geom.circle({ cx: e(`${W} / 2 + 20`), cy: e(`${H} / 2 + 2`), r: 34 }), { key: "c", fill: "$categorical[0]" })] }),
  ]),
  panel("trim", "trim: [0, 0.6]", "the first 60% of the line", [
    shape(geom.polyline({ from: "zig", x: e(`${W} / 2 - 50 + d.i * 25`), y: e(`${H} / 2 + 16 - d.v * 34`), curve: "monotone" }), { key: "ghost", stroke: dashed }),
    shape(geom.polyline({ from: "zig", x: e(`${W} / 2 - 50 + d.i * 25`), y: e(`${H} / 2 + 16 - d.v * 34`), curve: "monotone" }), { key: "line", trim: [0, 0.6], stroke: { paint: "$categorical[0]", width: 3, cap: "round" } }),
  ]),
  panel("transform", "transform", "scale: 1.3, rotate: 25 (degrees)", [
    shape(geom.rect({ x: -22, y: -16, w: 44, h: 32 }), { key: "ghost", transform: { translate: [e(`${W} / 2`), e(`${H} / 2 + 2`)] }, stroke: dashed }),
    shape(geom.rect({ x: -22, y: -16, w: 44, h: 32, r: 3 }), { key: "r", fill: "$categorical[0]", opacity: 0.85, transform: { scale: 1.3, rotate: 25, translate: [e(`${W} / 2`), e(`${H} / 2 + 2`)] } }),
  ]),
  panel("z", "z", "listed first, but z: 2 is on top", ([[2, -24], [1, 0], [0, 24]] as const).map(([z, dx], i) => group({ key: `s${i}`, z, children: [
    shape(geom.rect({ x: e(`${W} / 2 + ${dx} - 22`), y: e(`${H} / 2 + ${dx / 2} - 20`), w: 44, h: 40, r: 4 }), { key: "r", fill: `$categorical[${i}]`, stroke: { paint: "$paper", width: 2 } }),
    text(`z: ${z}`, [e(`${W} / 2 + ${dx} + 17`), e(`${H} / 2 + ${dx / 2} + 15`)], { key: "t", style: { font: "font.strong", size: "$size.small", ink: `on($categorical[${i}])`, align: "end", baseline: "bottom" } }),
  ] }))),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 18, padding: [12, 14, 8, 12] }, children: panels() });

export default doc({
  id: "sdk-node-options",
  title: "Node options",
  description: "Six panels: a faded group without and with isolate, a clipped circle, a line trimmed to 60 percent, a scaled and rotated rectangle, and three squares ordered by z.",
  size: [640, 300],
  data: { zig: data.values({ i: [0, 1, 2, 3, 4], v: [0.1, 0.9, 0.3, 1, 0.5] }, { key: "i" }) },
  scene: group({ key: "figure", children: [grid(3, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
});
