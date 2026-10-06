// Choreographies: how one transition's duration is spread over its elements. In each panel a grid of
// squares grows and turns colour when you step; the panel's rule picks `choreo.together()`,
// `stagger`, `wave` or `ripple` (`phased` splits exits, moves and entries instead).
import { doc, e, geom, group, motion, repeat, shape, signal, step, story, text, choreo, type Choreo, type Rule } from "@datars/sdk";

const CHOREOS: [string, string, Choreo][] = [
  ["together", "together()", choreo.together()],
  ["stagger", 'stagger("left", 0.7)', choreo.stagger("left", 0.7)],
  ["wave", "wave(0.7, 0.8)", choreo.wave(0.7, 0.8)],
  ["ripple", "ripple(0.7)", choreo.ripple(0.7)],
];
const COLS = 8, ROWS = 5;

const panel = ([key, label]: [string, string, Choreo]) => group({
  key,
  children: [
    text(`choreo.${label}`, [0, 0], { key: "label", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } }),
    repeat({ count: COLS * ROWS }, shape(geom.rect({
      x: e(`(d.index % ${COLS} + 0.5) * box.w / ${COLS} - (on ? 0.42 : 0.24) * box.w / ${COLS}`),
      y: e(`20 + (floor(d.index / ${COLS}) + 0.5) * (box.h - 20) / ${ROWS} - (on ? 0.42 : 0.24) * box.w / ${COLS}`),
      w: e(`(on ? 0.84 : 0.48) * box.w / ${COLS}`), h: e(`(on ? 0.84 : 0.48) * box.w / ${COLS}`), r: 2,
    }), { key: e("d.index"), fill: e('on ? "$accent" : "$muted"') })),
  ],
});

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 16, padding: [12, 14, 8, 14] }, children: CHOREOS.map(panel) });

export default doc({
  id: "sdk-motion-choreos",
  title: "Choreographies",
  description: "Four grids of forty squares growing and turning blue: all together, one after another from the left, in a diagonal wave, and in a ripple from the centre of the scene.",
  size: [640, 220],
  signals: { on: signal.bool(false) },
  scene: group({ key: "figure", children: [grid(4, 'sizeClass != "phone"'), grid(2, 'sizeClass == "phone"')] }),
  motion: motion(
    { duration: 1.6, easing: "cubic-in-out" },
    ...CHOREOS.map(([key, , c]): Rule => ({ select: { key: `figure/root/${key}` }, choreo: c })),
  ),
  program: story({ steps: [step("small", { set: { on: false } }), step("large", { set: { on: true } })] }),
});
