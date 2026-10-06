// bump: each series' rank at every time, 1 at the top. The eight most popular baby names in a
// fictional town, ranked by how many children were given them each year.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { bump } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const years = [2019, 2020, 2021, 2022, 2023, 2024];
const births: Record<string, number[]> = {
  Alma: [31, 35, 40, 44, 47, 52], Noah: [48, 46, 45, 41, 39, 37], Elsa: [44, 40, 36, 30, 28, 24], Leo: [22, 27, 33, 38, 45, 49],
  Maja: [39, 41, 38, 37, 33, 31], Liam: [36, 33, 31, 34, 36, 35], Ella: [27, 30, 29, 26, 23, 27], Hugo: [25, 23, 26, 28, 30, 29],
};
const rows = Object.entries(births).flatMap(([name, n]) => years.map((year, i) => ({ name, year, children: n[i] })));
const frame = { data: "names", x: "year", y: "children", series: "name", title: "The eight most popular baby names, by rank" } as const;

export default doc({
  title: "The most popular baby names, year by year",
  description: "Eight baby names ranked by births each year from 2019 to 2024 as a bump chart, then with two names picked out.",
  size: [640, 360],
  data: { names: data.values(rows, { key: ["name", "year"] }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      bump({ ...frame }, at("default")),
      bump({ ...frame, highlight: ["Alma", "Leo"] }, at("highlight")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "bump()", text: "Each name's rank every year, named at both ends; hover a dot for the count." }),
      step("highlight", { title: "highlight: [\"Alma\", \"Leo\"]", text: "The two climbers keep their colour and draw on top; the rest recede." }),
    ],
  }),
});
