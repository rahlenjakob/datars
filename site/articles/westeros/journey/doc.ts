// The Kingsroad.
// The Kingsroad, from Winterfell to King's Landing (hand-drawn). Step through with the buttons in
// the cards.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { card, geoLines, map, symbols, title } from "@datars/std";
import { fantasy } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const scenes = [
  {
    scene: "winterfell",
    at: "right",
    kicker: "Season 1",
    caption: "The king rides north — and asks Ned Stark to come south as his Hand.",
    title: "Winterfell",
    width: 250,
    box: [-20.128308275549603, 53.007080614567556, 18.128308275549603, 35.69500459498711],
  },
  {
    scene: "twins",
    at: "right",
    caption: "Out of the North through the marshes; the Twins guard the crossing west of the road.",
    title: "The Neck",
    width: 250,
    box: [-21.128308275549603, 47.4332964342392, 17.128308275549603, 28.30526540000503],
  },
  {
    scene: "trident",
    at: "right",
    caption: "Across the rivers of the Trident, past Riverrun's country.",
    title: "The Riverlands",
    width: 250,
    box: [-19.128308275549603, 43.52084167229596, 19.128308275549603, 23.280829772881784],
  },
  {
    scene: "capital",
    at: "left",
    caption: "The capital, on the Blackwater — where the game is played.",
    title: "King's Landing",
    width: 250,
    box: [-11.652159544181615, 37.425151052229694, 21.652159544181615, 18.752220576640738],
  },
  {
    scene: "whole",
    at: "bottom-left",
    caption: "The whole road, north to south: most of a continent.",
    width: 260,
    box: [-8, 48, 9, 25],
  },
];

export default doc({
  id: "westeros/journey",
  title: "The Kingsroad",
  size: [1000, 640],
  theme: fantasy,
  data: {
    "geo:roads": data.url("roads.geojson", { id: "id" }),
    "geo:seats": data.url("seats.geojson", { id: "id" }),
    "geo:westeros": data.url("westeros.geojson", { id: "id" }),
    realm: data.values({
      label: [
        "beyond-the-wall", "the-north", "the-vale", "the-riverlands", "iron-islands", "the-westerlands",
        "the-crownlands", "the-reach", "the-stormlands", "dorne",
      ],
      value: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    }, { key: "label" }),
  },
  signals: { scene: signal.str("winterfell") },
  keys: {
    "Balon Greyjoy": { color: "#2f3b3b" },
    "Joffrey Baratheon": { color: "#9b1b1b" },
    "Renly Baratheon": { color: "#4f7f3a" },
    "Robb Stark": { color: "#7d8a96" },
    "Stannis Baratheon": { color: "#c9a227" },
    "beyond-the-wall": { name: "Beyond the Wall", color: "#c9d1d6" },
    dorne: { name: "Dorne", color: "#d2691e" },
    "iron-islands": { name: "The Iron Islands", color: "#2f3b3b" },
    "the-crownlands": { name: "The Crownlands", color: "#4a3550" },
    "the-north": { name: "The North", color: "#7d8a96" },
    "the-reach": { name: "The Reach", color: "#4f7f3a" },
    "the-riverlands": { name: "The Riverlands", color: "#3f5f9e" },
    "the-stormlands": { name: "The Stormlands", color: "#c9a227" },
    "the-vale": { name: "The Vale", color: "#6f9bc8" },
    "the-westerlands": { name: "The Westerlands", color: "#9b1b1b" },
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
          title({ text: "Down the Kingsroad" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:westeros",
            data: "realm",
            key: "label",
            value: "value",
            colorType: "categorical",
            backdrop: false,
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            format: ".1~f",
            label: e("d.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}`)"),
            padding: 0,
            projection: "web-mercator",
            sea: true,
            children: [
              geoLines({ source: "geo:roads", ink: "$ink-2", width: 1.2 }),
              symbols({ source: "geo:seats", data: "geo:seats", key: "id", fill: "$ink", r: 3 }),
            ],
          }, { key: "body" }),
        ],
      })),
      ...scenes.map((s) => card({ text: s.caption, title: s.title, at: s.at, kicker: s.kicker, width: s.width }, {
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
      step("winterfell", {
        set: { scene: "winterfell" },
        title: "Winterfell",
        text: "The king rides north — and asks Ned Stark to come south as his Hand.",
        anchor: "winterfell",
      }),
      step("twins", {
        set: { scene: "twins" },
        title: "The Neck",
        text: "Out of the North through the marshes; the Twins guard the crossing west of the road.",
        anchor: "the-twins",
      }),
      step("trident", {
        set: { scene: "trident" },
        title: "The Riverlands",
        text: "Across the rivers of the Trident, past Riverrun's country.",
        anchor: "riverrun",
      }),
      step("capital", {
        set: { scene: "capital" },
        title: "King's Landing",
        text: "The capital, on the Blackwater — where the game is played.",
        anchor: "kings-landing",
      }),
      step("whole", { set: { scene: "whole" }, text: "The whole road, north to south: most of a continent." }),
    ],
  }),
});
