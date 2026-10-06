// Why datars, the page's opening figure. No chart library runs on every screen — D3 draws in a
// browser, Swift Charts on Apple's platforms, Android has none built in, and video and print mean
// redrawing it by hand — so one small chart is built once per screen, with five tools, and the
// copies drift apart (colours, corners, bar widths, type sizes, scales; a corrected number that
// reached some of them). Then the same chart as one document every host plays, and a republish
// that reaches all five at once. The numbers are made up.
import { doc, e, geom, group, motion, shape, signal, step, story, text, type Template } from "@datars/sdk";
import { arrow, narration, PHONE, SIZE } from "../how/_kit";

/** A screen the chart appears on, the tool its copy is made with, and how the copy differs. */
type Target = {
  key: string; name: string;
  /** The tool the copy is made with (`short`: on a phone). */
  file: string; short: string;
  /** The footer in each step: how the copy was made, what plays it, what the republish did. */
  made: string; host: string; after: string;
  /** The copy's look before: ink, bar corners, bar width (of its slot), value labels and their
   * size, the top of its scale, and whether it missed the corrected Q3. */
  ink: string; r: number; bw: number; values: boolean; valueSize?: number; max?: number; stale?: boolean;
};

const TARGETS: Target[] = [
  { key: "web", name: "Website", file: "D3", short: "D3", made: "web only", host: "<datars-view>", after: "updated, no deploy", ink: "$categorical[0]", r: 0, bw: 0.56, values: true },
  { key: "ios", name: "iPhone app", file: "Swift Charts", short: "Swift Charts", made: "Apple only", host: "DatarsKit", after: "updated, no app release", ink: "$categorical[1]", r: 7, bw: 0.8, values: false },
  { key: "android", name: "Android app", file: "a Kotlin library", short: "Kotlin", made: "Android only, and an old Q3", host: "DatarsView", after: "updated, no app release", ink: "$categorical[3]", r: 2, bw: 0.44, values: true, stale: true },
  { key: "video", name: "Video", file: "After Effects", short: "After Effects", made: "keyframed by hand", host: "datars video", after: "rendered again", ink: "$categorical[6]", r: 0, bw: 0.86, values: true, valueSize: 17 },
  { key: "print", name: "Print", file: "Illustrator", short: "Illustrator", made: "drawn by hand", host: "datars render", after: "rendered again", ink: "$muted", r: 0, bw: 0.3, values: true, max: 40 },
];

// Sales by quarter; Q4 arrives in the last step. The Android copy never got the corrected Q3.
// (Q4 is there all along, with no width, at the chart's right edge: arriving, it grows into its
// slot and its number counts up with it.)
const Q = ["Q1", "Q2", "Q3", "Q4"];
const V = [12, 15, 19, 24];
const STALE = 14;
const MAX = 26;
const DOC = "chart.ts";

/** One expression over the three steps: `f(phase)` for phase 0, 1 and 2. */
function sel(f: (p: number) => string | number): string {
  const [a, b, c] = [0, 1, 2].map((p) => String(f(p)));
  if (a === b && b === c) return a;
  return b === c ? `(phase == 0 ? ${a} : ${b})` : `(phase == 0 ? ${a} : phase == 1 ? ${b} : ${c})`;
}
const str = (s: string) => JSON.stringify(s);

