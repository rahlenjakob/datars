// The charts page (site/pages/features/charts.html): the recipe playground, "your data, every chart
// that fits it", and the catalogue as the playground's picker.
//
// The playground edits real documents: each std recipe's reference figure (site/figures/std/<name>.ts,
// published by the build at /play/std-<name>.json), reduced to its first state. Its controls are
// made from the recipe's own description (`datars describe --json`, at /play/std.json): a field
// becomes a column picker, an enum chips, a bool a switch, and so on. Every change goes to the
// running view with `setDocument`; the engine's sandbox expands the recipe again (the chart's slot
// asks for the engine build with the sandbox by starting from a document, not a bundle) and the
// chart morphs to it. The doc.ts beside it is written from the same document, and compiles back to
// it exactly (checked for every recipe: scripts in the page's PR, see `toTs`).
//
// "Every chart that fits" is a published bundle (site/figures/charts/fits.ts) on the light engine:
// its rows are a data slot, and the reader's own go in with `provideData`.
//
// The pure functions (`reduce`, `toTs`, `parseCsv`, `toCsv`) touch no DOM, so they run in Node too.

// ---- documents ----------------------------------------------------------------------------------

/** Does a `when` like `state == "a" || state != "b"` hold in state `name`? `null`: not that simple. */
export function holds(when, name) {
  const src = typeof when === "string" ? when : when?.expr;
  if (typeof src !== "string") return null;
  let any = false;
  for (const part of src.split("||")) {
    const m = part.trim().match(/^state\s*(==|!=)\s*"([^"]*)"$/);
    if (!m) return null;
    if ((m[1] === "==") === (m[2] === name)) any = true;
  }
  return any;
}

/** A figure's document as one chart: what its first state shows, without the program — nodes for
 * other states dropped, the first state's signal settings folded into the signals' defaults. */
export function reduce(input) {
  const doc = structuredClone(input);
  const first = doc.program?.states?.[0];
  if (!first) return doc;
  let unsure = false;
  const walk = (v) => {
    if (Array.isArray(v)) {
      for (let i = v.length - 1; i >= 0; i--) {
        const n = v[i];
        if (n && typeof n === "object" && n.when) {
          const h = holds(n.when, first.name);
          if (h === false) { v.splice(i, 1); continue; }
          if (h === true) delete n.when;
          else unsure = true;
        }
        walk(n);
      }
    } else if (v && typeof v === "object") {
      for (const x of Object.values(v)) walk(x);
    }
  };
  walk(doc.scene);
  if (unsure) {
    // Keep a program of the first state alone: the engine decides the rest.
    doc.program = { states: [{ name: first.name, ...(first.set && { set: first.set }) }] };
    return doc;
  }
  for (const [name, value] of Object.entries(first.set ?? {})) {
    if (doc.signals?.[name]) doc.signals[name].default = value;
  }
  delete doc.program;
  return doc;
}

const isExpr = (v) => v && typeof v === "object" && !Array.isArray(v) && typeof v.expr === "string" && Object.keys(v).length === 1;
const ident = (k) => (/^[A-Za-z_$][\w$]*$/.test(k) ? k : JSON.stringify(k));

/** A document as the TypeScript that makes it with @datars/sdk and @datars/std — `doc()`,
 * `data.values()`, `group()`, `signal.*()`, `e()` and a call per recipe — which compiles back to the
 * same JSON. `local` maps a local package (`@local/x`) to the path its recipes are imported from. */
