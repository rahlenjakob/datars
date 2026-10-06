// Europe, country by country: click a country to drill into it (chapters).
// Click-to-drill chapters. 2024 GDP: IMF WEO, Oct 2024, rounded; the 2004–2024 series is
// approximate.
// Figures as noted in the article (many are rounded or illustrative).
import { doc, chapter, data, e, group, motion, op, signal, step, story, table, text } from "@datars/sdk";
import { bar, card, line, map, plot, title } from "@datars/std";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const eu_gdp = {
  label: [
    "DEU", "FRA", "ITA", "ESP", "NLD", "POL", "BEL", "SWE", "IRL", "AUT", "DNK", "FIN", "ROU", "CZE", "PRT", "GRC",
    "HUN", "SVK", "BGR", "HRV", "LTU", "SVN", "LVA", "EST", "LUX", "CYP", "MLT",
  ],
  value: [
    4.71, 3.17, 2.38, 1.73, 1.23, 0.91, 0.67, 0.61, 0.55, 0.53, 0.43, 0.31, 0.38, 0.35, 0.31, 0.25, 0.22, 0.14, 0.11,
    0.09, 0.08, 0.07, 0.04, 0.04, 0.09, 0.03, 0.02,
  ],
};

const eu_gdp_history = {
  series: [
    "DEU", "DEU", "DEU", "DEU", "DEU", "FRA", "FRA", "FRA", "FRA", "FRA", "ITA", "ITA", "ITA", "ITA", "ITA", "ESP",
    "ESP", "ESP", "ESP", "ESP", "NLD", "NLD", "NLD", "NLD", "NLD", "POL", "POL", "POL", "POL", "POL", "BEL", "BEL",
    "BEL", "BEL", "BEL", "SWE", "SWE", "SWE", "SWE", "SWE", "IRL", "IRL", "IRL", "IRL", "IRL", "AUT", "AUT", "AUT",
    "AUT", "AUT", "DNK", "DNK", "DNK", "DNK", "DNK", "FIN", "FIN", "FIN", "FIN", "FIN", "ROU", "ROU", "ROU", "ROU",
    "ROU", "CZE", "CZE", "CZE", "CZE", "CZE", "PRT", "PRT", "PRT", "PRT", "PRT", "GRC", "GRC", "GRC", "GRC", "GRC",
    "HUN", "HUN", "HUN", "HUN", "HUN", "SVK", "SVK", "SVK", "SVK", "SVK", "BGR", "BGR", "BGR", "BGR", "BGR", "HRV",
    "HRV", "HRV", "HRV", "HRV", "LTU", "LTU", "LTU", "LTU", "LTU", "SVN", "SVN", "SVN", "SVN", "SVN", "LVA", "LVA",
    "LVA", "LVA", "LVA", "EST", "EST", "EST", "EST", "EST", "LUX", "LUX", "LUX", "LUX", "LUX", "CYP", "CYP", "CYP",
    "CYP", "CYP", "MLT", "MLT", "MLT", "MLT", "MLT",
  ],
  x: Array.from({ length: 135 }, (_, i) => 2004 + (i % 5) * 5),
  y: [
    2.85, 3.42, 3.89, 3.89, 4.71, 2.12, 2.69, 2.86, 2.73, 3.17, 1.8, 2.19, 2.16, 2.01, 2.38, 1.07, 1.49, 1.37, 1.39,
    1.73, 0.66, 0.86, 0.88, 0.91, 1.23, 0.26, 0.44, 0.54, 0.6, 0.91, 0.37, 0.48, 0.54, 0.54, 0.67, 0.39, 0.44, 0.58,
    0.53, 0.61, 0.19, 0.24, 0.26, 0.4, 0.55, 0.3, 0.4, 0.44, 0.45, 0.53, 0.25, 0.32, 0.35, 0.35, 0.43, 0.19, 0.25, 0.27,
    0.27, 0.31, 0.08, 0.17, 0.2, 0.25, 0.38, 0.12, 0.21, 0.21, 0.25, 0.35, 0.19, 0.24, 0.23, 0.24, 0.31, 0.24, 0.35,
    0.24, 0.21, 0.25, 0.1, 0.13, 0.14, 0.16, 0.22, 0.04, 0.09, 0.1, 0.11, 0.14, 0.03, 0.05, 0.06, 0.07, 0.11, 0.04,
    0.06, 0.06, 0.06, 0.09, 0.02, 0.04, 0.05, 0.05, 0.08, 0.03, 0.05, 0.05, 0.05, 0.07, 0.01, 0.03, 0.03, 0.03, 0.04,
    0.01, 0.02, 0.03, 0.03, 0.04, 0.03, 0.05, 0.07, 0.07, 0.09, 0.02, 0.03, 0.02, 0.03, 0.03, 0.01, 0.01, 0.01, 0.02,
    0.02,
  ],
};

