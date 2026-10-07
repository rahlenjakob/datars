// The "Maps" family on one table: Europe's population by country, keyed by ISO code, so a
// country's shaded region shrinks into its proportional circle and scatters into its dots. The grid
// cartogram crossfades: std's tileMap keys each tile inside its country's group, not as the
// country (so `datars lint` reports that change as a crossfade). Countries and borders come from the built-in atlas; the
// bundle carries what the camera shows. (Population, millions, approximate for 2024, rounded; for
// illustration.)
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { map, symbols, tileMap, dotDensity } from "@datars/std";

const people: Record<string, number> = {
  ALB: 2.4, AUT: 9.2, BEL: 11.8, BGR: 6.4, BIH: 3.2, BLR: 9.1, CHE: 9.0, CZE: 10.9, DEU: 84.5, DNK: 6.0, ESP: 48.6,
  EST: 1.4, FIN: 5.6, FRA: 68.4, GBR: 68.3, GRC: 10.4, HRV: 3.9, HUN: 9.6, IRL: 5.3, ISL: 0.4, ITA: 59.0, LTU: 2.9, LUX: 0.7,
  LVA: 1.9, MDA: 2.4, MKD: 1.8, MNE: 0.6, NLD: 17.9, NOR: 5.5, POL: 36.6, PRT: 10.6, ROU: 19.0, SRB: 6.6, SVK: 5.4,
  SVN: 2.1, SWE: 10.6, UKR: 37.0,
};
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const europe = { bbox: [-11, 35, 35, 66] };
const values = { data: "pop", key: "iso", value: "millions" };
const label = e("`${d.name}: ${format(d.millions, ',.1f')} million`");

export default doc({
  title: "Europe's population, four maps",
  description: "Thirty-seven European countries' populations as a choropleth, as proportional circles, as a hexagon grid cartogram and as one dot per 500,000 people.",
  size: [640, 400],
  data: { world: data.atlas("countries"), pop: data.values({ iso: Object.keys(people), millions: Object.values(people) }, { key: "iso" }) },
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key", duration: 1.3 },
    { select: { role: "region" }, matcher: "by-key", duration: 1.3 },
  ),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [10, 10, 10, 10] },
    children: [
      map({ source: "world", ...values, fit: europe, legend: true, format: ".1f", label }, at("map")),
      map({ source: "world", fit: europe, children: [symbols({ source: "world", ...values, max: 26, label })] }, at("symbols")),
      map({ source: "world", fit: europe, children: [dotDensity({ source: "world", ...values, per: 0.5, r: 1.3 })] }, at("dots")),
      tileMap({ ...values, layout: "europe", shape: "hex", format: ".1f" }, at("tiles")),
    ],
  }),
  program: story({
    steps: [
      step("map", { title: "map()", text: "Each country shaded by its population, on the theme's sequential ramp." }),
      step("symbols", { title: "symbols()", text: "A circle per country, its area the population." }),
      step("dots", { title: "dotDensity()", text: "A dot per half a million people, scattered inside each country." }),
      step("tiles", { title: "tileMap()", text: "Every country the same size, roughly in its place." }),
    ],
  }),
});
