// heatmap2d: 300,000 generated taxi pickups in a made-up city, counted into a grid of cells a few
// px across — where the pickups crowd (the centre, a station, an airport, a main road). Counted in
// one pass in the engine and kept: another grid is counted once more, and a frame draws the cells,
// not 300,000 dots.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, heatmap2d } from "@datars/std";

// Seeded draws: the same pickups on every platform. Where: 0 centre, 1 station, 2 airport, 3 the
// main road, 4 anywhere. Fare: a flag fall and a trip length (airport trips run long).
const pickups = data.generate(300_000, {
  id: e("d.i"),
  p: e("rand(d.i, 1) < 0.38 ? 0 : rand(d.i, 1) < 0.52 ? 1 : rand(d.i, 1) < 0.6 ? 2 : rand(d.i, 1) < 0.78 ? 3 : 4"),
  t: e("rand(d.i, 2) * 2 - 1"),
  x: e("round([0, 3, -7.5, d.t * 9, 0][d.p] + randn(d.i, 3) * [1.8, 0.35, 0.45, 0.22, 4.5][d.p], 3)"),
  y: e("round([0, -1.5, 5, d.t * 5 - 1, 0][d.p] + randn(d.i, 4) * [1.4, 0.35, 0.45, 0.22, 3.5][d.p], 3)"),
  fare: e("round(4 + 2.1 * ([4, 5, 22, 6, 5][d.p] * exp(randn(d.i, 5) * 0.5)), 1)"),
}, { keep: ["id", "x", "y", "fare"], key: "id" });

const extent = [-10, -7, 10, 7];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "pickups", x: "x", y: "y", xType: "linear", zero: false, xDomain: [extent[0], extent[2]], yDomain: [extent[1], extent[3]], clip: true, grid: false, title: "300,000 taxi pickups in a made-up city", xLabel: "km east", yLabel: "km north" } as const;

export default doc({
  title: "300,000 pickups on a grid",
  description: "Three hundred thousand generated taxi pickups counted into a grid of cells over the city, darker where more pickups happen; then a coarser grid; then each cell coloured by its mean fare instead of its count.",
  size: [640, 420],
  data: { pickups },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [heatmap2d({ extent, name: "300,000 pickups", unit: "pickups" })] }, at("default")),
      plot({ ...frame, children: [heatmap2d({ extent, cell: 12, name: "300,000 pickups", unit: "pickups" })] }, at("cell")),
      plot({ ...frame, children: [heatmap2d({ extent, cell: 12, value: "fare", format: "$,.0f", name: "300,000 pickups", unit: "pickups" })] }, at("fare")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "heatmap2d()", text: "Every pickup counted into a cell 6 px across: the centre, a station, the airport in the north-west and a main road stand out." }),
      step("cell", { title: "cell: 12", text: "Cells twice as wide: a quarter as many, smoother and coarser (the grid is counted once more, in the engine)." }),
      step("fare", { title: "value: \"fare\"", text: "The same cells coloured by their mean fare instead of their count: trips from the airport cost the most." }),
    ],
  }),
});
