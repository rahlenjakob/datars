// src/prop.ts
function e(src2) {
  return { expr: src2 };
}
function isExpr(v) {
  return typeof v === "object" && v !== null && typeof v.expr === "string";
}
function src(v) {
  if (v === void 0 || v === null) return "null";
  if (isExpr(v)) return `(${v.expr})`;
  if (typeof v === "function") return `(${lambdaBody(v)})`;
  if (typeof v === "string") return JSON.stringify(v);
  if (Array.isArray(v)) return `[${v.map(src).join(", ")}]`;
  return String(v);
}
function expr(strings, ...vals) {
  let out = strings[0];
  vals.forEach((v, i) => {
    if (isExpr(v)) out += `(${v.expr})`;
    else if (typeof v === "function") out += `(${lambdaBody(v)})`;
    else if (v !== null && typeof v === "object" && "__lit" in v) out += JSON.stringify(v.__lit);
    else out += String(v);
    out += strings[i + 1];
  });
  return { expr: out };
}
function lit(s) {
  return { __lit: s };
}
function field(name) {
  return { expr: /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(name) ? `d.${name}` : `d[${JSON.stringify(name)}]` };
}
function lambdaBody(f) {
  return f.toString().replace(/\*\*/g, "^^POW^^").replace(/\^\^POW\^\^/g, "**");
}
function norm(v) {
  if (v === void 0) return void 0;
  if (typeof v === "function") return { expr: lambdaBody(v) };
  if (Array.isArray(v)) return v.map(norm);
  return v;
}
function clean(o) {
  if (Array.isArray(o)) return o.map(clean);
  if (typeof o === "function") return norm(o);
  if (o === null || typeof o !== "object" || isExpr(o)) return o;
  const out = {};
  for (const [k, v] of Object.entries(o)) {
    if (v === void 0) continue;
    out[k] = clean(v);
  }
  return out;
}

// src/nodes.ts
function pick(signal2, options, labels) {
  return clean({ pick: signal2, options, labels });
}
function scrub(signal2, o = {}) {
  return clean({ scrub: signal2, axis: o.axis ?? "x", scale: o.scale, step: o.step || void 0, min: o.min, max: o.max });
}
function brush(signal2, axis = "x", scale2) {
  return clean({ brush: signal2, axis, scale: scale2 });
}
function brushed(signal2, value) {
  return { expr: `!${signal2}.active || (${value} >= ${signal2}.lo && ${value} <= ${signal2}.hi)` };
}
function node(kind, opts, body) {
  return clean({ kind, ...opts, ...body });
}
function group(opts = {}) {
  const { children, ...rest } = opts;
  return node("group", rest, { children: (children ?? []).filter(Boolean) });
}
function view(opts = {}) {
  const { children, camera, clip, ...rest } = opts;
  const cam = camera && "fit" in camera ? clean({ fit: camera.fit, padding: camera.padding, explore: camera.explore, max_zoom: camera.maxZoom, min_zoom: camera.minZoom }) : camera;
  return node("view", rest, { camera: cam, clip: clip === false ? false : void 0, children: (children ?? []).filter(Boolean) });
}
var geom = {
  rect: (g) => clean({ type: "rect", ...g }),
  circle: (g) => clean({ type: "circle", ...g }),
  ellipse: (g) => clean({ type: "ellipse", ...g }),
  arc: (g) => clean({ type: "arc", ...g }),
  segment: (g) => clean({ type: "segment", ...g }),
  polyline: (g) => clean({ type: "polyline", ...g }),
  area: (g) => clean({ type: "area", ...g }),
  path: (d) => clean({ type: "path", d }),
  symbol: (g) => clean({ type: "symbol", ...g }),
  /** A feature of a geo source by id — or, with no arguments inside a `tiles` layer template, the
   * current tile feature's own geometry. */
  feature: (source, id) => clean({ type: "feature", source, id })
};
function stroke(s) {
  if (!s) return void 0;
  const { nonScaling, ...rest } = s;
  return clean({ ...rest, non_scaling: nonScaling || void 0 });
}
function shape(g, opts = {}) {
  const { fill, stroke: st, markers, ...rest } = opts;
  return node("shape", rest, { geom: g, fill, stroke: stroke(st), markers });
}
function text(content, at, opts = {}) {
  const { style, halo, number, rotate, offset, ...rest } = opts;
  const st = style ? clean({ font: style.font, size: style.size, weight: style.weight, ink: style.ink, align: style.align, baseline: style.baseline, max_width: style.maxWidth, contain: style.contain }) : {};
  return node("text", rest, { text: content ?? "", at, style: st, halo, number, rotate, offset });
}
function repeat(from, template) {
  return clean({ kind: "repeat", from, template });
}
function instances(o) {
  const { from, proto, instanceKey, x, y, r, w, h, fill, opacity, stroke: st, screenSize, label, hit, reach, lod, ...rest } = o;
  const l = lod === true ? {} : lod ? clean({ points: lod.points, budget: lod.budget }) : void 0;
  return node("instances", rest, { from, proto: proto ?? "circle", instance_key: instanceKey, x, y, r, w, h, fill, instance_opacity: opacity, stroke: stroke(st), screen_size: screenSize || void 0, label, hit, reach, lod: l });
}
function tiles(opts) {
  const { source, layers, tileSize, ...rest } = opts;
  return node("tiles", rest, { source, layers: layers.map((l) => clean(l)), tile_size: tileSize });
}
function use(recipe2, params = {}, opts = {}) {
  return clean({ kind: "use", recipe: recipe2, params, ...opts });
}

