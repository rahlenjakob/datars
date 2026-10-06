// dataTable: a data table drawn by the engine — text left, numbers right in their formats, an inline
// bar, coloured cells and a sparkline per row. Rows are keyed by shop, so a new sort slides them.
// A fictional tea shop chain's year, shop by shop.
import { doc, data, e, group, signal, story, step } from "@datars/sdk";
import { dataTable } from "@datars/std";

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
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A tea shop chain's year, shop by shop",
  description: "Eight shops in a table — city, revenue with an inline bar, growth in cells from red to blue, a sparkline of the year and orders — as they come; then sorted by revenue; then by growth, the column a reader can click to sort by.",
  size: [640, 320],
  data: {
    shops: data.values({ shop: shops, city, revenue, growth, orders }, { key: "shop" }),
    monthly: data.values(monthly, { key: ["shop", "month"] }),
  },
  signals: { sortBy: signal.str("growth") },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      dataTable({ data: "shops", key: "shop", columns }, at("default")),
      dataTable({ data: "shops", key: "shop", columns, sort: "revenue" }, at("sort")),
      dataTable({ data: "shops", key: "shop", columns, sortable: "sortBy" }, at("sortable")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "dataTable({ data, key, columns })", text: "Text left, numbers right; a bar for revenue, colour for growth, a line for the year. Rows as they come." }),
      step("sort", { title: "sort: \"revenue\"", text: "Largest first: the rows slide to their places." }),
      step("sortable", { title: "sortable: \"sortBy\"", text: "Click a header to sort by it, again to reverse the sort; the signal holds the column (here growth)." }),
    ],
  }),
});
