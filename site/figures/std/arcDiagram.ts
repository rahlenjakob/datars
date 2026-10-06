// arcDiagram: nodes on a line, each link an arc above it. The characters of a fictional play,
// linked when they share a scene; ordered by cluster, the families sit together and their arcs stay
// short.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { arcDiagram } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Listed as the programme lists them (in order of appearance), not by family.
const cast: [string, string][] = [
  ["Ines", "Harbour"], ["Marek", "Hill"], ["Oda", "Harbour"], ["Pavel", "Hill"], ["Rosa", "Harbour"], ["Sten", "Mill"], ["Tove", "Hill"],
  ["Ugo", "Mill"], ["Vera", "Harbour"], ["Wim", "Mill"], ["Yara", "Hill"], ["Zeno", "Mill"], ["Alma", "Harbour"], ["Bjorn", "Hill"],
];
// [character, character, scenes together]
const scenes: [string, string, number][] = [
  ["Ines", "Oda", 6], ["Ines", "Rosa", 4], ["Oda", "Rosa", 3], ["Ines", "Vera", 2], ["Rosa", "Alma", 2], ["Vera", "Alma", 3], ["Oda", "Vera", 1],
  ["Marek", "Pavel", 5], ["Marek", "Tove", 3], ["Pavel", "Yara", 2], ["Tove", "Bjorn", 2], ["Yara", "Bjorn", 3], ["Marek", "Yara", 1],
  ["Sten", "Ugo", 4], ["Sten", "Wim", 2], ["Ugo", "Zeno", 3], ["Wim", "Zeno", 2],
  ["Ines", "Marek", 4], ["Rosa", "Sten", 1], ["Pavel", "Ugo", 1], ["Alma", "Bjorn", 2],
];

export default doc({
  title: "Who shares a scene in a fictional play",
  description: "Fourteen characters of three families on a line, an arc for every pair who share a scene: ordered by cluster, then coloured by family with arcs as thick as their scenes together, then in order of appearance.",
  size: [640, 320],
  data: {
    cast: data.values({ name: cast.map((c) => c[0]), family: cast.map((c) => c[1]) }, { key: "name" }),
    scenes: data.values({ a: scenes.map((s) => s[0]), b: scenes.map((s) => s[1]), n: scenes.map((s) => s[2]) }, { key: ["a", "b"] }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      arcDiagram({ nodes: "cast", links: "scenes", id: "name", source: "a", target: "b" }, at("default")),
      arcDiagram({ nodes: "cast", links: "scenes", id: "name", source: "a", target: "b", color: "family", weight: "n" }, at("family")),
      arcDiagram({ nodes: "cast", links: "scenes", id: "name", source: "a", target: "b", color: "family", weight: "n", order: "input" }, at("input")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "arcDiagram()", text: "Ordered by cluster: characters who share many scenes sit together, so most arcs are short." }),
      step("family", { title: "color: \"family\", weight: \"n\"", text: "Coloured by family; arcs within a family take its colour and grow with the scenes shared." }),
      step("input", { title: "order: \"input\"", text: "In order of appearance the families mix and the arcs reach across the stage." }),
    ],
  }),
});
