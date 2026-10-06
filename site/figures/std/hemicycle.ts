// hemicycle: a parliament, one dot per seat on concentric arcs, parties left to right. Seats are
// keyed (party, seat i), so they move to their new places when the rows change.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { hemicycle, legend } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "Seats in a fictional island assembly",
  description: "A 151-seat assembly of six parties, with the total in the middle, then on five rows, then without the total.",
  size: [640, 320],
  data: {
    seats: data.values({ party: ["Harbour List", "Valley Union", "Coast Party", "Forest Alliance", "City Front", "Island League"], seats: [18, 41, 12, 36, 29, 15] }, { key: "party" }),
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] },
    // The legend reads the same categorical scale the seats are coloured by.
    scales: { color: { type: "categorical", domain: { data: "seats", field: "party" }, range: "$categorical" } },
    children: [
      group({ key: "body", children: [
        hemicycle({ data: "seats", value: "seats", category: "party" }, at("default")),
        hemicycle({ data: "seats", value: "seats", category: "party", rows: 5 }, at("rows")),
        hemicycle({ data: "seats", value: "seats", category: "party", rows: 5, total: false }, at("no-total")),
      ] }),
      legend({ scale: "color" }, { size: { h: "auto" } }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "hemicycle()", text: "One dot per seat; the number of rows fits the seats." }),
      step("rows", { title: "rows: 5", text: "Five rows: the seats spread along longer arcs." }),
      step("no-total", { title: "total: false", text: "No seat count in the middle." }),
    ],
  }),
});
