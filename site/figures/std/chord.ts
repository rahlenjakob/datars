// chord: flows between groups around a circle. Students who left home for university, between five
// regions of a fictional country, ten years apart; ribbons are keyed (from, to), so they reshape
// rather than redraw when the value changes.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { chord } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// [from, to, students in 2015, in 2025] (hundreds)
const moves: [string, string, number, number][] = [
  ["North", "Capital", 32, 41], ["North", "Coast", 9, 8], ["North", "Lakes", 6, 5],
  ["Coast", "Capital", 24, 30], ["Coast", "North", 7, 6], ["Coast", "Valley", 5, 9],
  ["Lakes", "Capital", 18, 16], ["Lakes", "North", 8, 7], ["Lakes", "Valley", 4, 6],
  ["Valley", "Capital", 14, 22], ["Valley", "Coast", 6, 8], ["Valley", "Lakes", 3, 3],
  ["Capital", "Coast", 12, 11], ["Capital", "North", 10, 9], ["Capital", "Valley", 6, 12], ["Capital", "Lakes", 5, 4],
];

export default doc({
  title: "Where students go to university",
  description: "Students who moved between five regions of a fictional country to study, as a chord diagram: in 2015, then in 2025, then with the flows touching the Valley picked out.",
  size: [640, 360],
  data: {
    moves: data.values({ from: moves.map((m) => m[0]), to: moves.map((m) => m[1]), y2015: moves.map((m) => m[2]), y2025: moves.map((m) => m[3]) }, { key: ["from", "to"] }),
  },
  signals: { focus: signal.keyset() },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      chord({ data: "moves", source: "from", target: "to", value: "y2015", format: ",.0f" }, at("default")),
      chord({ data: "moves", source: "from", target: "to", value: "y2025", format: ",.0f" }, at("later")),
      chord({ data: "moves", source: "from", target: "to", value: "y2025", format: ",.0f", selected: "focus" }, at("focus")),
    ],
  }),
  program: story({
    steps: [
      step("default", { set: { focus: [] }, title: "chord()", text: "Each region's arc is as long as the students leaving and arriving; each ribbon, a flow, takes its origin's colour." }),
      step("later", { set: { focus: [] }, title: "value: \"y2025\"", text: "Ten years on: more students head for the Capital, and the ribbons reshape to match." }),
      step("focus", { set: { focus: ["Valley"] }, title: "selected: \"focus\"", text: "A keyset signal of regions: flows to and from the Valley stay strong." }),
    ],
  }),
});
