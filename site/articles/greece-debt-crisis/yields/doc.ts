// What markets charged: 10-year government bond yields for Greece and Germany, annual averages,
// 2000–2025; the crisis years up close; the years since. Data: Eurostat, irt_lt_mcby_a (Maastricht
// criterion bond yields) (../data/eurostat.json).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { financial } from "../theme";
import src from "../data/eurostat.json";

const scenes = [
  { scene: "both", caption: "For the euro's first decade, markets charged Greece barely more than Germany.", title: "Interest on 10-year government bonds, percent", xDomain: [2000, 2025] },
  { scene: "spike", caption: "By 2012 the yield averaged 22.5 percent: a price that assumed default.", title: "The crisis years, 2008–2015", xDomain: [2008, 2015] },
  { scene: "now", caption: "By 2025 Greece paid less than a point more than Germany.", title: "Since 2016", xDomain: [2016, 2025] },
];

export default doc({
  id: "greece-debt-crisis/yields",
  title: "Greek and German bond yields",
  size: [960, 480],
  theme: financial,
  data: { yields: data.values(src.yields) },
  signals: { scene: signal.str("both") },
  keys: { Greece: { color: "$accent" }, Germany: { color: "$muted" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      ...scenes.map((s) => plot({
        data: "yields",
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
        yDomain: [-1, 24],
        children: [
          line({ labels: true, curve: "linear", points: true }),
          ...(s.scene == "now" ? [] : [annotate({ x: e("scale.x(2012)"), y: e("scale.y(22.5)"), text: "2012: 22.5%", connector: "none", dot: true, dx: 0, dy: -12, head: false })]),
        ],
      }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
      ...scenes.map((s) => card({ text: s.caption, at: "top-left", width: 260 }, {
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