/** A copy of the chart in one screen's panel. `phone`: a row with the chart on its right. */
function panel(t: Target, phone: boolean): Template {
  const x0 = phone ? "box.w * 0.46" : "14";
  const x1 = "box.w - 14";
  const top = phone ? "12" : "50";
  const base = phone ? "box.h - 20" : "box.h - 56";
  const size = phone ? SIZE.small : SIZE.label;

  const bars = Q.map((q, j) => {
    const waiting = (p: number) => j === 3 && p < 2;
    const at = (p: number) => {
      const n = p === 2 ? 4 : 3;
      const v = waiting(p) ? 0 : p === 0 && t.stale && j === 2 ? STALE : V[j];
      const bw = p === 0 ? t.bw : 0.56;
      const max = p === 0 ? t.max ?? MAX : MAX;
      const slotW = `((${x1}) - (${x0})) / ${n}`;
      const x = waiting(p) ? `(${x1})` : `(${x0} + ${slotW} * ${j} + ${slotW} * ${(1 - bw) / 2})`;
      const w = waiting(p) ? "0" : `(${slotW} * ${bw})`;
      const h = `(${v} * ((${base}) - (${top}) - 16) / ${max})`;
      const y = `(${base} - ${h})`;
      const vs = p === 0 && t.valueSize && !phone ? t.valueSize : size;
      return { x, y, w, h, label: [`${x} + ${w} / 2`, `${base} + ${phone ? 10 : 12}`], value: [`${x} + ${w} / 2`, `${y} - ${vs / 2 + 3}`], v, vs };
    };
    const g = (k: "x" | "y" | "w" | "h") => e(sel((p) => at(p)[k]));
    const valueShown = sel((p) => (waiting(p) || (p === 0 && !t.values) ? 0 : 1));
    return group({
      key: q,
      children: [
        shape(geom.rect({ x: g("x"), y: g("y"), w: g("w"), h: g("h"), r: e(sel((p) => (p === 0 ? t.r : 3))) }), {
          key: "bar",
          fill: e(sel((p) => str(p === 0 ? t.ink : "$categorical[0]"))),
          semantics: { role: "datum", label: e(`${str(`${t.name}, ${q}: `)} + ${sel((p) => (waiting(p) ? str("not in yet") : at(p).v))}`) },
        }),
        text(q, [e(sel((p) => at(p).label[0])), e(sel((p) => at(p).label[1]))], {
          key: "label", opacity: e(sel((p) => (waiting(p) ? 0 : 1))), style: { size, ink: "$muted", align: "middle", baseline: "middle" },
        }),
        text("", [e(sel((p) => at(p).value[0])), e(sel((p) => at(p).value[1]))], {
          key: "value",
          opacity: e(valueShown),
          number: { value: e(sel((p) => at(p).v)), format: ".0f" },
          style: { size: e(sel((p) => at(p).vs)), font: "font.strong", ink: t.stale && j === 2 ? e('phase == 0 ? "$negative" : "$ink-2"') : "$ink-2", align: "middle", baseline: "middle" },
        }),
      ],
    });
  });

  return group({
    key: t.key,
    children: [
      shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h"), r: 10 }), { key: "frame", fill: "$surface", stroke: { paint: "$grid", width: 1 }, semantics: { role: "decoration" } }),
      text(t.name, [phone ? 12 : 14, phone ? 18 : 22], { key: "name", style: { size: phone ? SIZE.label : SIZE.body, font: "font.strong", ink: "$ink", baseline: "middle" } }),
      text(e(sel((p) => str(p === 0 ? t.made : p === 1 ? t.host : t.after))), [phone ? 12 : 14, e(phone ? "box.h - 16" : "box.h - 18")], {
        key: "how",
        style: {
          size: SIZE.small,
          ink: e(sel((p) => str(p === 0 ? (t.stale ? "$negative" : "$muted") : p === 1 ? "$muted" : "$accent"))),
          baseline: "middle",
          maxWidth: e(phone ? "box.w * 0.46 - 18" : "box.w - 28"),
        },
      }),
      shape(geom.segment({ x1: e(x0), y1: e(base), x2: e(x1), y2: e(base) }), { key: "axis", stroke: { paint: "$rule", width: 1 }, semantics: { role: "decoration" } }),
      ...bars,
    ],
  });
}

/** Where the charts come from: five tools, one per screen — then one document, and a republish. */
function sources(phone: boolean): Template {
  const fs = phone ? SIZE.small : SIZE.label;
  const chipH = phone ? 24 : 28;
  const pad = phone ? 16 : 22;
  const docW = `(measure(${str(DOC)}, ${fs}) + ${pad})`;
  // Each copy's place: a column's centre (wide) or a row's (phone).
  const slot = (i: number) => (phone
    ? `(${i} * ((box.h - 32) / 5 + 8) + (box.h - 32) / 10)`
    : `(${i} * ((box.w - 64) / 5 + 16) + (box.w - 64) / 10)`);
  const chip = (key: string, label: string, w: string, cx: string, cy: string, o: { on?: boolean; opacity?: string }) => group({
    key,
    opacity: o.opacity ? e(o.opacity) : undefined,
    children: [
      shape(geom.rect({ x: e(phone ? "0" : `${cx} - ${w} / 2`), y: e(`${cy} - ${chipH / 2}`), w: e(w), h: chipH, r: 7 }), {
        key: "box",
        fill: o.on ? "$accent@0.12" : "$paper",
        stroke: { paint: o.on ? "$accent" : "$rule", width: o.on ? 1.5 : 1 },
        semantics: { role: "decoration" },
      }),
      text(label, [e(phone ? `${w} / 2` : cx), e(cy)], { key: "name", style: { size: fs, font: o.on ? "font.strong" : "font.body", ink: o.on ? "$ink" : "$ink-2", align: "middle", baseline: "middle" } }),
    ],
  });
  const tool = (t: Target) => (phone ? t.short : t.file);
  const files = TARGETS.map((t, i) => {
    const w = `(measure(${str(tool(t))}, ${fs}) + ${pad})`;
    // The five tools fade as they drift a quarter of the way toward the document (all the way,
    // they'd pile up mid-fade), and the arrows swing over to it.
    const toward = (mid: string) => `(${slot(i)} + (${mid} - ${slot(i)}) * 0.25)`;
    const cx = phone ? "0" : sel((p) => (p === 0 ? slot(i) : toward("box.w / 2")));
    const cy = phone ? sel((p) => (p === 0 ? slot(i) : toward("box.h / 2"))) : `${chipH / 2}`;
    return chip(`file-${t.key}`, tool(t), w, cx, cy, { opacity: "phase == 0 ? 1 : 0" });
  });
  const docCy = phone ? "box.h / 2" : `${chipH / 2}`;
  const links = TARGETS.map((t, i) => {
    const w = `(measure(${str(tool(t))}, ${fs}) + ${pad})`;
    const ink = e('phase == 2 ? "$accent" : "$rule"');
    const width = e("phase == 2 ? 1.5 : 1");
    if (phone) {
      return arrow(`link-${t.key}`, e(sel((p) => (p === 0 ? w : docW))), e(sel((p) => (p === 0 ? slot(i) : docCy))), e("box.w - 2"), e(slot(i)), { ink, width });
    }
    return arrow(`link-${t.key}`, e(sel((p) => (p === 0 ? slot(i) : "box.w / 2"))), chipH + 2, e(slot(i)), e("box.h - 2"), { ink, width });
  });
  return group({
    key: "sources",
    children: [
      ...links,
      ...files,
      chip("doc", DOC, docW, phone ? "0" : "box.w / 2", docCy, { on: true, opacity: "phase == 0 ? 0 : 1" }),
      // (On a phone the arrows leave no room beside the chip: the lit arrows say it.)
      phone ? null : text("republished", [e(`box.w / 2 + ${docW} / 2 + 10`), e(docCy)], {
        key: "republished", opacity: e("phase == 2 ? 1 : 0"),
        style: { size: SIZE.small, font: "font.strong", ink: "$accent", baseline: "middle" },
      }),
    ],
  });
}

