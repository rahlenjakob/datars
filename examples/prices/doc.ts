// Overview + detail, linked by a brush: drag across the year below and the detail above zooms to
// the selection, with its mean. Views link through signals (the brush writes `range.lo/hi/active`),
// no host code. The steps pin two selections so the transitions are tested; readers brush freely.
// Illustrative prices (a seasonal pattern), not market data.
import { doc, data, e, group, text, signal, story, step, brushed } from "@datars/sdk";
import { plot, line, area } from "@datars/std";

const day: number[] = [];
const price: number[] = [];
for (let d = 1; d <= 365; d++) {
  day.push(d);
  const season = 55 * Math.cos(((d - 20) / 365) * 2 * Math.PI);
  const weather = 16 * Math.sin(d * 0.61) + 9 * Math.sin(d * 0.17 + 1);
  price.push(Math.round((80 + season + weather) * 10) / 10);
}

export default doc({
  id: "prices",
  title: "Electricity spot price through a year",
  size: [720, 460],
  data: { prices: data.values({ day, price }, { key: "day" }) },
  tables: { detail: { from: "prices", ops: [{ op: "filter", expr: brushed("range", "d.day") }] } },
  signals: { range: signal.range() },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 10, padding: [16, 20, 12, 12] },
    children: [
      plot({ data: "detail", x: "day", y: "price", xType: "linear", zero: false, format: ".0f", title: "Spot price, öre/kWh",
        children: [area({ opacity: 0.18 }), line({ width: 2 })] }, { key: "detail" }),
      text(e("'Mean ' + format(table.mean('detail', 'price'), '.0f') + ' öre/kWh over ' + format(table.count('detail'), 'd') + ' days'"), [0, 12],
        { key: "summary", style: { size: "$size.label", ink: "$ink-2" }, size: { h: 16 } }),
      plot({ data: "prices", x: "day", y: "price", xType: "linear", format: ".0f", title: "Drag across the year to zoom in", brush: "range", xDomain: [1, 365],
        children: [area({ opacity: 0.3 }), line({ width: 1 })] }, { key: "overview", size: { h: 130 } }),
    ],
  }),
  program: story({
    steps: [
      step("year", { set: { "range.active": false }, title: "The whole year" }),
      step("winter", { set: { "range.active": true, "range.lo": 1, "range.hi": 59 }, title: "January–February: the dear months" }),
      step("summer", { set: { "range.active": true, "range.lo": 152, "range.hi": 243 }, title: "June–August: cheap and calm" }),
    ],
  }),
});
