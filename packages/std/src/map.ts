// Maps as ordinary recipes (docs/09-geo.md): a group with a `geo` coordinate system, regions as
// keyed feature shapes, symbols and points as instances, lines as strokes. Map ↔ chart morphs need
// no bridge: a region's shape pairs with a bar by key like any other element.

import { e, group, instances, op, recipe, repeat, shape, geom, t, text, view, Cx, Prop, ScaleDecl, Template } from "@datars/sdk";

export interface MapParams {
  source: string; data: string; key: string; value: string;
  colorType: "sequential" | "diverging" | "categorical" | "piecewise"; stops: string;
  projection: string; fit: unknown; camera: unknown; padding: number;
  backdrop: boolean; sea: boolean; under: string; stroke: boolean; legend: boolean; labels: boolean; format: string;
  selected: string; label: Prop; chapter: string;
  regions: boolean; base: Template[]; children: Template[]; fillOpacity: number;
}

export const map = recipe<MapParams>({
  id: "@datars/std/map",
  doc: "A map: regions of a geo source, coloured by a data table joined on a key (choropleth), with neutral backdrop land, over which children (symbols, points, lines, annotations) draw in the same projection.",
  params: {
    source: t.table("A geo source (GeoJSON, TopoJSON or an atlas like `countries`)."),
    data: t.table("Values per region (optional)."), key: t.field("The data column holding region ids."), value: t.field("The value to colour by."),
    colorType: t.oneOf(["sequential", "diverging", "categorical", "piecewise"] as const, "sequential"),
    stops: t.string(undefined, "Piecewise colour stops: '#22c55e 2 · #f5a524 4 · #f97362 6.5'."),
    projection: t.string("equal-earth", "equal-earth, mercator, web-mercator, natural-earth, albers-usa, orthographic, sweref99tm, planar, …"),
    fit: t.json("What the projection fits: {keys: [...]}, {bbox: [lon0, lat0, lon1, lat1]} or the whole source (default). A fit clips the map to its box."),
    camera: t.json("A camera over the map: {fit: {keys: [...]} | {bbox: [x0, y0, x1, y1]}, padding} — bbox corners may be expressions like geo.x(lon, lat). Cameras fly between states; the projection stays."),
    padding: t.number(8),
    backdrop: t.bool(true, "Draw regions without data too (in $map.no-data); off: only regions with data."),
    sea: t.bool(false, "Fill the map's box with $map.water first (the sea around atlas regions, in themes that colour it)."),
    under: t.table("Another geo source drawn beneath as neutral land (countries around admin-1 regions)."),
    stroke: t.bool(true, "Borders between regions."),
    legend: t.bool(false, "A colour ramp (or swatches, for categorical colour) in the lower-left corner."),
    labels: t.bool(false), format: t.string(".1~f"),
    selected: t.string(undefined, "A keyset signal: selected regions stay strong, the others recede."),
    label: t.prop("Region label (tooltip, accessible name): an expression over the region row; default `name: value`."),
    chapter: t.string(undefined, "Clicking a region enters this program chapter with the region's id (drill-down)."),
    regions: t.bool(true, "Draw the source's regions; off: only the layers (a camera beat over a basemap)."),
    base: t.children("Layers beneath everything, moving with the camera: a basemap (`basemap({ part: 'base' })`)."),
    fillOpacity: t.number(undefined, "Region opacity; default 0.85 over a `base` basemap (its streets and water show through), else 1."),
    children: t.children("Layers drawn on top: symbols, geoPoints, geoLines, annotations."),
  },
  tokens: ["map.land", "map.no-data", "map.border", "map.water", "sequential", "diverging", "categorical"],
  expand(p, cx) {
    const scales: Record<string, never> = {} as never;
    let regions = p.source;
    if (p.data && p.key) {
      const vals = cx.table("values", p.data, op.derive("id", e(`d.${p.key}`)));
      // backdrop: every feature (no data → $map.no-data); otherwise only the features with data.
      regions = cx.table("regions", p.source, op.join(vals, "id", p.backdrop ? "left" : "inner"));
      (scales as Record<string, unknown>).color = p.colorType === "piecewise"
        ? { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } }
        : { type: p.colorType, domain: { data: p.data, field: p.colorType === "categorical" ? p.key : p.value }, range: p.colorType === "categorical" ? "$categorical" : p.colorType === "diverging" ? "$diverging" : "$sequential" };
    }
    const color = p.data ? `d.${p.value} == null ? "$map.no-data" : scale.color(d.${p.colorType === "categorical" ? p.key : p.value})` : `"$map.land"`;
    // Selected regions keep their colour; the others recede to neutral land (a colour, so the
    // change animates like any other fill).
    const fill = p.selected ? e(`${p.selected}.isEmpty() || ${p.selected}.has(d.id) ? (${color}) : "$map.no-data"`) : p.data ? e(color) : "$map.land";
    const label = p.label ?? (p.data ? e(`\`\${d.name ?? d.id}: \${d.${p.value} == null ? "no data" : format(d.${p.value}, ${JSON.stringify(p.format)})}\``) : e("d.name ?? d.id"));
    const coord = { type: "geo" as const, projection: p.projection, fit: p.fit ?? { source: p.source }, padding: p.padding };
    const layers: (Template | null)[] = [
      p.base?.length ? group({ key: "base", children: p.base }) : null,
      // Neutral land from another source beneath (decoration: it never pairs with data regions).
      p.under ? group({ key: "under", children: [repeat(p.under, shape(geom.feature(p.under, e("d.id")), {
        fill: "$map.land", stroke: p.stroke ? { paint: "$map.border", width: 0.5, nonScaling: true } : undefined, semantics: { role: "decoration" },
      }))] }) : null,
      p.regions === false ? null : group({ key: "regions", opacity: (p.fillOpacity ?? (p.base?.length ? 0.85 : 1)) < 1 ? (p.fillOpacity ?? 0.85) : undefined, children: [repeat(regions, shape(geom.feature(p.source, e("d.id")), {
        fill,
        stroke: p.stroke ? { paint: "$map.border", width: 0.5, nonScaling: true } : undefined,
        semantics: { role: "region", label, value: p.data ? e(`d.${p.value}`) : undefined },
        on: p.chapter ? { activate: { chapter: p.chapter, key: e("d.id") } } : undefined,
        pickable: true,
      }))] }),
      // Region names where they fit: small regions crowding together (a phone) keep only the first.
      p.labels && p.regions !== false ? group({ key: "labels", declutter: true, children: [repeat(regions, text(e("d.name ?? d.id"), [e(`geo.cx(${JSON.stringify(p.source)}, d.id)`), e(`geo.cy(${JSON.stringify(p.source)}, d.id)`)], { style: { size: "$size.small", ink: "$map.label", align: "middle", baseline: "middle" }, halo: ["$map.label-halo", 2] }))] }) : null,
      ...(p.children ?? []),
    ];
    const legend = p.legend && p.data ? colorLegend(p, cx) : null;
    // The sea stays put under a moving camera: it is the box, not a place.
    const sea = p.sea ? shape(geom.rect({ x: 0, y: 0, w: e("box.w"), h: e("box.h") }), { key: "sea", fill: "$map.water", semantics: { role: "decoration" } }) : null;
    if (p.camera) {
      // The layers move with the camera inside a view (in the map's coordinates, so the camera can
      // frame lon/lat boxes with geo.x/geo.y); the legend stays put.
      return group({
        key: "map",
        scales,
        semantics: { role: "group", label: "Map" },
        children: [sea, view({ key: "view", camera: p.camera as never, coord, children: layers }), legend],
      });
    }
    // A fit to part of the source (a bbox, some keys) leaves the rest outside the box: clip it.
    return group({ key: "map", scales, coord, clip: p.fit ? "box" : undefined, semantics: { role: "group", label: "Map" }, children: [sea, ...layers, legend] });
  },
});

