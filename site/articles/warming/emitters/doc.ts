// Emitters.
// Fossil CO₂ emissions in 2022: total (circle area ∝ emissions), then per person, then ranked.
// Global Carbon Project, rounded.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, card, map, plot, symbols, title } from "@datars/std";
import { science } from "../theme";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const emissions = {
  label: [
    "CHN", "USA", "IND", "RUS", "JPN", "IRN", "IDN", "SAU", "DEU", "KOR", "CAN", "MEX", "BRA", "TUR", "ZAF", "AUS",
    "GBR", "ITA", "POL", "FRA", "VNM", "THA", "KAZ", "EGY", "MYS", "ESP", "PAK", "ARE", "ARG", "NGA",
  ],
  value: [
    11.4, 5.1, 2.8, 1.7, 1.1, 0.75, 0.73, 0.69, 0.67, 0.6, 0.55, 0.49, 0.48, 0.45, 0.4, 0.39, 0.33, 0.32, 0.32, 0.3,
    0.3, 0.28, 0.26, 0.25, 0.25, 0.24, 0.24, 0.23, 0.19, 0.13,
  ],
};

const per_capita = {
  label: [
    "QAT", "SAU", "ARE", "AUS", "USA", "CAN", "KAZ", "KOR", "RUS", "JPN", "CHN", "DEU", "POL", "IRN", "ZAF", "MYS",
    "ITA", "GBR", "FRA", "ESP", "TUR", "MEX", "THA", "ARG", "SWE", "VNM", "EGY", "BRA", "IDN", "IND", "PAK", "NGA",
  ],
  value: [
    37.6, 18.2, 22.9, 15, 14.9, 14.3, 13, 11.6, 11.4, 8.5, 8, 8, 8.1, 8.5, 6.7, 7.6, 5.4, 4.7, 4.6, 5.1, 5.3, 3.8, 3.8,
    4.2, 3.6, 3.5, 2.3, 2.2, 2.6, 2, 1, 0.6,
  ],
};

const captions = [
  { scene: "total", at: "bottom-left", caption: "China emits more than the next three combined.", width: 270 },
  {
    scene: "percap",
    at: "bottom-left",
    caption: "Per person the picture flips: the Gulf states, Australia and North America lead; India is at 2 tonnes.",
    width: 290,
  },
  { scene: "ranked", at: "bottom-right", caption: "Qatar, at 37.6 tonnes a person, is far out on its own. China, by far the largest emitter in total, is 13th here.", width: 260 },
];

export default doc({
  id: "warming/emitters",
  title: "Emitters",
  size: [1000, 600],
  theme: science,
  data: {
    emissions: data.values(emissions, { key: "label" }),
    "geo:countries": data.atlas("countries"),
    per_capita: data.values(per_capita, { key: "label" }),
  },
  tables: { per_capita_1: table("per_capita", op.sort(["value", "desc"]), op.top(15, "value")) },
  signals: { scene: signal.str("total") },
  keys: {
    ARE: { name: "United Arab Emirates", color: "$categorical[2]" },
    ARG: { name: "Argentina", color: "$categorical[3]" },
    AUS: { name: "Australia", color: "$categorical[3]" },
    BRA: { name: "Brazil", color: "$categorical[7]" },
    CAN: { name: "Canada", color: "$categorical[5]" },
    CHN: { name: "China", color: "$categorical[0]" },
    DEU: { name: "Germany", color: "$categorical[1]" },
    EGY: { name: "Egypt", color: "$categorical[6]" },
    ESP: { name: "Spain", color: "$categorical[9]" },
    FRA: { name: "France", color: "$categorical[8]" },
    GBR: { name: "United Kingdom", color: "$categorical[7]" },
    IDN: { name: "Indonesia", color: "$categorical[8]" },
    IND: { name: "India", color: "$categorical[9]" },
    IRN: { name: "Iran", color: "$categorical[3]" },
    ITA: { name: "Italy", color: "$categorical[6]" },
    JPN: { name: "Japan", color: "$categorical[9]" },
    KAZ: { name: "Kazakhstan", color: "$categorical[6]" },
    KOR: { name: "South Korea", color: "$categorical[7]" },
    MEX: { name: "Mexico", color: "$categorical[1]" },
    MYS: { name: "Malaysia", color: "$categorical[5]" },
    NGA: { name: "Nigeria", color: "$categorical[1]" },
    PAK: { name: "Pakistan", color: "$categorical[0]" },
    POL: { name: "Poland", color: "$categorical[2]" },
    QAT: { name: "Qatar", color: "$categorical[0]" },
    RUS: { name: "Russia", color: "$categorical[8]" },
    SAU: { name: "Saudi Arabia", color: "$categorical[1]" },
    SWE: { name: "Sweden", color: "$categorical[4]" },
    THA: { name: "Thailand", color: "$categorical[2]" },
    TUR: { name: "Turkey", color: "$categorical[0]" },
    USA: { name: "United States of America", color: "$categorical[4]" },
    VNM: { name: "Vietnam", color: "$categorical[5]" },
    ZAF: { name: "South Africa", color: "$categorical[4]" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      group({
        key: "chart",
        when: e('scene == "total"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "Fossil CO₂ emissions, 2022 (billion tonnes)" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            camera: { fit: { bbox: geoBox([-130, 65, 150, -45]) }, padding: 0 },
            format: ",.2f",
            padding: 0,
            projection: "web-mercator",
            children: [
              symbols({
                source: "geo:countries",
                data: "emissions",
                key: "label",
                value: "value",
                format: ",.2f",
                label: e(
                  'd.value == null ? `${key.name(d.label)}` : (`${key.name(d.label)}: ${"" + format(d.value, ",.2f") + ""} bn tonnes`)',
                ),
                max: 56,
                stops: "#fee0b6 0.13 · #e08214 5.765 · #7f3b08 11.4",
              }),
            ],
          }, { key: "body" }),
        ],
      }),
      group({
        key: "chart",
        when: e('scene == "percap"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "Fossil CO₂ emissions per person, 2022 (tonnes)" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            data: "per_capita",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: true,
            camera: { fit: { bbox: geoBox([-130, 65, 150, -45]) }, padding: 0 },
            format: ",.1f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.1f") + ""} t per person`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#fff5eb 0.6 · #fd8d3c 19.1 · #7f2704 37.6",
            children: [],
          }, { key: "body" }),
        ],
      }),
      plot({
        data: "per_capita_1",
        x: "value",
        y: "label",
        color: "value",
        colorType: "piecewise",
        stops: "#fff5eb 0.6 · #fd8d3c 19.1 · #7f2704 37.6",
        xType: "linear",
        yType: "band",
        title: "Tonnes of CO₂ per person, 2022: the top 15 of the 32 countries shown",
        format: ",.1~f",
        padding: 0.05,
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "ranked"') }),
      ...captions.map((s) => card({ text: s.caption, at: s.at, width: s.width }, {
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
      step("total", { set: { scene: "total" }, text: "China emits more than the next three combined." }),
      step("percap", {
        set: { scene: "percap" },
        text: "Per person the picture flips: the Gulf states, Australia and North America lead; [India](IND) is at 2 tonnes.",
      }),
      step("ranked", { set: { scene: "ranked" }, text: "Qatar, at 37.6 tonnes a person, is far out on its own. China, by far the largest emitter in total, is 13th here." }),
    ],
  }),
});
