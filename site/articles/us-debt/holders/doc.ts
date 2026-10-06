// Who the United States owes: the public debt split into what's held by the public (people,
// companies, funds, foreign governments and the Federal Reserve) and what's held in government
// accounts (the Social Security and other trust funds), at the end of each month since September
// 1997, when the Treasury's daily figures begin to show the split. Data: Debt to the Penny
// (../data.json; ../SOURCES.md).
import { doc, data, e, group, op, text } from "@datars/sdk";
import { area, plot } from "@datars/std";
import { fiscal } from "../theme";
import src from "../data.json";

const last = src.months[src.months.length - 1];
// Each band labelled beside its end, in its colour: a name over its latest value.
const label = (key: string, name: string, value: number, mid: number, ink: string) => [
  text(name, [e(`scale.x(${last.x}) + 10`), e(`scale.y(${mid}) - 2`)], { key: `${key}-name`, style: { size: 12, weight: 600, ink } }),
  text(`$${value.toFixed(1)} trillion`, [e(`scale.x(${last.x}) + 10`), e(`scale.y(${mid}) + 13`)], { key: `${key}-value`, style: { size: 12, ink } }),
];

const chart = (labelled: boolean) => plot({
  data: "stacked",
  x: "x",
  y: "y1",
  color: "series",
  xType: "linear",
  xFormat: "d",
  title: "Who is owed, trillions of dollars",
  subtitle: "At the end of each month",
  format: ",.0f",
  prefix: "$",
  xDomain: [src.holders[0].x, last.x],
  nice: false,
  legend: !labelled,
  children: [
    area({ x: "x", y: "y1", y0: "y0", color: "series", curve: "linear", opacity: 0.9 }),
    ...(labelled
      ? [
          ...label("public", "Held by the public", last.public ?? 0, (last.public ?? 0) / 2, "$accent"),
          ...label("government", "Government accounts", last.intragov ?? 0, (last.public ?? 0) + (last.intragov ?? 0) / 2, "$muted"),
        ]
      : []),
  ],
}, { key: "chart" });

export default doc({
  id: "us-debt/holders",
  title: "Who holds the U.S. public debt",
  description: "The public debt held by the public and in government accounts, at the end of each month since September 1997, in trillions of dollars.",
  size: [960, 480],
  locale: "en-US",
  theme: fiscal,
  data: { holders: data.values(src.holders, { key: ["series", "x"] }) },
  keys: {
    "Held by the public": { color: "$accent" },
    "Held by government accounts": { color: "$muted" },
  },
  tables: {
    stacked: { from: "holders", ops: [op.stack({ x: "x", series: "series", value: "y", as: ["y0", "y1"] })] },
  },
  scene: group({
    key: "root",
    layout: { type: "stack" },
    children: [
      // Wide: each band labelled beside its end, with room for the labels at the right.
      group({ key: "wide", when: e("width >= 560"), layout: { type: "stack", padding: [16, 150, 12, 12] }, children: [chart(true)] }),
      // A phone: the plot takes the width, a legend under it names the bands.
      group({ key: "narrow", when: e("width < 560"), layout: { type: "stack", padding: [16, 14, 12, 12] }, children: [chart(false)] }),
    ],
  }),
});
