// The realm.
// Westeros, colored by the great house that rules each region. The North, the largest of the
// kingdoms, ruled from Winterfell by House Stark. The Wall closes the North off from the wild lands
// beyond it, held by the Night's Watch from Castle Black. King's Landing, the capital, in the
// Crownlands — with Dragonstone off the coast. Dorne, the southernmost kingdom, ruled by House
// Martell from Sunspear. The iron and the gold: the raiders of the Iron Islands, and the Lannisters
// of Casterly Rock.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, map, symbols, title } from "@datars/std";
import { fantasy } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const regions = [
  { id: "beyond-the-wall", name: "Beyond the Wall" }, { id: "the-north", name: "The North" }, { id: "the-vale", name: "The Vale" },
  { id: "the-riverlands", name: "Riverlands" }, { id: "iron-islands", name: "Iron Islands" }, { id: "the-westerlands", name: "Westerlands" },
  { id: "the-crownlands", name: "Crownlands" }, { id: "the-reach", name: "The Reach" }, { id: "the-stormlands", name: "Stormlands" }, { id: "dorne", name: "Dorne" },
];

const charts = [
  { scene: "all", title: "Westeros", bbox: geoBox([-13, 57, 11, 7]), padding: 0, notes: [] },
  {
    scene: "north",
    title: "The North",
    keys: ["the-north"],
    padding: 16,
    notes: [{ text: "Winterfell", seat: "winterfell" }],
    selected: "focus",
  },
  {
    scene: "wall",
    title: "The Wall",
    bbox: geoBox([-10, 57, 10, 48]),
    padding: 0,
    notes: [{ text: "Castle Black", seat: "castle-black" }],
    selected: "focus",
  },
  {
    scene: "capital",
    title: "King's Landing",
    bbox: geoBox([0, 34, 11, 24]),
    padding: 0,
    notes: [{ text: "King's Landing", seat: "kings-landing" }, { text: "Dragonstone", seat: "dragonstone" }],
    selected: "focus",
  },
  {
    scene: "dorne",
    title: "Dorne",
    keys: ["dorne"],
    padding: 16,
    notes: [{ text: "Sunspear", seat: "sunspear" }],
    selected: "focus",
  },
  {
    scene: "west",
    title: "The iron and the gold",
    bbox: geoBox([-13, 38, -2, 26]),
    padding: 0,
    notes: [{ text: "Pyke", seat: "pyke" }, { text: "Casterly Rock", seat: "casterly-rock" }],
    selected: "focus",
  },
];

export default doc({
  id: "westeros/realm",
  title: "The realm",
  size: [1000, 700],
  theme: fantasy,
  data: {
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
  signals: { focus: signal.keyset(), scene: signal.str("all") },
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
      ...charts.map((s) => group({
        key: "chart",
        when: e(`scene == "${s.scene}"`),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: s.title }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:westeros",
            data: "realm",
            key: "label",
            value: "value",
            colorType: "categorical",
            backdrop: false,
            camera: { fit: { bbox: s.bbox, keys: s.keys }, padding: s.padding },
            format: ".1~f",
            label: e("d.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}`)"),
            padding: 0,
            projection: "web-mercator",
            sea: true,
            selected: s.selected,
            children: [
              symbols({ source: "geo:seats", data: "geo:seats", key: "id", fill: "$ink", r: 3 }),
              ...(s.scene == "all" ? regions.map((r) => annotate({ x: e(`geo.cx("geo:westeros", "${r.id}")`), y: e(`geo.cy("geo:westeros", "${r.id}")`), text: r.name, connector: "none", dot: false, dx: 0, dy: 14, head: false, pin: true, width: 90 })) : []),
              ...s.notes.map((n) => annotate({
                x: e(`geo.cx("geo:seats", "${n.seat}")`),
                y: e(`geo.cy("geo:seats", "${n.seat}")`),
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
    steps: [
      step("all", { set: { focus: [], scene: "all" } }),
      step("north", { set: { focus: ["the-north"], scene: "north" } }),
      step("wall", { set: { focus: ["beyond-the-wall"], scene: "wall" } }),
      step("capital", { set: { focus: ["the-crownlands"], scene: "capital" } }),
      step("dorne", { set: { focus: ["dorne"], scene: "dorne" } }),
      step("west", { set: { focus: ["iron-islands", "the-westerlands"], scene: "west" } }),
    ],
    drivers: [{ scroll: "trigger" }],
  }),
});
