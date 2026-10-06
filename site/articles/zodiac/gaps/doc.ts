// Closing in.
// Days between the attributed attacks.
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { bar, card, plot } from "@datars/std";
import { noir } from "../theme";

export default doc({
  id: "zodiac/gaps",
  title: "Closing in",
  size: [960, 340],
  theme: noir,
  data: {
    gaps: data.values({
      label: [
        "Lake Herman Rd → Blue Rock Springs",
        "Blue Rock Springs → Lake Berryessa",
        "Lake Berryessa → Presidio Heights",
      ],
      value: [196, 85, 14],
    }, { key: "label" }),
  },
  signals: { scene: signal.str("gaps") },
  keys: {
    "Blue Rock Springs → Lake Berryessa": { color: "$categorical[1]" },
    "Lake Berryessa → Presidio Heights": { color: "$categorical[2]" },
    "Lake Herman Rd → Blue Rock Springs": { color: "$categorical[0]" },
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "gaps",
        x: "value",
        y: "label",
        color: "value",
        xType: "linear",
        yType: "band",
        colorType: "piecewise",
        title: "Days between attacks",
        format: ",.1~f",
        padding: 0.25,
        stops: "#c0392b 14 · #5a2a27 196",
        children: [bar({ format: ",.1~f", labels: true })],
      }, { key: "chart", when: e('scene == "gaps"') }),
      card({
        text: "Six months, then twelve weeks, then two: the attacks came faster each time — and then, as far as police know, stopped.",
        at: "auto",
        width: 260,
      }, { key: "caption", when: e('scene == "gaps"') }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
  ),
  program: story({
    steps: [
      step("gaps", {
        set: { scene: "gaps" },
        text: "Six months, then twelve weeks, then two: the attacks came faster each time — and then, as far as police know, stopped.",
      }),
    ],
    drivers: ["autoplay"],
  }),
});
