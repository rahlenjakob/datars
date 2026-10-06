// dumbbell: two values per category joined — before and after. How long a commute takes from
// eight districts, the year before a new tram line and the year after; rows keyed (district, year).
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, dumbbell } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const district = ["Old Town", "Harbour", "Riverside", "Hillcrest", "Northgate", "Millbrook", "Westend", "Eastfield"];
const before = [22, 38, 31, 44, 35, 41, 27, 33];
const after = [21, 29, 24, 43, 26, 39, 25, 32];
const rows = district.flatMap((d, i) => [{ district: d, year: "2023", minutes: before[i] }, { district: d, year: "2025", minutes: after[i] }]);
const frame = { data: "trips", x: "minutes", y: "district", xType: "linear", yType: "band", color: "year", legend: true, title: "Average commute to the city centre (minutes)" } as const;

export default doc({
  title: "Commutes before and after a new tram line",
  description: "Average commute times from eight districts in 2023 and 2025 as dumbbells, then with values at both ends, then upright.",
  size: [640, 340],
  data: { trips: data.values(rows, { key: ["district", "year"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [dumbbell()] }, at("default")),
      plot({ ...frame, grid: false, children: [dumbbell({ labels: true, format: "d" })] }, at("labels")),
      plot({ ...frame, x: "district", y: "minutes", xType: "band", yType: "linear", children: [dumbbell()] }, at("upright")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "dumbbell()", text: "A dot per year, joined per district: the long bars are where the tram helped." }),
      step("labels", { title: "labels: true", text: "The two values outside each pair." }),
      step("upright", { title: "xType: \"band\"", text: "Districts along the bottom: the same pairs, standing up." }),
    ],
  }),
});
