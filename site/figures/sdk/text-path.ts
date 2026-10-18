// Text along a path: names that follow an arc (a gauge's bands, a ring) and a wave. Each glyph
// sits on the path, turned with it; on the lower half the text still reads left to right.
import { doc, e, geom, group, shape, text } from "@datars/sdk";

const R = "min(box.h * 0.36, box.w * 0.2)";
const cx = "box.w * 0.27", cy = "box.h * 0.5";
const deg = (d: number) => (d * Math.PI) / 180;

export default doc({
  id: "sdk-text-path",
  title: "Text along a path",
  description: "Two labels on a ring, one over the top and one under the bottom, both reading left to right, and a sentence along a wave.",
  size: [640, 260],
  scene: group({
    key: "root",
    children: [
      shape(geom.circle({ cx: e(cx), cy: e(cy), r: e(R) }), { key: "ring", stroke: { paint: "$rule", width: 1.5 } }),
      text("Over the top of the ring", [0, 0], { key: "top", path: { arc: [e(cx), e(cy), e(`${R} + 4 + token("size.label") * 0.55`), deg(-84), deg(84)] }, style: { size: "$size.label", weight: 600, ink: "$accent", align: "middle", baseline: "middle" } }),
      text("and round the bottom", [0, 0], { key: "bottom", path: { arc: [e(cx), e(cy), e(`${R} + 4 + token("size.label") * 0.55`), deg(96), deg(264)] }, style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "middle" } }),
      shape(geom.path(e("`M ${box.w * 0.55} ${box.h * 0.55} C ${box.w * 0.68} ${box.h * 0.2}, ${box.w * 0.8} ${box.h * 0.9}, ${box.w * 0.97} ${box.h * 0.45}`")), { key: "wave", stroke: { paint: "$rule", width: 1.5 } }),
      text("A sentence that follows its wave", [0, 0], { key: "wave-text", path: { d: e("`M ${box.w * 0.55} ${box.h * 0.55} C ${box.w * 0.68} ${box.h * 0.2}, ${box.w * 0.8} ${box.h * 0.9}, ${box.w * 0.97} ${box.h * 0.45}`") }, style: { size: e('min(token("size.label"), box.w * 0.026)'), ink: "$ink", align: "middle", baseline: "bottom" } }),
    ],
  }),
});
