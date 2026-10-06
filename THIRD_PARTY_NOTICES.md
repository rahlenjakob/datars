# Third-party notices

datars's own code is proprietary, © 2026 Jakob Råhlén, all rights reserved (see
[LICENSE](LICENSE)). This repository also contains,
or builds into its binaries, material from third parties under their own licences. That material
is listed below.

Items marked **to confirm** could not be fully verified from the repository itself. The
maintainers should check them before a release.

## Fonts

All bundled fonts are under the [SIL Open Font License 1.1](https://openfontlicense.org). Each
licence file is next to its fonts. The publish compiler keeps the copyright and licence name
records (IDs 0, 13 and 14) in every subset it ships, and records each font's licence in the bundle
manifest.

| Font | Files | Copyright | Licence |
|---|---|---|---|
| Inter 4.000 | `fonts/Inter-{Regular,SemiBold,Bold}.ttf` (subsets), `fonts/source/Inter-*.ttf` | © 2016 The Inter Project Authors (<https://github.com/rsms/inter>) | OFL 1.1, `fonts/Inter-LICENSE.txt` |
| Newsreader 1.003 | `assets/fonts/Newsreader-{Regular,SemiBold}.ttf` | © 2020 The Newsreader Project Authors (<https://github.com/productiontype/Newsreader>) | OFL 1.1, `assets/fonts/Newsreader-OFL.txt` |
| Manrope 4.504 | `site/fonts/Manrope-{Regular,Bold}.ttf` (the website's "Make it yours" type) | © 2019 The Manrope Project Authors (<https://github.com/sharanda/manrope>) | OFL 1.1, `site/fonts/Manrope-OFL.txt` |
| Space Mono 1.003 | `site/fonts/SpaceMono-{Regular,Bold}.ttf` (the same) | © 2016 The Space Mono Project Authors (<https://github.com/googlefonts/spacemono>) | OFL 1.1, `site/fonts/SpaceMono-OFL.txt` |
| Fraunces 1.000 | `site/fonts/Fraunces-Bold.ttf` (the same) | © 2020 The Fraunces Project Authors (<https://github.com/undercasetype/Fraunces>) | OFL 1.1, `site/fonts/Fraunces-OFL.txt` |
| Noto Sans Hebrew 3.001 | `assets/fonts/NotoSansHebrew-Regular.ttf` | © 2022–2024 The Noto Project Authors (<https://github.com/notofonts/hebrew>) | OFL 1.1, `assets/fonts/NotoSansHebrew-OFL.txt` |

Inter is also compiled into the native libraries and the CLI (the `bundled-fonts` feature of
`datars-text`). It is copied with its licence into the web runtime (`packages/web/dist/fonts/`), the
Python package and the site build.

The showcase theming demo (`site/charts/brand.ts`) names Google Fonts families (Manrope, Space
Mono, Fraunces). They are downloaded at build time into a local cache and are not in this
repository. They are published under the OFL (**to confirm** per family), and the subsets that
ship in the site's bundles carry their licence records.

## Code

### QuickJS (compiled in, not vendored)

The recipe sandbox (`crates/datars-sandbox`) embeds **QuickJS-ng** through the
[`rquickjs`](https://crates.io/crates/rquickjs) crate. Its C sources come from the `rquickjs-sys`
0.9.0 crate at build time, and they are compiled into the CLI, the native libraries and the web
runtime's `datars_host_web` engine.

> The MIT License (MIT). Copyright (c) 2017-2021 Fabrice Bellard, Copyright (c) 2017-2021 Charlie
> Gordon, Copyright (c) 2023 Ben Noordhuis, Copyright (c) 2023 Saúl Ibarra Corretgé.

### Ported or adapted algorithms

These source files say they are ports of, or closely follow, third-party code. Keep the upstream
notices with them.

| Where | Upstream | Licence |
|---|---|---|
| `crates/datars-geo/src/earcut.rs` ("a faithful port of mapbox/earcut v2.2.x") | [earcut](https://github.com/mapbox/earcut), © Mapbox | ISC |
| `crates/datars-algo/src/delaunay.rs` ("a port of Delaunator's sweep-hull algorithm") | [Delaunator](https://github.com/mapbox/delaunator), © Mapbox | ISC |
| `crates/datars-algo/src/polylabel.rs` (Mapbox's polylabel) | [polylabel](https://github.com/mapbox/polylabel), © Mapbox | ISC |
| `crates/datars-geo/src/measure.rs`, `sphere.rs`, `clip/rejoin.rs`, `clip/sphere.rs`, `clip/rect.rs`, `project.rs`, `projection.rs` (ported from, or following, d3-geo) | [d3-geo](https://github.com/d3/d3-geo), © Mike Bostock | ISC. d3-geo's licence file also carries an MIT notice for GeographicLib (© Charles Karney). Whether any ported part derives from it is **to confirm** |
| `crates/datars-algo/src/pack.rs` (the front-chain packer and enclosing circle of d3-hierarchy), `tree.rs` (as in d3.tree / d3.cluster) | [d3-hierarchy](https://github.com/d3/d3-hierarchy), © Mike Bostock | ISC |
| `crates/datars-algo/src/force.rs` (the d3-force model) | [d3-force](https://github.com/d3/d3-force), © Mike Bostock | ISC |
| `crates/datars-algo/src/sankey.rs` ("after d3-sankey") | [d3-sankey](https://github.com/d3/d3-sankey), © Mike Bostock | BSD-3-Clause (**to confirm**) |
| `crates/datars-algo/src/hexbin.rs` (the d3-hexbin lattice) | [d3-hexbin](https://github.com/d3/d3-hexbin), © Mike Bostock | BSD-3-Clause (**to confirm**) |
| `crates/datars-algo/src/ticks.rs`, `crates/datars-data/src/scale/ticks.rs`, `crates/datars-data/src/num.rs` (d3's tick algorithm) | [d3-array](https://github.com/d3/d3-array) / [d3-scale](https://github.com/d3/d3-scale), © Mike Bostock | ISC |
| `crates/datars-text/src/format.rs` (d3-format's specifier language and semantics), `crates/datars-text/src/locale.rs` (values from d3-format / d3-time-format locale definitions, and CLDR where d3 has none) | [d3-format](https://github.com/d3/d3-format), [d3-time-format](https://github.com/d3/d3-time-format), © Mike Bostock; [Unicode CLDR](https://cldr.unicode.org) | ISC; CLDR data under the Unicode License v3 |
| `crates/datars-algo/src/predicates.rs` (Shewchuk's adaptive-precision predicates) | Jonathan Richard Shewchuk, *Adaptive Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates* | Public domain (reference code). Whether the code is a port or a reimplementation is **to confirm** |

Several modules implement algorithms from the literature without copying code. Examples are van
Wijk–Nuij camera paths, squarified treemaps (Bruls, Huizing & van Wijk), Machado et al. (2009)
colour-vision simulation, Douglas–Peucker and Visvalingam–Whyatt simplification, Liang–Barsky
clipping and marching squares. No licence applies to these.

### Dependencies

Rust crates (see `Cargo.lock`) and npm packages (see `pnpm-lock.yaml`) are not vendored in this
repository. They are fetched by Cargo and pnpm under their own licences. At the time of writing,
every Rust dependency is under a permissive licence (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib,
Unlicense, CC0 or Unicode-3.0, alone or as alternatives).

The web runtime links **wasi-libc** statically (`scripts/build-wasm.sh`) so that QuickJS has a C
library. wasi-libc is under Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, and includes
musl (MIT) and other permissively licensed parts. The exact notices are **to confirm**.

**TODO(maintainers):** binary distributions (the CLI, `@datars/web`'s wasm, the Swift and Android
libraries, Python wheels) should carry the notices of the crates compiled into them, for example
generated with `cargo about` or `cargo deny`.

## Geographic data

### Natural Earth: public domain

[Natural Earth](https://www.naturalearthdata.com) vector data is in the public domain. No
permission is needed; credit is appreciated.

- `assets/atlas/countries.geojson`: the built-in `countries` atlas (242 countries keyed by
  `ADM0_A3`, simplified with `datars-geo-build atlas`). It is copied into the web runtime and the
  Python package. The source is Natural Earth Admin 0 Countries. The 242 features and `ADM0_A3`
  ids match the 1:50m edition, but the scale is **to confirm**.
- `crates/datars-geo/tests/fixtures/ne_countries_subset.geojson`: a Natural Earth test fixture.
- `site/articles/swedish-election-2022/geography/admin1.geojson`,
  `site/articles/us-election-2024/{electoral,margins}/admin1.geojson`: Natural Earth
  `ne_10m_admin_1`, simplified.
- Natural Earth layers inside the tile archives below.

### OpenStreetMap: © OpenStreetMap contributors, ODbL 1.0

These vector-tile archives were cut by `datars-geo-build` from OpenStreetMap data, including
extracts fetched through the Overpass API and osmdata.openstreetmap.de land and coastline
polygons, which are derived from OpenStreetMap. They also contain Natural Earth layers.

| Archive | Built from |
|---|---|
| `assets/tiles/descent.pmtiles` | `assets/tiles/descent.job.json` |
| `examples/rio/basemap.auto.pmtiles` | `examples/rio/basemap.auto.job.json` (automatic basemap) |
| `site/articles/marathon/cities/basemap.pmtiles` | article basemap |
| `site/articles/zodiac/sites/basemap.pmtiles` | article basemap |

OpenStreetMap data is © OpenStreetMap contributors and available under the
[Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/)
(<https://www.openstreetmap.org/copyright>). The archives are databases derived from it: the
licence of datars's own code does not apply to them, and they are distributed under
the ODbL with the attribution "© OpenStreetMap contributors, Natural Earth". Maps drawn from them
must show that credit. The standard library's `attribution()` recipe draws it, and `datars lint`
warns when it's missing. As [docs/09-geo.md](docs/09-geo.md) notes, the exact share-alike
obligations for distributed archives still need a legal review (**to confirm**).

### Other map geometry in the articles

| File(s) | Source | Licence |
|---|---|---|
| `site/articles/us-election-2024/{county-map,county-lead}/counties.geojson` | Plotly's U.S. counties GeoJSON ([plotly/datasets](https://github.com/plotly/datasets)), after U.S. Census Bureau cartographic boundary files | Census data: public domain (U.S. government work). plotly/datasets repository licence: MIT (**to confirm**) |
| `site/articles/us-election-2024/{county-map,county-lead}/states.geojson` | U.S. state outlines | **to confirm** (likely Natural Earth admin-1 or dissolved from the county shapes) |
| `site/articles/*/cities.geojson`, `site/articles/zodiac/sites/sites.geojson` | Point positions placed for the articles from public descriptions | facts, no third-party licence (**to confirm**) |
| `site/articles/westeros/*/*.geojson` | Drawn by hand for the article: approximate, not an official map | original work. *Game of Thrones* is HBO's and *A Song of Ice and Fire* is George R. R. Martin's; names are used descriptively |

## Data in the articles (`site/articles`)

Each article names its sources under its figures and in its "About the data" notes. Articles with
data files document them in a `SOURCES.md`.

| Article | Data | Source | Terms |
|---|---|---|---|
| `oil-2020` | `data/*.csv`, `data.json` | U.S. Energy Information Administration (EIA), via FRED (Federal Reserve Bank of St. Louis). See `oil-2020/SOURCES.md` | Public domain (U.S. government work) |
| `us-debt` | `data/debt_to_penny.json`, `data.json`, live API | U.S. Treasury, Bureau of the Fiscal Service, *Debt to the Penny*. See `us-debt/SOURCES.md` | Public domain (U.S. government work) |
| `warming` | `co2/co2_annmean_mlo.csv` | NOAA Global Monitoring Laboratory, Mauna Loa annual mean CO₂ (1958–1974 data from C. D. Keeling, Scripps) | Freely available; NOAA GML asks for citation (see the file header) |
| `warming` | `ice/N_09_extent_v4.0.csv` | National Snow and Ice Data Center, *Sea Ice Index*, version 4 | Free to use with citation (**to confirm** NSIDC's citation text) |
| `warming` | temperatures (in the stories' `doc.ts`) | NASA GISS, GISTEMP v4 | Public domain (U.S. government work) |
| `warming` | emissions (in `emitters/doc.ts`, rounded) | Global Carbon Project, *Global Carbon Budget 2023* | CC BY 4.0 (**to confirm**) |
| `greece-debt-crisis` | `data/eurostat.json` | Eurostat (`gov_10dd_edpt1`, `irt_lt_mcby_a`, `nama_10_gdp`, `une_rt_a`) | Eurostat's reuse policy: free reuse with acknowledgement (**to confirm**; generally CC BY 4.0) |
| `us-election-2024` | `data/county_margin.json`, `county_lead.json`, `keys.json` | [tonmcg/US_County_Level_Election_Results_08-24](https://github.com/tonmcg/US_County_Level_Election_Results_08-24), compiled from official county returns | **to confirm**: the repository's licence and permission to redistribute the derived files |
| `swedish-election-2022` | results in `doc.ts` | Swedish Election Authority (Valmyndigheten), final results; county sums from its constituency counts as tabulated on Wikipedia | Official election results (facts). Wikipedia text is CC BY-SA 4.0 (**to confirm** nothing beyond facts was taken) |
| `marathon` | record times in `doc.ts` | World Athletics, marathon world record progression | Facts (**to confirm**) |
| `covid-lockdowns` | dates and populations in `doc.ts` | Stay-at-home order dates as reported at the time; approximate 2020 populations | Facts |
| `zodiac` | dates and sites in `doc.ts` / `sites.geojson` | Police records as widely reported; sites placed from public descriptions | Facts |
| `westeros` | audiences in `doc.ts` | Nielsen figures as widely reported | Facts |

## Sample data in `examples/`

Example documents use small, rounded or illustrative figures that are typed into their `doc.ts`.
Each file's header comment says where a figure's shape comes from, for example Valmyndigheten
(Swedish election results), NASA GISTEMP, Eurostat/IRENA or Israel's Central Bureau of
Statistics. The showcase charts in `site/charts/` work the same way (for example IMF WEO GDP,
rounded). Others use synthetic, seeded data (`galaxy`, `scatter`, `stocks`, `prices`) or
simulated snapshots (`election`). None of them redistributes a third-party dataset. The map
examples use the Natural Earth atlas and the OpenStreetMap-derived archives listed above.
