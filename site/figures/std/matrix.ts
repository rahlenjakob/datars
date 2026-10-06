// matrix: an adjacency matrix — a row and a column per node, a cell per link. Who co-wrote papers
// with whom at a small fictional institute; ordered by cluster, the three labs show as blocks
// along the diagonal.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { matrix } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// Listed in the order they joined, labs mixed.
const people: [string, string][] = [
  ["Noor", "Theory"], ["Ada", "Optics"], ["Hana", "Materials"], ["Oskar", "Theory"], ["Bruno", "Optics"], ["Ivo", "Materials"], ["Chiara", "Optics"],
  ["Pia", "Theory"], ["Jun", "Materials"], ["Dev", "Optics"], ["Quinn", "Theory"], ["Kemi", "Materials"], ["Elin", "Optics"], ["Rafa", "Theory"],
  ["Lars", "Materials"], ["Farah", "Optics"], ["Sade", "Theory"], ["Mira", "Materials"],
];
// [author, co-author, papers together]
const pairs: [string, string, number][] = [
  ["Ada", "Bruno", 6], ["Ada", "Chiara", 4], ["Ada", "Dev", 3], ["Ada", "Elin", 2], ["Bruno", "Chiara", 3], ["Bruno", "Farah", 2], ["Dev", "Elin", 1], ["Ada", "Farah", 1], ["Chiara", "Elin", 1],
  ["Hana", "Ivo", 5], ["Hana", "Jun", 3], ["Hana", "Kemi", 4], ["Ivo", "Lars", 2], ["Jun", "Mira", 2], ["Kemi", "Mira", 3], ["Ivo", "Kemi", 1], ["Lars", "Mira", 1], ["Jun", "Kemi", 2],
  ["Noor", "Oskar", 4], ["Noor", "Pia", 5], ["Noor", "Quinn", 2], ["Oskar", "Rafa", 3], ["Pia", "Sade", 2], ["Quinn", "Rafa", 1], ["Pia", "Quinn", 1], ["Sade", "Rafa", 2],
  ["Ada", "Hana", 3], ["Chiara", "Kemi", 1], ["Noor", "Ada", 2], ["Pia", "Hana", 1], ["Oskar", "Ivo", 1],
];

export default doc({
  title: "Who wrote papers with whom, as a matrix",
  description: "Eighteen researchers as the rows and columns of a matrix, a cell for every pair who co-wrote a paper: ordered by cluster, then coloured by lab and shaded by the papers they share, then in the order they joined.",
  size: [640, 400],
  data: {
    people: data.values({ name: people.map((p) => p[0]), lab: people.map((p) => p[1]) }, { key: "name" }),
    papers: data.values({ a: pairs.map((p) => p[0]), b: pairs.map((p) => p[1]), n: pairs.map((p) => p[2]) }, { key: ["a", "b"] }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      matrix({ nodes: "people", links: "papers", id: "name", source: "a", target: "b" }, at("default")),
      matrix({ nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n" }, at("lab")),
      matrix({ nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n", order: "input" }, at("input")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "matrix()", text: "Ordered by cluster: groups who write together form blocks along the diagonal." }),
      step("lab", { title: "color: \"lab\", weight: \"n\"", text: "Cells within a lab take its colour, darker the more papers two people share; pairs across labs stay grey." }),
      step("input", { title: "order: \"input\"", text: "In the order people joined, the same cells scatter and the labs disappear." }),
    ],
  }),
});
