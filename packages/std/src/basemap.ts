// The basemap as a recipe (docs/09-geo.md): a style is a function from a tile archive's layers to
// primitives, themed by the document's tokens — so it follows light/dark mode, can fade or be
// ejected, and needs no style language. The engine only knows "tiles, layers, templates"; the
// words land, water, road live here.
//
// Layers read (names overridable with `layers`): land, water, lakes, parks, rivers, buildings,
// roads, boundaries, places. Roads are classed by `kind` (highway | major | minor), or by the OSM /
// Natural Earth `type` for archives that keep raw tags. Places draw their `name`,
// most important first (`pop`, else `rank`).

import { e, geom, group, recipe, shape, t, text, tiles, ParamSpec, Template, TileLayer } from "@datars/sdk";

type LayerName = "land" | "water" | "lakes" | "parks" | "rivers" | "buildings" | "roads" | "boundaries" | "places";

export interface BasemapParams {
  source: string;
  /** `all`; `base` (everything but place names); `labels` (only place names — put it above data
   * layers, with a `base` basemap below them). */
  part: "all" | "base" | "labels";
  labels: boolean;
  roads: boolean;
  buildings: boolean;
  boundaries: boolean;
  /** Draw the sea behind the land (`$map.water`); off to show a map over something else. */
  sea: boolean;
  tileSize: number;
  layers: Partial<Record<LayerName, string>>;
}

const HIGHWAY = ["motorway", "trunk", "motorway_link", "trunk_link", "Major Highway"];
const MAJOR = ["primary", "secondary", "primary_link", "secondary_link", "Secondary Highway"];
const MINOR = ["tertiary", "tertiary_link", "residential", "unclassified", "living_street", "pedestrian", "Road"];

const roadClass = (kind: string, raw: string[]) => e(`d.kind == ${JSON.stringify(kind)} || ${JSON.stringify(raw)}.includes(d.type)`);

/** A value that steps up with the zoom drawn at: `[[zoom, value], …]` ascending. */
function byZoom(stops: [number, number][], extra = 0): string {
  let out = String(stops[0][1] + extra);
  for (const [z, w] of stops.slice(1)) out = `tile.zoom >= ${z} ? ${w + extra} : (${out})`;
  return `=${out}`;
}

const MINOR_W: [number, number][] = [[0, 0.5], [12, 1.3], [14, 2.6]];
const MAJOR_W: [number, number][] = [[0, 0.7], [9, 1.1], [11, 1.8], [13, 3]];
const HIGHWAY_W: [number, number][] = [[0, 0.9], [8, 1.4], [10, 2.2], [12, 3.2], [14, 4.5]];

