// Engine-drawn controls: the same on every platform, in video and in static exports, driven by
// the scrub intent (docs/08-programs.md). Hosts can still bind native controls to the same signals.

import { e, group, pick, recipe, scrub, shape, geom, t, text } from "@datars/sdk";

/** Controls answer at the pace of a UI, not a chart: a switch, a highlight or a list moves in a
 * fifth of a second whatever the document's transitions take (its own motion rules still win). */
const SNAPPY = [{ duration: 0.18, easing: "cubic-out" }];
/** A wash over what the pointer is on (`hover()`: never on touch screens). Ink at a low opacity
 * reads as a hover in light and dark mode alike: darker on paper, lighter on night. */
const WASH = (strength = 0.06) => e(`hover() ? ${strength} : 0`);

export interface SliderParams { signal: string; min: number; max: number; step: number; label: string; format: string }

export const slider = recipe<SliderParams>({
  id: "@datars/std/slider",
  doc: "A slider for a numeric signal: press or drag to set it (snapped to `step`). Shows the label and the current value.",
  params: {
    signal: t.string(undefined, "The numeric signal it sets."),
    min: t.number(0), max: t.number(100), step: t.number(0, "Snap step (0 = continuous)."),
    label: t.string(""), format: t.string(",.0f", "Value format."),
  },
  tokens: ["accent", "rule", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const v = p.signal;
    const x = e(`scale.x(${v})`);
    const y = 30;
    return group({
      key: "slider",
      size: { h: 44 },
      scales: { x: { type: "linear", domain: [p.min, p.max], range: [10, "=box.w - 10"] } },
      on: { drag: scrub(v, { step: p.step }) },
      pickable: true,
      // On the whole control (label, value, track): native bridges frame the adjustable element
      // by it, and touch exploration finds it anywhere on the slider.
      semantics: { role: "control", label: e(`\`${p.label}: \${format(${v}, ${JSON.stringify(p.format)})}\``) },
      children: [
        text(p.label, [0, 0], { key: "label", style: { size: "$size.label", ink: "$ink-2", baseline: "top" } }),
        text(e(`format(${v}, ${JSON.stringify(p.format)})`), [e("box.w"), 0], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", align: "end", baseline: "top" } }),
        shape(geom.segment({ x1: 10, y1: y, x2: e("box.w - 10"), y2: y }), { key: "track", stroke: { paint: "$rule", width: 4, cap: "round" } }),
        shape(geom.segment({ x1: 10, y1: y, x2: x, y2: y }), { key: "fill", stroke: { paint: "$accent", width: 4, cap: "round" } }),
        shape(geom.circle({ cx: x, cy: y, r: 15 }), { key: "halo", fill: "$accent", opacity: WASH(0.18) }),
        shape(geom.circle({ cx: x, cy: y, r: 8 }), { key: "thumb", fill: "$accent", stroke: { paint: "$paper", width: 2 } }),
      ],
    });
  },
});

// ---- choices ---------------------------------------------------------------------------------

/** A value as an expression literal: `"Food"`, `3`. */
const lit = (v: unknown) => JSON.stringify(v);
/** What option `i` says, as an expression: its label, else its key's name in the document's
 * keys (a key without one is itself), else the value. */
const say = (v: unknown, labels: unknown[] | undefined, i: number) => (labels?.[i] !== undefined ? lit(String(labels[i])) : typeof v === "string" ? `key.name(${lit(v)})` : lit(String(v)));
/** The option the signal holds, said: `sig == a ? "A" : … : str(sig)`. */
const current = (sig: string, opts: unknown[], labels: unknown[] | undefined) => `(${opts.map((v, i) => `${sig} == ${lit(v)} ? ${say(v, labels, i)} : `).join("")}\`\${${sig}}\`)`;
const LABEL = { size: "$size.label", ink: "$ink-2", baseline: "top" } as const;

export interface SegmentedParams { signal: string; options: unknown; labels: unknown; label: string }

export const segmented = recipe<SegmentedParams>({
  id: "@datars/std/segmented",
  doc: "One of a few options side by side (a segmented control): click one to set the signal to it; the highlight slides to it. For more options than fit, use `select`.",
  params: {
    signal: t.string(undefined, "The signal it sets."),
    options: t.json("The values to choose from (strings or numbers)."),
    labels: t.json("What each option says (default: its key's name from the document's keys, else the value)."),
    label: t.string("", "A label above the options."),
  },
  tokens: ["accent", "surface", "rule", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = (p.options ?? []) as unknown[];
    const labels = p.labels as unknown[] | undefined;
    const sig = p.signal;
    const n = Math.max(1, opts.length);
    const top = p.label ? 20 : 0;
    const h = 32;
    const at = opts.map((v, i) => `${sig} == ${lit(v)} ? ${i} : `).join("") + "-1";
    const w = `box.w / ${n}`;
    return group({
      key: "segmented",
      size: { h: top + h },
      children: [
        p.label ? text(p.label, [0, 0], { key: "label", style: LABEL }) : null,
        shape(geom.rect({ x: 0, y: top, w: e("box.w"), h, r: 8 }), { key: "track", fill: "$surface", stroke: { paint: "$rule", width: 1 } }),
        // One highlight that slides to the chosen option.
        shape(geom.rect({ x: e(`max(0, ${at}) * ${w} + 2`), y: top + 2, w: e(`${w} - 4`), h: h - 4, r: 6 }), { key: "thumb", fill: "$accent", opacity: e(`(${at}) >= 0 ? 1 : 0`) }),
        ...opts.map((v, i) => group({
          key: `option:${String(v)}`,
          pickable: true,
          on: { activate: { set: sig, value: v as never } },
          semantics: { role: "control", label: e(`${p.label ? `${lit(p.label + ": ")} + ` : ""}${say(v, labels, i)} + (${sig} == ${lit(v)} ? ", selected" : "")`) },
          children: [
            shape(geom.rect({ x: e(`${i} * ${w}`), y: top, w: e(w), h }), { key: "hit", pickable: true, fill: "transparent" }),
            shape(geom.rect({ x: e(`${i} * ${w} + 2`), y: top + 2, w: e(`${w} - 4`), h: h - 4, r: 6 }), { key: "hover", fill: "$ink", opacity: e(`hover() && ${sig} != ${lit(v)} ? 0.07 : 0`) }),
            text(e(say(v, labels, i)), [e(`(${i} + 0.5) * ${w}`), top + h / 2], { key: "text", style: { size: "$size.label", weight: 600, ink: e(`${sig} == ${lit(v)} ? "on($accent)" : "$ink"`), align: "middle", baseline: "middle", maxWidth: e(`${w} - 8`) } }),
          ],
        })),
      ],
    });
  },
});

export interface SelectParams { signal: string; options: unknown; labels: unknown; label: string; open: "down" | "up"; rows: number }

export const select = recipe<SelectParams>({
  id: "@datars/std/select",
  doc: "A dropdown for one of many options: click to open the list over the chart, click an option to set the signal to it; a click anywhere else closes it. The list floats above the whole scene. On phones and tablets the platform's own picker opens instead (web, iOS, Android).",
  params: {
    signal: t.string(undefined, "The signal it sets."),
    options: t.json("The values to choose from (strings or numbers)."),
    labels: t.json("What each option says (default: its key's name from the document's keys, else the value)."),
    label: t.string("", "A label above the box."),
    open: t.oneOf(["down", "up"] as const, "down", "Which way the list opens (up for a select near the bottom of the chart)."),
    rows: t.number(6, "At most this many options in a column: a longer list opens in columns side by side, so it fits a small chart."),
  },
  tokens: ["surface", "paper", "rule", "accent", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = (p.options ?? []) as unknown[];
    const labels = p.labels as unknown[] | undefined;
    const sig = p.signal;
    // Open while `<signal>.open` holds the value the signal had when it opened: choosing another
    // option changes the signal, which closes it — one action per click.
    const state = `${sig}.open`;
    const isOpen = `${state} == \`o:\${${sig}}\``;
    const top = p.label ? 20 : 0;
    const h = 34;
    const row = 28;
    // Balanced columns of at most `rows` options, each at least 110 px wide.
    const cols = Math.max(1, Math.ceil(opts.length / Math.max(1, p.rows || 6)));
    const perCol = Math.max(1, Math.ceil(opts.length / cols));
    const colW = `max(box.w / ${cols}, 110)`;
    const listH = perCol * row + 8;
    const listY = p.open === "up" ? top - listH - 4 : top + h + 4;
    const chevron = (up: boolean) => (up ? [top + h / 2 + 3, top + h / 2 - 2] : [top + h / 2 - 2, top + h / 2 + 3]);
    const [cy0, cy1] = chevron(false), [uy0, uy1] = chevron(true);
    return group({
      key: "select",
      size: { h: top + h },
      children: [
        p.label ? text(p.label, [0, 0], { key: "label", style: LABEL }) : null,
        group({
          key: "box",
          pickable: true,
          // A click opens the drawn list; a host with a picker of its own (a phone's) offers that
          // instead, from `pick`.
          on: { activate: { set: state, value: e(`${isOpen} ? "" : \`o:\${${sig}}\``) }, pick: pick(sig, opts as never, opts.map((v, i) => e(say(v, labels, i)))) },
          semantics: { role: "control", label: e(`${p.label ? `${lit(p.label + ": ")} + ` : ""}${current(sig, opts, labels)}`) },
          children: [
            shape(geom.rect({ x: 0, y: top, w: e("box.w"), h, r: 8 }), { key: "frame", pickable: true, fill: "$surface", stroke: { paint: e(`${isOpen} ? "$accent" : hover() ? "$ink-2" : "$rule"`), width: e(`${isOpen} ? 1.5 : 1`) } }),
            shape(geom.rect({ x: 1, y: top + 1, w: e("box.w - 2"), h: h - 2, r: 7 }), { key: "hover", fill: "$ink", opacity: e(`hover() && !(${isOpen}) ? 0.04 : 0`) }),
            text(e(current(sig, opts, labels)), [12, top + h / 2], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", baseline: "middle", maxWidth: e("box.w - 40") } }),
            // A chevron that points up while the list is open.
            shape(geom.segment({ x1: e("box.w - 22"), y1: e(`${isOpen} ? ${uy0} : ${cy0}`), x2: e("box.w - 17"), y2: e(`${isOpen} ? ${uy1} : ${cy1}`) }), { key: "chevron-a", stroke: { paint: "$ink-2", width: 1.5, cap: "round" } }),
            shape(geom.segment({ x1: e("box.w - 17"), y1: e(`${isOpen} ? ${uy1} : ${cy1}`), x2: e("box.w - 12"), y2: e(`${isOpen} ? ${uy0} : ${cy0}`) }), { key: "chevron-b", stroke: { paint: "$ink-2", width: 1.5, cap: "round" } }),
          ],
        }),
        // While open: a veil over everything (a click on it closes the list), and the list above it.
        shape(geom.rect({ x: -100000, y: -100000, w: 200000, h: 200000 }), { key: "veil", when: e(isOpen), z: 1000, pickable: true, fill: "transparent", on: { activate: { set: state, value: "" } } }),
        group({
          key: "list",
          when: e(isOpen),
          z: 1001,
          children: [
            shape(geom.rect({ x: 0, y: listY, w: e(`${colW} * ${cols}`), h: listH, r: 8 }), { key: "panel", fill: "$paper", stroke: { paint: "$rule", width: 1 } }),
            ...opts.map((v, i) => group({
              key: `option:${String(v)}`,
              pickable: true,
              on: { activate: { set: sig, value: v as never } },
              semantics: { role: "control", label: e(`${say(v, labels, i)} + (${sig} == ${lit(v)} ? ", selected" : "")`) },
              children: (() => {
                const x = `4 + ${Math.floor(i / perCol)} * ${colW}`;
                const y = listY + 4 + (i % perCol) * row;
                return [
                  // The chosen option has no target: a click on it falls through to the veil and closes.
                  shape(geom.rect({ x: e(x), y, w: e(`${colW} - 8`), h: row, r: 6 }), { key: "hit", when: e(`${sig} != ${lit(v)}`), pickable: true, fill: "transparent" }),
                  shape(geom.rect({ x: e(x), y, w: e(`${colW} - 8`), h: row, r: 6 }), { key: "chosen", fill: "$accent", opacity: e(`${sig} == ${lit(v)} ? 0.14 : 0`) }),
                  shape(geom.rect({ x: e(x), y, w: e(`${colW} - 8`), h: row, r: 6 }), { key: "hover", fill: "$ink", opacity: e(`hover() && ${sig} != ${lit(v)} ? 0.07 : 0`) }),
                  text(e(say(v, labels, i)), [e(`${x} + 10`), y + row / 2], { key: "text", style: { size: "$size.label", weight: e(`${sig} == ${lit(v)} ? 600 : 400`), ink: "$ink", baseline: "middle", maxWidth: e(`${colW} - 28`) } }),
                ];
              })(),
            })),
          ],
        }),
      ],
    });
  },
});

export interface ToggleParams { signal: string; label: string }

export const toggle = recipe<ToggleParams>({
  id: "@datars/std/toggle",
  doc: "A switch for a boolean signal: click to flip it. Shows its label beside the switch.",
  params: { signal: t.string(undefined, "The boolean signal it flips."), label: t.string("") },
  tokens: ["accent", "rule", "paper", "ink", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const sig = p.signal;
    return group({
      key: "toggle",
      size: { h: 28 },
      pickable: true,
      on: { activate: { set: sig, value: e(`!${sig}`) } },
      semantics: { role: "control", label: e(`${lit((p.label || sig) + ": ")} + (${sig} ? "on" : "off")`) },
      children: [
        shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: 28 }), { key: "hit", pickable: true, fill: "transparent" }),
        shape(geom.rect({ x: 0, y: 3, w: 40, h: 22, r: 11 }), { key: "track", fill: e(`${sig} ? "$accent" : "$rule"`) }),
        shape(geom.circle({ cx: e(`${sig} ? 29 : 11`), cy: 14, r: 13 }), { key: "halo", fill: "$ink", opacity: WASH(0.1) }),
        shape(geom.circle({ cx: e(`${sig} ? 29 : 11`), cy: 14, r: 8 }), { key: "knob", fill: "$paper" }),
        p.label ? text(p.label, [52, 14], { key: "label", style: { size: "$size.label", ink: "$ink", baseline: "middle" } }) : null,
      ],
    });
  },
});

