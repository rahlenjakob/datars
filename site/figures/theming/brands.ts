// The theming page's centrepiece: a small dashboard — a key figure on a card, grouped bars, lines
// with points and a donut — drawn only with theme tokens (no literal colour, face, size or corner),
// published once without knowing any brand. The page lays each fictional brand over it at runtime
// with `setTokens()` and `mode`. Illustrative figures.
import { doc, data, e, geom, group, interactive, shape, text } from "@datars/sdk";
import { grouped, kpi, line, pie, plot } from "@datars/std";

const channels = ["Online", "Stores", "Partners"];
const quarter: string[] = [], channel: string[] = [], orders: number[] = [];
["Q1", "Q2", "Q3", "Q4"].forEach((q, qi) => channels.forEach((c, ci) => {
  quarter.push(q);
  channel.push(c);
  orders.push(Math.round(18 + qi * (3 + ci * 2.5) + (2 - ci) * 7 + ((qi + ci) % 2) * 3));
}));

const regions = ["North", "Central", "South"];
const week: number[] = [], region: string[] = [], visits: number[] = [];
regions.forEach((r, ri) => {
  for (let w = 1; w <= 12; w++) {
    week.push(w);
    region.push(r);
    visits.push(Math.round((52 - ri * 13) * (1 + w * (0.03 + ri * 0.014)) + 4 * Math.sin((w + ri * 3) * 0.8)));
  }
});

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const total = [7.9, 8.2, 8.8, 9.1, 9.0, 9.6, 10.1, 10.4, 10.2, 11.0, 11.8, 12.6];

/** The key figure on a card: the card's surface, line and corners are tokens too. */
const card = () => group({ key: "kpi", children: [
  shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: e('token("radius.card")') }), { key: "surface", fill: "$card", stroke: { paint: "$card-line", width: 1 } }),
  group({ key: "body", layout: { type: "stack", padding: [14, 16, 12, 16] }, children: [
    kpi({ label: "Orders this month (k)", data: "monthly", x: "month", y: "orders", format: ",.1f", compareLabel: "vs November" }, { key: "figure" }),
  ] }),
] });
const bars = () => plot({ data: "orders", x: "quarter", y: "orders", color: "channel", title: "Orders by channel (k)", legend: true,
  children: [grouped({ series: "channel" })] }, { key: "bars" });
const lines = () => plot({ data: "visits", x: "week", y: "visits", xType: "linear", color: "region", title: "Weekly visits (k)", zero: false, labelSpace: 64,
  children: [line({ labels: true, points: true })] }, { key: "lines" });
const donut = () => group({ key: "donut", layout: { type: "rows", gap: 6 }, children: [
  text("Share of orders", [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } }),
  pie({ data: "share", value: "share", category: "region", inner: 0.58, format: ".0f" }, { key: "pie" }),
] });

export default doc({
  id: "theming-brands",
  title: "One dashboard, any brand",
  description: "A key figure on a card, orders by channel as grouped bars, weekly visits by region as lines, and each region's share as a donut, drawn only with theme tokens.",
  size: [880, 560],
  data: {
    orders: data.values({ quarter, channel, orders }, { key: ["quarter", "channel"] }),
    visits: data.values({ week, region, visits }, { key: ["region", "week"] }),
    share: data.values({ region: regions, share: [46, 33, 21] }, { key: "region" }),
    monthly: data.values({ month: months.map((_, i) => i + 1), orders: total }, { key: "month" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 18, 12, 12] },
    // Two rows when there's room, one column on a phone — the same keys, so a resize morphs.
    children: [
      group({ key: "panels", when: e("box.w >= 600"), layout: { type: "rows", gap: 22 }, children: [
        group({ key: "top", size: { h: "48%" }, layout: { type: "columns", gap: 26 }, children: [{ ...card(), size: { w: "34%" } }, bars()] }),
        group({ key: "bottom", layout: { type: "columns", gap: 26 }, children: [lines(), { ...donut(), size: { w: "32%" } }] }),
      ] }),
      group({ key: "panels", when: e("box.w < 600"), layout: { type: "rows", gap: 16 }, children: [
        { ...card(), size: { h: "17%" } }, { ...bars(), size: { h: "29%" } }, { ...lines(), size: { h: "29%" } }, donut(),
      ] }),
    ],
  }),
  program: interactive(),
});
