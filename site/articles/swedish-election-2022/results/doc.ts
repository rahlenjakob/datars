// Riksdag 2022.
// 2018. Four years earlier, the Social Democrats led with 28 percent. The Moderates were second; the
// Sweden Democrats third, at 17.5 percent. 2022. The Sweden Democrats gained three points, to 20.5
// percent, and overtook the Moderates for the first time. The Social Democrats also grew — but their
// partners shrank. Seats, by bloc. Proportional representation turns those shares into 349 seats.
// Lined up from left to right, the red-green parties reach 173 — two short of the 175 needed. The
// new chamber. The Liberals, Christian Democrats, Moderates and Sweden Democrats together hold 176.
// Their deal, signed at Tidö castle, made the Sweden Democrats the government's largest supporting
// party.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, card, hemicycle, plot, stacked, title } from "@datars/std";
import { broadsheet } from "../theme";

const captions = [
  { scene: "2018", at: "auto", caption: "2018: the Social Democrats (S) first, the Moderates (M) second, the Sweden Democrats (SD) third.", width: 280 },
  {
    scene: "2022",
    at: "right",
    caption: "The Sweden Democrats (SD) gained three points and passed the Moderates (M).",
    width: 280,
  },
  {
    scene: "blocs",
    at: "auto",
    caption: "Seats, left to right in the chamber. The line marks a majority: 175.",
    width: 260,
  },
  { scene: "chamber", at: "auto", caption: "349 seats. The Tidö parties — L, KD, M and SD — hold 176.", width: 260 },
];

export default doc({
  id: "swedish-election-2022/results",
  title: "Riksdag 2022",
  size: [960, 560],
  theme: broadsheet,
  data: {
    seats2022: data.values({
      label: ["V", "MP", "S", "C", "L", "KD", "M", "SD"],
      value: [24, 18, 107, 24, 16, 19, 68, 73],
    }, { key: "label" }),
    share2018: data.values({
      label: ["S", "SD", "M", "C", "V", "KD", "L", "MP"],
      value: [28.26, 17.53, 19.84, 8.61, 8, 6.32, 5.49, 4.41],
    }, { key: "label" }),
    share2022: data.values({
      label: ["S", "SD", "M", "V", "C", "KD", "MP", "L"],
      value: [30.33, 20.54, 19.1, 6.75, 6.71, 5.34, 5.08, 4.61],
    }, { key: "label" }),
  },
  tables: { stack_1: table("seats2022", op.derive("whole", e('""'))) },
  signals: { scene: signal.str("2018") },
  keys: {
    C: { color: "#009933" },
    FI: { color: "#c62d70" },
    KD: { color: "#005ea8" },
    L: { color: "#3a8fd6" },
    M: { color: "#1b49dd" },
    MP: { color: "#83cf39" },
    S: { color: "#e8112d" },
    SD: { color: "#dddd00" },
    V: { color: "#a01313" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "share2018",
        x: "label",
        y: "value",
        color: "label",
        title: "Vote share, 2018 (%)",
        format: ",.1~f",
        padding: 0.3,
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "2018"') }),
      plot({
        data: "share2022",
        x: "label",
        y: "value",
        color: "label",
        title: "Vote share, 2022 (%)",
        format: ",.1~f",
        padding: 0.3,
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "2022"') }),
      plot({
        data: "stack_1",
        x: "value",
        y: "whole",
        color: "label",
        xType: "linear",
        yType: "band",
        title: "Seats by bloc: 173 against 176",
        axes: "none",
        grid: false,
        legend: true,
        padding: 0.3,
        xDomain: [0, 1],
        children: [stacked({ offset: "expand", segmentKey: e("d.label"), series: "label" })],
      }, { key: "chart", when: e('scene == "blocs"') }),
      group({
        key: "chart",
        when: e('scene == "chamber"'),
        layout: { type: "rows", gap: 8 },
        children: [
          title({ text: "The new Riksdag" }, { key: "title", size: { h: "auto" } }),
          hemicycle({ data: "seats2022", value: "value", category: "label" }, { key: "body" }),
        ],
      }),
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
    { when: { from: "blocs", to: "chamber" }, select: { role: "datum" }, matcher: { hierarchy: { partition: "auto" } } },
    {
      when: { from: "blocs", to: "chamber" },
      select: { kind: "instance" },
      matcher: { hierarchy: { partition: "auto" } },
    },
  ),
  program: story({
    steps: [
      step("2018", { set: { scene: "2018" }, text: "2018: the Social Democrats (S) first, the Moderates (M) second, the Sweden Democrats (SD) third." }),
      step("2022", {
        set: { scene: "2022" },
        text: "[The Sweden Democrats](SD) gained three points and passed [the Moderates](M).",
        anchor: "SD",
      }),
      step("blocs", {
        set: { scene: "blocs" },
        text: "Seats, left to right in the chamber. The line marks a majority: 175.",
      }),
      step("chamber", { set: { scene: "chamber" }, text: "349 seats. The Tidö parties — L, KD, M and SD — hold 176." }),
    ],
  }),
});
