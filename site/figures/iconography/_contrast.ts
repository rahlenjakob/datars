// Not published (the leading underscore): the iconography page runs `datars lint` on it to show the
// icon-contrast check. A labelled icon in a pale ink stands for a word, so it must reach 3:1
// against what's behind it (WCAG 1.4.11); the decorative one beside it isn't held to that.
import { doc, icon, group } from "@datars/sdk";

export default doc({
  id: "iconography-contrast",
  title: "A pale warning",
  description: "A labelled warning icon in a pale yellow on white, and a decorative one.",
  size: [200, 80],
  scene: group({ key: "root", children: [
    icon("triangle-alert", [50, 40], { key: "delayed", size: 32, ink: "#f3e3a0", label: "Delayed" }),
    icon("sparkles", [140, 40], { key: "flourish", size: 32, ink: "#f3e3a0" }),
  ] }),
});