export function toTs(doc, { local = {}, header = "" } = {}) {
  const sdk = new Set(["doc"]);
  const std = new Set();
  const locals = new Map(); // path → names
  const W = 96;

  /** One value at depth `d`: inline when it fits, else one entry a line. */
  function v(x, d) {
    if (x === null || x === undefined) return "null";
    if (typeof x !== "object") return JSON.stringify(x);
    if (isExpr(x)) { sdk.add("e"); return `e(${JSON.stringify(x.expr)})`; }
    if (Array.isArray(x)) return list(x.map((y) => v(y, d + 1)), "[", "]", d);
    if (x.kind === "use" && typeof x.recipe === "string") return call(x, d);
    if (x.kind === "group" && Array.isArray(x.children)) {
      sdk.add("group");
      const { kind, ...rest } = x;
      return `group(${obj(rest, d)})`;
    }
    return obj(x, d);
  }
  function obj(o, d, skip = []) {
    const parts = Object.entries(o).filter(([k, y]) => y !== undefined && !skip.includes(k)).map(([k, y]) => `${ident(k)}: ${v(y, d + 1)}`);
    return list(parts, "{", "}", d, true);
  }
  function list(parts, open, close, d, spaced = false) {
    if (!parts.length) return open + close;
    const pad = spaced ? " " : "";
    const flat = `${open}${pad}${parts.join(", ")}${pad}${close}`;
    if (flat.length + d * 2 <= W && !flat.includes("\n")) return flat;
    // Short scalars (a column of numbers) fill lines instead of taking one each.
    if (parts.every((p) => p.length <= 24 && !p.includes("\n") && !/^[{[]/.test(p))) {
      const lines = [];
      let line = "";
      for (const p of parts) {
        if (line && line.length + p.length + 2 + (d + 1) * 2 > W) { lines.push(line); line = ""; }
        line += (line ? " " : "") + p + ",";
      }
      lines.push(line);
      return `${open}\n${lines.map((l) => "  ".repeat(d + 1) + l).join("\n")}\n${"  ".repeat(d)}${close}`;
    }
    return `${open}\n${parts.map((p) => `${"  ".repeat(d + 1)}${p},`).join("\n")}\n${"  ".repeat(d)}${close}`;
  }
  function call(n, d) {
    const { kind, recipe, params, ...opts } = n;
    const at = recipe.lastIndexOf("/");
    const mod = recipe.slice(0, at), name = recipe.slice(at + 1);
    if (mod === "@datars/std") std.add(name);
    else {
      const path = local[mod] ?? `./recipes/${mod.replace(/^@local\//, "")}`;
      if (!locals.has(path)) locals.set(path, new Set());
      locals.get(path).add(name);
    }
    const p = params && Object.keys(params).length ? obj(params, d) : "";
    const o = Object.keys(opts).length ? obj(opts, d) : "";
    return `${name}(${o ? `${p || "{}"}, ${o}` : p})`;
  }
  function source(s, d) {
    if (s && typeof s === "object" && "values" in s && !("url" in s) && Object.keys(s).every((k) => ["values", "key", "types", "live"].includes(k))) {
      sdk.add("data");
      const o = {};
      if (s.key) o.key = s.key.length === 1 ? s.key[0] : s.key;
      if (s.types) o.types = s.types;
      if (s.live) o.live = s.live;
      return `data.values(${v(s.values, d)}${Object.keys(o).length ? `, ${obj(o, d)}` : ""})`;
    }
    if (s && typeof s === "object" && Object.keys(s).length === 1 && typeof s.atlas === "string") {
      sdk.add("data");
      return `data.atlas(${JSON.stringify(s.atlas)})`;
    }
    return v(s, d);
  }
  function sig(s, d) {
    const keys = Object.keys(s).sort().join(",");
    if (keys === "default,type") {
      const k = s.type, def = s.default;
      if (k === "keyset" && Array.isArray(def)) { sdk.add("signal"); return def.length ? `signal.keyset(${v(def, d)})` : "signal.keyset()"; }
      if (k === "bool" && typeof def === "boolean") { sdk.add("signal"); return `signal.bool(${def})`; }
      if (k === "num" && typeof def === "number") { sdk.add("signal"); return def === 0 ? "signal.num()" : `signal.num(${def})`; }
      if (k === "str" && typeof def === "string") { sdk.add("signal"); return def === "" ? "signal.str()" : `signal.str(${JSON.stringify(def)})`; }
      if (k === "key" && (def === null || typeof def === "string")) { sdk.add("signal"); return def === null ? "signal.key()" : `signal.key(${JSON.stringify(def)})`; }
    }
    return v(s, d);
  }

  const { datars, locale, size, data, signals, scene, packages, ...rest } = doc;
  const fields = [];
  for (const k of ["id", "title", "description"]) if (rest[k] !== undefined) fields.push(`${k}: ${v(rest[k], 1)}`);
  if (size) fields.push(`size: [${size.width}, ${size.height}]`);
  if (locale && locale !== "en") fields.push(`locale: ${JSON.stringify(locale)}`);
  if (rest.theme !== undefined) fields.push(`theme: ${v(rest.theme, 1)}`);
  if (data && Object.keys(data).length) fields.push(`data: ${list(Object.entries(data).map(([k, s]) => `${ident(k)}: ${source(s, 2)}`), "{", "}", 1, true)}`);
  if (rest.tables) fields.push(`tables: ${v(rest.tables, 1)}`);
  if (signals && Object.keys(signals).length) fields.push(`signals: ${list(Object.entries(signals).map(([k, s]) => `${ident(k)}: ${sig(s, 2)}`), "{", "}", 1, true)}`);
  if (rest.keys) fields.push(`keys: ${v(rest.keys, 1)}`);
  if (rest.motion !== undefined) fields.push(`motion: ${v(rest.motion, 1)}`);
  fields.push(`scene: ${v(scene, 1)}`);
  if (rest.program) fields.push(`program: ${v(rest.program, 1)}`);
  // Local packages are embedded by the build from their files (`recipes/<name>.ts`): not written here.
  const others = (packages ?? []).filter((p) => !String(p.name).startsWith("@local/"));
  if (others.length) fields.push(`packages: ${v(others, 1)}`);

  const imports = [`import { ${[...sdk].sort().join(", ")} } from "@datars/sdk";`];
  if (std.size) imports.push(`import { ${[...std].join(", ")} } from "@datars/std";`);
  for (const [path, names] of locals) imports.push(`import { ${[...names].join(", ")} } from ${JSON.stringify(path)};`);
  return `${header}${imports.join("\n")}\n\nexport default doc({\n${fields.map((f) => `  ${f},`).join("\n")}\n});\n`;
}

// ---- CSV ----------------------------------------------------------------------------------------

/** Text from a spreadsheet or a CSV file → `{ columns: [names], rows: [[cells]] }`. Tabs (a paste
 * from a spreadsheet), semicolons or commas, whichever the header uses; quotes as in RFC 4180. */
export function parseCsv(text) {
  const head = text.split(/\r?\n/, 1)[0] ?? "";
  const sep = head.includes("\t") ? "\t" : head.includes(";") && !head.includes(",") ? ";" : ",";
  const rows = [];
  let row = [], cell = "", q = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (q) {
      if (c === '"' && text[i + 1] === '"') { cell += '"'; i++; }
      else if (c === '"') q = false;
      else cell += c;
    } else if (c === '"' && cell === "") q = true;
    else if (c === sep) { row.push(cell); cell = ""; }
    else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(cell); cell = "";
      if (row.some((x) => x.trim() !== "")) rows.push(row);
      row = [];
    } else cell += c;
  }
  row.push(cell);
  if (row.some((x) => x.trim() !== "")) rows.push(row);
  const [names = [], ...body] = rows;
  return { columns: names.map((n) => n.trim()), rows: body };
}

