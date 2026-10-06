// Recipes: functions from typed params to scene templates, expanded by the engine's sandbox.
// The standard library is written with exactly this API (docs/07-extensibility.md, P5).

import { clean, e, Expr, field as fieldExpr, Prop, src } from "./prop.js";
import { Template, use } from "./nodes.js";
import { Op } from "./data.js";

declare const host: ((name: string, args: unknown) => unknown) | undefined;

// ---- parameter schemas (typed params → docs, inspectors, agent descriptions) -----------------

export interface ParamSpec<T = unknown> { type: string; default?: T; doc?: string; optional?: boolean; values?: readonly string[] }

export const t = {
  table: (doc?: string): ParamSpec<string> => ({ type: "table", doc }),
  field: (doc?: string): ParamSpec<string> => ({ type: "field", doc }),
  number: (def?: number, doc?: string): ParamSpec<number> => ({ type: "number", default: def, doc }),
  string: (def?: string, doc?: string): ParamSpec<string> => ({ type: "string", default: def, doc }),
  bool: (def?: boolean, doc?: string): ParamSpec<boolean> => ({ type: "bool", default: def, doc }),
  ink: (def?: string, doc?: string): ParamSpec<string> => ({ type: "ink", default: def, doc }),
  prop: (doc?: string): ParamSpec<Prop> => ({ type: "prop", doc }),
  oneOf: <V extends string>(values: readonly V[], def?: V, doc?: string): ParamSpec<V> => ({ type: "enum", values, default: def, doc }),
  children: (doc?: string): ParamSpec<Template[]> => ({ type: "children", default: [], doc }),
  json: (doc?: string): ParamSpec<unknown> => ({ type: "json", doc }),
};

// ---- the expansion context -------------------------------------------------------------------

export interface ScaleRef {
  (v: Prop): Expr;
  name: string;
  bandwidth(): Expr;
  step(): Expr;
  ink(v: Prop): Expr;
  min(): Expr;
  max(): Expr;
}

export function scale(name: string): ScaleRef {
  const f = ((v: Prop) => e(`scale.${name}(${src(v)})`)) as ScaleRef;
  Object.defineProperty(f, "name", { value: name });
  f.bandwidth = () => e(`scale.${name}.bandwidth()`);
  f.step = () => e(`scale.${name}.step()`);
  f.ink = (v: Prop) => e(`scale.${name}.ink(${src(v)})`);
  f.min = () => e(`scale.${name}.min()`);
  f.max = () => e(`scale.${name}.max()`);
  return f;
}

export interface Cx {
  /** The size of the box the recipe is expanded for (a hint; layout happens in the engine). */
  size: [number, number];
  sizeClass: "phone" | "tablet" | "wide";
  locale: string;
  /** A theme colour as a late-bound ink (`"$accent"`). */
  ink(token: string): string;
  /** A theme number/text token, evaluated at resolve time. */
  token(name: string): Expr;
  /** Measure text with the engine's shaper (host call; identical on every platform). */
  measure(text: string, style?: { size?: number; weight?: number; family?: string }): { w: number; h: number };
  field(name: string): Expr;
  scale(name: string): ScaleRef;
  /** Register a derived table produced by this expansion; returns its (namespaced) name. */
  table(name: string, from: string, ...ops: Op[]): string;
  /** A stable id unique within this expansion. */
  uid(prefix: string): string;
}

/** FNV-1a (32-bit) as 8 hex digits: a short, deterministic content id. */
function fnv1a(s: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}

class ExpandCx implements Cx {
  size: [number, number];
  sizeClass: "phone" | "tablet" | "wide";
  locale: string;
  tables: Record<string, { from: string; ops: Op[] }> = {};
  private n = 0;
  constructor(raw: { size?: [number, number]; sizeClass?: string; locale?: string }, private prefix: string) {
    this.size = raw.size ?? [800, 480];
    this.sizeClass = (raw.sizeClass as Cx["sizeClass"]) ?? "wide";
    this.locale = raw.locale ?? "en";
  }
  ink(token: string) {
    return token.startsWith("$") || token.startsWith("#") ? token : `$${token}`;
  }
  token(name: string) {
    return e(`token(${JSON.stringify(name)})`);
  }
  measure(text: string, style: { size?: number; weight?: number; family?: string } = {}) {
    if (typeof host === "function") return host("measure", { text, ...style }) as { w: number; h: number };
    return { w: text.length * (style.size ?? 11) * 0.55, h: (style.size ?? 11) * 1.2 };
  }
  field(name: string) {
    return fieldExpr(name);
  }
  scale(name: string) {
    return scale(name);
  }
  table(name: string, from: string, ...ops: Op[]) {
    // Named by content: two instances of a recipe (two plots over different data) get different
    // tables, and identical specs share one.
    const spec = clean({ from, ops });
    const full = `${this.prefix}:${name}:${fnv1a(JSON.stringify(spec))}`;
    this.tables[full] = spec;
    return full;
  }
  uid(prefix: string) {
    this.n += 1;
    return `${prefix}-${this.n}`;
  }
}