export const basemap = recipe<BasemapParams>({
  id: "@datars/std/basemap",
  doc: "A basemap from a vector-tile source (our own OSM / Natural Earth archives): sea, land, water, parks, buildings, roads by class, borders and place labels, in the enclosing map's projection, styled by the theme's map tokens. Put it first inside a view with a `geo` coordinate system; data layers go after it (or between `part: base` and `part: labels`).",
  params: {
    source: t.table("A `tiles` source (data.tiles(url))."),
    part: t.oneOf(["all", "base", "labels"] as const, "all", "Split the map around data layers: `base` below them, `labels` above."),
    labels: t.bool(true, "Place names (screen-space placement, no overlaps)."),
    roads: t.bool(true),
    buildings: t.bool(true),
    boundaries: t.bool(true, "Country borders."),
    sea: t.bool(true, "Fill the world behind the land with $map.water."),
    tileSize: t.number(512, "On-screen tile size the zoom aims for (px). Smaller: more detail, more tiles."),
    layers: t.json("Archive layer names, if they differ from the defaults: { land, water, lakes, parks, rivers, buildings, roads, boundaries, places }.") as ParamSpec<BasemapParams["layers"]>,
  },
  tokens: ["map.water", "map.land", "map.park", "map.building", "map.road", "map.road-major", "map.border", "map.label", "map.label-halo"],
  expand(p) {
    const L = { land: "land", water: "water", lakes: "lakes", parks: "parks", rivers: "rivers", buildings: "buildings", roads: "roads", boundaries: "boundaries", places: "places", ...(p.layers ?? {}) };
    const fill = (layer: string, ink: string, o: Partial<TileLayer> = {}): TileLayer => ({ layer, template: shape(geom.feature(), { fill: ink }), ...o });
    const line = (layer: string, ink: string, width: string | number, o: Partial<TileLayer> & { dash?: number[] } = {}): TileLayer => {
      const { dash, ...rest } = o;
      return { layer, template: shape(geom.feature(), { stroke: { paint: ink, width, nonScaling: true, cap: "round", join: "round", dash } }), ...rest };
    };
    const base = p.part !== "labels";
    const labels = p.part !== "base" && p.labels;
    const layers: TileLayer[] = [];
    if (base) {
      layers.push(
        fill(L.land, "$map.land"),
        fill(L.parks, "$map.park"),
        fill(L.water, "$map.water", { filter: e("d.$type == 'polygon'") }),
        fill(L.lakes, "$map.water"),
        line(L.rivers, "$map.water", byZoom([[0, 0.6], [7, 1], [10, 1.6]]), { minzoom: 5 }),
      );
      if (p.buildings) layers.push(fill(L.buildings, "$map.building", { minzoom: 12 }));
      if (p.roads) {
        const minor = roadClass("minor", MINOR);
        layers.push(
          // Street-level minor roads get a casing so white streets read on pale land.
          line(L.roads, "$map.building", byZoom(MINOR_W, 1.6), { id: "roads-minor-casing", filter: minor, minzoom: 12 }),
          line(L.roads, "$map.road", byZoom(MINOR_W), { id: "roads-minor", filter: minor, minzoom: 10 }),
          line(L.roads, "$map.road-major", byZoom(MAJOR_W), { id: "roads-major", filter: roadClass("major", MAJOR), minzoom: 7 }),
          line(L.roads, "$map.road-major", byZoom(HIGHWAY_W), { id: "roads-highway", filter: roadClass("highway", HIGHWAY), minzoom: 5 }),
        );
      }
      if (p.boundaries) layers.push(line(L.boundaries, "$map.border", byZoom([[0, 0.6], [4, 0.9], [8, 1.3]]), { dash: [3, 2] }));
    }
    if (labels) {
      layers.push({
        layer: L.places,
        labels: true,
        priority: e("d.pop ?? (100 - (d.rank ?? 50))"),
        template: text(e("d.name"), [e("d.$x"), e("d.$y")], {
          key: e("d.name"),
          style: { size: e("d.rank != null && d.rank <= 2 ? 13 : 11.5"), weight: e("d.rank != null && d.rank <= 2 ? 600 : 400"), ink: "$map.label", align: "middle", baseline: "middle" },
          halo: ["$map.label-halo", 2.5],
          semantics: { role: "label", label: e("d.name") },
        }),
      });
    }
    const children: Template[] = [];
    if (base && p.sea) {
      // The Mercator world square in content units (other projections: its bounding box).
      children.push(shape(geom.rect({ x: e("geo.x(-180, 85.0511)"), y: e("geo.y(-180, 85.0511)"), w: e("geo.x(180, -85.0511) - geo.x(-180, 85.0511)"), h: e("geo.y(180, -85.0511) - geo.y(-180, 85.0511)") }), { key: "sea", fill: "$map.water" }));
    }
    children.push(tiles({ key: "tiles", source: p.source, tileSize: p.tileSize, layers }));
    return group({ key: p.part === "labels" ? "basemap-labels" : "basemap", semantics: { role: "decoration" }, children });
  },
});

export interface AttributionParams { text: string; ink: string; link: string }

export const attribution = recipe<AttributionParams>({
  id: "@datars/std/attribution",
  doc: "The data credit a map owes (OpenStreetMap's licence requires it), in the bottom-right corner of its box, legible and linked to the licence page (hosts make it a real link; SVG/PDF keep it). Place it outside the map's view so it stays put while the camera moves.",
  // It sits on the map, so it takes the map's label colours (legible on a dark basemap too).
  params: {
    text: t.string("© OpenStreetMap contributors, Natural Earth"), ink: t.ink("$map.label"),
    link: t.string("https://www.openstreetmap.org/copyright", "Where the credit links (OpenStreetMap's copyright page, as its guidelines ask)."),
  },
  tokens: ["map.label", "map.label-halo"],
  expand(p) {
    // The corner of the box it's laid out in, whatever size the chart is drawn at: not the size it
    // was expanded for (a published chart is expanded once, at its document's size).
    return text(p.text, [e("box.w - 6"), e("box.h - 5")], {
      key: "attribution", style: { size: 10.5, ink: p.ink, align: "end", baseline: "bottom" }, halo: ["$map.label-halo", 2.5],
      semantics: { role: "annotation", label: p.text, link: p.link || undefined },
    });
  },
});