export interface ChecklistParams { signal: string; options: unknown; labels: unknown; label: string }

export const checklist = recipe<ChecklistParams>({
  id: "@datars/std/checklist",
  doc: "Checkboxes for a key set: click an option to add it to the signal or take it out (series to show, regions to compare). Options wrap to the width.",
  params: {
    signal: t.string(undefined, "The key-set signal it toggles keys in."),
    options: t.json("The keys to offer."),
    labels: t.json("What each option says (default: its key's name from the document's keys, else the key)."),
    label: t.string("", "A label above the options."),
  },
  tokens: ["accent", "ink", "ink-2", "paper", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const opts = (p.options ?? []) as unknown[];
    const labels = p.labels as unknown[] | undefined;
    const sig = p.signal;
    const on = (v: unknown) => `${sig}.has(${lit(String(v))})`;
    return group({
      key: "checklist",
      size: { h: "auto" },
      layout: { type: "rows", gap: 6 },
      children: [
        p.label ? text(p.label, [0, 0], { key: "label", size: { h: "auto" }, style: LABEL }) : null,
        group({
          key: "options",
          size: { h: "auto" },
          layout: { type: "flow", gap: 14 },
          children: opts.map((v, i) => group({
            key: `option:${String(v)}`,
            pickable: true,
            on: { activate: { toggle: sig, value: String(v) } },
            semantics: { role: "control", label: e(`${say(v, labels, i)} + (${on(v)} ? ", checked" : ", not checked")`) },
            children: [
              // The box and the gap up to the label are one target (the label is another).
              shape(geom.rect({ x: -3, y: -3, w: 29, h: 24 }), { key: "hit", pickable: true, fill: "transparent" }),
              shape(geom.rect({ x: -5, y: -5, w: 28, h: 28, r: 7 }), { key: "hover", fill: "$ink", opacity: WASH(0.07) }),
              shape(geom.rect({ x: 0, y: 0, w: 18, h: 18, r: 4 }), { key: "box", pickable: true, fill: e(`${on(v)} ? "$accent" : "$paper"`), stroke: { paint: e(`${on(v)} ? "$accent" : hover() ? "$ink" : "$ink-2"`), width: 1.5 } }),
              shape(geom.segment({ x1: 4.5, y1: 9.5, x2: 7.8, y2: 12.8 }), { key: "check-a", opacity: e(`${on(v)} ? 1 : 0`), stroke: { paint: "on($accent)", width: 2, cap: "round" } }),
              shape(geom.segment({ x1: 7.8, y1: 12.8, x2: 13.5, y2: 6 }), { key: "check-b", opacity: e(`${on(v)} ? 1 : 0`), stroke: { paint: "on($accent)", width: 2, cap: "round" } }),
              text(e(say(v, labels, i)), [26, 9], { key: "text", pickable: true, style: { size: "$size.label", ink: "$ink", baseline: "middle" } }),
            ],
          })),
        }),
      ],
    });
  },
});

