// One dataset, many shapes: bars → treemap → waffle → hemicycle → dots. Every party keeps its key,
// so each change of chart type is a keyed transition (units split out of bars and merge back).
// Figures: Swedish Riksdag election 2022, vote share (%), rounded (approximate).
import { doc, data, e, group, signal, story, step, motion } from "@datars/sdk";
import { plot, bar, treemap, waffle, dot } from "@datars/std";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const share = [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6];
const keys = { S: { color: "#e8112d" }, SD: { color: "#dddd00" }, M: { color: "#1b49dd" }, V: { color: "#a01313" }, C: { color: "#009933" }, KD: { color: "#005ea8" }, MP: { color: "#83cf39" }, L: { color: "#3a8fd6" } };
const when = (s: string) => e(`shape == ${JSON.stringify(s)}`);

export default doc({
  id: "shapes",
  title: "One dataset, many shapes",
  size: [760, 460],
  data: { votes: data.values({ party, share }, { key: "party" }) },
  keys,
  signals: { shape: signal.str("bars") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share, 2022 (%)", children: [bar({ labels: true })] }, { key: "chart", when: when("bars") }),
      treemap({ data: "votes", value: "share", category: "party" }, { key: "chart", when: when("treemap") }),
      waffle({ data: "votes", value: "share", category: "party" }, { key: "chart", when: when("waffle") }),
      plot({ data: "votes", x: "share", y: "party", xType: "linear", yType: "band", color: "party", title: "As dots", children: [dot({ r: 7 })] }, { key: "chart", when: when("dots") }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  program: story({ steps: ["bars", "treemap", "waffle", "dots"].map((s) => step(s, { set: { shape: s } })) }),
});
