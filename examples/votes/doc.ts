// Swedish Riksdag election 2022: vote share by party. One dataset, four shapes — every change is a
// keyed transition (each party keeps its key: its bar becomes its slice).
// Figures: Valmyndigheten, rounded (approximate).
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pie, card } from "@datars/std";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const share = [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6];

export default doc({
  id: "votes",
  title: "Vote share by party, Sweden 2022",
  size: [720, 440],
  data: { votes: data.values({ party, share }, { key: "party" }) },
  keys: {
    S: { name: "Social Democrats", color: "#e8112d" }, SD: { name: "Sweden Democrats", color: "#dddd00" },
    M: { name: "Moderates", color: "#1b49dd" }, V: { name: "Left", color: "#a01313" }, C: { name: "Centre", color: "#009933" },
    KD: { name: "Christian Democrats", color: "#005ea8" }, MP: { name: "Greens", color: "#83cf39" }, L: { name: "Liberals", color: "#3a8fd6" },
  },
  tables: { ranked: { from: "votes", ops: [{ op: "sort", by: [["share", "desc"]] }] } },
  signals: { shape: signal.str("bars") },
  // Parties keep their identity across recipes: pair data marks by their own key, not their path.
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share by party, 2022 (%)", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
      plot({ data: "ranked", x: "share", y: "party", xType: "linear", yType: "band", color: "party", title: "Ranked", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "ranked"') }),
      pie({ data: "votes", value: "share", category: "party" }, { key: "chart", when: e('shape == "pie"') }),
      pie({ data: "votes", value: "share", category: "party", inner: 0.58, total: true }, { key: "chart", when: e('shape == "donut"') }),
      // One card narrates every step (the step's title and text), drawn over the chart.
      card({ title: e("narration.title"), text: e("narration.text"), width: 230 }, { key: "caption" }),
    ],
  }),
  program: story({
    steps: [
      step("bars", { set: { shape: "bars" }, title: "30.3%", text: "for the Social Democrats, still Sweden's largest party." }),
      step("ranked", { set: { shape: "ranked" }, text: "Ranked: eight parties cleared the 4% threshold." }),
      step("pie", { set: { shape: "pie" }, text: "Each bar becomes a slice." }),
      step("donut", { set: { shape: "donut" }, text: "A donut frees the middle for the total." }),
    ],
  }),
});
