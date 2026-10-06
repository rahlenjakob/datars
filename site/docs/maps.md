---
title: Maps
description: Choropleths, symbols, dot density, routes and basemaps in any projection; automatic street-level basemaps from OpenStreetMap; camera flights from the world to a street.
lede: A map is a scene like any other — regions are keyed shapes, the basemap is a node, the camera belongs to a view. So a country can morph into its bar, and one camera can fly from the globe to a street corner.
---

## A choropleth in one call

```ts
import { doc, data, group } from "@datars/sdk";
import { map } from "@datars/std";

export default doc({
  title: "Renewable share of energy",
  size: [760, 480],
  data: {
    world: data.atlas("countries"),
    values: data.values({ id: ["SWE", "NOR", "FIN", "DNK", "ISL"], share: [66, 75, 48, 44, 85] }, { key: "id" }),
  },
  scene: group({ key: "root", layout: { type: "stack", padding: [16, 20, 12, 12] }, children: [
    map({ source: "world", data: "values", key: "id", value: "share", colorType: "sequential", legend: true }),
  ] }),
});
```

This is `datars new mymap --template map`. The `map` recipe draws the regions of a geo source, joins your table on `key`, and colours by `value` with the theme's `sequential` ramp. Regions without data are drawn in `$map.no-data` (turn that off with `backdrop: false`). Tooltips and accessible names default to `name: value`; set `label` to change them.

## Geo sources

| Source | What it is |
|---|---|
| `data.atlas("countries")` | The built-in world atlas: every country with its ISO 3166-1 alpha-3 code as `id` (`"SWE"`) and its `name`. Published charts ship it as a compact topology (about 84 KB gzipped), shared by hash between every chart that uses it. |
| `data.url("regions.geojson", { id: "code" })` | A GeoJSON file next to the document. `id` names the feature property that holds each region's id. |
| `data.geojson(featureCollection, { id })`, `data.topojson(topology, { id })` | Inline GeoJSON or TopoJSON. |

Every geo source is also a table — one row per feature, with `id`, `name` and the feature's properties — so you can filter and join it like any other.

`datars data profile values.csv` recognises columns of country codes and tells you they join to the atlas.

## Layers on a map

`map` takes `children` drawn on top, in the same projection, and `base` layers drawn beneath:

| Recipe | Draws |
|---|---|
| [`symbols`](/docs/std/symbols/) | a circle per region at its visual centre, area proportional to a value |
| [`dotDensity`](/docs/std/dotDensity/) | one dot per `per` units of a region's value, scattered evenly inside it (seeded, the same on every platform) |
| [`geoPoints`](/docs/std/geoPoints/) | points at longitude/latitude — cities, events, stations |
| [`geoLines`](/docs/std/geoLines/) | line features — roads, borders, courses — as strokes that keep their width at any zoom |
| [`route`](/docs/std/route/) | great-circle routes between two longitude/latitude points per row |
| [`track`](/docs/std/track/) | a path over time (a storm, a flight), drawn up to a data-time signal with a moving head |
| [`basemap`](/docs/std/basemap/) | streets, water, parks, buildings and place names from vector tiles (below) |
| [`attribution`](/docs/std/attribution/) | the data credit OpenStreetMap's licence requires |

Inside a map, `geo.x(lon, lat)` and `geo.y(lon, lat)` are expressions that project a point, so any primitive can be placed on it:

```ts
instances({ from: "cities", x: e("geo.x(d.lon, d.lat)"), y: e("geo.y(d.lon, d.lat)"),
  r: e("scale.r(d.pop)"), fill: "$accent@0.72", screenSize: true })
```

Useful `map` parameters: `projection`, `fit` (what the projection frames: `{ keys: [...] }`, `{ bbox: [lon0, lat0, lon1, lat1] }`, or the whole source), `camera` (a camera that flies between states while the projection stays), `stops` (piecewise colour stops like `'#22c55e 2 · #f5a524 4 · #f97362 6.5'`), `selected` (a keyset signal: selected regions stay strong), `chapter` (click a region to drill into it), `under` (another source drawn beneath as neutral land), `sea`, `labels`, `legend`. The [map reference](/docs/std/map/) lists them all.

