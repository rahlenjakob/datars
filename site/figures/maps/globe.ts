// Maps page: a globe that turns by itself. The orthographic projection's centre is an expression
// over a clock signal — `signal.clock(6)` counts up six degrees a second while the chart is on
// screen — so the countries, the great-circle routes and the hubs all turn with it. The clock
// pauses off screen and for readers who asked for less motion; renders see its default.
// Routes: some of the world's busiest long-haul connections (illustrative, not traffic data).
import { doc, data, e, geom, group, repeat, shape, signal } from "@datars/sdk";
import { geoPoints, route } from "@datars/std";

const hubs: [string, number, number][] = [
  ["London", -0.45, 51.47], ["Dubai", 55.36, 25.25], ["Singapore", 103.99, 1.36], ["New York", -73.78, 40.64],
  ["Tokyo", 139.78, 35.55], ["São Paulo", -46.47, -23.43], ["Johannesburg", 28.24, -26.14], ["Sydney", 151.18, -33.95],
  ["Los Angeles", -118.41, 33.94], ["Delhi", 77.1, 28.56], ["Nairobi", 36.93, -1.32], ["Hong Kong", 113.92, 22.31],
];
const links: [string, string][] = [
  ["London", "New York"], ["London", "Dubai"], ["London", "Singapore"], ["London", "Johannesburg"], ["Dubai", "Delhi"],
  ["Dubai", "Sydney"], ["Dubai", "Nairobi"], ["Singapore", "Sydney"], ["Singapore", "Hong Kong"], ["Hong Kong", "Tokyo"],
  ["Tokyo", "Los Angeles"], ["Los Angeles", "New York"], ["New York", "São Paulo"], ["São Paulo", "Johannesburg"],
  ["Los Angeles", "Sydney"], ["Delhi", "Singapore"], ["Nairobi", "Johannesburg"], ["Dubai", "Hong Kong"],
];
const at = (n: string) => hubs.find((h) => h[0] === n)!;

export default doc({
  title: "A globe that turns by itself",
  description: "An orthographic globe turning slowly eastward, with twelve airport hubs and eighteen long-haul routes drawn as great circles.",
  size: [560, 520],
  data: {
    world: data.atlas("countries"),
    hubs: data.values({ name: hubs.map((h) => h[0]), lon: hubs.map((h) => h[1]), lat: hubs.map((h) => h[2]) }, { key: "name" }),
    links: data.values({
      id: links.map(([a, b]) => `${a}–${b}`),
      lon0: links.map(([a]) => at(a)[1]), lat0: links.map(([a]) => at(a)[2]),
      lon1: links.map(([, b]) => at(b)[1]), lat1: links.map(([, b]) => at(b)[2]),
    }, { key: "id" }),
  },
  // Six degrees a second: once round the world a minute.
  signals: { spin: signal.clock(6) },
  scene: group({
    key: "root",
    coord: { type: "geo", projection: "orthographic", center: [e("20 - spin"), 18], fit: { sphere: true }, padding: 16 } as never,
    children: [
      shape(geom.circle({ cx: e("box.w / 2"), cy: e("box.h / 2"), r: e("min(box.w, box.h) / 2 - 16") }), { key: "ocean", fill: "$map.water", semantics: { role: "decoration" } }),
      repeat("world", shape(geom.feature("world", e("d.id")), { key: e("d.id"), fill: "$map.land", stroke: { paint: "$map.border", width: 0.5, nonScaling: true }, semantics: { role: "decoration" } })),
      route({ data: "links", lon0: "lon0", lat0: "lat0", lon1: "lon1", lat1: "lat1", width: 1.4, ink: "$accent@0.75" }),
      geoPoints({ data: "hubs", lon: "lon", lat: "lat", key: "name", r: 3.5, fill: "$accent", label: e("d.name") }),
    ],
  }),
});
