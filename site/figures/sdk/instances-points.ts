// `instances`: 3,000 generated points in one node — a scatter, then the same points sorted into
// three blocks by group. Each point keeps its `instanceKey`, so it travels to its new place.
import { data, doc, e, group, instances, op, signal, step, story, text } from "@datars/sdk";

const grid = 'mode == "grid"';
// Scatter: three clouds. Grid: a block per group (about 1,000 points; room for 1,100), as many across as suit the
// block's shape, filled in row order.
const bw = "(box.w / 3 - 20)", bh = "(box.h - 44)";
const cols = `ceil(sqrt(1100 * ${bw} / ${bh}))`;
const cell = `min(${bw} / ${cols}, ${bh} / ceil(1100 / ${cols}))`;
const x = e(`${grid} ? 10 + d.g * box.w / 3 + ((d.r - 1) % ${cols}) * ${cell} : box.w * (0.5 + d.x / 9)`);
const y = e(`${grid} ? 34 + floor((d.r - 1) / ${cols}) * ${cell} : 30 + (box.h - 40) * (0.5 + d.y / 7)`);

export default doc({
  id: "sdk-instances-points",
  title: "Three thousand instances",
  description: "Three thousand generated points in three groups, first as three overlapping clouds, then sorted into a block per group.",
  size: [640, 280],
  data: {
    // Rows the engine generates: a group, then a seeded normal draw around that group's centre.
    pts: data.generate(3000, {
      id: e("d.i"),
      g: e("floor(rand(d.i, 1) * 3)"),
      x: e("(d.g - 1) * 1.6 + randn(d.i, 2) * 0.9"),
      y: e("(d.g == 1 ? -0.8 : 0.6) + randn(d.i, 3) * 0.8"),
    }, { key: "id" }),
  },
  tables: { ranked: { from: "pts", ops: [op.window("rank", "id", "r", { partition: ["g"], order: "id" })] } },
  signals: { mode: signal.str("scatter") },
  scene: group({
    key: "root",
    children: [
      instances({
        key: "points",
        from: "ranked",
        instanceKey: e("d.id"),
        x, y, r: 1.7,
        fill: e('d.g == 0 ? "$categorical[0]" : d.g == 1 ? "$categorical[1]" : "$categorical[2]"'),
        opacity: 0.8,
        label: e("`Point ${d.id}, group ${d.g + 1}`"),
      }),
      text(e('mode == "grid" ? "sorted by group: x and y read a rank column" : "3,000 rows, one instances node"'), [10, 8], { key: "note", style: { size: "$size.small", ink: "$muted", baseline: "top" } }),
    ],
  }),
  program: story({ steps: [step("scatter", { set: { mode: "scatter" } }), step("grid", { set: { mode: "grid" } })] }),
});