// ---- recipe definitions ------------------------------------------------------------------------

export interface RecipeDef<P> {
  /** Full id: `"@datars/std/bar"`. */
  id: string;
  doc?: string;
  params: { [K in keyof P]-?: ParamSpec<P[K]> };
  expand(params: P, cx: Cx): Template;
  /** Motion defaults for the nodes this recipe makes (rules, docs/05), or a function of the
   * params when they depend on them (horizontal bars grow from the left). */
  motion?: unknown | ((params: P) => unknown);
  /** Theme tokens this recipe reads (for specimen boards and docs). */
  tokens?: string[];
  examples?: string[];
}

export interface Recipe<P> {
  (params: Partial<P>, opts?: Record<string, unknown>): Template;
  id: string;
  def: RecipeDef<P>;
  /** Called by the engine inside the sandbox. */
  __expand(params: unknown, cx: unknown): { template: Template; tables: Record<string, unknown>; unknown?: string[] };
  describe(): unknown;
}

function withDefaults<P>(def: RecipeDef<P>, params: Record<string, unknown>): P {
  const out: Record<string, unknown> = { ...params };
  for (const [k, spec] of Object.entries(def.params) as [string, ParamSpec][]) {
    if (out[k] === undefined && spec.default !== undefined) out[k] = spec.default;
  }
  return out as P;
}

// What an enclosing recipe hands its children (a plot's marks get its data and encodings):
// not a typo when a child doesn't declare it.
const INHERITED = new Set(["data", "x", "y", "color", "xType", "yType", "clip", "series"]);

/** Edit distance with swapped neighbours as one edit (`titel` → `title`): small strings only. */
function distance(a: string, b: string): number {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++)
    for (let j = 1; j <= b.length; j++) {
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  return d[a.length][b.length];
}

/** Settings given to a recipe that it doesn't have, with the closest one it does ("did you mean"). */
function unknownParams(def: RecipeDef<unknown>, params: Record<string, unknown>): string[] {
  const known = Object.keys(def.params as Record<string, unknown>);
  return Object.keys(params)
    .filter((k) => !known.includes(k) && !INHERITED.has(k) && params[k] !== undefined)
    .map((k) => {
      const near = known.map((n) => [n, distance(k.toLowerCase(), n.toLowerCase())] as const).sort((x, y) => x[1] - y[1])[0];
      return near && near[1] <= Math.max(1, Math.floor(k.length / 3)) ? `${k} (did you mean \`${near[0]}\`?)` : k;
    });
}

export function recipe<P>(def: RecipeDef<P>): Recipe<P> {
  const f = ((params: Record<string, unknown> = {}, opts: Record<string, unknown> = {}) => use(def.id, params, opts)) as Recipe<P>;
  f.id = def.id;
  f.def = def;
  f.__expand = (params: unknown, rawCx: unknown) => {
    const cx = new ExpandCx((rawCx ?? {}) as never, def.id.split("/").pop() ?? "recipe");
    const full = withDefaults(def, (params ?? {}) as Record<string, unknown>);
    const template = clean(def.expand(full, cx));
    if (!template.prov) (template as Record<string, unknown>).prov = def.id;
    // The recipe's own motion (a line draws on): the engine scopes it to where the recipe is
    // used, under the document's rules.
    const motion = typeof def.motion === "function" ? (def.motion as (p: P) => unknown)(full) : def.motion;
    if (motion && !(template as Record<string, unknown>).motion) (template as Record<string, unknown>).motion = motion;
    const unknown = unknownParams(def as RecipeDef<unknown>, (params ?? {}) as Record<string, unknown>);
    return unknown.length ? { template, tables: cx.tables, unknown } : { template, tables: cx.tables };
  };
  f.describe = () => ({
    id: def.id,
    doc: def.doc,
    params: Object.fromEntries(Object.entries(def.params as Record<string, ParamSpec>).map(([k, s]) => [k, clean({ ...s })])),
    tokens: def.tokens,
    examples: def.examples,
  });
  return f;
}