/** The map's legend, in its lower-left corner: a stepped ramp with the extremes labelled, or
 * swatches for categorical colour. */
function colorLegend(p: MapParams, cx: Cx): Template {
  if (p.colorType === "categorical") {
    return group({ key: "legend", transform: { translate: [8, e("box.h - 18")] }, semantics: { role: "legend", label: "Legend" }, layout: { type: "flow", gap: 14 }, children: [
      repeat({ legend: "color" }, group({ semantics: { role: "legend-item", label: e("key.name(d.label)") }, children: [
        shape(geom.rect({ x: 0, y: 0, w: 10, h: 10, r: 2 }), { fill: e("d.ink") }),
        text(e("key.name(d.label)"), [15, 9], { style: { size: "$size.label", ink: "$ink-2" } }),
      ] })),
    ] });
  }
  // The ramp spans the pinned stops (piecewise colour), else the data's extent — as a one-row
  // table, so the swatches and labels read `d.lo` / `d.hi`.
  const pinned = p.colorType === "piecewise" && p.stops ? p.stops.split(/[\s·,;]+/).filter((s) => s !== "" && !Number.isNaN(Number(s))).map(Number) : [];
  const extent = pinned.length >= 2
    ? cx.table("extent", p.data, op.aggregate([], { n: ["count"] }), op.derive("lo", pinned[0]), op.derive("hi", pinned[pinned.length - 1]))
    : cx.table("extent", p.data, op.aggregate([], { lo: ["min", p.value], hi: ["max", p.value] }));
  const steps = 6, w = 22;
  return group({ key: "legend", transform: { translate: [8, e("box.h - 30")] }, semantics: { role: "legend", label: "Colour scale" }, children: [
    repeat(extent, group({ key: "ramp", children: [
      ...Array.from({ length: steps }, (_, i) => shape(geom.rect({ x: i * w, y: 0, w, h: 9 }), { key: `step-${i}`, fill: e(`scale.color(d.lo + (d.hi - d.lo) * ${i / (steps - 1)})`), semantics: { role: "decoration" } })),
      text("", [0, 22], { key: "lo", number: { value: e("d.lo"), format: p.format }, style: { size: "$size.small", ink: "$ink-2" }, halo: ["$paper", 2] }),
      text("", [steps * w, 22], { key: "hi", number: { value: e("d.hi"), format: p.format }, style: { size: "$size.small", ink: "$ink-2", align: "end" }, halo: ["$paper", 2] }),
    ] })),
  ] });
}

