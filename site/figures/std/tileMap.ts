// tileMap: a grid cartogram — every state the same size, roughly in its place, coloured by value
// and marked with its postal code. Unemployment by US state (approximate annual averages for 2023,
// rounded; for illustration).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { tileMap } from "@datars/std";

const rate: Record<string, number> = {
  AL: 2.5, AK: 4.2, AZ: 3.9, AR: 3.3, CA: 4.8, CO: 3.2, CT: 4.0, DE: 4.0, DC: 5.0, FL: 2.9, GA: 3.3, HI: 3.0, ID: 3.2,
  IL: 4.5, IN: 3.3, IA: 2.9, KS: 2.7, KY: 4.2, LA: 3.6, ME: 3.0, MD: 1.9, MA: 3.4, MI: 3.9, MN: 2.8, MS: 3.3, MO: 3.0,
  MT: 2.9, NE: 2.3, NV: 5.4, NH: 2.4, NJ: 4.4, NM: 3.9, NY: 4.2, NC: 3.4, ND: 2.0, OH: 3.5, OK: 3.1, OR: 4.0, PA: 3.4,
  RI: 3.1, SC: 3.0, SD: 1.9, TN: 3.3, TX: 4.0, UT: 2.7, VT: 1.9, VA: 2.9, WA: 4.1, WV: 3.7, WI: 3.0, WY: 3.2,
};
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const common = { data: "jobs", key: "state", value: "rate", format: ".1f" };

export default doc({
  title: "Unemployment by US state",
  description: "Fifty states and DC as equal squares in a grid, coloured by unemployment rate (about 2 to 5.5 percent), with a colour legend; then as hexagons; then with each rate written under the state's code.",
  size: [640, 380],
  data: { jobs: data.values({ state: Object.keys(rate), rate: Object.values(rate) }, { key: "state" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      tileMap(common, at("default")),
      tileMap({ ...common, shape: "hex" }, at("hex")),
      tileMap({ ...common, shape: "hex", values: true }, at("values")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "tileMap({ key, value })", text: "A square per state, darker where more people are out of work; Nevada and DC highest." }),
      step("hex", { title: "shape: \"hex\"", text: "Hexagons: every other row shifted half a tile." }),
      step("values", { title: "values: true", text: "The rate under each code, where tiles have room." }),
    ],
  }),
});
