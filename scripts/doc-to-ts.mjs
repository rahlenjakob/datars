// Turn a JSON document (the IR) into TypeScript source that builds it with @datars/sdk and
// @datars/std — the way a person would write it: recipe calls for `use` nodes, SDK constructors
// for groups, text, data, tables, signals, motion and programs, `e("…")` for expressions, data as
// compact columns (or a sibling JSON file when large), and repetition folded into a table of rows
// mapped through one template (a story's scenes, their captions and its steps).
//
//   node scripts/doc-to-ts.mjs doc.json [--out doc.ts] [--check] [--comment "…"]
//        [--theme ../theme.ts:travel] [--data-limit 20000]
//
// `--check` compiles the output back (scripts/doc-to-json.mjs) and fails unless it's the same
// document (up to the normalizations in `normalize`) and every state resolves to the same scene
// (`datars inspect` hashes). The generator only emits a constructor call when the SDK builds
// exactly the IR it replaces; anything else stays a plain object literal, so the output is always
// the same document even where it's less idiomatic.
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

// ---- code tree + printer ------------------------------------------------------------------------
// A tiny Wadler-style printer: every list prints flat when it fits the line, else one item per line
// (or packed, for long runs of numbers and short strings). Calls hug an object argument, the way
// the examples are written: `plot({ … }, { key: "chart" })`.

const WIDTH = 120;
const raw = (s) => ({ t: "raw", s });
const arr = (items, o = {}) => ({ t: "arr", items, fill: o.fill ?? false });
const obj = (entries) => ({ t: "obj", entries });
const call = (fn, args) => ({ t: "call", fn, args });
const seq = (...parts) => ({ t: "seq", parts });
const arrow = (params, body) => ({ t: "arrow", params, body });
const spread = (x) => seq(raw("..."), x);

const flatCache = new WeakMap();
function flat(n) {
  if (n.t === "raw") return n.s;
  let s = flatCache.get(n);
  if (s !== undefined) return s;
  switch (n.t) {
    case "arr": s = `[${n.items.map(flat).join(", ")}]`; break;
    case "obj": s = n.entries.length ? `{ ${n.entries.map(entryFlat).join(", ")} }` : "{}"; break;
    case "call": s = `${n.fn}(${n.args.map(flat).join(", ")})`; break;
    case "seq": s = n.parts.map(flat).join(""); break;
    case "arrow": s = `(${n.params}) => ${flat(n.body)}`; break;
  }
  flatCache.set(n, s);
  return s;
}
const entryFlat = (e) => (e.spread ? `...${flat(e.spread)}` : `${e.k}: ${flat(e.v)}`);

/** Print `n` starting at column `col` with indentation `ind` (spaces). */
function print(n, ind, col) {
  const f = flat(n);
  if (col + f.length <= WIDTH && !f.includes("\n")) return f;
  const pad = " ".repeat(ind + 2);
  switch (n.t) {
    case "raw": return n.s;
    case "seq": {
      let out = "";
      for (let i = 0; i < n.parts.length; i++) {
        const p = n.parts[i];
        const s = print(p, ind, endCol(col, out));
        out += s;
      }
      return out;
    }
    case "arrow": {
      const head = `(${n.params}) => `;
      return head + print(n.body, ind, col + head.length);
    }
    case "arr": {
      if (n.fill) {
        // Pack items into lines of at most WIDTH.
        const lines = [];
        let line = "";
        for (const it of n.items) {
          const s = flat(it) + ",";
          if (line && ind + 2 + line.length + 1 + s.length > WIDTH) { lines.push(line); line = s; } else line = line ? `${line} ${s}` : s;
        }
        if (line) lines.push(line);
        return `[\n${lines.map((l) => pad + l).join("\n")}\n${" ".repeat(ind)}]`;
      }
      return `[\n${n.items.map((it) => pad + print(it, ind + 2, ind + 2) + ",").join("\n")}\n${" ".repeat(ind)}]`;
    }
    case "obj": {
      return `{\n${n.entries.map((e) => pad + printEntry(e, ind + 2) + ",").join("\n")}\n${" ".repeat(ind)}}`;
    }
    case "call": {
      const head = `${n.fn}(`;
      const args = n.args;
      const huggable = (a) => a.t === "obj" || a.t === "arr" || a.t === "arrow" || a.t === "call";
      // Hug the last argument: `fn(a, b, {` … `})`.
      if (args.length && huggable(args[args.length - 1]) && args.slice(0, -1).every((a) => a.t === "raw")) {
        const before = args.slice(0, -1).map(flat).join(", ");
        const prefix = head + before + (before ? ", " : "");
        if (col + prefix.length < WIDTH - 10) {
          const last = print(args[args.length - 1], ind, col + prefix.length);
          if (!before || firstLine(last).length + col + prefix.length <= WIDTH) return `${prefix}${last})`;
        }
      }
      // Hug the first argument when the rest is short: `plot({` … `}, { key: "chart" })`.
      if (args.length === 2 && huggable(args[0])) {
        const rest = args.slice(1).map(flat).join(", ");
        const first = print(args[0], ind, col + head.length);
        if (endCol(col, head + first) + 2 + rest.length + 1 <= WIDTH) return `${head}${first}, ${rest})`;
        // The rest broken too: hug the first, then the others each hugged in turn.
        let out = `${head}${first}`;
        for (const a of args.slice(1)) out += `, ${print(a, ind, endCol(col, out) + 2)}`;
        return `${out})`;
      }
      return `${head}\n${args.map((a) => pad + print(a, ind + 2, ind + 2) + ",").join("\n")}\n${" ".repeat(ind)})`;
    }
  }
  return f;
}
function printEntry(e, ind) {
  if (e.spread) return `...${print(e.spread, ind, ind + 3)}`;
  return `${e.k}: ${print(e.v, ind, ind + e.k.length + 2)}`;
}
const lastLine = (s) => s.length - s.lastIndexOf("\n") - 1;
/** The column after printing `s` from column `col`. */
const endCol = (col, s) => (s.includes("\n") ? lastLine(s) : col + s.length);
const firstLine = (s) => (s.includes("\n") ? s.slice(0, s.indexOf("\n")) : s);

// ---- literals -----------------------------------------------------------------------------------

/** A string literal in the examples' style: double quotes, single when the text has double quotes. */
function str(s) {
  if (s.includes('"') && !s.includes("'")) return `'${JSON.stringify(s).slice(1, -1).replace(/\\"/g, '"')}'`;
  return JSON.stringify(s);
}
const IDENT = /^[A-Za-z_$][A-Za-z0-9_$]*$/;
const RESERVED = new Set("break case catch class const continue debugger default delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with yield let static implements interface package private protected public await".split(" "));
const propKey = (k) => (IDENT.test(k) ? k : str(k));
const num = (n) => (Object.is(n, -0) ? "0" : String(n));
const isPlain = (v) => v !== null && typeof v === "object" && !Array.isArray(v);
const isExprObj = (v) => isPlain(v) && Object.keys(v).length === 1 && typeof v.expr === "string";

// ---- markers the factoring passes leave in the tree ---------------------------------------------

const REF = Symbol("ref"); // { [REF]: "s.title" } → that code
const TPL = Symbol("tpl"); // { [TPL]: [strings, codes, isExpr] } → `…${s.scene}…` (inside e() when isExpr)
const MAP = Symbol("map"); // { [MAP]: { rows, param, template, lift } } → rows.map((param) => template)
const SPREAD = Symbol("spread"); // { [SPREAD]: "s.notes" } in an array → ...s.notes
const CODE = Symbol("code"); // { [CODE]: tree } → that code tree, printed as is
const ref = (name) => ({ [REF]: name });

// ---- lifting: IR values → code --------------------------------------------------------------------

const NODE_KINDS = new Set(["group", "use", "text", "shape", "repeat", "instances", "tiles", "view"]);
const isNode = (v) => isPlain(v) && typeof v.kind === "string" && NODE_KINDS.has(v.kind);