export interface RangeParams { lo: string; hi: string; min: number; max: number; step: number; label: string; format: string }

export const range = recipe<RangeParams>({
  id: "@datars/std/range",
  doc: "A slider with two thumbs for a range: drag either end to set the `lo` or `hi` signal (snapped to `step`); the thumbs can't cross. Filter rows with `d.x >= lo && d.x <= hi`.",
  params: {
    lo: t.string(undefined, "The numeric signal for the low end."),
    hi: t.string(undefined, "The numeric signal for the high end."),
    min: t.number(0), max: t.number(100), step: t.number(0, "Snap step (0 = continuous)."),
    label: t.string(""), format: t.string(",.0f", "Value format."),
  },
  tokens: ["accent", "rule", "paper", "ink", "ink-2", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const { lo, hi } = p;
    const f = lit(p.format);
    const y = 30;
    const x = (s: string) => `scale.x(${s})`;
    // Each thumb takes the half of the gap between them that's on its side (16 px at most), so
    // two thumbs at the same value can still be told apart: low to the left, high to the right.
    const half = `clamp((${x(hi)} - ${x(lo)}) / 2, 0, 16)`;
    const thumb = (key: "lo" | "hi") => group({
      key,
      pickable: true,
      on: { drag: scrub(key === "lo" ? lo : hi, { step: p.step, ...(key === "lo" ? { max: e(hi) } : { min: e(lo) }) }) },
      semantics: { role: "control", label: e(`\`${p.label || "Range"} ${key === "lo" ? "from" : "to"}: \${format(${key === "lo" ? lo : hi}, ${f})}\``) },
      children: [
        key === "lo"
          ? shape(geom.rect({ x: e(`${x(lo)} - 16`), y: 12, w: e(`16 + ${half}`), h: 32 }), { key: "hit", fill: "transparent" })
          : shape(geom.rect({ x: e(`${x(hi)} - ${half}`), y: 12, w: e(`16 + ${half}`), h: 32 }), { key: "hit", fill: "transparent" }),
        shape(geom.circle({ cx: e(x(key === "lo" ? lo : hi)), cy: y, r: 15 }), { key: "halo", fill: "$accent", opacity: WASH(0.18) }),
        shape(geom.circle({ cx: e(x(key === "lo" ? lo : hi)), cy: y, r: 8 }), { key: "thumb", fill: "$accent", stroke: { paint: "$paper", width: 2 } }),
      ],
    });
    return group({
      key: "range",
      size: { h: 44 },
      scales: { x: { type: "linear", domain: [p.min, p.max], range: [10, "=box.w - 10"] } },
      semantics: { role: "control", label: e(`\`${p.label || "Range"}: \${format(${lo}, ${f})}–\${format(${hi}, ${f})}\``) },
      children: [
        text(p.label, [0, 0], { key: "label", style: LABEL }),
        text(e(`\`\${format(${lo}, ${f})} – \${format(${hi}, ${f})}\``), [e("box.w"), 0], { key: "value", style: { size: "$size.label", weight: 600, ink: "$ink", align: "end", baseline: "top" } }),
        shape(geom.segment({ x1: 10, y1: y, x2: e("box.w - 10"), y2: y }), { key: "track", stroke: { paint: "$rule", width: 4, cap: "round" } }),
        shape(geom.segment({ x1: e(x(lo)), y1: y, x2: e(x(hi)), y2: y }), { key: "fill", stroke: { paint: "$accent", width: 4, cap: "round" } }),
        thumb("lo"),
        thumb("hi"),
      ],
    });
  },
});

