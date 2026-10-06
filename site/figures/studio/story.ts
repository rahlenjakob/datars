// Theme studio: a small story — a line with a text card the engine draws and places, moving
// between three steps. The card's surface, inks, corner and the accent kicker are tokens; the
// steps morph, so new type and shapes move in rather than snap. Illustrative figures.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { card, plot, line } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const swims = [12, 11, 13, 15, 21, 31, 39, 36, 22, 15, 12, 11]; // thousands, a fictional city pool
const notes: Record<string, { kicker: string; title: string; text: string; at?: "top" }> = {
  winter: { kicker: "Winter", title: "Steady at 12,000", text: "The regulars: a lane each before work." },
  peak: { kicker: "Summer", title: "July: 39,000 swims", text: "Three times a winter month.", at: "top" },
  autumn: { kicker: "Autumn", title: "Back down by October", text: "Schools start; the outdoor pool closes." },
};

export default doc({
  title: "Swims at a city pool",
  description: "A year of swims at a city pool as a line, with a card that moves from the winter regulars to the July peak and back down in autumn.",
  size: [960, 380],
  data: { pool: data.values({ month: months, swims }, { key: "month" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 16, 10, 10] },
    children: [
      plot({ data: "pool", x: "month", y: "swims", xType: "point", yDomain: [0, 50], title: "Swims at the city pool (thousands)", children: [line({ points: true })] }, { key: "chart" }),
      ...Object.entries(notes).map(([state, n]) => card({ kicker: n.kicker, title: n.title, text: n.text, width: 230, ...(n.at ? { at: n.at } : {}) }, { key: "note", when: e(`state == "${state}"`) })),
    ],
  }),
  program: story({ steps: [step("winter", {}), step("peak", {}), step("autumn", {})] }),
});
