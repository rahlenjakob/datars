// Under the hood, §5 (timing): choreography gives every element a window [start, end] of the
// plan's time. Each step here is reached with a different choreography — everyone together,
// staggered in key order over half the plan, or phased (exits, then updates, then enters) — and
// the chart on the right draws the windows it gives, with a playhead running through plan time.
// The windows are the formulas in crates/datars-motion/src/choreo.rs.
import { choreo, data, doc, e, geom, group, motion, op, repeat, shape, signal, step, story, type Template } from "@datars/sdk";
import { label, line, narration, PHONE, SIZE, WIDE } from "./_kit";

const K = ["a", "b", "c", "d", "e", "f", "g"];
const H = {
  together: [3, 5, 2, 6, 4, 5, 0],
  stagger: [6, 2, 5, 3, 6, 2, 0],
  phased: [0, 4, 6, 3, 5, 2, 5],
};
const DURATION = 1.6;
const SPREAD = 0.5;
const PHASED = [0.3, 0.5, 0.2] as const;
const W = 230; // the timing chart's width: the playhead's entering ghost starts this far left

// Each element's window in each step, as choreo.rs computes it.
const win = K.map((_, i) => {
  const o = i / 5; // stagger rank over a…f, data order
  const [sx, su] = PHASED;
  return {
    together: i < 6 ? [0, 1] : null,
    stagger: i < 6 ? [o * SPREAD, o * SPREAD + (1 - SPREAD)] : null,
    phased: i === 0 ? [0, sx] : i === 6 ? [sx + su, 1] : [sx, sx + su],
  };
});

const slotX = "(d.i - (phase == 2 ? 1 : 0)) * box.w / 6 + box.w / 12";
const barW = "min(40, box.w / 6 - 14)";
const base = "box.h - 22";
const unit = `(${base} - 10) / 6`;

const bars = (): Template => group({
  key: "bars",
  children: [
    line("axis", 0, e(base), e("box.w"), e(base), { ink: "$rule" }),
    repeat("shown", shape(geom.rect({ x: e(`${slotX} - ${barW} / 2`), y: e(`${base} - d.h * ${unit}`), w: e(barW), h: e(`d.h * ${unit}`), r: 2 }), {
      key: e("d.k"), fill: e("'$categorical[' + d.i + ']'"), semantics: { role: "datum", label: e("d.k + ': ' + d.h") },
    })),
    repeat("shown", label(e("d.k + '-name'"), e("d.k"), [e(slotX), e(`${base} + 7`)], { size: SIZE.small, ink: "$ink-2", align: "middle", baseline: "top", strong: true })),
  ],
});

