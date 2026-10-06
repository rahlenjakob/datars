// radar: one spoke per dimension, each series a closed shape through its values. A fictional
// design team's skills, as its members rated themselves from 0 to 5.
import { doc, data, e, group, op, story, step, table } from "@datars/sdk";
import { radar } from "@datars/std";

const skills = ["Research", "Design", "Writing", "Data", "Code", "Presenting"];
const scores: Record<string, number[]> = { Ana: [4, 5, 3, 2, 2, 4], Ben: [2, 3, 2, 5, 4, 2], Chloé: [5, 2, 4, 3, 1, 5] };
const rows = Object.entries(scores).flatMap(([person, s]) => skills.map((skill, i) => ({ person, skill, score: s[i] })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A design team's skills",
  description: "Six skills rated from 0 to 5: first one member's shape; then all three members, one colour each; then as outlines on a fixed scale of 0 to 5 with five rings.",
  size: [640, 360],
  data: { team: data.values(rows, { key: ["person", "skill"] }) },
  tables: { ana: table("team", op.filter(e('d.person == "Ana"'))) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      radar({ data: "ana", axis: "skill", value: "score" }, at("default")),
      radar({ data: "team", axis: "skill", value: "score", series: "person" }, at("series")),
      radar({ data: "team", axis: "skill", value: "score", series: "person", max: 5, levels: 5, fill: false }, at("outline")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "radar({ axis, value })", text: "One member: a spoke per skill, her ratings joined into a shape." }),
      step("series", { title: "series: \"person\"", text: "Every member, a colour each, named below." }),
      step("outline", { title: "max: 5, levels: 5, fill: false", text: "The whole 0–5 scale in five rings, shapes as outlines." }),
    ],
  }),
});
