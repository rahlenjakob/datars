// Under the hood, §3 (adapting to the device): the page's work share, frame by frame. The rule is
// the page's own (packages/web/src/index.ts, `noteFrame`): start at 0.5; a frame over 14 ms cuts
// the share to 70 % at once; while the running average of frame times is over 8 ms it drops 10 %
// a frame; below 4 ms (and a frame under 8) it grows 5 % a frame; never under 0.2 or over 0.75.
// The workload is made up: each frame costs 2 ms plus its pending work times the share — cheap
// before and after, 24 ms of work a frame during a camera flight.
import { data, doc, group } from "@datars/sdk";
import { line, plot, rule, span } from "@datars/std";

const frame: number[] = [], ms: number[] = [], share: number[] = [];
let s = 0.5, avg = 0;
for (let f = 0; f < 150; f++) {
  const load = f >= 30 && f < 95 ? 24 : 2;
  const cost = 2 + load * s;
  frame.push(f);
  ms.push(Math.round(cost * 10) / 10);
  share.push(Math.round(s * 1000) / 1000);
  // noteFrame(cost), as the page runs it after every moving frame:
  avg = avg ? avg * 0.85 + cost * 0.15 : cost;
  if (cost > 14) s = Math.max(0.2, s * 0.7);
  else if (avg > 8) s = Math.max(0.2, s * 0.9);
  else if (avg < 4 && cost < 8) s = Math.min(0.75, s * 1.05);
}

export default doc({
  id: "how-share",
  title: "The page's work share, frame by frame",
  description: "A made-up camera flight from frame 30 to 95: frame times jump to 20 ms, the page cuts its work share from 75% to its floor of 20% within a few frames, holds it while frames cost 4 to 8 ms, and raises it again once they are cheap.",
  size: [680, 320],
  data: { frames: data.values({ frame, ms, share }, { key: "frame" }) },
  // One plot, two axes (so both lines share the frame axis): time on the left, the share on the right.
  scene: group({
    key: "root",
    layout: { type: "rows", padding: [14, 14, 10, 14] },
    children: [
      plot({
        data: "frames", x: "frame", y: "ms", xType: "linear", xFormat: "d", xLabel: "frame", yDomain: [0, 24], yLabel: "ms per frame",
        right: { y: "share", domain: [0, 1], format: ".0%", label: "work share" },
        title: "A flight's frame times, and the work share the page answers with",
        subtitle: "Blue: main-thread time per frame (left). Green: the engines' share of their per-frame budgets (right), 20% to 75%.",
        children: [
          span({ from: 30, to: 95, label: "a camera flight" }),
          rule({ value: 8, label: "8 ms" }),
          line({ width: 2, curve: "step" }),
          line({ y: "share", yScale: "y2", width: 2, curve: "step", stroke: "$categorical[4]" }),
        ],
      }, { key: "plot" }),
    ],
  }),
});