const captions = [
  {
    scene: "europe",
    at: "bottom-left",
    kicker: "Nominal GDP, $ trillion",
    caption: "Click any of the twenty-seven to follow it: where it ranks, and how it got there.",
    title: "Pick a country",
    width: 320,
  },
  {
    scene: "ranking",
    at: "bottom-right",
    caption: "Or pick one from the ranking — the bars are clickable too.",
    width: 280,
  },
  {
    scene: "country#0",
    at: "right",
    kicker: "Country",
    caption: "{c.name} produced ${c.value} trillion in 2024.",
    title: "{c.name}",
    width: 260,
  },
  {
    scene: "country#1",
    at: "bottom-right",
    caption: "Lifted off the map and into line: {c.name} against the other twenty-six.",
    width: 280,
  },
  {
    scene: "country#2",
    at: "bottom-right",
    caption: "Twenty years of {c.name}, in current dollars — so exchange rates move the line as much as growth does.",
    width: 300,
  },
];

const charts = [
  {
    text: e("`${key.name(c)}`"),
    when: "0",
    children: map({
      source: "geo:countries",
      data: "eu_gdp",
      key: "label",
      value: "value",
      colorType: "piecewise",
      backdrop: true,
      camera: { fit: { keys: "=c" }, padding: 16 },
      format: ".1~f",
      legend: true,
      padding: 0,
      projection: "web-mercator",
      selected: "focus",
      stops: "#fef3c7 0.02 · #f59e0b 2.365 · #7c2d12 4.71",
      children: [],
    }, { key: "body" }),
  },
  {
    text: e("`${key.name(c)} among the EU's economies ($ trillion)`"),
    when: "1",
    children: plot({
      data: "eu_gdp_2",
      x: "value",
      y: "label",
      color: "label",
      xType: "linear",
      yType: "band",
      format: ",.1~f",
      padding: 0.2,
      children: [bar({ format: ",.1~f", labels: true, selected: "focus" })],
    }, { key: "body" }),
  },
  {
    text: e("`${key.name(c)}: nominal GDP, 2004–2024 ($ trillion)`"),
    when: "2",
    children: plot({
      data: "eu_gdp_history_3",
      x: "x",
      y: "y",
      color: "series",
      xType: "linear",
      format: ",.1~f",
      padding: 0.2,
      zero: false,
      children: [line({ labels: true, selected: "focus" })],
    }, { key: "body" }),
  },
];

