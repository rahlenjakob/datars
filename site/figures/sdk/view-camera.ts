// `view` with a fit camera: the content lives in its own units (a 1000 × 600 plane); the camera fits
// the marks named by a key-set signal (`fit: { keys: "=focus" }`) into the view and clips the rest.
// Steps change the keys and the camera flies: the plane itself, the towns of the north-east, two
// towns. `explore` lets the reader drag and zoom (the position is kept in the signals `cam.x`,
// `cam.y`, `cam.zoom`). Labels are pinned: they stay at their town at screen size (and, keyed
// `label-…`, don't count in the fit).
import { data, doc, e, geom, group, repeat, shape, signal, step, story, text, view } from "@datars/sdk";

// A seeded scatter of towns on the plane (integer maths, the same numbers every build).
let seed = 7;
const next = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const towns = Array.from({ length: 36 }, (_, i) => ({ id: `T${i + 1}`, x: Math.round(40 + next() * 920), y: Math.round(40 + next() * 520), size: Math.round(3 + next() * 9) }));

const northEast = towns.filter((t) => t.x >= 600 && t.y <= 300).map((t) => t.id);

export default doc({
  id: "sdk-view-camera",
  title: "A view and its camera",
  description: "Thirty-six towns on a 1000 by 600 plane inside a view: the camera fits the whole plane, then flies to the towns of the north-east, then to two towns — each picked by key.",
  size: [640, 300],
  data: { towns: data.values(towns, { key: "id" }) },
  signals: { focus: signal.keyset(["plane"]) },
  scene: group({
    key: "root",
    layout: { padding: [10, 12, 10, 12] },
    children: [
      view({
        key: "map",
        camera: {
          fit: { keys: "=focus" },
          padding: 32,
          explore: "cam",
          maxZoom: 12,
        },
        children: [
          shape(geom.rect({ x: 0, y: 0, w: 1000, h: 600, r: 12 }), { key: "plane", fill: "$surface", stroke: { paint: "$rule", width: 1, nonScaling: true } }),
          repeat({ count: 9 }, shape(geom.segment({ x1: e("(d.index + 1) * 100"), y1: 0, x2: e("(d.index + 1) * 100"), y2: 600 }), { key: e("'v' + d.index"), stroke: { paint: "$grid", width: 1, nonScaling: true } })),
          repeat({ count: 5 }, shape(geom.segment({ x1: 0, y1: e("(d.index + 1) * 100"), x2: 1000, y2: e("(d.index + 1) * 100") }), { key: e("'h' + d.index"), stroke: { paint: "$grid", width: 1, nonScaling: true } })),
          repeat("towns", shape(geom.circle({ cx: e("d.x"), cy: e("d.y"), r: e("d.size") }), { key: e("d.id"), fill: "$accent", opacity: 0.8, pickable: true, semantics: { role: "datum", label: e("d.id") } })),
          // Pinned: a label stays at its town but keeps its screen size at any zoom. Decluttered:
          // one that would land on an earlier label tries the other side of its town, else hides.
          group({ key: "labels", declutter: true, children: [repeat("towns", text(e("d.id"), [e("d.x"), e("d.y")], { key: e("'label-' + d.id"), pin: true, offset: [0, -14], halo: ["$surface", 2], style: { size: "$size.small", ink: "$ink-2", align: "middle", baseline: "bottom" } }))] }),
        ],
      }),
    ],
  }),
  program: story({ steps: [
    step("all", { set: { focus: ["plane"] } }),
    step("north-east", { set: { focus: northEast } }),
    step("two towns", { set: { focus: ["T32", "T9"] } }),
  ] }),
});