export interface SymbolsParams { source: string; data: string; key: string; value: string; max: number; r: number; fill: Prop; stops: string; format: string; label: Prop }

export const symbols = recipe<SymbolsParams>({
  id: "@datars/std/symbols",
  doc: "Proportional symbols: a circle per region (or point feature) at its visual centre, area ∝ value — or all one size (`r`) without a value field.",
  params: {
    source: t.table(), data: t.table(), key: t.field(), value: t.field("Size by this field (omit for same-size dots)."),
    max: t.number(28, "Largest radius (px)."), r: t.number(4, "Radius without a value field (px)."), fill: t.prop(),
    stops: t.string(undefined, "Colour by value on piecewise stops ('#bae6fd 7 · #0b1f3a 14.2') instead of one fill."),
    format: t.string(",.0f"),
    label: t.prop("Label per symbol (tooltip, accessible name); default `name: value`."),
  },
  expand(p) {
    const name = `key.name(d.${p.key})`;
    const scales: Record<string, ScaleDecl> = {};
    if (p.value) scales.size = { type: "sqrt", domain: { data: p.data, field: p.value }, range: [0, p.max], zero: true };
    if (p.value && p.stops) scales.fill = { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } };
    const fill = p.value && p.stops ? e(`scale.fill(d.${p.value})`) : p.fill ?? "$accent@0.7";
    return group({
      key: "symbols",
      scales: p.value ? scales : undefined,
      children: [instances({
        key: "circles", from: p.data, instanceKey: e(`d.${p.key}`),
        x: e(`geo.cx(${JSON.stringify(p.source)}, d.${p.key})`), y: e(`geo.cy(${JSON.stringify(p.source)}, d.${p.key})`),
        r: p.value ? e(`scale.size(d.${p.value})`) : p.r, fill, stroke: { paint: "$paper", width: 0.75 }, screenSize: true,
        label: p.label ?? (p.value ? e(`\`\${${name}}: \${format(d.${p.value}, ${JSON.stringify(p.format)})}\``) : e(name)),
      })],
    });
  },
});

