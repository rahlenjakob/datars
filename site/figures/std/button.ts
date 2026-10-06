// button: fires a program event or sets a signal — here Previous and Next step the story from
// inside the chart, and Clear sets the highlight back to nothing.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { plot, bar, button } from "@datars/std";

export default doc({
  title: "Books borrowed (thousands)",
  description: "Buttons drawn by the engine: Previous and Next step the story, Clear sets a signal.",
  size: [640, 320],
  data: { b: data.values({ branch: ["Central", "Harbour", "Hill", "Park", "River"], n: [42, 18, 25, 31, 12] }, { key: "branch" }) },
  signals: { picked: signal.str("") },
  scene: group({ key: "root", layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] }, children: [
    group({ key: "buttons", size: { h: 36 }, layout: { type: "columns", gap: 8 }, children: [
      button({ label: "Previous", event: "prev" }, { key: "prev", size: { w: 110 } }),
      button({ label: "Next", event: "next", kind: "primary" }, { key: "next", size: { w: 110 } }),
      button({ label: "Clear", set: "picked", value: "" }, { key: "clear", size: { w: 110 } }),
    ] }),
    plot({ data: "b", x: "branch", y: "n", yDomain: [0, 50], children: [bar({ labels: true, fill: e('picked == "" || d.branch == picked ? "$accent" : "$muted@0.4"') })] }, { key: "chart" }),
  ] }),
  program: story({ steps: [
    step("all", { set: { picked: "" }, title: "Every branch" }),
    step("central", { set: { picked: "Central" }, title: 'picked = "Central"' }),
    step("river", { set: { picked: "River" }, title: 'picked = "River"' }),
  ] }),
});
