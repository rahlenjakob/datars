// The world stays home.
// Early March. Only China has locked down a region. The virus is already spreading in northern
// Italy, Iran and South Korea. 9 March. Italy extends its northern "red zone" to the whole country:
// sixty million people told to stay home, the first national order. Two weeks later, Spain, France,
// Belgium, Germany, the UK and most of their neighbours have followed. Sweden, famously, relies on
// recommendations instead. 25 March. India gives 1.3 billion people four hours' notice of a 21-day
// lockdown. South Africa, New Zealand, Colombia and Argentina lock down the same week. The US never
// has a national order; its states act one by one. End of April. The countries that went first have
// been home for six weeks or more. Many are beginning to plan their reopening.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { annotate, card, map, title } from "@datars/std";
import { health } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const lockdown = {
  series: [
    "CHN", "CHN", "CHN", "ITA", "ITA", "ITA", "ESP", "ESP", "ESP", "AUT", "AUT", "AUT", "CZE", "CZE", "CZE", "PER",
    "PER", "PER", "PHL", "PHL", "PHL", "FRA", "FRA", "FRA", "BEL", "BEL", "BEL", "MYS", "MYS", "MYS", "PRT", "PRT",
    "PRT", "ISR", "ISR", "ISR", "ARG", "ARG", "ARG", "MAR", "MAR", "MAR", "LKA", "LKA", "LKA", "RWA", "RWA", "RWA",
    "JOR", "JOR", "JOR", "TUN", "TUN", "TUN", "DEU", "DEU", "DEU", "GBR", "GBR", "GBR", "GRC", "GRC", "GRC", "NLD",
    "NLD", "NLD", "POL", "POL", "POL", "IND", "IND", "IND", "COL", "COL", "COL", "NZL", "NZL", "NZL", "BGD", "BGD",
    "BGD", "ZAF", "ZAF", "ZAF", "IRL", "IRL", "IRL", "HUN", "HUN", "HUN", "RUS", "RUS", "RUS", "NGA", "NGA", "NGA",
    "SGP", "SGP", "SGP", "SWE", "SWE", "NOR", "NOR", "DNK", "DNK", "FIN", "FIN", "USA", "USA", "CAN", "CAN", "MEX",
    "MEX", "BRA", "BRA", "JPN", "JPN", "KOR", "KOR", "AUS", "AUS", "TWN", "TWN",
  ],
  x: [
    "2020-01-22", "2020-01-23", "2020-04-30", "2020-03-08", "2020-03-09", "2020-04-30", "2020-03-13", "2020-03-14",
    "2020-04-30", "2020-03-15", "2020-03-16", "2020-04-30", "2020-03-15", "2020-03-16", "2020-04-30", "2020-03-15",
    "2020-03-16", "2020-04-30", "2020-03-15", "2020-03-16", "2020-04-30", "2020-03-16", "2020-03-17", "2020-04-30",
    "2020-03-17", "2020-03-18", "2020-04-30", "2020-03-17", "2020-03-18", "2020-04-30", "2020-03-18", "2020-03-19",
    "2020-04-30", "2020-03-18", "2020-03-19", "2020-04-30", "2020-03-19", "2020-03-20", "2020-04-30", "2020-03-19",
    "2020-03-20", "2020-04-30", "2020-03-19", "2020-03-20", "2020-04-30", "2020-03-20", "2020-03-21", "2020-04-30",
    "2020-03-20", "2020-03-21", "2020-04-30", "2020-03-21", "2020-03-22", "2020-04-30", "2020-03-21", "2020-03-22",
    "2020-04-30", "2020-03-22", "2020-03-23", "2020-04-30", "2020-03-22", "2020-03-23", "2020-04-30", "2020-03-22",
    "2020-03-23", "2020-04-30", "2020-03-24", "2020-03-25", "2020-04-30", "2020-03-24", "2020-03-25", "2020-04-30",
    "2020-03-24", "2020-03-25", "2020-04-30", "2020-03-25", "2020-03-26", "2020-04-30", "2020-03-25", "2020-03-26",
    "2020-04-30", "2020-03-26", "2020-03-27", "2020-04-30", "2020-03-26", "2020-03-27", "2020-04-30", "2020-03-27",
    "2020-03-28", "2020-04-30", "2020-03-29", "2020-03-30", "2020-04-30", "2020-03-29", "2020-03-30", "2020-04-30",
    "2020-04-06", "2020-04-07", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29",
    "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29",
    "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29", "2020-04-30", "2020-02-29",
    "2020-04-30", "2020-02-29", "2020-04-30",
  ],
  y: [
    0, 1, 99, 0, 1, 53, 0, 1, 48, 0, 1, 46, 0, 1, 46, 0, 1, 46, 0, 1, 46, 0, 1, 45, 0, 1, 44, 0, 1, 44, 0, 1, 43, 0, 1,
    43, 0, 1, 42, 0, 1, 42, 0, 1, 42, 0, 1, 41, 0, 1, 41, 0, 1, 40, 0, 1, 40, 0, 1, 39, 0, 1, 39, 0, 1, 39, 0, 1, 37, 0,
    1, 37, 0, 1, 37, 0, 1, 36, 0, 1, 36, 0, 1, 35, 0, 1, 35, 0, 1, 34, 0, 1, 32, 0, 1, 32, 0, 1, 24, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  ],
};

