// The data page's pipeline: one table, reshaped a step at a time with derived tables — filter,
// aggregate, top-n, a window — each step a state. The cities are keyed by (country, city) and the
// countries by country, so when cities are summed into their country the engine knows each
// country's bar is their parent: the cities gather into it. No transition code, only keys.
// NOTE: populations are rounded UN estimates of urban agglomerations (World Urbanization
// Prospects 2018, for 2020), millions. Approximate.
import { doc, data, e, group, motion, op, signal, step, story, table } from "@datars/sdk";
import { bar, plot } from "@datars/std";

const cities: [string, string, number][] = [
  ["Tokyo", "Japan", 37.4], ["Delhi", "India", 30.3], ["Shanghai", "China", 27.1], ["São Paulo", "Brazil", 22.0],
  ["Mexico City", "Mexico", 21.8], ["Dhaka", "Bangladesh", 21.0], ["Cairo", "Egypt", 20.9], ["Beijing", "China", 20.5],
  ["Mumbai", "India", 20.4], ["Osaka", "Japan", 19.2], ["New York", "United States", 18.8], ["Karachi", "Pakistan", 16.1],
  ["Chongqing", "China", 15.9], ["Istanbul", "Türkiye", 15.2], ["Buenos Aires", "Argentina", 15.2], ["Kolkata", "India", 14.9],
  ["Lagos", "Nigeria", 14.4], ["Manila", "Philippines", 13.9], ["Rio de Janeiro", "Brazil", 13.5], ["Los Angeles", "United States", 12.4],
];

const STEPS: [string, string, string][] = [
  ["rows", "data.values(…, { key: [\"country\", \"city\"] })", "The twenty largest cities, keyed by country and city."],
  ["filter", "op.filter(e(\"d.pop >= 15\"))", "Cities under 15 million leave."],
  ["aggregate", "op.aggregate([\"country\"], { pop: [\"sum\", \"pop\"] })", "Each country's cities gather into one bar: its key is their parent."],
  ["top", "op.top(6, \"pop\"), op.sort([\"pop\", \"desc\"])", "The six largest, ranked; the rest leave."],
  ["share", "op.window(\"share_of_total\", \"pop\", \"share\")", "Each as a share of the six together."],
];

export default doc({
  title: "A table, reshaped step by step",
  description: "Twenty of the world's largest cities as bars, then filtered to those over 15 million, summed into countries, cut to the top six, and shown as shares of the six — each step animating from the last by key.",
  size: [640, 500],
  data: {
    cities: data.values({ city: cities.map((c) => c[0]), country: cities.map((c) => c[1]), pop: cities.map((c) => c[2]) }, { key: ["country", "city"] }),
  },
  // Each table reads the one before it.
  tables: {
    filtered: table("cities", op.filter(e("d.pop >= 15"))),
    summed: table("filtered", op.aggregate(["country"], { pop: ["sum", "pop"] })),
    ranked: table("summed", op.top(6, "pop"), op.sort(["pop", "desc"])),
    shares: table("ranked", op.window("share_of_total", "pop", "share")),
  },
  signals: { stage: signal.str("rows") },
  motion: motion({ duration: 1.2, easing: "cubic-in-out", matcher: "by-key" }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 10] },
    children: [
      plot({ data: "cities", x: "pop", y: "city", xType: "linear", yType: "band", title: "Millions of people, by city", format: ".1~f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('stage == "rows"') }),
      plot({ data: "filtered", x: "pop", y: "city", xType: "linear", yType: "band", title: "Millions of people, by city", format: ".1~f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('stage == "filter"') }),
      plot({ data: "summed", x: "pop", y: "country", xType: "linear", yType: "band", title: "Millions of people in its largest cities, by country", format: ".1~f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('stage == "aggregate"') }),
      plot({ data: "ranked", x: "pop", y: "country", xType: "linear", yType: "band", title: "The six largest, ranked", format: ".1~f", children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart", when: e('stage == "top"') }),
      plot({ data: "shares", x: "share", y: "country", xType: "linear", yType: "band", title: "Share of the six", format: ".0%", children: [bar({ labels: true, fill: "$accent", format: ".0%" })] }, { key: "chart", when: e('stage == "share"') }),
    ],
  }),
  program: story({ steps: STEPS.map(([id, title, text]) => step(id, { set: { stage: id }, title, text })) }),
});
