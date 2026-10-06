// sankey: flows between nodes — nodes in columns by depth, ribbons as wide as their flow. Every
// link is a keyed row, so the ribbons re-lay themselves when the nodes change.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { sankey } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const flows: [string, string, number][] = [
  ["Salary", "Budget", 3100], ["Side job", "Budget", 600], ["Budget", "Housing", 1250], ["Budget", "Food", 700],
  ["Budget", "Transport", 380], ["Budget", "Savings", 650], ["Budget", "Leisure", 470], ["Budget", "Other", 250],
];

export default doc({
  title: "Where a household's monthly income goes",
  description: "Two incomes flow through one budget into six uses, then with wider nodes and more space between them, then with the links touching Food selected.",
  size: [640, 320],
  data: {
    flows: data.values({ id: flows.map(([s, t]) => `${s}→${t}`), from: flows.map((f) => f[0]), to: flows.map((f) => f[1]), eur: flows.map((f) => f[2]) }, { key: "id" }),
  },
  signals: { focus: signal.keyset() },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      sankey({ data: "flows", source: "from", target: "to", value: "eur", selected: "focus" }, at("default")),
      sankey({ data: "flows", source: "from", target: "to", value: "eur", selected: "focus", nodeWidth: 24, nodePadding: 20 }, { key: "chart", when: e('state != "default"') }),
    ],
  }),
  program: story({
    steps: [
      step("default", { set: { focus: [] }, title: "sankey()", text: "Ribbons as wide as the euros that flow along them." }),
      step("nodes", { set: { focus: [] }, title: "nodeWidth: 24, nodePadding: 20", text: "Wider nodes, more space between them." }),
      step("selected", { set: { focus: ["Food"] }, title: "selected: \"focus\"", text: "A keyset signal of node names: links touching Food stay strong." }),
    ],
  }),
});
