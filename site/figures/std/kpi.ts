// kpi: a key figure — the number, its change against a comparison and, from a table, a sparkline
// of how it got there. A fictional online tea shop's year by month: revenue, orders and the
// returns rate (for returns, down is good). `value` and `compare` take numbers or expressions too.
import { doc, data, e, geom, group, shape, story, step, Template } from "@datars/sdk";
import { kpi } from "@datars/std";

const months = ["2025-10", "2025-11", "2025-12", "2026-01", "2026-02", "2026-03", "2026-04", "2026-05", "2026-06", "2026-07", "2026-08", "2026-09"].map((m) => `${m}-01`);
const revenue = [38200, 41900, 55400, 36100, 37800, 40300, 42600, 44100, 43300, 45900, 47200, 51400];
const orders = [1010, 1105, 1460, 952, 990, 1046, 1098, 1132, 1117, 1164, 1190, 1284];
const returns = [0.052, 0.049, 0.061, 0.058, 0.055, 0.051, 0.05, 0.047, 0.049, 0.046, 0.048, 0.041];
// A dashboard tile: the card surface, the figure inside it.
const tile = (key: string, inside: Template) => group({ key, children: [
  shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 8 }), { key: "card", fill: "$card", stroke: { paint: "$card-line", width: 1 } }),
  group({ key: "inside", layout: { type: "stack", padding: 14 }, children: [inside] }),
] });

// Three tiles side by side (stacked on a phone), one set per step: keyed alike, they morph.
const tiles = (state: string, o: Record<string, unknown> = {}) => group({
  key: "tiles", when: e(`state == "${state}"`), layout: { type: "columns", gap: 12, wrap: 420 },
  children: [
    tile("revenue", kpi({ label: "Revenue", data: "shop", x: "month", y: "revenue", format: "$,.0f", compareLabel: "vs August", ...o })),
    tile("orders", kpi({ label: "Orders", data: "shop", x: "month", y: "orders", compareLabel: "vs August", ...o })),
    tile("returns", kpi({ label: "Returns", data: "shop", x: "month", y: "returns", format: ".1%", better: "down", compareLabel: "vs August", ...o, deltaFormat: o.delta === "absolute" ? ".1%" : undefined })),
  ],
});

export default doc({
  title: "A tea shop's September in three numbers",
  description: "Three key figures — revenue, orders and the returns rate, where a fall is good — each with its change on August and a year's sparkline; then the changes in the figures' own units; then centred.",
  size: [640, 220],
  data: { shop: data.values({ month: months, revenue, orders, returns }, { key: "month", types: { month: "date" } }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [tiles("default"), tiles("absolute", { delta: "absolute" }), tiles("center", { delta: "absolute", align: "center" })],
  }),
  program: story({
    steps: [
      step("default", { title: "kpi({ label, data, x, y })", text: "The last month against the one before, and a year's sparkline. Returns fell: with better: \"down\" that's good, so green." }),
      step("absolute", { title: "delta: \"absolute\"", text: "The change in the figure's own units: +$4,200, +94 orders." }),
      step("center", { title: "align: \"center\"", text: "Centred in the tile." }),
    ],
  }),
});
