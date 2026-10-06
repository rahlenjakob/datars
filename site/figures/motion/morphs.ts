// Path morphs: when a mark changes kind — a star (a path) into a bar (a rectangle) — its outline has
// to become another. Three panels, three strategies: `disc` travels through an area-matched disc and
// never folds (the default), `resample` lerps equal-length outlines point by point, `crossfade` swaps
// opacity. Same key in both steps, so each panel's mark is one element morphing.
import { doc, e, geom, group, motion, shape, step, story, text, type Rule } from "@datars/sdk";

const MORPHS = ["disc", "resample", "crossfade"] as const;

// A five-pointed star round (0, 0), outer radius 48, inner 20, point up.
const star = Array.from({ length: 10 }, (_, k) => {
  const r = k % 2 ? 20 : 48, a = (k * Math.PI) / 5 - Math.PI / 2;
  return `${k ? "L" : "M"}${(r * Math.cos(a)).toFixed(2)},${(r * Math.sin(a)).toFixed(2)}`;
}).join(" ") + " Z";

const panel = (name: string) => group({
  key: name,
  children: [
    shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 6 }), { key: "bg", fill: "$surface" }),
    text(`"${name}"`, [8, 8], { key: "label", style: { font: "font.strong", size: "$size.small", ink: "$ink", baseline: "top" } }),
    group({
      key: "mark",
      // Sized to the panel: three across, even in a phone's width.
      transform: { translate: [e("box.w / 2"), e("box.h / 2 + 9")], scale: e("min(1.3, min(box.w, box.h - 26) / 112)") },
      children: [
        shape(geom.path(star), { key: "shape", when: e('state == "star"'), fill: "$accent", semantics: { role: "datum", label: "A star" } }),
        shape(geom.rect({ x: -18, y: -52, w: 36, h: 104, r: 4 }), { key: "shape", when: e('state == "bar"'), fill: "$categorical[2]", semantics: { role: "datum", label: "A bar" } }),
      ],
    }),
  ],
});

export default doc({
  id: "motion-morphs",
  title: "Path morphs",
  description: "Three panels in which a star becomes a bar: through a disc, point by point, and by a crossfade.",
  size: [640, 220],
  scene: group({ key: "figure", children: [group({ key: "root", layout: { type: "grid", columns: 3, gap: 8, padding: [8, 8, 8, 8] }, children: MORPHS.map(panel) })] }),
  motion: motion(
    { duration: 1.6, easing: "cubic-in-out" },
    ...MORPHS.map((m): Rule => ({ select: { key: `figure/root/${m}` }, morph: m })),
  ),
  program: story({ steps: [step("star"), step("bar")] }),
});
