// Record cities.
// Men's and women's marathon world records by host city (circle area ∝ records), each record city
// labelled.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, attribution, basemap, card, map, symbols, title } from "@datars/std";
import { sport } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const scenes = [
  {
    scene: "men",
    at: "bottom-left",
    caption: "men's records since 1967 were set in Berlin.",
    title: "9 of 18",
    width: 250,
    title2: "Men's world records by city, since 1967",
    box: [-110, 62, 145, 20],
    format: ",.0f",
    children: [
      symbols({
        source: "geo:cities",
        data: "wr_men",
        key: "label",
        value: "value",
        format: ",.0f",
        label: e(
          'd.value == null ? `${key.name(d.label)}` : (`${key.name(d.label)}: ${"" + format(d.value, ",.0f") + ""} world records`)',
        ),
        max: 34,
        stops: "#f3cdbf 1 · #c4431d 9",
      }),
      annotate({ x: e("geo.x(13.4, 52.52)"), y: e("geo.y(13.4, 52.52)"), text: "Berlin: 9", dx: 40, dy: -40, width: 150, pin: true }),
      annotate({ x: e("geo.x(-87.63, 41.88)"), y: e("geo.y(-87.63, 41.88)"), text: "Chicago: 3", dx: 40, dy: -40, width: 150, pin: true }),
      annotate({ x: e("geo.x(130.4, 33.59)"), y: e("geo.y(130.4, 33.59)"), text: "Fukuoka: 2", dx: -40, dy: -40, width: 150, pin: true }),
      annotate({ x: e("geo.x(4.48, 51.92)"), y: e("geo.y(4.48, 51.92)"), text: "Rotterdam 2, Antwerp 1, London 1", dx: -60, dy: 60, width: 150, pin: true }),
    ],
  },
  {
    scene: "women",
    at: "bottom-left",
    caption: "For women, Chicago leads: four of the last ten records.",
    width: 250,
    title2: "Women's world records by city, since 1985",
    box: [-110, 62, 145, 20],
    format: ",.0f",
    children: [
      symbols({
        source: "geo:cities",
        data: "wr_women",
        key: "label",
        value: "value",
        format: ",.0f",
        label: e(
          'd.value == null ? `${key.name(d.label)}` : (`${key.name(d.label)}: ${"" + format(d.value, ",.0f") + ""} world records`)',
        ),
        max: 34,
        stops: "#c9d6e3 1 · #2f5f8a 4",
      }),
      annotate({ x: e("geo.x(-87.63, 41.88)"), y: e("geo.y(-87.63, 41.88)"), text: "Chicago: 4", dx: 40, dy: -40, width: 150, pin: true }),
      annotate({ x: e("geo.x(13.4, 52.52)"), y: e("geo.y(13.4, 52.52)"), text: "Berlin: 3", dx: 40, dy: -40, width: 150, pin: true }),
      annotate({ x: e("geo.x(-0.13, 51.51)"), y: e("geo.y(-0.13, 51.51)"), text: "London 2, Rotterdam 1", dx: -60, dy: 60, width: 150, pin: true }),
    ],
  },
];

export default doc({
  id: "marathon/cities",
  title: "Record cities",
  size: [1000, 600],
  theme: sport,
  data: {
    basemap: data.tiles("basemap.pmtiles"),
    "geo:cities": data.url("cities.geojson", { id: "id" }),
    "geo:countries": data.atlas("countries"),
    wr_men: data.values({
      label: ["berlin", "chicago", "rotterdam", "fukuoka", "antwerp", "london"],
      value: [9, 3, 2, 2, 1, 1],
    }, { key: "label" }),
    wr_women: data.values({ label: ["berlin", "chicago", "rotterdam", "london"], value: [3, 4, 1, 2] }, { key: "label" }),
  },
  signals: { scene: signal.str("men") },
  keys: {
    antwerp: { name: "Antwerp" },
    berlin: { name: "Berlin" },
    chicago: { name: "Chicago" },
    fukuoka: { name: "Fukuoka" },
    london: { name: "London" },
    rotterdam: { name: "Rotterdam" },
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
          title({ text: s.title2 }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            fit: { bbox: [-120.33449999999999, 0.8855000000000075, 163.1045, 85.2245] },
            format: s.format,
            padding: 0,
            projection: "web-mercator",
            regions: false,
            base: [basemap({ source: "basemap", part: "base" })],
            children: s.children,
          }, { key: "body" }),
        ],
      })),
      ...scenes.map((s) => card({ text: s.caption, title: s.title, at: s.at, width: s.width }, {
        key: "caption",
        when: e(`scene == "${s.scene}"`),
      })),
      attribution({}, { key: "attribution", when: e('["men","women"].includes(scene)') }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene }, title: s.title, text: s.caption })),
  }),
});
