// The iconography page's dial: a fictional mood index in five named zones, the names set along the
// arc (or straight, turned to it), the needle a wedge, a line or an arrow. The page's slider sets
// `mood` (`setSignal`); its buttons set `needle` and `names`. Every look is a gauge with the same
// key, so switching morphs one dial into the next.
import { doc, e, group, signal } from "@datars/sdk";
import { gauge } from "@datars/std";

const looks = (["wedge", "line", "arrow"] as const).flatMap((needle) => (["curved", "rotated"] as const).map((names) => ({ needle, names })));

export default doc({
  id: "iconography-dial",
  title: "Market mood dial",
  description: "A fictional investor-mood index from 0 to 100 in five zones — panic, fear, neutral, greed, euphoria — with a needle at the current reading.",
  size: [480, 300],
  signals: { mood: signal.num(62), needle: signal.str("wedge"), names: signal.str("curved") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [10, 14, 10, 14] },
    children: looks.map(({ needle, names }) => gauge({
      value: e("mood"), min: 0, max: 100, arc: 200, label: "Market mood", needleStyle: needle, bandText: names,
      bands: [20, 40, 60, 80, 100], names: ["Panic", "Fear", "Neutral", "Greed", "Euphoria"],
      inks: ["$diverging~0", "$diverging~0.25", "$diverging~0.5", "$diverging~0.75", "$diverging~1"],
    }, { key: "dial", when: e(`needle == "${needle}" && names == "${names}"`) })),
  }),
});