/** The five tools leave before the document arrives, and come back after it (and the
 * "republished" beside it) has gone, so their names never fade across its name. */
const handover = ["wide", "phone"].flatMap((layout) => {
  const at = (k: string) => `root/${layout}/stage/from/sources/${k}`;
  return [
    ...TARGETS.map((t) => ({ when: { from: "today" }, select: { key: at(`file-${t.key}`) }, duration: 0.7 })),
    { when: { from: "today" }, select: { key: at("doc") }, delay: 0.55, duration: 0.65 },
    { when: { to: "today" }, select: { key: at("doc") }, duration: 0.45 },
    { when: { from: "changed once" }, select: { key: at("republished") }, duration: 0.3 },
    ...TARGETS.map((t) => ({ when: { to: "today" }, select: { key: at(`file-${t.key}`) }, delay: 0.35, duration: 0.85 })),
  ];
});

const wide = group({
  key: "wide",
  when: e(`!(${PHONE})`),
  layout: { type: "rows", gap: 10, padding: [16, 18, 8, 18] },
  children: [
    group({ key: "stage", layout: { type: "rows", gap: 0 }, children: [
      group({ key: "from", size: { h: 72 }, children: [sources(false)] }),
      group({ key: "panels", layout: { type: "columns", gap: 16 }, children: TARGETS.map((t) => panel(t, false)) }),
    ] }),
    narration(2, 3),
  ],
});

const phone = group({
  key: "phone",
  when: e(PHONE),
  layout: { type: "rows", gap: 10, padding: [12, 12, 6, 12] },
  children: [
    group({ key: "stage", layout: { type: "columns", gap: 0 }, children: [
      group({ key: "from", size: { w: 100 }, children: [sources(true)] }),
      group({ key: "panels", layout: { type: "rows", gap: 8 }, children: TARGETS.map((t) => panel(t, true)) }),
    ] }),
    narration(2, 3),
  ],
});

export default doc({
  id: "why-builds",
  title: "Five screens, five tools — or one document",
  description: "A small bar chart of sales by quarter on five screens. No one library runs on all of them, so it is built five times: with D3 for the website (web only), Swift Charts for the iPhone app (Apple only), a Kotlin library for the Android app, After Effects for the video and Illustrator for print — and the copies differ; the Android one still shows an old number. Then one datars document that every host plays, identically, and a republish that adds the fourth quarter to all five at once.",
  size: [1000, 380],
  signals: { phase: signal.num(0) },
  scene: group({ key: "root", children: [wide, phone] }),
  motion: motion(
    { duration: 1.2 },
    ...handover,
  ),
  program: story({ steps: [
    step("today", { set: { phase: 0 }, text: "No library runs on all five: D3 is web-only, Swift Charts is Apple-only, and video and print mean redrawing it by hand. So it's built five times." }),
    step("made once", { set: { phase: 1 }, text: "With datars: one document, played by the same engine in every host. The five copies become one chart, with the same pixels, labels and motion." }),
    step("changed once", { set: { phase: 2 }, text: "Q4 is in. Publish once: the site and both apps update without a deploy or an app release, and the video and print files render again." }),
  ] }),
});
