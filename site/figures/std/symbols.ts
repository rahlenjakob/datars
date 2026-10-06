// symbols: a circle at each country's visual centre, its area proportional to the population —
// drawn as a child of a map of the countries atlas.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { map, symbols } from "@datars/std";

// Population, millions (rounded, 2023).
const pop: Record<string, number> = {
  DEU: 84, FRA: 68, GBR: 68, ITA: 59, ESP: 48, POL: 37, ROU: 19, NLD: 18, BEL: 12, CZE: 11, SWE: 10.5, GRC: 10.4,
  PRT: 10.4, HUN: 9.6, AUT: 9.1, CHE: 8.8, BGR: 6.4, DNK: 5.9, FIN: 5.6, NOR: 5.5, SVK: 5.4, IRL: 5.3, HRV: 3.9,
  LTU: 2.9, SVN: 2.1, LVA: 1.9, EST: 1.4,
};
const label = e("`${d.id}: ${d.millions} million people`");
const chart = (layer: ReturnType<typeof symbols>, state: string) =>
  map({ source: "world", fit: { bbox: [-11, 35, 32, 70] }, children: [layer] }, { key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Population of European countries",
  description: "Circles sized by population on a map of Europe, then with a smaller maximum radius, then all one size without a value field.",
  size: [640, 400],
  data: { world: data.atlas("countries"), pop: data.values({ id: Object.keys(pop), millions: Object.values(pop) }, { key: "id" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [
      chart(symbols({ source: "world", data: "pop", key: "id", value: "millions", label }), "default"),
      chart(symbols({ source: "world", data: "pop", key: "id", value: "millions", max: 18, label }), "max"),
      chart(symbols({ source: "world", data: "pop", key: "id", r: 6, label }), "same"),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "symbols({ value })", text: "Circle area proportional to the value; the largest has a radius of 28 px." }),
      step("max", { title: "max: 18", text: "A smaller largest radius (px): every circle shrinks with it, and crowded places clear." }),
      step("same", { title: "r: 6, no value", text: "Without a value field every circle is the same size." }),
    ],
  }),
});
