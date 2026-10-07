// Every platform: one document laid out in a phone, a tablet and a laptop — the same keys in each,
// so the engine morphs the device and the chart inside it re-lays itself out. Illustrative.
import { data, doc, e, geom, group, motion, shape, step, story, text } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { SIZE } from "./_kit";

const DEVICES = {
  phone: { w: 118, h: 214, r: 18, bezel: [18, 8, 18, 8], label: "iOS and Android" },
  tablet: { w: 262, h: 196, r: 14, bezel: [12, 12, 12, 12], label: "Tablets" },
  laptop: { w: 340, h: 196, r: 8, bezel: [10, 10, 10, 10], label: "Web and desktop" },
} as const;

const S = "min(box.w / 420, box.h / 260)";
const device = (name: keyof typeof DEVICES) => {
  const d = DEVICES[name];
  return group({
    key: "device", when: e(`state == "${name}"`),
    // Drawn for a 420 × 260 box, scaled to the one it gets (a phone's card is narrower).
    transform: { translate: [e(`(box.w - ${d.w} * ${S}) / 2`), e(`(box.h - (${d.h} + 22) * ${S}) / 2`)], scale: e(S) },
    children: [
      shape(geom.rect({ x: 0, y: 0, w: d.w, h: d.h, r: d.r }), { key: "frame", fill: "$surface", stroke: { paint: "$ink-2", width: 2 } }),
      group({ key: "screen", transform: { translate: [d.bezel[3], d.bezel[0]] }, size: { w: d.w - d.bezel[1] - d.bezel[3], h: d.h - d.bezel[0] - d.bezel[2] }, layout: { type: "stack", padding: [4, 4, 2, 2] }, children: [
        plot({ data: "units", x: name === "phone" ? "n" : "os", y: name === "phone" ? "os" : "n", xType: name === "phone" ? "linear" : "band", yType: name === "phone" ? "band" : "linear", color: "os", grid: false, children: [bar()] }, { key: "chart" }),
      ] }),
      text(d.label, [e(`${d.w / 2}`), d.h + 8], { key: "label", style: { font: "font.strong", size: "$size.label", ink: "$ink-2", align: "middle", baseline: "top" } }),
    ],
  });
};

export default doc({
  id: "overview-platforms",
  title: "One chart, three screens",
  description: "The same bar chart in a phone, a tablet and a laptop: horizontal bars on the narrow screen, vertical on the wider ones.",
  size: SIZE,
  data: { units: data.values({ os: ["Web", "iOS", "Android", "Mac"], n: [46, 31, 27, 12] }, { key: "os" }) },
  scene: group({ key: "root", children: [device("phone"), device("tablet"), device("laptop")] }),
  motion: motion({ duration: 1.1, easing: "cubic-in-out" }),
  program: story({ steps: [step("phone"), step("tablet"), step("laptop")] }),
});
