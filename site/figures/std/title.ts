// title: a chart's title, with an optional subtitle and source line, above whatever it heads —
// here a plot of monthly rainfall. The title wraps to its box; the plot takes the rest.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { title, plot, bar } from "@datars/std";

const months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const mm = [250, 190, 195, 115, 105, 125, 140, 190, 265, 270, 275, 260]; // rounded normals
const at = (state: string) => ({ key: "title", size: { h: "auto" as const }, when: e(`state == "${state}"`) });

export default doc({
  title: "Rain in Bergen",
  description: "A title over a bar chart of monthly rainfall, then with a subtitle, then with a source line.",
  size: [640, 320],
  data: { rain: data.values({ month: months, mm }, { key: "month" }) },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] },
    children: [
      title({ text: "Rain in Bergen" }, at("default")),
      title({ text: "Rain in Bergen", subtitle: "Average rainfall per month, millimetres" }, at("subtitle")),
      title({ text: "Rain in Bergen", subtitle: "Average rainfall per month, millimetres", source: "Source: climate normals, rounded" }, at("source")),
      plot({ data: "rain", x: "month", y: "mm", children: [bar()] }, { key: "chart" }),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "title({ text })", text: "One line in the theme's title font." }),
      step("subtitle", { title: "subtitle", text: "A second line in the body size." }),
      step("source", { title: "source", text: "A small, muted third line for the credit." }),
    ],
  }),
});
