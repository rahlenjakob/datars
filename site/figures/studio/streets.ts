// Theme studio: a street basemap from a vector-tile archive — water, land, parks, buildings, roads,
// place names and their halo are map tokens; the sights are accent dots. Central Stockholm.
import { doc, data, e, group, signal, view } from "@datars/sdk";
import { attribution, basemap, geoPoints } from "@datars/std";

// Sight, lon, lat, visitors a year in millions (rough orders of magnitude).
const sights: [string, number, number, number][] = [
  ["Royal Palace", 18.0717, 59.3268, 0.8], ["City Hall", 18.0548, 59.3275, 0.4], ["Vasa Museum", 18.0915, 59.328, 1.5],
  ["Skansen", 18.1037, 59.326, 1.4], ["Nationalmuseum", 18.0781, 59.3289, 0.6], ["Central Station", 18.0578, 59.3303, 0.9],
];
const city = [18.03, 59.315, 18.115, 59.337];

export default doc({
  title: "Central Stockholm",
  description: "A street-level basemap of central Stockholm in the theme's map colours, with six sights as dots sized by visitors.",
  size: [960, 420],
  data: {
    tiles: data.tiles("../../../assets/tiles/descent.pmtiles"),
    sights: data.values({ name: sights.map((s) => s[0]), lon: sights.map((s) => s[1]), lat: sights.map((s) => s[2]), visitors: sights.map((s) => s[3]) }, { key: "name" }),
  },
  // The camera frames lon/lat bounds held in a signal (as the basemap reference figure does).
  signals: { bounds: signal.keyset(city.map(String)) },
  scene: group({
    key: "root",
    children: [
      view({
        key: "map",
        coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
        camera: { fit: { geo: "=bounds" }, padding: 0 },
        children: [
          basemap({ source: "tiles", part: "base" }, { key: "base" }),
          geoPoints({ data: "sights", lon: "lon", lat: "lat", key: "name", r: e("3 + d.visitors * 4"), fill: "$accent", label: e("d.name") }),
          basemap({ source: "tiles", part: "labels" }, { key: "labels" }),
        ],
      }),
      attribution({}),
    ],
  }),
});
