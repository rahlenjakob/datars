# 09 — Geo: maps as ordinary primitives, on our own OSM data

Good map work needs one camera from world to street, van Wijk flights, PMTiles range reads and tile
decode in workers — without becoming a *second engine* bridged to charts, and without a third-party
planet (such as Protomaps' daily build) at runtime. The data comes from our own Rust pipeline,
OpenStreetMap and Natural Earth to PMTiles, shipped as local files with no tile server.
datars builds maps from the same primitives as charts, on that pipeline, and removes the special
cases.

## A map is a scene like any other

| Map concept | Primitive |
|---|---|
| Projection | `Coord::Geo { projection }` on a `Group` |
| Regions (countries, counties, kommuner) | keyed `Shape`s with `Path` geometry, keyed by feature id |
| Points, cities, seats, dot density | `Instances` |
| Roads, borders, courses, walks, tracks | `Shape` polylines (non-scaling strokes) |
| Routes | segments with `geodesic: true`, resampled into great circles |
| The basemap | a `Tiles` node + a style recipe |
| Camera | the camera of a `View` |
| Labels | `Text` through the shared label placement engine |
| Choropleth colours | `Paint::Lookup` — a per-feature colour table rewritten per frame |

So a country lifting off the map into a bar is an ordinary transition: a `Path` pairs with a `Rect`
by key and morphs with the `disc` strategy. No bridge, no `chart_level` blend.

## Coordinate systems

- **Projections:** Web Mercator, Mercator, Equal Earth, Natural Earth, Albers / Albers USA
  (composite), Lambert conformal conic, orthographic (globe), transverse Mercator and national grids
  (SWEREF 99 TM, UTM zones), via parameterized projection code in `datars-geo`.
- **Adaptive resampling** in screen space, antimeridian cutting, and sphere clipping for the globe.
- **Planar CRS** for things that aren't on Earth or don't need a projection: the airport terminal's
  floor plan, Westeros, a stadium, a factory floor — instead of faking them as tiny degrees around
  (0, 0).
- **Precision:** cameras in f64, geometry uploaded to the GPU relative to the
  camera centre (or tile origin), so street zoom doesn't jitter in f32.

## Region atlases at every zoom

Choropleths need *joinable* regions — stable ids, consistent borders — at several levels:

- `datars-geo-build` produces **atlas packages**: countries (ISO 3166-1), first-level subdivisions
  (ISO 3166-2), and national sets (Swedish kommuner and län, US states and counties, …) from OSM
  administrative boundaries or official sources where OSM isn't authoritative (US counties come from
  the Census). Each feature carries its codes and multilingual names.
- **Topology-preserving simplification** (shared arcs) per zoom band, so neighbours never gap or
  overlap at any zoom.
- **Pre-triangulated meshes** per zoom band and quantized coordinates, so the runtime never runs
  earcut on big polygons.
- **Visual centres** (polylabel) and "core landmass" boxes precomputed for symbols, labels and
  `fit(region)` cameras (a country is framed by its core landmass, not its overseas territories).
- Joins by semantic hint: a column tagged `geo: iso3` or `geo: kommun-code` joins to the atlas
  automatically.

