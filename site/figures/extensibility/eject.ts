// Eject: std/bar copied into recipes/bar.ts by `datars eject std/bar`, unedited, next to the
// standard one. The two states draw the same scene (`datars diff eject.ts --states yours,std`): the
// extensibility page's workbench starts from this and lets the reader edit the copy.
import { doc, e, group, motion, story, step } from "@datars/sdk";
import { plot, bar as stdBar } from "@datars/std";
import { bar } from "./recipes/bar";
import { colour, visits } from "./_data";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "visits", x: "channel", y: "visitors", color: "channel", title: "Where a hundred visitors came from" };

export default doc({
  title: "An ejected bar recipe beside the standard one",
  description: "Six channels as bars drawn by a copy of std/bar ejected into the project, then by std/bar itself: the same chart.",
  size: [640, 360],
  data: { visits },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.1 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    scales: colour,
    children: [
      plot({ ...frame, children: [bar({ labels: true })] }, at("yours")),
      plot({ ...frame, children: [stdBar({ labels: true })] }, at("std")),
    ],
  }),
  program: story({
    steps: [
      step("yours", { title: "recipes/bar.ts", text: "Your copy of the recipe." }),
      step("std", { title: "std/bar", text: "The standard one." }),
    ],
  }),
});
