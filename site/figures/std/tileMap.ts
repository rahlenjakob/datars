// tileMap: a grid cartogram — every state or country the same size, roughly in its place, coloured
// by value and marked with its code. Unemployment by US state (approximate annual averages for
// 2023, rounded; for illustration), then Europe's population (approximate, 2024, millions; for
// illustration), as squares and as hexagons — each built-in layout has a hexagon grid of its own.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { tileMap } from "@datars/std";

const rate: Record<string, number> = {
  AL: 2.5, AK: 4.2, AZ: 3.9, AR: 3.3, CA: 4.8, CO: 3.2, CT: 4.0, DE: 4.0, DC: 5.0, FL: 2.9, GA: 3.3, HI: 3.0, ID: 3.2,
  IL: 4.5, IN: 3.3, IA: 2.9, KS: 2.7, KY: 4.2, LA: 3.6, ME: 3.0, MD: 1.9, MA: 3.4, MI: 3.9, MN: 2.8, MS: 3.3, MO: 3.0,
  MT: 2.9, NE: 2.3, NV: 5.4, NH: 2.4, NJ: 4.4, NM: 3.9, NY: 4.2, NC: 3.4, ND: 2.0, OH: 3.5, OK: 3.1, OR: 4.0, PA: 3.4,
  RI: 3.1, SC: 3.0, SD: 1.9, TN: 3.3, TX: 4.0, UT: 2.7, VT: 1.9, VA: 2.9, WA: 4.1, WV: 3.7, WI: 3.0, WY: 3.2,
};
// Population, millions (approximate, 2024), by ISO 3166 alpha-3 code.
const people: Record<string, number> = {
  ALB: 2.4, AUT: 9.2, BEL: 11.8, BGR: 6.4, BIH: 3.2, BLR: 9.1, CHE: 9.0, CYP: 0.9, CZE: 10.9, DEU: 84.5, DNK: 6.0, ESP: 48.6,
  EST: 1.4, FIN: 5.6, FRA: 68.4, GBR: 68.3, GRC: 10.4, HRV: 3.9, HUN: 9.6, IRL: 5.3, ISL: 0.4, ITA: 59.0, LTU: 2.9, LUX: 0.7,
  LVA: 1.9, MDA: 2.4, MKD: 1.8, MLT: 0.6, MNE: 0.6, NLD: 17.9, NOR: 5.5, POL: 36.6, PRT: 10.6, ROU: 19.0, SRB: 6.6, SVK: 5.4,
  SVN: 2.1, SWE: 10.6, TUR: 85.4, UKR: 37.0,
};
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const common = { data: "jobs", key: "state", value: "rate", format: ".1f" };
const europe = { data: "europe", key: "iso", value: "people", layout: "europe", format: ".1f" };

export default doc({
  title: "Unemployment by US state, Europe's population",
  description: "Fifty states and DC as equal squares in a grid, coloured by unemployment rate (about 2 to 5.5 percent), with a colour legend; then as hexagons; then with each rate written under the state's code; then forty European countries by population (0.4 to 85 million), as squares and as hexagons.",
  size: [640, 380],
  data: {
    jobs: data.values({ state: Object.keys(rate), rate: Object.values(rate) }, { key: "state" }),
    europe: data.values({ iso: Object.keys(people), people: Object.values(people) }, { key: "iso" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      tileMap(common, at("default")),
      tileMap({ ...common, shape: "hex" }, at("hex")),
      tileMap({ ...common, shape: "hex", values: true }, at("values")),
      tileMap(europe, at("europe")),
      tileMap({ ...europe, shape: "hex" }, at("europe-hex")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "tileMap({ key, value })", text: "A square per state, darker where more people are out of work; Nevada and DC highest." }),
      step("hex", { title: "shape: \"hex\"", text: "Hexagons, on a grid of their own: Florida still meets Georgia and Alabama, Maine New Hampshire." }),
      step("values", { title: "values: true", text: "The rate under each code, where tiles have room." }),
      step("europe", { title: "layout: \"europe\"", text: "Forty countries by ISO code (alpha-3 here, as the countries atlas; `codes: \"alpha2\"` for DE, FR…), coloured by population: Malta's tile is as big as Germany's." }),
      step("europe-hex", { title: "layout: \"europe\", shape: \"hex\"", text: "Europe as hexagons: Germany among its neighbours, the Balkans joined up, nobody adrift." }),
    ],
  }),
});