// src/data.ts
function keys(k) {
  return k === void 0 ? void 0 : Array.isArray(k) ? k : [k];
}
var data = {
  /** Inline columns `{col: [...]}` or records `[{...}]`. */
  values: (v, o = {}) => clean({ values: v, key: keys(o.key), types: o.types, live: o.live }),
  csv: (text2, o = {}) => clean({ csv: text2, key: keys(o.key), types: o.types, live: o.live }),
  /** Fetched by the host (the engine never does IO). A GeoJSON file is a geo source: `id` names the
   * feature property holding each feature's id (default `id`, else the feature's own id). `rows`:
   * where a JSON API's response keeps its records (`"data"`, `"results.items"`). A remote URL is
   * fetched at publish time too: the bundle opens on that snapshot. */
  url: (url, o = {}) => clean({ url, key: keys(o.key), types: o.types, live: o.live, id: o.id, rows: o.rows }),
  /** Provided by the host app at runtime (a data slot): the chart ships without data and each
   * user's rows are handed in on the device. `sample` rows show until then (previews, tests, the
   * static fallback) and say which columns the host's data must have. */
  slot: (name, o = {}) => clean({ slot: name, key: keys(o.key), types: o.types, live: o.live, sample: o.sample }),
  /** Inline GeoJSON (FeatureCollection); `id` names the property holding each feature's id. */
  geojson: (fc, o = {}) => clean({ geojson: fc, id: o.id }),
  topojson: (topo, o = {}) => clean({ topojson: topo, id: o.id }),
  /** A built-in atlas: "countries" (ISO 3166-1 alpha-3 ids), … */
  atlas: (name) => ({ atlas: name }),
  /** A vector-tile archive (PMTiles) by URL, never fetched whole: the engine asks the host for the
   * byte ranges the visible views need. Drawn with `tiles(…)` nodes (or the std `basemap`).
   * `data.tiles.auto()`: an automatic basemap — `datars render/dev/publish` cut an archive to this
   * document's own cameras (every state and the flights between) from OpenStreetMap and Natural
   * Earth, fetched once into a local cache; street level anywhere on Earth, no archive to make. */
  tiles: Object.assign((url) => ({ tiles: url }), { auto: () => ({ tiles: "auto" }) }),
  /** A font file (TrueType/OpenType) by path or URL, or a Google Fonts family as
   * `"google:Noto Sans Hebrew"`: its families join every text's fallback chain, so labels in
   * other scripts (Hebrew, Arabic, CJK…) draw. Published bundles ship a subset of it. Theme fonts
   * are font tokens instead (`font.file`, `font.google`). */
  font: (url) => ({ font: url }),
  /** Rows the engine generates — synthetic data too big to write down (simulations, load tests,
   * procedural datasets) as a few expressions instead of millions of values. Each column is an
   * expression over the row, evaluated in order: `d.i` is the row number, `d.<name>` any column
   * before it, and `rand(key, stream)` / `randn(key, stream)` give seeded uniform / normal draws
   * (the same rows on every platform). `keep`: the columns the table ends up with (default all;
   * the others are working values). */
  generate: (rows, columns, o = {}) => clean({ generate: clean({ rows, columns: Object.entries(columns).map(([as, x]) => ({ as, expr: exprSource(x) })), keep: o.keep }), key: keys(o.key), types: o.types })
};
function exprSource(x) {
  if (isExpr(x)) return x.expr;
  if (typeof x === "function") return x.toString();
  if (typeof x === "string") return x;
  return JSON.stringify(x ?? null);
}
var op = {
  filter: (expr2) => clean({ op: "filter", expr: expr2 }),
  derive: (as, expr2) => clean({ op: "derive", as, expr: expr2 }),
  aggregate: (groupby, ops) => ({ op: "aggregate", groupby, ops: Object.entries(ops).map(([as, [fn, field2]]) => clean({ as, op: fn, field: field2 })) }),
  sort: (...by) => ({ op: "sort", by: by.map((b) => Array.isArray(b) ? b : [b, "asc"]) }),
  top: (n, by, other) => clean({ op: "top", n, by, other }),
  bin: (field2, as, o = {}) => clean({ op: "bin", field: field2, as, ...o }),
  /** A per-row value over the row's partition in `order` (`"-field"` descends). `fn`: `cumsum`,
   * `rank`, `dense_rank`, `lag`/`lead` (`k` rows), `rolling_mean`/`rolling_sum`/`rolling_std`
   * (population σ)/`rolling_min`/`rolling_max` (the last `k` values), `ema` (span `k`),
   * `cummax`/`cummin` (running extremes), `first`/`last` (the partition's), `share_of_total`,
   * `pct_change`. `min`: rolling windows and the EMA with fewer values than this give null
   * instead of a partial start (default 1). */
  window: (fn, field2, as, o = {}) => clean({ op: "window", fn, field: field2, as, ...o }),
  join: (withTable, on, kind = "left") => ({ op: "join", with: withTable, on: Array.isArray(on) ? on : [on], kind }),
  pivot: (key, value, index) => ({ op: "pivot", key, value, index }),
  unpivot: (columns, as) => ({ op: "unpivot", columns, as }),
  union: (other) => ({ op: "union", with: other }),
  /** These rows instead of the input's: inline columns (`{ id: [...], col: [...] }`) or records,
   * keyed by `key` — a recipe's own small lookup table (a tile map's grid layout) to join data onto. */
  values: (v, o = {}) => clean({ op: "values", values: v, key: keys(o.key) }),
  sample: (n, seed = 1) => ({ op: "sample", n, seed }),
  interpolate: (key, time, value, at) => clean({ op: "interpolate", key, time, value, at }),
  // Layout algorithms (datars-algo) — add columns to rows:
  /** `order`: which series sit at the baseline — input order (default), `reverse`, `ascending` or
   * `descending` by total, or `inside-out` (the biggest in the middle: a streamgraph's, with `wiggle`). */
  stack: (o) => clean({ op: "stack", ...o }),
  pie: (o) => clean({ op: "pie", ...o }),
  treemap: (o) => clean({ op: "treemap", ...o }),
  pack: (o) => clean({ op: "pack", ...o }),
  beeswarm: (o) => clean({ op: "beeswarm", ...o }),
  /** 1-D: push positions (an expression, e.g. `scale.y(d.v)`) at least `gap` apart, moving them as little as possible (labels at line ends). */
  spread: (o) => clean({ op: "spread", ...o }),
  /** 1-D intervals (expressions: a label's left and right edge, e.g. `scale.x(d.date)` and that
   * plus its measured width) packed into as few lanes as possible, `gap` apart; the lane number
   * (0, 1, …) is the column `as` (default `lane`). Past `max` lanes a row gets null (left out). */
  lanes: (o) => clean({ op: "lanes", ...o }),
  /** One row per unit of `value` (rounded), keyed under its parent row with a unit part; the unit
   * number is the column `as` (default `__unit`). */
  units: (o) => clean({ op: "units", ...o }),
  parliament: (o) => clean({ op: "parliament", ...o }),
  waffle: (o) => clean({ op: "waffle", ...o }),
  /** Adds `cx`, `cy` (the day's cell within its year's panel), `cell` and `panel` (the year, counted from the first year in the rows: stack panels by it). */
  calendar: (o) => clean({ op: "calendar", ...o }),
  sankeyNodes: (o) => clean({ op: "sankey-nodes", ...o }),
  sankeyLinks: (o) => clean({ op: "sankey-links", ...o }),
  waterfall: (o) => clean({ op: "waterfall", ...o }),
  /** `count` points spread inside each row's feature (its id in `key`, default the table's key): rows `(key, dot)` with `lon`, `lat`, keyed by region and dot index. */
  scatterIn: (o) => clean({ op: "scatter-in", ...o }),
  // Big data — millions of rows into bins, computed once per data (the table reads only its rows):
  /** Hexagonal bins of the rows at `x`/`y` (columns): a new table, one row per non-empty hexagon of
   * a lattice `columns` hexagons across `extent` ([x0, y0, x1, y1]; default the rows' own), regular
   * on screen when the extent is drawn `aspect` times as tall as it is wide. Columns: `hex` (the
   * key), `x`, `y` (centre), `count`, `value` (`fn` — count, sum, mean, min, max — of the column
   * `value`; the count without one), `hw`, `hh` (half width, and circumradius in y: corners at
   * (x, y ± hh) and (x ± hw, y ± hh / 2)), `col`, `row`. */
  hexbin: (o) => clean({ op: "hexbin", ...o }),
  /** A regular grid over `extent` (default the rows' own), `columns` × `rows` cells, the rows at
   * `x`/`y` counted into it in one pass: a new table, one row per non-empty cell (every cell with
   * `empty`) with `cell` (the key), `x0`, `x1`, `y0`, `y1`, `x`, `y` (centre), `count`, `value`
   * (`fn` of the column `value`, else the count), `col` and `row` (0 at the low end of y). */
  bin2d: (o) => clean({ op: "bin2d", ...o }),
  /** Density contours of the rows at `x`/`y`: a Gaussian density (`bandwidth` in cells of a
   * `columns` × `rows` grid over `extent`, default the rows' own widened by `pad`; `weight` a
   * column of row weights) cut at `levels` thresholds — where the region inside holds 1/(levels+1),
   * 2/(levels+1)… of the rows, exactly `shares` when given, or with `by: "density"` evenly spaced
   * up to the peak. A new table of ring vertices in order: `level` (0 the outermost), `ring`, `x`,
   * `y`, `share` (of the rows inside the level) and `density` (rows per unit² there) — join them
   * into one path per level with `op.paths`. */
  contours: (o) => clean({ op: "contours", ...o }),
  /** One row per group (`by`; none — one of every row): its rows' points (`x`, `y`, expressions,
   * usually `scale.x(d.t)`) joined in row order into SVG path data (`as`, default `path`) for
   * [`geom.path`] — a new subpath where the `ring` column changes and after a missing point, each
   * closed with `closed`. Keeps the group's first row, adds `end_x`, `end_y` (its last point),
   * keyed by `by`: thousands of series as one path each, contour levels with their holes. */
  paths: (o) => clean({ op: "paths", ...o }),
  // Graphs and hierarchies — nodes (keyed by `id`, default the table's key) and a `links` table of
  // `source` / `target` ids, or a parent-child table:
  /** A deterministic force-directed layout of the nodes (a fixed number of `iterations` from starts
   * seeded by each node's id, so a data change moves the others little): link springs of rest
   * length `distance`, many-body `charge` (negative repels), `gravity` toward the centre, and
   * collision of circles of `radius` (an expression per node) plus `collide` px — all inside
   * `width` × `height`. Heavier links (`weight`) pull harder. Adds `x`, `y` (or `as`) and
   * `degree` (the summed weight of each node's links). Computed once per data and box. */
  force: (o) => clean({ op: "force", ...o }),
  /** Clusters of densely linked nodes (Louvain modularity; `resolution` above 1 finds smaller ones):
   * adds `community` (0 = the largest), `degree` and `order` (a position that keeps each community
   * together, best connected first — sort by it for an adjacency matrix or an arc diagram). */
  communities: (o) => clean({ op: "communities", ...o }),
  /** Another table's columns (`values`, named `as`) on each row, from the row of `from` whose `key`
   * (default its key) equals this row's `field` — a link's endpoint positions from laid-out nodes. */
  lookup: (o) => clean({ op: "lookup", ...o }),
  /** A node-link tree of a parent-child table (`parent` holds each row's parent id; none: a root):
   * the tidy tree (Reingold–Tilford), or with `method: "cluster"` a dendrogram with every leaf on
   * the last level. Adds `x` (across, sibling order kept), `y` (depth: roots at 0, the deepest level
   * at `height`), `px`, `py` (the parent's position; null for roots), `depth`, `leaf` and `path`
   * (the names from the root down, joined by " / ": the `label` column's, else the ids). Lay out in
   * angle × radius (`width: 2π`) for a radial tree. */
  tree: (o) => clean({ op: "tree", ...o }),
  /** An adjacency partition of a parent-child table (icicles; sunbursts in angle × radius): a band
   * per depth, each node spanning a share of its parent ∝ its `value` (leaves' values summed up;
   * without `value`, leaves count 1). `sort`: siblings largest first. Adds `x0`, `y0`, `x1`, `y1`,
   * `depth`, `leaf`, `sum` (the subtree's value), `share` (of its root's), `branch` (the id of
   * its top-level ancestor, for colour) and `path` (as `op.tree`'s). */
  partition: (o) => clean({ op: "partition", ...o }),
  /** A table of a chord diagram's groups from a links table: `name`, `a0`, `a1` (radians clockwise
   * from 12 o'clock, `pad` apart), `value` (the flows touching it) and `index`, keyed by name. */
  chordGroups: (o) => clean({ op: "chord-groups", ...o }),
  /** Each link's ribbon: `sa0`, `sa1` (its span on the source group), `ta0`, `ta1` (on the target),
   * and with `r` its outline `path` around (`cx`, `cy`) for [`geom.path`]. */
  chordRibbons: (o) => clean({ op: "chord-ribbons", ...o }),
  /** A Gaussian kernel density of `field` per group (`groupby`): a new table with `steps` rows per
   * group (64) on one grid shared by every group — the field's extent, or `extent` — holding the
   * grid `value` and the `density` there (or `as`), keyed (group…, sample). Each group's density
   * integrates to 1. `bandwidth` (value units) defaults to Silverman's rule per group; `weight`
   * names a column of row weights. `trim`: each group's curve over its own values' extent (violins
   * end at their data); `extend`: the extent widened by this many bandwidths (tails that taper to
   * nothing, as ridgelines draw them). The outline of violins and ridgelines. */
  kde: (o) => clean({ op: "kde", ...o })
};
function table(from, ...ops) {
  return { from, ops };
}

