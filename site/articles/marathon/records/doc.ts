// The record.
// Marathon world records, men since 1967 and women since 1985, in minutes (120 = two hours). Step
// through: all records, the men's last minutes, the women's.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { sport } from "../theme";

const records = {
  series: [
    "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men", "Men",
    "Men", "Men", "Women", "Women", "Women", "Women", "Women", "Women", "Women", "Women", "Women", "Women",
  ],
  x: [
    1967, 1969, 1981, 1984, 1985, 1988, 1998, 1999, 2002, 2003, 2007, 2008, 2011, 2013, 2014, 2018, 2022, 2023, 1985,
    1998, 1999, 2001, 2001.1, 2002, 2003, 2019, 2023, 2024,
  ],
  y: [
    129.6, 128.55, 128.3, 128.083, 127.2, 126.833, 126.083, 125.7, 125.633, 124.917, 124.433, 123.983, 123.633, 123.383,
    122.95, 121.65, 121.15, 120.583, 141.1, 140.783, 140.717, 139.767, 138.783, 137.3, 135.417, 134.067, 131.883,
    129.933,
  ],
};

const scenes = [
  {
    scene: "all",
    caption: "Fifty-six years of records: the men's has fallen by nine minutes, the women's by eleven since 1985.",
    width: 280,
    notes: [],
    data: "records",
    title2: "Marathon world record, minutes (120 = 2 hours)",
  },
  {
    scene: "men",
    caption: "Kelvin Kiptum, Chicago, 2023 — 35 seconds from two hours.",
    width: 250,
    title: "2:00:35",
    notes: [{ text: "Kiptum", x: "2023", y: "120.583" }, { text: "Kipchoge", x: "2018", y: "121.65" }],
    data: "records_1",
    title2: "Men since 1998: two hours in sight",
    clip: true,
    xDomain: [1996, 2025],
    yDomain: [119.5, 127],
  },
  {
    scene: "women",
    caption: "Ruth Chepngetich, Chicago, 2024: the first woman under 2:10.",
    width: 250,
    title: "2:09:56",
    notes: [{ text: "Radcliffe", x: "2003", y: "135.417" }],
    data: "records_2",
    title2: "Women since 1998",
    clip: true,
    xDomain: [1996, 2025],
    yDomain: [128.5, 142],
  },
];

export default doc({
  id: "marathon/records",
  title: "The record",
  size: [960, 520],
  theme: sport,
  data: { records: data.values(records) },
  tables: {
    records_1: table("records", op.filter(e('["Men"].includes(d.series)'))),
    records_2: table("records", op.filter(e('["Women"].includes(d.series)'))),
  },
  signals: { scene: signal.str("all") },
  keys: { Men: { color: "$accent" }, Women: { color: "#2f5f8a" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...scenes.map((s) => plot({
        data: s.data,
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        title: s.title2,
        format: ",.1~f",
        padding: 0.2,
        zero: false,
        clip: s.clip,
        xDomain: s.xDomain,
        yDomain: s.yDomain,
        children: [
          line({ labels: true, curve: "step-after" }),
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
      ...scenes.map((s) => card({ text: s.caption, title: s.title, at: "top-right", width: s.width }, {
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
    steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene }, title: s.title, text: s.caption })),
  }),
});