const scenes = [
  {
    scene: "march",
    at: "bottom-left",
    caption: "Early March: only China has sealed off a region — Hubei, since Jan. 23.",
    width: 300,
    box: [-130, 65, 150, -45],
    notes: [],
    data: "lockdown_1",
  },
  {
    scene: "italy",
    at: "right",
    caption: "March 9: Italy orders its whole population home — the first country to do so.",
    width: 260,
    box: [-12, 62, 32, 34],
    notes: [],
    data: "lockdown_2",
    selected: "focus",
  },
  {
    scene: "europe",
    at: "bottom-left",
    caption: "Two weeks later most of western Europe has followed. Sweden never issues an order.",
    width: 300,
    box: [-12, 66, 32, 34],
    notes: [{ text: "Sweden: no order", country: "SWE", lon: 15.5, lat: 62.5 }],
    data: "lockdown_3",
  },
  {
    scene: "world",
    at: "bottom-left",
    caption: "March 25: India — 1.3 billion people, four hours' notice. By April, a third of humanity is under some order.",
    width: 320,
    box: [-130, 65, 150, -45],
    notes: [{ text: "India", country: "IND" }, { text: "USA: state orders only", country: "USA" }],
    data: "lockdown_4",
  },
  {
    scene: "april",
    at: "bottom-left",
    caption: "End of April: the first to lock down have been home for six weeks or more (the scale tops out at 50 days).",
    width: 300,
    box: [-130, 65, 150, -45],
    notes: [],
    data: "lockdown_5",
  },
];

