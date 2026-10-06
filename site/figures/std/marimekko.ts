// marimekko: columns as wide as each category's share of the total, split by each series' share
// within it — every area a share of everything. Bicycles sold in four regions of a fictional
// country, by type; segments keyed (type, region), as stacked bars are.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, marimekko, stacked } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const regions = ["North", "East", "South", "West"];
const sold: Record<string, number[]> = { City: [42, 18, 30, 12], Road: [12, 9, 22, 6], Mountain: [8, 14, 6, 3], Electric: [26, 7, 20, 9] };
const rows = Object.entries(sold).flatMap(([type, n]) => n.map((k, i) => ({ type, region: regions[i], k })));
const title = "Bicycles sold, by region and type (thousands)";

export default doc({
  title: "Bicycles sold by region and type",
  description: "Bicycle sales in four regions by type as a marimekko, then without labels inside, then morphing into 100 % stacked bars of equal width.",
  size: [640, 380],
  data: { bikes: data.values(rows, { key: ["type", "region"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      marimekko({ data: "bikes", x: "region", series: "type", value: "k", title }, at("default")),
      marimekko({ data: "bikes", x: "region", series: "type", value: "k", title, labels: false }, at("plain")),
      plot({ data: "bikes", x: "region", y: "k", color: "type", legend: true, title, children: [stacked({ offset: "expand" })] }, at("stacked")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "marimekko()", text: "The North sells most bikes (the widest column); in the East, one in four is a mountain bike." }),
      step("plain", { title: "labels: false", text: "Without labels inside: the legend and the tooltips carry the names." }),
      step("stacked", { title: "stacked({ offset: \"expand\" })", text: "The same (type, region) keys: every segment morphs into its 100 % bar, all columns one width." }),
    ],
  }),
});
