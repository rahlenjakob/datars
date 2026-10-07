// A scene of your own, no recipe: circles packed by a table operation, a label in each that fits —
// plain nodes in the document. Every circle is keyed by its channel, as std/pie keys its slices,
// so the bubbles morph into the pie. The extensibility page's workbench lets the reader edit this
// part of the document as JSON.
import { doc, e, geom, group, motion, op, repeat, shape, story, step, text } from "@datars/sdk";
import { pie } from "@datars/std";
import { colour, visits } from "./_data";

const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

export default doc({
  title: "A hundred visitors as packed bubbles, then as a pie",
  description: "Six channels as circles packed by size, drawn from primitives without a recipe; then std's pie, each bubble morphing into its slice.",
  size: [640, 360],
  data: { visits },
  // Circle packing as a table operation: the engine computes cx, cy and r for the box.
  tables: { packed: { from: "visits", ops: [op.pack({ value: "visitors", width: e("box.w"), height: e("box.h"), padding: 3 })] } },
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.2 }),
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [14, 18, 10, 10] },
    scales: colour,
    children: [
      group({ key: "chart", when: e('state == "yours"'), children: [
        repeat("packed", shape(geom.circle({ cx: e("d.cx"), cy: e("d.cy"), r: e("d.r") }), {
          fill: e("scale.color(d.channel)"),
          semantics: { role: "datum", label: e("d.channel + ': ' + d.visitors") },
          pickable: true,
        })),
        repeat("packed", text(e("d.channel"), [e("d.cx"), e("d.cy")], {
          key: e("'name-' + d.channel"),
          when: e("d.r > 30"),
          style: { size: 13, weight: 600, align: "middle", baseline: "middle", ink: e("'on(' + scale.color(d.channel) + ')'") },
        })),
      ] }),
      pie({ data: "visits", category: "channel", value: "visitors" }, at("std")),
    ],
  }),
  program: story({
    steps: [
      step("yours", { title: "Your scene", text: "Circles packed by a table operation, labels where they fit." }),
      step("std", { title: "std/pie", text: "Each bubble becomes its slice." }),
    ],
  }),
});
