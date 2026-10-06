// Five energy prices through 2020, one row each: the price at the start and the end of the year,
// the change, and a sparkline of every trading day between. Data: U.S. Energy Information
// Administration spot prices (public domain) — see ../SOURCES.md.
import { doc, data, e, group, op, repeat, text } from "@datars/sdk";
import { sparkline } from "@datars/std";
import { financial } from "../theme";
import src from "../data.json";

const names: Record<string, string> = { WTI: "U.S. crude", Brent: "Brent crude", Gasoline: "Gasoline", "Heating oil": "Heating oil", "Natural gas": "Natural gas" };

export default doc({
  id: "oil-2020/prices",
  title: "Five energy prices through 2020",
  description: "Spot prices of two crudes, gasoline, heating oil and natural gas through 2020: where each started and ended the year, and the path between.",
  size: [760, 320],
  theme: financial,
  data: { days: data.values(src.days, { key: ["series", "date"] }) },
  keys: Object.fromEntries(Object.keys(names).map((k) => [k, { name: names[k] }])),
  tables: {
    year: { from: "days", ops: [
      op.filter(e("d.j >= 257 && d.j <= 514")),
      op.derive("unit", e('d.series == "WTI" ? "WTI, $ a barrel" : d.series == "Brent" ? "North Sea, $ a barrel" :d.series == "Natural gas" ? "$ per million Btu" : "$ a gallon, New York"')),
    ] },
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [12, 16, 10, 12] },
    children: [
      text("Five energy prices through 2020", [0, 0], { key: "title", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top", maxWidth: e("box.w") }, semantics: { role: "title", label: "Five energy prices through 2020" } }),
      text("Spot prices on the first and last trading day of the year, and every day between", [0, 0], { key: "subtitle", size: { h: "auto" }, style: { size: "$size.body", ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }),
      group({
        key: "rows",
        layout: { type: "grid", columns: 1, gap: 6 },
        children: [repeat({ groups: "year", by: "series" }, group({
          layout: { type: "columns", gap: 12 },
          children: [
            group({ key: "name", size: { w: "30%" }, children: [
              text(e("key.name(d.series)"), [0, 4], { key: "label", style: { size: "$size.body", weight: 600, ink: "$ink", baseline: "top", maxWidth: e("box.w") } }),
              text(e("d.unit"), [0, 22], { key: "unit", style: { size: "$size.small", ink: "$muted", baseline: "top", maxWidth: e("box.w") } }),
            ] }),
            sparkline({ data: "@group", x: "date", y: "close", name: "close" }),
            group({ key: "change", size: { w: 104 }, children: [
              text(e('format(group.first("close"), ".2f") + " → " + format(group.last("close"), ".2f")'), [104, 4], { key: "ends", style: { size: "$size.small", ink: "$ink-2", baseline: "top", align: "end" } }),
              text(e('format(group.last("close") / group.first("close") - 1, "+.0%")'), [104, 21], { key: "pct", style: { size: "$size.body", weight: 600, ink: e('group.last("close") >= group.first("close") ? "$up" : "$down"'), baseline: "top", align: "end" } }),
            ] }),
          ],
        }))],
      }),
    ],
  }),
});
