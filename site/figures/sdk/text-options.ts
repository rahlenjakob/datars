// Text options: font tokens and sizes, wrapping to `maxWidth`, a halo, `rotate`, `offset`, and
// `number` — a value formatted each frame, so it counts when it changes (step through).
import { doc, e, geom, group, repeat, shape, signal, step, story, text, type Template } from "@datars/sdk";

const title = (s: string) => text(s, [0, 0], { key: "title", style: { font: "font.strong", size: "$size.body", ink: "$ink", baseline: "top" } });
const small = (s: string, at: [number | ReturnType<typeof e>, number | ReturnType<typeof e>], key: string, align = "start") =>
  text(s, at, { key, style: { size: "$size.small", ink: "$muted", baseline: "top", align } });
const panel = (key: string, children: Template[]) => group({ key, children: [title(key), ...children] });
const dot = (key: string, x: number | ReturnType<typeof e>, y: number | ReturnType<typeof e>) => shape(geom.circle({ cx: x, cy: y, r: 2.5 }), { key, fill: "$ink" });

const SENTENCE = "Text wraps at word breaks to fit the width it is given.";
const FORMATS = [",.0f", ",.2f", ".3s", "$,.0f", "+,.1f"];

const panels = (): Template[] => [
  panel("style", [
    text("font.body, 13", [0, 28], { key: "body", style: { font: "font.body", size: 13, ink: "$ink", baseline: "top" } }),
    text("font.strong, 13", [0, 50], { key: "strong", style: { font: "font.strong", size: 13, ink: "$ink", baseline: "top" } }),
    text("font.title, 17", [0, 72], { key: "title-font", style: { font: "font.title", size: 17, ink: "$ink", baseline: "top" } }),
    text("ink: \"$accent\", 11", [0, 100], { key: "accent", style: { size: 11, ink: "$accent", baseline: "top" } }),
  ]),
  panel("maxWidth", [
    shape(geom.segment({ x1: e("box.w - 12"), y1: 24, x2: e("box.w - 12"), y2: e("box.h - 4") }), { key: "limit", stroke: { paint: "$muted", width: 1, dash: [2, 2] } }),
    text(SENTENCE, [0, 28], { key: "wide", style: { size: "$size.body", ink: "$ink", baseline: "top", maxWidth: e("box.w - 12") } }),
    shape(geom.segment({ x1: e("(box.w - 12) * 0.55"), y1: 84, x2: e("(box.w - 12) * 0.55"), y2: e("box.h - 4") }), { key: "limit-2", stroke: { paint: "$muted", width: 1, dash: [2, 2] } }),
    text(SENTENCE, [0, 88], { key: "narrow", style: { size: "$size.small", ink: "$ink-2", baseline: "top", maxWidth: e("(box.w - 12) * 0.55") } }),
  ]),
  panel("halo", [
    // A busy background: diagonal hatching.
    group({ key: "hatch", clip: [0, 26, e("box.w"), 80], children: [
      repeat({ count: 30 }, shape(geom.segment({ x1: e("d.index * 12 - 60"), y1: 106, x2: e("d.index * 12 + 20"), y2: 26 }), { key: e("d.index"), stroke: { paint: "$accent", width: 2.5 } })),
    ] }),
    text("no halo", [8, 48], { key: "plain", style: { font: "font.strong", size: 15, ink: "$ink", baseline: "middle" } }),
    text("halo: [\"$paper\", 3]", [8, 84], { key: "haloed", halo: ["$paper", 3], style: { font: "font.strong", size: 15, ink: "$ink", baseline: "middle" } }),
  ]),
  panel("rotate", [
    ...([[0, 0.12], [-30, 0.45], [-90, 0.8]] as const).flatMap(([deg, f], i) => [
      dot(`at-${i}`, e(`box.w * ${f}`), 92),
      text(`${deg}°`, [e(`box.w * ${f}`), 92], { key: `r-${i}`, rotate: deg, style: { size: "$size.body", ink: "$accent", baseline: "bottom" } }),
    ]),
    small("rotate (degrees) about at", [0, 104], "note"),
  ]),
  panel("number", [
    ...FORMATS.map((f, i) => group({ key: `f-${i}`, children: [
      small(`"${f}"`, [0, 26 + i * 17], "spec"),
      text("", [e("box.w - 8"), 25 + i * 17], { key: "value", number: { value: e("v"), format: f }, style: { size: "$size.body", ink: "$ink", baseline: "top", align: "end" } }),
    ] })),
  ]),
  panel("offset", [
    dot("at", 24, 70),
    text("offset: [8, -6]", [24, 70], { key: "label", offset: [8, -6], style: { size: "$size.body", ink: "$accent", baseline: "bottom" } }),
    small("screen px from at: stays put", [0, 88], "n1"),
    small("at any camera zoom", [0, 101], "n2"),
  ]),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 22, padding: [12, 14, 6, 12] }, children: panels() });

export default doc({
  id: "sdk-text-options",
  title: "Text options",
  description: "Fonts and sizes, wrapping to a maximum width, a halo over busy marks, rotated labels, formatted numbers that count when they change, and a screen offset from a point.",
  size: [640, 300],
  signals: { v: signal.num(1234.5) },
  scene: group({ key: "figure", children: [grid(3, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
  program: story({ steps: [step("1234.5", { set: { v: 1234.5 } }), step("98765.4", { set: { v: 98765.4 } }), step("-42", { set: { v: -42 } })] }),
});
