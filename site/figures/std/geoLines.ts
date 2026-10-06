// geoLines: the line features of a geo source as strokes that keep their width at any zoom — here
// ferry lines across the Baltic Sea, as inline GeoJSON over the countries atlas.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { map, geoLines } from "@datars/std";

// Line, passengers a year in millions (illustrative), [lon, lat] waypoints.
const lines: [string, number, number[][]][] = [
  ["Helsinki–Tallinn", 9, [[24.95, 60.16], [24.8, 59.8], [24.75, 59.44]]],
  ["Stockholm–Helsinki", 2.5, [[18.1, 59.32], [19, 59.6], [19.93, 60.1], [21.5, 60], [23, 59.9], [24.95, 60.16]]],
  ["Stockholm–Turku", 2.2, [[18.1, 59.32], [19, 59.6], [19.93, 60.1], [21, 60.25], [22.2, 60.43]]],
  ["Stockholm–Tallinn", 1.3, [[18.1, 59.32], [19.2, 59.4], [21.5, 59.5], [24.75, 59.44]]],
  ["Stockholm–Riga", 0.6, [[18.1, 59.32], [19.3, 58.9], [21, 57.9], [23, 57.3], [24.1, 57]]],
  ["Nynäshamn–Visby", 1, [[17.95, 58.9], [18.29, 57.64]]], ["Oskarshamn–Visby", 0.5, [[16.45, 57.26], [18.29, 57.64]]],
  ["Gdynia–Karlskrona", 0.7, [[18.55, 54.53], [17.5, 55.2], [15.59, 56.16]]], ["Travemünde–Trelleborg", 0.9, [[10.87, 53.96], [12, 54.6], [13.16, 55.37]]],
];
const ferries = { type: "FeatureCollection", features: lines.map(([id, , coordinates]) => ({ type: "Feature", properties: { id }, geometry: { type: "LineString", coordinates } })) };
const chart = (layer: ReturnType<typeof geoLines>, state: string) =>
  map({ source: "world", fit: { bbox: [10, 53.6, 26.5, 61] }, children: [layer] }, { key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Ferry lines across the Baltic Sea",
  description: "Nine ferry lines drawn as lines, then dashed, then as data: one line per row, coloured by passengers a year.",
  size: [640, 400],
  data: {
    world: data.atlas("countries"),
    ferries: data.geojson(ferries, { id: "id" }),
    passengers: data.values({ line: lines.map((l) => l[0]), millions: lines.map((l) => l[1]) }, { key: "line" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [
      chart(geoLines({ source: "ferries", ink: "$accent", width: 2 }), "default"),
      chart(geoLines({ source: "ferries", ink: "$accent", width: 2, dash: true }), "dash"),
      chart(geoLines({ source: "ferries", data: "passengers", key: "line", value: "millions", stops: "#f2b134 0.5 · #c2362b 3", width: 3, format: ".1f" }), "value"),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "geoLines({ source, ink, width })", text: "Every line feature of the source, as decoration." }),
      step("dash", { title: "dash: true", text: "Dashed strokes, for routes that aren't roads." }),
      step("value", { title: "data, key, value, stops", text: "One line per row, coloured on piecewise stops; hover one for its passengers." }),
    ],
  }),
});