// ---- actions ---------------------------------------------------------------------------------

export interface ButtonParams { label: string; event: string; set: string; value: unknown; kind: "primary" | "secondary" }

export const button = recipe<ButtonParams>({
  id: "@datars/std/button",
  doc: "A button: click to fire a program event (`next`, `prev`, `back`, `goto:<state>`) or set a signal to a value (a reset, a preset). As wide as its box.",
  params: {
    label: t.string(""),
    event: t.string(undefined, "A program event to fire."),
    set: t.string(undefined, "A signal to set (with `value`)."),
    value: t.json("The value `set` sets (a literal, or an expression)."),
    kind: t.oneOf(["primary", "secondary"] as const, "secondary"),
  },
  tokens: ["accent", "surface", "rule", "ink", "size.label"],
  motion: SNAPPY,
  expand(p) {
    const primary = p.kind === "primary";
    const action = p.event ? { event: p.event } : { set: p.set, value: p.value as never };
    return group({
      key: "button",
      size: { h: 36 },
      pickable: true,
      on: { activate: action },
      semantics: { role: "control", label: p.label },
      children: [
        shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: 36, r: 8 }), { key: "face", pickable: true, fill: primary ? "$accent" : "$surface", stroke: primary ? undefined : { paint: e(`hover() ? "$ink-2" : "$rule"`), width: 1 } }),
        shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: 36, r: 8 }), { key: "hover", fill: "$ink", opacity: WASH(primary ? 0.12 : 0.05) }),
        text(p.label, [e("box.w / 2"), 18], { key: "text", style: { size: "$size.label", weight: 600, ink: primary ? "on($accent)" : "$ink", align: "middle", baseline: "middle", maxWidth: e("box.w - 12") } }),
      ],
    });
  },
});