class Lifter {
  constructor(opts = {}) {
    this.sdk = new Set(["doc"]);
    this.std = new Set();
    this.local = new Map(); // recipe id → { name, from }
    this.recipes = opts.recipes ?? {}; // non-std recipe id → { name, from }
    this.aliases = new Map();
    this.helpers = new Set();
  }

  /** The name a std export is imported as (std's `route` recipe would shadow the SDK's motion
   * routes, so it's imported under another name when both are used). */
  stdName(name) {
    this.std.add(name);
    return name === "route" ? "routeLine" : name;
  }

  value(v) {
    if (v === null) return raw("null");
    if (typeof v === "boolean") return raw(String(v));
    if (typeof v === "number") return raw(num(v));
    if (typeof v === "string") return raw(str(v));
    if (v === undefined) return raw("undefined");
    if (v[REF]) return raw(v[REF]);
    if (v[CODE]) return v[CODE];
    if (v[TPL]) return this.template(v[TPL]);
    if (v[MAP]) return this.mapped(v[MAP]);
    if (Array.isArray(v)) return this.array(v);
    if (isExprObj(v)) { this.sdk.add("e"); return call("e", [raw(str(v.expr))]); }
    if (isPlain(v) && typeof v["~call"] === "string") {
      this.helpers.add(v["~call"]);
      if (v["~call"] === "geoBox") this.sdk.add("e");
      return call(v["~call"], Object.entries(v).filter(([k]) => k !== "~call").map(([, x]) => this.value(x)));
    }
    if (isNode(v)) return this.node(v);
    return this.object(v);
  }

  array(v) {
    const items = [];
    for (const x of v) {
      if (x && x[MAP]) items.push(spread(this.mapped(x[MAP])));
      else if (x && x[SPREAD]) items.push(spread(raw(x[SPREAD])));
      else items.push(this.value(x));
    }
    const atomic = v.length > 8 && v.every((x) => typeof x === "number" || (typeof x === "string" && x.length < 24) || x === null || typeof x === "boolean");
    return arr(items, { fill: atomic });
  }

  object(o, order = LEAD) {
    const keys = Object.keys(o).filter((k) => o[k] !== undefined);
    keys.sort((a, b) => rank(order, a) - rank(order, b));
    return obj(keys.map((k) => ({ k: propKey(k), v: this.value(o[k]) })));
  }

