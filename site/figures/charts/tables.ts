// The "Tables and profiles" family on one fictional tea shop chain: its shops in a table drawn by
// the engine — inline bars, coloured cells, a sparkline per row, sortable by a click on a header —
// then three shops' ratings as radar shapes. (Made-up numbers.)
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { dataTable, radar } from "@datars/std";

const shops = ["Harbour", "Old Town", "Station", "Market Hall", "Riverside", "University", "Airport", "Garden"];
const city = ["Portsmouth", "York", "Leeds", "Bristol", "Norwich", "Oxford", "Luton", "Bath"];
const revenue = [412000, 356000, 331000, 298000, 244000, 219000, 187000, 142000];
const growth = [0.041, -0.022, 0.118, 0.063, -0.071, 0.152, 0.029, -0.034];
const orders = [18400, 16900, 21300, 12800, 11100, 14700, 9900, 6100];
// Twelve months per shop: a gentle seasonal wave that ends up where the year's growth says.
const monthly = shops.flatMap((shop, s) => Array.from({ length: 12 }, (_, m) => ({
  shop, month: m + 1, revenue: Math.round((revenue[s] / 12) * (1 - growth[s] / 2 + (growth[s] * m) / 11) * (1 + 0.012 * Math.sin((m + s) * 1.3))),
})));
const columns = [
  { field: "shop", label: "Shop" },
  { field: "city", label: "City", optional: true },
  { field: "revenue", label: "Revenue", format: "$,.0f", bar: true },
  { field: "growth", label: "Growth", format: "+.1%", color: true, stops: "$diverging~1 -0.2 · $diverging~0 0.2" },
  { label: "12 months", spark: { data: "monthly", x: "month", y: "revenue" }, optional: true },
  { field: "orders", label: "Orders", format: ",.0f", optional: true },
];
const aspects = ["Tea", "Service", "Price", "Space", "Food", "Location"];
const ratings: Record<string, number[]> = { Harbour: [5, 4, 3, 4, 3, 5], Station: [3, 4, 4, 2, 4, 5], University: [4, 3, 5, 5, 3, 3] };
const rated = Object.entries(ratings).flatMap(([shop, r]) => aspects.map((aspect, i) => ({ shop, aspect, score: r[i] })));
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const table = { data: "shops", key: "shop", columns, sortable: "sortBy" };

export default doc({
  title: "A tea shop chain's year, shop by shop",
  description: "Eight shops in a table — city, revenue with an inline bar, growth in coloured cells, a sparkline of the year and orders — then sorted by growth; then three shops' customer ratings on six aspects as radar shapes, and as outlines.",
  size: [640, 400],
  data: {
    shops: data.values({ shop: shops, city, revenue, growth, orders }, { key: "shop" }),
    monthly: data.values(monthly, { key: ["shop", "month"] }),
    ratings: data.values(rated, { key: ["shop", "aspect"] }),
  },
  signals: { sortBy: signal.str("revenue") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    children: [
      dataTable(table, { key: "chart", when: e('state == "table" || state == "sorted"') }),
      radar({ data: "ratings", axis: "aspect", value: "score", series: "shop", max: 5, levels: 5 }, at("radar")),
      radar({ data: "ratings", axis: "aspect", value: "score", series: "shop", max: 5, levels: 5, fill: false }, at("outline")),
    ],
  }),
  program: story({
    steps: [
      step("table", { set: { sortBy: "revenue" }, title: "dataTable()", text: "Bars, coloured cells and sparklines in the rows; click a header to sort." }),
      step("sorted", { set: { sortBy: "growth" }, title: "sortable: \"sortBy\"", text: "Sorted by growth: each row slides to its place." }),
      step("radar", { title: "radar()", text: "Three shops' ratings, a spoke per aspect." }),
      step("outline", { title: "radar({ fill: false })", text: "As outlines, on a fixed scale of 0 to 5." }),
    ],
  }),
});
