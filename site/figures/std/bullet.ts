// bullet: actual against target on qualitative bands (poor, fair, good — darkest first), a row per
// measure. A fictional tea shop's third-quarter sales by region, each against its own target.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { bullet } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const common = { data: "regions", label: "region", value: "sales", target: "target", bands: ["poor", "fair", "good"] };

export default doc({
  title: "A tea shop's quarter by region, against target",
  description: "Four regions' third-quarter sales as bullet graphs — a bar for sales, a tick for the target, bands for poor, fair and good — each on its own scale; then with a line under each name; then all on one scale.",
  size: [640, 320],
  data: {
    regions: data.values({
      region: ["North", "Coast", "City", "Online"],
      stores: ["4 shops", "6 shops", "9 shops", "web and app"],
      sales: [182, 236, 412, 318],
      target: [200, 220, 450, 300],
      poor: [120, 150, 300, 200], fair: [170, 200, 400, 280], good: [240, 280, 520, 380],
    }, { key: "region" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      bullet(common, at("default")),
      bullet({ ...common, sublabel: "stores" }, at("sublabel")),
      bullet({ ...common, sublabel: "stores", shared: true }, at("shared")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "bullet({ value, target, bands })", text: "Sales in thousand dollars: the bar; the target: the tick; bands from poor (dark) to good. Each row on its own scale." }),
      step("sublabel", { title: "sublabel: \"stores\"", text: "A smaller second line under each name." }),
      step("shared", { title: "shared: true", text: "One scale for every row, ticked under the last: the regions compare with each other." }),
    ],
  }),
});