export interface GeoPointsParams { data: string; lon: string; lat: string; r: Prop; fill: Prop; label: Prop; key: string }

export const geoPoints = recipe<GeoPointsParams>({
  id: "@datars/std/geoPoints",
  doc: "Points at lon/lat (cities, events, stations), in the enclosing map's projection, in $map.marker ringed by $map.label-halo so they read on any land.",
  params: { data: t.table(), lon: t.field(), lat: t.field(), r: t.prop(), fill: t.prop(), label: t.prop(), key: t.field() },
  tokens: ["map.marker", "map.label-halo"],
  expand(p) {
    return instances({
      key: "points", from: p.data, instanceKey: p.key ? e(`d.${p.key}`) : undefined,
      x: e(`geo.x(d.${p.lon}, d.${p.lat})`), y: e(`geo.y(d.${p.lon}, d.${p.lat})`),
      r: p.r ?? 3, fill: p.fill ?? "$map.marker", stroke: { paint: "$map.label-halo", width: 1 }, screenSize: true, label: p.label,
    });
  },
});

export interface GeoLinesParams { source: string; ink: string; width: number; dash: boolean; data: string; key: string; value: string; stops: string; format: string }

export const geoLines = recipe<GeoLinesParams>({
  id: "@datars/std/geoLines",
  doc: "Line features of a geo source (roads, borders, courses, routes) as non-scaling strokes. With `data` and `key`, one line per row (the feature with that id) — data, not decoration — coloured by `value` on piecewise `stops`.",
  params: {
    source: t.table(), ink: t.ink("$map.border"), width: t.number(1), dash: t.bool(false),
    data: t.table(), key: t.field("The data field holding each row's feature id."), value: t.field("Colour lines by this field (with `stops`)."),
    stops: t.string(undefined, "Piecewise colour stops for `value` (`#a 0 · #b 10`)."), format: t.string(",.1~f"),
  },
  expand(p) {
    const stroke = (paint: Prop) => ({ paint, width: p.width, nonScaling: true, dash: p.dash ? [4, 3] : undefined });
    if (!p.data || !p.key) {
      return group({ key: `lines-${p.source}`, children: [repeat(p.source, shape(geom.feature(p.source, e("d.id")), { stroke: stroke(p.ink), semantics: { role: "decoration" } }))] });
    }
    const coloured = !!(p.value && p.stops);
    const label = p.value ? e(`key.name(d.${p.key}) + ": " + format(d.${p.value}, ${JSON.stringify(p.format)})`) : e(`key.name(d.${p.key})`);
    return group({
      key: `lines-${p.source}`,
      scales: coloured ? { stroke: { type: "piecewise", stops: p.stops, domain: { data: p.data, field: p.value } } } : undefined,
      children: [repeat(p.data, shape(geom.feature(p.source, e(`d.${p.key}`)), {
        key: e(`d.${p.key}`),
        stroke: stroke(coloured ? e(`scale.stroke(d.${p.value})`) : p.ink),
        semantics: { role: "datum", label },
        pickable: true,
      }))],
    });
  },
});

export interface RouteParams { data: string; lon0: string; lat0: string; lon1: string; lat1: string; ink: string; width: Prop }

