// Units split and merge: a parliament's seats gather into a bar per party, and the bars shatter into
// a waffle of squares. Nothing here asks for it: the seats are keyed (party, seat i) and the
// squares (party, square i), so the engine sees that a bar is the parent of its units — it
// partitions the bar's rectangle into near-square cells and flies each to its place, or gathers
// them back into it. Seats: the Riksdag after the 2022 election.
import { data, doc, e, group, motion, step, story, choreo } from "@datars/sdk";
import { bar, hemicycle, plot, waffle } from "@datars/std";

const party = ["V", "S", "MP", "C", "L", "M", "KD", "SD"];
const seats = [24, 107, 18, 24, 16, 68, 19, 73];
const keys = { S: { color: "#e8112d" }, SD: { color: "#dddd00" }, M: { color: "#1b49dd" }, V: { color: "#a01313" }, C: { color: "#009933" }, KD: { color: "#005ea8" }, MP: { color: "#83cf39" }, L: { color: "#3a8fd6" } };
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  id: "motion-units",
  title: "Seats into bars into squares",
  description: "The 349 seats of the Swedish Riksdag as a hemicycle of one dot per seat, then gathered into a bar per party, then split into a waffle of 349 squares.",
  size: [640, 380],
  data: { riksdag: data.values({ party, seats }, { key: "party" }) },
  keys,
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 12] },
    children: [
      hemicycle({ data: "riksdag", value: "seats", category: "party" }, at("seats")),
      plot({ data: "riksdag", x: "party", y: "seats", color: "party", title: "Seats by party", children: [bar({ labels: true, format: ",.0f" })] }, at("bars")),
      waffle({ data: "riksdag", value: "seats", category: "party", columns: 25, rows: 14 }, at("squares")),
    ],
  }),
  motion: motion({ select: { role: "datum" }, duration: 1.6, easing: "cubic-in-out", choreo: choreo.stagger("left", 0.45) }),
  program: story({
    steps: [
      step("seats", { title: "349 seats", text: "One dot per seat, by party, left to right." }),
      step("bars", { title: "Merge", text: "Each party's seats gather into its bar: the seats' keys say whose they are." }),
      step("squares", { title: "Split", text: "Each bar breaks into squares, one per seat, which fly to their place." }),
    ],
  }),
});