export default doc({
  id: "covid-lockdowns/spread",
  title: "The world stays home",
  size: [1000, 600],
  theme: health,
  data: { "geo:countries": data.atlas("countries"), lockdown: data.values(lockdown, { types: { x: "date" } }) },
  tables: {
    lockdown_1: table(
      "lockdown",
      op.interpolate("series", "x", "y", e("time")),
      op.aggregate(["series"], { value: ["first", "y"] }),
      op.derive("label", e("d.series")),
    ),
    lockdown_2: table(
      "lockdown",
      op.interpolate("series", "x", "y", e("time")),
      op.aggregate(["series"], { value: ["first", "y"] }),
      op.derive("label", e("d.series")),
    ),
    lockdown_3: table(
      "lockdown",
      op.interpolate("series", "x", "y", e("time")),
      op.aggregate(["series"], { value: ["first", "y"] }),
      op.derive("label", e("d.series")),
    ),
    lockdown_4: table(
      "lockdown",
      op.interpolate("series", "x", "y", e("time")),
      op.aggregate(["series"], { value: ["first", "y"] }),
      op.derive("label", e("d.series")),
    ),
    lockdown_5: table(
      "lockdown",
      op.interpolate("series", "x", "y", e("time")),
      op.aggregate(["series"], { value: ["first", "y"] }),
      op.derive("label", e("d.series")),
    ),
  },
  signals: { focus: signal.keyset(), scene: signal.str("march"), time: signal.num(18321) },
  keys: {
    ARG: { name: "Argentina" },
    AUS: { name: "Australia" },
    AUT: { name: "Austria" },
    BEL: { name: "Belgium" },
    BGD: { name: "Bangladesh" },
    BRA: { name: "Brazil" },
    CAN: { name: "Canada" },
    CHN: { name: "China" },
    COL: { name: "Colombia" },
    CZE: { name: "Czechia" },
    DEU: { name: "Germany" },
    DNK: { name: "Denmark" },
    ESP: { name: "Spain" },
    FIN: { name: "Finland" },
    FRA: { name: "France" },
    GBR: { name: "United Kingdom" },
    GRC: { name: "Greece" },
    HUN: { name: "Hungary" },
    IND: { name: "India" },
    IRL: { name: "Ireland" },
    ISR: { name: "Israel" },
    ITA: { name: "Italy" },
    JOR: { name: "Jordan" },
    JPN: { name: "Japan" },
    KOR: { name: "South Korea" },
    LKA: { name: "Sri Lanka" },
    MAR: { name: "Morocco" },
    MEX: { name: "Mexico" },
    MYS: { name: "Malaysia" },
    NGA: { name: "Nigeria" },
    NLD: { name: "Netherlands" },
    NOR: { name: "Norway" },
    NZL: { name: "New Zealand" },
    PER: { name: "Peru" },
    PHL: { name: "Philippines" },
    POL: { name: "Poland" },
    PRT: { name: "Portugal" },
    RUS: { name: "Russia" },
    RWA: { name: "Rwanda" },
    SGP: { name: "Singapore" },
    SWE: { name: "Sweden" },
    TUN: { name: "Tunisia" },
    TWN: { name: "Taiwan" },
    USA: { name: "United States of America" },
    ZAF: { name: "South Africa" },
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
          title({ text: "Days under a stay-at-home order, 2020" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            data: s.data,
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: true,
            camera: { fit: { bbox: geoBox(s.box) }, padding: 0 },
            format: ",.0f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (d.value == 0 ? `${(d.name ?? d.id)}: no national order` : `${(d.name ?? d.id)}: ${"" + format(d.value, ",.0f") + " days"} under a national stay-at-home order`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#f1e6cf 0 · #a9ddd2 1 · #1f8a7e 25 · #0b3440 50",
            selected: s.selected,
            children: [
              ...s.notes.map((n) => annotate({
                x: e("lon" in n ? `geo.x(${n.lon}, ${n.lat})` : `geo.cx("geo:countries", "${n.country}")`),
                y: e("lat" in n ? `geo.y(${n.lon}, ${n.lat})` : `geo.cy("geo:countries", "${n.country}")`),
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
      ...scenes.map((s) => card({ text: s.caption, at: s.at, width: s.width }, {
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
      step("march", {
        set: { focus: [], scene: "march", time: 18327 },
        text: "Early March: only [China](CHN) has sealed off a region — Hubei, since Jan. 23.",
      }),
      step("italy", {
        set: { focus: ["ITA"], scene: "italy", time: 18331 },
        text: "March 9: [Italy](ITA) orders its whole population home — the first country to do so.",
        anchor: "ITA",
      }),
      step("europe", {
        set: { focus: [], scene: "europe", time: 18345 },
        text: "Two weeks later most of western Europe has followed. [Sweden](SWE) never issues an order.",
      }),
      step("world", {
        set: { focus: [], scene: "world", time: 18353 },
        text: "March 25: [India](IND) — 1.3 billion people, four hours' notice. By April, a third of humanity is under some order.",
      }),
      step("april", {
        set: { focus: [], scene: "april", time: 18382 },
        text: "End of April: the first to lock down have been home for six weeks or more (the scale tops out at 50 days).",
      }),
    ],
    drivers: [{ scroll: "trigger" }],
  }),
});
