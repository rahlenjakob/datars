// Accessible: bars whose every mark carries a label and a value (what screen readers and keyboard
// focus read); the overview page walks a focus through them with a keyset signal and shows what a
// screen reader says for each. Illustrative figures.
import { data, doc, group, interactive, signal } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { PAD, SIZE } from "./_kit";

export default doc({
  id: "overview-accessibility",
  title: "Bikes rented by weekday",
  description: "Bikes rented on each day of a week, in hundreds, as bars.",
  size: SIZE,
  data: { bikes: data.values({ day: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"], hundreds: [12, 14, 13, 15, 19, 26, 22] }, { key: "day" }) },
  signals: { focus: signal.keyset([]) },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [
    plot({ data: "bikes", x: "day", y: "hundreds", children: [bar({ selected: "focus", labels: true, format: ".0f" })] }, { key: "chart" }),
  ] }),
  program: interactive(),
});