export default doc({
  id: "europe-drill",
  title: "Europe, country by country",
  size: [1000, 620],
  data: {
    eu_gdp: data.values(eu_gdp, { key: "label" }),
    eu_gdp_history: data.values(eu_gdp_history),
    "geo:countries": data.atlas("countries"),
  },
  tables: {
    eu_gdp_1: table("eu_gdp", op.sort(["value", "desc"])),
    eu_gdp_2: table("eu_gdp", op.sort(["value", "desc"])),
    eu_gdp_history_3: table("eu_gdp_history", op.filter(e("d.series == c"))),
  },
  signals: { scene: signal.str("europe") },
  keys: {
    AUT: { name: "Austria", color: "$categorical[9]" },
    BEL: { name: "Belgium", color: "$categorical[6]" },
    BGR: { name: "Bulgaria", color: "$categorical[8]" },
    CYP: { name: "Cyprus", color: "$categorical[5]" },
    CZE: { name: "Czechia", color: "$categorical[3]" },
    DEU: { name: "Germany", color: "$categorical[0]" },
    DNK: { name: "Denmark", color: "$categorical[0]" },
    ESP: { name: "Spain", color: "$categorical[3]" },
    EST: { name: "Estonia", color: "$categorical[3]" },
    FIN: { name: "Finland", color: "$categorical[1]" },
    FRA: { name: "France", color: "$categorical[1]" },
    GRC: { name: "Greece", color: "$categorical[5]" },
    HRV: { name: "Croatia", color: "$categorical[9]" },
    HUN: { name: "Hungary", color: "$categorical[6]" },
    IRL: { name: "Ireland", color: "$categorical[8]" },
    ITA: { name: "Italy", color: "$categorical[2]" },
    LTU: { name: "Lithuania", color: "$categorical[0]" },
    LUX: { name: "Luxembourg", color: "$categorical[4]" },
    LVA: { name: "Latvia", color: "$categorical[2]" },
    MLT: { name: "Malta", color: "$categorical[6]" },
    NLD: { name: "Netherlands", color: "$categorical[4]" },
    POL: { name: "Poland", color: "$categorical[5]" },
    PRT: { name: "Portugal", color: "$categorical[4]" },
    ROU: { name: "Romania", color: "$categorical[2]" },
    SVK: { name: "Slovakia", color: "$categorical[7]" },
    SVN: { name: "Slovenia", color: "$categorical[1]" },
    SWE: { name: "Sweden", color: "$categorical[7]" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      group({
        key: "chart",
        when: e('scene == "europe"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "EU economies, 2024" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            data: "eu_gdp",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: true,
            camera: { fit: { bbox: geoBox([-11, 66, 32, 35]) }, padding: 0 },
            chapter: "country",
            format: ".1~f",
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#fef3c7 0.02 · #f59e0b 2.365 · #7c2d12 4.71",
            children: [],
          }, { key: "body" }),
        ],
      }),
      plot({
        data: "eu_gdp_1",
        x: "value",
        y: "label",
        color: "label",
        xType: "linear",
        yType: "band",
        title: "All twenty-seven, ranked ($ trillion)",
        format: ",.1~f",
        padding: 0.2,
        children: [bar({ chapter: "country", format: ",.1~f", labels: true, selected: "focus" })],
      }, { key: "chart", when: e('scene == "ranking"') }),
      ...charts.map((s) => group({
        key: "chart",
        when: e(`scene == "country#${s.when}"`),
        layout: { type: "rows", gap: 8 },
        children: [
          text(
            s.text,
            [0, 0],
            {
              key: "title",
              size: { h: "auto" },
              semantics: { role: "title" },
              style: { baseline: "top", font: "font.title", ink: "$ink", size: "$size.title" },
            },
          ),
          s.children,
        ],
      })),
      ...captions.map((s) => card({ text: s.caption, title: s.title, at: s.at, kicker: s.kicker, width: s.width }, {
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
      step("europe", {
        set: { scene: "europe" },
        title: "Pick a country",
        text: "Click any of the twenty-seven to follow it: where it ranks, and how it got there.",
      }),
      step("ranking", { set: { scene: "ranking" }, text: "Or pick one from the ranking — the bars are clickable too." }),
    ],
    chapters: {
      country: chapter("c", [
        step("{c}-zoom", {
          set: { focus: "{c}", scene: "country#0" },
          title: "{c.name}",
          text: "[{c.name}]({c}) produced **${c.value} trillion** in 2024.",
          anchor: "{c}",
        }),
        step("{c}-rank", {
          set: { focus: "{c}", scene: "country#1" },
          text: "Lifted off the map and into line: [{c.name}]({c}) against the other twenty-six.",
        }),
        step("{c}-trend", {
          set: { focus: "", scene: "country#2" },
          text: "Twenty years of [{c.name}]({c}), in current dollars — so exchange rates move the line as much as growth does.",
        }),
      ]),
    },
  }),
});
