// Scene template builders. Every builder returns plain IR JSON (see crates/datars-ir).

import { clean, Prop } from "./prop.js";

export type Json = null | boolean | number | string | Json[] | { [k: string]: Json | undefined };
export type Template = Record<string, unknown> & { kind: string };

export interface Action {
  set?: string; toggle?: string; value?: Prop; event?: string; chapter?: string; key?: Prop;
  /** A choice among `options` for a host to offer with its own picker (see `pick()`). */
  pick?: string; options?: Json[]; labels?: Prop[];
  /** Range selection by dragging (see `brush()`). */
  brush?: string; axis?: "x" | "y"; scale?: string;
  /** Set a signal to the value under the pointer (see `scrub()`), kept within `min`/`max`. */
  scrub?: string; step?: number; min?: Prop; max?: Prop;
}

/** One value for `signal` among `options`, each said by the matching `labels` entry (else by
 * itself): what a host offers with the platform's own picker — a native select on touch screens —
 * in place of the chart's drawn list. Bind it as `on: { pick: pick(…) }` beside the `activate`
 * that opens the drawn list (std's `select` does). */
export function pick(signal: string, options: Json[], labels?: Prop[]): Action {
  return clean({ pick: signal, options, labels });
}

/** Press or drag to set `signal` to the data value under the pointer, through the scale of the
 * axis (or `scale`), snapped to `step` and kept within `min`/`max` (numbers or expressions — a
 * range's low thumb: `max: e("hi")`) — sliders and time scrubbers. Bind it to `drag`. */
export function scrub(signal: string, o: { axis?: "x" | "y"; scale?: string; step?: number; min?: Prop; max?: Prop } = {}): Action {
  return clean({ scrub: signal, axis: o.axis ?? "x", scale: o.scale, step: o.step || undefined, min: o.min, max: o.max });
}

/** Drag across the node to select a range along a scale. Continuous scales write
 * `<signal>.lo` / `<signal>.hi` (data units), band scales the keyset `<signal>`; both set
 * `<signal>.active`, and a click clears it. Filter with `brushed(signal, "d.x")`. */
export function brush(signal: string, axis: "x" | "y" = "x", scale?: string): Action {
  return clean({ brush: signal, axis, scale });
}

/** An expression that keeps rows inside the brush (or all rows while nothing is brushed). */
export function brushed(signal: string, value: string): { expr: string } {
  return { expr: `!${signal}.active || (${value} >= ${signal}.lo && ${value} <= ${signal}.hi)` };
}

export interface NodeOpts {
  key?: Prop;
  id?: string;
  when?: Prop;
  opacity?: Prop;
  transform?: { translate?: [Prop, Prop]; rotate?: Prop; scale?: Prop };
  /** Draw order among siblings (higher on top). At 1000 and above the node floats: drawn and
   * picked above the whole scene, outside its ancestors' clips — a menu's list over the chart
   * below it, a popover. */
  z?: number;
  clip?: "box" | [Prop, Prop, Prop, Prop];
  trim?: [Prop, Prop];
  isolate?: boolean;
  /** Pinned: the origin follows cameras and scales above, the content keeps screen size (map callouts).
   * A text's origin is its `at` (a place name stays at its map point). */
  pin?: boolean;
  /** `link`: a URL this element links to (hosts make it clickable; vector exports keep it). */
  semantics?: { role: Role; label?: Prop; value?: Prop; link?: Prop };
  pickable?: boolean;
  on?: Partial<Record<Intent, Action>>;
  anchors?: { name: string; at: [Prop, Prop] }[];
  layout?: Layout;
  size?: { w?: Size; h?: Size };
  scales?: Record<string, ScaleDecl>;
  /** A box behind the group's content, sized to it after layout (cards, legends): `fit: "width"`
   * spans the group's box across and hugs the content's height. */
  backdrop?: { fill?: Prop; stroke?: { paint: Prop; width?: Prop }; radius?: Prop; padding?: Prop | [Prop, Prop, Prop, Prop]; fit?: "content" | "width" };
  /** Place the content after layout at the first of these anchors where it covers the least data
   * and text (`top-left` … `bottom-right`, `center`; an anchor may be an expression). A single
   * anchor is a fixed place: the content stays there whatever it covers. */
  dodge?: Prop[];
  /** Keep this group's texts off each other (and off every other text) after layout: in order, a
   * text that would land on an earlier one tries the other side of its point (it's `offset` from
   * it), else it's left out — place names that crowd together on a phone. List the important first. */
  declutter?: boolean;
  /** A coordinate system for descendants: `{ type: "geo", projection, fit, padding }`, … */
  coord?: { type: "geo" | "planar" | "cartesian"; projection?: string; fit?: unknown; padding?: number; [k: string]: unknown };
  prov?: string;
}

export type Role = "group" | "datum" | "series" | "region" | "axis" | "tick" | "grid" | "legend" | "legend-item"
  | "annotation" | "title" | "label" | "tooltip" | "control" | "decoration";
