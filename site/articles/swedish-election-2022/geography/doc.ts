// SD by county.
// Each county, shaded by the Sweden Democrats' share of the vote (darker is higher); ranked, nearly
// twice the share in Blekinge as in Västerbotten; then the south, Blekinge, Skåne and Kalmar. Shares:
// the Election Authority's constituency vote counts, summed by county.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, card, map, plot, title } from "@datars/std";
import { broadsheet } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const captions = [
  {
    scene: "map",
    at: "auto",
    caption: "Strongest in the south; weakest in Stockholm, on Gotland and in Västerbotten.",
    width: 280,
  },
  {
    scene: "ranked",
    at: "auto",
    caption: "Ranked: Blekinge at the top, Stockholm and Västerbotten at the bottom.",
    width: 260,
  },
  { scene: "south", at: "right", caption: "Blekinge, Skåne and Kalmar: three of the party's four strongest counties.", width: 260 },
];

export default doc({
  id: "swedish-election-2022/geography",
  title: "SD by county",
  size: [960, 620],
  theme: broadsheet,
  data: {
    "geo:admin1": data.url("admin1.geojson", { id: "iso_3166_2" }),
    "geo:countries": data.atlas("countries"),
    sd_county: data.values({
      label: [
        "SE-K", "SE-H", "SE-M", "SE-D", "SE-G", "SE-X", "SE-W", "SE-U", "SE-F", "SE-N", "SE-T", "SE-E", "SE-Y", "SE-S",
        "SE-O", "SE-Z", "SE-BD", "SE-I", "SE-C", "SE-AB", "SE-AC",
      ],
      value: [
        28.5, 24.5, 25.1, 23.0, 23.6, 24.1, 25.7, 23.7, 23.3, 22.6, 22.1, 21.2, 20.7, 22.8, 20.6, 20.1, 20.3, 15.7, 18.2, 14.6, 14.5,
      ],
    }, { key: "label" }),
  },
  tables: { sd_county_1: table("sd_county", op.sort(["value", "desc"]), op.top(21, "value")) },
  signals: { focus: signal.keyset(), scene: signal.str("map") },
  keys: {
    "SE-AB": { name: "Stockholm" },
    "SE-AC": { name: "Västerbotten" },
    "SE-BD": { name: "Norrbotten" },
    "SE-C": { name: "Uppsala" },
    "SE-D": { name: "Södermanland" },
    "SE-E": { name: "Östergötland" },
    "SE-F": { name: "Jönköping" },
    "SE-G": { name: "Kronoberg" },
    "SE-H": { name: "Kalmar" },
    "SE-I": { name: "Gotland" },
    "SE-K": { name: "Blekinge" },
    "SE-M": { name: "Skåne" },
    "SE-N": { name: "Halland" },
    "SE-O": { name: "Västra Götaland" },
    "SE-S": { name: "Värmland" },
    "SE-T": { name: "Örebro" },
    "SE-U": { name: "Västmanland" },
    "SE-W": { name: "Dalarna" },
    "SE-X": { name: "Gävleborg" },
    "SE-Y": { name: "Västernorrland" },
    "SE-Z": { name: "Jämtland" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      group({
        key: "chart",
        when: e('scene == "map"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "Sweden Democrats' share by county, 2022" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:admin1",
            data: "sd_county",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: false,
            camera: {
              fit: {
                keys: [
                  "SE-K", "SE-H", "SE-M", "SE-D", "SE-G", "SE-X", "SE-W", "SE-U", "SE-F", "SE-N", "SE-T", "SE-E",
                  "SE-Y", "SE-S", "SE-O", "SE-Z", "SE-BD", "SE-I", "SE-C", "SE-AB", "SE-AC",
                ],
              },
              padding: 16,
            },
            format: ",.1f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.1f") + "%"} voted SD`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#efeab4 14.5 · #d2cc00 21.5 · #7a6f00 28.5",
            under: "geo:countries",
            children: [],
          }, { key: "body" }),
        ],
      }),
      plot({
        data: "sd_county_1",
        x: "value",
        y: "label",
        color: "value",
        xType: "linear",
        yType: "band",
        colorType: "piecewise",
        stops: "#efeab4 14.5 · #d2cc00 21.5 · #7a6f00 28.5",
        title: "Sweden Democrats' share of the vote by county, percent",
        format: ",.1~f",
        padding: 0.25,
        children: [bar({ format: ",.1~f", labels: true, selected: "focus" })],
      }, { key: "chart", when: e('scene == "ranked"') }),
      group({
        key: "chart",
        when: e('scene == "south"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "The south-east" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:admin1",
            data: "sd_county",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: false,
            camera: { fit: { bbox: geoBox([11.5, 58.2, 17.5, 55.2]) }, padding: 0 },
            format: ",.1f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.1f") + "%"} voted SD`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            selected: "focus",
            stops: "#efeab4 14.5 · #d2cc00 21.5 · #7a6f00 28.5",
            under: "geo:countries",
            children: [],
          }, { key: "body" }),
        ],
      }),
      ...captions.map((s) => card({ text: s.caption, at: s.at, kicker: s.kicker, width: s.width }, {
        key: "caption",
        when: e(`scene == "${s.scene}"`),
      })),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("map", {
        set: { focus: [], scene: "map" },
        text: "Strongest in the south; weakest in Stockholm, on Gotland and in Västerbotten.",
      }),
      step("ranked", {
        set: { focus: [], scene: "ranked" },
        text: "Ranked: [Blekinge](SE-K) at the top, [Stockholm](SE-AB) and [Västerbotten](SE-AC) at the bottom.",
      }),
      step("south", {
        set: { focus: ["SE-K", "SE-M", "SE-H"], scene: "south" },
        text: "[Blekinge](SE-K), [Skåne](SE-M) and [Kalmar](SE-H): three of the party's four strongest counties.",
        anchor: "SE-K",
      }),
    ],
  }),
});
