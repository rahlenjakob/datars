// card: a text card drawn over the chart by the engine (so it's in PNG, PDF and video too),
// placed where it covers the least data. Here it points out the peak of a year of swims.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { card, plot, line } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const swims = [12, 11, 13, 15, 21, 31, 39, 36, 22, 15, 12, 11]; // thousands, a fictional city pool
const note = { title: "July: 39,000 swims", text: "Three times a winter month." };
const at = (state: string) => ({ key: "note", when: e(`state == "${state}"`) });

export default doc({
  title: "Swims at a city pool",
  description: "A card over a line chart, placed automatically where it covers no data, then with a kicker, then anchored at the top over the peak.",
  size: [640, 320],
  data: { pool: data.values({ month: months, swims }, { key: "month" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "pool", x: "month", y: "swims", xType: "point", yDomain: [0, 60], title: "Swims (thousands)",
        children: [line({ points: true })] }, { key: "chart" }),
      card(note, at("default")),
      card({ ...note, kicker: "Summer" }, at("kicker")),
      card({ ...note, kicker: "Summer", at: "top", width: 220 }, at("placed")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "card({ title, text })", text: "Placed where it covers the least data and text." }),
      step("kicker", { title: "kicker", text: "A small accent line above the title." }),
      step("placed", { title: "at: \"top\", width: 220", text: "An anchor to try first (over the peak), and a narrower card." }),
    ],
  }),
});
