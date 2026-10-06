// 4,000,000 stars: a synthetic barred spiral generated from a seed (galaxy.ts), drawn with level
// of detail. The engine indexes the rows once into a pyramid of tiles and each frame draws only
// what the camera shows — a sample that keeps the galaxy's density at every zoom, every single star
// once you're close. Drag to pan, scroll or pinch to zoom, hover a star; the tour flies from the
// whole galaxy into a spiral arm, the bar, a globular cluster and one small neighbourhood.
import { doc, e, group, motion, shape, geom, signal, step, story, text } from "@datars/sdk";
import { cloud } from "@datars/std";
import { armPoint, clusterCentre, stars } from "./galaxy";

const STARS = 4_000_000;

/** A camera target: a square `size` light-years across around (x, y). */
const around = ([x, y]: [number, number], size: number) => ({ x0: x - size / 2, y0: y - size / 2, x1: x + size / 2, y1: y + size / 2 });
const WHOLE = { x0: -40_000, y0: -40_000, x1: 40_000, y1: 40_000 };
const ARM = around(armPoint(0, 27_000), 9_000);
const BAR = around([0, 0], 17_000);
const CLUSTER = around(clusterCentre(45), 600);
const LOCAL = around(armPoint(1, 26_500).map((v) => v + 900) as [number, number], 900);

// Temperature → colour and spectral class (hot blue O stars to cool red M dwarfs).
const CLASSES: [string, number, string][] = [["M", 3700, "#ffb06e"], ["K", 5200, "#ffd29c"], ["G", 6000, "#fff0d6"], ["F", 7500, "#f7f5ff"], ["A", 10000, "#d8e2ff"], ["B", 30000, "#aec2ff"], ["O", Infinity, "#98afff"]];
const byTemp = (pick: (c: [string, number, string]) => string) =>
  CLASSES.slice(0, -1).reduceRight((rest, c) => `d.temp < ${c[1]} ? ${JSON.stringify(pick(c))} : ${rest}`, JSON.stringify(pick(CLASSES[CLASSES.length - 1])));

// The tooltip: the star's catalogue number, class, temperature, luminosity and where it is.
const LABEL = "`Star ${format(d.$row + 1, \",\")} · ${" + byTemp((c) => c[0]) + "} · ${format(d.temp, \",\")} K · "
  + "${format(pow(10, d.lum), \",.2~r\")} suns · ${format(sqrt(d.x * d.x + d.y * d.y), \",.0f\")} ly from the centre`";

// The scale bar: a round length of about 110 px at the current zoom (the fit's zoom × the reader's).
const PAD = 12;
const fit = `min((box.w - ${2 * PAD}) / (bx1 - bx0), (box.h - ${2 * PAD}) / (by1 - by0)) * (sky.zoom ?? 1)`;
const raw = `(110 / (${fit}))`;
const nice = `pow(10, floor(log10(${raw})))`;
const length = `(${raw} / ${nice} < 2 ? 1 : ${raw} / ${nice} < 5 ? 2 : 5) * ${nice}`;

export default doc({
  id: "galaxy",
  title: "4,000,000 stars",
  description: "A synthetic barred spiral galaxy of four million stars, generated from a seed and drawn with level of detail: pan and zoom from the whole galaxy down to single stars.",
  size: [1000, 680],
  theme: { use: "datars/neutral", tokens: { paper: "#03050c", ink: "#eef1ff", "ink-2": "#c3cbe8", muted: "#8a93b5", rule: "#2a3150" } },
  data: { stars: stars(STARS) },
  signals: {
    bx0: signal.num(WHOLE.x0), by0: signal.num(WHOLE.y0), bx1: signal.num(WHOLE.x1), by1: signal.num(WHOLE.y1),
  },
  // Flights take their time; the scale bar follows the camera.
  motion: motion({ select: { kind: "view" }, duration: 2.6, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    children: [
      cloud({
        data: "stars", x: "x", y: "y",
        bbox: [e("bx0"), e("by0"), e("bx1"), e("by1")],
        padding: PAD, explore: "sky", maxZoom: 3000,
        // Up to 80,000 stars a frame, over a glow of 30,000 of them drawn large and faint — the
        // luminous ones strongest, as they dominate a galaxy's light.
        points: 80_000, glow: 6, glowOpacity: e("clamp(0.03 + 0.03 * d.lum, 0.015, 0.2)"), glowPoints: 30_000,
        // Brighter stars are bigger; the faintest dwarfs recede.
        r: e("clamp(0.95 + 0.34 * d.lum, 0.75, 3.6)"),
        opacity: e("clamp(0.62 + 0.16 * d.lum, 0.22, 1)"),
        fill: e(byTemp((c) => c[2])),
        label: e(LABEL),
        name: "4,000,000 stars",
      }, { key: "sky" }),
      text("4,000,000 stars", [22, 20], { key: "title", style: { font: "font.title", size: 22, ink: "$ink", baseline: "top" }, halo: ["$paper", 3] }),
      text("A barred spiral, generated from a seed", [22, 50], { key: "subtitle", style: { size: 12.5, ink: "$ink-2", baseline: "top" }, halo: ["$paper", 3] }),
      group({
        key: "scale",
        children: [
          shape(geom.rect({ x: 22, y: e("box.h - 26"), w: e(`${length} * (${fit})`), h: 2 }), { key: "bar", fill: "$ink-2" }),
          text(e(`format(${length}, ",") + " light-years"`), [22, e("box.h - 32")], { key: "label", style: { size: 11.5, ink: "$ink-2", baseline: "bottom" }, halo: ["$paper", 3] }),
        ],
      }),
      group({
        key: "legend",
        children: CLASSES.map(([cls, , ink], i) =>
          text(cls, [e(`box.w - ${22 + (CLASSES.length - 1 - i) * 17}`), e("box.h - 24")], { key: cls, style: { size: 12, weight: 600, ink, align: "end", baseline: "middle" }, halo: ["$paper", 3] }),
        ),
      }),
    ],
  }),
  program: story({
    steps: [
      step("galaxy", { set: bounds(WHOLE), title: "Four million stars", text: "Every dot is one star. At this zoom the engine draws a sample of them that keeps the galaxy's density; zoom in and the rest arrive." }),
      step("arm", { set: bounds(ARM), title: "A spiral arm", text: "Young, hot, blue stars trace the arms; the disk between them is older and redder." }),
      step("bar", { set: bounds(BAR), title: "The bar", text: "An elongated cloud of old stars across the centre, the densest part of the galaxy." }),
      step("cluster", { set: bounds(CLUSTER), title: "A globular cluster", text: "Five hundred old stars in a ball a couple of hundred light-years across, one of 160 in the halo. Every star is drawn." }),
      step("local", { set: bounds(LOCAL), title: "One neighbourhood", text: "Nine hundred light-years of a spiral arm, star by star. Hover one." }),
    ],
  }),
});

/** A camera target as the signals the view's fit reads. */
function bounds(b: { x0: number; y0: number; x1: number; y1: number }) {
  return { bx0: Math.round(b.x0), by0: Math.round(b.y0), bx1: Math.round(b.x1), by1: Math.round(b.y1) };
}
