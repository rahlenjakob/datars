// Charts are content: the same chart as published at nine and republished at twenty to ten with
// new numbers — every page that embeds it shows the new version, morphing there. Illustrative.
import { data, doc, e, group, motion, step, story, text } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { PAD, SIZE } from "./_kit";

const party = ["Blue", "Red", "Green", "Gold"];
const versions = { v1: [34, 29, 11, 8], v2: [31, 33, 12, 9] };
const shown = (v: keyof typeof versions) => group({ key: "chart", when: e(`state == "${v}"`), layout: { type: "rows", gap: 4 }, children: [
  text(v === "v1" ? "v1 · published 09:00 · 12 % counted" : "v2 · republished 09:40 · 31 % counted", [0, 0], { key: "ver", size: { h: "auto" }, style: { font: "font.strong", size: "$size.label", ink: "$ink-2", baseline: "top" } }),
  plot({ data: v, x: "party", y: "seats", color: "party", children: [bar({ labels: true, format: ".0f" })] }, { key: "plot" }),
] });

export default doc({
  id: "overview-delivery",
  title: "An election count, republished",
  description: "Seats per party in an early count, then in a later one, republished to the same address.",
  size: SIZE,
  data: {
    v1: data.values({ party, seats: versions.v1 }, { key: "party" }),
    v2: data.values({ party, seats: versions.v2 }, { key: "party" }),
  },
  scene: group({ key: "root", layout: { type: "stack", padding: PAD }, children: [shown("v1"), shown("v2")] }),
  motion: motion({ duration: 1.1 }),
  program: story({ steps: [step("v1"), step("v2")] }),
});
