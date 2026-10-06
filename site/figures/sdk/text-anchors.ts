// Where a `text` sits relative to its `at` point: `style.align` across (start, middle, end) and
// `style.baseline` up and down (top, middle, alphabetic, bottom). The cross marks `at`.
import { doc, e, geom, group, shape, text, type Template } from "@datars/sdk";

const ALIGN = ["start", "middle", "end"] as const;
const BASELINE = ["top", "middle", "alphabetic", "bottom"] as const;
// On a phone the labels shorten to the value alone, and the sample to two letters.
const short = (name: string, v: string) => e(`sizeClass == "phone" ? ${JSON.stringify(`"${v}"`)} : ${JSON.stringify(`${name}: "${v}"`)}`);
const head = (s: ReturnType<typeof e>, key: string, align: "middle" | "end" = "middle") =>
  text(s, [e(align === "middle" ? "box.w / 2" : "box.w - 6"), e("box.h / 2")], { key, style: { font: "font.strong", size: "$size.label", ink: "$ink-2", align, baseline: "middle" } });

const sample = (a: string, b: string) => group({
  key: `${a}-${b}`,
  children: [
    shape(geom.rect({ x: 2, y: 2, w: e("box.w - 4"), h: e("box.h - 4"), r: 4 }), { key: "cell", fill: "$surface" }),
    shape(geom.segment({ x1: e("box.w / 2"), y1: 6, x2: e("box.w / 2"), y2: e("box.h - 6") }), { key: "v", stroke: { paint: "$rule", width: 1 } }),
    shape(geom.segment({ x1: 8, y1: e("box.h / 2"), x2: e("box.w - 8"), y2: e("box.h / 2") }), { key: "h", stroke: { paint: "$rule", width: 1 } }),
    text(e('sizeClass == "phone" ? "Agy" : "Agy 12"'), [e("box.w / 2"), e("box.h / 2")], { key: "text", style: { size: 17, ink: "$accent", align: a, baseline: b } }),
    shape(geom.circle({ cx: e("box.w / 2"), cy: e("box.h / 2"), r: 2.5 }), { key: "at", fill: "$ink" }),
  ],
});

const cells = (): Template[] => [
  group({ key: "corner" }),
  ...ALIGN.map((a) => head(short("align", a), `col-${a}`)),
  ...BASELINE.flatMap((b) => [head(short("baseline", b), `row-${b}`, "end"), ...ALIGN.map((a) => sample(a, b))]),
];

export default doc({
  id: "sdk-text-anchors",
  title: "Text alignment and baseline",
  description: "A three by four grid: each cell draws the same text at its centre point with one of three alignments and one of four baselines.",
  size: [640, 300],
  scene: group({ key: "root", layout: { type: "grid", columns: 4, gap: 4, padding: [4, 10, 6, 4] }, children: cells() }),
});
