// A third of humanity.
// Share of the world's population under a national stay-at-home order in this article's dataset, by
// day. Derived from the order dates and approximate 2020 populations — a floor: countries outside
// the dataset count as "no order".
// Sources and rounding: the article's notes on the data.
import { doc, data, e, group, motion, signal, step, story } from "@datars/sdk";
import { annotate, area, card, plot } from "@datars/std";
import { health } from "../theme";

const world_share = {
  series: [
    "Under a national order", "Under a national order", "Under a national order", "Under a national order",
    "Under a national order", "Under a national order", "Under a national order", "Under a national order",
    "Under a national order", "Under a national order", "Under a national order", "Under a national order",
    "Under a national order", "Under a national order", "Under a national order", "Under a national order",
    "Under a national order", "Under a national order",
  ],
  x: [
    "2020-02-29", "2020-03-09", "2020-03-14", "2020-03-16", "2020-03-17", "2020-03-18", "2020-03-19", "2020-03-20",
    "2020-03-21", "2020-03-22", "2020-03-23", "2020-03-25", "2020-03-26", "2020-03-27", "2020-03-28", "2020-03-30",
    "2020-04-07", "2020-04-30",
  ],
  y: [0.7, 1.5, 2.1, 3.5, 4.4, 4.9, 5.2, 6.5, 6.8, 8, 9.3, 28.1, 30.3, 31.1, 31.2, 33.4, 33.5, 33.5],
};

const notes = [{ text: "Italy, March 9", x: "18330", y: "1.5" }, { text: "India, March 25", x: "18346", y: "28.1" }];

export default doc({
  id: "covid-lockdowns/share",
  title: "A third of humanity",
  size: [960, 460],
  theme: health,
  data: { world_share: data.values(world_share, { types: { x: "date" } }) },
  signals: { scene: signal.str("share") },
  keys: { "Under a national order": { color: "#1f8a7e" } },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({
        data: "world_share",
        x: "x",
        y: "y",
        color: "series",
        xType: "linear",
        title: "Share of the world's population under a stay-at-home order, 2020, percent",
        format: ",.1~f",
        padding: 0.2,
        yDomain: [0, 40],
        zero: true,
        children: [
          area({ opacity: 0.7 }),
          ...notes.map((s) => annotate({
            x: e(`scale.x(${s.x})`),
            y: e(`scale.y(${s.y})`),
            text: s.text,
            connector: "none",
            dot: true,
            dx: 0,
            dy: -10,
            head: false,
          })),
        ],
      }, { key: "chart", when: e('scene == "share"') }),
      card({ text: "Counting only the countries in this dataset, so the true share was higher.", at: "auto", width: 260 }, {
        key: "caption",
        when: e('scene == "share"'),
      }),
    ],
  }),
  motion: motion(
    { select: { role: "datum" }, matcher: "by-key" },
    { select: { role: "region" }, matcher: "by-key" },
    { select: { kind: "instance" }, matcher: "by-key" },
    { when: { to: "share" }, select: { kind: "polyline" }, duration: 3, easing: "cubic-out", enter: { trim: 0 } },
  ),
  program: story({
    steps: [
      step("share", { set: { scene: "share" }, text: "Counting only the countries in this dataset, so the true share was higher." }),
    ],
    drivers: ["autoplay"],
  }),
});
