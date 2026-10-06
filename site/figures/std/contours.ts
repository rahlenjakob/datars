// contours: the shape of a crowd of 150,000 generated geyser eruptions — how long each lasted
// against how long until the next. A density of the rows is cut into nested levels, each holding
// a further fifth of them: two crowds, short and long eruptions. Computed once per data in the
// engine; a frame draws a few paths, not 150,000 dots.
import { doc, data, e, group, story, step } from "@datars/sdk";
import { plot, contours } from "@datars/std";

// Seeded draws: a third short eruptions, the rest long; the longer the eruption, the longer the
// wait for the next.
const eruptions = data.generate(150_000, {
  id: e("d.i"),
  long: e("rand(d.i, 1) < 0.35 ? 0 : 1"),
  minutes: e("round(d.long ? 4.3 + randn(d.i, 2) * 0.38 : 2 + randn(d.i, 2) * 0.26, 3)"),
  wait: e("round(33 + 10.8 * d.minutes + randn(d.i, 3) * 5.5, 2)"),
}, { keep: ["id", "minutes", "wait"], key: "id" });

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });
const frame = { data: "eruptions", x: "minutes", y: "wait", xType: "linear", zero: false, xDomain: [1, 6], yDomain: [40, 110], grid: false, title: "150,000 eruptions of a geyser", xLabel: "eruption, minutes", yLabel: "wait until the next, minutes" } as const;
const common = { name: "150,000 eruptions", unit: "eruptions" };

export default doc({
  title: "The shape of 150,000 eruptions",
  description: "One hundred and fifty thousand generated geyser eruptions as density contours of eruption length against the wait until the next: filled bands, each holding a further fifth of the eruptions; then the same levels as lines over a sample of the eruptions themselves; then less smoothing and more levels.",
  size: [640, 400],
  data: { eruptions },
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [16, 20, 12, 12] },
    children: [
      plot({ ...frame, children: [contours(common)] }, at("default")),
      plot({ ...frame, children: [contours({ ...common, style: "lines", dots: 3000 })] }, at("lines")),
      plot({ ...frame, children: [contours({ ...common, bandwidth: 7, levels: 6 })] }, at("detail")),
    ],
  }),
  program: story({
    steps: [
      step("default", { title: "contours()", text: "Four levels: the innermost band holds the densest fifth of the eruptions, each band around it a further fifth. Two crowds: short eruptions and long ones." }),
      step("lines", { title: "style: \"lines\", dots: 3000", text: "The same levels as lines, over a seeded sample of 3,000 of the eruptions." }),
      step("detail", { title: "bandwidth: 7, levels: 6", text: "Half the smoothing and six levels: more of the crowds' shape, and a little more noise." }),
    ],
  }),
});