// src/recipe.ts
var t = {
  table: (doc2) => ({ type: "table", doc: doc2 }),
  field: (doc2) => ({ type: "field", doc: doc2 }),
  number: (def, doc2) => ({ type: "number", default: def, doc: doc2 }),
  string: (def, doc2) => ({ type: "string", default: def, doc: doc2 }),
  bool: (def, doc2) => ({ type: "bool", default: def, doc: doc2 }),
  ink: (def, doc2) => ({ type: "ink", default: def, doc: doc2 }),
  prop: (doc2) => ({ type: "prop", doc: doc2 }),
  oneOf: (values, def, doc2) => ({ type: "enum", values, default: def, doc: doc2 }),
  children: (doc2) => ({ type: "children", default: [], doc: doc2 }),
  json: (doc2) => ({ type: "json", doc: doc2 })
};
function scale(name) {
  const f = ((v) => e(`scale.${name}(${src(v)})`));
  Object.defineProperty(f, "name", { value: name });
  f.bandwidth = () => e(`scale.${name}.bandwidth()`);
  f.step = () => e(`scale.${name}.step()`);
  f.ink = (v) => e(`scale.${name}.ink(${src(v)})`);
  f.min = () => e(`scale.${name}.min()`);
  f.max = () => e(`scale.${name}.max()`);
  return f;
}
function fnv1a(s) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}
var ExpandCx = class {
  constructor(raw, prefix) {
    this.prefix = prefix;
    this.tables = {};
    this.n = 0;
    this.size = raw.size ?? [800, 480];
    this.sizeClass = raw.sizeClass ?? "wide";
    this.locale = raw.locale ?? "en";
  }
  ink(token) {
    return token.startsWith("$") || token.startsWith("#") ? token : `$${token}`;
  }
  token(name) {
    return e(`token(${JSON.stringify(name)})`);
  }
  measure(text2, style = {}) {
    if (typeof host === "function") return host("measure", { text: text2, ...style });
    return { w: text2.length * (style.size ?? 11) * 0.55, h: (style.size ?? 11) * 1.2 };
  }
  field(name) {
    return field(name);
  }
  scale(name) {
    return scale(name);
  }
  table(name, from, ...ops) {
    const spec = clean({ from, ops });
    const full = `${this.prefix}:${name}:${fnv1a(JSON.stringify(spec))}`;
    this.tables[full] = spec;
    return full;
  }
  uid(prefix) {
    this.n += 1;
    return `${prefix}-${this.n}`;
  }
};
function withDefaults(def, params) {
  const out = { ...params };
  for (const [k, spec] of Object.entries(def.params)) {
    if (out[k] === void 0 && spec.default !== void 0) out[k] = spec.default;
  }
  return out;
}
var INHERITED = /* @__PURE__ */ new Set(["data", "x", "y", "color", "xType", "yType", "clip", "series"]);
function distance(a, b) {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i, ...Array(b.length).fill(0)]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++)
    for (let j = 1; j <= b.length; j++) {
      d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
      if (i > 1 && j > 1 && a[i - 1] === b[j - 2] && a[i - 2] === b[j - 1]) d[i][j] = Math.min(d[i][j], d[i - 2][j - 2] + 1);
    }
  return d[a.length][b.length];
}
function unknownParams(def, params) {
  const known = Object.keys(def.params);
  return Object.keys(params).filter((k) => !known.includes(k) && !INHERITED.has(k) && params[k] !== void 0).map((k) => {
    const near = known.map((n) => [n, distance(k.toLowerCase(), n.toLowerCase())]).sort((x, y) => x[1] - y[1])[0];
    return near && near[1] <= Math.max(1, Math.floor(k.length / 3)) ? `${k} (did you mean \`${near[0]}\`?)` : k;
  });
}
function recipe(def) {
  const f = ((params = {}, opts = {}) => use(def.id, params, opts));
  f.id = def.id;
  f.def = def;
  f.__expand = (params, rawCx) => {
    const cx = new ExpandCx(rawCx ?? {}, def.id.split("/").pop() ?? "recipe");
    const full = withDefaults(def, params ?? {});
    const template = clean(def.expand(full, cx));
    if (!template.prov) template.prov = def.id;
    const motion2 = typeof def.motion === "function" ? def.motion(full) : def.motion;
    if (motion2 && !template.motion) template.motion = motion2;
    const unknown = unknownParams(def, params ?? {});
    return unknown.length ? { template, tables: cx.tables, unknown } : { template, tables: cx.tables };
  };
  f.describe = () => ({
    id: def.id,
    doc: def.doc,
    params: Object.fromEntries(Object.entries(def.params).map(([k, s]) => [k, clean({ ...s })])),
    tokens: def.tokens,
    examples: def.examples
  });
  return f;
}

