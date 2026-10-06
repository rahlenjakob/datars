// gauge: a dial filled up to the value — or, with bands, coloured ranges and a needle that turns
// when the value changes. Three of a fictional tea shop's service numbers for September.
import { doc, e, group, story, step } from "@datars/sdk";
import { gauge } from "@datars/std";

const status = { bands: [70, 90, 100], inks: ["$negative", "$highlight", "$positive"] };
// Three dials side by side (stacked on a phone), one set per step: keyed alike, they morph.
const dials = (state: string, o: Record<string, unknown> = {}) => group({
  key: "dials", when: e(`state == "${state}"`), layout: { type: "columns", gap: 16, wrap: 420 },
  children: [
    gauge({ value: 86, label: "Satisfaction", suffix: " / 100", ...o }, { key: "satisfaction" }),
    gauge({ value: 94, label: "Orders on time", suffix: "%", ...o }, { key: "on-time" }),
    gauge({ value: 64, label: "Stock available", suffix: "%", ...o }, { key: "stock" }),
  ],
});

export default doc({
  title: "A tea shop's service in three dials",
  description: "Customer satisfaction, orders on time and stock available as dials filled to their values; then on red, amber and green bands with needles; then as half circles.",
  size: [640, 240],
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [dials("default"), dials("bands", status), dials("half", { ...status, arc: 180 })],
  }),
  program: story({
    steps: [
      step("default", { title: "gauge({ value, label })", text: "An arc from 0 to 100, filled to the value, which reads in the middle." }),
      step("bands", { title: "bands: [70, 90, 100], inks", text: "Poor, fair and good ranges along the arc, and a needle at the value." }),
      step("half", { title: "arc: 180", text: "A half-circle dial." }),
    ],
  }),
});
