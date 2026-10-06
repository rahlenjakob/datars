// Greek government debt as a share of the economy, 2000–2025, with the euro area for scale: the
// run-up to the first bailout, the three bailouts and the bondholder losses of 2012, then the
// pandemic peak and the fall since. Data: Eurostat, gov_10dd_edpt1 (../data/eurostat.json).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { financial } from "../theme";
import src from "../data/eurostat.json";

const scenes = [
  {
    scene: "run-up",
    caption: "Already larger than the whole economy before the crisis: 105 percent of GDP in 2007.",
    note: { text: "2010: first bailout", x: 2010, y: 147.8 },
    title: "Government debt, percent of GDP: the run-up",
    xDomain: [2000, 2010.5],
  },
  {
    scene: "bailouts",
    caption: "Losses forced on private bondholders in 2012 cut the ratio, but only for a year.",
    note: { text: "2012: bondholders take losses", x: 2012, y: 164.1 },
    title: "Government debt, percent of GDP: three bailouts",
    xDomain: [2000, 2016],
  },
  {
    scene: "long",
    caption: "Above 200 percent in the pandemic year, then down fast as the economy grew and prices rose.",
    note: { text: "2020: 209 percent", x: 2020, y: 209.4 },
    title: "Government debt, percent of GDP, 2000–2025",
    xDomain: [2000, 2025],
  },
];

export default doc({
  id: "greece-debt-crisis/debt",
  title: "Greek government debt",
  size: [960, 560],
  theme: financial,
  data: { debt: data.values(src.debt) },
  signals: { scene: signal.str("run-up") },
  keys: { Greece: { color: "$accent" }, "Euro area": { color: "$muted" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...scenes.map((s) => plot({
        data: "debt",
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        xFormat: "d",
        title: s.title,
        clip: true,
        format: ",.0f",
        padding: 0.2,
        xDomain: s.xDomain,
        yDomain: [0, 220],
        children: [
          line({ labels: true, curve: "linear", points: true }),
          annotate({ x: e(`scale.x(${s.note.x})`), y: e(`scale.y(${s.note.y})`), text: s.note.text, connector: "none", dot: true, dx: 0, dy: -12, head: false }),
        ],
      }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
      ...scenes.map((s) => card({ text: s.caption, at: "bottom-right", width: 260 }, {
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
  program: story({ steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene }, text: s.caption })) }),
});
