// Data sources and derived tables.

import { clean, isExpr, Prop } from "./prop.js";

export interface SourceOpts {
  key?: string | string[];
  /** Column types, applied to what arrives: APIs send numbers and dates as strings. */
  types?: Record<string, "num" | "str" | "bool" | "date">;
  /** Refreshed on the reader's device every `every` seconds (and at once when the chart opens on a
   * snapshot taken at publish time). */
  live?: { every?: number; mode?: "snapshot" | "upsert" | "append"; window?: number };
}

function keys(k?: string | string[]) {
  return k === undefined ? undefined : Array.isArray(k) ? k : [k];
}

export const data = {
  /** Inline columns `{col: [...]}` or records `[{...}]`. */
  values: (v: Record<string, unknown[]> | Record<string, unknown>[], o: SourceOpts = {}) => clean({ values: v, key: keys(o.key), types: o.types, live: o.live }),
  csv: (text: string, o: SourceOpts = {}) => clean({ csv: text, key: keys(o.key), types: o.types, live: o.live }),
  /** Fetched by the host (the engine never does IO). A GeoJSON file is a geo source: `id` names the
   * feature property holding each feature's id (default `id`, else the feature's own id). `rows`:
   * where a JSON API's response keeps its records (`"data"`, `"results.items"`). A remote URL is
   * fetched at publish time too: the bundle opens on that snapshot. */
  url: (url: string, o: SourceOpts & { id?: string; rows?: string } = {}) => clean({ url, key: keys(o.key), types: o.types, live: o.live, id: o.id, rows: o.rows }),
  /** Provided by the host app at runtime (a data slot): the chart ships without data and each
   * user's rows are handed in on the device. `sample` rows show until then (previews, tests, the
   * static fallback) and say which columns the host's data must have. */
  slot: (name: string, o: SourceOpts & { sample?: Record<string, unknown[]> | Record<string, unknown>[] } = {}) => clean({ slot: name, key: keys(o.key), types: o.types, live: o.live, sample: o.sample }),
  /** Inline GeoJSON (FeatureCollection); `id` names the property holding each feature's id. */
  geojson: (fc: unknown, o: { id?: string } = {}) => clean({ geojson: fc, id: o.id }),
  topojson: (topo: unknown, o: { id?: string } = {}) => clean({ topojson: topo, id: o.id }),
  /** A built-in atlas: "countries" (ISO 3166-1 alpha-3 ids), … */
  atlas: (name: string) => ({ atlas: name }),
  /** A vector-tile archive (PMTiles) by URL, never fetched whole: the engine asks the host for the
   * byte ranges the visible views need. Drawn with `tiles(…)` nodes (or the std `basemap`).
   * `data.tiles.auto()`: an automatic basemap — `datars render/dev/publish` cut an archive to this
   * document's own cameras (every state and the flights between) from OpenStreetMap and Natural
   * Earth, fetched once into a local cache; street level anywhere on Earth, no archive to make. */
  tiles: Object.assign((url: string) => ({ tiles: url }), { auto: () => ({ tiles: "auto" }) }),
  /** A font file (TrueType/OpenType) by path or URL, or a Google Fonts family as
   * `"google:Noto Sans Hebrew"`: its families join every text's fallback chain, so labels in
   * other scripts (Hebrew, Arabic, CJK…) draw. Published bundles ship a subset of it. Theme fonts
   * are font tokens instead (`font.file`, `font.google`). */
  font: (url: string) => ({ font: url }),
  /** Rows the engine generates — synthetic data too big to write down (simulations, load tests,
   * procedural datasets) as a few expressions instead of millions of values. Each column is an
   * expression over the row, evaluated in order: `d.i` is the row number, `d.<name>` any column
   * before it, and `rand(key, stream)` / `randn(key, stream)` give seeded uniform / normal draws
   * (the same rows on every platform). `keep`: the columns the table ends up with (default all;
   * the others are working values). */
  generate: (rows: number, columns: Record<string, Prop>, o: SourceOpts & { keep?: string[] } = {}) =>
    clean({ generate: clean({ rows, columns: Object.entries(columns).map(([as, x]) => ({ as, expr: exprSource(x) })), keep: o.keep }), key: keys(o.key), types: o.types }),
};

