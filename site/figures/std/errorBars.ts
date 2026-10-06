// errorBars: each value with its interval — the value ± `error`, or `lo`/`hi` columns. A fictional
// poll: every party's support with its margin of error, alone, over bars, and as a range.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, bar, errorBars } from "@datars/std";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const party = ["Labour", "Conservatives", "Greens", "Liberals", "Centre", "Pirates"];
const share = [27.0, 28.9, 11.6, 4.1, 6.3, 4.4];
// A 95 % margin of error for a poll of 1,000: 1.96 · √(p(1 − p) / n), in points.
const moe = share.map((p) => Math.round(196 * Math.sqrt((p / 100) * (1 - p / 100) / 1000) * 10) / 10);
const lo = share.map((p, i) => Math.round((p - moe[i]) * 10) / 10);
const hi = share.map((p, i) => Math.round((p + moe[i]) * 10) / 10);
const frame = { data: "poll", title: "Support in a poll of 1,000 voters (%)" } as const;

export default doc({
  title: "A poll's results with their margins of error",
  description: "Six parties' support in a fictional poll as points with error bars, then as horizontal ranges from low and high columns, then over bars.",
  size: [640, 320],
  data: { poll: data.values({ party, share, moe, lo, hi }, { key: "party" }) },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, x: "party", y: "share", children: [errorBars({ error: "moe" }, { key: "errors" })] }, at("default")),
      plot({ ...frame, x: "share", y: "party", xType: "linear", yType: "band", children: [errorBars({ lo: "lo", hi: "hi", stroke: "$accent" }, { key: "errors" })] }, at("range")),
      plot({ ...frame, x: "party", y: "share", children: [bar({ fill: "$rule" }), errorBars({ error: "moe", r: 0 }, { key: "errors" })] }, at("bars")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "errorBars({ error: \"moe\" })", text: "Each party's share, ± its margin of error: Labour and the Conservatives overlap." }),
      step("range", { title: "lo: \"lo\", hi: \"hi\"", text: "Intervals from low and high columns, parties up the side." }),
      step("bars", { title: "r: 0, over bar()", text: "Without dots, over bars: the value axis reaches the intervals' ends." }),
    ],
  }),
});
