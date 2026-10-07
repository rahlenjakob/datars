// A recipe from scratch (recipes/isotype.ts) beside std/bar: the isotype's dots are keyed
// (channel, unit i) and the bars (channel), so each row's dots gather into its bar and split out
// of it again. The extensibility page's
// workbench starts from this and lets the reader edit the recipe.
import { doc, e, group, motion, story, step } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { isotype } from "./recipes/isotype";
import { colour, points, visits } from "./_data";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A hundred visitors as an isotype, then as bars",
  description: "Six channels as rows of dots, one per percentage point, drawn by a custom recipe; then each row's dots gathered into std's bar.",
  size: [640, 360],
  data: { visits },
  tables: { points },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.3 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 18, 12, 10] },
    scales: colour,
    children: [
      isotype({ data: "points", category: "channel", value: "points" }, at("yours")),
      plot({ data: "points", x: "points", y: "channel", xType: "linear", yType: "band", color: "channel",
        children: [bar({ labels: true })] }, at("std")),
    ],
  }),
  program: story({
    steps: [
      step("yours", { title: "recipes/isotype.ts", text: "A dot per percentage point, a row per channel." }),
      step("std", { title: "std/bar", text: "Each row's dots gather into its bar." }),
    ],
  }),
});
