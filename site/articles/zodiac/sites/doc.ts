// The Zodiac's Bay Area: the four attacks attributed to him, 1968–69, one at a time in order, each
// with the ones before it; then the disputed abduction of 1970 on Highway 132. Sites are placed from
// public descriptions, to within a few hundred meters.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, attribution, basemap, card, map, symbols, title } from "@datars/std";
import { noir } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const scenes = [
  {
    scene: "bay",
    caption: "Four attacks in ten months, all within about 60 miles of San Francisco.",
    title: "The Zodiac attacks",
    box: [-122.75, 38.75, -121, 37.55],
    children: [
      symbols({ source: "geo:sites", data: "a_all", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
    ],
  },
  {
    scene: "lake-herman",
    caption: "Lake Herman Road, Benicia: two teenagers shot in a lovers' lane turnout.",
    title: "Dec. 20, 1968: Lake Herman Road, Benicia",
    box: [-122.62, 38.66, -121.92, 37.98],
    children: [
      symbols({ source: "geo:sites", data: "a1968", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
      annotate({
        x: e('geo.cx("geo:sites", "lake-herman")'),
        y: e('geo.cy("geo:sites", "lake-herman")'),
        text: "Lake Herman Road",
        connector: "none",
        dot: true,
        dx: 0,
        dy: -10,
        head: false,
        pin: true,
      }),
    ],
  },
  {
    scene: "blue-rock",
    caption: "Blue Rock Springs, Vallejo, about four miles away: one killed, one wounded.",
    title: "July 4, 1969: Blue Rock Springs, Vallejo",
    box: [-122.62, 38.66, -121.92, 37.98],
    children: [
      symbols({ source: "geo:sites", data: "a1969a", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
      annotate({
        x: e('geo.cx("geo:sites", "blue-rock")'),
        y: e('geo.cy("geo:sites", "blue-rock")'),
        text: "Blue Rock Springs",
        connector: "none",
        dot: true,
        dx: 0,
        dy: -10,
        head: false,
        pin: true,
      }),
    ],
  },
  {
    scene: "berryessa",
    caption: "Lake Berryessa: a hooded attacker stabs two students; one survives.",
    title: "Sept. 27, 1969: Lake Berryessa",
    box: [-122.62, 38.72, -121.92, 38.04],
    children: [
      symbols({ source: "geo:sites", data: "a1969b", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
      annotate({
        x: e('geo.cx("geo:sites", "berryessa")'),
        y: e('geo.cy("geo:sites", "berryessa")'),
        text: "Lake Berryessa",
        connector: "none",
        dot: true,
        dx: 0,
        dy: -10,
        head: false,
        pin: true,
      }),
    ],
  },
  {
    scene: "presidio-heights",
    caption: "Washington and Cherry, San Francisco: a cab driver shot in Presidio Heights.",
    title: "Oct. 11, 1969: Presidio Heights, San Francisco",
    box: [-122.56, 37.84, -122.34, 37.7],
    children: [
      symbols({ source: "geo:sites", data: "a1969c", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
      annotate({
        x: e('geo.cx("geo:sites", "presidio-heights")'),
        y: e('geo.cy("geo:sites", "presidio-heights")'),
        text: "Washington & Cherry",
        connector: "none",
        dot: true,
        dx: 0,
        dy: -10,
        head: false,
        pin: true,
      }),
    ],
  },
  {
    scene: "all",
    caption: "With the disputed 1970 abduction on Highway 132, the pattern stretches inland.",
    title: "1968–1970",
    box: [-122.75, 38.75, -121, 37.4],
    children: [
      symbols({ source: "geo:sites", data: "a_all", key: "label", fill: "$ink", r: 4 }),
      basemap({ source: "basemap", part: "labels" }, { key: "basemap-labels" }),
      annotate({
        x: e('geo.cx("geo:sites", "highway-132")'),
        y: e('geo.cy("geo:sites", "highway-132")'),
        text: "Highway 132 (disputed)",
        connector: "none",
        dot: true,
        dx: 0,
        dy: -10,
        head: false,
        pin: true,
      }),
    ],
  },
];

export default doc({
  id: "zodiac/sites",
  title: "The Zodiac's Bay Area",
  size: [1000, 640],
  theme: noir,
  data: {
    a1968: data.values({ label: ["lake-herman"], value: [2] }, { key: "label" }),
    a1969a: data.values({ label: ["lake-herman", "blue-rock"], value: [2, 2] }, { key: "label" }),
    a1969b: data.values({ label: ["lake-herman", "blue-rock", "berryessa"], value: [2, 2, 2] }, { key: "label" }),
    a1969c: data.values({ label: ["lake-herman", "blue-rock", "berryessa", "presidio-heights"], value: [2, 2, 2, 1] }, {
      key: "label",
    }),
    a_all: data.values({
      label: ["lake-herman", "blue-rock", "berryessa", "presidio-heights", "highway-132"],
      value: [2, 2, 2, 1, 1],
    }, { key: "label" }),
    basemap: data.tiles("basemap.pmtiles"),
    "geo:countries": data.atlas("countries"),
    "geo:sites": data.url("sites.geojson", { id: "id" }),
  },
  signals: { scene: signal.str("bay") },
  keys: {
    berryessa: { name: "Lake Berryessa" },
    "blue-rock": { name: "Blue Rock Springs" },
    "highway-132": { name: "Highway 132 (disputed)" },
    "lake-herman": { name: "Lake Herman Road" },
    "presidio-heights": { name: "Washington & Cherry" },
  },
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
            source: "geo:countries",
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            fit: { bbox: [-122.635405, 37.459295, -121.069295, 38.742405000000005] },
            format: ".1~f",
            padding: 0,
            projection: "web-mercator",
            regions: false,
            base: [basemap({ source: "basemap", part: "base" })],
            children: [...s.children],
          }, { key: "body" }),
        ],
      })),
      ...scenes.map((s) => card({ text: s.caption, at: "auto", width: 260 }, {
        key: "caption",
        when: e(`scene == "${s.scene}"`),
      })),
      attribution({}, {
        key: "attribution",
        when: e('["bay","lake-herman","blue-rock","berryessa","presidio-heights","all"].includes(scene)'),
      }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene }, text: s.caption })),
    drivers: [{ scroll: "trigger" }],
  }),
});
