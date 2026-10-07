// Maps page: one atlas in six projections. Each projection is its own map, shown in its own state;
// the countries are keyed by ISO code, so when the state changes every country morphs from one
// projection into the next instead of the map being redrawn.
// NOTE: urban shares are rounded World Bank figures (2022), approximate.
import { doc, data, e, group, motion, story, step, signal } from "@datars/sdk";
import { map } from "@datars/std";

const urban: [string, number][] = [
  ["SWE", 89], ["DNK", 88], ["NOR", 84], ["FIN", 86], ["ISL", 94], ["PRT", 67], ["ESP", 81], ["FRA", 82], ["DEU", 78], ["GBR", 84],
  ["IRL", 64], ["ITA", 72], ["POL", 60], ["NLD", 93], ["BEL", 98], ["RUS", 75], ["TUR", 77], ["EGY", 43], ["NGA", 54], ["KEN", 29],
  ["ETH", 23], ["ZAF", 68], ["COD", 47], ["IND", 36], ["PAK", 38], ["BGD", 40], ["CHN", 64], ["JPN", 92], ["KOR", 81], ["IDN", 58],
  ["PHL", 48], ["VNM", 39], ["THA", 53], ["AUS", 87], ["NZL", 87], ["USA", 83], ["CAN", 82], ["MEX", 81], ["BRA", 88], ["ARG", 92],
  ["CHL", 88], ["PER", 79], ["COL", 82], ["SAU", 85], ["IRN", 77], ["DZA", 75], ["MAR", 65], ["SDN", 36], ["AGO", 68], ["TZA", 37],
  ["KAZ", 58], ["MNG", 69], ["UKR", 70], ["VEN", 88], ["BOL", 71], ["MDG", 40], ["MOZ", 38], ["NER", 17], ["MLI", 45], ["TCD", 24],
];

const PROJECTIONS: [string, string, string][] = [
  ["equal-earth", "Equal Earth", "Areas true to size: the default for world maps."],
  ["natural-earth", "Natural Earth", "A compromise with rounded edges, easy on the eye."],
  ["mercator", "Mercator", "Angles true, areas not: Greenland as big as Africa."],
  ["equirectangular", "Equirectangular", "Longitude and latitude as a plain grid."],
  ["orthographic", "Orthographic", "The globe, seen from space over the Atlantic."],
];

export default doc({
  title: "One atlas, five projections",
  description: "The world's countries, coloured by how much of their population lives in cities, morphing from one map projection to the next: Equal Earth, Natural Earth, Mercator, equirectangular and orthographic.",
  size: [760, 420],
  data: {
    world: data.atlas("countries"),
    urban: data.values({ id: urban.map((u) => u[0]), share: urban.map((u) => u[1]) }, { key: "id" }),
  },
  signals: { proj: signal.str("equal-earth") },
  motion: motion({ select: { role: "region" }, matcher: "by-key", morph: "resample", duration: 1.4, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [8, 8, 8, 8] },
    children: PROJECTIONS.map(([id]) => map({
      source: "world", data: "urban", key: "id", value: "share", projection: id, format: ".0f", stroke: false,
      label: e("d.share == null ? d.name : `${d.name}: ${d.share}% live in towns and cities`"),
      ...(id === "mercator" ? { fit: { bbox: [-180, -58, 180, 83] } } : {}),
      ...(id === "orthographic" ? { fit: { sphere: true } } : {}),
    }, { key: "map", when: e(`proj == "${id}"`) })),
  }),
  program: story({ steps: PROJECTIONS.map(([id, name, text]) => step(id, { set: { proj: id }, title: name, text })) }),
});
