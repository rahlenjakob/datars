// Twenty-four years.
// Vote share by party in Riksdag elections, 1998–2022 (%). Source: Valmyndigheten, rounded. Step
// through: all eight parties, the three largest, then the Sweden Democrats' rise up close.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { broadsheet } from "../theme";

const charts = [
  { scene: "all", notes: [], data: "party_history", title: "Vote share by party, 1998–2022 (%)" },
  { scene: "three", notes: [], data: "party_history_1", title: "The three largest parties (%)" },
  {
    scene: "climb",
    notes: [{ text: "Enters the Riksdag", x: "2010", y: "5.7" }, { text: "20.5%", x: "2022", y: "20.5" }],
    data: "party_history_2",
    title: "The Sweden Democrats, 2006–2022 (%)",
    clip: true,
    xDomain: [2005, 2023],
    yDomain: [0, 24],
  },
];

export default doc({
  id: "swedish-election-2022/history",
  title: "Twenty-four years",
  size: [960, 560],
  theme: broadsheet,
  data: {
    party_history: data.values({
      series: Array.from({ length: 56 }, (_, i) => ["S", "M", "SD", "V", "C", "KD", "L", "MP"][Math.floor(i / 7)]),
      x: Array.from({ length: 56 }, (_, i) => 1998 + (i % 7) * 4),
      y: [
        36.4, 39.9, 35, 30.7, 31, 28.3, 30.3, 22.9, 15.3, 26.2, 30.1, 23.3, 19.8, 19.1, 0.4, 1.4, 2.9, 5.7, 12.9, 17.5,
        20.5, 12, 8.4, 5.8, 5.6, 5.7, 8, 6.8, 5.1, 6.2, 7.9, 6.6, 6.1, 8.6, 6.7, 11.8, 9.1, 6.6, 5.6, 4.6, 6.3, 5.3,
        4.7, 13.4, 7.5, 7.1, 5.4, 5.5, 4.6, 4.5, 4.6, 5.2, 7.3, 6.9, 4.4, 5.1,
      ],
    }),
  },
  tables: {
    party_history_1: table("party_history", op.filter(e('["S","M","SD"].includes(d.series)'))),
    party_history_2: table("party_history", op.filter(e('["SD"].includes(d.series)'))),
  },
  signals: { scene: signal.str("all") },
  keys: {
    C: { name: "Centre", color: "#009933" },
    FI: { color: "#c62d70" },
    KD: { name: "Christian Dem.", color: "#005ea8" },
    L: { name: "Liberals", color: "#3a8fd6" },
    M: { name: "Moderates", color: "#1b49dd" },
    MP: { name: "Greens", color: "#83cf39" },
    S: { name: "Social Democrats", color: "#e8112d" },
    SD: { name: "Sweden Democrats", color: "#dddd00" },
    V: { name: "Left", color: "#a01313" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...charts.map((s) => plot({
        data: s.data,
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        title: s.title,
        format: ",.1~f",
        padding: 0.2,
        zero: false,
        clip: s.clip,
        xDomain: s.xDomain,
        yDomain: s.yDomain,
        children: [
          line({ labels: true, curve: "linear", points: true }),
          ...s.notes.map((n) => annotate({
            x: e(`scale.x(${n.x})`),
            y: e(`scale.y(${n.y})`),
            text: n.text,
            connector: "none",
            dot: true,
            dx: 0,
            dy: -10,
            head: false,
          })),
        ],
      }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
      card({
        text: "Eight parties, seven elections. The Social Democrats have come first every time — but never with the 40% they once took for granted.",
        at: "top-right",
        kicker: "Seven elections",
        width: 300,
      }, { key: "caption", when: e('scene == "all"') }),
      card({
        text: "In 2022 SD overtook the Moderates — the first time since the 1970s that the Moderates were not the largest party on the right.",
        at: "left",
        width: 280,
      }, { key: "caption", when: e('scene == "three"') }),
      card({
        text: "From 2.9% in 2006 to 20.5% in 2022: one of the steepest rises in modern Swedish politics.",
        title: "×7",
        at: "top-left",
        width: 280,
      }, { key: "caption", when: e('scene == "climb"') }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("all", {
        set: { scene: "all" },
        title: "Seven elections",
        text: "Eight parties, seven elections. The Social Democrats have come first every time — but never with the 40% they once took for granted.",
      }),
      step("three", {
        set: { scene: "three" },
        text: "In 2022 [SD](SD) overtook [the Moderates](M) — the first time since the 1970s that the Moderates were not the largest party on the right.",
        anchor: "SD",
      }),
      step("climb", {
        set: { scene: "climb" },
        title: "×7",
        text: "From 2.9% in 2006 to 20.5% in 2022: one of the steepest rises in modern Swedish politics.",
      }),
    ],
  }),
});
