// Election night, replayed: a live source re-ranks the parties as the count comes in. The host
// refetches `count.json` on schedule (live.every) and each snapshot animates into place — keyed
// bars move to their new ranks. `datars serve` replays `count.json.d/` as the feed.
// SIMULATED snapshots, converging on the 2022 result. Not official data.
import { doc, data, e, group, text, interactive } from "@datars/sdk";
import { plot, bar } from "@datars/std";

export default doc({
  id: "election",
  title: "Election night, replayed",
  size: [720, 440],
  data: { count: data.url("count.json", { key: "party", live: { every: 2 } }) },
  tables: { ranked: { from: "count", ops: [{ op: "sort", by: [["share", "desc"]] }] } },
  keys: {
    S: { name: "Social Democrats", color: "#e8112d" }, SD: { name: "Sweden Democrats", color: "#dddd00" },
    M: { name: "Moderates", color: "#1b49dd" }, V: { name: "Left", color: "#a01313" }, C: { name: "Centre", color: "#009933" },
    KD: { name: "Christian Democrats", color: "#005ea8" }, MP: { name: "Greens", color: "#83cf39" }, L: { name: "Liberals", color: "#3a8fd6" },
  },
  scene: group({
    key: "root",
    layout: { type: "rows", gap: 8, padding: [16, 20, 12, 12] },
    children: [
      text(e('`Election night — ${format(table.max("count", "counted"), ".0f")}% of districts counted`'), [0, 0], {
        key: "headline", size: { h: "auto" }, style: { font: "font.title", size: "$size.title", ink: "$ink", baseline: "top" },
      }),
      plot({ data: "ranked", x: "share", y: "party", xType: "linear", yType: "band", color: "party", xDomain: [0, 35],
        title: "Vote share (%)", children: [bar({ labels: true, format: ".1f" })] }, { key: "chart" }),
    ],
  }),
  program: interactive(),
});
