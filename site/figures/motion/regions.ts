// A country becomes its bar: the Nordic countries on a map, then ranked as bars, each region's
// outline morphing into its rectangle (through an area-matched disc, so nothing folds on the way)
// and back. The map's regions and the plot's bars come from two recipes; they pair because both
// are keyed by the country's ISO code. Populations 2024, millions, rounded.
import { data, doc, e, group, motion, step, story, choreo } from "@datars/sdk";
import { bar, map, plot } from "@datars/std";

const id = ["SWE", "DNK", "FIN", "NOR", "ISL"];
const name = ["Sweden", "Denmark", "Finland", "Norway", "Iceland"];
const pop = [10.6, 5.9, 5.6, 5.5, 0.4];
const keys = { SWE: { color: "#3b82c4", name: "Sweden" }, DNK: { color: "#d94a4a", name: "Denmark" }, FIN: { color: "#6aa7d8", name: "Finland" }, NOR: { color: "#c0392b", name: "Norway" }, ISL: { color: "#2e5f9e", name: "Iceland" } };
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  id: "motion-regions",
  title: "Regions into bars",
  description: "Sweden, Denmark, Finland, Norway and Iceland on a map, then as bars ranked by population — each country's outline morphing into its bar, and back.",
  size: [640, 400],
  data: { world: data.atlas("countries"), nordics: data.values({ id, name, pop }, { key: "id" }) },
  keys,
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 12] },
    children: [
      map({ source: "world", data: "nordics", key: "id", value: "id", colorType: "categorical", backdrop: false, fit: { keys: id }, label: e("`${d.name}: ${d.pop} million`") }, at("map")),
      plot({ data: "nordics", x: "id", y: "pop", color: "id", title: "Population, millions (2024)", children: [bar({ labels: true, format: ".1f" })] }, at("bars")),
    ],
  }),
  motion: motion(
    { duration: 1.6, easing: "cubic-in-out", choreo: choreo.stagger("left", 0.3) },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { role: "datum" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("map", { title: "Regions", text: "Five countries from the built-in atlas, keyed by ISO code." }),
      step("bars", { title: "Bars", text: "Each outline morphs into its country's bar: same key, different recipe." }),
    ],
  }),
});
