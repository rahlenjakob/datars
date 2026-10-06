// Your scene, then ours: a rose drawn by hand — one `geom.arc` per party, no recipe — morphs into
// the standard library's bar chart and pie, and back. The engine doesn't know (or care) which
// marks a recipe made: the party's petal, bar and slice all carry the party's key, so a matcher by
// key pairs them, and every change of chart is one planned transition.
// Vote share: Swedish Riksdag election 2022, rounded.
import { data, doc, e, geom, group, motion, repeat, shape, step, story, text, choreo } from "@datars/sdk";
import { bar, pie, plot } from "@datars/std";

const party = ["S", "SD", "M", "V", "C", "KD", "MP", "L"];
const share = [30.3, 20.5, 19.1, 6.8, 6.7, 5.3, 5.1, 4.6];
const keys = { S: { color: "#e8112d" }, SD: { color: "#dddd00" }, M: { color: "#1b49dd" }, V: { color: "#a01313" }, C: { color: "#009933" }, KD: { color: "#005ea8" }, MP: { color: "#83cf39" }, L: { color: "#3a8fd6" } };
const at = (state: string) => ({ key: "chart", when: e(`state == "${state}"`) });

// The rose: a petal per party, its length ∝ √share (so its area ∝ share), round a hub.
const cx = "box.w / 2", cy = "box.h / 2 + 12";
const R = "min(box.w, box.h - 40) * 0.46";
const step8 = (2 * Math.PI) / 8;
const a0 = `d.i * ${step8} + 0.04`, a1 = `(d.i + 1) * ${step8} - 0.04`, mid = `(d.i + 0.5) * ${step8}`;
const r1 = `${R} * (0.16 + 0.84 * sqrt(d.share / 30.3))`;
const rose = group({
  ...at("rose"),
  children: [
    repeat("votes", shape(geom.arc({ cx: e(cx), cy: e(cy), r0: e(`${R} * 0.16`), r1: e(r1), a0: e(a0), a1: e(a1) }), {
      key: e("d.party"), fill: e("key.color(d.party)"), semantics: { role: "datum", label: e("d.party + ': ' + format(d.share, '.1f') + '%'"), value: e("d.share") },
    })),
    repeat("votes", text(e("d.party"), [e(`${cx} + (${r1} + 11) * sin(${mid})`), e(`${cy} - (${r1} + 11) * cos(${mid})`)], {
      key: e("d.party + '-petal'"), style: { font: "font.strong", size: "$size.small", ink: "$ink", align: "middle", baseline: "middle" },
    })),
  ],
});

export default doc({
  id: "motion-custom",
  title: "A hand-drawn rose, then the standard library",
  description: "Swedish vote shares in 2022 as a rose of eight petals drawn with plain arcs, then as the standard library's bar chart and pie — each party's petal becoming its bar and its slice.",
  size: [640, 380],
  data: { votes: data.values({ party, share, i: party.map((_, i) => i) }, { key: "party" }) },
  keys,
  scene: group({
    key: "root",
    layout: { type: "stack", padding: [12, 16, 10, 12] },
    children: [
      rose,
      plot({ data: "votes", x: "party", y: "share", color: "party", title: "Vote share, 2022 (%)", children: [bar({ labels: true })] }, at("bars")),
      pie({ data: "votes", value: "share", category: "party", inner: 0.5 }, at("pie")),
    ],
  }),
  motion: motion({ select: { role: "datum" }, matcher: "by-key", duration: 1.3, choreo: choreo.stagger("value", 0.35) }),
  program: story({
    steps: [
      step("rose", { title: "Your scene", text: "Eight petals drawn with geom.arc — no recipe, no chart type." }),
      step("bars", { title: "std bar", text: "Each petal becomes its party's bar: the keys match." }),
      step("pie", { title: "std pie", text: "And the bars become slices, largest first." }),
    ],
  }),
});