// src/program.ts
function step(name, o = {}) {
  const narration = o.title || o.text || o.anchor ? clean({ title: o.title, text: o.text, anchor: o.anchor }) : void 0;
  return clean({ name, set: o.set, hold: o.hold, narration });
}
function story(o) {
  const edges = o.loop && o.steps.length > 1 ? [{ from: o.steps[o.steps.length - 1].name, on: "next", to: o.steps[0].name }] : void 0;
  return clean({ preset: "story", states: o.steps, edges, drivers: o.drivers ?? ["steps", "keys"], chapters: o.chapters });
}
function scrolly(o) {
  return clean({ preset: "story", states: o.steps, drivers: [{ scroll: "scrub" }, "keys"], chapters: o.chapters });
}
function autoplay(o) {
  return story({ steps: o.steps, loop: o.loop, drivers: ["autoplay", "steps", "keys"] });
}
function chapter(param, steps) {
  return { param, program: { states: steps } };
}
function interactive() {
  return { preset: "interactive", states: [{ name: "main" }] };
}
function film(steps) {
  return { preset: "film", states: steps, drivers: ["autoplay"] };
}
function loop(steps) {
  return story({ steps, loop: true, drivers: ["autoplay"] });
}
function dashboard() {
  return { preset: "dashboard", states: [{ name: "main" }] };
}

