// Theme studio: a world choropleth — the sequential ramp, water, land, borders and the colour of
// countries with no data (most of them, on purpose). Illustrative figures.
import { doc, data, group } from "@datars/sdk";
import { map, title } from "@datars/std";

// Readers by country (k).
const COUNTRIES = "USA 820 CAN 140 MEX 95 BRA 260 ARG 70 CHL 40 COL 55 GBR 310 FRA 220 DEU 290 ESP 150 ITA 130 NLD 90 SWE 120 NOR 60 " +
  "FIN 45 DNK 50 POL 80 UKR 35 TUR 60 EGY 30 NGA 45 KEN 25 ZAF 55 IND 380 CHN 150 JPN 240 KOR 130 IDN 90 PHL 50 VNM 40 AUS 160 NZL 30";
const cf = COUNTRIES.split(" ");
const id = cf.filter((_, i) => i % 2 === 0), readers = cf.filter((_, i) => i % 2 === 1).map(Number);

export default doc({
  title: "Readers by country",
  description: "Readers in thousands for 34 countries on a world map, shaded on the sequential ramp; the other countries have no data.",
  size: [960, 440],
  data: { world: data.atlas("countries"), countries: data.values({ id, readers }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [14, 16, 10, 12] },
    children: [
      title({ text: "Readers by country (k)" }, { size: { h: "auto" } }),
      map({ source: "world", data: "countries", key: "id", value: "readers", projection: "equal-earth", sea: true, legend: true, format: ",.0f", fit: { bbox: [-170, -56, 180, 78] } }, { key: "map" }),
    ],
  }),
});