**Zooming across levels is a transition.** Zooming into a country can split it into its subdivisions
(`by_parent` matcher, partitioned by the children's real geometry) and merge them back on zoom-out —
drill-down as a continuous zoom, not only as chapters.

## The basemap: `Tiles` + a style recipe

- `Tiles` streams features for the camera's viewport and zoom: the engine emits tile requests; the host
  fulfils them (P10). Decoding and tessellation run in workers; GPU uploads are time-budgeted per
  frame; meshes live in an LRU cache; missing tiles overzoom a parent; each tile scissors to its own
  square.

  **As built (Phase 6):** a source `{"tiles": url}` is a PMTiles archive; a `tiles` template node
  names it and lists layers — `{layer, filter, minzoom, maxzoom, template, merge, labels, priority}`,
  each template seeing the feature as `d` (properties plus `$type`, `$id`, `$x`/`$y`), the signal
  `tile.zoom`, and its geometry as `{"type": "feature"}`. Because cameras move *between* resolved
  scenes, a `tiles` node resolves to an empty placeholder plus a binding, and every frame — settled
  or mid-flight — fills it from the camera it has (`crates/datars-engine/src/tiles`): the tiles
  covering the view at the zoom where a tile spans `tile_size` px, requested as
  `Request::Range {name, url, offset, length}` (header and directories first; `provide_range`, or a
  synchronous `set_range_fetch` for disk and app bundles), decoded once, built once per zoom
  (templates resolved per feature, identical styles batched into one path: ~26k features → ~80
  draw ops for central Stockholm), placed by an affine transform for Mercator (reprojected once per
  tile otherwise) and clipped to their square snapped to whole pixels. Pending tiles draw a cached
  ancestor; tiles outside a regional extract overzoom the nearest ancestor that has data; frames
  report `pending_tiles`; transitions prefetch the tiles along the flight. Label layers are placed
  per frame in screen space, highest priority first, without overlaps; during a flight each frame
  also samples the last 0.3 s, and a label's opacity is the share of samples that placed it, so
  labels fade instead of popping. Live frames are budgeted by counts: 64 KB of tile bytes decoded
  (at least one tile; a tile too big waits for the landing, its ancestor standing in) and 8,000
  map features styled (at least one tile; a tile never styled before shows its nearest styled
  ancestor meanwhile), scaled to the device by the host's work share; the renderer tessellates 24
  meshes and promotes 64 buffers a frame, thin straight borders keep their path's last mesh while
  moving. A square whose finer data arrives fades it in over 0.3 s above what it drew before (its
  ancestor's data, overzoomed) instead of swapping in one frame, so arriving detail doesn't pop. Idle time warms the next step until its tiles are in, decoded and styled. Not yet:
  workers, several archives in one source.
- **Styling is a recipe**, not a separate style language: a function from a tile's feature tables
  (water, landuse, roads by class, buildings, places, boundaries) to primitives, with zoom-dependent
  expressions. The std `basemap` recipe covers light, dark and blueprint looks, labels off and a
  label language. Because it's a recipe, a basemap can be themed with the document's tokens,
  animated (fade the basemap during a lift), hit-tested, and ejected. An importer for a subset of
  MapLibre/Mapbox GL styles can come later for teams with existing styles.
- The label engine is the same one charts use (priorities, collision, halos), so map and chart
  labels never fight.
- **As built:** `@datars/std/basemap` (`packages/std/src/basemap.ts`) draws sea, land, water,
  parks, rivers, buildings, roads by class (with street casings), dashed borders and place labels
  from the tokens `map.water … map.label-halo`, in light and dark. `part: "base"` / `"labels"` split
  it around data layers so names stay on top; `@datars/std/attribution` credits the data outside
  the moving view. It reads our schema (`kind` on roads) and raw OSM `type` tags alike.
  `examples/descent` flies world → Sweden → central Stockholm over `assets/tiles/descent.pmtiles`
  (~0.9 MB, a camera-aware extract: `assets/tiles/descent.job.json`, built by
  `datars-geo-build tiles` with per-band regions, filters, renames, classes and area thresholds).

## Our own tiles: the OSM pipeline

No runtime dependency on third-party map services. Tiles are built by us and served as static files
(any CDN or bucket supports HTTP range requests) or bundled into apps.

```
 sources (build time only)                          datars-geo-build                       outputs
 ─────────────────────────                          ────────────────                       ───────
 OSM planet / regional PBF extracts  ─┐   parse (osmpbf) → tag schema → features            regional archives
 coastline-derived land polygons     ─┼─► multipolygon assembly (even-odd)            ─►     world z0–6 · country z0–12 ·
 Natural Earth (low zooms)           ─┤   robust boolean clipping (geo crate)                 city z12–16  (.pmtiles)
 official boundary sets (e.g. Census)─┘   simplify per zoom (topology-aware)                  figure extracts (camera-aware)
                                          MVT encode · PMTiles write · tile dedup             atlas packages (regions)
```

- **Grown from an earlier tile pipeline:** idempotent downloads with mirror failover; clipping,
  simplification and MVT encoding; a PMTiles reader + writer with Hilbert ids and dedup; MVT
  encode + decode — plus a sans-IO PMTiles reader for the runtime.
  Build-time code may use heavier crates (`geo`, `osmpbf`); none of it ships in the runtime.
- **Incremental and regional:** rebuild a region or a zoom band without rebuilding the world; add
  coverage by adding a source.
- **Camera-aware extraction:** the publish compiler knows every camera target a story visits (and the
  flight paths between them), so a figure's basemap extract contains exactly the tiles along that path
  at the zooms it reaches, plus a margin ([12](12-delivery.md)) — the way a backdrop layer is cut
  to where the cameras go, generalized to the basemap. **As built** (`datars-geo-build`; wired into
  the build as [automatic basemaps](#automatic-basemaps-tiles-auto)): `camera` turns views (a box a camera fits, or a centre and zoom, in a map of
  known size) into the zoom they're drawn at, samples the flights between consecutive views on the
  engine's own van Wijk–Nuij path, and cuts a cover — tile ranges per zoom, plus the whole world at
  z0–1 so nothing outside ever draws as sea; `recipe` fills a cover with the layers std `basemap`
  reads, from Natural Earth, osmdata land polygons (read straight out of the planet shapefile, only
  near the cameras) and OSM extracts, and records the street-level views the local data can't serve.
  `datars-geo-build camera views.json --data … --out x.pmtiles` does both. Extracts come to a few
  hundred KB (about 1 MB with a city's OSM streets and buildings down to z14), and every sampled
  frame of their flights draws from its own tiles, no overzoomed ancestors.
- **Explore mode** (free pan and zoom) points at a shared regional archive instead of a figure extract
  — or, with an automatic basemap, gets a few zoom levels of real data around each explorable view.
- **Apps** bundle archives and memory-map them — maps work fully offline.

### Automatic basemaps (`tiles: "auto"`)

A story about Rio needs Rio's streets; nobody should have to make an archive for it. A tiles source
whose URL is `"auto"` (`data.tiles.auto()` in the SDK) asks the build tools for one:

1. **Views from the document itself.** `Engine::tile_views(samples)` evaluates every program state's
   cameras and 48 frames of each flight between states (consecutive ones and the program's edges) —
   the same walk as the frame pass, computing each placeholder's lon/lat extent and fractional tile
   zoom instead of looking tiles up. Explorable views are flagged; they get up to three extra zoom
   levels around them (within a 6000-tile budget). Nothing is fetched to evaluate cameras.
2. **A cover and the standard recipe** (`datars_geo_build::auto::plan`, `recipe::auto_job`): settled
   views take the zoom below theirs too (a narrower screen than authored) and the one above when
   near it; flight frames take their own. Natural Earth to zoom 9 (osmdata's simplified coastline
   polygons from 7), OpenStreetMap from 10: land assembled from the OSM coastline (`coast`: ways
   joined into chains, cut to each tile range, closed along the box edge counter-clockwise — land is
   on the coastline's left; islands and enclosed water even-odd; the nearest coastline's side, or
   Natural Earth where none is near, decides a box no coastline crosses), water, rivers, parks and
   woods, roads classed per band, buildings from 13, place names in the document's language.
3. **Acquisition through a cache** (`datars_geo_build::fetch`, `overpass`): Natural Earth from its
   GitHub GeoJSON (or a seed directory — `$DATARS_GEO_SEED`), osmdata's
   simplified land polygons (≈ 25 MB zip, once), and OpenStreetMap per *cell* from the Overpass API —
   Web-Mercator tiles at a fixed zoom per detail level (z9 cells for the z10 band, z10 for z11, z12
   for z12, z13 with buildings only), so a nudged camera, a second story in the same city or a
   reader panning reuses what's cached. One request at a time, spaced out, a real User-Agent,
   mirror failover with a 30 s rest for a mirror that answers 429/406, answers checked for
   Overpass's "runtime error" remarks (partial results are never cached), gzip on the wire and on
   disk. The cache (`$DATARS_GEO_CACHE`, else `~/.cache/datars/geo`: `ne/`, `osmdata/`,
   `overpass/<key>.json.gz` + `.query`, `overpass/usage.json`) is keyed by query text, so
   rebuilds are offline and byte-identical; `$DATARS_GEO_OFFLINE=1` forbids the network. HTTP goes
   through `curl`.

   **Whose servers.** The public Overpass instances are shared, donated machines. Their policy:
   under 10,000 queries and 1 GB a day is harmless for one user, a hundredth of that for a regular
   application, "use extracts if you need a lot of data", and commercial use on a self-hosted or
   paid instance. A dense city at street level is a few hundred MB of answers (Rio to Copacabana:
   ~65 cells), fetched once. So the public instances are an author's convenience, within a daily
   budget the cache keeps (`$DATARS_OVERPASS_BUDGET`, default 5,000 queries / 500 MB: half the
   one-user figure); past it, fetching stops with a message. A product, a CI farm or a batch
   import points elsewhere: `$DATARS_OVERPASS_URL` (an instance of our own, used alone and
   without a budget) or `$DATARS_GEO_CELLS` — a URL template (`{level}/{z}/{x}/{y}`) of a static
   server of pre-cut cells, the same answers made from planet or regional extracts on our own
   hardware. The cache doesn't care where a cell came from: its key is the query.
4. **The archive next to the document**, `<source>.auto.pmtiles`, with its job beside it
   (`<source>.auto.job.json`: the lockfile — an archive whose job matches the current plan is used
   as it is). The engine reads an `"auto"` source from that relative URL (`datars_ir::tiles_file`),
   so `datars render/check/film/…`, `datars test`, `datars serve /view/…` and the MCP tools all find
   it; the CLI and MCP rebuild it first when the cameras changed, and `datars serve` builds a missing
   one on its first request. `datars publish` ships it content-addressed
   (`tiles/<source>.<hash>.pmtiles`, the bundle's documents rewritten to point there); `datars
   bundle` writes `<stem>.<source>.pmtiles` next to the `.datars` file. The job and the archive's
   metadata record the OpenStreetMap timestamp (`data_version`).
5. **The dev loop never waits** (`datars dev`): a worker plans the archive, builds it at once from
   what the cache holds (Natural Earth immediately; street cells not fetched yet left out, Natural
   Earth's land standing in for their coast), serves it from memory under a versioned URL, then
   fetches the missing cells one by one and rebuilds every few seconds as they land. The page swaps
   each archive in with `setTilesUrl` (`Engine::set_tiles_url`): the view keeps its state and where
   it was explored to. When a reader pans or zooms an explorable map, the page posts
   `tileViews()` (`Engine::tile_views_now`) to `/__views` and the worker fetches data for those views
   too. Publishing stays deterministic: `datars publish` plans from the document alone.

`datars basemap doc.ts [--build]` shows the plan: the views, tiles per zoom, inputs to fetch, and
whether the archive next to the document is current.

`examples/rio` flies world → Brazil → Rio de Janeiro → Copacabana and Sugarloaf at street level on
an automatic basemap (1.2 MB, 114 tiles). During development, the street-level stories of an
earlier corpus ([15](15-chart-coverage.md): Lisbon, Sydney, Berlin, Botkyrka, Borlänge, the Bay
Area) were made the same way, in a batch, from a cell source fit for it. Not yet: the cell server itself (planet →
cells, weekly), cells shared across levels, OSM admin boundaries (country borders stay Natural
Earth's), a pure-Rust HTTP client. Public mirrors can lag (one served data four months old): the
archive's `data_version` records the range of OSM timestamps it was made from.
- **Schema:** evaluate existing open tile schemas (Shortbread, OpenMapTiles, Protomaps' basemap
  layers) against a minimal schema of our own; the choice affects licensing and style portability
  (open question in [17](17-roadmap.md)).
- **Licensing:** OSM data is ODbL. Rendered maps need attribution (the basemap recipe always renders
  it); distributed tile archives are likely derivative databases with share-alike obligations. Needs a
  legal review before anyone hosts tiles as a service for others.

## Cameras

- A `View`'s camera is `(center, zoom, bearing)` in projected space (pitch later).
- **Interpolation:** the van Wijk–Nuij "smooth and efficient zooming and panning" path is the
  default camera interpolator; linear and custom are alternatives.
- **Targets:** `fit(keys | bbox | geometry, padding)` with core-landmass framing, `center(lon, lat,
  zoom)`, `follow(expr)` (ride along a track head bound to data time — the hurricane example).
- **Explore:** `pan`, `zoom` and `pinch` intents bound to the camera with bounds and zoom limits;
  stepping the story flies back into the narrative.
- The same camera works on cartesian charts (zooming into a dense region of a scatter plot).

## Map features from the examples

| Feature | How |
|---|---|
| Choropleth, pinned colour stops | region recipe + piecewise colour scale + `Paint::Lookup` |
| Categorical fill (`fill palette`) | ordinal colour scale over keys |
| Backdrop (neutral land under sparse data) | a second region layer below, neutral paint |
| Proportional symbols | `Instances` at polylabel centres, area ∝ value (sqrt scale); regions recede |
| Dot density (397k dots) | `scatter_in(region, value / per, seed)` → `Instances`, colour by region LUT, GPU interpolation |
| Custom layers (roads, borders, cities, seats, route ends) | line / point recipes over GeoJSON tables |
| Great-circle routes | geodesic segments |
| Tracks over time | polyline with `trim` bound to data time + head marker; `follow` camera |
| Time-indexed recolour | `interpolate_at(year)` → LUT per frame (cheap: one small table) |
| Map ↔ chart lift | ordinary transition, `disc` path morph, simplified in flight |
| Drill (click a region → sub-story) | parameterized chapter state + `fit` camera + split matcher |
| Indoor and fictional maps | `Coord::Planar` |
| World → street descent | one camera, atlases + `Tiles` with LOD, van Wijk flight |
