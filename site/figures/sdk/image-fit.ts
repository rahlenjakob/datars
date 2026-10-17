// `image()`: a picture (PNG) drawn into a rect four ways. The dashed frame is the rect; `cover`
// and `none` are cut to it. Pictures without `alt` are decorative.
import { doc, e, geom, group, image, shape, text } from "@datars/sdk";

const fits = ["contain", "cover", "fill", "none"] as const;

export default doc({
  id: "sdk-image-fit",
  title: "One picture, four fits",
  description: "A small landscape picture drawn into the same wide rect four ways: contained, covering it, stretched to fill it, and at its own size.",
  size: [640, 220],
  scene: group({
    key: "root",
    layout: { type: "columns", gap: 10, padding: [8, 8, 8, 8], wrap: 420 },
    children: fits.map((fit) => group({
      key: fit,
      children: [
        image("assets/valley.png", [4, 4, e("box.w - 8"), e("box.h - 34")], { key: "picture", fit, alt: fit === "contain" ? "A valley with a lake under the sun" : undefined }),
        shape(geom.rect({ x: 4, y: 4, w: e("box.w - 8"), h: e("box.h - 34") }), { key: "frame", stroke: { paint: "$rule", width: 1, dash: [3, 3] } }),
        text(`fit: "${fit}"`, [e("box.w / 2"), e("box.h - 10")], { key: "caption", style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "bottom" } }),
      ],
    })),
  }),
});
