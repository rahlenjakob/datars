// The showcase's theming demo: every colour, font, size, stroke and radius here is a theme token (no
// literal colours), so a host restyles the published chart at runtime — `setTokens({...})` for a
// brand, `mode="dark"` for the platform — without republishing: bars, lines, a world map, a card.
// The brands' faces travel with the bundle as document fonts, so a brand can switch the type too.
// Illustrative figures.
import { doc, data, e, geom, group, interactive, shape, text } from "@datars/sdk";
import { plot, grouped, line, map } from "@datars/std";

const products = ["Maps", "Stories", "Dashboards"];
const quarter: string[] = [], product: string[] = [], revenue: number[] = [];
["Q1", "Q2", "Q3", "Q4"].forEach((q, qi) => products.forEach((p, pi) => {
  quarter.push(q);
  product.push(p);
  revenue.push(Math.round(24 + qi * (4 + pi * 3) + pi * 6 + ((qi + pi) % 2) * 5));
}));

const platform = ["Web", "iOS", "Android", "Desktop"];
const month: number[] = [], device: string[] = [], readers: number[] = [];
platform.forEach((d, di) => {
  for (let m = 1; m <= 12; m++) {
    month.push(m);
    device.push(d);
    readers.push(Math.round((70 - di * 14) * (1 + m * (0.035 + di * 0.012)) + 5 * Math.sin((m + di * 2) * 0.9)));
  }
});

// Readers by country (k), for the map.
const COUNTRIES = "USA 820 CAN 140 MEX 95 BRA 260 ARG 70 CHL 40 COL 55 GBR 310 FRA 220 DEU 290 ESP 150 ITA 130 NLD 90 SWE 120 NOR 60 " +
  "FIN 45 DNK 50 POL 80 UKR 35 TUR 60 EGY 30 NGA 45 KEN 25 ZAF 55 IND 380 CHN 150 JPN 240 KOR 130 IDN 90 PHL 50 VNM 40 AUS 160 NZL 30";
const cf = COUNTRIES.split(" ");
const country = cf.filter((_, i) => i % 2 === 0), count = cf.filter((_, i) => i % 2 === 1).map(Number);

const bars = () => plot({ data: "revenue", x: "quarter", y: "revenue", color: "product", title: "Revenue by product (M)", legend: true,
  children: [grouped({ series: "product" })] }, { key: "bars" });
const lines = () => plot({ data: "readers", x: "month", y: "readers", xType: "linear", color: "device", title: "Monthly readers (k)", zero: false, labelSpace: 70,
  children: [line({ labels: true, points: true })] }, { key: "lines" });
const world = () => group({ key: "map", layout: { type: "rows", gap: 6 }, children: [
  text("Readers by country (k)", [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } }),
  map({ source: "world", data: "countries", key: "id", value: "readers", projection: "equal-earth", sea: true, format: ",.0f", fit: { bbox: [-170, -56, 180, 78] } }, { key: "m" }),
] });
// A card in the theme's card style: its surface, line, corner radius and type. Stacked as rows, so
// a brand's larger or wider type pushes the stats down instead of running into them.
const kpi = () => group({ key: "kpi", children: [
  shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: e('token("radius.card")') }), { key: "surface", fill: "$card", stroke: { paint: "$card-line", width: 1 } }),
  group({ key: "body", layout: { type: "rows", gap: 6, padding: [16, 18, 14, 18] }, children: [
    text("Readers this year", [0, 0], { key: "label", size: { h: "auto" }, style: { font: "font.strong", size: "$size.small", ink: "$card-ink-2", baseline: "top" } }),
    text("4.2M", [0, 0], { key: "value", size: { h: "auto" }, style: { font: "font.title", size: 38, ink: "$accent", baseline: "top" } }),
    text("Up 38% on last year.", [0, 0], { key: "note", size: { h: "auto" }, style: { font: "font.body", size: "$size.body", ink: "$card-ink", baseline: "top", maxWidth: e("box.w") } }),
    group({ key: "stats", size: { h: 58 }, children: [
      shape(geom.segment({ x1: 0, y1: 8, x2: e("box.w"), y2: 8 }), { key: "rule", stroke: { paint: "$card-line", width: 1 } }),
      ...([["34", "countries", 0], ["820k", "top market: US", e("box.w / 2 + 4")]] as const).map(([v, l, x], i) => group({ key: `stat-${i}`, children: [
        text(v, [x, 20], { key: "v", style: { font: "font.title", size: 20, ink: "$card-ink", baseline: "top" } }),
        text(l, [x, 44], { key: "l", style: { font: "font.body", size: "$size.small", ink: "$card-ink-2", baseline: "top" } }),
      ] })),
    ] }),
  ] }),
] });

export default doc({
  id: "brand",
  title: "One chart, any brand",
  description: "Quarterly revenue by product as grouped bars, and monthly readers by platform as lines, styled entirely by theme tokens.",
  size: [880, 600],
  data: {
    revenue: data.values({ quarter, product, revenue }, { key: ["quarter", "product"] }),
    readers: data.values({ month, device, readers }, { key: ["device", "month"] }),
    countries: data.values({ id: country, readers: count }, { key: "id" }),
    world: data.atlas("countries"),
    // The brands' faces (site.js BRANDS), shipped with the chart so a host can switch to them.
    newsreader: data.font("../../assets/fonts/Newsreader-Regular.ttf"),
    newsreaderSemi: data.font("../../assets/fonts/Newsreader-SemiBold.ttf"),
    manrope: data.font("google:Manrope:400"),
    manropeBold: data.font("google:Manrope:700"),
    mono: data.font("google:Space Mono:400"),
    monoBold: data.font("google:Space Mono:700"),
    fraunces: data.font("google:Fraunces:700"),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    // Two rows of two when there's room, one column on a phone — same keys, so a resize morphs
    // between them.
    children: [
      group({ key: "panels", when: e("box.w >= 600"), layout: { type: "rows", gap: 24 }, children: [
        group({ key: "top", size: { h: "52%" }, layout: { type: "columns", gap: 28 }, children: [{ ...bars(), size: { w: "46%" } }, lines()] }),
        group({ key: "bottom", layout: { type: "columns", gap: 28 }, children: [{ ...world(), size: { w: "64%" } }, kpi()] }),
      ] }),
      group({ key: "panels", when: e("box.w < 600"), layout: { type: "rows", gap: 18 }, children: [
        { ...bars(), size: { h: "27%" } }, { ...lines(), size: { h: "27%" } }, { ...world(), size: { h: "28%" } }, kpi(),
      ] }),
    ],
  }),
  program: interactive(),
});
