// The accessibility lab (site/pages/features/accessibility.html): a chart with everything a screen
// reader has to deal with — a title, a value that changes, clickable marks, story steps with
// narration — so the page can show what each reader gets: the semantics tree, the buttons a
// keyboard reaches, the live region's announcements, the chart under simulated colour-vision
// deficiencies, in high contrast and with reduced motion.
//
// Riksdag seats after the 2022 Swedish election (Valmyndigheten): 349 seats, 175 for a majority.
import { data, doc, e, geom, group, op, shape, signal, step, story, text } from "@datars/sdk";
import { bar, plot, title } from "@datars/std";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const seats = [107, 73, 68, 24, 24, 19, 18, 16];
const SUM = "table.sum('picked', 'seats')";

export default doc({
  id: "accessibility-lab",
  title: "Riksdag 2022: build a majority",
  description: "Seats per party after the 2022 Swedish election. Pick parties to form a coalition; 175 of the 349 seats is a majority.",
  size: [720, 500],
  data: { riksdag: data.values({ party, seats }, { key: "party" }) },
  keys: {
    S: { name: "Social Democrats", color: "#e8112d" }, SD: { name: "Sweden Democrats", color: "#dddd00" },
    M: { name: "Moderates", color: "#1b49dd" }, V: { name: "Left", color: "#a01313" }, C: { name: "Centre", color: "#009933" },
    KD: { name: "Christian Democrats", color: "#005ea8" }, MP: { name: "Greens", color: "#83cf39" }, L: { name: "Liberals", color: "#3a8fd6" },
  },
  signals: { coalition: signal.keyset() },
  tables: { picked: { from: "riksdag", ops: [op.filter(e("coalition.has(d.party)"))] } },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 14, padding: [16, 20, 12, 14] },
    children: [
      title({ text: "Riksdag 2022: build a majority", subtitle: "Click a party to add it to the coalition, or Tab to it and press Enter" }, { key: "title" }),
      // The coalition's seats against the 349, with the line a majority has to cross.
      group({ key: "majority", size: { h: 50 }, semantics: { role: "group", label: e(`'Coalition: ' + format(${SUM}, 'd') + ' of 349 seats, ' + (${SUM} >= 175 ? 'a majority' : format(175 - ${SUM}, 'd') + ' short of a majority')`) }, children: [
        text(e(`format(${SUM}, 'd') + ' seats: ' + (${SUM} >= 175 ? 'a majority' : format(175 - ${SUM}, 'd') + ' short of a majority')`), [0, 14], { key: "sum", style: { size: "$size.label", weight: 600, ink: "$ink" } }),
        shape(geom.rect({ x: 0, y: 24, w: e("box.w"), h: 10, r: 5 }), { key: "track", fill: "$grid" }),
        shape(geom.rect({ x: 0, y: 24, w: e(`box.w * min(1, ${SUM} / 349)`), h: 10, r: 5 }), { key: "fill", fill: e(`${SUM} >= 175 ? "$accent" : "$muted"`) }),
        shape(geom.segment({ x1: e("box.w * 175 / 349"), y1: 19, x2: e("box.w * 175 / 349"), y2: 39 }), { key: "line", stroke: { paint: "$ink", width: 2 } }),
        text("175", [e("box.w * 175 / 349"), 50], { key: "goal", style: { size: 11, ink: "$ink-2", align: "middle", baseline: "bottom" } }),
      ] }),
      plot({ data: "riksdag", x: "party", y: "seats", color: "party", format: ".0f", yDomain: [0, 120],
        children: [bar({ selected: "coalition", labels: true, format: ".0f",
          label: e("`${key.name(d.party)}: ${d.seats} seats${coalition.has(d.party) ? ', in the coalition' : ''}`") })] }, { key: "seats" }),
    ],
  }),
  program: story({
    steps: [
      step("parties", { set: { coalition: [] }, title: "Eight parties", text: "349 seats in all: a majority needs 175." }),
      step("tidö", { set: { coalition: ["M", "SD", "KD", "L"] }, title: "The Tidö parties", text: "Moderates, Sweden Democrats, Christian Democrats and Liberals: 176 seats, a majority of one." }),
      step("others", { set: { coalition: ["S", "V", "C", "MP"] }, title: "The other four", text: "Social Democrats, Left, Centre and Greens: 173 seats, two short." }),
      step("largest", { set: { coalition: ["S", "M"] }, title: "The two largest", text: "Social Democrats and Moderates alone: exactly 175." }),
    ],
  }),
});
