// route: a great-circle arc per row, from (lon0, lat0) to (lon1, lat1) — the shortest way round the
// globe, curved by the projection. Here long-haul flights from Helsinki, with a dot at each end.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { map, route, geoPoints } from "@datars/std";

// Destination, lon, lat, flights a week (illustrative).
const routes: [string, number, number, number][] = [
  ["New York", -74.0, 40.7, 7], ["Chicago", -87.6, 41.9, 4], ["Tokyo", 139.7, 35.7, 14], ["Seoul", 127.0, 37.6, 5],
  ["Shanghai", 121.5, 31.2, 7], ["Singapore", 103.8, 1.35, 7], ["Bangkok", 100.5, 13.75, 10], ["Delhi", 77.2, 28.6, 5], ["Dubai", 55.3, 25.2, 7],
];
const flights = { data: "routes", lon0: "lon0", lat0: "lat0", lon1: "lon1", lat1: "lat1" };
const chart = (layer: ReturnType<typeof route>, state: string) => map({
  source: "world", fit: { bbox: [-95, 0, 145, 72] },
  children: [layer, geoPoints({ data: "routes", lon: "lon1", lat: "lat1", key: "to", label: e("`Helsinki → ${d.to}: ${d.weekly} flights a week`") })],
}, { key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Long-haul flights from Helsinki",
  description: "Nine routes from Helsinki as great-circle arcs, then with each arc as thick as its flights a week.",
  size: [640, 300],
  data: {
    world: data.atlas("countries"),
    routes: data.values({ to: routes.map((r) => r[0]), lon0: routes.map(() => 24.94), lat0: routes.map(() => 60.17), lon1: routes.map((r) => r[1]), lat1: routes.map((r) => r[2]), weekly: routes.map((r) => r[3]) }, { key: "to" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 12, 12, 12] },
    children: [chart(route(flights), "default"), chart(route({ ...flights, width: e("d.weekly / 2.5") }), "width")],
  }),
  program: story({
    steps: [
      step("default", { title: "route({ lon0, lat0, lon1, lat1 })", text: "The shortest path over the globe: routes to Asia cross Siberia." }),
      step("width", { title: "width: e(\"d.weekly / 2.5\")", text: "The stroke width as an expression over the row: flights a week." }),
    ],
  }),
});
