// Your brand: a title, bars and a donut drawn only with theme tokens; the overview page lays a
// few brands over it with `setTokens()`. Illustrative figures.
import { data, doc, group, interactive, text } from "@datars/sdk";
import { bar, pie, plot } from "@datars/std";
import { PAD, SIZE } from "./_kit";

export default doc({
  id: "overview-theming",
  title: "Sales by region",
  description: "Sales in four regions as bars and as a donut.",
  size: SIZE,
  data: { sales: data.values({ region: ["North", "East", "South", "West"], m: [38, 27, 21, 14] }, { key: "region" }) },
  scene: group({ key: "root", layout: { type: "rows", gap: 6, padding: PAD }, children: [
    text("Sales by region", [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" } }),
    group({ key: "body", layout: { type: "columns", gap: 14 }, children: [
      { ...plot({ data: "sales", x: "region", y: "m", color: "region", children: [bar()] }, { key: "bars" }), size: { w: "56%" } },
      pie({ data: "sales", value: "m", category: "region", inner: 0.56, labels: false }, { key: "donut" }),
    ] }),
  ] }),
  program: interactive(),
});
