// dendrogram: a cluster layout of a parent-child table — every leaf on one line, each family
// joined where its members meet. The instruments of a symphony orchestra by family and kind.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { dendrogram } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
// [name, family]
const orchestra: [string, string | null][] = [
  ["Orchestra", null],
  ["Strings", "Orchestra"], ["Bowed", "Strings"], ["Violin", "Bowed"], ["Viola", "Bowed"], ["Cello", "Bowed"], ["Double bass", "Bowed"], ["Harp", "Strings"],
  ["Woodwinds", "Orchestra"], ["Flutes", "Woodwinds"], ["Flute", "Flutes"], ["Piccolo", "Flutes"], ["Reeds", "Woodwinds"], ["Oboe", "Reeds"], ["Clarinet", "Reeds"], ["Bassoon", "Reeds"],
  ["Brass", "Orchestra"], ["Trumpet", "Brass"], ["Horn", "Brass"], ["Trombone", "Brass"], ["Tuba", "Brass"],
  ["Percussion", "Orchestra"], ["Timpani", "Percussion"], ["Snare drum", "Percussion"], ["Cymbals", "Percussion"],
];

export default doc({
  title: "The instruments of an orchestra",
  description: "Nineteen instruments grouped by family as a dendrogram: every instrument on one line at the right, then with curved links, then as a ring around the orchestra.",
  size: [640, 380],
  data: { orchestra: data.values({ name: orchestra.map((o) => o[0]), family: orchestra.map((o) => o[1]) }, { key: "name" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 12, 12] },
    children: [
      dendrogram({ data: "orchestra", id: "name", parent: "family" }, at("default")),
      dendrogram({ data: "orchestra", id: "name", parent: "family", link: "curve" }, at("curve")),
      dendrogram({ data: "orchestra", id: "name", parent: "family", orientation: "radial" }, at("radial")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "dendrogram()", text: "Every instrument on one line; each family joins its members where they meet, by right-angled elbows." }),
      step("curve", { title: "link: \"curve\"", text: "The same joins as smooth curves." }),
      step("radial", { title: "orientation: \"radial\"", text: "The instruments on a ring, the orchestra in the middle." }),
    ],
  }),
});
