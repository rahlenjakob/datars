// hexbin: 200,000 generated trips binned into hexagons — distance against travel time. Each mode
// of transport is a ridge of its own speed; the hexagons show where the trips crowd, which a
// scatter of 200,000 dots would paint over. Binned once per data in the engine: every step reuses
// the bins, and a frame draws a few thousand hexagons, not 200,000 dots.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, hexbin } from "@datars/std";

// Seeded draws: the same trips on every platform, nothing stored. Mode: walk, bike, bus, car.
const trips = data.generate(200_000, {
  id: e("d.i"),
  m: e("rand(d.i, 1) < 0.18 ? 0 : rand(d.i, 1) < 0.4 ? 1 : rand(d.i, 1) < 0.68 ? 2 : 3"),
  km: e("round([1.3, 4.2, 8, 12][d.m] * exp(randn(d.i, 2) * [0.45, 0.5, 0.55, 0.6][d.m]), 2)"),
  // Minutes: distance at the mode's speed, plus waiting or parking, ±10 % (a slow day, a fast one).
  min: e("round((d.km / [5, 16, 20, 55][d.m] * 60 + [1, 2, 9, 5][d.m]) * exp(randn(d.i, 3) * 0.1), 1)"),
}, { keep: ["id", "km", "min"], key: "id" });

// The plot shows 0–40 km and 0–100 minutes; the bins cover exactly that (a few long trips run past it).
const extent = [0, 0, 40, 100];
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "trips", x: "km", y: "min", xType: "linear", zero: false, xDomain: [extent[0], extent[2]], yDomain: [extent[1], extent[3]], clip: true, grid: false, title: "200,000 trips: distance and travel time", xLabel: "km", yLabel: "minutes" } as const;

export default doc({
  title: "200,000 trips in hexagons",
  description: "Two hundred thousand generated trips binned into hexagons by distance and travel time, coloured by how many trips each holds; then larger hexagons; then hexagons sized by their count as well.",
  size: [640, 400],
  data: { trips },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [hexbin({ extent, name: "200,000 trips", unit: "trips" })] }, at("default")),
      plot({ ...frame, children: [hexbin({ extent, radius: 14, name: "200,000 trips", unit: "trips" })] }, at("radius")),
      plot({ ...frame, children: [hexbin({ extent, radius: 12, size: true, name: "200,000 trips", unit: "trips" })] }, at("size")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "hexbin()", text: "Every trip counted into a hexagon 8 px across the plot. Walking, cycling, bus and car each make a ridge of their own speed." }),
      step("radius", { title: "radius: 14", text: "Bigger hexagons: smoother, less detail. The trips are binned again once, in the engine." }),
      step("size", { title: "size: true", text: "Each hexagon also sized by its count, so the sparse edges shrink away." }),
    ],
  }),
});
