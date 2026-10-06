// Where the world's energy goes: primary sources → electricity → end-use sectors → useful energy
// or losses, as a sankey. Every link is a keyed row; ribbons are as wide as their flow.
// Illustrative, rounded figures (exajoules) in the shape of published global energy-flow charts.
import { doc, data, group } from "@datars/sdk";
import { sankey } from "@datars/std";

const flows: [string, string, number][] = [
  ["Oil", "Transport", 105], ["Oil", "Industry", 30], ["Oil", "Buildings", 15], ["Oil", "Electricity", 5],
  ["Coal", "Electricity", 95], ["Coal", "Industry", 60], ["Coal", "Buildings", 8],
  ["Gas", "Electricity", 55], ["Gas", "Industry", 45], ["Gas", "Buildings", 40],
  ["Nuclear", "Electricity", 29],
  ["Renewables", "Electricity", 50], ["Renewables", "Buildings", 25], ["Renewables", "Industry", 10], ["Renewables", "Transport", 4],
  ["Electricity", "Industry", 40], ["Electricity", "Buildings", 45], ["Electricity", "Transport", 3], ["Electricity", "Lost as heat", 146],
  ["Industry", "Useful energy", 110], ["Industry", "Lost as heat", 75],
  ["Buildings", "Useful energy", 90], ["Buildings", "Lost as heat", 43],
  ["Transport", "Useful energy", 25], ["Transport", "Lost as heat", 87],
];

export default doc({
  id: "energy",
  title: "Where the world's energy goes",
  description: "Primary energy sources flow through electricity generation into industry, buildings and transport, and end as useful energy or losses.",
  size: [880, 460],
  data: {
    flows: data.values({
      id: flows.map(([s, t]) => `${s}→${t}`),
      source: flows.map((f) => f[0]),
      target: flows.map((f) => f[1]),
      ej: flows.map((f) => f[2]),
    }, { key: "id" }),
  },
  keys: {
    Oil: { color: "#8c6d46" }, Coal: { color: "#5f6673" }, Gas: { color: "#e39b2d" }, Nuclear: { color: "#9b7ae0" }, Renewables: { color: "#2fae76" },
    Electricity: { color: "#3f7bd9" }, Industry: { color: "#c75b7a" }, Buildings: { color: "#d98c4a" }, Transport: { color: "#4aa3b5" },
    "Useful energy": { color: "#3aa35b" }, "Lost as heat": { color: "#a3a8b3" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 100, 16, 88] },
    children: [sankey({ data: "flows", source: "source", target: "target", value: "ej", nodeWidth: 12, nodePadding: 16, label: "=`${d.source} → ${d.target}: ${d.ej} EJ`" })],
  }),
});
