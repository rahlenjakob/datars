// Under the hood, §9 (testing): a transition sampled 64 times, and the checks `datars test` runs
// over the samples (crates/datars-test/src/lib.rs, check_motion, and the endpoint hashes). Each
// step plots one element's property at every sample: in blue a plan that passes, in red the bug
// the check exists for, with the harness's message. The curves are made up; the rules are the
// harness's: an entering element never fades back out, a staying one never dips below a quarter
// of its opacity, nothing visible jumps more than 0.35 × the view's diagonal between samples, and
// the first and last samples hash exactly to the two scenes.
import { data, doc, e, geom, group, instances, motion, shape, signal, step, story, type Template } from "@datars/sdk";
import { box, label, line, narration, PHONE, SIZE } from "./_kit";

const N = 64;
const ease = (u: number) => (u < 0.5 ? 4 * u * u * u : 1 - (-2 * u + 2) ** 3 / 2);
const t = Array.from({ length: N }, (_, i) => i / (N - 1));
const bump = (u: number, at: number, w: number) => Math.exp(-(((u - at) / w) ** 2));
const r3 = (v: number) => Math.round(v * 1000) / 1000;
// Each check's two curves over t, normalized to [0, 1] on the chart.
const curves = {
  flash: [t.map(ease), t.map((u) => Math.min(1, Math.max(0, ease(Math.min(1, u * 1.5)) - 0.75 * bump(u, 0.6, 0.09))))],
  blink: [t.map(() => 0.92), t.map((u) => 0.92 - 0.82 * bump(u, 0.5, 0.07))],
  pop: [t.map(ease), t.map((u, i) => (i < 32 ? ease(u) * 0.2 : 0.8 + ease(u) * 0.2))],
  ends: [t.map(ease), t.map((u) => ease(u) * 0.95)],
};
const KEYS = ["flash", "blink", "pop", "ends"] as const;
const cols: Record<string, number[]> = { i: t.map((_, i) => i), t };
KEYS.forEach((k, j) => { cols[`g${j}`] = curves[k][0].map(r3); cols[`b${j}`] = curves[k][1].map(r3); });

const pick = (p: string) => `(phase == 0 ? d.${p}0 : phase == 1 ? d.${p}1 : phase == 2 ? d.${p}2 : d.${p}3)`;
const TEXT = {
  axis: ["OPACITY OF AN ENTERING ELEMENT", "OPACITY OF AN ELEMENT THAT STAYS", "POSITION OF AN ELEMENT", "POSITION OF AN ELEMENT"],
  rule: ["An entering element's opacity never goes down; an exiting one's never goes up.", "An element on both sides never dips below a quarter of its opacity and back.", "Nothing visible moves more than 0.35 × the view's diagonal between two samples.", "Sample 0 hashes to the old scene and sample 63 to the new one, exactly."],
  bad: ["flash: entering E fades back out mid-transition", "blink: S dips to 0.10 opacity mid-transition", "pop: S jumps 300 px between samples", "endpoints are not exact: at(1) is 5% short"],
};
const str = (xs: string[]) => `(phase == 0 ? ${JSON.stringify(xs[0])} : phase == 1 ? ${JSON.stringify(xs[1])} : phase == 2 ? ${JSON.stringify(xs[2])} : ${JSON.stringify(xs[3])})`;

const PAD = { l: 8, t: 24, b: 28 };
const X = (v: string) => `${PAD.l} + (${v}) * (box.w - ${PAD.l + 8})`;
const Y = (v: string) => `${PAD.t} + (1 - (${v})) * (box.h - ${PAD.t + PAD.b})`;

