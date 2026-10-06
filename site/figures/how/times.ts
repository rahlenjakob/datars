// Under the hood, the overview: the three places work happens, on one timeline (not to scale).
// Publishing runs once, on the author's machine, and takes what it takes. On the reader's device,
// idle time between steps prepares the next ones, and frames — only while something moves — do
// the least: evaluate the plan at t, fill tiles and point levels within budgets, flatten, draw.
import { doc, e, geom, group, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { label, line, narration, PHONE, SIZE } from "./_kit";

/** Wide: lane names on the left, the timeline beside them. Phone: names above, the timeline under
 * them, and each lane's words under that. */
type Cfg = { dx0: number; lane: number; top: number; phone: boolean };
const WIDE: Cfg = { dx0: 196, lane: 60, top: 0, phone: false };
const NARROW: Cfg = { dx0: 10, lane: 110, top: 22, phone: true };

const lit = (n: number) => e(`focus == ${n} ? 1 : 0.3`);
const TICKS = Array.from({ length: 16 }, (_, i) => 0.42 + i * 0.0105);

function timeline(c: Cfg): Template {
  /** x of a fraction of the device part of the timeline. */
  const dev = (f: number) => `${c.dx0} + ${f} * (box.w - ${c.dx0 + (c.phone ? 10 : 0)})`;
  const span = (f: number) => `(box.w - ${c.dx0}) * ${f}`;
  const cy = (n: number) => n * c.lane + c.top; // top of a lane's picture row
  const lane = (n: number, key: string, name: string, sub: string, body: Template[]): Template => group({
    key,
    opacity: lit(n),
    children: [
      shape(geom.rect({ x: 0, y: n * c.lane, w: e("box.w"), h: c.lane - 8, r: 6 }), { key: "bg", fill: e(`focus == ${n} ? "$accent@0.07" : "$surface"`), semantics: { role: "decoration" } }),
      label("name", name, [10, n * c.lane + 17], { strong: true }),
      label("sub", sub, c.phone ? [e(`10 + measure(${JSON.stringify(name)}, 12, 600) + 8`), n * c.lane + 17] : [10, n * c.lane + 36], { size: SIZE.small, ink: "$muted" }),
      ...body,
    ],
  });
  const chip = (key: string, x: string, y: number, w: string, s: string, ink: string) => group({ key, children: [
    shape(geom.rect({ x: e(x), y, w: e(w), h: 22, r: 4 }), { key: "box", fill: `${ink}@0.16`, stroke: { paint: ink, width: 1 }, semantics: { role: "decoration" } }),
    s ? text(s, [e(`${x} + ${w} / 2`), y + 11], { key: "t", style: { size: 11, ink: "$ink", align: "middle", baseline: "middle", font: "font.strong" } }) : null,
  ] });
  // A lane's words: beside its picture (wide) or under it (phone).
  const note = (key: string, s: string, n: number, x: number, w: number) => c.phone
    ? label(key, s, [10, n * c.lane + 70], { size: SIZE.small, ink: "$ink-2", maxWidth: e("box.w - 20"), baseline: "top" })
    : label(key, s, [e(dev(x)), cy(n) + 26], { size: SIZE.small, ink: "$ink-2", maxWidth: e(span(w)) });
  const axisY = 3 * c.lane + 4;
  return group({
    key: "timeline",
    children: [
      lane(0, "publish", "Publish time", "the author's machine, once", [
        chip("work", String(c.dx0), cy(0) + 15, "110", "datars publish", "$accent"),
        c.phone
          ? note("what", "expand recipes · fetch data · subset fonts · bake every state · draw the poster · cut map archives", 0, 0, 1)
          : label("what", "expand recipes · fetch data · subset fonts · bake every state · draw the poster · cut map archives", [c.dx0 + 124, cy(0) + 26], { size: SIZE.small, ink: "$ink-2", maxWidth: e(`box.w - ${c.dx0 + 124}`) }),
      ]),
      lane(1, "idle", "Idle time", "between steps, on the device", [
        ...[0.07, 0.12, 0.17, 0.7, 0.75].map((f, i) => chip(`s${i}`, dev(f), cy(1) + 15, span(0.04), "", "$categorical[4]")),
        note("what", "resolve and plan the next steps, warm their meshes, fetch their tiles", 1, 0.24, 0.42),
      ]),
      lane(2, "frame", "A frame", "16.7 ms at 60 Hz", [
        ...TICKS.map((f, i) => shape(geom.rect({ x: e(dev(f)), y: cy(2) + 13, w: 2, h: 24 }), { key: `f${i}`, fill: i === 8 ? "$ink" : "$categorical[2]", semantics: { role: "decoration" } })),
        note("what", "plan.at(t) · fill tiles and points within budgets · flatten · draw", 2, 0.62, 0.38),
      ]),
      group({ key: "axis", children: [
        // The device's clock: the publish lane above it happened before any reader, not on it.
        c.phone ? null : label("clock", "the reader's clock", [10, axisY], { size: SIZE.small, ink: "$muted", strong: true }),
        line("rule", e(String(c.dx0)), axisY, e(dev(1)), axisY, { ink: "$rule" }),
        ...(([[0, "page opens"], [0.42, "a step"], [0.58, "settled"], [1, "sleeps"]] as [number, string][]).map(([f, s], i) => group({ key: `e${i}`, children: [
          line("tick", e(dev(f)), axisY - 4, e(dev(f)), axisY + 4, { ink: "$rule" }),
          label("t", s, [e(dev(f)), axisY + 8], { size: SIZE.small, ink: "$muted", baseline: "top", align: f === 1 ? "end" : f === 0 ? "start" : f < 0.5 ? "end" : "start" }),
        ] }))),
      ] }),
    ],
  });
}

const layout = (c: Cfg) => group({
  key: c.phone ? "phone" : "wide",
  when: e(c.phone ? PHONE : `!(${PHONE})`),
  layout: { type: "rows", gap: 12, padding: c.phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: [group({ key: "main", children: [timeline(c)] }), narration(2, 3)],
});

export default doc({
  id: "how-times",
  title: "Three places work happens",
  description: "A timeline, not to scale: publishing once on the author's machine, then on the reader's device idle-time slices between steps and a burst of frames while a step's transition plays.",
  size: [794, 292],
  signals: { focus: signal.num(0) },
  scene: group({ key: "root", children: [layout(WIDE), layout(NARROW)] }),
  program: story({ steps: [
    step("publish", { set: { focus: 0 }, text: "Publish time: everything that doesn't depend on the reader runs once, on the author's machine, however long it takes." }),
    step("idle", { set: { focus: 1 }, text: "Idle time: while the reader reads, the engine prepares the steps next to this one, a slice of an idle callback at a time." }),
    step("frame", { set: { focus: 2 }, text: "A frame, only while something moves: no recipe and no author code, just the plan at t, budgeted fills, flattening and drawing." }),
  ] }),
});
