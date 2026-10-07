// The "Networks and hierarchies" family on one small institute: twenty researchers in three labs,
// who reports to whom (a hierarchy) and who wrote papers with whom (a network). People are keyed
// by name in every chart, so a person's node in the tree is their leaf in the dendrogram, their
// band, their arc, their dot in the network and on the arc diagram; the chord shows the labs.
import { doc, data, e, group, motion, story, step } from "@datars/sdk";
import { tree, dendrogram, icicle, sunburst, network, arcDiagram, matrix, chord } from "@datars/std";

// [name, lab, reports to, papers this year]
const people: [string, string, string, number][] = [
  ["Ada", "Optics", "Institute", 9], ["Bruno", "Optics", "Ada", 6], ["Chiara", "Optics", "Ada", 5], ["Dev", "Optics", "Ada", 3], ["Elin", "Optics", "Ada", 2], ["Farah", "Optics", "Ada", 3],
  ["Hana", "Materials", "Institute", 8], ["Ivo", "Materials", "Hana", 5], ["Jun", "Materials", "Hana", 4], ["Kemi", "Materials", "Hana", 6], ["Lars", "Materials", "Hana", 2], ["Mira", "Materials", "Hana", 3],
  ["Noor", "Theory", "Institute", 7], ["Oskar", "Theory", "Noor", 4], ["Pia", "Theory", "Noor", 5], ["Quinn", "Theory", "Noor", 3], ["Rafa", "Theory", "Noor", 3], ["Sade", "Theory", "Noor", 2],
];
const pairs: [string, string, number][] = [
  ["Ada", "Bruno", 6], ["Ada", "Chiara", 4], ["Ada", "Dev", 3], ["Ada", "Elin", 2], ["Bruno", "Chiara", 3], ["Bruno", "Farah", 2], ["Dev", "Elin", 1], ["Ada", "Farah", 1],
  ["Hana", "Ivo", 5], ["Hana", "Jun", 3], ["Hana", "Kemi", 4], ["Ivo", "Lars", 2], ["Jun", "Mira", 2], ["Kemi", "Mira", 3], ["Ivo", "Kemi", 1], ["Lars", "Mira", 1],
  ["Noor", "Oskar", 4], ["Noor", "Pia", 5], ["Noor", "Quinn", 2], ["Oskar", "Rafa", 3], ["Pia", "Sade", 2], ["Quinn", "Rafa", 1], ["Pia", "Quinn", 1],
  ["Ada", "Hana", 3], ["Chiara", "Kemi", 1], ["Noor", "Ada", 2], ["Pia", "Hana", 1], ["Oskar", "Ivo", 1],
];
const lab = Object.fromEntries(people.map((p) => [p[0], p[1]]));
// Papers between labs, both ways (a chord's flows).
const between: Record<string, number> = {};
for (const [a, b, n] of pairs) {
  if (lab[a] === lab[b]) continue;
  for (const [s, t] of [[lab[a], lab[b]], [lab[b], lab[a]]]) between[`${s}→${t}`] = (between[`${s}→${t}`] ?? 0) + n;
}
const flows = Object.entries(between).map(([k, n]) => [...k.split("→"), n] as [string, string, number]);
// Within each lab too, so each lab's arc is as long as all its papers.
for (const l of ["Optics", "Materials", "Theory"]) flows.push([l, l, pairs.filter(([a, b]) => lab[a] === l && lab[b] === l).reduce((s, p) => s + p[2], 0)]);

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const org = { data: "org", id: "name", parent: "boss" };
const net = { nodes: "people", links: "papers", id: "name", source: "a", target: "b", color: "lab", weight: "n" };

export default doc({
  title: "One institute, eight ways",
  description: "Eighteen researchers in three labs as a tree, a dendrogram, an icicle and a sunburst of who reports to whom, then as a network, an arc diagram and a matrix of who wrote papers with whom, and a chord of papers between the labs.",
  size: [640, 400],
  data: {
    org: data.values({ name: ["Institute", ...people.map((p) => p[0])], boss: [null, ...people.map((p) => p[2])], papers: [0, ...people.map((p) => p[3])] }, { key: "name" }),
    people: data.values({ name: people.map((p) => p[0]), lab: people.map((p) => p[1]) }, { key: "name" }),
    papers: data.values({ a: pairs.map((p) => p[0]), b: pairs.map((p) => p[1]), n: pairs.map((p) => p[2]) }, { key: ["a", "b"] }),
    labs: data.values({ from: flows.map((f) => f[0]), to: flows.map((f) => f[1]), n: flows.map((f) => f[2]) }, { key: ["from", "to"] }),
  },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.2 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      tree({ ...org }, at("tree")),
      dendrogram({ ...org, orientation: "radial" }, at("dendrogram")),
      icicle({ ...org, value: "papers" }, at("icicle")),
      sunburst({ ...org, value: "papers" }, at("sunburst")),
      network({ ...net }, at("network")),
      arcDiagram({ ...net }, at("arcs")),
      matrix({ ...net }, at("matrix")),
      chord({ data: "labs", source: "from", target: "to", value: "n" }, at("chord")),
    ],
  }),
  program: story({
    steps: [
      step("tree", { title: "tree()", text: "Who reports to whom, parents centred over their people." }),
      step("dendrogram", { title: "dendrogram()", text: "Every person on one circle, joined at their lab." }),
      step("icicle", { title: "icicle()", text: "Each lab as wide as its papers, each person their share." }),
      step("sunburst", { title: "sunburst()", text: "The same shares as rings." }),
      step("network", { title: "network()", text: "Who wrote papers with whom: a force layout, the same on every device." }),
      step("arcs", { title: "arcDiagram()", text: "The same people on a line, papers as arcs." }),
      step("matrix", { title: "matrix()", text: "A row and a column each, a cell per pair." }),
      step("chord", { title: "chord()", text: "Papers between the labs." }),
    ],
  }),
});
