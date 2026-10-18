// The charts page (site/pages/features/charts.html): the families of the standard library, each
// a published figure the site mounts like any chart, and the parts-of-a-whole figure's "your data":
// the reader's two columns go into the published chart with `provideData` (the light engine, no
// recipe code), validated first — a table that can't be drawn is never handed over, so the chart
// keeps what it shows.
//
// Also the document helpers other pages use, which touch no DOM (so they run in Node too): `toTs`
// writes a document as the doc.ts that makes it (the extensibility page's workbench shows it), and
// the CSV reader.

// ---- documents ----------------------------------------------------------------------------------

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

// ---- the page -----------------------------------------------------------------------------------

if (typeof document !== "undefined") {
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

// ---- your data, every chart that fits it ---------------------------------------------------------

function yourData(section) {
  const slot = document.querySelector('.chart[data-chart="charts-fits"]');
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
