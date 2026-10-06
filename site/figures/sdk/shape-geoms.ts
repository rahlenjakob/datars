// Every `geom.*` kind, one per cell: a `shape` node drawn in its cell's own box (`box.w`, `box.h`).
// Five columns on a wide box, three on a phone (two layouts of the same keys, chosen by `when`).
import { data, doc, e, geom, group, repeat, shape, text, type Geom, type Template } from "@datars/sdk";

const mx = "box.w / 2", my = "box.h / 2 - 8";
const at = (dx: number, dy: number) => [e(`${mx} + ${dx}`), e(`${my} + ${dy}`)] as const;
const label = (name: string) => text(`geom.${name}`, [e("box.w / 2"), e("box.h - 6")], { key: "label", style: { size: "$size.label", ink: "$ink-2", align: "middle", baseline: "bottom" } });
const cell = (name: string, g: Geom, stroked = false) => group({
  key: name,
  children: [
    shape(g, stroked ? { key: "shape", stroke: { paint: "$accent", width: 2.5, cap: "round", join: "round" } } : { key: "shape", fill: "$accent" }),
    label(name),
  ],
});
const wave = (y: string) => e(`${my} + 16 - ${y} * 46`);

const cells = (): Template[] => [
  cell("rect", geom.rect({ x: at(-36, -26)[0], y: at(-36, -26)[1], w: 72, h: 50, r: 6 })),
  cell("circle", geom.circle({ cx: at(0, 0)[0], cy: at(0, 0)[1], r: 27 })),
  cell("ellipse", geom.ellipse({ cx: at(0, 0)[0], cy: at(0, 0)[1], rx: 40, ry: 22 })),
  cell("arc", geom.arc({ cx: at(0, 0)[0], cy: at(0, 0)[1], r0: 15, r1: 31, a0: -2.2, a1: 1.4 })),
  cell("segment", geom.segment({ x1: at(-36, 16)[0], y1: at(-36, 16)[1], x2: at(36, -18)[0], y2: at(36, -18)[1] }), true),
  cell("polyline", geom.polyline({ from: "wave", x: e(`${mx} - 42 + d.i * 14`), y: wave("d.v"), curve: "monotone" }), true),
  cell("area", geom.area({ from: "wave", x: e(`${mx} - 42 + d.i * 14`), y0: e(`${my} + 16`), y1: wave("d.v"), curve: "monotone" })),
  // An SVG path string, built by an expression so it follows the cell's box.
  cell("path", geom.path(e(`"M" + (${mx} - 30) + " " + (${my} + 18) + " C" + (${mx} - 30) + " " + (${my} - 34) + " " + (${mx} + 30) + " " + (${my} - 34) + " " + (${mx} + 30) + " " + (${my} + 18) + " L" + ${mx} + " " + (${my} + 4) + " Z"`))),
  group({
    key: "symbol",
    children: [
      repeat("kinds", shape(geom.symbol({ symbol: e("d.k"), x: e(`${mx} - 34 + (d.i % 3) * 34`), y: e(`${my} - 14 + floor(d.i / 3) * 30`), size: 10 }), { key: e("d.k"), fill: "$accent" })),
      label("symbol"),
    ],
  }),
  group({
    key: "feature",
    children: [
      group({ key: "map", coord: { type: "geo", projection: "equirectangular", fit: { bbox: [-2, -1, 13, 11] }, padding: 26 }, children: [shape(geom.feature("blob", "blob"), { key: "shape", fill: "$accent" })] }),
      label("feature"),
    ],
  }),
];

const grid = (columns: number, when: string) => group({ key: "root", when: e(when), layout: { type: "grid", columns, gap: 6, padding: [10, 8, 6, 8] }, children: cells() });

export default doc({
  id: "sdk-shape-geoms",
  title: "Geometry kinds",
  description: "The ten geom kinds a shape node draws: rect, circle, ellipse, arc, segment, polyline, area, path, symbol and feature.",
  size: [640, 280],
  data: {
    wave: data.values({ i: [0, 1, 2, 3, 4, 5, 6], v: [0.3, 0.75, 0.45, 0.9, 0.35, 0.6, 0.2] }, { key: "i" }),
    kinds: data.values({ k: ["circle", "square", "diamond", "triangle", "cross", "star"], i: [0, 1, 2, 3, 4, 5] }, { key: "k" }),
    // A made-up region in lon/lat, drawn through a geo coordinate system.
    blob: data.geojson({ type: "FeatureCollection", features: [{ type: "Feature", id: "blob", properties: {}, geometry: { type: "Polygon", coordinates: [[[0, 0], [8, 1], [11, 6], [7, 10], [2, 9], [-1, 5], [0, 0]]] } }] }),
  },
  scene: group({ key: "figure", children: [grid(5, 'sizeClass != "phone"'), grid(3, 'sizeClass == "phone"')] }),
});