const NUM = /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i;
/** Parsed CSV as typed columns `{ name: values }`: a column of numbers is numbers, of true/false
 * booleans, else text; an empty cell is null. */
export function typedColumns({ columns, rows }) {
  const out = {};
  columns.forEach((name, j) => {
    const cells = rows.map((r) => (r[j] ?? "").trim());
    const filled = cells.filter((c) => c !== "");
    const kind = filled.length && filled.every((c) => NUM.test(c.replace(/,/g, ""))) ? "num"
      : filled.length && filled.every((c) => /^(true|false)$/i.test(c)) ? "bool" : "str";
    out[name] = cells.map((c) => (c === "" ? null : kind === "num" ? Number(c.replace(/,/g, "")) : kind === "bool" ? /^true$/i.test(c) : c));
  });
  return out;
}

/** Columns `{ name: values }` (or records) → CSV text. */
export function toCsv(values) {
  const cols = Array.isArray(values)
    ? Object.fromEntries([...new Set(values.flatMap((r) => Object.keys(r)))].map((k) => [k, values.map((r) => r[k])]))
    : values;
  const names = Object.keys(cols);
  const n = Math.max(0, ...names.map((k) => cols[k].length));
  const cell = (x) => {
    if (x === null || x === undefined) return "";
    const s = typeof x === "object" ? JSON.stringify(x) : String(x);
    return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
  };
  const lines = [names.map(cell).join(",")];
  for (let i = 0; i < n; i++) lines.push(names.map((k) => cell(cols[k][i])).join(","));
  return lines.join("\n");
}

// ---- the page -----------------------------------------------------------------------------------

if (typeof document !== "undefined") {
  const { highlight } = await import("./site.js");
  const pg = document.getElementById("recipe-playground");
  if (pg) playground(pg, highlight);
  const fits = document.getElementById("fits");
  if (fits) yourData(fits);
}

/** The slot's <datars-view> once its engine is running. */
function whenReady(slot) {
  return new Promise((resolve) => {
    const look = () => {
      const v = slot.querySelector("datars-view");
      if (!v) return false;
      if (v.status) resolve(v);
      else v.addEventListener("state", () => resolve(v), { once: true });
      return true;
    };
    if (look()) return;
    const mo = new MutationObserver(() => { if (look()) mo.disconnect(); });
    mo.observe(slot, { childList: true });
  });
}

