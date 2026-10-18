// The iconography page's pictures: five fictional Fernmouth coffee roasters ranked by cups sold, each
// beside its logo — a PNG, drawn as it is (a logo keeps its own colours; an icon would take the
// theme's) and read by screen readers through its `alt`. The build ships each file in the bundle.
import { data, doc, e, geom, group, image, repeat, shape, text } from "@datars/sdk";

const shops = [
  { id: "tidewater", name: "Tidewater Roasters", cups: 4820 },
  { id: "northlight", name: "Northlight", cups: 4110 },
  { id: "ember-oak", name: "Ember & Oak", cups: 3570 },
  { id: "little-bean", name: "Little Bean Co.", cups: 2960 },
  { id: "cliffside", name: "Cliffside Coffee", cups: 2240 },
];
const n = shops.length;
const rowH = `((box.h - 8) / ${n})`;
const y = `(4 + ${rowH} * d.rank + ${rowH} / 2)`;
const logo = `min(40, ${rowH} * 0.8)`;
const labelW = "min(150, box.w * 0.3)";
const x0 = `(${logo} + 14 + ${labelW})`;
const barW = `((box.w - ${x0} - token("size.label") * 3.4 - 12) * d.cups / ${shops[0].cups})`;

export default doc({
  id: "iconography-roasters",
  title: "Fernmouth's coffee roasters",
  description: "Five fictional coffee roasters by cups sold in a week: Tidewater Roasters 4,820, Northlight 4,110, Ember & Oak 3,570, Little Bean Co. 2,960 and Cliffside Coffee 2,240, each with its logo.",
  size: [640, 250],
  data: { shops: data.values({ id: shops.map((s) => s.id), name: shops.map((s) => s.name), cups: shops.map((s) => s.cups), rank: shops.map((_, i) => i), logo: shops.map((s) => `assets/${s.id}.png`) }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [8, 12, 8, 8] },
    children: [
      repeat("shops", group({
        key: e("d.id"),
        semantics: { role: "datum", label: e('d.name + ": " + format(d.cups, ",.0f") + " cups a week"'), value: e("d.cups") },
        children: [
          image(e("d.logo"), [2, e(`${y} - ${logo} / 2`), e(logo), e(logo)], { key: "logo", alt: e('d.name + " logo"') }),
          text(e("d.name"), [e(`${logo} + 12`), e(y)], { key: "name", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle", maxWidth: e(labelW) } }),
          shape(geom.rect({ x: e(x0), y: e(`${y} - ${rowH} * 0.2`), w: e(barW), h: e(`${rowH} * 0.4`), r: 4 }), { key: "bar", fill: "$accent", semantics: { role: "decoration" } }),
          text(e('format(d.cups, ",.0f")'), [e(`${x0} + ${barW} + 8`), e(y)], { key: "cups", style: { size: "$size.label", ink: "$ink-2", baseline: "middle" } }),
        ],
      })),
    ],
  }),
});
