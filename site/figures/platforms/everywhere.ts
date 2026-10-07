// The platforms page's centrepiece: one document drawn by every target — the web runtime on the
// page, the iOS sample app in the simulator (Metal), and `datars render` / `datars video` at build
// time (PNG from the CPU reference, SVG, PDF, MP4) — all at the iPhone 16 Pro's chart box
// (402 × 812 pt: the screen under the status bar, where the sample app's view sits), so every
// target lays the chart out in the same box and the captures can be laid over each other.
// The same dataset keeps changing shape — ranked bars, a donut, a treemap, a waffle — each a keyed
// morph. Figures: world electricity generation by source, 2023, rounded to whole percent —
// approximate, in the shape of Ember's Global Electricity Review (as the home page's hero).
import { doc, data, e, group, motion, signal, step, story, text } from "@datars/sdk";
import { pie, plot, bar, treemap, waffle } from "@datars/std";

const source = ["Coal", "Gas", "Hydro", "Nuclear", "Wind", "Solar", "Bioenergy", "Oil"];
const share = [35, 23, 14, 9, 8, 5, 3, 3];
const keys = {
  Coal: { color: "#7c8494" }, Gas: { color: "#f59e0b" }, Hydro: { color: "#3b82f6" }, Nuclear: { color: "#a78bfa" },
  Wind: { color: "#2dd4bf" }, Solar: { color: "#facc15" }, Bioenergy: { color: "#4ade80" }, Oil: { color: "#b45309" },
};
const when = (s: string) => e(`shape == ${JSON.stringify(s)}`);

export default doc({
  id: "platforms-everywhere",
  title: "Where the world's electricity came from, 2023",
  description: "World electricity generation by source in 2023, in percent, as ranked bars, a donut, a treemap and a waffle of 100 squares: coal 35, gas 23, hydro 14, nuclear 9, wind 8, solar 5, bioenergy 3, oil 3.",
  size: [402, 812],
  data: { mix: data.values({ source, share }, { key: "source" }) },
  keys,
  signals: { shape: signal.str("bars"), note: signal.str("Ranked: coal still makes over a third.") },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 6, padding: [20, 20, 24, 20] },
    children: [
      text("The world's electricity, 2023", [0, 0], { key: "title", size: { h: 30 }, style: { font: "font.title", size: 21, ink: "$ink", baseline: "top" }, semantics: { role: "title" } }),
      text("Share of generation by source (%)", [0, 0], { key: "sub", size: { h: 22 }, style: { size: 14, ink: "$muted", baseline: "top" } }),
      text(e("note"), [0, 0], { key: "note", size: { h: 34 }, style: { font: "font.strong", size: 15, ink: "$ink-2", baseline: "top" } }),
      group({
        key: "stage", size: { h: "fill" },
        children: [
          plot({ data: "mix", x: "share", y: "source", xType: "linear", yType: "band", color: "source", xDomain: [0, 40], padding: 0.28,
            children: [bar({ labels: true, format: ".0f", radius: 4, suffix: " %" })] }, { key: "chart", when: when("bars") }),
          pie({ data: "mix", value: "share", category: "source", inner: 0.56, format: ".0f" }, { key: "chart", when: when("donut") }),
          treemap({ data: "mix", value: "share", category: "source", format: ".0f" }, { key: "chart", when: when("treemap") }),
          waffle({ data: "mix", value: "share", category: "source", gap: 3 }, { key: "chart", when: when("waffle") }),
        ],
      }),
      text("Source: Ember, Global Electricity Review (rounded)", [0, 0], { key: "credit", size: { h: 16 }, style: { size: 11, ink: "$muted", baseline: "top" } }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.2 }),
  program: story({
    steps: [
      step("bars", { set: { shape: "bars", note: "Ranked: coal still makes over a third." } }),
      step("donut", { set: { shape: "donut", note: "Fossil fuels: 61 of every 100 units." } }),
      step("treemap", { set: { shape: "treemap", note: "Area is share: hydro outweighs wind and solar." } }),
      step("waffle", { set: { shape: "waffle", note: "One square, one percent." } }),
    ],
  }),
});
