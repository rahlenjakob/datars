// Motion rules (docs/05-time-and-motion.md). The engine's datars-motion interprets them; the shapes
// here mirror its serde format exactly (tagged `{type}` objects for choreographies and routes,
// kebab-case unit variants, key paths as arrays of keys).

import { clean } from "./prop.js";
import type { Role } from "./nodes.js";

export type Matcher = "by-path" | "by-key" | "nearest" | "none" | { hierarchy: { partition?: "slices" | "grid" } };

export type Choreo =
  | { type: "together" }
  | { type: "stagger"; order?: Order; spread?: number }
  | { type: "phased"; exit: number; update: number; enter: number }
  | { type: "wave"; spread?: number; angle?: number }
  | { type: "ripple"; origin?: [number, number]; spread?: number };

export type Order = "data" | "left" | "right" | "center-out" | "value" | { random: number };

export type Route =
  | { type: "straight" } | { type: "arc"; height?: number } | { type: "elbow" } | { type: "spiral"; turns?: number }
  | { type: "explode" } | { type: "hop"; height?: number } | { type: "drift"; seed?: number; amount?: number }
  | { type: "drop"; bounce?: number };

export type Origin = "center" | "bottom" | "top" | "left" | "right" | { baseline: number } | { point: [number, number] };

export interface Ghost {
  opacity?: number;
  scale?: number;
  origin?: Origin;
  dx?: number;
  dy?: number;
  from?: "parent" | "event" | { point: [number, number] };
  /** The share of a shape's length drawn, from its start: `0` — an entering line draws on, an
   * exiting one draws off. Shapes only. */
  trim?: number;
}

export interface Rule {
  /// Glob patterns over the from/to state names.
  when?: { from?: string; to?: string };
  /// `key` is a key-path prefix written `"root/chart"` (one key per segment).
  select?: { role?: Role; kind?: string; key?: string };
  /// Seconds.
  duration?: number;
  delay?: number;
  /// `"cubic-in-out"`, `"cubic-bezier(.2,.8,.2,1)"`, `"spring(170, 26)"`, `"steps(4)"`, …
  easing?: string;
  matcher?: Matcher;
  choreo?: Choreo;
  route?: Route;
  morph?: "disc" | "resample" | "crossfade";
  enter?: Ghost;
  exit?: Ghost;
}

function keyPath(p: string): string[][] {
  return p.split("/").filter((s) => s.length > 0).map((s) => [s]);
}

function toIr(r: Rule): Record<string, unknown> {
  const { select, ...rest } = r;
  const out: Record<string, unknown> = { ...rest };
  if (select) {
    const { key, ...sel } = select;
    out.select = key === undefined ? sel : { ...sel, key_prefix: keyPath(key) };
  }
  return clean(out) as Record<string, unknown>;
}

export function motion(...rules: Rule[]) {
  return { rules: rules.map(toIr) };
}

export const choreo = {
  together: (): Choreo => ({ type: "together" }),
  stagger: (order: Order = "data", spread = 0.4): Choreo => ({ type: "stagger", order, spread }),
  phased: (exit = 0.3, update = 0.5, enter = 0.2): Choreo => ({ type: "phased", exit, update, enter }),
  wave: (spread = 0.5, angle = 0): Choreo => ({ type: "wave", spread, angle }),
  ripple: (spread = 0.5, origin?: [number, number]): Choreo => ({ type: "ripple", spread, origin }),
};

export const route = {
  straight: (): Route => ({ type: "straight" }),
  arc: (height = 0.45): Route => ({ type: "arc", height }),
  elbow: (): Route => ({ type: "elbow" }),
  spiral: (turns = 1): Route => ({ type: "spiral", turns }),
  explode: (): Route => ({ type: "explode" }),
  hop: (height = 20): Route => ({ type: "hop", height }),
  drift: (seed = 1, amount = 30): Route => ({ type: "drift", seed, amount }),
  drop: (bounce = 0.3): Route => ({ type: "drop", bounce }),
};

export const ghost = {
  fade: (): Ghost => ({ opacity: 0 }),
  grow: (origin: Origin = "bottom"): Ghost => ({ scale: 0, origin }),
  fromParent: (): Ghost => ({ from: "parent" }),
  slide: (dx: number, dy: number): Ghost => ({ opacity: 0, dx, dy }),
};
