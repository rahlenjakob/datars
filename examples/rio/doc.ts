// From the world to Copacabana: one camera flies from the globe to Brazil, to Rio de Janeiro and down
// to the street, over an automatic basemap. `data.tiles.auto()` asks the build for it: `datars
// render/dev/publish` cut an archive to this document's own cameras — every state and the flights
// between them — from OpenStreetMap (streets, water, parks, buildings, coastline) and Natural Earth,
// fetched once into the local geodata cache (docs/09-geo.md). Nobody made a tile archive for Rio.
// NOTE: populations are rounded, illustrative figures (IBGE 2022 municipal counts), not official
// statistics; visitor numbers are rough orders of magnitude.
import { doc, data, e, group, instances, motion, repeat, scale, signal, step, story, text, view } from "@datars/sdk";
import { attribution, basemap } from "@datars/std";

const cities: [string, number, number, number][] = [
  ["São Paulo", -46.63, -23.55, 11_450_000], ["Rio de Janeiro", -43.2, -22.91, 6_210_000], ["Brasília", -47.88, -15.79, 2_820_000],
  ["Fortaleza", -38.54, -3.73, 2_430_000], ["Salvador", -38.5, -12.97, 2_420_000], ["Belo Horizonte", -43.94, -19.92, 2_320_000],
  ["Manaus", -60.02, -3.12, 2_060_000], ["Curitiba", -49.27, -25.43, 1_770_000], ["Recife", -34.88, -8.05, 1_490_000],
  ["Porto Alegre", -51.23, -30.03, 1_330_000], ["Belém", -48.5, -1.46, 1_300_000],
];

// id, name, lon, lat, visitors (millions a year), which side of its dot the label sits on across the
// city (at street level every label starts at its dot: there is room, and none leaves the frame).
const sights: [string, string, number, number, number, string][] = [
  ["cristo", "Christ the Redeemer", -43.2105, -22.9519, 2.0, "end"], ["sugarloaf", "Sugarloaf Mountain", -43.1566, -22.9486, 1.5, "start"],
  ["copacabana", "Copacabana Beach", -43.1822, -22.9711, 3.0, "middle"], ["ipanema", "Ipanema Beach", -43.2046, -22.9868, 2.0, "middle"],
  ["maracana", "Maracanã", -43.2302, -22.9122, 1.0, "middle"], ["amanha", "Museum of Tomorrow", -43.1795, -22.8944, 1.0, "middle"],
  ["forte", "Copacabana Fort", -43.1872, -22.9864, 0.4, "middle"], ["urca", "Urca", -43.1655, -22.9485, 0.3, "middle"],
];

// Centred so the width-limited fit (with its padding) stays inside the world.
const world = [-174, -56, 174, 80];
const brazil = [-62, -33.5, -33, 3];
const rio = [-43.55, -23.08, -43.08, -22.82];
const copacabana = [-43.198, -22.99, -43.152, -22.944];

export default doc({
  id: "rio",
  title: "From the world to Copacabana",
  description: "A camera flies from the world to Brazil, Rio de Janeiro and down to Copacabana and Sugarloaf Mountain, over a basemap made automatically from OpenStreetMap for exactly these views, with Brazil's largest cities and Rio's sights on top.",
  size: [760, 480],
  data: {
    // No archive to make: the build cuts one to this document's cameras.
    basemap: data.tiles.auto(),
    cities: data.values({ name: cities.map((c) => c[0]), lon: cities.map((c) => c[1]), lat: cities.map((c) => c[2]), pop: cities.map((c) => c[3]) }, { key: "name" }),
    sights: data.values({ id: sights.map((s) => s[0]), name: sights.map((s) => s[1]), lon: sights.map((s) => s[2]), lat: sights.map((s) => s[3]), visitors: sights.map((s) => s[4]), side: sights.map((s) => s[5]) }, { key: "id" }),
  },
  // `minLabel`: the smaller sights are named only at street level, where there is room.
  signals: { bounds: signal.keyset(world.map(String)), layer: signal.str("cities"), minLabel: signal.num(0) },
  motion: motion(
    { select: { kind: "view" }, duration: 3.2, easing: "cubic-in-out" },
    { select: { role: "datum" }, matcher: "by-key" },
    { when: { from: "brazil", to: "rio" }, select: { key: "root/map/cities" }, duration: 0.5 },
    { when: { from: "brazil", to: "rio" }, select: { key: "root/map/sights" }, delay: 2.5, duration: 0.7 },
    { when: { from: "rio", to: "brazil" }, select: { key: "root/map/sights" }, duration: 0.5 },
    { when: { from: "rio", to: "brazil" }, select: { key: "root/map/cities" }, delay: 2.5, duration: 0.7 },
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
            scales: { r: { type: "sqrt", domain: [0, 12_000_000], range: [0, 16] } },
            children: [
              instances({
                key: "circles", from: "cities", instanceKey: e("d.name"),
                x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: e("scale.r(d.pop)"),
                fill: "$accent@0.72", stroke: { paint: "$paper", width: 1 }, screenSize: true,
                label: e('`${d.name}: ${format(d.pop, ",.0f")} people`'),
                semantics: { role: "datum", label: "Brazil's largest cities by population" },
              }),
            ],
          }),
          group({
            key: "sights",
            // Names kept off each other (below the dot, or out) and inside the frame.
            declutter: true,
            when: e('layer == "sights"'),
            scales: { r: { type: "sqrt", domain: [0, 3], range: [0, 10] } },
            children: [
              instances({
                key: "dots", from: "sights", instanceKey: e("d.id"),
                x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: e("scale.r(d.visitors)"),
                fill: "$accent@0.85", stroke: { paint: "$paper", width: 1.25 }, screenSize: true,
                label: e('`${d.name}: about ${format(d.visitors, ".1f")} million visitors a year`'),
                semantics: { role: "datum", label: "Rio de Janeiro's sights" },
              }),
              repeat("sights", text(e("d.visitors >= minLabel ? d.name : \"\""), [e("geo.x(d.lon, d.lat)"), e("geo.y(d.lon, d.lat)")], {
                key: e("d.id"),
                offset: [0, e("-scale.r(d.visitors) - 3")],
                style: { size: 11.5, weight: 600, ink: "$ink", align: e(`minLabel > 0 ? d.side : "start"`), baseline: "bottom", contain: true },
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
      step("world", { set: { bounds: world, layer: "cities" }, title: "One camera", text: "The whole world, from a basemap nobody had to make: Natural Earth at this zoom." }),
      step("brazil", { set: { bounds: brazil, layer: "cities" }, title: "Brazil", text: "The camera flies in. Circles: Brazil's largest cities by population." }),
      step("rio", { set: { bounds: rio, layer: "sights", minLabel: 1 }, title: "Rio de Janeiro", text: "OpenStreetMap from here down: the coastline, Guanabara Bay, the Tijuca forest and the main roads, fetched for these views when the document was built." }),
      step("copacabana", { set: { bounds: copacabana, layer: "sights", minLabel: 0 }, title: "Copacabana", text: "Down to the street: the beach, Leme and Sugarloaf Mountain, with every street and building of the neighbourhood." }),
    ],
  }),
});