## Projections

Set `projection` on `map`, or on any view's geo coordinate system (`coord: { type: "geo", projection }`).

| Name | Use it for |
|---|---|
| `equal-earth` (default) | world maps where area matters |
| `natural-earth` | world maps with a softer look |
| `web-mercator` | basemaps and street-level views (tiles are cut in it) |
| `mercator` | navigation-style maps |
| `equirectangular` (`plate-carree`) | simple longitude/latitude grids |
| `albers` | mid-latitude countries |
| `albers-usa` | the United States with Alaska and Hawaii inset |
| `lambert` (`lambert-conformal-conic`) | regional maps |
| `orthographic` (`globe`) | a globe; its `center: [lon, lat]` may be expressions, so a signal can turn it |
| `sweref99tm`, `utm-33n` (any zone, `n` or `s`) | national and UTM grids |
| `planar`, `planar-y-down` | things that aren't on Earth: floor plans, stadiums, fantasy worlds |

A spinning globe is a clock signal in the projection's centre:

```ts
signals: { spin: signal.clock(8) },   // counts up 8 units a second while the chart is on screen
// …
coord: { type: "geo", projection: "orthographic", center: [e("15 - spin"), 15], fit: "sphere" },
```

Clocks pause off screen, during transitions and for readers who prefer reduced motion.

## Views and cameras

A `view` is a window with a camera. Give it a geo coordinate system and a camera that fits something; the camera is resolved per state, and between states it flies.

```ts
view({
  key: "map",
  coord: { type: "geo", projection: "web-mercator", fit: { bbox: [-180, -85.0511, 180, 85.0511] }, padding: 0 },
  camera: { fit: { geo: "=bounds" }, padding: 12 },    // a lon/lat box held in a signal
  children: [ /* basemap, data layers, labels */ ],
})
```

A camera can fit:

- `{ geo: [west, south, east, north] }` — a longitude/latitude box (numbers, expressions, or a signal: `"=bounds"`);
- `{ keys: [...] }` or `{ keys: "=focus" }` — the regions with these ids;
- `{ bbox: [x0, y0, x1, y1] }` — a box in the view's own units.

Add `explore: "map"` to let readers drag, wheel and pinch the view; the camera then writes the signals `map.x`, `map.y` and `map.zoom`, zooming between `minZoom` and `maxZoom` times the fit (defaults 0.5 and 64). Stepping the story flies back into the narrative.

### Flights

When the camera's target changes from one state to the next, the engine flies it along the van Wijk–Nuij path — zoom out, pan, zoom in — so a world-to-street move reads as one continuous flight. There's nothing to write; set the flight's timing with a motion rule on the view:

```ts
motion: motion({ select: { kind: "view" }, duration: 3.2, easing: "cubic-in-out" }),
```

Place labels fade during a flight and settle when it lands. Every frame of the flight is a pure function of the plan and the time, so it is the same in a browser, in an app, in `datars video` and in a test.

## Basemaps

The `basemap` recipe draws a vector-tile source — sea, land, water, parks, rivers, buildings, roads by class, borders and place names — in the view's projection, coloured by the theme's `map.*` tokens (light and dark come with the built-in theme). Split it around your data so place names stay on top:

```ts
children: [
  basemap({ source: "basemap", part: "base" }),     // everything below the data
  instances({ /* your points */ }),
  basemap({ source: "basemap", part: "labels" }),   // place names above
],
```

Turn parts off with `roads: false`, `buildings: false`, `labels: false`, `boundaries: false`; `tileSize` (default 512 px) trades detail for tile count.

### Where the tiles come from

A tiles source is a PMTiles archive: one static file that the engine reads by HTTP range request, fetching only the tiles the camera shows. There is no tile server, no API key and no per-view bill — any static host or CDN that answers range requests will do.

**Your own archive:**

```ts
data: { basemap: data.tiles("tiles/stockholm.pmtiles") },
```

**An automatic basemap:**

```ts
data: { basemap: data.tiles.auto() },
```

