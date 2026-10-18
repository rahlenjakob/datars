// The iconography page's pictogram lab: one survey as bars, as a pictogram, as a grid of a hundred icons —
// in either of two icon sets. The page drives it through the public API: `setSignal("shape", …)`,
// `setSignal("set", …)` and `setTokens({ categorical })` for the inks. Every combination is a state
// too, so the published bundle carries both sets' icons (a runtime never fetches one).
import { doc, data, e, group, motion, signal, story, step } from "@datars/sdk";
import { plot, bar, pictogram } from "@datars/std";

const modes = ["Car", "Bus", "Train", "Bike", "Walk"];
const transport = { Car: "car", Bus: "bus", Train: "train-front", Bike: "bike", Walk: "footprints" };
const people = { Car: "user", Bus: "users", Train: "users-round", Bike: "user-round", Walk: "person-standing" };
const pick = (set: "transport" | "people") => e(`set == "${set}"`);
const when = (shape: string, set?: "transport" | "people") => e(set ? `shape == "${shape}" && set == "${set}"` : `shape == "${shape}"`);

export default doc({
  id: "iconography-lab",
  title: "How Fernmouth gets to work, in icons",
  description: "A fictional city's 100,000 commuters by mode — 41,000 by car, 23,000 by bus, 17,000 by train, 12,000 by bike, 7,000 on foot — as bars, as a pictogram of one icon per two thousand, and as a grid of a hundred icons.",
  size: [640, 300],
  data: { trips: data.values({ mode: modes, thousands: [41, 23, 17, 12, 7] }, { key: "mode" }) },
  signals: { shape: signal.str("bars"), set: signal.str("transport") },
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 10] },
    children: [
      plot({ data: "trips", x: "mode", y: "thousands", color: "mode", title: "Commuters by mode (thousands)", children: [bar({ labels: true })] }, { key: "chart", when: when("bars") }),
      ...(["transport", "people"] as const).flatMap((set) => {
        const icons = set === "transport" ? transport : people;
        return [
          pictogram({ data: "trips", value: "thousands", category: "mode", icons, per: 2, unit: "k" }, { key: "chart", when: when("icons", set) }),
          pictogram({ data: "trips", value: "thousands", category: "mode", icons, layout: "grid" }, { key: "chart", when: when("grid", set) }),
        ];
      }),
    ],
  }),
  // No narration: the page's own controls say what's shown.
  program: story({
    steps: [
      step("bars", { set: { shape: "bars", set: "transport" } }),
      step("icons", { set: { shape: "icons", set: "transport" } }),
      step("grid", { set: { shape: "grid", set: "transport" } }),
      step("people", { set: { shape: "icons", set: "people" } }),
      step("people-grid", { set: { shape: "grid", set: "people" } }),
    ],
  }),
});
