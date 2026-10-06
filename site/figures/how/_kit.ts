// Shared look for the "under the hood" figures (site/pages/under-the-hood.html). Not a figure
// itself: the site build registers only files named like `name.ts`, and this one starts with `_`.
import { e, geom, group, shape, text, type NodeOpts, type Prop, type Template } from "@datars/sdk";

/** Type sizes (px): step text, labels, small print. */
export const SIZE = { body: 13, label: 12, small: 11 };

/** True on a phone-width view (the engine's size class). */
export const PHONE = 'sizeClass == "phone"';
export const WIDE = 'sizeClass != "phone"';

/** A rounded box: `on` (an expression) switches it to the accent look. */
export function box(key: Prop, x: Prop, y: Prop, w: Prop, h: Prop, o: { on?: string; dashed?: boolean; fill?: Prop; r?: number } & NodeOpts = {}): Template {
  const { on, dashed, fill, r, ...rest } = o;
  const base = shape(geom.rect({ x, y, w, h, r: r ?? 8 }), {
    key: on ? "base" : key,
    fill: fill ?? "$surface",
    stroke: { paint: dashed ? "$rule" : "$grid", width: 1, dash: dashed ? [4, 3] : undefined },
    semantics: { role: "decoration" },
    ...(on ? {} : rest),
  });
  if (!on) return base;
  // The accent look as an overlay that fades in and out: a paint interpolated from grey to a
  // translucent blue would pass through a stronger blue than either end.
  const lit = shape(geom.rect({ x, y, w, h, r: r ?? 8 }), {
    key: "lit", fill: "$accent@0.12", stroke: { paint: "$accent", width: 1.5 }, opacity: e(`${on} ? 1 : 0`), semantics: { role: "decoration" },
  });
  return group({ key, children: [base, lit], ...rest });
}

type Style = { size?: Prop; ink?: Prop; strong?: boolean; align?: Prop; baseline?: Prop; maxWidth?: Prop };

/** A label: 12 px, `$ink`, left-aligned, middle baseline unless told otherwise. */
export function label(key: Prop, content: Prop, at: [Prop, Prop], s: Style = {}, o: NodeOpts = {}): Template {
  return text(content, at, {
    key,
    style: { font: s.strong ? "font.strong" : "font.body", size: s.size ?? SIZE.label, ink: s.ink ?? "$ink", align: s.align ?? "start", baseline: s.baseline ?? "middle", maxWidth: s.maxWidth },
    ...o,
  });
}

/** A straight arrow from (x1, y1) to (x2, y2). */
export function arrow(key: Prop, x1: Prop, y1: Prop, x2: Prop, y2: Prop, o: { ink?: Prop; width?: Prop; dash?: number[] } & NodeOpts = {}): Template {
  const { ink, width, dash, ...rest } = o;
  return shape(geom.segment({ x1, y1, x2, y2 }), { key, stroke: { paint: ink ?? "$rule", width: width ?? 1.2, dash }, markers: { end: { type: "arrow", size: 6 } }, semantics: { role: "decoration" }, ...rest });
}

/** A line with no head. */
export function line(key: Prop, x1: Prop, y1: Prop, x2: Prop, y2: Prop, o: { ink?: Prop; width?: Prop; dash?: number[] } & NodeOpts = {}): Template {
  const { ink, width, dash, ...rest } = o;
  return shape(geom.segment({ x1, y1, x2, y2 }), { key, stroke: { paint: ink ?? "$rule", width: width ?? 1, dash }, semantics: { role: "decoration" }, ...rest });
}

/** The current step's words, drawn on the canvas along the figure's bottom edge (so the page
 * under the figure never changes height when a step changes). `lines`: the height reserved. */
export function narration(lines = 2, phoneLines = lines): Template {
  const h = (n: number) => n * 18 + 6;
  return group({
    key: "narration",
    size: { h: phoneLines === lines ? h(lines) : e(`${PHONE} ? ${h(phoneLines)} : ${h(lines)}`) },
    children: [
      line("rule", 0, 0.5, e("box.w"), 0.5, { ink: "$grid" }),
      text(e("narration.text"), [0, 10], { key: "text", style: { size: SIZE.body, ink: "$ink-2", baseline: "top", maxWidth: e("box.w") } }),
    ],
  });
}
