// The maps page's playground (site/pages/features/maps.html, features-maps.js): one camera over an
// automatic basemap, flown by the page to any of the places below — the world, countries, cities
// and street corners. `data.tiles.auto()` makes the build cut one archive for exactly these views
// and the flights between them; the page reads it by range request as the camera moves and shows
// the bytes. Layers are signals the page toggles; the basemap's look is theme tokens it sets.
// NOTE: populations are rounded UN estimates of urban agglomerations (World Urbanization
// Prospects 2018, for 2020); urban shares are rounded World Bank figures (2022). Approximate.
import { doc, data, e, geom, group, instances, motion, op, repeat, shape, signal, step, story, table, view } from "@datars/sdk";
import { attribution, basemap, route } from "@datars/std";

/** Where the camera can go: id, name, kind, [west, south, east, north]. The archive is cut for
 * these views (and the flights from each to the next), so the list is what it covers. */
export const PLACES: [string, string, string, number[]][] = [
  ["world", "The world", "world", [-165, -48, 165, 72]],
  ["iceland", "Iceland", "country", [-24.6, 63.2, -13.3, 66.6]],
  ["sweden", "Sweden", "country", [10.5, 55.2, 24.5, 69.2]],
  ["stockholm", "Stockholm", "city", [17.86, 59.25, 18.24, 59.4]],
  ["gamla-stan", "Gamla stan, Stockholm", "street", [18.062, 59.3205, 18.081, 59.3283]],
  ["denmark", "Denmark", "country", [7.8, 54.5, 13, 57.8]],
  ["copenhagen", "Copenhagen", "city", [12.3, 55.57, 12.8, 55.78]],
  ["portugal", "Portugal", "country", [-10, 36.8, -6, 42.2]],
  ["lisbon", "Lisbon", "city", [-9.48, 38.62, -8.95, 38.86]],
  ["kenya", "Kenya", "country", [33.8, -4.8, 42, 5.2]],
  ["india", "India", "country", [68, 6.5, 97.5, 35.7]],
  ["japan", "Japan", "country", [129, 30.5, 146, 45.7]],
  ["new-zealand", "New Zealand", "country", [166, -47.5, 179, -34]],
  ["usa", "United States", "country", [-125, 24.5, -66.9, 49.5]],
  ["brazil", "Brazil", "country", [-62, -33.5, -33, 3]],
  ["rio", "Rio de Janeiro", "city", [-43.55, -23.08, -43.08, -22.82]],
  ["copacabana", "Copacabana, Rio de Janeiro", "street", [-43.198, -22.99, -43.152, -22.944]],
];

// City, lon, lat, millions (urban agglomeration, rounded).
const cities: [string, number, number, number][] = [
  ["Tokyo", 139.69, 35.69, 37.4], ["Delhi", 77.21, 28.61, 30.3], ["Shanghai", 121.47, 31.23, 27.1], ["São Paulo", -46.63, -23.55, 22.0],
  ["Mexico City", -99.13, 19.43, 21.8], ["Dhaka", 90.41, 23.81, 21.0], ["Cairo", 31.24, 30.04, 20.9], ["Beijing", 116.41, 39.9, 20.5],
  ["Mumbai", 72.88, 19.08, 20.4], ["Osaka", 135.5, 34.69, 19.2], ["New York", -74.01, 40.71, 18.8], ["Karachi", 67.01, 24.86, 16.1],
  ["Buenos Aires", -58.38, -34.6, 15.2], ["Istanbul", 28.98, 41.01, 15.2], ["Lagos", 3.38, 6.52, 14.4], ["Manila", 120.98, 14.6, 13.9],
  ["Rio de Janeiro", -43.2, -22.91, 13.5], ["Moscow", 37.62, 55.76, 12.5], ["Los Angeles", -118.24, 34.05, 12.4], ["Paris", 2.35, 48.86, 11.0],
  ["Jakarta", 106.85, -6.21, 10.8], ["Lima", -77.04, -12.05, 10.7], ["London", -0.13, 51.51, 9.3], ["Nairobi", 36.82, -1.29, 4.7],
  ["Sydney", 151.21, -33.87, 4.9], ["Lisbon", -9.14, 38.72, 2.9], ["Stockholm", 18.07, 59.33, 1.6], ["Copenhagen", 12.57, 55.68, 1.4],
  ["Auckland", 174.76, -36.85, 1.6], ["Reykjavík", -21.94, 64.15, 0.2],
];

