// dotDensity: one dot per `per` units of each country's value, scattered evenly inside it (seeded,
// so the same dots everywhere) — a child of a map of the countries atlas.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { map, dotDensity } from "@datars/std";

// Forest area, km² (rounded, about 2020).
const forest = { SWE: 279_000, FIN: 224_000, NOR: 121_000, LVA: 34_000, EST: 25_000, LTU: 22_000, DNK: 6_000 };
const chart = (layer: ReturnType<typeof dotDensity>, state: string) =>
  map({ source: "world", fit: { bbox: [4, 54, 32, 71] }, children: [layer] }, { key: "chart", when: e(`state == "${state}"`) });
const dots = { source: "world", data: "forest", key: "country", value: "km2" };

export default doc({
  title: "Forest around the Baltic Sea",
  description: "The forest area of seven countries as one dot per 1,000 km², then one per 500 km², then with larger dots.",
  size: [640, 400],
  data: { world: data.atlas("countries"), forest: data.values({ country: Object.keys(forest), km2: Object.values(forest) }, { key: "country" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [
      chart(dotDensity({ ...dots, per: 1000 }), "default"),
      // A new key: a different set of dots fades in (the default's would slide to new places).
      chart(dotDensity({ ...dots, per: 500 }, { key: "dots-500" }), "per"),
      chart(dotDensity({ ...dots, per: 1000, r: 2.2 }), "r"),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "dotDensity({ value, per: 1000 })", text: "One dot per 1,000 km² of forest." }),
      step("per", { title: "per: 500", text: "Half the units per dot: twice the dots." }),
      step("r", { title: "r: 2.2", text: "Dot radius in px (default 1.2): the same dots, easier to see." }),
    ],
  }),
});