/** An expression's source text (a column expression is always an expression: a plain string is
 * its source). */
function exprSource(x: Prop): string {
  if (isExpr(x)) return x.expr;
  if (typeof x === "function") return x.toString();
  if (typeof x === "string") return x;
  return JSON.stringify(x ?? null);
}

export type Op = Record<string, unknown> & { op: string };

/** Table operations (interpreted by the engine; heavy ones run as Rust algorithms). */
export const op = {
  filter: (expr: Prop): Op => clean({ op: "filter", expr }) as Op,
  derive: (as: string, expr: Prop): Op => clean({ op: "derive", as, expr }) as Op,
  aggregate: (groupby: string[], ops: Record<string, [string, string?]>): Op =>
    ({ op: "aggregate", groupby, ops: Object.entries(ops).map(([as, [fn, field]]) => clean({ as, op: fn, field })) }),
  sort: (...by: (string | [string, "asc" | "desc"])[]): Op => ({ op: "sort", by: by.map((b) => (Array.isArray(b) ? b : [b, "asc"])) }),
  top: (n: number, by: string, other?: string): Op => clean({ op: "top", n, by, other }) as Op,
  bin: (field: string, as: string, o: { step?: number; count?: number; unit?: string; part?: string } = {}): Op => clean({ op: "bin", field, as, ...o }) as Op,
  /** A per-row value over the row's partition in `order` (`"-field"` descends). `fn`: `cumsum`,
   * `rank`, `dense_rank`, `lag`/`lead` (`k` rows), `rolling_mean`/`rolling_sum`/`rolling_std`
   * (population σ)/`rolling_min`/`rolling_max` (the last `k` values), `ema` (span `k`),
   * `cummax`/`cummin` (running extremes), `first`/`last` (the partition's), `share_of_total`,
   * `pct_change`. `min`: rolling windows and the EMA with fewer values than this give null
   * instead of a partial start (default 1). */
  window: (fn: string, field: string, as: string, o: { partition?: string[]; order?: string; k?: number; min?: number } = {}): Op => clean({ op: "window", fn, field, as, ...o }) as Op,
  join: (withTable: string, on: string | string[], kind: "inner" | "left" = "left"): Op => ({ op: "join", with: withTable, on: Array.isArray(on) ? on : [on], kind }),
  pivot: (key: string, value: string, index: string[]): Op => ({ op: "pivot", key, value, index }),
  unpivot: (columns: string[], as: [string, string]): Op => ({ op: "unpivot", columns, as }),
  union: (other: string): Op => ({ op: "union", with: other }),
  /** These rows instead of the input's: inline columns (`{ id: [...], col: [...] }`) or records,
   * keyed by `key` — a recipe's own small lookup table (a tile map's grid layout) to join data onto. */
  values: (v: Record<string, unknown[]> | Record<string, unknown>[], o: { key?: string | string[] } = {}): Op => clean({ op: "values", values: v, key: keys(o.key) }) as Op,
  sample: (n: number, seed = 1): Op => ({ op: "sample", n, seed }),
  interpolate: (key: string, time: string, value: string, at: Prop): Op => clean({ op: "interpolate", key, time, value, at }) as Op,
  // Layout algorithms (datars-algo) — add columns to rows:
  /** `order`: which series sit at the baseline — input order (default), `reverse`, `ascending` or
   * `descending` by total, or `inside-out` (the biggest in the middle: a streamgraph's, with `wiggle`). */
  stack: (o: { x: string; series: string; value: string; offset?: "zero" | "expand" | "silhouette" | "wiggle"; order?: "input" | "reverse" | "ascending" | "descending" | "inside-out"; as?: [string, string] }): Op => clean({ op: "stack", ...o }) as Op,
  pie: (o: { value: string; sort?: "none" | "asc" | "desc"; pad?: number; start?: number; end?: number; as?: [string, string] }): Op => clean({ op: "pie", ...o }) as Op,
  treemap: (o: { value: string; width: Prop; height: Prop; as?: [string, string, string, string] }): Op => clean({ op: "treemap", ...o }) as Op,
  pack: (o: { value: string; width: Prop; height: Prop; padding?: number }): Op => clean({ op: "pack", ...o }) as Op,
  beeswarm: (o: { position: Prop; radius: Prop; as?: string }): Op => clean({ op: "beeswarm", ...o }) as Op,
  /** 1-D: push positions (an expression, e.g. `scale.y(d.v)`) at least `gap` apart, moving them as little as possible (labels at line ends). */
  spread: (o: { position: Prop; gap?: Prop; min?: Prop; max?: Prop; as?: string }): Op => clean({ op: "spread", ...o }) as Op,
  /** 1-D intervals (expressions: a label's left and right edge, e.g. `scale.x(d.date)` and that
   * plus its measured width) packed into as few lanes as possible, `gap` apart; the lane number
   * (0, 1, …) is the column `as` (default `lane`). Past `max` lanes a row gets null (left out). */
  lanes: (o: { start: Prop; end: Prop; gap?: Prop; max?: Prop; as?: string }): Op => clean({ op: "lanes", ...o }) as Op,
  /** One row per unit of `value` (rounded), keyed under its parent row with a unit part; the unit
   * number is the column `as` (default `__unit`). */
  units: (o: { value: string; as?: string }): Op => clean({ op: "units", ...o }) as Op,
  parliament: (o: { cx: Prop; cy: Prop; r0: Prop; r1: Prop; rows?: number }): Op => clean({ op: "parliament", ...o }) as Op,
  waffle: (o: { columns?: number; rows?: number; width: Prop; height: Prop; gap?: number }): Op => clean({ op: "waffle", ...o }) as Op,
  /** Adds `cx`, `cy` (the day's cell within its year's panel), `cell` and `panel` (the year, counted from the first year in the rows: stack panels by it). */
  calendar: (o: { date: string; cell: Prop; weekStart?: "monday" | "sunday" }): Op => clean({ op: "calendar", ...o }) as Op,
  sankeyNodes: (o: { links: string; source: string; target: string; value: string; width: Prop; height: Prop; nodeWidth?: number; nodePadding?: number }): Op => clean({ op: "sankey-nodes", ...o }) as Op,
  sankeyLinks: (o: { links: string; source: string; target: string; value: string; width: Prop; height: Prop; nodeWidth?: number; nodePadding?: number }): Op => clean({ op: "sankey-links", ...o }) as Op,
  waterfall: (o: { value: string; total?: string }): Op => clean({ op: "waterfall", ...o }) as Op,
  /** `count` points spread inside each row's feature (its id in `key`, default the table's key): rows `(key, dot)` with `lon`, `lat`, keyed by region and dot index. */
  scatterIn: (o: { geo: string; count: Prop; seed?: number; key?: string }): Op => clean({ op: "scatter-in", ...o }) as Op,
  // Big data — millions of rows into bins, computed once per data (the table reads only its rows):
  /** Hexagonal bins of the rows at `x`/`y` (columns): a new table, one row per non-empty hexagon of
   * a lattice `columns` hexagons across `extent` ([x0, y0, x1, y1]; default the rows' own), regular
   * on screen when the extent is drawn `aspect` times as tall as it is wide. Columns: `hex` (the
   * key), `x`, `y` (centre), `count`, `value` (`fn` — count, sum, mean, min, max — of the column
   * `value`; the count without one), `hw`, `hh` (half width, and circumradius in y: corners at
   * (x, y ± hh) and (x ± hw, y ± hh / 2)), `col`, `row`. */
  hexbin: (o: { x: string; y: string; columns?: Prop; aspect?: Prop; extent?: [Prop, Prop, Prop, Prop]; value?: string; fn?: "count" | "sum" | "mean" | "min" | "max" }): Op => clean({ op: "hexbin", ...o }) as Op,
  /** A regular grid over `extent` (default the rows' own), `columns` × `rows` cells, the rows at
   * `x`/`y` counted into it in one pass: a new table, one row per non-empty cell (every cell with
   * `empty`) with `cell` (the key), `x0`, `x1`, `y0`, `y1`, `x`, `y` (centre), `count`, `value`
   * (`fn` of the column `value`, else the count), `col` and `row` (0 at the low end of y). */
  bin2d: (o: { x: string; y: string; columns?: Prop; rows?: Prop; extent?: [Prop, Prop, Prop, Prop]; value?: string; fn?: "count" | "sum" | "mean" | "min" | "max"; empty?: boolean }): Op => clean({ op: "bin2d", ...o }) as Op,
  /** Density contours of the rows at `x`/`y`: a Gaussian density (`bandwidth` in cells of a
   * `columns` × `rows` grid over `extent`, default the rows' own widened by `pad`; `weight` a
   * column of row weights) cut at `levels` thresholds — where the region inside holds 1/(levels+1),
   * 2/(levels+1)… of the rows, exactly `shares` when given, or with `by: "density"` evenly spaced
   * up to the peak. A new table of ring vertices in order: `level` (0 the outermost), `ring`, `x`,
   * `y`, `share` (of the rows inside the level) and `density` (rows per unit² there) — join them
   * into one path per level with `op.paths`. */
  contours: (o: { x: string; y: string; columns?: Prop; rows?: Prop; extent?: [Prop, Prop, Prop, Prop]; pad?: Prop; bandwidth?: Prop; levels?: Prop; shares?: number[]; by?: "share" | "density"; weight?: string }): Op => clean({ op: "contours", ...o }) as Op,
  /** One row per group (`by`; none — one of every row): its rows' points (`x`, `y`, expressions,
   * usually `scale.x(d.t)`) joined in row order into SVG path data (`as`, default `path`) for
   * [`geom.path`] — a new subpath where the `ring` column changes and after a missing point, each
   * closed with `closed`. Keeps the group's first row, adds `end_x`, `end_y` (its last point),
   * keyed by `by`: thousands of series as one path each, contour levels with their holes. */
  paths: (o: { x: Prop; y: Prop; by?: string | string[]; ring?: string; closed?: boolean; as?: string }): Op => clean({ op: "paths", ...o }) as Op,
  // Graphs and hierarchies — nodes (keyed by `id`, default the table's key) and a `links` table of
  // `source` / `target` ids, or a parent-child table:
  /** A deterministic force-directed layout of the nodes (a fixed number of `iterations` from starts
   * seeded by each node's id, so a data change moves the others little): link springs of rest
   * length `distance`, many-body `charge` (negative repels), `gravity` toward the centre, and
   * collision of circles of `radius` (an expression per node) plus `collide` px — all inside
   * `width` × `height`. Heavier links (`weight`) pull harder. Adds `x`, `y` (or `as`) and
   * `degree` (the summed weight of each node's links). Computed once per data and box. */
  force: (o: { links: string; source?: string; target?: string; id?: string; weight?: string; radius?: Prop; width: Prop; height: Prop; distance?: Prop; charge?: Prop; gravity?: Prop; collide?: Prop; iterations?: number; seed?: number; as?: [string, string] }): Op => clean({ op: "force", ...o }) as Op,
  /** Clusters of densely linked nodes (Louvain modularity; `resolution` above 1 finds smaller ones):
   * adds `community` (0 = the largest), `degree` and `order` (a position that keeps each community
   * together, best connected first — sort by it for an adjacency matrix or an arc diagram). */
  communities: (o: { links: string; source?: string; target?: string; id?: string; weight?: string; resolution?: Prop }): Op => clean({ op: "communities", ...o }) as Op,
  /** Another table's columns (`values`, named `as`) on each row, from the row of `from` whose `key`
   * (default its key) equals this row's `field` — a link's endpoint positions from laid-out nodes. */
  lookup: (o: { from: string; key?: string; field: string; values: string[]; as?: string[] }): Op => clean({ op: "lookup", ...o }) as Op,
  /** A node-link tree of a parent-child table (`parent` holds each row's parent id; none: a root):
   * the tidy tree (Reingold–Tilford), or with `method: "cluster"` a dendrogram with every leaf on
   * the last level. Adds `x` (across, sibling order kept), `y` (depth: roots at 0, the deepest level
   * at `height`), `px`, `py` (the parent's position; null for roots), `depth`, `leaf` and `path`
   * (the names from the root down, joined by " / ": the `label` column's, else the ids). Lay out in
   * angle × radius (`width: 2π`) for a radial tree. */
  tree: (o: { parent: string; id?: string; label?: string; method?: "tidy" | "cluster"; width: Prop; height: Prop; separation?: Prop }): Op => clean({ op: "tree", ...o }) as Op,
  /** An adjacency partition of a parent-child table (icicles; sunbursts in angle × radius): a band
   * per depth, each node spanning a share of its parent ∝ its `value` (leaves' values summed up;
   * without `value`, leaves count 1). `sort`: siblings largest first. Adds `x0`, `y0`, `x1`, `y1`,
   * `depth`, `leaf`, `sum` (the subtree's value), `share` (of its root's), `branch` (the id of
   * its top-level ancestor, for colour) and `path` (as `op.tree`'s). */
  partition: (o: { parent: string; id?: string; label?: string; value?: string; width: Prop; height: Prop; padding?: Prop; sort?: boolean }): Op => clean({ op: "partition", ...o }) as Op,
  /** A table of a chord diagram's groups from a links table: `name`, `a0`, `a1` (radians clockwise
   * from 12 o'clock, `pad` apart), `value` (the flows touching it) and `index`, keyed by name. */
  chordGroups: (o: { source: string; target: string; value?: string; pad?: Prop }): Op => clean({ op: "chord-groups", ...o }) as Op,
  /** Each link's ribbon: `sa0`, `sa1` (its span on the source group), `ta0`, `ta1` (on the target),
   * and with `r` its outline `path` around (`cx`, `cy`) for [`geom.path`]. */
  chordRibbons: (o: { source: string; target: string; value?: string; pad?: Prop; cx?: Prop; cy?: Prop; r?: Prop }): Op => clean({ op: "chord-ribbons", ...o }) as Op,
  /** A Gaussian kernel density of `field` per group (`groupby`): a new table with `steps` rows per
   * group (64) on one grid shared by every group — the field's extent, or `extent` — holding the
   * grid `value` and the `density` there (or `as`), keyed (group…, sample). Each group's density
   * integrates to 1. `bandwidth` (value units) defaults to Silverman's rule per group; `weight`
   * names a column of row weights. `trim`: each group's curve over its own values' extent (violins
   * end at their data); `extend`: the extent widened by this many bandwidths (tails that taper to
   * nothing, as ridgelines draw them). The outline of violins and ridgelines. */
  kde: (o: { field: string; groupby?: string[]; bandwidth?: Prop; steps?: number; extent?: [Prop, Prop]; trim?: boolean; extend?: Prop; weight?: string; as?: [string, string] }): Op => clean({ op: "kde", ...o }) as Op,
};

export function table(from: string, ...ops: Op[]) {
  return { from, ops };
}
