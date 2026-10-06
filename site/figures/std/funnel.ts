// funnel: stages as centred bars in order, joined by tapering connectors labelled with the share
// that carries on to the next stage.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { funnel } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "An online bakery's checkout funnel",
  description: "Five stages from product views to repeat orders, with the conversion between stages, then without it, then in a compact number format.",
  size: [640, 320],
  data: {
    stages: data.values({ stage: ["Viewed a cake", "Added to basket", "Started checkout", "Paid", "Ordered again"], visitors: [24800, 6900, 3100, 2450, 610] }, { key: "stage" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      funnel({ data: "stages", stage: "stage", value: "visitors", title: "Visitors in May" }, at("default")),
      funnel({ data: "stages", stage: "stage", value: "visitors", title: "Visitors in May", conversion: false }, at("plain")),
      funnel({ data: "stages", stage: "stage", value: "visitors", title: "Visitors in May", format: ".2~s", fill: "$positive" }, at("format")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "funnel()", text: "Each connector says how many carry on: ↓ 28% add a cake to the basket." }),
      step("plain", { title: "conversion: false", text: "Bars only, packed closer together." }),
      step("format", { title: "format: \".2~s\", fill: \"$positive\"", text: "Compact numbers (25k) and another ink for the bars." }),
    ],
  }),
});