// Share of people living in towns and cities, % (rounded).
const urban: [string, number][] = [
  ["SWE", 89], ["DNK", 88], ["NOR", 84], ["FIN", 86], ["ISL", 94], ["PRT", 67], ["ESP", 81], ["FRA", 82], ["DEU", 78], ["GBR", 84],
  ["IRL", 64], ["ITA", 72], ["POL", 60], ["NLD", 93], ["BEL", 98], ["RUS", 75], ["TUR", 77], ["EGY", 43], ["NGA", 54], ["KEN", 29],
  ["ETH", 23], ["ZAF", 68], ["COD", 47], ["IND", 36], ["PAK", 38], ["BGD", 40], ["CHN", 64], ["JPN", 92], ["KOR", 81], ["IDN", 58],
  ["PHL", 48], ["VNM", 39], ["THA", 53], ["AUS", 87], ["NZL", 87], ["USA", 83], ["CAN", 82], ["MEX", 81], ["BRA", 88], ["ARG", 92],
  ["CHL", 88], ["PER", 79], ["COL", 82], ["SAU", 85], ["IRN", 77],
];

const col = <T,>(rows: T[][], i: number) => rows.map((r) => r[i]);
const where = (id: string) => PLACES.find((p) => p[0] === id)![3];
const centre = (b: number[]) => [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];

export default doc({
  id: "maps-playground",
  title: "Fly anywhere the basemap covers",
  description: "One camera over an automatic basemap cut from OpenStreetMap and Natural Earth for exactly these views — the world, ten countries, four cities and two street corners — with thirty cities sized by population, flows from the place on screen to the eleven largest and a choropleth of how urban each country is.",
  size: [960, 560],
  data: {
    // No archive to make: the build cuts one to this document's cameras.
    tiles: data.tiles.auto(),
    world: data.atlas("countries"),
    cities: data.values({ name: col(cities, 0), lon: col(cities, 1), lat: col(cities, 2), pop: col(cities, 3) }, { key: "name" }),
    urban: data.values({ id: col(urban, 0), share: col(urban, 1) }, { key: "id" }),
  },
  tables: {
    // Countries joined to their urban share; flows from wherever the camera is to the eleven largest cities.
    choro: table("world", op.join("urban", "id", "inner")),
    flows: table("cities", op.filter(e("d.pop >= 18")), op.derive("lon0", e("hereLon")), op.derive("lat0", e("hereLat"))),
  },
  signals: {
    bounds: signal.keyset(where("world").map(String)),
    hereLon: signal.num(centre(where("world"))[0]),
    hereLat: signal.num(centre(where("world"))[1]),
    // What kind of place is on screen: a choropleth of countries means nothing in a street.
    scope: signal.str("world"),
    // The layers the page toggles.
    choropleth: signal.bool(false),
    symbols: signal.bool(true),
    flows: signal.bool(false),
    labels: signal.bool(true),
    streets: signal.bool(true),
  },
  motion: motion(
    { select: { kind: "view" }, duration: 3, easing: "cubic-in-out" },
    { select: { role: "datum" }, matcher: "by-key", duration: 0.8 },
  ),
  scene: group({
    key: "root",
    children: [
      view({
        key: "map",
        coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
        camera: { fit: { geo: "=bounds" }, padding: 12 },
        children: [
          group({ key: "with-streets", when: e("streets"), children: [basemap({ source: "tiles", part: "base" })] }),
          group({ key: "without-streets", when: e("!streets"), children: [basemap({ source: "tiles", part: "base", roads: false, buildings: false })] }),
          group({
            key: "choropleth",
            when: e('choropleth && (scope == "world" || scope == "country")'),
            opacity: 0.6,
            scales: { color: { type: "sequential", domain: [20, 100], range: "$sequential" } },
            children: [repeat("choro", shape(geom.feature("world", e("d.id")), {
              fill: e("scale.color(d.share)"),
              stroke: { paint: "$map.border", width: 0.5, nonScaling: true },
              semantics: { role: "region", label: e("`${d.name}: ${d.share}% urban`"), value: e("d.share") },
              pickable: true,
            }))],
          }),
          group({ key: "flows", when: e("flows"), children: [route({ data: "flows", lon0: "lon0", lat0: "lat0", lon1: "lon", lat1: "lat", width: 1.5 })] }),
          group({
            key: "symbols",
            when: e("symbols"),
            scales: { r: { type: "sqrt", domain: [0, 38], range: [0, 18] } },
            children: [instances({
              key: "cities", from: "cities", instanceKey: e("d.name"),
              x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: e("scale.r(d.pop)"),
              fill: "$accent@0.7", stroke: { paint: "$paper", width: 1 }, screenSize: true,
              label: e('`${d.name}: ${format(d.pop, ".1f")} million people`'),
              semantics: { role: "datum", label: "The world's largest cities" },
            })],
          }),
          // Place names above the data.
          group({ key: "names", when: e("labels"), children: [basemap({ source: "tiles", part: "labels" })] }),
        ],
      }),
      attribution({}),
    ],
  }),
  // A state per place: the build cuts the archive for each view and the flight to the next.
  program: story({
    steps: PLACES.map(([id, name, kind, b]) => step(id, { set: { bounds: b, scope: kind, hereLon: centre(b)[0], hereLat: centre(b)[1] }, title: name })),
  }),
});
