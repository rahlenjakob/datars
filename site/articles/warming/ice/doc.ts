// Arctic sea ice in September, when it is smallest: the month's average extent (million km²), every
// year since satellites began measuring it in 1979; then the record low of 2012. Data: NSIDC Sea Ice
// Index, version 4 (N_09_extent_v4.0.csv).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { science } from "../theme";
import ice from "./ice.json";

const charts = [
  { scene: "all", notes: [], title: "Arctic sea ice in September, million square kilometers" },
  {
    scene: "low",
    notes: [
      annotate({ x: e("scale.x(2012)"), y: e("scale.y(3.57)"), text: "2012: 3.57", connector: "none", dot: true, dx: 0, dy: 16, head: false }),
      annotate({ x: e("scale.x(1979)"), y: e("scale.y(7.05)"), text: "1979: 7.05", connector: "none", dot: true, dx: 0, dy: -12, head: false }),
    ],
    title: "2012, the smallest on record",
  },
];

export default doc({
  id: "warming/ice",
  title: "Arctic sea ice in September",
  size: [960, 460],
  theme: science,
  data: { ice: data.values({ series: ice.x.map(() => "September extent"), x: ice.x, y: ice.y }) },
  signals: { scene: signal.str("all") },
  keys: { "September extent": { name: "September extent", color: "#2f5f8a" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...charts.map((s) => plot({
        data: "ice",
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        xFormat: "d",
        title: s.title,
        format: ",.0f",
        padding: 0.2,
        yDomain: [0, 8],
        children: [line({ curve: "linear", points: true }), ...s.notes],
      }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
      card({ text: "The last ten Septembers averaged 4.6 million square kilometers, more than a third less than the first ten.", at: "bottom-left", width: 280 }, {
        key: "caption",
        when: e('scene == "all"'),
      }),
      card({ text: "in September 2012, about half the ice of 1979.", title: "3.57 million km²", at: "bottom-left", width: 250 }, {
        key: "caption",
        when: e('scene == "low"'),
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
      step("all", { set: { scene: "all" }, text: "The last ten Septembers averaged 4.6 million square kilometers, more than a third less than the first ten." }),
      step("low", { set: { scene: "low" }, title: "3.57 million km²", text: "in September 2012, about half the ice of 1979." }),
    ],
  }),
});