  /** `e(\`scene == "${s.scene}"\`)`: an expression with holes filled from a row. */
  template([strings, codes, isExpr]) {
    let s = "`";
    strings.forEach((part, i) => {
      s += part.replace(/[`\\]/g, (c) => `\\${c}`).replace(/\$\{/g, "\\${");
      if (i < codes.length) s += "${" + codes[i] + "}";
    });
    if (!isExpr) return raw(s + "`");
    this.sdk.add("e");
    return call("e", [raw(s + "`")]);
  }

  /** `rows.map((s) => template)`, the template lifted as what it stands for (a node, a step). */
  mapped({ rows, param, template, lift }) {
    const body = lift === "step" ? this.step(template) : lift === "rule" ? this.rule(template) : this.value(template);
    return call(`${rows}.map`, [arrow(param, body)]);
  }

  // ---- scene nodes ----

  node(n) {
    if (n.kind === "use") return this.use(n);
    if (n.kind === "group") return this.group(n);
    if (n.kind === "text") return this.text(n) ?? this.object(n);
    return this.object(n);
  }

  use(n) {
    const { kind, recipe, params, ...opts } = n;
    let fn;
    if (typeof recipe === "string" && recipe.startsWith("@datars/std/")) fn = this.stdName(recipe.slice("@datars/std/".length));
    else if (this.recipes[recipe]) { fn = this.recipes[recipe].name; this.local.set(recipe, this.recipes[recipe]); }
    if (!fn || (params !== undefined && !isPlain(params) && !params[REF])) {
      this.sdk.add("use");
      return call("use", [raw(str(recipe)), this.value(params ?? {}), ...(Object.keys(opts).length ? [this.object(opts, NODE_ORDER)] : [])]);
    }
    const args = [params && params[REF] ? this.value(params) : this.object(params ?? {}, paramOrder(params ?? {}))];
    if (Object.keys(opts).length) args.push(this.object(opts, NODE_ORDER));
    return call(fn, args);
  }

  group(n) {
    this.sdk.add("group");
    const { kind, ...rest } = n;
    if (!rest.children) rest.children = [];
    return call("group", [this.object(rest, NODE_ORDER)]);
  }

  /** `text(content, at, opts)`, when the node only has what `text()` builds. */
  text(n) {
    const { kind, text: content, at, style, halo, number, rotate, offset, ...rest } = n;
    const STYLE = { font: "font", size: "size", weight: "weight", ink: "ink", align: "align", baseline: "baseline", max_width: "maxWidth", contain: "contain" };
    if (style !== undefined && (!isPlain(style) || Object.keys(style).some((k) => !(k in STYLE)))) return null;
    if (content === undefined || at === undefined) return null;
    this.sdk.add("text");
    const opts = { ...rest };
    if (style && Object.keys(style).length) opts.style = Object.fromEntries(Object.entries(style).map(([k, v]) => [STYLE[k], v]));
    Object.assign(opts, { halo, number, rotate, offset });
    const args = [this.value(content), this.value(at)];
    if (Object.values(opts).some((v) => v !== undefined)) args.push(this.object(opts, NODE_ORDER));
    return call("text", args);
  }
}

// ---- document parts -------------------------------------------------------------------------------

/** A value standing for something a factoring pass put elsewhere (a row field, a shared const). */
const isMarker = (v) => v !== null && typeof v === "object" && (v[REF] !== undefined || v[TPL] !== undefined || v[MAP] !== undefined || v[SPREAD] !== undefined || v[CODE] !== undefined);

Object.assign(Lifter.prototype, {
  /** A data source: `data.values(…)`, `data.url(…)`, `data.atlas(…)`, … when the SDK builds it. */
  source(s) {
    if (isMarker(s)) return this.value(s);
    const { key, types, live, id, ...from } = s;
    const kinds = Object.keys(from);
    const one = kinds.length === 1 ? kinds[0] : null;
    const o = {};
    if (key !== undefined) {
      if (!Array.isArray(key)) return this.object(s);
      o.key = key.length === 1 ? key[0] : key;
    }
    if (types !== undefined) o.types = types;
    if (live !== undefined) o.live = live;
    const opts = () => (Object.keys(o).length ? [this.object(o)] : []);
    const d = (name, args) => { this.sdk.add("data"); return call(`data.${name}`, args); };
    if (one === "values" && id === undefined) return d("values", [this.value(from.values), ...opts()]);
    if (one === "csv" && id === undefined) return d("csv", [this.value(from.csv), ...opts()]);
    if (one === "url" && typeof from.url === "string") {
      if (id !== undefined) o.id = id;
      return d("url", [raw(str(from.url)), ...opts()]);
    }
    if (!Object.keys(o).length && typeof from[one] === "string" && id === undefined) {
      if (one === "atlas") return d("atlas", [raw(str(from.atlas))]);
      if (one === "tiles") return from.tiles === "auto" ? d("tiles.auto", []) : d("tiles", [raw(str(from.tiles))]);
      if (one === "font") return d("font", [raw(str(from.font))]);
    }
    if (!Object.keys(o).length && (one === "geojson" || one === "topojson")) return d(one, [this.value(from[one]), ...(id !== undefined ? [this.object({ id })] : [])]);
    return this.object(s);
  },

  /** A derived table: `table(from, op.sort(…), …)`. */
  table(t) {
    if (!isPlain(t) || typeof t.from !== "string" || !Array.isArray(t.ops) || Object.keys(t).length !== 2) return this.value(t);
    this.sdk.add("table");
    return call("table", [raw(str(t.from)), ...t.ops.map((o) => this.op(o))]);
  },

  op(o) {
    const has = (...ks) => Object.keys(o).length === ks.length + 1 && ks.every((k) => k in o);
    const opCall = (name, args) => { this.sdk.add("op"); return call(`op.${name}`, args); };
    const v = (x) => this.value(x);
    const rest = () => { const { op: _, ...r } = o; return r; };
    switch (o.op) {
      case "sort":
        // `op.sort(f)` sorts ascending (`[f, "asc"]`); anything else is given as the pair.
        if (has("by") && Array.isArray(o.by) && o.by.every((b) => Array.isArray(b))) return opCall("sort", o.by.map((b) => (b.length === 2 && b[1] === "asc" ? v(b[0]) : v(b))));
        break;
      case "filter": if (has("expr")) return opCall("filter", [v(o.expr)]); break;
      case "derive": if (has("as", "expr")) return opCall("derive", [v(o.as), v(o.expr)]); break;
      case "top":
        if (has("n", "by")) return opCall("top", [v(o.n), v(o.by)]);
        if (has("n", "by", "other")) return opCall("top", [v(o.n), v(o.by), v(o.other)]);
        break;
      case "aggregate": {
        const ok = has("groupby", "ops") && Array.isArray(o.ops) && o.ops.every((x) => isPlain(x) && typeof x.as === "string" && typeof x.op === "string" && Object.keys(x).every((k) => ["as", "op", "field"].includes(k)));
        if (ok && new Set(o.ops.map((x) => x.as)).size === o.ops.length) {
          return opCall("aggregate", [v(o.groupby), obj(o.ops.map((x) => ({ k: propKey(x.as), v: arr(x.field === undefined ? [v(x.op)] : [v(x.op), v(x.field)]) })))]);
        }
        break;
      }
      case "interpolate": if (has("key", "time", "value", "at")) return opCall("interpolate", [v(o.key), v(o.time), v(o.value), v(o.at)]); break;
      case "join": if (has("with", "on", "kind") && Array.isArray(o.on) && ["inner", "left"].includes(o.kind)) return opCall("join", [v(o.with), o.on.length === 1 ? v(o.on[0]) : v(o.on), ...(o.kind === "left" ? [] : [v(o.kind)])]); break;
      case "unpivot": if (has("columns", "as")) return opCall("unpivot", [v(o.columns), v(o.as)]); break;
      case "pivot": if (has("key", "value", "index")) return opCall("pivot", [v(o.key), v(o.value), v(o.index)]); break;
      case "union": if (has("with")) return opCall("union", [v(o.with)]); break;
      case "sample": if (has("n", "seed")) return opCall("sample", [v(o.n), v(o.seed)]); break;
      // The layout algorithms take their options object as is.
      case "stack": case "pie": case "treemap": case "pack": case "beeswarm": case "spread": case "units": case "parliament": case "waffle": case "waterfall":
        return opCall(o.op, [this.object(rest())]);
    }
    return this.object(o);
  },

  signal(s) {
    if (isMarker(s)) return this.value(s);
    const only = (...ks) => Object.keys(s).every((k) => ks.includes(k));
    const sig = (name, args) => { this.sdk.add("signal"); return call(`signal.${name}`, args.map((a) => (a && a.t ? a : this.value(a)))); };
    const ctl = s.control !== undefined ? [this.control(s.control)] : [];
    if (s.type === "str" && typeof s.default === "string" && only("type", "default", "control")) return sig("str", [s.default, ...ctl]);
    if (s.type === "num" && typeof s.default === "number" && only("type", "default", "control")) return sig("num", [s.default, ...ctl]);
    if (s.type === "num" && typeof s.clock === "number" && typeof s.default === "number" && only("type", "default", "clock")) return sig("clock", [s.clock, s.default]);
    if (s.type === "bool" && typeof s.default === "boolean" && only("type", "default")) return sig("bool", [s.default]);
    if (s.type === "keyset" && Array.isArray(s.default) && only("type", "default")) return sig("keyset", s.default.length ? [s.default] : []);
    if (s.type === "key" && s.default !== undefined && only("type", "default")) return sig("key", s.default === null ? [] : [s.default]);
    if (s.type === "range" && s.default === null && only("type", "default")) return sig("range", []);
    return this.object(s);
  },

  control(c) {
    const only = (...ks) => Object.keys(c).every((k) => ks.includes(k));
    const ctl = (name, args) => { this.sdk.add("control"); return call(`control.${name}`, args.map((a) => this.value(a))); };
    const label = c.label !== undefined ? [c.label] : [];
    if (c.type === "slider" && only("type", "min", "max", "step", "label") && typeof c.step === "number") return ctl("slider", [c.min, c.max, c.step, ...label]);
    if (c.type === "select" && only("type", "options", "label")) return ctl("select", [c.options, ...label]);
    if (c.type === "toggle" && only("type", "label")) return ctl("toggle", label);
    return this.object(c);
  },

  /** `motion(…rules)`: key paths back to `"root/chart"`, choreographies, routes and ghosts through
   * their helpers when those build exactly the same thing. */
  motion(m) {
    if (!isPlain(m) || Object.keys(m).length !== 1 || !Array.isArray(m.rules)) return this.value(m);
    this.sdk.add("motion");
    return call("motion", m.rules.map((r) => (r && r[MAP] ? spread(this.value(r)) : this.rule(r))));
  },

  rule(r) {
    if (!isPlain(r) || isMarker(r)) return this.value(r);
    const out = {};
    for (const [k, v] of Object.entries(r)) {
      if (k === "select" && isPlain(v) && v.key_prefix !== undefined) {
        const { key_prefix, ...sel } = v;
        const simple = Array.isArray(key_prefix) && key_prefix.every((seg) => Array.isArray(seg) && seg.length === 1 && typeof seg[0] === "string" && seg[0] && !seg[0].includes("/"));
        if (!simple) return this.object(r);
        out.select = obj([...Object.entries(sel).map(([a, b]) => ({ k: propKey(a), v: this.value(b) })), { k: "key", v: raw(str(key_prefix.map((s) => s[0]).join("/"))) }]);
      } else if (k === "choreo") out.choreo = this.choreo(v);
      else if (k === "route") out.route = this.routeOf(v);
      else if (k === "enter" || k === "exit") out[k] = this.ghost(v);
      else out[k] = this.value(v);
    }
    const order = ["when", "select", "duration", "delay", "easing", "matcher", "choreo", "route", "morph", "enter", "exit"];
    return obj(Object.keys(out).sort((a, b) => rank(order, a) - rank(order, b)).map((k) => ({ k: propKey(k), v: out[k] })));
  },

  choreo(c) {
    const only = (...ks) => isPlain(c) && Object.keys(c).length === ks.length && ks.every((k) => k in c);
    const use = (name, args) => { this.sdk.add("choreo"); return call(`choreo.${name}`, args.map((a) => this.value(a))); };
    if (c?.type === "together" && only("type")) return use("together", []);
    if (c?.type === "stagger" && only("type", "order", "spread") && typeof c.spread === "number") return use("stagger", [c.order, c.spread]);
    if (c?.type === "phased" && only("type", "exit", "update", "enter")) return use("phased", [c.exit, c.update, c.enter]);
    if (c?.type === "wave" && only("type", "spread", "angle")) return use("wave", [c.spread, c.angle]);
    if (c?.type === "ripple" && only("type", "spread", "origin")) return use("ripple", [c.spread, c.origin]);
    return this.value(c);
  },

  routeOf(r) {
    const only = (...ks) => isPlain(r) && Object.keys(r).length === ks.length && ks.every((k) => k in r);
    const use = (name, args) => { this.sdk.add("route"); return call(`route.${name}`, args.map((a) => this.value(a))); };
    if (r?.type === "straight" && only("type")) return use("straight", []);
    if (r?.type === "elbow" && only("type")) return use("elbow", []);
    if (r?.type === "explode" && only("type")) return use("explode", []);
    if (r?.type === "arc" && only("type", "height")) return use("arc", [r.height]);
    if (r?.type === "hop" && only("type", "height")) return use("hop", [r.height]);
    if (r?.type === "spiral" && only("type", "turns")) return use("spiral", [r.turns]);
    if (r?.type === "drift" && only("type", "seed", "amount")) return use("drift", [r.seed, r.amount]);
    if (r?.type === "drop" && only("type", "bounce")) return use("drop", [r.bounce]);
    return this.value(r);
  },

  ghost(g) {
    const only = (...ks) => isPlain(g) && Object.keys(g).length === ks.length && ks.every((k) => k in g);
    const use = (name, args) => { this.sdk.add("ghost"); return call(`ghost.${name}`, args.map((a) => this.value(a))); };
    if (only("opacity") && g.opacity === 0) return use("fade", []);
    if (only("scale", "origin") && g.scale === 0 && typeof g.origin === "string") return use("grow", g.origin === "bottom" ? [] : [g.origin]);
    if (only("opacity", "dx", "dy") && g.opacity === 0) return use("slide", [g.dx, g.dy]);
    if (only("from") && g.from === "parent") return use("fromParent", []);
    return this.value(g);
  },

  /** A program: `story({ steps: [step(…)] })`, with chapters as `chapter(param, steps)`. */
  program(p) {
    const known = ["preset", "states", "drivers", "chapters"];
    if (!isPlain(p) || p.preset !== "story" || !(Array.isArray(p.states) || isMarker(p.states)) || Object.keys(p).some((k) => !known.includes(k))) return this.value(p);
    const entries = [{ k: "steps", v: this.steps(p.states) }];
    const drivers = p.drivers ?? [];
    if (JSON.stringify(drivers) !== JSON.stringify(["steps", "keys"])) entries.push({ k: "drivers", v: this.value(drivers) });
    // An empty `chapters` is the same as none (serde default).
    if (p.chapters && Object.keys(p.chapters).length) {
      entries.push({ k: "chapters", v: obj(Object.entries(p.chapters).map(([name, c]) => ({ k: propKey(name), v: this.chapter(c) }))) });
    }
    this.sdk.add("story");
    return call("story", [obj(entries)]);
  },

  steps(states) {
    if (isMarker(states)) return this.value(states);
    return arr(states.map((s) => (s && s[MAP] ? spread(this.value(s)) : this.step(s))));
  },

  /** `step(name, { set, hold, title, text, anchor })` — narration's fields sit on the step. */
  step(s) {
    if (!isPlain(s) || isMarker(s)) return this.value(s);
    const n = s.narration;
    const plainish = (x) => typeof x === "string" || isMarker(x);
    const ok = Object.keys(s).every((k) => ["name", "set", "hold", "narration"].includes(k)) && plainish(s.name)
      && (n === undefined || (isPlain(n) && !isMarker(n) && Object.keys(n).every((k) => ["title", "text", "anchor"].includes(k)) && Object.values(n).every(plainish) && Object.keys(n).length > 0));
    if (!ok) return this.object(s);
    const o = [];
    if (s.set !== undefined) o.push({ k: "set", v: this.value(s.set) });
    if (s.hold !== undefined) o.push({ k: "hold", v: this.value(s.hold) });
    for (const k of ["title", "text", "anchor"]) if (n && n[k] !== undefined) o.push({ k, v: this.value(n[k]) });
    this.sdk.add("step");
    return call("step", [this.value(s.name), ...(o.length ? [obj(o)] : [])]);
  },

  chapter(c) {
    if (isPlain(c) && typeof c.param === "string" && isPlain(c.program) && Object.keys(c).length === 2 && Object.keys(c.program).length === 1 && Array.isArray(c.program.states)) {
      this.sdk.add("chapter");
      return call("chapter", [raw(str(c.param)), this.steps(c.program.states)]);
    }
    return this.value(c);
  },

  /** A theme: a named ThemeDef is `theme({ … })` (the document uses it); others as they are. */
  themeDef(t) {
    this.sdk.add("theme");
    return call("theme", [this.object(t, ["name", "extends", "scheme", "tokens", "modes", "locked", "checks"])]);
  },
});

const NODE_ORDER = ["key", "id", "when", "layout", "size", "scales", "coord", "camera", "opacity", "transform", "semantics", "style", "children"];
const PARAM_ORDER = ["source", "part", "data", "key", "value", "x", "y", "color", "xType", "yType", "colorType", "text", "title"];
/** Plain objects lead with what names them (`{ type: "rows", gap: 8 }`, `{ name, color }`). */
const LEAD = ["type", "kind", "name", "id", "key", "text", "title"];
function rank(order, k) {
  const i = order.indexOf(k);
  if (k === "children" || k === "base") return 1000;
  return i < 0 ? 500 : i;
}
const paramOrder = () => PARAM_ORDER;

// ---- the document -----------------------------------------------------------------------------------

const camel = (s) => s.replace(/^[^A-Za-z_$]+/, "").replace(/[^A-Za-z0-9_$]+(.)?/g, (_, c) => (c ? c.toUpperCase() : "")) || "value";
const jsonSize = (v) => JSON.stringify(v).length;
const deepEqual = (a, b) => JSON.stringify(sortKeys(a)) === JSON.stringify(sortKeys(b));
function sortKeys(v) {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (isPlain(v)) return Object.fromEntries(Object.keys(v).sort().map((k) => [k, sortKeys(v[k])]));
  return v;
}

/**
 * The TypeScript for a document. Options:
 * - `comment`: lines for the header comment (what the chart shows, where the figures come from).
 * - `theme`: `{ from, name, def }` — a ThemeDef shared by several documents (imported when the
 *   document's theme is exactly it).
 * - `recipes`: non-std recipe id → `{ name, from }`, the module exporting the recipe function.
 * - `packages`: package name → `file` (a package module next to the document, see local-recipes.mjs).
 * - `dataLimit`: inline data larger than this (JSON bytes) goes to a sibling file; `writeData(name,
 *   value)` writes it and returns the import path.
 */
export function docToTs(input, opts = {}) {
  const doc = structuredClone(input);
  const L = new Lifter({ recipes: opts.recipes });
  const decls = []; // [name, code tree, comment]
  const imports = []; // extra import lines
  const taken = new Set([...SDK_NAMES, ...STD_NAMES, "routeLine"]);
  const fresh = (base) => {
    let n = camel(base);
    if (RESERVED.has(n)) n += "Data";
    for (let i = 2; taken.has(n); i++) n = `${camel(base)}${i}`;
    taken.add(n);
    return n;
  };

  // Factor repetition (scenes, captions, steps) into row tables before lifting.
  if (doc.scene !== undefined) doc.scene = idioms(doc.scene);
  const rows = opts.factor === false ? [] : factor(doc, fresh);

  // Data: inline columns hoisted to named consts above the document (or a sibling JSON file).
  const data = {};
  for (const [name, s] of Object.entries(doc.data ?? {})) {
    if (isPlain(s) && s.values !== undefined && !isMarker(s.values)) {
      // Columns that follow a rule are written as the rule; what's left is the data proper.
      const compact = compactColumns(s.values);
      const size = jsonSize(compact);
      if (opts.writeData && size > (opts.dataLimit ?? 20000)) {
        const id = fresh(name);
        const path = opts.writeData(name, s.values);
        imports.push(`import ${id} from ${str(path)};`);
        data[name] = L.source({ ...s, values: ref(id) });
        continue;
      }
      if (size > 300) {
        const id = fresh(name);
        decls.push([id, L.value(compact)]);
        data[name] = L.source({ ...s, values: ref(id) });
        continue;
      }
      data[name] = L.source({ ...s, values: compact });
      continue;
    }
    data[name] = L.source(s);
  }

  for (const r of rows) decls.push([r.name, arr(r.rows.map((row) => L.object(row, ["scene", "name", "id", "key"]))), r.comment]);
  for (const h of opts.factor === false ? [] : hoist(doc, fresh)) decls.push([h.name, L.value(h.node)]);

  // The theme: imported when shared, else declared here.
  let theme;
  if (doc.theme !== undefined) {
    const t = doc.theme;
    const named = isPlain(t) && Array.isArray(t.themes) && t.themes.length === 1 && Object.keys(t).length === 2 && t.use === t.themes[0]?.name;
    if (named && opts.theme && deepEqual(t.themes[0], opts.theme.def)) {
      imports.push(`import { ${opts.theme.name} } from ${str(opts.theme.from)};`);
      taken.add(opts.theme.name);
      theme = raw(opts.theme.name);
    } else if (named) {
      const id = fresh(String(t.use).split("/").pop());
      decls.push([id, L.themeDef(t.themes[0])]);
      theme = raw(id);
    } else if (typeof t === "object" && t !== null && Object.keys(t).length === 1 && typeof t.use === "string") theme = raw(str(t.use));
    else theme = L.value(t);
  }

  const entries = [];
  const put = (k, v) => entries.push({ k, v });
  if (doc.id !== undefined) put("id", L.value(doc.id));
  if (doc.title !== undefined) put("title", L.value(doc.title));
  if (doc.description !== undefined) put("description", L.value(doc.description));
  const size = doc.size;
  if (isPlain(size) && Object.keys(size).length === 2 && typeof size.width === "number" && typeof size.height === "number") put("size", arr([raw(num(size.width)), raw(num(size.height))]));
  else if (size !== undefined) put("size", L.value(size));
  if (theme) put("theme", theme);
  if (doc.locale !== undefined && doc.locale !== "en") put("locale", L.value(doc.locale));
  if (doc.packages !== undefined) {
    put("packages", arr(doc.packages.map((p) => (opts.packages?.[p.name] && Object.keys(p).every((k) => ["name", "source"].includes(k))
      ? obj([{ k: "name", v: raw(str(p.name)) }, { k: "file", v: raw(str(opts.packages[p.name])) }])
      : L.value(p)))));
  }
  if (doc.data !== undefined) put("data", obj(Object.entries(data).map(([k, v]) => ({ k: propKey(k), v }))));
  if (doc.tables !== undefined) put("tables", obj(Object.entries(doc.tables).map(([k, t]) => ({ k: propKey(k), v: L.table(t) }))));
  if (doc.signals !== undefined) put("signals", obj(Object.entries(doc.signals).map(([k, s]) => ({ k: propKey(k), v: L.signal(s) }))));
  if (doc.keys !== undefined) {
    // Thousands of keys (a county per row) are data: a sibling file, like large inline data.
    if (opts.writeData && jsonSize(doc.keys) > (opts.dataLimit ?? 20000)) {
      const id = fresh("keys");
      imports.push(`import ${id} from ${str(opts.writeData("keys", doc.keys))};`);
      put("keys", raw(id));
    } else put("keys", L.value(doc.keys));
  }
  if (doc.scene !== undefined) put("scene", L.value(doc.scene));
  if (doc.motion !== undefined) put("motion", L.motion(doc.motion));
  if (doc.program !== undefined) put("program", L.program(doc.program));
  for (const k of Object.keys(doc)) {
    if (!["datars", "id", "title", "description", "size", "theme", "locale", "packages", "data", "tables", "signals", "keys", "scene", "motion", "program"].includes(k)) put(propKey(k), L.value(doc[k]));
  }

  const out = [];
  for (const line of opts.comment ?? []) out.push(line ? `// ${line}` : "//");
  const sdk = [...L.sdk].sort((a, b) => (a === "doc" ? -1 : b === "doc" ? 1 : a.localeCompare(b)));
  out.push(`import { ${sdk.join(", ")} } from "@datars/sdk";`);
  if (L.std.size) out.push(`import { ${[...L.std].sort().map((n) => (n === "route" ? "route as routeLine" : n)).join(", ")} } from "@datars/std";`);
  const byModule = new Map();
  for (const { name, from } of L.local.values()) byModule.set(from, [...(byModule.get(from) ?? []), name]);
  for (const [from, names] of byModule) out.push(`import { ${[...new Set(names)].sort().join(", ")} } from ${str(from)};`);
  out.push(...imports);
  out.push("");
  for (const h of [...L.helpers].sort()) out.push(...HELPERS[h], "");
  for (const [name, code, comment] of decls) {
    if (comment) out.push(`// ${comment}`);
    out.push(`const ${name} = ${print(code, 0, 7 + name.length + 3)};`);
    out.push("");
  }
  out.push(`export default ${print(call("doc", [obj(entries)]), 0, 15)};`);
  return out.join("\n") + "\n";
}

/** A theme module: `export const <name> = theme({ … })` for themes several documents share. */
export function themeToTs(def, name, comment = []) {
  const L = new Lifter();
  const code = print(L.themeDef(def), 0, 14 + name.length);
  return [...comment.map((l) => `// ${l}`), `import { theme } from "@datars/sdk";`, "", `export const ${name} = ${code};`, ""].join("\n");
}

const SDK_NAMES = ["doc", "e", "expr", "lit", "field", "group", "view", "geom", "shape", "text", "repeat", "instances", "tiles", "use", "data", "op", "table", "recipe", "t", "scale", "step", "story", "scrolly", "autoplay", "chapter", "interactive", "film", "loop", "dashboard", "motion", "choreo", "route", "ghost", "theme", "font", "signal", "control", "scrub", "brush", "brushed", "FORMAT"];
const STD_NAMES = ["plot", "axis", "grid", "bar", "line", "area", "point", "dot", "pareto", "pie", "donut", "legend", "title", "rule", "span", "annotate", "card", "cell", "stripes", "stacked", "grouped", "treemap", "waffle", "hemicycle", "swarm", "sankey", "waterfall", "funnel", "calendar", "facet", "map", "symbols", "geoPoints", "geoLines", "track", "dotDensity", "slider", "basemap", "attribution"];

// ---- factoring: repetition → a table of rows and one template --------------------------------------

// A story is usually one scene per step — the same chart, title and caption, a few values apart —
// and the IR spells each one out. Anti-unification finds the shared template of such siblings and
// the values where they differ ("holes"); the siblings become `rows.map((s) => template)` over a
// table of those values. Tables that line up (the charts, their captions and the program's steps
// all keyed by the same scene names) merge into one, so a story reads as its table of scenes
// followed by templates. Everything is exact: holes carry the original values, and the check
// compiles the output back.

const HOLE = Symbol("hole");
const size = (v) => JSON.stringify(v)?.length ?? 0;

/** The template shared by `vals`, recording where they differ in `holes`. */
function antiUnify(vals, path, holes) {
  const first = vals[0];
  if (vals.every((v) => deepEqual(v, first))) return first;
  const hole = (kind, values, extra = {}) => {
    const h = { path, kind, values, ...extra };
    holes.push(h);
    return h;
  };
  // Strings (and expressions) that differ in the middle only: `scene == "asia"` → `scene == "${s.scene}"`.
  const allExpr = vals.every(isExprObj);
  if (allExpr || vals.every((v) => typeof v === "string")) {
    const ss = vals.map((v) => (allExpr ? v.expr : v));
    const t = splitCommon(ss, allExpr);
    if (t) {
      const h = hole("string", t.middles, { prefix: t.prefix, suffix: t.suffix });
      return { [TPL]: [[t.prefix, t.suffix], [h], allExpr], [HOLE]: h };
    }
    return { [HOLE]: hole("value", vals) };
  }
  // Nodes of different kinds or recipes are different things: the whole node is the hole.
  if (vals.some(isNode) && !vals.every((v) => isNode(v) && v.kind === first.kind && v.recipe === first.recipe)) return { [HOLE]: hole("value", vals) };
  if (vals.every((v) => isPlain(v) && !isExprObj(v))) {
    // Keys some of them lack are holes whose value is `undefined` there: the SDK drops undefined
    // fields (`clean`), so `title: s.title` builds exactly the object without a title.
    const all = [...new Set(vals.flatMap((v) => Object.keys(v)))];
    const optional = all.filter((k) => !vals.every((v) => k in v));
    if (optional.length <= 3 && !(isNode(first) && optional.some((k) => k === "kind" || k === "recipe"))) {
      const out = {};
      for (const k of all) {
        out[k] = optional.includes(k) ? { [HOLE]: hole("value", vals.map((v) => v[k]), { path: [...path, k] }) } : antiUnify(vals.map((v) => v[k]), [...path, k], holes);
      }
      return out;
    }
    return { [HOLE]: hole("value", vals) };
  }
  // Lists of numbers (a point, a box, padding) are one value: `box: [120, 45, 146, 30]`.
  if (vals.every((v) => Array.isArray(v) && v.length > 0 && v.every((x) => typeof x === "number"))) return { [HOLE]: hole("value", vals) };
  if (vals.every(Array.isArray)) {
    const len = first.length;
    if (vals.every((v) => v.length === len)) return first.map((_, i) => antiUnify(vals.map((v) => v[i]), [...path, i], holes));
    // Different lengths: the equal head and tail stay, the middle is a spread (a map's labels).
    let head = 0;
    const min = Math.min(...vals.map((v) => v.length));
    while (head < min && vals.every((v) => deepEqual(v[head], first[head]))) head++;
    let tail = 0;
    while (tail < min - head && vals.every((v) => deepEqual(v[v.length - 1 - tail], first[first.length - 1 - tail]))) tail++;
    const h = hole("tail", vals.map((v) => v.slice(head, v.length - tail)));
    return [...first.slice(0, head), { [SPREAD]: h, [HOLE]: h }, ...first.slice(first.length - tail)];
  }
  return { [HOLE]: hole("value", vals) };
}

/** Common prefix and suffix of strings whose middles are short atoms (names, numbers): worth a
 * template. Expressions allow any middle without quotes; prose needs a substantial frame. */
function splitCommon(ss, isExpr) {
  let p = 0;
  const min = Math.min(...ss.map((s) => s.length));
  while (p < min && ss.every((s) => s[p] === ss[0][p])) p++;
  let q = 0;
  while (q < min - p && ss.every((s) => s[s.length - 1 - q] === ss[0][ss[0].length - 1 - q])) q++;
  // Don't split inside a word or number: back off to a boundary.
  const atom = /[\w.:-]/;
  while (p > 0 && atom.test(ss[0][p - 1]) && ss.some((s) => atom.test(s[p] ?? ""))) p--;
  while (q > 0 && atom.test(ss[0][ss[0].length - q]) && ss.some((s) => atom.test(s[s.length - q - 1] ?? ""))) q--;
  const middles = ss.map((s) => s.slice(p, s.length - q));
  const frame = p + q;
  const ok = isExpr
    ? frame >= 3 && middles.every((m) => m.length > 0 && m.length <= 40 && !/["'`\\]/.test(m))
    : frame >= 8 && middles.every((m) => /^[\w.:-]{1,24}$/.test(m));
  return ok ? { prefix: ss[0].slice(0, p), suffix: ss[0].slice(ss[0].length - q), middles } : null;
}

/** Is factoring `vals` into `tpl` + holes worth it (shorter, and few enough holes to read)? */
function worthIt(vals, tpl, holes) {
  if (holes.length === 0 || holes.length > 8) return false;
  if (holes.some((h) => h.path.length === 0)) return false; // the whole node differs
  const before = vals.reduce((a, v) => a + size(v), 0);
  const after = size(tpl) + holes.reduce((a, h) => a + h.values.reduce((b, x) => b + size(x) + 8, 0), 0);
  return after < before * 0.8;
}

/** A readable field name for a hole, from where it sits. */
function holeName(h, tplRoot) {
  if (h.kind === "string" && h.prefix) {
    const m = h.prefix.match(/([A-Za-z_]\w*)\s*(==|!=)\s*"$/);
    if (m) return m[1];
    // A feature of a geo source: `geo.cx("geo:routes", "…")` → route.
    const g = h.prefix.match(/"geo:(\w+)",\s*"$/);
    if (g) return camel(singular(g[1]));
    if (/\(\s*"$/.test(h.prefix)) return "id";
  }
  const keys = h.path.filter((k) => typeof k === "string" && k !== "params" && k !== "expr");
  let name = keys[keys.length - 1] ?? "value";
  // A recipe's `text` (a title, a caption card) is named by the node's key.
  if (name === "text" || name === "rest") {
    let node = tplRoot;
    let owner = null;
    for (const k of h.path) {
      if (isPlain(node) && node.kind === "use") owner = node;
      node = node?.[k];
    }
    if (owner && typeof owner.key === "string") name = name === "rest" ? `${owner.key}s` : owner.key;
    else if (owner && owner !== tplRoot) name = name === "rest" ? plural(owner.recipe.split("/").pop()) : owner.recipe.split("/").pop();
  }
  // A whole node that differs (each scene's chart) is named for what it is.
  if (h.kind === "value" && h.values.every(isNode)) {
    const k = h.values[0].key;
    name = typeof k === "string" && !["body", "chart"].includes(k) ? k : "chart";
  }
  if (h.kind === "tail") {
    const recipes = new Set(h.values.flat().map((n) => (isPlain(n) && n.kind === "use" ? n.recipe.split("/").pop() : "?")));
    if (recipes.size === 1 && !recipes.has("?")) name = plural([...recipes][0]);
  }
  return camel(name);
}
const plural = (w) => ({ annotate: "notes", card: "cards", children: "items" })[w] ?? (w.endsWith("s") ? w : `${w}s`);
const singular = (w) => (w.endsWith("ies") ? `${w.slice(0, -3)}y` : w.endsWith("s") ? w.slice(0, -1) : w);

// ---- idioms: IR patterns written with a small helper ----------------------------------------------

const CALL = "~call"; // { "~call": helper, ...args } — a plain object, so anti-unification sees it

/** Helpers a document may use, by name: their definition (printed above the document). */
const HELPERS = {
  geoBox: [
    "/** A lon/lat box [west, north, east, south] as a camera fits it: its projected corners. */",
    "const geoBox = ([w, n, east, s]: number[]) => [",
    "  e(`geo.x(${w}, ${n})`), e(`geo.y(${w}, ${n})`), e(`geo.x(${east}, ${s})`), e(`geo.y(${east}, ${s})`),",
    "];",
  ],
};

/** `[geo.x(a, b), geo.y(a, b), geo.x(c, d), geo.y(c, d)]` → `geoBox([a, b, c, d])` — exact when the
 * numbers print back as written. */
function idioms(v) {
  if (Array.isArray(v)) {
    if (v.length === 4 && v.every(isExprObj)) {
      const m = v.map((x, i) => x.expr.match(new RegExp(`^geo\\.${i % 2 ? "y" : "x"}\\((-?[\\d.]+), (-?[\\d.]+)\\)$`)));
      const canon = (s) => String(Number(s)) === s;
      if (m.every(Boolean) && m[0][1] === m[1][1] && m[0][2] === m[1][2] && m[2][1] === m[3][1] && m[2][2] === m[3][2]
        && [m[0][1], m[0][2], m[2][1], m[2][2]].every(canon)) {
        return { [CALL]: "geoBox", box: [m[0][1], m[0][2], m[2][1], m[2][2]].map(Number) };
      }
    }
    return v.map(idioms);
  }
  if (isPlain(v)) return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, idioms(x)]));
  return v;
}

/** Replace hole markers in a template by references to the row's fields. */
function bindHoles(tpl, param) {
  if (tpl === null || typeof tpl !== "object") return tpl;
  if (tpl[HOLE]) {
    const h = tpl[HOLE];
    const code = `${param}.${h.field}`;
    if (tpl[TPL]) return { [TPL]: [tpl[TPL][0], [code], tpl[TPL][2]] };
    // A list factored in turn: `...s.notes.map((n) => annotate(…))` over each row's sub-rows.
    const sub = h.column?.sub;
    if (tpl[SPREAD] && sub) return { [MAP]: { rows: code, param: sub.param, template: bindHoles(sub.tpl, sub.param), lift: "node" } };
    if (tpl[SPREAD]) return { [SPREAD]: code };
    return ref(code);
  }
  if (Array.isArray(tpl)) return tpl.map((x) => bindHoles(x, param));
  return Object.fromEntries(Object.entries(tpl).map(([k, v]) => [k, bindHoles(v, param)]));
}

/** A column holding a list of nodes per row (each scene's map labels): when the nodes across all
 * rows share a template, the rows hold small objects instead (`{ route, text }`) and the template
 * maps them — one level deep. */
function subRows(h) {
  const c = h.column;
  if (h.kind !== "tail" || c.sub || c.values !== h.values) return;
  const lists = c.values;
  if (!lists.every((l) => Array.isArray(l) && l.every(isNode))) return;
  const flat = lists.flat();
  if (flat.length < 2 || !flat.every((n) => n.kind === flat[0].kind && n.recipe === flat[0].recipe)) return;
  const holes = [];
  const tpl = antiUnify(flat, [], holes);
  if (!worthIt(flat, tpl, holes) || holes.some((x) => x.kind === "tail")) return;
  const cols = [];
  for (const x of holes) {
    const same = cols.find((y) => deepEqual(y.values, x.values));
    if (same) { x.column = same; continue; }
    const y = { field: holeName(x, flat[0]), values: x.values };
    cols.push(y);
    x.column = y;
  }
  const seen = new Set();
  for (const y of cols) {
    let f = y.field;
    for (let k = 2; seen.has(f); k++) f = `${y.field}${k}`;
    seen.add(f);
    y.field = f;
  }
  for (const x of holes) x.field = x.column.field;
  let k = 0;
  c.values = lists.map((l) => l.map(() => {
    const i = k++;
    return Object.fromEntries(cols.map((y) => [y.field, y.values[i]]));
  }));
  c.sub = { param: "n", tpl };
}

const hasMarker = (v) => v !== null && typeof v === "object" && (isMarker(v) || v[HOLE] !== undefined || Object.values(v).some(hasMarker));

/**
 * Data columns that follow a rule, written as the rule: a cycle (`MONTHS[i % 12]`), a count that
 * steps every k rows (`1950 + Math.floor(i / 12)`), a sequence, a constant. Each rule is checked
 * against every value, so the column is exactly the one it replaces.
 */
function compactColumns(values) {
  if (!isPlain(values)) return values;
  const out = {};
  for (const [name, col] of Object.entries(values)) out[name] = Array.isArray(col) ? compactColumn(col) : col;
  return out;
}

function compactColumn(col) {
  const n = col.length;
  if (n < 24) return col;
  const same = (f) => col.every((v, i) => Object.is(v, f(i)));
  const lit = (v) => (typeof v === "string" ? str(v) : num(v));
  const from = (body) => ({ [CODE]: call("Array.from", [raw(`{ length: ${n} }`), arrow("_, i", body)]) });
  if (same(() => col[0])) return ref(`Array(${n}).fill(${lit(col[0])})`);
  // Names numbered by row: `p0`, `p1`, …
  const numbered = typeof col[0] === "string" && col[0].match(/^(.*?)0$/);
  if (numbered && same((i) => `${numbered[1]}${i}`) && !/[`$\\]/.test(numbered[1])) return from(raw(`\`${numbered[1]}\${i}\``));
  // A cycle: the first p values over and over.
  for (let p = 2; p <= Math.min(24, n / 2); p++) {
    if (same((i) => col[i % p])) {
      const cycle = col.slice(0, p);
      const d = cycle[1] - cycle[0];
      if (cycle.every((v, i) => Number.isInteger(v) && v === cycle[0] + i * d)) {
        const step = `(i % ${p})${d === 1 ? "" : ` * ${d}`}`;
        return from(raw(cycle[0] === 0 ? step.replace(/^\((.*)\)$/, "$1") : `${cycle[0]} + ${step}`));
      }
      if (cycle.every((v) => typeof v === "string" || typeof v === "number")) return from(seq(arr(cycle.map((v) => raw(lit(v))), { fill: p > 8 }), raw(`[i % ${p}]`)));
    }
  }
  // Runs: each of a few values k times (a series per block of rows).
  let k = 1;
  while (k < n && Object.is(col[k], col[0])) k++;
  if (k > 1 && n % k === 0 && n / k <= 24) {
    const runs = Array.from({ length: n / k }, (_, j) => col[j * k]);
    if (runs.every((v) => typeof v === "string" || typeof v === "number") && same((i) => runs[Math.floor(i / k)]) && !(runs.every(Number.isInteger) && runs.every((v, j) => v === runs[0] + j * (runs[1] - runs[0])))) {
      return from(seq(arr(runs.map((v) => raw(lit(v))), { fill: runs.length > 8 }), raw(`[Math.floor(i / ${k})]`)));
    }
  }
  // Integers that step by d every k rows (years × 12 months; a plain sequence when k = 1).
  if (col.every((v) => Number.isInteger(v))) {
    let k = 1;
    while (k < n && col[k] === col[0]) k++;
    const d = (col[k] ?? col[0]) - col[0];
    const a = col[0];
    const rule = k === 1 ? `${a} + i${d === 1 ? "" : ` * ${d}`}` : `${a} + Math.floor(i / ${k})${d === 1 ? "" : ` * ${d}`}`;
    if (d !== 0 && same((i) => a + Math.floor(i / k) * d)) return from(raw(rule.replace(/^0 \+ /, "")));
  }
  return col;
}

/** The same node written out in several places (a map's symbol layer in two scenes that aren't
 * neighbours): declared once as a const and referenced. Only whole, sizable, marker-free nodes. */
function hoist(doc, fresh) {
  const count = new Map();
  const visit = (v, f) => {
    if (Array.isArray(v)) return v.forEach((x) => visit(x, f));
    if (!v || typeof v !== "object" || isMarker(v)) return;
    if (isNode(v) && f(v) === false) return;
    Object.values(v).forEach((x) => visit(x, f));
  };
  visit(doc.scene, (n) => {
    if (hasMarker(n)) return;
    const s = JSON.stringify(n);
    if (s.length >= 160) count.set(s, (count.get(s) ?? 0) + 1);
  });
  const names = new Map();
  const replace = (v) => {
    if (Array.isArray(v)) return v.map(replace);
    if (!v || typeof v !== "object" || isMarker(v)) return v;
    if (isNode(v) && !hasMarker(v)) {
      const s = JSON.stringify(v);
      if ((count.get(s) ?? 0) >= 2) {
        if (!names.has(s)) {
          const src = v.params?.source;
          const base = typeof src === "string" ? src.replace(/^geo:/, "") : typeof v.key === "string" && !["chart", "body", "title"].includes(v.key) ? v.key : v.kind === "use" ? v.recipe.split("/").pop() : v.kind;
          names.set(s, { name: fresh(base), node: v });
        }
        return ref(names.get(s).name);
      }
    }
    if (v[MAP]) return v; // templates keep their own nodes
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, replace(x)]));
  };
  doc.scene = replace(doc.scene);
  return [...names.values()];
}

/** Runs of siblings in `arr` that share a template: [start, end) with its holes. */
function runs(arr, same) {
  const out = [];
  let i = 0;
  while (i < arr.length) {
    let j = i + 1;
    while (j < arr.length && same(arr[i], arr[j])) j++;
    if (j - i >= 2) out.push([i, j]);
    i = j;
  }
  return out;
}
const sameNodeKind = (a, b) => isNode(a) && isNode(b) && a.kind === b.kind && a.recipe === b.recipe && JSON.stringify(a.key) === JSON.stringify(b.key);

/**
 * Factor a document in place: node sibling runs anywhere in the scene, the program's steps and the
 * motion rules. Returns the row tables to declare: `{ name, rows, comment }`.
 */
function factor(doc, fresh) {
  const tables = []; // { base, columns: [{ field, values }], n, uses: [markers] }
  const param = "s";

  /** `joinsOnly`: factor even when not shorter, if every column is one a table already has (the
   * steps of a story whose scenes are a table: `scenes.map((s) => step(s.scene, …))`). */
  function factorRun(vals, lift, baseName, joinsOnly = false) {
    const holes = [];
    const tpl = antiUnify(vals, [], holes);
    // Holes with the same values are one column.
    const columns = [];
    for (const h of holes) {
      const same = columns.find((c) => deepEqual(c.values, h.values));
      if (same) { h.column = same; continue; }
      const c = { field: holeName(h, vals[0]), values: h.values };
      columns.push(c);
      h.column = c;
    }
    const joins = joinsOnly && holes.length > 0 && holes.length <= 8 && !holes.some((h) => h.path.length === 0)
      && columns.every((c) => tables.some((t) => t.n === vals.length && t.columns.some((tc) => deepEqual(tc.values, c.values))));
    if (!joins && !worthIt(vals, tpl, holes)) return null;
    for (const h of holes) if (h.kind === "tail" && h.column.values === h.values) subRows(h);
    const marker = { [MAP]: { rows: null, param, template: tpl, lift, holes } };
    tables.push({ base: baseName, columns, n: vals.length, markers: [marker] });
    return marker;
  }

  // Scene: top-down, so an outer run's template is factored before its insides.
  function walk(n) {
    if (Array.isArray(n)) {
      if (n.length >= 2 && n.some(isNode)) {
        const rs = runs(n, sameNodeKind);
        for (const [a, b] of rs.reverse()) {
          const vals = n.slice(a, b);
          const key = typeof vals[0].key === "string" ? vals[0].key : vals[0].kind === "use" ? vals[0].recipe.split("/").pop() : vals[0].kind;
          const m = factorRun(vals, "node", plural(key));
          if (m) n.splice(a, b - a, m);
        }
      }
      n.forEach(walk);
      return;
    }
    if (!n || typeof n !== "object") return;
    if (n[MAP]) return walk(n[MAP].template);
    if (n[HOLE] && !n[SPREAD]) return;
    for (const v of Object.values(n)) walk(v);
  }
  walk(doc.scene);

  const states = doc.program?.states;
  if (Array.isArray(states) && states.length >= 2) {
    const m = factorRun(states, "step", "steps", true);
    if (m) doc.program.states = m;
  }

  // Merge tables that line up: same length and a column with the same values.
  for (let i = 0; i < tables.length; i++) {
    for (let j = i + 1; j < tables.length; j++) {
      const A = tables[i], B = tables[j];
      if (!A || !B || A.n !== B.n) continue;
      if (!B.columns.some((cb) => A.columns.some((ca) => deepEqual(ca.values, cb.values)))) continue;
      for (const cb of B.columns) {
        const ca = A.columns.find((c) => deepEqual(c.values, cb.values));
        if (ca) { for (const m of B.markers) for (const h of m[MAP].holes) if (h.column === cb) h.column = ca; continue; }
        A.columns.push(cb);
      }
      A.markers.push(...B.markers);
      if (B.columns.some((c) => c.field === "scene") || A.columns.some((c) => c.field === "scene")) A.base = "scenes";
      tables[j] = null;
    }
  }

  const out = [];
  for (const t of tables.filter(Boolean)) {
    // Unique field names within the table.
    const seen = new Set();
    for (const c of t.columns) {
      let f = c.field;
      for (let k = 2; seen.has(f); k++) f = `${c.field}${k}`;
      seen.add(f);
      c.field = f;
    }
    const name = fresh(t.base);
    for (const m of t.markers) {
      for (const h of m[MAP].holes) h.field = h.column.field;
      m[MAP].template = bindHoles(m[MAP].template, param);
      m[MAP].rows = name;
    }
    const rows = Array.from({ length: t.n }, (_, i) => Object.fromEntries(t.columns.map((c) => [c.field, c.values[i]])));
    out.push({ name, rows });
  }
  return out;
}

// ---- check ----------------------------------------------------------------------------------------------

/** What the comparison ignores: fields the SDK fills with their defaults, and empty chapters. */
export function normalize(doc) {
  const d = structuredClone(doc);
  d.datars ??= 1;
  d.locale ??= "en";
  if (d.program?.chapters && !Object.keys(d.program.chapters).length) delete d.program.chapters;
  return sortKeys(d);
}

export { Lifter, print, flat, raw, arr, obj, call, seq, arrow, spread, str, propKey, num, isPlain, isExprObj, REF, TPL, MAP, ref, NODE_ORDER, deepEqual, sortKeys };

// ---- CLI --------------------------------------------------------------------------------------------------

async function main() {
  const args = process.argv.slice(2);
  const flag = (name) => { const i = args.indexOf(name); return i >= 0 ? args.splice(i, 2)[1] : undefined; };
  const check = args.includes("--check") && args.splice(args.indexOf("--check"), 1);
  const outFile = flag("--out");
  const comment = flag("--comment");
  const themeArg = flag("--theme");
  const dataLimit = Number(flag("--data-limit") ?? 20000);
  const file = resolve(args[0]);
  const doc = JSON.parse(readFileSync(file, "utf8"));
  const dir = dirname(outFile ? resolve(outFile) : file);
  let theme;
  if (themeArg) {
    const [from, name] = themeArg.split(":");
    const json = execFileSync("node", ["-e", `import(${JSON.stringify(resolve(dir, from))}).then((m) => process.stdout.write(JSON.stringify(m[${JSON.stringify(name)}])))`]).toString();
    theme = { from, name, def: JSON.parse(json) };
  }
  const ts = docToTs(doc, {
    comment: comment ? comment.split("\n") : undefined, theme, dataLimit,
    writeData: (name, values) => { const f = `${name.replace(/[^A-Za-z0-9_-]+/g, "-")}.json`; writeFileSync(join(dir, f), JSON.stringify(values)); return `./${f}`; },
  });
  if (outFile) writeFileSync(outFile, ts);
  else process.stdout.write(ts);
  if (check && outFile) {
    const r = checkRoundTrip(file, resolve(outFile));
    console.error(r.ok ? `ok: ${r.states} states, same document and scenes` : `MISMATCH: ${r.why}`);
    if (!r.ok) process.exit(1);
  }
}

/** Compile `tsFile` and compare it with `jsonFile`: the document (normalized) and every state's
 * scene hash. */
export function checkRoundTrip(jsonFile, tsFile) {
  const cli = join(root, "target/release/datars");
  const a = JSON.parse(readFileSync(jsonFile, "utf8"));
  const text = execFileSync("node", [join(root, "scripts/doc-to-json.mjs"), tsFile], { maxBuffer: 1 << 30 }).toString();
  const b = JSON.parse(text);
  const na = normalize(a), nb = normalize(b);
  if (JSON.stringify(na) !== JSON.stringify(nb)) return { ok: false, why: firstDiff(na, nb) };
  // The compiled document resolves next to the original (data files, tiles).
  const tmp = join(dirname(jsonFile), `.roundtrip-${basename(jsonFile)}`);
  writeFileSync(tmp, text);
  try {
    const states = execFileSync(cli, ["states", jsonFile]).toString().trim().split("\n").filter(Boolean).length || 1;
    for (let i = 0; i < states; i++) {
      const ha = execFileSync(cli, ["inspect", jsonFile, "--state", String(i)], { maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] }).toString().split("\n")[0];
      const hb = execFileSync(cli, ["inspect", tmp, "--state", String(i)], { maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] }).toString().split("\n")[0];
      if (ha !== hb) return { ok: false, why: `state ${i}: ${ha} vs ${hb}` };
    }
    return { ok: true, states };
  } finally {
    rmSync(tmp, { force: true });
  }
}

export function firstDiff(a, b, path = "") {
  if (JSON.stringify(a) === JSON.stringify(b)) return null;
  if (isPlain(a) && isPlain(b)) {
    for (const k of new Set([...Object.keys(a), ...Object.keys(b)])) {
      const d = firstDiff(a[k], b[k], `${path}.${k}`);
      if (d) return d;
    }
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return `${path}: length ${a.length} vs ${b.length}`;
    for (let i = 0; i < a.length; i++) { const d = firstDiff(a[i], b[i], `${path}[${i}]`); if (d) return d; }
  }
  return `${path}: ${JSON.stringify(a)?.slice(0, 200)} vs ${JSON.stringify(b)?.slice(0, 200)}`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
