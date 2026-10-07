// The data page's live source: a URL the chart asks for again every 1.5 seconds. On this page
// every request is answered by the page itself through `datarequest` — a simulated API with the
// latency and failure rate the reader sets — and each answer animates in by key: the regions
// re-rank, the total counts. Published, the bundle opens on feeds/orders.json (the snapshot beside this
// file); on a server of your own, the URL is your API and nothing on the page is needed.
// SIMULATED: invented orders, not anybody's real traffic.
import { doc, data, e, group, interactive, op, text } from "@datars/sdk";
import { bar, plot } from "@datars/std";

export default doc({
  title: "Orders a minute, live",
  description: "Simulated orders a minute in six Nordic regions, refreshed every 1.5 seconds from a live source; each refresh re-ranks the bars and counts the total up or down.",
  size: [640, 420],
  data: { orders: data.url("feeds/orders.json", { key: "region", live: { every: 1.5 } }) },
  tables: { ranked: { from: "orders", ops: [op.sort(["orders", "desc"])] } },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 4, padding: [14, 18, 10, 10] },
    children: [
      group({ key: "head", size: { h: 58 }, children: [
        text("", [0, 0], { key: "total", number: { value: e('table.sum("orders", "orders")'), format: ",.0f" }, style: { font: "font.title", size: 34, ink: "$ink", baseline: "top" }, semantics: { role: "datum", label: e('`${format(table.sum("orders", "orders"), ",.0f")} orders a minute`') } }),
        text(e('`orders a minute · refresh ${format(table.max("orders", "tick"), ",.0f")}`'), [0, 42], { key: "sub", style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
      ] }),
      plot({ data: "ranked", x: "orders", y: "region", xType: "linear", yType: "band", xDomain: [0, 250], children: [bar({ labels: true, fill: "$accent" })] }, { key: "chart" }),
    ],
  }),
  program: interactive(),
});
