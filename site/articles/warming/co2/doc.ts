// Carbon dioxide at Mauna Loa, the annual mean (ppm), every year since 1959: the whole record, then
// the years since 2000 up close. Data: NOAA Global Monitoring Laboratory (co2_annmean_mlo.csv).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { science } from "../theme";
import co2 from "./co2.json";

const charts = [
  { scene: "all", notes: [], title: "Carbon dioxide in the air at Mauna Loa, Hawaii, parts per million" },
  {
    scene: "recent",
    notes: [
      annotate({ x: e("scale.x(2025)"), y: e("scale.y(427.35)"), text: "427 in 2025", connector: "none", dot: true, dx: 0, dy: -12, head: false }),
      annotate({ x: e("scale.x(2000)"), y: e("scale.y(369.71)"), text: "370 in 2000", connector: "none", dot: true, dx: 0, dy: -12, head: false }),
    ],
    title: "Since 2000, parts per million",
    clip: true,
    xDomain: [1998, 2026],
    yDomain: [360, 435],
  },
];

export default doc({
  id: "warming/co2",
  title: "Carbon dioxide at Mauna Loa",
  size: [960, 460],
  theme: science,
  data: { co2: data.values({ series: co2.x.map(() => "CO₂"), x: co2.x, y: co2.y }) },
  signals: { scene: signal.str("all") },
  keys: { "CO₂": { name: "Annual mean", color: "#2f5f8a" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...charts.map((s) => plot({
        data: "co2",
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        xFormat: "d",
        title: s.title,
        format: ",.0f",
        padding: 0.2,
        zero: false,
        clip: s.clip,
        xDomain: s.xDomain,
        yDomain: s.yDomain,
        children: [line({ labels: s.scene == "all", curve: "linear" }), ...s.notes],
      }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
      card({ text: "Measured since 1958 near the top of a Hawaiian volcano. The yearly average has risen every year.", at: "bottom-right", width: 270 }, {
        key: "caption",
        when: e('scene == "all"'),
      }),
      card({ text: "in the 25 years since 2000, more than in the 30 years before.", title: "+58 ppm", at: "bottom-right", width: 250 }, {
        key: "caption",
        when: e('scene == "recent"'),
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
      step("all", { set: { scene: "all" }, text: "Measured since 1958 near the top of a Hawaiian volcano. The yearly average has risen every year." }),
      step("recent", { set: { scene: "recent" }, title: "+58 ppm", text: "in the 25 years since 2000, more than in the 30 years before." }),
    ],
  }),
});
