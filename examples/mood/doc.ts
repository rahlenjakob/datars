// A newsroom's market-mood dial: a weekly index from 0 to 100 in five named zones, the needle
// turning from week to week. A fictional index; illustrative values.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { gauge, title, card } from "@datars/std";

const weeks = [
  { week: "Week 1", value: 71, text: "Investors shrugged off the rate decision." },
  { week: "Week 2", value: 52, text: "A weak jobs report took the edge off." },
  { week: "Week 3", value: 28, text: "Bank results sent the index into fear." },
  { week: "Week 4", value: 9, text: "Panic: the lowest reading this year." },
];

export default doc({
  id: "mood",
  title: "Market mood",
  description: "A fictional weekly index of investor mood from 0 (panic) to 100 (euphoria), in five zones: panic, fear, neutral, greed and euphoria.",
  size: [640, 460],
  data: { weeks: data.values({ week: weeks.map((w) => w.week), value: weeks.map((w) => w.value) }, { key: "week" }) },
  signals: { mood: signal.num(weeks[0].value) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 14, 20] },
    children: [
      group({ key: "page", layout: { type: "rows", gap: 6 }, children: [
      title({ text: "Market mood", subtitle: "Investor sentiment index, 0–100 (illustrative)" }, { key: "title", size: { h: "auto" } }),
      gauge({
        value: e("mood"), min: 0, max: 100, arc: 200, label: "This week",
        bands: [20, 40, 60, 80, 100],
        names: ["Panic", "Fear", "Neutral", "Greed", "Euphoria"],
        inks: ["$diverging~0", "$diverging~0.25", "$diverging~0.5", "$diverging~0.75", "$diverging~1"],
      }, { key: "dial", size: { h: "fill" } }),
      // The week's note in its own row under the dial: never over the bands, and at a reader's
      // bigger text the dial gives it room rather than the other way round.
      card({ title: e("narration.title"), text: e("narration.text"), width: 220, at: "left", dodge: false }, { key: "caption", size: { h: "auto" } }),
      ] }),
    ],
  }),
  program: story({
    steps: weeks.map((w) => step(w.week, { set: { mood: w.value }, title: w.week, text: w.text })),
  }),
});