With `data.tiles.auto()`, nobody has to make an archive. `datars render`, `dev`, `check` and `publish` look at the document's own cameras — every state, and 48 frames of each flight between states — and cut an archive for exactly those views: Natural Earth for the world, OpenStreetMap from zoom 10 down to streets, buildings and place names in the document's language. A story about Rio gets Rio's streets; the whole flight from the world to Copacabana is {{archive:rio}}.

<figure class="fig"><div class="frame"><div class="chart" data-chart="rio" data-caption></div></div><figcaption><b>From the world to Copacabana</b> — one camera over an automatic basemap, made for these four views when the document was built.</figcaption>{{alt:rio}}</figure>

How it works:

- **The archive lives next to the document** as `<source>.auto.pmtiles`, with its job file `<source>.auto.job.json` beside it — a lockfile. While the cameras don't change, the archive is used as it is; commit both and every build is offline and byte-identical. `datars publish` ships the archive content-addressed (`tiles/<source>.<hash>.pmtiles`).
- **The data is fetched once into a cache**: Natural Earth, simplified land polygons from osmdata, and OpenStreetMap by map cell from the public Overpass API. The cache is keyed by query, so a second story in the same city, or a nudged camera, reuses it.
- **`datars dev` never waits**: it shows a preview at once from what the cache holds, then fetches the missing cells one by one and swaps better archives in as they land — also for places a reader explores.
- **See the plan first**: `datars basemap doc.ts` prints the views, the tiles per zoom, the data it needs and whether the archive is current; `--build` fetches and builds it.

| Environment variable | Effect |
|---|---|
| `DATARS_GEO_CACHE` | where the geodata cache lives (default `~/.cache/datars/geo`) |
| `DATARS_GEO_OFFLINE=1` | never touch the network; build from the cache only |
| `DATARS_OVERPASS_BUDGET` | the daily limit on public Overpass use (default 5,000 queries / 500 MB) |
| `DATARS_OVERPASS_URL` | an Overpass instance of your own, used alone and without a budget |
| `DATARS_GEO_CELLS` | a URL template (`{level}/{z}/{x}/{y}`) of a static server of pre-cut cells |

> **Warning** The public Overpass instances are shared, donated machines. Their policy allows a single author a modest daily volume; a street-level city can be a few hundred megabytes of answers, fetched once. datars keeps a daily budget and stops with a message when it's spent. For a product, a CI farm or a batch of maps, point `DATARS_OVERPASS_URL` or `DATARS_GEO_CELLS` at infrastructure of your own.

## Credits

OpenStreetMap data is licensed under the ODbL, which requires a visible credit. Add `attribution()` beside the map — outside the moving view, so it stays put:

```ts
scene: group({ key: "root", children: [ view({ /* the map */ }), attribution({}) ] }),
```

It draws "© OpenStreetMap contributors" (and Natural Earth), legible on any basemap, and links to the licence page: the web runtime lays a real link over it, and SVG and PDF exports keep it. `datars lint` warns about a chart that draws vector tiles without it (`maps/attribution`).

## Maps and charts in one scene

Because regions are keyed shapes, a country can become its bar. Give the map's regions and the bars the same keys (the ISO code), put them in two states, and pair data marks by key:

```ts
motion: motion({ select: { role: "region" }, matcher: "by-key" }),
scene: group({ children: [
  view({ camera: { fit: { keys: "=focus" } }, when: e('shape == "map"'),
    children: [map({ source: "world", data: "renew", key: "id", value: "share" })] }),
  plot({ data: "nordic", x: "share", y: "name", children: [bar({ labels: true })] }, { when: e('shape == "bars"') }),
] }),
```

<figure class="fig"><div class="frame"><div class="chart" data-chart="renewables" data-caption></div></div><figcaption><b>Regions become bars</b> — the camera flies to Europe and the Nordics, then every country's outline morphs into its bar.</figcaption>{{alt:renewables}}</figure>

## See also

- [Stories](/docs/stories/) — steps that set signals like `bounds` and `focus`.
- [Motion](/docs/motion/) — flight timing and what moves when.
- [Themes](/docs/theming/) — the `map.*` tokens.
- [Maps feature page](/features/maps/) — the live descent from the world to Stockholm.
