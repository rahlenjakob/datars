// Iconography: a fictional city's commuters by mode as bars, then an icon per two thousand, then a
// grid of a hundred icons — keyed (mode, unit), so each bar splits into its icons. Illustrative.
import { data, doc, group, motion, step, story } from "@datars/sdk";
import { bar, pictogram, plot } from "@datars/std";
import { PAD, SIZE, at } from "./_kit";

const modes = ["Car", "Bus", "Train", "Bike", "Walk"];
const icons = { Car: "car", Bus: "bus", Train: "train-front", Bike: "bike", Walk: "footprints" };

export default doc({
  id: "overview-iconography",
  title: "Commuters as bars, icons and a grid",
  description: "A fictional city's 100,000 commuters by mode — 41,000 by car, 23,000 by bus, 17,000 by train, 12,000 by bike, 7,000 on foot — as bars, as an icon per two thousand, and as a grid of a hundred icons.",
  size: SIZE,
  data: { trips: data.values({ mode: modes, thousands: [41, 23, 17, 12, 7] }, { key: "mode" }) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ data: "trips", x: "mode", y: "thousands", color: "mode", children: [bar()] }, at("bars")),
    pictogram({ data: "trips", value: "thousands", category: "mode", icons, per: 2, unit: "k", note: false }, at("icons")),
    pictogram({ data: "trips", value: "thousands", category: "mode", icons, layout: "grid", legend: false }, at("grid")),
  ] }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  program: story({ steps: [step("bars"), step("icons"), step("grid")] }),
});