export type Intent = "inspect" | "activate" | "pan" | "zoom" | "brush" | "drag" | "pick";
/** px, measured content, a share of what is left, a percentage — or an expression over the parent box in px (`e("min(260, box.w - 24)")`). */
export type Size = number | "auto" | "fill" | `${number}%` | { fill: number } | { expr: string };
export interface Layout {
  type?: "stack" | "rows" | "columns" | "grid" | "flow"; gap?: number; padding?: number | [number] | [number, number] | [number, number, number] | [number, number, number, number]; columns?: number; align?: "start" | "center" | "end" | "stretch";
  /** `columns` only: narrower than this (px), the children stack as rows, equal shares (side by side on a desktop, one above the other on a phone). */
  wrap?: number;
}

export interface ScaleDecl {
  type: "linear" | "log" | "sqrt" | "pow" | "symlog" | "time" | "band" | "point" | "ordinal" | "quantize" | "quantile"
    | "threshold" | "sequential" | "diverging" | "categorical" | "piecewise";
  domain?: Json;
  range?: Json;
  zero?: boolean;
  nice?: boolean;
  padding?: number;
  /** `pow` only: values map through `sign(x)·|x|^exponent` (default 1, linear; `sqrt` is pow 0.5). */
  exponent?: number;
  [param: string]: unknown;
}

function node(kind: string, opts: NodeOpts | undefined, body: Record<string, unknown>): Template {
  return clean({ kind, ...opts, ...body }) as Template;
}

export function group(opts: NodeOpts & { children?: (Template | null | undefined | false)[] } = {}): Template {
  const { children, ...rest } = opts;
  return node("group", rest, { children: (children ?? []).filter(Boolean) });
}

/** A fit camera: `fit` a bbox (`{ bbox: [x0, y0, x1, y1] }`, numbers or expressions), a lon/lat box
 * (`{ geo: … }`) or node keys, `padding` px around it. `explore` names signals (`<explore>.x`,
 * `.y`, `.zoom`) that drag and wheel/pinch move, zooming between `minZoom` and `maxZoom` × the fit
 * (defaults 0.5 and 64). */
export interface FitCamera { fit: Json; padding?: number; explore?: string; maxZoom?: number; minZoom?: number }

export function view(opts: NodeOpts & { camera?: FitCamera | { x: Prop; y: Prop; zoom: Prop }; clip?: boolean; children?: (Template | null | undefined | false)[] } = {}): Template {
  const { children, camera, clip, ...rest } = opts;
  const cam = camera && "fit" in camera
    ? clean({ fit: camera.fit, padding: camera.padding, explore: camera.explore, max_zoom: camera.maxZoom, min_zoom: camera.minZoom })
    : camera;
  return node("view", rest as NodeOpts, { camera: cam, clip: clip === false ? false : undefined, children: (children ?? []).filter(Boolean) } as Record<string, unknown>);
}

export type Geom = Record<string, unknown> & { type: string };

export const geom = {
  rect: (g: { x: Prop; y: Prop; w: Prop; h: Prop; r?: Prop }): Geom => clean({ type: "rect", ...g }) as Geom,
  circle: (g: { cx: Prop; cy: Prop; r: Prop }): Geom => clean({ type: "circle", ...g }) as Geom,
  ellipse: (g: { cx: Prop; cy: Prop; rx: Prop; ry: Prop }): Geom => clean({ type: "ellipse", ...g }) as Geom,
  arc: (g: { cx: Prop; cy: Prop; r0?: Prop; r1: Prop; a0: Prop; a1: Prop }): Geom => clean({ type: "arc", ...g }) as Geom,
  segment: (g: { x1: Prop; y1: Prop; x2: Prop; y2: Prop }): Geom => clean({ type: "segment", ...g }) as Geom,
  polyline: (g: { from: string; x: Prop; y: Prop; curve?: string; closed?: boolean }): Geom => clean({ type: "polyline", ...g }) as Geom,
  area: (g: { from: string; x: Prop; y0: Prop; y1: Prop; curve?: string }): Geom => clean({ type: "area", ...g }) as Geom,
  path: (d: Prop): Geom => clean({ type: "path", d }) as Geom,
  symbol: (g: { symbol?: Prop; x: Prop; y: Prop; size: Prop }): Geom => clean({ type: "symbol", ...g }) as Geom,
  /** A feature of a geo source by id — or, with no arguments inside a `tiles` layer template, the
   * current tile feature's own geometry. */
  feature: (source?: string, id?: Prop): Geom => clean({ type: "feature", source, id }) as Geom,
};

export interface StrokeOpts { paint: Prop; width?: Prop; dash?: number[]; cap?: "butt" | "round" | "square"; join?: "miter" | "round" | "bevel"; nonScaling?: boolean }

function stroke(s?: StrokeOpts) {
  if (!s) return undefined;
  const { nonScaling, ...rest } = s;
  return clean({ ...rest, non_scaling: nonScaling || undefined });
}

