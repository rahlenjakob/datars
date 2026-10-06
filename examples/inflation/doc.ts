// Inflation (CPIF), annual %, Sweden — an area drawn on over time with an annotation.
// NOTE: numbers are illustrative placeholders, not official statistics.
import { doc, data, e, group } from "@datars/sdk";
import { plot, area, line, rule, annotate } from "@datars/std";

export default doc({
  id: "inflation",
  title: "Inflation (CPIF), annual %",
  size: [720, 380],
  data: { cpif: data.values({ year: [2015, 2016, 2017, 2018, 2019, 2020, 2021, 2022, 2023], value: [0.9, 1.4, 2.0, 2.1, 1.7, 0.5, 2.4, 7.7, 6.0] }, { key: "year" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 24, 12, 12] },
    children: [
      plot({
        data: "cpif", x: "year", y: "value", xType: "linear", title: "Inflation (CPIF), annual % — Sweden", format: ".0f",
        xDomain: [2015, 2023],
        children: [
          area({ opacity: 0.25 }),
          line({ width: 2.5 }),
          rule({ axis: "y", value: 2, label: "Target 2%" }),
          annotate({ x: e("scale.x(2022)"), y: e("scale.y(7.7)"), text: "2022 spike", dx: -40, dy: -24 }),
        ],
      }),
    ],
  }),
});
