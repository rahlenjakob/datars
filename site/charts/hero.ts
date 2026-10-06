// The showcase's hero: one dataset that keeps changing shape — bars, a donut, a treemap, a waffle
// of 100 squares, and one stacked bar of fossil vs low-carbon. Every source keeps its key, so each
// change is a keyed morph (bars become slices, slices become tiles, tiles split into squares).
// Figures: world electricity generation by source, 2023, rounded to whole percent — approximate,
// in the shape of Ember's Global Electricity Review.
import { doc, data, e, group, signal, autoplay, step, motion } from "@datars/sdk";
import { plot, bar, pie, treemap, waffle, stacked } from "@datars/std";

const source = ["Coal", "Gas", "Hydro", "Nuclear", "Wind", "Solar", "Bioenergy", "Oil"];
const share = [35, 23, 14, 9, 8, 5, 3, 3];
const kind = ["Fossil", "Fossil", "Low-carbon", "Low-carbon", "Low-carbon", "Low-carbon", "Low-carbon", "Fossil"];
const keys = {
  Coal: { color: "#7c8494" }, Gas: { color: "#f59e0b" }, Hydro: { color: "#3b82f6" }, Nuclear: { color: "#a78bfa" },
  Wind: { color: "#2dd4bf" }, Solar: { color: "#facc15" }, Bioenergy: { color: "#4ade80" }, Oil: { color: "#b45309" },
};
const when = (s: string) => e(`shape == ${JSON.stringify(s)}`);

export default doc({
  id: "hero",
  title: "Where the world's electricity comes from",
  description: "World electricity generation by source in 2023, shown as bars, a donut, a treemap, a waffle and a fossil versus low-carbon split.",
  size: [880, 500],
  data: { mix: data.values({ source, share, kind }, { key: "source" }) },
  keys,
  signals: { shape: signal.str("bars") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 8, 8] },
    children: [
      plot({ data: "mix", x: "source", y: "share", color: "source", title: "Electricity by source, 2023 (%)", yDomain: [0, 40], children: [bar({ labels: true, format: ".0f", radius: 3 })] }, { key: "chart", when: when("bars") }),
      pie({ data: "mix", value: "share", category: "source", inner: 0.56, format: ".0f" }, { key: "chart", when: when("donut") }),
      treemap({ data: "mix", value: "share", category: "source", format: ".0f" }, { key: "chart", when: when("treemap") }),
      waffle({ data: "mix", value: "share", category: "source", gap: 3 }, { key: "chart", when: when("waffle") }),
      plot({ data: "mix", x: "share", y: "kind", xType: "linear", yType: "band", color: "source", title: "Fossil vs low-carbon (%)", legend: true,
        children: [stacked({ series: "source", segmentKey: e("d.source") })] }, { key: "chart", when: when("split") }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.2 }),
  program: autoplay({
    loop: true,
    steps: [
      step("bars", { set: { shape: "bars" }, hold: 2.6 }),
      step("donut", { set: { shape: "donut" }, hold: 2.6 }),
      step("treemap", { set: { shape: "treemap" }, hold: 2.6 }),
      step("waffle", { set: { shape: "waffle" }, hold: 2.6 }),
      step("split", { set: { shape: "split" }, hold: 3 }),
    ],
  }),
});
