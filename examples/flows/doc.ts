// Flows: where energy goes (illustrative TWh), as a sankey.
import { doc, data, e, group } from "@datars/sdk";
import { sankey } from "@datars/std";

export default doc({
  id: "flows",
  title: "Energy flows",
  size: [760, 420],
  data: {
    flows: data.values({
      source: ["Hydro", "Nuclear", "Wind", "Wind", "Hydro", "Nuclear", "Industry", "Homes", "Transport"],
      target: ["Grid", "Grid", "Grid", "Export", "Export", "Grid", "Loss", "Loss", "Loss"],
      value: [66, 49, 20, 8, 5, 3, 4, 3, 1],
      id: ["h-g", "n-g", "w-g", "w-e", "h-e", "n-g2", "i-l", "h-l", "t-l"],
    }, { key: "id" }),
  },
  // At most 580 px wide, centred (90 px either side at the authored size); a phone gets its whole width.
  scene: group({ key: "root", layout: { type: "stack", padding: 16, align: "center" }, children: [sankey({ data: "flows", source: "source", target: "target", value: "value" }, { size: { w: e("min(box.w - 32, 580)") } })] }),
});