// src/motion.ts
function keyPath(p) {
  return p.split("/").filter((s) => s.length > 0).map((s) => [s]);
}
function toIr(r) {
  const { select, ...rest } = r;
  const out = { ...rest };
  if (select) {
    const { key, ...sel } = select;
    out.select = key === void 0 ? sel : { ...sel, key_prefix: keyPath(key) };
  }
  return clean(out);
}
function motion(...rules) {
  return { rules: rules.map(toIr) };
}
var choreo = {
  together: () => ({ type: "together" }),
  stagger: (order = "data", spread = 0.4) => ({ type: "stagger", order, spread }),
  phased: (exit = 0.3, update = 0.5, enter = 0.2) => ({ type: "phased", exit, update, enter }),
  wave: (spread = 0.5, angle = 0) => ({ type: "wave", spread, angle }),
  ripple: (spread = 0.5, origin) => ({ type: "ripple", spread, origin })
};
var route = {
  straight: () => ({ type: "straight" }),
  arc: (height = 0.45) => ({ type: "arc", height }),
  elbow: () => ({ type: "elbow" }),
  spiral: (turns = 1) => ({ type: "spiral", turns }),
  explode: () => ({ type: "explode" }),
  hop: (height = 20) => ({ type: "hop", height }),
  drift: (seed = 1, amount = 30) => ({ type: "drift", seed, amount }),
  drop: (bounce = 0.3) => ({ type: "drop", bounce })
};
var ghost = {
  fade: () => ({ opacity: 0 }),
  grow: (origin = "bottom") => ({ scale: 0, origin }),
  fromParent: () => ({ from: "parent" }),
  slide: (dx, dy) => ({ opacity: 0, dx, dy })
};

