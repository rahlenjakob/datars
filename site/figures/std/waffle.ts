// waffle: one square per unit — here one per commuter out of 100. Units are keyed (mode, unit i),
// so the squares merge into the same rows' bars and split out again.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { waffle, legend, plot, bar } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "How 100 commuters get to work",
  description: "A hundred commuters as a 10 × 10 waffle with wider gaps, then merged into one bar per mode.",
  size: [640, 320],
  data: { modes: data.values({ mode: ["Car", "Bicycle", "Bus", "Walking", "Train"], people: [38, 22, 18, 14, 8] }, { key: "mode" }) },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "columns", gap: 16, padding: [16, 20, 12, 12] },
    // The legend reads the same categorical scale the waffle colours by.
    scales: { color: { type: "categorical", domain: { data: "modes", field: "mode" }, range: "$categorical" } },
    children: [
      group({ key: "body", children: [
        waffle({ data: "modes", value: "people", category: "mode" }, at("default")),
        waffle({ data: "modes", value: "people", category: "mode", gap: 5 }, at("gap")),
        plot({ data: "modes", x: "mode", y: "people", color: "mode", children: [bar({ labels: true })] }, at("bars")),
      ] }),
      legend({ scale: "color" }, { size: { w: 90 } }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "waffle()", text: "Ten columns of ten: each square is one person." }),
      step("gap", { title: "gap: 5", text: "More space between the squares (px)." }),
      step("bars", { title: "Keyed units", text: "Each mode's squares merge into its bar." }),
    ],
  }),
});
