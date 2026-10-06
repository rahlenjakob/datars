// Europe's fortnight.
// Day in March 2020 the national stay-at-home order began (approximate).
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, card, plot } from "@datars/std";
import { health } from "../theme";

export default doc({
  id: "covid-lockdowns/europe",
  title: "Europe's fortnight",
  size: [960, 520],
  theme: health,
  data: {
    eu_order_day: data.values({
      label: ["ITA", "ESP", "AUT", "CZE", "FRA", "BEL", "PRT", "DEU", "GBR", "GRC", "NLD", "POL", "IRL", "HUN"],
      value: [9, 14, 16, 16, 17, 18, 19, 22, 23, 23, 23, 25, 27, 28],
    }, { key: "label" }),
  },
  tables: { eu_order_day_1: table("eu_order_day", op.sort("value")) },
  signals: { scene: signal.str("days") },
  keys: {
    AUT: { name: "Austria" },
    BEL: { name: "Belgium" },
    CZE: { name: "Czechia" },
    DEU: { name: "Germany" },
    ESP: { name: "Spain" },
    FRA: { name: "France" },
    GBR: { name: "Britain" },
    GRC: { name: "Greece" },
    HUN: { name: "Hungary" },
    IRL: { name: "Ireland" },
    ITA: { name: "Italy" },
    NLD: { name: "Netherlands" },
    POL: { name: "Poland" },
    PRT: { name: "Portugal" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "eu_order_day_1",
        x: "value",
        y: "label",
        color: "value",
        xType: "linear",
        yType: "band",
        colorType: "piecewise",
        title: "The day in March 2020 each country ordered its people to stay home",
        format: ",.1~f",
        padding: 0.2,
        stops: "#0b3440 9 · #7fcbbd 28",
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "days"') }),
      card({ text: "From Italy, on March 9, to Hungary, on March 28: 19 days.", at: "auto", width: 260 }, {
        key: "caption",
        when: e('scene == "days"'),
      }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("days", { set: { scene: "days" }, text: "From Italy, on March 9, to Hungary, on March 28: 19 days." }),
    ],
  }),
});
