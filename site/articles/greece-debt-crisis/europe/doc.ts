// Government debt across the EU in 2025, as a share of each economy: on the map, ranked, then Greece.
// Data: Eurostat, gov_10dd_edpt1 (../data/eurostat.json).
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, card, map, plot, title } from "@datars/std";
import { financial } from "../theme";
import src from "../data/eurostat.json";

/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */
const geoBox = ([w, n, east, s]: number[]) => [
  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),
];

const eu_debt = src.eu;

const captions = [
  {
    scene: "map",
    at: "auto",
    caption: "The heaviest debts are in the south and west of the union.",
    width: 300,
  },
  {
    scene: "ranked",
    at: "auto",
    caption: "Greece still owes the most, ahead of Italy, 15 years after its first bailout.",
    width: 260,
  },
  { scene: "greece", at: "right", caption: "Greece: debt worth nearly a year and a half of everything the economy produces.", width: 260 },
];

export default doc({
  id: "greece-debt-crisis/europe",
  title: "EU debt",
  size: [960, 620],
  theme: financial,
  data: { eu_debt: data.values(eu_debt, { key: "label" }), "geo:countries": data.atlas("countries") },
  tables: { eu_debt_1: table("eu_debt", op.sort(["value", "desc"]), op.top(12, "value")) },
  signals: { focus: signal.keyset(), scene: signal.str("map") },
  keys: {
    AUT: { name: "Austria" },
    BEL: { name: "Belgium" },
    BGR: { name: "Bulgaria" },
    CYP: { name: "Cyprus" },
    CZE: { name: "Czechia" },
    DEU: { name: "Germany" },
    DNK: { name: "Denmark" },
    Debt: { color: "#b4361c" },
    ESP: { name: "Spain" },
    EST: { name: "Estonia" },
    Everyone: { color: "#1d4ed8" },
    FIN: { name: "Finland" },
    FRA: { name: "France" },
    GRC: { name: "Greece" },
    HRV: { name: "Croatia" },
    HUN: { name: "Hungary" },
    IRL: { name: "Ireland" },
    ITA: { name: "Italy" },
    LTU: { name: "Lithuania" },
    LUX: { name: "Luxembourg" },
    LVA: { name: "Latvia" },
    MLT: { name: "Malta" },
    NLD: { name: "Netherlands" },
    POL: { name: "Poland" },
    PRT: { name: "Portugal" },
    ROU: { name: "Romania" },
    "Real GDP": { color: "#0f766e" },
    SVK: { name: "Slovakia" },
    SVN: { name: "Slovenia" },
    SWE: { name: "Sweden" },
    "Under 25": { color: "#f59e0b" },
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
          title({ text: "Government debt, percent of GDP, 2025" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            data: "eu_debt",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: true,
            camera: { fit: { bbox: geoBox([-11, 66, 34, 34]) }, padding: 0 },
            format: ",.0f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.0f") + "% of GDP"}`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            stops: "#fbeee6 20 · #e0835c 83 · #7f2a12 146",
            children: [],
          }, { key: "body" }),
        ],
      }),
      plot({
        data: "eu_debt_1",
        x: "value",
        y: "label",
        color: "value",
        colorType: "piecewise",
        stops: "#fbeee6 20 · #e0835c 83 · #7f2a12 146",
        xType: "linear",
        yType: "band",
        title: "The 12 most indebted EU countries, percent of GDP, 2025",
        format: ",.1~f",
        padding: 0.2,
        children: [bar({ format: ",.1~f", labels: true, selected: "focus" })],
      }, { key: "chart", when: e('scene == "ranked"') }),
      group({
        key: "chart",
        when: e('scene == "greece"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "Greece" }, { key: "title", size: { h: "auto" } }),
          map({
            source: "geo:countries",
            data: "eu_debt",
            key: "label",
            value: "value",
            colorType: "piecewise",
            backdrop: true,
            camera: { fit: { bbox: geoBox([18, 42.5, 30, 34]) }, padding: 0 },
            format: ",.0f",
            label: e(
              'd.value == null ? `${(d.name ?? d.id)}` : (`${(d.name ?? d.id)}: ${"" + format(d.value, ",.0f") + "% of GDP"}`)',
            ),
            legend: true,
            padding: 0,
            projection: "web-mercator",
            selected: "focus",
            stops: "#fbeee6 20 · #e0835c 83 · #7f2a12 146",
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
        text: "The heaviest debts are in the south and west of the union.",
      }),
      step("ranked", {
        set: { focus: [], scene: "ranked" },
        text: "[Greece](GRC) still owes the most, ahead of [Italy](ITA), 15 years after its first bailout.",
      }),
      step("greece", {
        set: { focus: ["GRC"], scene: "greece" },
        text: "[Greece](GRC): debt worth nearly **a year and a half** of everything the economy produces.",
        anchor: "GRC",
      }),
    ],
  }),
});