export const route = recipe<RouteParams>({
  id: "@datars/std/route",
  doc: "Great-circle routes between two lon/lat points per row.",
  params: { data: t.table(), lon0: t.field(), lat0: t.field(), lon1: t.field(), lat1: t.field(), ink: t.ink("$accent@0.6"), width: t.prop() },
  expand(p) {
    return group({ key: "routes", children: [repeat(p.data, shape(geom.path(e(`geo.geodesic(d.${p.lon0}, d.${p.lat0}, d.${p.lon1}, d.${p.lat1})`)), { stroke: { paint: p.ink, width: p.width ?? 1.5, nonScaling: true, cap: "round" } }))] });
  },
});

export interface TrackParams { data: string; time: string; lon: string; lat: string; until: Prop; ink: string; head: boolean; future: boolean }

export const track = recipe<TrackParams>({
  id: "@datars/std/track",
  doc: "A path over time (a storm, a flight): drawn up to `until` (a data-time signal) with a moving head.",
  params: { data: t.table(), time: t.field(), lon: t.field(), lat: t.field(), until: t.prop("Data time to draw up to (e.g. the `year` signal)."), ink: t.ink("$negative"), head: t.bool(true),
    future: t.bool(false, "Draw the whole path faintly underneath (where it will go)."),
  },
  expand(p, cx) {
    const until = typeof p.until === "number" ? String(p.until) : p.until && typeof p.until === "object" && "expr" in (p.until as object) ? (p.until as { expr: string }).expr : "1e18";
    const drawn = cx.table("drawn", p.data, op.filter(e(`d.${p.time} <= (${until})`)));
    return group({
      key: "track",
      children: [
        p.future ? shape(geom.polyline({ from: p.data, x: e(`geo.x(d.${p.lon}, d.${p.lat})`), y: e(`geo.y(d.${p.lon}, d.${p.lat})`), curve: "catmull-rom" }), { key: "future", stroke: { paint: p.ink, width: 1.5, nonScaling: true, cap: "round", join: "round", dash: [4, 4] }, opacity: 0.35 }) : null,
        // The head is the path's own end: when the clock moves between steps the path grows along
        // itself (a line that runs on is trimmed, not bent) and the head rides its tip.
        shape(geom.polyline({ from: drawn, x: e(`geo.x(d.${p.lon}, d.${p.lat})`), y: e(`geo.y(d.${p.lon}, d.${p.lat})`), curve: "catmull-rom" }), {
          key: "path", stroke: { paint: p.ink, width: 2.5, nonScaling: true, cap: "round", join: "round" }, markers: p.head ? { end: { type: "dot", r: 5 } } : undefined,
        }),
      ],
    });
  },
});

export interface DotDensityParams { source: string; data: string; key: string; value: string; per: number; r: number; fill: Prop; seed: number }

export const dotDensity = recipe<DotDensityParams>({
  id: "@datars/std/dotDensity",
  doc: "Dot density: one dot per `per` units of a region's value, scattered evenly inside it (seeded, deterministic). Dots are keyed (region, i): in another state a region's dots pair only with its own, and the region's key splits into them.",
  params: {
    source: t.table("The geo source whose features the dots fill."), data: t.table("Values per region."),
    key: t.field("The data column holding region ids (default: the table's key)."), value: t.field(),
    per: t.number(1000, "Units per dot."), r: t.number(1.2, "Dot radius (px)."), fill: t.prop(), seed: t.number(1),
  },
  expand(p, cx) {
    const dots = cx.table("dots", p.data, op.scatterIn({ geo: p.source, key: p.key, count: e(`floor(d.${p.value} / ${p.per})`), seed: p.seed }));
    // Keyed by the table's (region, dot) key; each says whose it is.
    const label = p.key ? e(`key.name(d.${p.key})`) : undefined;
    return instances({ key: "dots", from: dots, x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"), r: p.r, fill: p.fill ?? "$ink@0.7", screenSize: true, label });
  },
});
