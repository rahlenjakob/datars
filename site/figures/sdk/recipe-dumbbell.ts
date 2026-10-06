// A document using a local recipe (recipes/dumbbell.ts): imported by path, embedded as a package
// when the document is built.
import { data, doc, group } from "@datars/sdk";
import { dumbbell } from "./recipes/dumbbell";

export default doc({
  id: "sdk-recipe-dumbbell",
  title: "A recipe of your own",
  description: "Five regions' scores before and after, as a dumbbell chart drawn by a local recipe: a grey dot for before, a blue dot for after, sorted by after.",
  size: [640, 240],
  data: { scores: data.values({ region: ["Northern", "Southern", "Eastern", "Western", "Central"], before: [54, 61, 47, 70, 58], after: [68, 59, 63, 74, 71] }, { key: "region" }) },
  scene: group({ key: "root", layout: { padding: [14, 16, 10, 12] }, children: [dumbbell({ data: "scores", y: "region", a: "before", b: "after" }, { key: "chart" })] }),
});
