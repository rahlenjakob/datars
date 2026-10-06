// Labels in another script: Hebrew city names, drawn right to left from a font the document
// brings (Noto Sans Hebrew, OFL). The font is a source like any other — hosts fetch it with the
// data, bundles ship it by hash — and its glyphs join every label's fallback chain.
// Populations are rounded and illustrative (Israel's Central Bureau of Statistics, ~2023).
import { doc, data, group } from "@datars/sdk";
import { plot, bar } from "@datars/std";

export default doc({
  id: "hebrew",
  title: "Israel's largest cities",
  size: [720, 420],
  data: {
    hebrew: data.font("../../assets/fonts/NotoSansHebrew-Regular.ttf"),
    cities: data.values({
      city: ["ירושלים", "תל אביב–יפו", "חיפה", "ראשון לציון", "פתח תקווה"],
      pop: [981, 475, 290, 260, 256],
    }, { key: "city" }),
  },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "cities", x: "pop", y: "city", xType: "linear", yType: "band", title: "Population, thousands — הערים הגדולות", format: ",.0f",
        children: [bar({ labels: true, format: ",.0f", fill: "$accent" })] }),
    ],
  }),
});
