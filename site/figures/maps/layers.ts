// Maps page: one table of populations drawn four ways — a choropleth, proportional symbols, dot
// density and bars. Every mark is keyed by the country's ISO code (the dots by code and dot
// number), so a country's circle breaks into its dots and its region lifts off the map into its bar:
// the engine pairs them by key, no transition code.
// NOTE: populations are rounded 2023 estimates (UN World Population Prospects), approximate.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { bar, dotDensity, map, plot, symbols } from "@datars/std";

const rows: [string, string, number][] = [
  ["BRA", "Brazil", 216], ["COL", "Colombia", 52], ["ARG", "Argentina", 46], ["PER", "Peru", 34], ["VEN", "Venezuela", 28],
  ["CHL", "Chile", 20], ["ECU", "Ecuador", 18], ["BOL", "Bolivia", 12], ["PRY", "Paraguay", 7], ["URY", "Uruguay", 3.4],
];
const fit = { bbox: [-82, -56, -34, 13] };
const shared = { source: "world", data: "pop", key: "id" };

export default doc({
  title: "One table, four maps",
  description: "The populations of ten South American countries as a choropleth, as proportional circles, as one dot per two million people and as bars, the circles breaking into dots and the countries morphing into bars by their codes.",
  size: [600, 560],
  data: {
    world: data.atlas("countries"),
    pop: data.values({ id: rows.map((r) => r[0]), name: rows.map((r) => r[1]), people: rows.map((r) => r[2]) }, { key: "id" }),
  },
  tables: { ranked: { from: "pop", ops: [{ op: "sort", by: [["people", "desc"]] }] } },
  signals: { drawn: signal.str("choropleth") },
  motion: motion(
    { duration: 1.4, easing: "cubic-in-out", matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { role: "datum" }, matcher: "by-key" },
  ),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [10, 10, 10, 10] },
    children: [
      map({ ...shared, value: "people", fit, format: ".0f", legend: true, label: e("d.people == null ? d.name : `${d.name}: ${d.people} million people`") }, { key: "chart", when: e('drawn == "choropleth"') }),
      map({ source: "world", fit, children: [symbols({ ...shared, value: "people", max: 46, format: ".0f", label: e("`${key.name(d.id)}: ${d.people} million people`") })] }, { key: "chart", when: e('drawn == "symbols"') }),
      map({ source: "world", fit, children: [dotDensity({ ...shared, value: "people", per: 2, r: 1.6, fill: "$accent" })] }, { key: "chart", when: e('drawn == "dots"') }),
      plot({ data: "ranked", x: "people", y: "name", xType: "linear", yType: "band", title: "Population, millions", format: ".0f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('drawn == "bars"') }),
    ],
  }),
  program: story({
    steps: [
      step("choropleth", { set: { drawn: "choropleth" }, title: "map()", text: "A choropleth: each country coloured by its population." }),
      step("symbols", { set: { drawn: "symbols" }, title: "symbols()", text: "Each region becomes a circle at its visual centre, its area proportional to the people." }),
      step("dots", { set: { drawn: "dots" }, title: "dotDensity()", text: "Each circle breaks into one dot per two million people, scattered inside the country." }),
      step("bars", { set: { drawn: "bars" }, title: "bar()", text: "And each country lifts off the map into its bar: the same keys, now a ranked chart." }),
    ],
  }),
});
