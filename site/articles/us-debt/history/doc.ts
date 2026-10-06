// What the United States owes, April 1993 to today, one chart through the story's steps: the
// Treasury's first year of daily counts, the one year the total fell (2000), the financial crisis,
// the pandemic and the climb to $40 trillion — the axis rescaling at each step, so each era reads
// at its own scale. Month-end totals from Debt to the Penny (../data.json; ../SOURCES.md).
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, area, line, plot } from "@datars/std";
import { fiscal } from "../theme";
import src from "../data.json";

const scenes = [
  { scene: "start", xDomain: [1993.2, 2001.5], yDomain: [0, 7], note: { text: "April 1993: $4.2 trillion", x: 1993.33, y: 4.254, dx: 30, dy: -34 } },
  { scene: "surplus", xDomain: [1993.2, 2008.4], yDomain: [0, 10], note: { text: "2000: the one year it fell", x: 2000.99, y: 5.662, dx: 20, dy: -44 } },
  { scene: "crisis", xDomain: [1993.2, 2012.5], yDomain: [0, 17], note: { text: "Sept. 30, 2008: $10 trillion", x: 2008.7459, y: 10, dx: -150, dy: -40 } },
  { scene: "pandemic", xDomain: [1993.2, 2022.5], yDomain: [0, 32], note: { text: "May 5, 2020: $25 trillion", x: 2020.3415, y: 25, dx: -150, dy: -30 } },
  { scene: "forty", xDomain: [1993.2, 2027], yDomain: [0, 44], note: { text: "Aug. 18, 2026: $40 trillion", x: 2026.6274, y: 40, dx: -160, dy: -8 } },
];

export default doc({
  id: "us-debt/history",
  title: "U.S. public debt since 1993",
  description: "Total public debt outstanding at the end of each month, April 1993 to September 2026, in trillions of dollars: the surplus years, the financial crisis, the pandemic and the climb past $40 trillion.",
  size: [960, 560],
  locale: "en-US",
  theme: fiscal,
  data: { months: data.values(src.months, { key: "date" }) },
  signals: { scene: signal.str("start") },
  motion: motion({ duration: 1.1, easing: "cubic-in-out" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: scenes.map((s) => plot({
      data: "months",
      x: "x",
      y: "total",
      xType: "linear",
      xFormat: "d",
      title: "What the U.S. owes, trillions of dollars",
      subtitle: "Total public debt outstanding at the end of each month",
      clip: true,
      format: ",.0f",
      prefix: "$",
      xDomain: s.xDomain,
      yDomain: s.yDomain,
      children: [
        area({ x: "x", y: "total", curve: "linear", opacity: 0.12, fill: "$accent" }),
        line({ curve: "linear" }),
        annotate({ x: e(`scale.x(${s.note.x})`), y: e(`scale.y(${s.note.y})`), text: s.note.text, dx: s.note.dx, dy: s.note.dy, width: 150 }, { key: "note" }),
      ],
    }, { key: "chart", when: e(`scene == "${s.scene}"`) })),
  }),
  program: story({ steps: scenes.map((s) => step(s.scene, { set: { scene: s.scene } })) }),
});
