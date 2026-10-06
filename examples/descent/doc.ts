// From the world to a street: one camera descends from the globe to Sweden to central Stockholm,
// over our own vector tiles (OSM + Natural Earth, a camera-aware extract in assets/tiles — no tile
// service), with a data layer on top. Each flight is the view's camera interpolated along the van
// Wijk path; the basemap asks for the tiles of every zoom it passes (docs/09-geo.md).
// NOTE: populations are rounded, illustrative municipal figures (roughly 2023, SCB), not official
// statistics; visitor numbers are rough orders of magnitude.
import { doc, data, e, group, instances, motion, repeat, scale, signal, step, story, text, view } from "@datars/sdk";
import { attribution, basemap } from "@datars/std";

const cities: [string, number, number, number][] = [
  ["Stockholm", 18.07, 59.33, 984_000], ["Göteborg", 11.97, 57.71, 604_000], ["Malmö", 13.0, 55.6, 362_000],
  ["Uppsala", 17.64, 59.86, 245_000], ["Linköping", 15.62, 58.41, 167_000], ["Örebro", 15.21, 59.27, 158_000],
  ["Västerås", 16.54, 59.61, 158_000], ["Helsingborg", 12.69, 56.05, 151_000], ["Norrköping", 16.19, 58.59, 145_000],
  ["Jönköping", 14.16, 57.78, 144_000], ["Umeå", 20.26, 63.83, 132_000], ["Gävle", 17.14, 60.67, 104_000],
  ["Sundsvall", 17.31, 62.39, 99_000], ["Luleå", 22.15, 65.58, 79_000], ["Östersund", 14.64, 63.18, 65_000],
  ["Kiruna", 20.23, 67.86, 22_000],
];

const sights: [string, string, number, number, number][] = [
  ["slottet", "Royal Palace", 18.0717, 59.3268, 0.8], ["stadshuset", "City Hall", 18.0548, 59.3275, 0.4],
  ["vasa", "Vasa Museum", 18.0915, 59.328, 1.5], ["skansen", "Skansen", 18.1037, 59.326, 1.4],
  ["grona-lund", "Gröna Lund", 18.0966, 59.3233, 1.2], ["fotografiska", "Fotografiska", 18.0858, 59.3178, 0.5],
  ["nationalmuseum", "Nationalmuseum", 18.0781, 59.3289, 0.6], ["central", "Central Station", 18.0578, 59.3303, 0.9],
];

// Centred so the width-limited fit (with its padding) stays inside the world.
const world = [-174, -56, 174, 80];
const sweden = [10.5, 55.2, 24.5, 69.2];
const stockholm = [18.02, 59.308, 18.12, 59.343];

export default doc({
  id: "descent",
  title: "From the world to a street",
  description: "A camera descends from the world to Sweden to central Stockholm over OpenStreetMap vector tiles, with Sweden's largest cities and Stockholm's most visited sights on top.",
  size: [760, 480],
  data: {
    basemap: data.tiles("../../assets/tiles/descent.pmtiles"),
    cities: data.values({ name: cities.map((c) => c[0]), lon: cities.map((c) => c[1]), lat: cities.map((c) => c[2]), pop: cities.map((c) => c[3]) }, { key: "name" }),
    sights: data.values({ id: sights.map((s) => s[0]), name: sights.map((s) => s[1]), lon: sights.map((s) => s[2]), lat: sights.map((s) => s[3]), visitors: sights.map((s) => s[4]) }, { key: "id" }),
  },
  signals: { bounds: signal.keyset(world.map(String)), layer: signal.str("cities") },
  // Flights take their time: the camera eases along the van Wijk path (zoom out, pan, zoom in). Data
  // layers step aside while the camera leaves and arrive once it's there.
  motion: motion(
    { select: { kind: "view" }, duration: 3.2, easing: "cubic-in-out" },
    { select: { role: "datum" }, matcher: "by-key" },
    { when: { from: "sweden", to: "stockholm" }, select: { key: "root/map/cities" }, duration: 0.5 },
    { when: { from: "sweden", to: "stockholm" }, select: { key: "root/map/sights" }, delay: 2.5, duration: 0.7 },
    { when: { from: "stockholm", to: "sweden" }, select: { key: "root/map/sights" }, duration: 0.5 },
    { when: { from: "stockholm", to: "sweden" }, select: { key: "root/map/cities" }, delay: 2.5, duration: 0.7 },
  ),
  scene: group({
    key: "root",
    children: [
      view({
        key: "map",
        coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
        camera: { fit: { geo: "=bounds" }, padding: 12 },
        children: [
          basemap({ source: "basemap", part: "base" }),
          group({
            key: "cities",
            when: e('layer == "cities"'),
            scales: { r: { type: "sqrt", domain: [0, 1_000_000], range: [0, 15] } },
            children: [
              instances({
                key: "circles", from: "cities", instanceKey: e("d.name"),
                x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: e("scale.r(d.pop)"),
                fill: "$accent@0.72", stroke: { paint: "$paper", width: 1 }, screenSize: true,
                label: e('`${d.name}: ${format(d.pop, ",.0f")} people`'),
                semantics: { role: "datum", label: "Swedish cities by population" },
              }),
            ],
          }),
          group({
            key: "sights",
            // Names that would land on each other in a narrow box: the next one goes below its dot, or out.
            declutter: true,
            when: e('layer == "sights"'),
            scales: { r: { type: "sqrt", domain: [0, 1.5], range: [0, 11] } },
            children: [
              instances({
                key: "dots", from: "sights", instanceKey: e("d.id"),
                x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: e("scale.r(d.visitors)"),
                fill: "$accent@0.8", stroke: { paint: "$paper", width: 1.25 }, screenSize: true,
                label: e('`${d.name}: about ${format(d.visitors, ".1f")} million visitors a year`'),
                semantics: { role: "datum", label: "Stockholm's most visited sights" },
              }),
              repeat("sights", text(e("d.name"), [e("geo.x(d.lon, d.lat)"), e("geo.y(d.lon, d.lat)")], {
                key: e("d.id"),
                offset: [0, e("-scale.r(d.visitors) - 3")],
                style: { size: 11.5, weight: 600, ink: "$ink", align: "middle", baseline: "bottom" },
                halo: ["$paper", 2.5],
              })),
            ],
          }),
          // Place names above the data.
          basemap({ source: "basemap", part: "labels" }),
        ],
      }),
      attribution({}),
    ],
  }),
  program: story({
    steps: [
      step("world", { set: { bounds: world, layer: "cities" }, title: "One camera", text: "The whole world, drawn from our own vector tiles: Natural Earth at this zoom." }),
      step("sweden", { set: { bounds: sweden, layer: "cities" }, title: "Sweden", text: "The camera flies in; tiles of every zoom it passes stream in on the way. Circles: the largest municipalities." }),
      step("stockholm", { set: { bounds: stockholm, layer: "sights" }, title: "Stockholm", text: "Down to the streets: OpenStreetMap roads, water and buildings, and the city's most visited sights." }),
    ],
  }),
});
