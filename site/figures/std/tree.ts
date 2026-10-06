// tree: a tidy tree of a parent-child table. The org chart of a fictional bicycle maker, keyed by
// team, so the same nodes and links move to their new places when the orientation changes.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { tree } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// [team, reports to]
const org: [string, string | null][] = [
  ["Director", null],
  ["Design", "Director"], ["Frames", "Design"], ["Wheels", "Design"], ["Electrics", "Design"],
  ["Workshop", "Director"], ["Welding", "Workshop"], ["Painting", "Workshop"], ["Assembly", "Workshop"], ["Testing", "Assembly"], ["Packing", "Assembly"],
  ["Sales", "Director"], ["Shops", "Sales"], ["Online", "Sales"], ["Repairs", "Sales"],
  ["Office", "Director"], ["Accounts", "Office"], ["People", "Office"],
];

export default doc({
  title: "How a small bicycle maker is organised",
  description: "Eighteen teams of a fictional bicycle maker as a tidy tree: from the director at the left, then from the top, then radiating from the middle.",
  size: [640, 360],
  data: { org: data.values({ team: org.map((o) => o[0]), boss: org.map((o) => o[1]) }, { key: "team" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      tree({ data: "org", id: "team", parent: "boss" }, at("default")),
      tree({ data: "org", id: "team", parent: "boss", orientation: "vertical" }, at("vertical")),
      tree({ data: "org", id: "team", parent: "boss", orientation: "radial" }, at("radial")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "tree()", text: "The director at the left, each team right of the one it reports to; teams at the end of a line named after their node." }),
      step("vertical", { title: "orientation: \"vertical\"", text: "Top-down, like an org chart: the same nodes and links, moved." }),
      step("radial", { title: "orientation: \"radial\"", text: "The director in the middle, every team on a ring by its distance from the top." }),
    ],
  }),
});
