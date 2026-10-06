// A chart file as a project writes one: the SDK and std from node_modules, a local module, a CSV
// next to it, a Google font in its theme, and its own say on publish mode.
import { doc, data, e, font, group, signal, step, story, theme } from "@datars/sdk";
import { bar, plot } from "@datars/std";
import { TITLE } from "./lib";

export const publish = false;

export default doc({
  title: TITLE,
  size: [640, 400],
  theme: theme({ name: "fixture", extends: "datars/neutral", tokens: { "font.title": font.google("Source Serif 4", { weight: 600 }) } }),
  data: { sales: data.url("./sales.csv", { key: "region", types: { amount: "num" } }) },
  signals: { sort: signal.str("region") },
  scene: group({ key: "root", children: [plot({ data: "sales", x: "region", y: "amount", children: [bar({})] }, { key: "chart", when: e("true") })] }),
  program: story({ steps: [step("first"), step("second")] }),
});
