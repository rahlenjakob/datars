// basemap: streets, water, parks, buildings and place names from a vector-tile archive (a static
// file read by range), in the view's projection and the theme's map colours. Central Stockholm.
import { doc, data, e, group, motion, signal, story, step, view } from "@datars/sdk";
import { attribution, basemap, geoPoints } from "@datars/std";

// Sight, lon, lat, visitors a year in millions (rough orders of magnitude).
const sights: [string, number, number, number][] = [
  ["Royal Palace", 18.0717, 59.3268, 0.8], ["City Hall", 18.0548, 59.3275, 0.4], ["Vasa Museum", 18.0915, 59.328, 1.5],
  ["Skansen", 18.1037, 59.326, 1.4], ["Nationalmuseum", 18.0781, 59.3289, 0.6], ["Central Station", 18.0578, 59.3303, 0.9],
];
const city = [18.03, 59.31, 18.115, 59.341], region = [16.2, 58.75, 19.4, 60.15];
const when = (test: string) => ({ when: e(test) });

export default doc({
  title: "Central Stockholm",
  description: "A street-level basemap of central Stockholm, then split around a layer of sights so place names stay on top, then flown out to the region around the city.",
  size: [640, 400],
  data: {
    tiles: data.tiles("../../../assets/tiles/descent.pmtiles"),
    sights: data.values({ name: sights.map((s) => s[0]), lon: sights.map((s) => s[1]), lat: sights.map((s) => s[2]), visitors: sights.map((s) => s[3]) }, { key: "name" }),
  },
  signals: { bounds: signal.keyset(city.map(String)) },
  motion: motion({ select: { kind: "view" }, duration: 2.4, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    children: [
      view({
        key: "map",
        coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
        camera: { fit: { geo: "=bounds" }, padding: 0 },
        children: [
          basemap({ source: "tiles" }, { key: "all", ...when('state == "default"') }),
          basemap({ source: "tiles", part: "base" }, { key: "base", ...when('state != "default"') }),
          geoPoints({ data: "sights", lon: "lon", lat: "lat", key: "name", r: e("3 + d.visitors * 4"), fill: "$accent", label: e("d.name") }, when('state == "part"')),
          basemap({ source: "tiles", part: "labels" }, { key: "labels", ...when('state != "default"') }),
        ],
      }),
      attribution({}),
    ],
  }),
  program: story({
    steps: [
      step("default", { set: { bounds: city }, title: "basemap({ source })", text: "Everything the archive has at this zoom, in the theme's map colours." }),
      step("part", { set: { bounds: city }, title: "part: \"base\" and \"labels\"", text: "Two basemaps around a data layer: place names stay above the dots." }),
      step("zoom", { set: { bounds: region }, title: "Any zoom", text: "The camera flies out: the archive holds tiles for each zoom, and place names re-place themselves." }),
    ],
  }),
});
