// The Riksdag: 349 seats as a hemicycle, then the blocs as a stacked bar.
// Seats: the 2022 election result.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { hemicycle, plot, bar } from "@datars/std";

const party = ["V", "S", "MP", "C", "L", "M", "KD", "SD"];
const seats = [24, 107, 18, 24, 16, 68, 19, 73];

export default doc({
  id: "riksdag",
  title: "The Riksdag after 2022",
  size: [760, 440],
  data: { seats: data.values({ party, seats }, { key: "party" }) },
  keys: { S: { color: "#e8112d" }, SD: { color: "#dddd00" }, M: { color: "#1b49dd" }, V: { color: "#a01313" }, C: { color: "#009933" }, KD: { color: "#005ea8" }, MP: { color: "#83cf39" }, L: { color: "#3a8fd6" } },
  signals: { view: signal.str("seats") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 16, 16, 16] },
    children: [
      hemicycle({ data: "seats", value: "seats", category: "party" }, { key: "chart", when: e('view == "seats"') }),
      plot({ data: "seats", x: "party", y: "seats", color: "party", title: "Seats by party", children: [bar({ labels: true, format: ",.0f" })] }, { key: "chart", when: e('view == "bars"') }),
    ],
  }),
  program: story({ steps: [step("seats", { set: { view: "seats" }, text: "349 seats; 175 for a majority." }), step("bars", { set: { view: "bars" } })] }),
});