const ROW = 22;
const timing = (): Template => group({
  key: "timing",
  children: [
    label("head", "WINDOWS IN PLAN TIME", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    group({ key: "chart", transform: { translate: [18, 24] }, children: [
      // One window per element that moves in this step, in its colour, labelled by its phase.
      repeat("windows", group({ key: e("d.k"), children: [
        label("name", e("d.k"), [-10, e(`d.row * ${ROW} + ${ROW / 2}`)], { size: SIZE.small, ink: "$ink-2", align: "middle", strong: true }),
        shape(geom.rect({ x: 0, y: e(`d.row * ${ROW} + 5`), w: W, h: ROW - 10, r: 3 }), { key: "track", fill: "$surface", semantics: { role: "decoration" } }),
        shape(geom.rect({ x: e(`d.s * ${W}`), y: e(`d.row * ${ROW} + 5`), w: e(`(d.e - d.s) * ${W}`), h: ROW - 10, r: 3 }), { key: "win", fill: e("'$categorical[' + d.i + ']'"), semantics: { role: "decoration" } }),
        label("what", e("d.what"), [e(`d.e * ${W} - 4`), e(`d.row * ${ROW} + ${ROW / 2}`)], { size: 10, ink: e("'on($categorical[' + d.i + '])'"), align: "end", strong: true }, { when: e("phase == 2") }),
      ] })),
      // Plan time: 0 to 1 below the rows.
      line("t-axis", 0, e(`rows * ${ROW} + 6`), W, e(`rows * ${ROW} + 6`), { ink: "$rule" }),
      label("t0", "0", [0, e(`rows * ${ROW} + 12`)], { size: SIZE.small, ink: "$muted", align: "middle", baseline: "top" }),
      label("t-mid", `plan time t  ·  ${DURATION} s`, [W / 2, e(`rows * ${ROW} + 12`)], { size: SIZE.small, ink: "$muted", align: "middle", baseline: "top" }),
      label("t1", "1", [W, e(`rows * ${ROW} + 12`)], { size: SIZE.small, ink: "$muted", align: "middle", baseline: "top" }),
      // The playhead: a new one enters with every step, from t = 0, and runs to t = 1 in step with
      // the plan (its entering ghost is W px to the left; its easing is linear).
      group({ key: "playhead", children: [
        line(e("'at-' + state"), W, 0, W, e(`rows * ${ROW} + 6`), { ink: "$ink", width: 1.5 }),
      ] }),
    ] }),
  ],
});

const layout = (phone: boolean) => group({
  key: phone ? "phone" : "wide",
  when: e(phone ? PHONE : WIDE),
  layout: { type: "rows", gap: 12, padding: phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: phone
    ? [bars(), group({ key: "side", size: { h: 212 }, children: [timing()] }), narration(3)]
    : [group({ key: "content", layout: { type: "columns", gap: 32 }, children: [bars(), group({ key: "side", size: { w: W + 30 }, children: [timing()] })] }), narration(2)],
});

const stepNames = ["together", "stagger", "phased"] as const;
const rows: Record<string, unknown[]> = { k: [], i: [], step: [], s: [], e: [], row: [], what: [] };
stepNames.forEach((st) => {
  let row = 0;
  K.forEach((k, i) => {
    const w = win[i][st];
    if (!w) return;
    rows.k.push(k); rows.i.push(i); rows.step.push(st); rows.s.push(w[0]); rows.e.push(w[1]); rows.row.push(row++);
    rows.what.push(i === 0 ? "exit" : i === 6 ? "enter" : "update");
  });
});

export default doc({
  id: "how-timing",
  title: "Choreography: windows in plan time",
  description: "Six bars change height three ways: all together, staggered in key order over half the plan, and phased (an exit, then five updates, then an enter), each beside a chart of every element's window in plan time.",
  size: [680, 300],
  data: {
    heights: data.values({ k: K, i: K.map((_, i) => i), together: H.together, stagger: H.stagger, phased: H.phased }, { key: "k" }),
    allWindows: data.values(rows, { key: ["step", "k"] }),
  },
  signals: { phase: signal.num(0), rows: signal.num(6) },
  tables: {
    shown: { from: "heights", ops: [op.derive("h", e("phase == 0 ? d.together : phase == 1 ? d.stagger : d.phased")), op.filter(e("d.h > 0"))] },
    windows: { from: "allWindows", ops: [op.filter(e("d.step == state"))] },
  },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  motion: motion(
    { duration: DURATION, easing: "cubic-in-out" },
    { select: { role: "datum" }, enter: { scale: 0, origin: "bottom" }, exit: { scale: 0, origin: "bottom" } },
    { when: { to: "stagger" }, select: { role: "datum" }, choreo: choreo.stagger("data", SPREAD) },
    { when: { to: "phased" }, select: { role: "datum" }, choreo: choreo.phased(...PHASED) },
    // The windows switch at once, so the new ones are in place while the playhead runs.
    ...["wide", "phone"].map((l) => ({ select: { key: `root/${l}/${l === "wide" ? "content/" : ""}side/timing/chart` }, duration: 0.2, easing: "cubic-out" })),
    ...["wide", "phone"].map((l) => ({ select: { key: `root/${l}/${l === "wide" ? "content/" : ""}side/timing/chart/playhead` }, duration: DURATION, easing: "linear", enter: { dx: -W, opacity: 1 }, exit: { opacity: 0 } })),
  ),
  program: story({ steps: [
    step("together", { set: { phase: 0, rows: 6 }, text: "Together: every element's window is the whole plan, so all six bars start and land at once." }),
    step("stagger", { set: { phase: 1, rows: 6 }, text: "Staggered in key order over half the plan: each window starts a tenth later and lasts half as long." }),
    step("phased", { set: { phase: 2, rows: 7 }, text: "Phased: the exit takes the first 30%, updates the next 50%, the enter the last 20%." }),
  ] }),
});
