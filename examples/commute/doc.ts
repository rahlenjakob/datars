// How a city gets to work: one survey, shown as bars, then as a pictogram — each mode's bar splits
// into its icons (one per two thousand commuters), then a grid of a hundred (one per thousand). A fictional city; illustrative figures.
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pictogram, card } from "@datars/std";

const mode = ["Car", "Bus", "Train", "Bike", "Walk"];
const thousands = [41, 23, 17, 12, 7];
const icons = { Car: "car", Bus: "bus", Train: "train-front", Bike: "bike", Walk: "footprints" };

export default doc({
  id: "commute",
  title: "How Fernmouth gets to work",
  description: "A fictional city's commuters by mode: 41,000 by car, 23,000 by bus, 17,000 by train, 12,000 by bike and 7,000 on foot.",
  size: [720, 440],
  data: { trips: data.values({ mode, thousands }, { key: "mode" }) },
  signals: { shape: signal.str("bars") },
  // Each mode keeps its identity from bar to icons: pair data marks by their own key.
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      group({ key: "page", layout: { type: "rows", gap: 8 }, children: [
      group({ key: "body", size: { h: "fill" }, children: [
        plot({ data: "trips", x: "mode", y: "thousands", color: "mode", title: "Commuters by mode (thousands)", children: [bar({ labels: true })] }, { key: "chart", when: e('shape == "bars"') }),
        pictogram({ data: "trips", value: "thousands", category: "mode", icons, per: 2, unit: "k" }, { key: "chart", when: e('shape == "icons"') }),
        pictogram({ data: "trips", value: "thousands", category: "mode", icons, layout: "grid", columns: 10, rows: 10 }, { key: "chart", when: e('shape == "grid"') }),
      ] }),
      ] }),
      card({ title: e("narration.title"), text: e("narration.text"), width: 230 }, { key: "caption" }),
    ],
  }),
  program: story({
    steps: [
      step("bars", { set: { shape: "bars" }, title: "41,000", text: "drive to work, more than ride the bus and the train together." }),
      step("icons", { set: { shape: "icons" }, title: "One icon,", text: "two thousand commuters: each bar is its people." }),
      step("grid", { set: { shape: "grid" }, title: "A hundred", text: "thousand trips a morning, an icon each." }),
    ],
  }),
});