// src/theme.ts
function theme(def) {
  return def;
}
var font = {
  /** A face from a file next to the document (or an `https://` URL). */
  file(family, src2, opts = {}) {
    return { family, weight: opts.weight ?? 400, ...opts.italic ? { italic: true } : {}, src: src2 };
  },
  /** A Google Fonts family (downloaded by the build step, shipped in the bundle). `fallback`:
   * families to try after it for characters it lacks. */
  google(family, opts = {}) {
    return { google: family, weight: opts.weight ?? 400, ...opts.italic ? { italic: true } : {}, ...opts.fallback?.length ? { family: opts.fallback } : {} };
  }
};

// src/doc.ts
var FORMAT = 1;
function doc(d) {
  const size = Array.isArray(d.size) ? { width: d.size[0], height: d.size[1] } : d.size ?? { width: 800, height: 480 };
  let theme2 = void 0;
  if (typeof d.theme === "string") theme2 = { use: d.theme };
  else if (d.theme && "name" in d.theme) theme2 = { use: d.theme.name, themes: [d.theme] };
  else if (d.theme) theme2 = d.theme;
  return clean({
    datars: FORMAT,
    id: d.id,
    title: d.title,
    description: d.description,
    size,
    theme: theme2,
    locale: d.locale ?? "en",
    packages: d.packages,
    data: d.data,
    tables: d.tables,
    signals: d.signals,
    keys: d.keys,
    scene: d.scene,
    motion: d.motion,
    program: d.program
  });
}
var signal = {
  num: (def = 0, control2) => clean({ type: "num", default: def, control: control2 }),
  str: (def = "", control2) => clean({ type: "str", default: def, control: control2 }),
  bool: (def = false) => ({ type: "bool", default: def }),
  keyset: (def = []) => ({ type: "keyset", default: def }),
  key: (def = null) => ({ type: "key", default: def }),
  range: () => ({ type: "range", default: null }),
  /** A clock: counts up from `def` at `rate` units per second while a settled scene reads it (a
   * spinning globe: `projection center: [e("spin"), 15]`); paused during transitions, off screen
   * and for reduced motion. Renders and goldens see `def`. */
  clock: (rate, def = 0) => ({ type: "num", default: def, clock: rate })
};
var control = {
  slider: (min, max, step2 = 1, label) => clean({ type: "slider", min, max, step: step2, label }),
  toggle: (label) => clean({ type: "toggle", label }),
  select: (options, label) => clean({ type: "select", options, label })
};
export {
  FORMAT,
  autoplay,
  brush,
  brushed,
  chapter,
  choreo,
  clean,
  control,
  dashboard,
  data,
  doc,
  e,
  expr,
  field,
  film,
  font,
  geom,
  ghost,
  group,
  instances,
  interactive,
  isExpr,
  lit,
  loop,
  motion,
  norm,
  op,
  pick,
  recipe,
  repeat,
  route,
  scale,
  scrolly,
  scrub,
  shape,
  signal,
  src,
  step,
  story,
  t,
  table,
  text,
  theme,
  tiles,
  use,
  view
};
