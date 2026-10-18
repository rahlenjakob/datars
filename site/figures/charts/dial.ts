// The charts page's dial: a mood index in five named zones, the band names set along the arc, the
// needle turning as the page's slider sets `mood` (`setSignal`). In our theme's diverging inks.
import { doc, e, group, signal } from "@datars/sdk";
import { gauge } from "@datars/std";

export default doc({
  id: "charts-dial",
  title: "Market mood dial",
  description: "A fictional investor-mood index from 0 to 100 in five zones — panic, fear, neutral, greed, euphoria — with a needle at the current reading.",
  size: [480, 300],
  signals: { mood: signal.num(62) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [10, 14, 10, 14] },
    children: [
      gauge({
        value: e("mood"), min: 0, max: 100, arc: 200, label: "Market mood",
        bands: [20, 40, 60, 80, 100], names: ["Panic", "Fear", "Neutral", "Greed", "Euphoria"],
        inks: ["$diverging~0", "$diverging~0.25", "$diverging~0.5", "$diverging~0.75", "$diverging~1"],
      }, { key: "dial" }),
    ],
  }),
});