const el = (tag, attrs = {}, ...kids) => {
  const n = document.createElement(tag);
  for (const [k, x] of Object.entries(attrs)) {
    if (x === undefined || x === null || x === false) continue;
    if (k === "text") n.textContent = x;
    else if (k in n && typeof x !== "string") n[k] = x;
    else n.setAttribute(k, x === true ? "" : x);
  }
  for (const k of kids) if (k) n.append(k);
  return n;
};
const firstSentence = (s = "") => (s.match(/^.*?[.!?](\s|$)/)?.[0] ?? s).trim();
/** Messages from the engine worth showing the reader (not fonts still on their way). */
const worth = (diags) => (diags ?? []).filter((d) => !/is not loaded .*no fonts are loaded/.test(d) && !/^font '/.test(d));

/** Settings an enclosing plot hands its marks (the SDK's list): edited where they're set. */
const INHERITED = new Set(["data", "x", "y", "color", "xType", "yType", "clip", "series"]);
/** Theme inks offered for an ink parameter, besides a colour of the reader's own. */
const INKS = ["$accent", "$mark", "$highlight", "$muted", "$ink", "$categorical[1]", "$categorical[2]", "$categorical[3]", "$up", "$down"];
const FORMATS = [",.0f", ",.1~f", ".0%", ".1%", "$,.2s", ",.2s", ".3~g"];

function playground(section, highlight) {
  const slot = section.querySelector(".chart[data-src]");
  const own = slot.closest("[data-own-look]");
  const picker = section.querySelector("#rp-recipe");
  const prevBtn = section.querySelector(".rp-prev");
  const nextBtn = section.querySelector(".rp-next");
  const about = section.querySelector(".rp-about");
  const notes = section.querySelector(".rp-notes");
  const form = section.querySelector(".rp-settings");
  const csvBox = section.querySelector("#rp-csv");
  const csvNote = section.querySelector(".rp-csv-note");
  const csvReset = section.querySelector(".rp-csv-reset");
  const codeEl = section.querySelector("#rp-code");
  const copyBtn = section.querySelector(".rp-copy");
  const root = new URL(slot.dataset.src.replace(/\/c\/[^/]+$/, "/"), location.href);
  const reduced = matchMedia("(prefers-reduced-motion: reduce)");

  const docs = new Map(); // recipe → reduced document as published
  const fetchDoc = (name) => {
    if (!docs.has(name)) docs.set(name, fetch(new URL(`play/std-${name.toLowerCase()}.json`, root)).then((r) => r.json()).then(reduce));
    return docs.get(name);
  };
  const stdReady = fetch(new URL("play/std.json", root)).then((r) => r.json());

  let std = null; // { groups, recipes }
  let order = []; // recipe names in the catalogue's order
  let name = null; // the recipe on screen
  let doc = null; // the working document
  let target = null, parent = null; // the recipe's node, and the plot (or recipe) it sits in
  let csvSource = null; // the data source the CSV box edits
  let view = null;
  let loading = 0;

  const want = () => {
    const m = location.hash.match(/^#play=([A-Za-z0-9]+)$/);
    return m ? m[1] : null;
  };

  // ---- the document's parts ----
  function find(d, recipe) {
    let found = null, up = null;
    const walk = (v, host) => {
      if (found) return;
      if (Array.isArray(v)) return v.forEach((x) => walk(x, host));
      if (!v || typeof v !== "object") return;
      if (v.kind === "use" && v.recipe === recipe) { found = v; up = host; return; }
      const next = v.kind === "use" ? v : host;
      for (const x of Object.values(v)) walk(x, next);
    };
    walk(d.scene, null);
    return [found, up];
  }
  const spec = (r) => std.recipes[r]?.params ?? {};
  const desc = () => spec(target.recipe);
  /** Where a parameter's value lives: the recipe's own params, or — for one its plot hands down
   * and the recipe doesn't set itself — the plot's, which owns the scales (a mark's own `color`
   * would ask the plot for a colour scale it never made). */
  const home = (k) => (INHERITED.has(k) && parent && target.params?.[k] === undefined && k in spec(parent.recipe) ? parent : target);
  /** A setting's value: the recipe's (or where it's handed down from), or `owner`'s own. */
  const read = (k, owner) => (owner ?? home(k)).params?.[k];
  function write(k, value, owner) {
    const node = owner ?? home(k);
    node.params ??= {};
    const def = spec(node.recipe)[k]?.default ?? desc()[k]?.default;
    if (value === undefined || value === "" || (def !== undefined && JSON.stringify(value) === JSON.stringify(def))) delete node.params[k];
    else node.params[k] = value;
  }
  /** The source a table name reads (a derived table's `from`, followed). */
  function sourceOf(table) {
    let t = table;
    for (let i = 0; i < 8 && doc.tables?.[t]; i++) t = doc.tables[t].from;
    return doc.data?.[t] ? t : null;
  }
  function columnsOf(table) {
    const s = sourceOf(table);
    const vals = s && doc.data[s].values;
    const cols = !vals ? [] : Array.isArray(vals) ? [...new Set(vals.flatMap((r) => Object.keys(r)))] : Object.keys(vals);
    // Columns the derived tables on the way add.
    let t = table;
    for (let i = 0; i < 8 && doc.tables?.[t]; i++) {
      for (const o of doc.tables[t].ops ?? []) if (typeof o.as === "string" && !cols.includes(o.as)) cols.push(o.as);
      t = doc.tables[t].from;
    }
    return cols;
  }
  const tableNow = () => read("data") ?? parent?.params?.data;

  // ---- the controls ----
  function controls() {
    form.replaceChildren();
    const params = desc();
    const kinds = { field: 0, table: 0, enum: 1, bool: 1, number: 1, ink: 1, string: 2, prop: 2, json: 2 };
    const rows = [[], [], []];
    for (const [k, p] of Object.entries(params).sort(([a], [b]) => (a === "data" ? -1 : b === "data" ? 1 : 0))) {
      if (!(p.type in kinds)) continue;
      rows[kinds[p.type]].push(row(k, p));
    }
    const groups = [["Data", rows[0]], ["Look", rows[1]], ["Text and expressions", rows[2]]];
    for (const [title, list] of groups) {
      if (!list.length) continue;
      const box = title === "Text and expressions" ? el("details", { class: "rp-more" }, el("summary", { text: `${title} (${list.length})` })) : el("div", { class: "rp-group" }, el("h3", { text: title }));
      box.append(...list);
      form.append(box);
    }
    // The frame around a mark (a plot's title, axes, padding): its own settings, folded away.
    if (parent && parent !== target) {
      const own = Object.entries(spec(parent.recipe)).filter(([k, p]) => !INHERITED.has(k) && p.type in kinds && p.type !== "field" && p.type !== "table");
      if (own.length) {
        const name = parent.recipe.replace("@datars/std/", "");
        const box = el("details", { class: "rp-more" }, el("summary", { text: `Around it: ${name}() (${own.length})` }));
        box.append(...own.map(([k, p]) => row(k, p, parent)));
        form.append(box);
      }
    }
    if (!form.childElementCount) form.append(el("p", { class: "rp-hint", text: "This recipe has no settings to edit here." }));
  }

  let uid = 0;
  function row(k, p, owner) {
    const id = `rp-${k}-${++uid}`;
    const cur = read(k, owner);
    const label = el("label", { for: id }, el("code", { text: k }), p.default !== undefined && p.type !== "bool" ? el("span", { class: "rp-def", text: `default ${typeof p.default === "string" ? JSON.stringify(p.default) : JSON.stringify(p.default)}` }) : null);
    const wrap = el("div", { class: `rp-row rp-${p.type}` }, label);
    const hint = p.doc ? el("p", { class: "rp-hint", text: p.doc }) : null;
    let input;
    const set = (value, now = true) => { write(k, value, owner); apply(now); };
    switch (p.type) {
      case "field":
      case "table": {
        const options = p.type === "table" ? [...Object.keys(doc.data ?? {}), ...Object.keys(doc.tables ?? {})] : columnsOf(tableNow());
        if (typeof cur === "string" && !options.includes(cur)) options.push(cur);
        input = el("select", { id }, el("option", { value: "", text: "—" }), ...options.map((o) => el("option", { value: o, text: o, selected: o === cur })));
        if (cur === undefined) input.value = "";
        input.addEventListener("change", () => {
          set(input.value || undefined);
          if (k === "data") { controls(); dataBox(); }
        });
        break;
      }
      case "enum": {
        const v = cur ?? p.default;
        input = el("div", { class: "chips small", role: "group", "aria-label": k }, ...p.values.map((o) => el("button", { type: "button", "data-v": o, "aria-pressed": String(o === v), text: o })));
        input.addEventListener("click", (e) => {
          const b = e.target.closest("button[data-v]");
          if (!b) return;
          for (const o of input.children) o.setAttribute("aria-pressed", String(o === b));
          set(b.dataset.v);
        });
        break;
      }
      case "bool": {
        const box = el("input", { type: "checkbox", id, checked: !!(cur ?? p.default) });
        box.addEventListener("change", () => set(box.checked));
        wrap.replaceChildren(el("label", { class: "mp-switch rp-switch", for: id }, box, el("span", {}, el("code", { text: k }))));
        if (hint) wrap.append(hint);
        return wrap;
      }
      case "number": {
        const def = Number(p.default ?? 0);
        const v = typeof cur === "number" ? cur : def;
        const frac = (def > 0 && def < 1) || /inner|opacity|pad|share|dim|smooth/i.test(k);
        const step = frac ? (def && def < 0.05 ? 0.001 : 0.01) : 1;
        const max = frac ? (def && def < 0.05 ? 0.1 : 1) : Math.max(20, Math.ceil(Math.abs(def) * 4), Math.abs(v));
        const min = def < 0 || v < 0 ? -max : 0;
        const range = el("input", { type: "range", min: String(min), max: String(max), step: String(step), value: String(v), "aria-label": k });
        const num = el("input", { type: "number", id, step: String(step), value: String(v), class: "rp-num" });
        let t = 0;
        range.addEventListener("input", () => {
          num.value = range.value;
          write(k, Number(range.value), owner);
          // Follows the drag, at most every 90 ms: each change is a new expansion and a morph.
          if (!t) t = setTimeout(() => { t = 0; apply(true); }, 90);
        });
        num.addEventListener("input", () => {
          if (num.value === "" || !Number.isFinite(Number(num.value))) return;
          range.value = num.value;
          set(Number(num.value), false);
        });
        input = el("div", { class: "rp-range" }, range, num);
        break;
      }
      case "ink": {
        const v = cur ?? p.default;
        const color = el("input", { type: "color", value: /^#[0-9a-f]{6}$/i.test(v ?? "") ? v : "#e4572e", "aria-label": `${k}: a colour of your own`, title: "A colour of your own" });
        const chips = el("div", { class: "chips small", role: "group", "aria-label": k }, ...INKS.map((o) => el("button", { type: "button", "data-v": o, "aria-pressed": String(o === v), text: o })), color);
        chips.addEventListener("click", (e) => {
          const b = e.target.closest("button[data-v]");
          if (!b) return;
          for (const o of chips.querySelectorAll("button")) o.setAttribute("aria-pressed", String(o === b));
          set(b.dataset.v);
        });
        color.addEventListener("input", () => {
          for (const o of chips.querySelectorAll("button")) o.setAttribute("aria-pressed", "false");
          set(color.value, false);
        });
        input = chips;
        break;
      }
      case "string": {
        input = el("input", { type: "text", id, value: typeof cur === "string" ? cur : "", placeholder: p.default ?? "", spellcheck: "false", autocomplete: "off" });
        if (k === "format") {
          const dl = el("datalist", { id: `${id}-list` }, ...FORMATS.map((f) => el("option", { value: f })));
          input.setAttribute("list", dl.id);
          wrap.append(dl);
        }
        input.addEventListener("input", () => set(input.value || undefined, false));
        break;
      }
      case "prop": {
        // A prop is an ink or a value ("$accent", "#c33", 12) or an expression over the row.
        const shown = isExpr(cur) ? cur.expr : cur === undefined ? "" : String(cur);
        input = el("input", { type: "text", id, value: shown, placeholder: "an ink ($accent) or an expression (d.value > 30 ? \"$accent\" : \"$muted\")", spellcheck: "false", autocomplete: "off", class: "rp-mono" });
        input.addEventListener("input", () => {
          const s = input.value.trim();
          set(!s ? undefined : /^[$#]/.test(s) && !/[\s()?:]/.test(s) ? s : NUM.test(s) ? Number(s) : { expr: s }, false);
        });
        break;
      }
      case "json": {
        input = el("textarea", { id, rows: "3", spellcheck: "false", class: "rp-mono", text: cur === undefined ? "" : JSON.stringify(cur) });
        input.addEventListener("input", () => {
          const s = input.value.trim();
          try {
            const value = s ? JSON.parse(s) : undefined;
            input.removeAttribute("aria-invalid");
            set(value, false);
          } catch {
            input.setAttribute("aria-invalid", "true");
          }
        });
        break;
      }
    }
    wrap.append(input);
    if (hint) wrap.append(hint);
    return wrap;
  }

  // ---- the data ----
  function dataBox() {
    const table = tableNow();
    csvSource = table ? sourceOf(table) : null;
    const s = csvSource && doc.data[csvSource];
    const editable = !!s?.values;
    csvBox.disabled = !editable;
    csvBox.value = editable ? toCsv(s.values) : "";
    csvBox.removeAttribute("aria-invalid");
    csvReset.disabled = !editable;
    const kind = s ? Object.keys(s).find((k) => !["key", "types", "live", "id"].includes(k)) : null;
    csvNote.textContent = editable
      ? `The table “${csvSource}”, ${Object.values(Array.isArray(s.values) ? { r: s.values } : s.values)[0]?.length ?? 0} rows, keyed by ${(s.key ?? []).join(", ") || "row"}. Edit it, or paste your own (CSV, or straight from a spreadsheet).`
      : kind === "generate" ? "This example's rows are generated by the engine (data.generate): expressions, not a table to edit."
      : kind === "atlas" ? "This example reads a built-in atlas; its values table is the one to edit."
      : kind ? `This example's data is a ${kind} source, not an inline table.` : "This recipe reads no table.";
  }
  let csvTimer = 0;
  csvBox.addEventListener("input", () => {
    clearTimeout(csvTimer);
    csvTimer = setTimeout(applyCsv, 250);
  });
  function applyCsv() {
    const s = csvSource && doc.data[csvSource];
    if (!s?.values) return;
    const parsed = parseCsv(csvBox.value);
    if (!parsed.columns.length || !parsed.rows.length || parsed.columns.some((c) => !c)) {
      csvBox.setAttribute("aria-invalid", "true");
      csvNote.textContent = "A header row and at least one row of values, please.";
      return;
    }
    csvBox.removeAttribute("aria-invalid");
    const before = Array.isArray(s.values) ? [...new Set(s.values.flatMap((r) => Object.keys(r)))] : Object.keys(s.values);
    const cols = typedColumns(parsed);
    const names = Object.keys(cols);
    // Renamed columns: settings that named the old ones follow them, by position.
    const renamed = new Map();
    before.forEach((c, i) => { if (!names.includes(c) && names[i] && !before.includes(names[i])) renamed.set(c, names[i]); });
    if (renamed.size) rename(renamed);
    s.values = cols;
    if (s.key && !s.key.every((k) => names.includes(k))) s.key = s.key.map((k) => renamed.get(k) ?? k).filter((k) => names.includes(k));
    if (s.key && !s.key.length) s.key = [names[0]];
    if (s.types) s.types = Object.fromEntries(Object.entries(s.types).filter(([k]) => names.includes(k)));
    csvNote.textContent = `${parsed.rows.length} rows, ${names.length} columns${renamed.size ? ` — ${[...renamed].map(([a, b]) => `${a} → ${b}`).join(", ")}` : ""}.`;
    apply(false);
    controls();
  }
  /** Field settings anywhere in the document that name a renamed column. */
  function rename(map) {
    const walk = (v) => {
      if (Array.isArray(v)) return v.forEach(walk);
      if (!v || typeof v !== "object") return;
      if (v.kind === "use" && v.params) {
        const ps = spec(v.recipe);
        for (const [k, x] of Object.entries(v.params)) {
          if ((ps[k]?.type === "field" || (!ps[k] && INHERITED.has(k) && k !== "data")) && map.has(x)) v.params[k] = map.get(x);
        }
      }
      Object.values(v).forEach(walk);
    };
    walk(doc.scene);
  }
  csvReset.addEventListener("click", async () => {
    const fresh = await fetchDoc(name);
    if (csvSource && fresh.data?.[csvSource]) {
      doc.data[csvSource] = structuredClone(fresh.data[csvSource]);
      // The settings go back too: they may have followed renamed columns.
      doc.scene = structuredClone(fresh.scene);
      [target, parent] = find(doc, `@datars/std/${name}`);
      controls();
      dataBox();
      apply(true);
    }
  });

  // ---- out to the engine ----
  let codeTimer = 0;
  let applyTimer = 0;
  /** Send the document to the view (now, or after the reader pauses typing) and write its code. */
  function apply(now) {
    clearTimeout(applyTimer);
    if (!now) return void (applyTimer = setTimeout(() => apply(true), 280));
    if (view) {
      const p = view.setDocument(doc);
      p?.catch?.((e) => say([String(e?.message ?? e)]));
      notesLater();
    }
    clearTimeout(codeTimer);
    codeTimer = setTimeout(code, 60);
  }
  function code() {
    codeEl.textContent = toTs(doc, { header: `// ${name}: the reference figure's first state, as you've set it.\n` });
    highlight(codeEl);
  }
  /** The engine's messages, once they've settled: a map's atlas or a font arrives a moment after
   * the document, and "unknown table" until then isn't news. */
  let notesTimers = [];
  function notesLater() {
    notesTimers.forEach(clearTimeout);
    notesTimers = [700, 2500].map((ms) => setTimeout(() => say(worth(view?.status?.diagnostics)), ms));
  }
  function say(list) {
    notes.replaceChildren(...list.map((d) => el("li", { text: d.replace(/\n\s+at [\s\S]*$/, "") })));
    notes.hidden = !list.length;
  }

  // ---- choosing a recipe ----
  async function load(next, { push = true } = {}) {
    if (!std.recipes[`@datars/std/${next}`]) return;
    const ticket = ++loading;
    picker.value = next;
    const fresh = await fetchDoc(next);
    if (ticket !== loading) return;
    name = next;
    doc = structuredClone(fresh);
    [target, parent] = find(doc, `@datars/std/${name}`);
    if (!target) return; // every figure uses its recipe (the build checks)
    const r = std.recipes[target.recipe];
    about.replaceChildren(
      el("b", { text: `${name}()` }), ` ${firstSentence(r.doc)} `,
      el("a", { href: new URL(`docs/std/${name}/`, root).href, text: "Reference →" }),
    );
    for (const n of section.querySelectorAll(".rp-name")) n.textContent = name;
    if (push) history.replaceState(null, "", `#play=${name}`);
    controls();
    dataBox();
    apply(true);
  }

  function fillPicker() {
    picker.replaceChildren(...std.groups.map(([h, , list]) => el("optgroup", { label: h }, ...list.filter((n) => std.recipes[`@datars/std/${n}`]).map((n) => el("option", { value: n, text: n })))));
    order = [...picker.querySelectorAll("option")].map((o) => o.value);
  }
  picker.addEventListener("change", () => load(picker.value));
  const step = (d) => load(order[(order.indexOf(name) + d + order.length) % order.length]);
  prevBtn.addEventListener("click", () => step(-1));
  nextBtn.addEventListener("click", () => step(1));
  addEventListener("hashchange", () => { const w = want(); if (w && w !== name) load(w, { push: false }); });

  // The catalogue below is the visual index: a thumbnail opens its recipe here (a modified click
  // still follows the link to the reference).
  for (const a of document.querySelectorAll("#catalogue a.std-mini")) {
    a.addEventListener("click", (e) => {
      if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      const n = a.querySelector("code")?.textContent;
      if (!n || !std?.recipes[`@datars/std/${n}`]) return;
      e.preventDefault();
      load(n);
      section.scrollIntoView({ behavior: reduced.matches ? "auto" : "smooth", block: "start" });
    });
  }

  copyBtn.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(codeEl.textContent);
      copyBtn.textContent = "Copied";
    } catch {
      copyBtn.textContent = "Select and copy";
    }
    setTimeout(() => { copyBtn.textContent = "Copy"; }, 1600);
  });

  // The chart starts from the first document: the slot waits for it (`data-own-look`), so the view
  // asks for the engine with the recipe sandbox, and its first frame is the recipe.
  slot.addEventListener("chartmount", (e) => {
    const v = e.detail.view;
    if (doc) v.setDocument(doc);
    whenReady(slot).then((ready) => {
      view = ready;
      notesLater();
    });
  });

  (async () => {
    std = await stdReady;
    fillPicker();
    const first = want() && std.recipes[`@datars/std/${want()}`] ? want() : picker.dataset.start || "bar";
    await load(first, { push: false });
    own.dataset.ownLook = "ready";
    document.dispatchEvent(new Event("ownlookready"));
  })();
}

// ---- your data, every chart that fits it ---------------------------------------------------------

function yourData(section) {
  const slot = section.querySelector('.chart[data-chart="charts-fits"]');
  const box = section.querySelector("#fits-csv");
  const note = section.querySelector(".fits-note");
  const reset = section.querySelector(".fits-reset");
  const example = box.value;
  let timer = 0;
  const show = (text, bad = false) => {
    note.textContent = text;
    if (bad) box.setAttribute("aria-invalid", "true");
    else box.removeAttribute("aria-invalid");
  };
  async function provide() {
    const parsed = parseCsv(box.value);
    const cols = typedColumns(parsed);
    const names = Object.keys(cols);
    const label = names.find((n) => cols[n].some((x) => typeof x === "string"));
    const value = names.find((n) => n !== label && cols[n].every((x) => x === null || typeof x === "number"));
    if (!label || !value) return show("Two columns, please: names, then numbers.", true);
    const rows = cols[label].map((l, i) => [l, cols[value][i]]).filter(([l, v]) => l != null && v != null);
    if (rows.length < 2 || rows.length > 24) return show(`Between 2 and 24 rows, please (you have ${rows.length}).`, true);
    if (rows.some(([, v]) => v < 0)) return show("Parts of a whole can't be negative: all values 0 or more, please.", true);
    const seen = new Set();
    if (rows.some(([l]) => seen.size === seen.add(String(l)).size)) return show("Each name once, please: the names are the keys that marks morph by.", true);
    const view = await whenReady(slot);
    try {
      // The chart's own column names: the slot's sample says which the chart needs.
      view.provideData("visits", { channel: rows.map(([l]) => String(l)), visitors: rows.map(([, v]) => v) });
      show(box.value === example ? "The example. Edit it, or paste two columns of your own." : `${rows.length} rows of “${label}” by “${value}”, drawn by every recipe in turn.`);
    } catch (e) {
      show(String(e?.message ?? e), true);
    }
  }
  box.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(provide, 300);
  });
  reset.addEventListener("click", () => {
    box.value = example;
    provide();
  });
}
