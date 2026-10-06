// Viewers of each season's finale in the United States, millions (Nielsen, live and same day, as
// widely reported): one bar a season, each taller than the last.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { bar, card, plot } from "@datars/std";
import { fantasy } from "../theme";

export default doc({
  id: "westeros/ratings",
  title: "Finale viewers",
  size: [960, 420],
  theme: fantasy,
  data: {
    finales: data.values({
      label: ["Season 1", "Season 2", "Season 3", "Season 4", "Season 5", "Season 6", "Season 7", "Season 8"],
      value: [3.04, 4.2, 5.39, 7.09, 8.11, 8.89, 12.07, 13.61],
    }, { key: "label" }),
  },
  signals: { scene: signal.str("r") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "finales",
        x: "label",
        y: "value",
        xType: "band",
        title: "Viewers of each season's finale in the U.S., millions",
        format: ",.0f",
        padding: 0.3,
        children: [bar({ format: ",.1f", labels: true, fill: "$accent" })],
      }, { key: "chart", when: e('scene == "r"') }),
      card({ text: "Every finale drew more viewers than the one before, even the divisive last.", at: "top-left", width: 260 }, {
        key: "caption",
        when: e('scene == "r"'),
      }),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
  program: story({ steps: [step("r", { set: { scene: "r" }, text: "Every finale drew more viewers than the one before, even the divisive last." })] }),
});
