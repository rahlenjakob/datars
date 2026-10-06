// geoPoints: points at longitude/latitude, projected by the map they're in. `r` and `fill` take
// expressions over the row, so the steps size the cities by population and mark the capitals.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { map, geoPoints } from "@datars/std";

// City, lon, lat, population in thousands (municipal, rounded), capital?
const cities: [string, number, number, number, boolean][] = [
  ["Stockholm", 18.07, 59.33, 984, true], ["Oslo", 10.75, 59.91, 709, true], ["Helsinki", 24.94, 60.17, 664, true],
  ["Copenhagen", 12.57, 55.68, 660, true], ["Riga", 24.11, 56.95, 605, true], ["Vilnius", 25.28, 54.69, 590, true],
  ["Tallinn", 24.75, 59.44, 457, true], ["Gothenburg", 11.97, 57.71, 604, false], ["Malmö", 13.0, 55.6, 362, false],
  ["Aarhus", 10.2, 56.16, 360, false], ["Bergen", 5.32, 60.39, 290, false], ["Tampere", 23.76, 61.5, 250, false],
  ["Uppsala", 17.64, 59.86, 245, false], ["Trondheim", 10.4, 63.43, 212, false], ["Oulu", 25.47, 65.01, 212, false],
  ["Turku", 22.27, 60.45, 200, false], ["Umeå", 20.26, 63.83, 132, false], ["Luleå", 22.15, 65.58, 79, false],
  ["Tromsø", 18.96, 69.65, 78, false], ["Tartu", 26.72, 58.38, 97, false], ["Kaunas", 23.9, 54.9, 300, false],
];
const pts = { data: "cities", lon: "lon", lat: "lat", key: "name", label: e("`${d.name}: ${format(d.pop * 1000, ',')} people`") };
const chart = (layer: ReturnType<typeof geoPoints>, state: string) =>
  map({ source: "world", fit: { bbox: [4, 54, 32, 71] }, children: [layer] }, { key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Cities around the Baltic Sea",
  description: "Twenty-one cities as points, then sized by population, then with the capitals in the accent colour.",
  size: [640, 400],
  data: {
    world: data.atlas("countries"),
    cities: data.values({ name: cities.map((c) => c[0]), lon: cities.map((c) => c[1]), lat: cities.map((c) => c[2]), pop: cities.map((c) => c[3]), capital: cities.map((c) => c[4]) }, { key: "name" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [
      chart(geoPoints(pts), "default"),
      chart(geoPoints({ ...pts, r: e("sqrt(d.pop) / 4") }), "r"),
      chart(geoPoints({ ...pts, r: e("sqrt(d.pop) / 4"), fill: e('d.capital ? "$accent" : "$map.marker"') }), "fill"),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "geoPoints({ lon, lat })", text: "A marker at each city, ringed so it reads on any land." }),
      step("r", { title: "r: e(\"sqrt(d.pop) / 4\")", text: "The radius as an expression over the row: area by population." }),
      step("fill", { title: "fill: e(…)", text: "The fill as an expression too: capitals in the accent colour." }),
    ],
  }),
});
