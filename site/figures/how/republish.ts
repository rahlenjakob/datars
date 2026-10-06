// Under the hood, §2 (content addressing): publishing the votes example, then publishing it again
// after changing one step's narration. Chunks are named by the hash of their bytes, so the second
// publish writes only the 9 chunks whose bytes changed (the accessible text, both documents, the
// program, the pie step's scene, the four variant entries) and finds the other 7 already there;
// then it writes the manifest to a temporary name and renames it over c/votes, so a reader never
// sees a manifest whose chunks aren't there yet (crates/datars-cli/src/serve.rs, `publish`).
// The chunk lists are what `datars publish --json` reported for exactly this change (written 9,
// present 7).
import { doc, e, geom, group, motion, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { arrow, label, narration, PHONE, SIZE } from "./_kit";

type Chunk = { id: string; kind: string; what: string; v: 1 | 2 | 0 }; // v: only in 1, only in 2, or both (0)
const CHUNKS: Chunk[] = [
  { id: "poster", kind: "poster", what: "svg", v: 0 },
  { id: "a11y", kind: "a11y", what: "text", v: 1 },
  { id: "doc", kind: "doc", what: "source", v: 1 },
  { id: "docx", kind: "doc", what: "expanded", v: 1 },
  { id: "f400", kind: "font", what: "Inter 400", v: 0 },
  { id: "f600", kind: "font", what: "Inter 600", v: 0 },
  { id: "f700", kind: "font", what: "Inter 700", v: 0 },
  { id: "program", kind: "program", what: "steps", v: 1 },
  { id: "s-bars", kind: "scene", what: "bars", v: 0 },
  { id: "s-ranked", kind: "scene", what: "ranked", v: 0 },
  { id: "s-pie", kind: "scene", what: "pie", v: 1 },
  { id: "s-donut", kind: "scene", what: "donut", v: 0 },
  { id: "e0", kind: "entry", what: "T0", v: 1 },
  { id: "e1", kind: "entry", what: "T1", v: 1 },
  { id: "e2", kind: "entry", what: "T2", v: 1 },
  { id: "e3", kind: "entry", what: "T3", v: 1 },
  ...["a11y:text", "doc:source", "docx:expanded", "program:steps", "s-pie:pie", "e0:T0", "e1:T1", "e2:T2", "e3:T3"].map((s) => {
    const [id, what] = s.split(":");
    return { id: `${id}-2`, kind: CHUNKS_KIND(id), what, v: 2 as const };
  }),
];
function CHUNKS_KIND(id: string) {
  return id.startsWith("s-") ? "scene" : id.startsWith("e") ? "entry" : id === "docx" ? "doc" : id;
}

/** Wide: 5 columns of chunks. Phone: 4. */
const tile = (c: Chunk, i: number, cols: number): Template => {
  const col = i % cols, row = Math.floor(i / cols);
  const x = `${col} * (box.w / ${cols})`, w = `box.w / ${cols} - 6`;
  const y = 22 + row * 36;
  // Named by the manifest readers see: v1's chunks before the rename, v2's after it.
  const named = c.v === 0 ? "true" : c.v === 1 ? "phase < 2" : "phase == 2";
  return group({
    key: c.id,
    when: e(c.v === 2 ? "phase >= 1" : "true"),
    opacity: e(c.v === 2 ? "1" : `${named} ? 1 : 0.35`),
    children: [
      shape(geom.rect({ x: e(x), y, w: e(w), h: 30, r: 5 }), {
        key: "box",
        fill: e(`${named} ? "$accent@0.14" : "$surface"`),
        stroke: { paint: e(`${named} ? "$accent" : "$rule"`), width: 1 },
        semantics: { role: "decoration" },
      }),
      // Written but not yet named by the manifest readers see: dashed.
      c.v === 2 ? shape(geom.rect({ x: e(x), y, w: e(w), h: 30, r: 5 }), { key: "pending", when: e("phase == 1"), stroke: { paint: "$accent", width: 1, dash: [4, 3] }, fill: "$surface", semantics: { role: "decoration" } }) : null,
      text(c.kind, [e(`${x} + 8`), y + 10], { key: "kind", style: { size: 10, ink: "$ink", baseline: "middle", font: "font.strong" } }),
      text(c.what, [e(`${x} + 8`), y + 21], { key: "what", style: { size: 10, ink: "$muted", baseline: "middle" } }),
      // New in the second publish: a dot in the corner.
      c.v === 2 ? shape(geom.circle({ cx: e(`${x} + ${w} - 8`), cy: y + 8, r: 3 }), { key: "new", fill: "$accent", semantics: { role: "decoration" } }) : null,
    ],
  });
};

const chunks = (cols: number): Template => group({
  key: "chunks",
  children: [
    label("head", "chunks/", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    label("count", e("phase == 0 ? '16 written' : '9 new · 7 already there'"), [e("box.w - 6"), 0], { size: SIZE.small, ink: "$ink-2", baseline: "top", align: "end" }),
    shape(geom.circle({ cx: e("box.w - 14 - measure('9 new · 7 already there', 11)"), cy: 7, r: 3 }), { key: "new-key", when: e("phase >= 1"), fill: "$accent", semantics: { role: "decoration" } }),
    ...CHUNKS.map((c, i) => tile(c, i, cols)),
  ],
});

/** The manifest folder: c/votes, and the temporary file the second publish writes first. */
const manifests = (): Template => group({
  key: "c",
  children: [
    label("head", "c/", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    label("reader", "a reader asks for c/votes", [0, 30], { size: SIZE.small, ink: "$ink-2" }),
    arrow("ask", 60, 40, 60, 60, { ink: "$ink-2" }),
    // v1 at c/votes until the rename; v2 written at c/.votes.tmp, then renamed into its place.
    group({ key: "v1", when: e("phase < 2"), children: [
      shape(geom.rect({ x: 0, y: 64, w: e("box.w"), h: 44, r: 6 }), { key: "box", fill: "$accent@0.14", stroke: { paint: "$accent", width: 1.5 }, semantics: { role: "decoration" } }),
      label("name", "votes", [10, 78], { strong: true }),
      label("what", "names 16 chunks", [10, 96], { size: SIZE.small, ink: "$ink-2" }),
    ] }),
    group({ key: "v2", when: e("phase >= 1"), children: [
      shape(geom.rect({ x: 0, y: e("phase == 1 ? 124 : 64"), w: e("box.w"), h: 44, r: 6 }), { key: "box", fill: "$accent@0.14", stroke: { paint: "$accent", width: 1.5 }, opacity: e("phase == 1 ? 0 : 1"), semantics: { role: "decoration" } }),
      shape(geom.rect({ x: 0, y: e("phase == 1 ? 124 : 64"), w: e("box.w"), h: 44, r: 6 }), { key: "tmp", fill: "$surface", stroke: { paint: "$accent", width: 1.5, dash: [4, 3] }, opacity: e("phase == 1 ? 1 : 0"), semantics: { role: "decoration" } }),
      label("name", e("phase == 1 ? '.votes.tmp' : 'votes'"), [10, e("phase == 1 ? 138 : 78")], { strong: true }),
      label("what", "names 16 chunks: 7 old, 9 new", [10, e("phase == 1 ? 156 : 96")], { size: SIZE.small, ink: "$ink-2" }),
    ] }),
    label("cache", e("phase == 2 ? 'renamed: readers switch at once' : phase == 1 ? 'written last, under another name' : 'the only file that changes'"), [0, 186], { size: SIZE.small, ink: "$muted", maxWidth: e("box.w") }),
  ],
});

const layout = (phone: boolean) => group({
  key: phone ? "phone" : "wide",
  when: e(phone ? PHONE : `!(${PHONE})`),
  layout: { type: "rows", gap: 12, padding: phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: [
    group({ key: "main", layout: phone ? { type: "rows", gap: 10 } : { type: "columns", gap: 28 }, children: [
      group({ key: "c-at", size: phone ? { h: 200 } : { w: 180 }, children: [manifests()] }),
      group({ key: "chunks-at", children: [chunks(phone ? 4 : 5)] }),
    ] }),
    narration(2, 3),
  ],
});

export default doc({
  id: "how-republish",
  title: "Republishing, chunk by chunk",
  description: "The votes chart published as a manifest naming 16 content-addressed chunks, then republished after one narration changed: 9 new chunks are written beside the old ones, 7 are already there, and the manifest is renamed into place last.",
  size: [794, 292],
  signals: { phase: signal.num(0) },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  motion: motion({ duration: 0.9 }),
  program: story({ steps: [
    step("published", { set: { phase: 0 }, text: "Published: c/votes names 16 chunks, each stored under the hash of its bytes." }),
    step("chunks", { set: { phase: 1 }, text: "One step's narration changed. The new publish writes the 9 chunks whose bytes changed, skips the 7 that are there, and writes its manifest under a temporary name." }),
    step("renamed", { set: { phase: 2 }, text: "Last, the manifest is renamed over c/votes. Every chunk it names was already there; the old ones stay, immutable and cacheable forever." }),
  ] }),
});
