// Decades.
// Seconds cut from the men's record in each decade (derived from the record progression).
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { bar, card, plot } from "@datars/std";
import { sport } from "../theme";

export default doc({
  id: "marathon/decades",
  title: "Decades",
  size: [960, 420],
  theme: sport,
  data: {
    decades: data.values({
      label: ["1970s", "1980s", "1990s", "2000s", "2010s", "2020s"],
      value: [0, 103, 68, 103, 140, 64],
    }, { key: "label" }),
  },
  signals: { scene: signal.str("d") },
  keys: {
    "1970s": { color: "$categorical[0]" },
    "1980s": { color: "$categorical[1]" },
    "1990s": { color: "$categorical[2]" },
    "2000s": { color: "$categorical[3]" },
    "2010s": { color: "$categorical[4]" },
    "2020s": { color: "$categorical[5]" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "decades",
        x: "label",
        y: "value",
        color: "value",
        colorType: "piecewise",
        title: "Seconds cut from the men's record, by decade",
        format: ",.1~f",
        padding: 0.3,
        stops: "#ffd1b3 0 · #ff4d00 140",
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "d"') }),
      card({
        text: "The 2010s took 140 seconds off — the most of any decade — as shoe technology and pacing changed.",
        at: "top-left",
        width: 290,
      }, { key: "caption", when: e('scene == "d"') }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("d", {
        set: { scene: "d" },
        text: "The 2010s took 140 seconds off — the most of any decade — as shoe technology and pacing changed.",
      }),
    ],
    drivers: ["autoplay"],
  }),
});
