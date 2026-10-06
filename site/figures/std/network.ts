// network: a node-link diagram laid out by a deterministic force simulation. Who co-wrote papers
// with whom at a small fictional institute: nodes keyed by name, links by the pair, so the layout
// moves rather than redraws when a parameter changes.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { network } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const people: [string, string][] = [
  ["Ada", "Optics"], ["Bruno", "Optics"], ["Chiara", "Optics"], ["Dev", "Optics"], ["Elin", "Optics"], ["Farah", "Optics"], ["Gus", "Optics"],
  ["Hana", "Materials"], ["Ivo", "Materials"], ["Jun", "Materials"], ["Kemi", "Materials"], ["Lars", "Materials"], ["Mira", "Materials"],
  ["Noor", "Theory"], ["Oskar", "Theory"], ["Pia", "Theory"], ["Quinn", "Theory"], ["Rafa", "Theory"], ["Sade", "Theory"], ["Tomas", "Theory"],
];
// [author, co-author, papers together]
const pairs: [string, string, number][] = [
  ["Ada", "Bruno", 6], ["Ada", "Chiara", 4], ["Ada", "Dev", 3], ["Ada", "Elin", 2], ["Bruno", "Chiara", 3], ["Bruno", "Farah", 2], ["Chiara", "Gus", 2],
  ["Dev", "Elin", 1], ["Farah", "Gus", 1], ["Ada", "Farah", 1],
  ["Hana", "Ivo", 5], ["Hana", "Jun", 3], ["Hana", "Kemi", 4], ["Ivo", "Lars", 2], ["Jun", "Mira", 2], ["Kemi", "Mira", 3], ["Ivo", "Kemi", 1], ["Lars", "Mira", 1],
  ["Noor", "Oskar", 4], ["Noor", "Pia", 5], ["Noor", "Quinn", 2], ["Oskar", "Rafa", 3], ["Pia", "Sade", 2], ["Quinn", "Tomas", 2], ["Rafa", "Tomas", 1], ["Pia", "Quinn", 1], ["Sade", "Tomas", 1],
  // Across labs: optics and materials share a microscope; theory advises both.
  ["Ada", "Hana", 3], ["Chiara", "Kemi", 1], ["Noor", "Ada", 2], ["Pia", "Hana", 1], ["Oskar", "Ivo", 1],
];

export default doc({
  title: "Who wrote papers with whom",
  description: "Twenty researchers at a fictional institute, linked when they co-wrote a paper: sized by how many co-authors they have, then coloured by lab with links as thick as the papers they share, then with Ada's links picked out.",
  size: [640, 360],
  data: {
    people: data.values({ name: people.map((p) => p[0]), lab: people.map((p) => p[1]) }, { key: "name" }),
    papers: data.values({ a: pairs.map((p) => p[0]), b: pairs.map((p) => p[1]), n: pairs.map((p) => p[2]) }, { key: ["a", "b"] }),
  },
  signals: { focus: signal.keyset() },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      network({ nodes: "people", links: "papers", id: "name", source: "a", target: "b" }, at("default")),
      network({ nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n" }, at("lab")),
      network({ nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n", selected: "focus" }, at("focus")),
    ],
  }),
  program: story({
    steps: [
      step("default", { set: { focus: [] }, title: "network()", text: "Linked people pull together, everyone else pushes apart; the most connected are biggest and named." }),
      step("lab", { set: { focus: [] }, title: "color: \"lab\", weight: \"n\"", text: "Coloured by lab; links as thick, and pulling as hard, as the papers two people share." }),
      step("focus", { set: { focus: ["Ada"] }, title: "selected: \"focus\"", text: "A keyset signal of names: Ada and her links stay strong." }),
    ],
  }),
});
