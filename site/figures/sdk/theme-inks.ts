// Inks: every colour in a scene is an ink string, resolved against the theme when the frame is drawn
// — so this figure follows the page into dark mode. Named tokens, a palette by index, a ramp by
// position, alpha, and `on(…)` (the theme's ink or paper, whichever reads on that colour).
import { doc, e, geom, group, shape, text, type Template } from "@datars/sdk";

/** A chip `i` of `n` across, on line `line` of its row. */
const swatch = (ink: string, i: number, n: number, label = ink, line = 0) => group({
  key: label || ink,
  children: [
    shape(geom.rect({ x: e(`${i} * box.w / ${n} + 1`), y: 18 + line * 44, w: e(`box.w / ${n} - 4`), h: 22, r: 3 }), { key: "chip", fill: ink, stroke: { paint: "$rule", width: 0.5 } }),
    text(label, [e(`${i} * box.w / ${n} + 1`), 44 + line * 44], { key: "label", style: { size: 9, ink: "$ink-2", baseline: "top", maxWidth: e(`box.w / ${n} - 4`) } }),
  ],
});
const row = (title: string, chips: Template[], h = 60) => group({ key: title, size: { h }, children: [
  text(title, [0, 0], { key: "title", style: { font: "font.strong", size: "$size.label", ink: "$ink", baseline: "top" } }),
  ...chips,
] });

const TOKENS = ["$paper", "$ink", "$ink-2", "$muted", "$rule", "$grid", "$surface", "$accent", "$highlight", "$positive", "$negative"];
const ramp = (name: string) => Array.from({ length: 11 }, (_, i) => swatch(`$${name}~${i / 10}`, i, 11, i % 5 === 0 ? `~${i / 10}` : ""));

export default doc({
  id: "sdk-theme-inks",
  title: "Inks",
  description: "Swatches of the theme's named colour tokens, its categorical palette by index, its sequential and diverging ramps by position, and inks with alpha and contrast.",
  size: [640, 372],
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 4, padding: [12, 14, 4, 14] },
    children: [
      row('tokens: "$name"', TOKENS.map((t, i) => swatch(t, i % 6, 6, t, Math.floor(i / 6))), 104),
      row('palette by index: "$categorical[i]"', Array.from({ length: 10 }, (_, i) => swatch(`$categorical[${i}]`, i, 10, `[${i}]`))),
      row('ramp by position: "$sequential~t"', ramp("sequential")),
      row('"$diverging~t"', ramp("diverging")),
      row("alpha, contrast, literal", [
        swatch("$accent@0.4", 0, 4),
        group({ key: "on", children: [
          shape(geom.rect({ x: e("box.w / 4 + 1"), y: 18, w: e("box.w / 4 - 4"), h: 22, r: 3 }), { key: "chip", fill: "$accent" }),
          text("Label", [e("box.w / 4 + 8"), 29], { key: "t", style: { font: "font.strong", size: 10, ink: "on($accent)", baseline: "middle" } }),
          text("text: on($accent)", [e("box.w / 4 + 1"), 44], { key: "label", style: { size: 9, ink: "$ink-2", baseline: "top" } }),
        ] }),
        group({ key: "on2", children: [
          shape(geom.rect({ x: e("box.w / 2 + 1"), y: 18, w: e("box.w / 4 - 4"), h: 22, r: 3 }), { key: "chip", fill: "$highlight" }),
          text("Label", [e("box.w / 2 + 8"), 29], { key: "t", style: { font: "font.strong", size: 10, ink: "on($highlight)", baseline: "middle" } }),
          text("text: on($highlight)", [e("box.w / 2 + 1"), 44], { key: "label", style: { size: 9, ink: "$ink-2", baseline: "top" } }),
        ] }),
        swatch("#e8112d", 3, 4, '"#e8112d" (every mode)'),
      ]),
    ],
  }),
});
