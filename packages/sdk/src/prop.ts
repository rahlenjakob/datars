// Property values: literals, or expressions the engine evaluates (per row, per frame, in Rust).
// Expressions can be written three ways — all compile to the same engine expression:
//   e("d.share * 2")          a string of expression source
//   expr`scale.x(d.${field})` a template that splices field names / other expressions
//   d => d.share * 2          a lambda; its source is parsed by the engine (a JS-like subset:
//                             no loops, no closures over local variables — see docs/07)

export interface Expr { expr: string }
export type Literal = number | string | boolean | null;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type Lambda = (...args: any[]) => unknown;
export type Prop = Literal | Expr | Lambda | Prop[] | undefined;

export function e(src: string): Expr {
  return { expr: src };
}

export function isExpr(v: unknown): v is Expr {
  return typeof v === "object" && v !== null && typeof (v as Expr).expr === "string";
}

/** Source text of a value when spliced into an expression. */
export function src(v: Prop): string {
  if (v === undefined || v === null) return "null";
  if (isExpr(v)) return `(${v.expr})`;
  if (typeof v === "function") return `(${lambdaBody(v)})`;
  if (typeof v === "string") return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(src).join(", ")}]`;
  return String(v);
}

/** expr`…`: raw text with `${}` splices. Strings splice raw (identifiers, field names); use `lit()`
 * to splice a string literal; expressions and numbers splice as values. */
export function expr(strings: TemplateStringsArray, ...vals: unknown[]): Expr {
  let out = strings[0];
  vals.forEach((v, i) => {
    if (isExpr(v)) out += `(${v.expr})`;
    else if (typeof v === "function") out += `(${lambdaBody(v as Lambda)})`;
    else if (v !== null && typeof v === "object" && "__lit" in (v as object)) out += JSON.stringify((v as { __lit: string }).__lit);
    else out += String(v);
    out += strings[i + 1];
  });
  return { expr: out };
}

export function lit(s: string): { __lit: string } {
  return { __lit: s };
}

/** A column of the current datum: `field("share")` → `d.share`. */
export function field(name: string): Expr {
  return { expr: /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? `d.${name}` : `d[${JSON.stringify(name)}]` };
}

function lambdaBody(f: Lambda): string {
  // Keep the whole arrow function: the engine's parser accepts `d => …` and `(d, i) => …`.
  return f.toString().replace(/\*\*/g, "^^POW^^").replace(/\^\^POW\^\^/g, "**");
}

/** Normalize a prop into the IR's JSON form. */
export function norm(v: Prop): unknown {
  if (v === undefined) return undefined;
  if (typeof v === "function") return { expr: lambdaBody(v) };
  if (Array.isArray(v)) return v.map(norm);
  return v;
}

/** Drop undefined fields and normalize props (shallow, recursing into plain objects/arrays). */
export function clean<T>(o: T): T {
  if (Array.isArray(o)) return o.map(clean) as unknown as T;
  if (typeof o === "function") return norm(o as unknown as Lambda) as T;
  if (o === null || typeof o !== "object" || isExpr(o)) return o;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(o as Record<string, unknown>)) {
    if (v === undefined) continue;
    out[k] = clean(v);
  }
  return out as T;
}