export function shape(g: Geom, opts: NodeOpts & { fill?: Prop; stroke?: StrokeOpts; markers?: { start?: Json; end?: Json } } = {}): Template {
  const { fill, stroke: st, markers, ...rest } = opts;
  return node("shape", rest, { geom: g, fill, stroke: stroke(st), markers });
}

export interface TextStyleOpts { font?: string; size?: Prop; weight?: Prop; ink?: Prop; align?: Prop; baseline?: Prop; maxWidth?: Prop; /** Nudged back inside the canvas if it would cross an edge. */ contain?: Prop }

/** Text at `at`. `offset` shifts it in screen px (a label beside a point, at any camera zoom). */
export function text(content: Prop, at: [Prop, Prop], opts: NodeOpts & { style?: TextStyleOpts; halo?: [Prop, Prop]; number?: { value: Prop; format?: Prop }; rotate?: Prop; offset?: [Prop, Prop] } = {}): Template {
  const { style, halo, number, rotate, offset, ...rest } = opts;
  const st = style ? clean({ font: style.font, size: style.size, weight: style.weight, ink: style.ink, align: style.align, baseline: style.baseline, max_width: style.maxWidth, contain: style.contain }) : {};
  return node("text", rest, { text: content ?? "", at, style: st, halo, number, rotate, offset });
}

export type RepeatFrom = string | { groups: string; by: string } | { ticks: string; count?: Prop } | { legend: string } | { count: Prop };

export function repeat(from: RepeatFrom, template: Template): Template {
  return clean({ kind: "repeat", from, template }) as Template;
}

export interface InstancesOpts extends NodeOpts {
  from: string;
  proto?: "circle" | "square" | "diamond" | "triangle" | "cross" | "star" | "rect";
  instanceKey?: Prop;
  x: Prop; y: Prop;
  /** Radius (symbols). */
  r?: Prop; w?: Prop; h?: Prop; fill?: Prop; opacity?: Prop;
  stroke?: StrokeOpts; screenSize?: boolean; label?: Prop;
  /** Where the pointer finds the instances: `"marks"` (default), each mark and a few px around it;
   * `"line"`, anywhere along the line through them in row order, within `reach` px (default 16),
   * finding the instance nearest to where it meets the line — a line's value wherever it's hovered
   * or tapped. */
  hit?: "marks" | "line";
  reach?: number;
  /** Level of detail for rows beyond what a frame can draw (millions): indexed once into a pyramid,
   * each frame draws a density-preserving sample of the rows in view — at most `points` of them
   * (150,000) at the document's size, that share of it in a smaller view (the same density on a
   * phone), every row once they fit. Rows sit at their `x`/`y` (which may read the row only);
   * put the node in a view or under a transform to map them to the screen. `budget`: rows per
   * tile of the index at average density (2048). */
  lod?: boolean | { points?: number; budget?: number };
}

export function instances(o: InstancesOpts): Template {
  const { from, proto, instanceKey, x, y, r, w, h, fill, opacity, stroke: st, screenSize, label, hit, reach, lod, ...rest } = o;
  const l = lod === true ? {} : lod ? clean({ points: lod.points, budget: lod.budget }) : undefined;
  return node("instances", rest, { from, proto: proto ?? "circle", instance_key: instanceKey, x, y, r, w, h, fill, instance_opacity: opacity, stroke: stroke(st), screen_size: screenSize || undefined, label, hit, reach, lod: l });
}

/** One layer of a `tiles` node: the archive layer's features through a template (resolved once per
 * feature and tile). Inside, `d` is the feature — its properties plus `$type` (point | line |
 * polygon), `$id`, `$x`/`$y` (its anchor) — the signal `tile.zoom` is the zoom drawn at, and
 * `geom.feature()` is its geometry. */
export interface TileLayer {
  /** The layer's name in the archive. */
  layer: string;
  /** Names this layer's nodes (default: `layer`). */
  id?: string;
  filter?: Prop;
  /** Draws in `[minzoom, maxzoom)`. */
  minzoom?: number;
  maxzoom?: number;
  template: Template;
  /** Batch features of identical style into one node per tile (default true). */
  merge?: boolean;
  /** Labels: above every tile, placed in screen space by `priority` (highest first), no overlaps. */
  labels?: boolean;
  priority?: Prop;
}

/** Features of a `tiles` source for what the enclosing view shows, at a zoom matching its camera,
 * in the enclosing `geo` coordinate system. Resolved per frame, so camera flights stream tiles. */
export function tiles(opts: NodeOpts & { source: string; layers: TileLayer[]; tileSize?: number }): Template {
  const { source, layers, tileSize, ...rest } = opts;
  return node("tiles", rest, { source, layers: layers.map((l) => clean(l)), tile_size: tileSize });
}

/** A recipe instance: expanded by the engine's sandbox. */
export function use(recipe: string, params: Record<string, unknown> = {}, opts: NodeOpts = {}): Template {
  return clean({ kind: "use", recipe, params, ...opts }) as Template;
}
