// Vote lead.
// Each county's winning lead in votes (circle area ∝ lead), in the winner's color.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { card, map, symbols, title } from "@datars/std";
import county_lead from "../data/county_lead.json";
import { civic } from "../theme";
import keys from "../data/keys.json";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const charts = [
  { scene: "us", title: "Each county's lead in votes, 2024", box: [-125, 50, -66, 24], symbols: [] },
  {
    scene: "ne",
    title: "The Northeast corridor",
    box: [-80, 43.6, -69.8, 37.8],
    symbols: [symbols({ source: "geo:cities", data: "geo:cities", key: "id", fill: "$ink", r: 3 })],
  },
];

export default doc({
  id: "us-election-2024/county-lead",
  title: "Vote lead",
  size: [1000, 620],
  theme: civic,
  data: {
    county_lead: data.values(county_lead, { key: "label" }),
    "geo:cities": data.url("cities.geojson", { id: "id" }),
    "geo:counties": data.url("counties.geojson", { id: "id" }),
    "geo:countries": data.atlas("countries"),
    "geo:states": data.url("states.geojson", { id: "id" }),
  },
  signals: { scene: signal.str("us") },
  keys: keys,
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...charts.map((s) => group({
        key: "chart",
        when: e(`scene == "${s.scene}"`),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: s.title }, { key: "title", size: { h: "auto" } }),
          map({
            // The land is drawn by state (51 shapes, not 3,100 counties): the circles carry the counties.
            source: "geo:states",
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            format: ",.0f",
            padding: 0,
            projection: "web-mercator",
            under: "geo:countries",
            children: [
              symbols({
                source: "geo:counties",
                data: "county_lead",
                key: "label",
                value: "value",
                fill: e("key.color(d.label)"),
                format: ",.0f",
                label: e(
                  'd.value == null ? `${key.name(d.label)}` : (`${key.name(d.label)}: won by ${"" + format(d.value, ",.0f") + ""} votes`)',
                ),
                max: 30,
              }),
              ...s.symbols,
            ],
          }, { key: "body" }),
        ],
      })),
      card({
        text: "Red covers most of the land; blue's circles are fewer, and far bigger — the cities.",
        at: "bottom-left",
        width: 270,
      }, { key: "caption", when: e('scene == "us"') }),
      card({ text: "Boston to Washington: a chain of big blue circles.", at: "bottom-right", width: 240 }, {
        key: "caption",
        when: e('scene == "ne"'),
      }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("us", {
        set: { scene: "us" },
        text: "Red covers most of the land; blue's circles are fewer, and far bigger — the cities.",
      }),
      step("ne", { set: { scene: "ne" }, text: "Boston to Washington: a chain of big blue circles." }),
    ],
  }),
});