const chart = (): Template => group({
  key: "chart",
  children: [
    label("axis", e(str(TEXT.axis)), [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    line("base", e(X("0")), e(Y("0")), e(X("1")), e(Y("0")), { ink: "$rule" }),
    line("top", e(X("0")), e(Y("1")), e(X("1")), e(Y("1")), { ink: "$grid", dash: [3, 3] }),
    // The target the last sample must land on (the endpoints step).
    label("t0", "t = 0", [e(X("0")), e(`${Y("0")} + 8`)], { size: SIZE.small, ink: "$muted", baseline: "top" }),
    label("tn", e(`'${N} samples'`), [e(X("0.5")), e(`${Y("0")} + 8`)], { size: SIZE.small, ink: "$muted", baseline: "top", align: "middle" }),
    label("t1", "t = 1", [e(X("1")), e(`${Y("0")} + 8`)], { size: SIZE.small, ink: "$muted", baseline: "top", align: "end" }),
    shape(geom.polyline({ from: "samples", x: e(X("d.t")), y: e(Y(pick("b"))) }), { key: "bad-line", stroke: { paint: "$negative", width: 1.5, dash: [4, 3] }, semantics: { role: "decoration" } }),
    shape(geom.polyline({ from: "samples", x: e(X("d.t")), y: e(Y(pick("g"))) }), { key: "good-line", stroke: { paint: "$accent", width: 1.5 }, semantics: { role: "decoration" } }),
    instances({ key: "bad", from: "samples", x: e(X("d.t")), y: e(Y(pick("b"))), r: 2.2, fill: "$negative", instanceKey: e("d.i") }),
    instances({ key: "good", from: "samples", x: e(X("d.t")), y: e(Y(pick("g"))), r: 2.2, fill: "$accent", instanceKey: e("d.i") }),
  ],
});

const check = (): Template => group({
  key: "check",
  children: [
    label("head", "THE CHECK", [0, 0], { size: SIZE.small, ink: "$muted", strong: true, baseline: "top" }),
    box("card", 0, 20, e("box.w"), e("box.h - 20")),
    label("rule", e(str(TEXT.rule)), [14, 36], { ink: "$ink", baseline: "top", maxWidth: e("box.w - 28") }),
    shape(geom.circle({ cx: 19, cy: e("box.h - 58"), r: 4 }), { key: "k-good", fill: "$accent", semantics: { role: "decoration" } }),
    label("good", "passes", [30, e("box.h - 58")], { size: SIZE.small, ink: "$ink-2" }),
    shape(geom.circle({ cx: 19, cy: e("box.h - 34"), r: 4 }), { key: "k-bad", fill: "$negative", semantics: { role: "decoration" } }),
    label("bad", e(str(TEXT.bad)), [30, e("box.h - 34")], { size: SIZE.small, ink: "$negative", maxWidth: e("box.w - 44") }),
  ],
});

const layout = (phone: boolean) => group({
  key: phone ? "phone" : "wide",
  when: e(phone ? PHONE : `!(${PHONE})`),
  layout: { type: "rows", gap: 14, padding: phone ? [12, 12, 8, 12] : [14, 18, 10, 18] },
  children: [
    group({ key: "main", layout: phone ? { type: "rows", gap: 14 } : { type: "columns", gap: 28 }, children: [
      chart(),
      group({ key: "side", size: phone ? { h: 150 } : { w: 230 }, children: [check()] }),
    ] }),
    narration(2, 3),
  ],
});

export default doc({
  id: "how-sweep",
  title: "A transition, sampled 64 times",
  description: "One element's opacity or position at each of 64 samples of a transition, for a plan that passes each motion check and for the bug each check catches: a flash, a blink, a pop and inexact endpoints.",
  size: [680, 300],
  data: { samples: data.values(cols, { key: "i" }) },
  signals: { phase: signal.num(0) },
  scene: group({ key: "root", children: [layout(false), layout(true)] }),
  motion: motion({ duration: 0.8 }),
  program: story({ steps: [
    step("flash", { set: { phase: 0 }, text: "Every transition of every example is sampled 64 times, and every sample is hashed and checked." }),
    step("blink", { set: { phase: 1 }, text: "An element that is there before and after must not blink out and back on the way." }),
    step("pop", { set: { phase: 2 }, text: "Between two samples, nothing on screen may jump a third of the view: that would read as a teleport." }),
    step("endpoints", { set: { phase: 3 }, text: "The first and last samples must be the two scenes bit for bit: settled frames are exact." }),
  ] }),
});
