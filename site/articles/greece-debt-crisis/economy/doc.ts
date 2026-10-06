// The human cost: Greece's real GDP against the euro area's (2008 = 100), then unemployment, for
// everyone and for the under-25s. Data: Eurostat, nama_10_gdp (chain-linked volumes) and une_rt_a
// (../data/eurostat.json).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, card, line, plot } from "@datars/std";
import { financial } from "../theme";
import src from "../data/eurostat.json";

export default doc({
  id: "greece-debt-crisis/economy",
  title: "Greece's economy and jobs",
  size: [960, 500],
  theme: financial,
  data: { gdp: data.values(src.gdp), jobs: data.values(src.jobs) },
  signals: { scene: signal.str("gdp") },
  keys: {
    Greece: { color: "$accent" },
    "Euro area": { color: "$muted" },
    Everyone: { name: "All workers", color: "$accent" },
    "Under 25": { name: "Under 25", color: "#2f5f8a" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "gdp", x: "x", y: "y", color: "series", xType: "linear", xFormat: "d", format: ",.0f", padding: 0.2, yDomain: [60, 125],
        title: "The size of the economy, after inflation, 2008 = 100",
        children: [
          line({ labels: true, curve: "linear", points: true }),
          annotate({ x: e("scale.x(2013)"), y: e("scale.y(73)"), text: "2013: 27% smaller", connector: "none", dot: true, dx: 0, dy: 18, head: false }),
        ],
      }, { key: "chart", when: e('scene == "gdp"') }),
      plot({
        data: "jobs", x: "x", y: "y", color: "series", xType: "linear", xFormat: "d", format: ",.0f", padding: 0.2, yDomain: [0, 65],
        title: "Unemployment, percent of the labor force",
        children: [
          line({ labels: true, curve: "linear", points: true }),
          annotate({ x: e("scale.x(2013)"), y: e("scale.y(59.2)"), text: "59%", connector: "none", dot: true, dx: 0, dy: -12, head: false }),
          annotate({ x: e("scale.x(2013)"), y: e("scale.y(27.8)"), text: "28%", connector: "none", dot: true, dx: 0, dy: -12, head: false }),
        ],
      }, { key: "chart", when: e('scene == "jobs"') }),
      card({ text: "More than a quarter of Greek output was gone by 2013. In 2025 the economy was still 14 percent smaller than in 2008; the euro area's was 17 percent larger.", at: "top-left", width: 300 }, {
        key: "caption",
        when: e('scene == "gdp"'),
      }),
      card({ text: "In 2013 more than a quarter of the workforce, and nearly three in five under-25s, had no job.", at: "top-right", width: 260 }, {
        key: "caption",
        when: e('scene == "jobs"'),
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
      step("gdp", { set: { scene: "gdp" }, text: "More than a quarter of Greek output was gone by 2013. In 2025 the economy was still 14 percent smaller than in 2008; the euro area's was 17 percent larger." }),
      step("jobs", { set: { scene: "jobs" }, text: "In 2013 more than a quarter of the workforce, and nearly three in five under-25s, had no job." }),
    ],
  }),
});
