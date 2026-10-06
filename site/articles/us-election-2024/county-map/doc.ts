// Counties.
// 3,100 counties, each shaded by its margin. The state lines and the largest cities sit on top.
// Pennsylvania: Philadelphia and Pittsburgh vote heavily for the Democrats; nearly everything
// between them is red. Georgia: metro Atlanta against the rest of the state. Wisconsin: Milwaukee
// and Madison — and the counties around them decided the closest state of all. Arizona: Maricopa
// County, home to three in five Arizonans, went to Trump by 3.5 points. The Rio Grande Valley:
// heavily Hispanic border counties such as Starr and Hidalgo — Democratic for generations — voted
// for Trump.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, geoLines, map, symbols, title } from "@datars/std";
import county_margin from "../data/county_margin.json";
import { civic } from "../theme";
import keys from "../data/keys.json";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const scenes = [
  { scene: "us", title: "Margin by county, 2024 (Trump − Harris, points)", box: [-125, 50, -66, 24], notes: [] },
  {
    scene: "pa",
    title: "Pennsylvania",
    box: [-80.8, 42.3, -74.6, 39.6],
    notes: [
      {
        text: "Philadelphia: Harris +59",
        x: e('geo.cx("geo:cities", "philadelphia")'),
        y: e('geo.cy("geo:cities", "philadelphia")'),
      },
      {
        text: "Pittsburgh (Allegheny): Harris +20",
        x: e('geo.cx("geo:cities", "pittsburgh")'),
        y: e('geo.cy("geo:cities", "pittsburgh")'),
      },
    ],
  },
  {
    scene: "ga",
    title: "Georgia",
    box: [-85.7, 35.1, -80.8, 30.3],
    notes: [
      {
        text: "Atlanta (Fulton): Harris +45",
        x: e('geo.cx("geo:cities", "atlanta")'),
        y: e('geo.cy("geo:cities", "atlanta")'),
      },
    ],
  },
  {
    scene: "wi",
    title: "Wisconsin",
    box: [-93, 47.2, -86.7, 42.4],
    notes: [
      {
        text: "Milwaukee: Harris +39",
        x: e('geo.cx("geo:cities", "milwaukee")'),
        y: e('geo.cy("geo:cities", "milwaukee")'),
      },
      {
        text: "Madison (Dane): Harris +52",
        x: e('geo.cx("geo:cities", "madison")'),
        y: e('geo.cy("geo:cities", "madison")'),
      },
    ],
  },
  {
    scene: "az",
    title: "Arizona",
    box: [-115, 37.1, -108.9, 31.2],
    notes: [
      { text: "Maricopa: Trump +3.5", x: e('geo.cx("geo:cities", "phoenix")'), y: e('geo.cy("geo:cities", "phoenix")') },
    ],
  },
  {
    scene: "tx",
    title: "The Rio Grande Valley",
    box: [-100.5, 28.4, -96.8, 25.6],
    notes: [
      {
        text: "Starr County: Trump +16",
        x: e('geo.cx("geo:counties", "48427")'),
        y: e('geo.cy("geo:counties", "48427")'),
      },
      {
        text: "Hidalgo County: Trump +3",
        x: e('geo.cx("geo:counties", "48215")'),
        y: e('geo.cy("geo:counties", "48215")'),
      },
    ],
  },
];

export default doc({
  id: "us-election-2024/county-map",
  title: "Counties",
  size: [1000, 620],
  theme: civic,
  data: {
    county_margin: data.values(county_margin, { key: "label" }),
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
      ...scenes.map((s) => group({
        key: "chart",
        when: e(`scene == "${s.scene}"`),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: s.title }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:counties",
            data: "county_margin",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: false,
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            format: ",.1f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.1f") + ""} pts (Trump − Harris)`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#1e3a8a -50 · #60a5fa -10 · #f5f5f4 0 · #f87171 10 · #7f1d1d 50",
            under: "geo:countries",
            children: [
              geoLines({ source: "geo:states", ink: "$map.border", width: 0.8 }),
              symbols({ source: "geo:cities", data: "geo:cities", key: "id", fill: "$ink", r: 3 }),
              ...s.notes.map((n) => annotate({
                x: n.x,
                y: n.y,
                text: n.text,
                connector: "none",
                dot: true,
                dx: 0,
                dy: -10,
                head: false,
                pin: true,
              })),
            ],
          }, { key: "body" }),
        ],
      })),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene } })),
    drivers: [{ scroll: "trigger" }],
  }),
});
