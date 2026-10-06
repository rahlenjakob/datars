// Renewable share of energy use in Europe: a world map zooms to Europe, then to the Nordics, and
// the Nordic countries become a ranked bar chart — each country's region morphs into its bar
// because both are keyed by the ISO code (docs/09-geo.md). The atlas is fetched by the host.
// NOTE: rounded, illustrative figures (roughly 2022, Eurostat / IRENA), not official statistics.
import { doc, data, e, group, motion, signal, story, step, view } from "@datars/sdk";
import { plot, bar, map } from "@datars/std";

const rows: [string, string, number][] = [
  ["ISL", "Iceland", 85], ["NOR", "Norway", 75], ["SWE", "Sweden", 66], ["FIN", "Finland", 48], ["LVA", "Latvia", 43],
  ["DNK", "Denmark", 42], ["EST", "Estonia", 38], ["PRT", "Portugal", 35], ["AUT", "Austria", 34], ["LTU", "Lithuania", 30],
  ["HRV", "Croatia", 29], ["SVN", "Slovenia", 25], ["ROU", "Romania", 24], ["GRC", "Greece", 23], ["ESP", "Spain", 22],
  ["DEU", "Germany", 21], ["FRA", "France", 20], ["CYP", "Cyprus", 19], ["ITA", "Italy", 19], ["BGR", "Bulgaria", 19],
  ["CZE", "Czechia", 18], ["SVK", "Slovakia", 18], ["POL", "Poland", 17], ["HUN", "Hungary", 15], ["NLD", "Netherlands", 15],
  ["GBR", "United Kingdom", 14], ["LUX", "Luxembourg", 14], ["BEL", "Belgium", 14], ["MLT", "Malta", 13], ["IRL", "Ireland", 13],
];
const nordics = ["ISL", "NOR", "SWE", "FIN", "DNK"];

export default doc({
  id: "renewables",
  title: "Renewable share of energy use, Europe",
  size: [760, 480],
  data: {
    world: data.atlas("countries"),
    renew: data.values({ id: rows.map((r) => r[0]), name: rows.map((r) => r[1]), share: rows.map((r) => r[2]) }, { key: "id" }),
  },
  tables: {
    nordic: { from: "renew", ops: [{ op: "filter", expr: { expr: `${JSON.stringify(nordics)}.includes(d.id)` } }, { op: "sort", by: [["share", "desc"]] }] },
  },
  signals: { focus: signal.keyset(), shape: signal.str("map") },
  // Regions and bars pair by their own key (the ISO code) wherever they sit in the tree.
  motion: motion({ select: { role: "region" }, matcher: "by-key" }, { select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      view({
        key: "chart",
        when: e('shape == "map"'),
        camera: { fit: { keys: "=focus" }, padding: 24 },
        children: [map({ source: "world", data: "renew", key: "id", value: "share", projection: "equal-earth", format: ".0f" })],
      }),
      plot({ data: "nordic", x: "share", y: "name", xType: "linear", yType: "band", title: "Renewable share of energy use (%)", format: ".0f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('shape == "bars"') }),
    ],
  }),
  program: story({
    steps: [
      step("world", { set: { focus: [], shape: "map" }, title: "Renewables", text: "Share of energy use from renewable sources." }),
      step("europe", { set: { focus: rows.map((r) => r[0]), shape: "map" }, text: "Across Europe the share ranges from 13% to 85%." }),
      step("nordics", { set: { focus: nordics, shape: "map" }, text: "The Nordic countries lead." }),
      step("ranked", { set: { focus: nordics, shape: "bars" }, text: "Iceland and Norway run mostly on hydro and geothermal power." }),
    ],
  }),
});
