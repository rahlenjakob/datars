# 15 — Chart coverage: every chart in the corpus, on the new foundation

The design has to carry a real body of work, not a wish list. The yardstick is a corpus of data
stories written for an earlier prototype — 15 articles, **115 stories**, 153 story files in its
text DSL and a handful of JavaScript components — and this is the inventory of how each chart and
feature in it maps onto datars primitives, algorithms and std recipes. The corpus itself lives
outside this repository and was used during development ([the coverage gate](#the-coverage-gate)).
The article pages around the charts (Markdown pages, figure and scrolly directives, caption-card
designs and page themes) were demo scaffolding and are out of scope; the charts, maps, motion and
interactions inside them are in scope.

Counts are scene-level uses across all stories. Where a feature is quoted in code (`fill palette`,
`backdrop [source]`), that is the corpus's own DSL, kept to name the feature precisely.

## Chart types

| Chart | Uses | std recipe | Primitives | Algorithm (`datars-algo`) | Keys | Motion defaults / notes |
|---|---|---|---|---|---|---|
| `bars` | 29 | `bar` (vertical) | `Shape::Rect` or `Instances(Rect)`; band + linear scales | — | category | grow from baseline; value labels count (`Content::Number`) |
| `hbars` (+ race) | 36 | `bar` (horizontal) + `rank` | as above | `rank`, `top_n`, `interpolate_at(time)` | category | race: states per data-time step with position swaps, or per-frame rank from interpolated values for continuous races |
| `dots` | 3 | `dot` | `Instances(Circle)` | — | category | pop in |
| `pie` | 3 | `arc` (inner 0) in polar coord | `Shape::Arc` | `pie` (angles, sort, pad) | category | sweep from neighbours; outside labels with leaders, **legend fallback when labels don't fit** (resolve-time text measurement + `cx.when`) |
| `donut` | 8 | `arc` (inner > 0) | `Shape::Arc` | `pie` | category | as pie; centre total as `Text` |
| `stack` (one 100 % bar) | 4 | `bar` + `stack(normalize)` | `Shape::Rect` | `stack` | category | segments slide |
| `parliament` | 7 | `hemicycle` | `Instances(Circle)` | `parliament` (seat rows on arcs, party order) | `(party, Unit(i))` | `by_hierarchy` split: a party's bar shatters into its seats |
| `treemap` | 5 | `treemap` | `Shape::Rect` + fitted `Text` | `treemap` (squarified) | category | labels shrink/hide by priority |
| `waffle` | 4 | `waffle` | `Instances(Rect)` | `waffle` (10 × 10 units) | `(key, Unit(i))` | split from bars/slices; wave presets |
| `grouped` | 6 | `bar` + `dodge` | `Shape::Rect` | `dodge` | `(series, x)` | same keys as heatmap cells and stacked segments, so the three morph |
| `beeswarm` | 2 | `swarm` | `Instances(Circle)` | `beeswarm` (windowed neighbour search, ported: 20k dots in ~1 s) | row key | stagger by value |
| `heatmap` | 3 | `cell` | `Instances(Rect)` + colour scale | — | `(series, x)` | colour interpolates in OKLab |
| `calendar` | 1 | `calendar` | `Instances(Rect)` | `calendar` (week × weekday per year) | `(year, day)` | morphs with heatmap and the `spiral` component |
| `stripes` | 1 | `stripe` | `Shape::Rect` full height | — | ordered x | diverging colour around zero |
| `sankey` | 2 | `sankey` | node `Rect`s + link `Ribbon`s | `sankey` (columns by depth, ribbons by flow) | link `(source, target)` | bars of the same rows morph into ribbons |
| `stacked` (+ `percent`) | 3 | `bar` + `stack` | `Shape::Rect` | `stack(zero \| normalize)` | `(series, x)` | segments restack |
| `waterfall` | 1 | `waterfall` | `Shape::Rect` + connector segments | `waterfall` (running totals, `=` subtotals) | category | bars rise into place |
| `funnel` | 1 | `funnel` | centred `Shape::Rect` + conversion `Text` | derive (conversion from previous stage) | stage | hbars narrow into it |
| `line` (+ reveal, facet, bands) | 36 | `line` | `Shape::Polyline` (curves), `trim` for reveal | — | series | draw-on via `trim` clip; position-wise vertex interpolation |
| `area` (+ `stacked`) | 9 | `area` | `Shape::Area` | `stack` | series | data-level interpolation of stacks (no sliding vertices) |
| `points` / `mark point` (scatter) | 4 | `point` | `Instances(Circle \| Symbol)` | — | row key | 100k points with GPU interpolation; staggered reveal by x |
| `mark hbar/donut { value: expr }` | 2 | any recipe with channel expressions | — | — | — | expressions reading an input signal (`value * price / 100`) |

## Custom components (become example user packages)

| Component | What it does | What it needs from the API |
|---|---|---|
| `seasons` | every month of every year as a dot, coloured by year | `Instances`, sequential colour scale, keys shared with `calendar`/`heatmap` |
| `spiral` | daily values wound into a spiral, one turn per year | polar geometry (`Shape::Arc` per day), diverging scale, keys `(year, day)` |
| `bubbles` | packed circles | `pack` transform |
| `petals` | petal/rose shapes per category | `Shape::Path` builders |
| `cloud` | seeded clusters of dots | seeded `random`, `Instances` |
| `formation` | dots form a word or a silhouette from a bitmap; same keys fly to new letters | `scatter_in` / bitmap-to-points, `nearest` or keyed matching |

## Maps

| Feature | Uses | How (details in [09](09-geo.md)) |
|---|---|---|
| Choropleth (`map … values`) | 104 map scenes | `region` recipe: keyed `Path`s in `Coord::Geo`, colour scale, `Paint::Lookup` |
| Pinned colour stops (`fill #a 2 #b 4 #c 6.5`) | many | piecewise colour scale |
| Categorical fill (`fill palette`) | ~15 | ordinal colour scale over keys |
| Backdrop (`backdrop [source]`) | ~50 | a neutral region layer beneath (same source's undata'd features, or another source) |
| Proportional symbols (`symbols N`) | ~12 | `Instances` at polylabel centres, sqrt area scale |
| Dot density (`dot-density N size N`) | 2 (397k dots) | `scatter_in` → `Instances`, per-region colour LUT (GPU interpolation planned) |
| Custom geographies (`geo X { file … }`) | 31 | GeoJSON → keyed tables; triangulated per zoom band at build |
| Overlay layers (`layer roads/states/cities/seats/ends/walk/course`) | 45 | line and point recipes; non-scaling strokes; labels via the label engine |
| Great-circle routes | std map recipes | geodesic segments |
| Tracks over time (`track`, `tracks`) | 5 | polyline with `trim` bound to data time + head marker |
| Vector basemap (`basemap { theme, lang, labels }`) | 8 | `Tiles` + std `basemap` style recipe on our OSM tiles |
| Cameras: `bbox` 68 · `center … zoom` 18 · `region` 15 · `follow` 2 | 103 | `View` camera targets `fit`, `center`, `follow`; van Wijk flights |
| Chart cameras (`camera { x … y … }`) | 11 | the same `View` camera over cartesian coordinates |
| Explore (free pan/zoom) | 2 | `pan`/`zoom` intents bound to the camera |
| Time-indexed recolour (`time`, `timeline`) | 11 timelines | `interpolate_at(year)` → LUT per frame |
| Map ↔ chart lift | field guide, US election | ordinary transition, `disc` morph |
| Drill (`click chapter`) | 8 | parameterized chapter state + `fit` + split matcher |
| Indoor floor plan (airport terminal), fictional map (Westeros) | 2 articles | `Coord::Planar` (no more fake lon/lat) |
| Multi-level regions (countries → admin1 → counties/kommuner) | several | atlas packages; `by_parent` split/merge on zoom |

## Features and interactions

| Feature | Uses | How |
|---|---|---|
| Annotations (`annotation at … { text, dx/dy, arrow line\|curve\|elbow, head, dot }`) | 83 | std `annotate`: anchors to a key, data point or lon/lat; connectors as `Segment`/`Path` with markers; collision-aware placement; enters/exits with states |
| Narration (`caption`) | 247 | step narration + anchor (`at "SD"`) exposed to the host; card designs are host concerns. Inline `[words](key)` → key-coloured text spans |
| Tooltips (`tooltip { text, zero, unit, prefix, decimals, scale }`) | 99 | std `tooltip`: engine-drawn by default (same on every platform), templates with number formats; host-rendered via anchors optionally |
| Highlight | 34 | a `KeySet` signal per state; others recede through a frame-time expression |
| Legends (`legend on\|off`) | 17 | std legend recipes (swatches, colour ramp, size), defaulted by chart recipes |
| Display names (`labels { … }`) and colours (`palette { … }`) | ~40 | a key-metadata table (name, colour per key) feeding ordinal scales and formatters |
| Reference lines and bands (`rule y 100 "Target"`, `band x 2008 2012`) | 11 | std `rule` / `span` annotations in data coordinates |
| Uncertainty / fan charts (`band from lo hi`, `band N`) | 5 | `area` between series; overlapping opacity |
| Overlays (`overlay pareto`, `overlay line … axis right`) | 2 | `window` transforms (cumulative share, rolling mean) + a second y scale with a right axis |
| Small multiples (`facet N`) | 1 | std `facet`: grid of `View`s, shared or free scales; keys carry the facet so single → faceted morphs |
| Number formats (`format compact`, `prefix`, `percent`), `locale sv` | 6 + | `datars-text` formatting with locale data; scale-driven tick formats |
| `sort`, `limit`, `only`, top N | ~20 | transforms |
| Tables and queries (`table … { … }`) | 4 | typed tables + transforms (group by, time bins, split into series, aggregates, filters, top N + Other) |
| Timelines with dates (`timeline … dates`) | 6 | data-time signal with real date types |
| Reveals (`reveal … { duration, easing }`) | 10 | `trim` / clip clips |
| Transition styles (`style cascade/elbow/wave/spiral/drop/settle/phased/arc/scatter/breathe/explode/fade`, `order`, `stagger`, `enter/exit`) | 22 | std motion presets ([05](05-time-and-motion.md) table) |
| Unit splits (`K#i` keys) | waffle, parliament | hierarchical keys + `by_hierarchy` matcher with partitions |
| For-each scenes (`for c in … top N`) | 3 | TypeScript loops generating states |
| Chapters | 6 | nested parameterized states |
| Live data (`live … every N ticks N`) | 1 | live source + snapshot versions + retargeting; recorded replay |
| Reader inputs (`input price slider …`) | 1 | a signal + engine-drawn slider (or host control) |
| Accessible data table (`data-table visible`) | 1 (always generated) | generated from semantics on every platform |
| Drivers: `click` 76 · `autoplay` 28 · `scroll` 9 · `explore` 2 | 115 | story program drivers (`steps`, `autoplay`, `scroll` scrub/trigger, camera intents) |
| Themes (11) | demo | tokens in std themes; the page CSS parts are out of scope |
| Posters, video (`--format vertical`) | all | static and film profiles |

## Beyond the corpus: financial charts

The corpus had no market data; finance users expect TradingView / Bloomberg / FT-style
charts. They are std recipes over generic engine pieces (window ops, a gap-free date axis), shown
end to end in `examples/stocks`.

| Chart | std recipe | Primitives | Data ops | Keys | Motion / notes |
|---|---|---|---|---|---|
| Candlesticks (hollow option) | `candlestick` in `plot` | `Shape::Segment` wick + `Shape::Rect` body | `window lag` (previous close, for the tooltip's change) | row (ticker, date) | grow from their middle; a range change (1Y → 1M) morphs every day that stays |
| OHLC bars | `ohlc` | `Shape::Path` (high–low, open tick left, close tick right) | as candles | row | — |
| Volume under the price | `volume` → the plot's `lower` pane | `Shape::Rect` on a second value scale over a box below the plot area; its axis under the price axis | — | row | grow from the base |
| Trading-day time axis | `plot({ xType: "band" })` over a date field | band scale whose slots are dates | — | tick value | calendar ticks at the first session of each week / month / quarter / year |
| Moving averages (SMA, EMA) | `movingAverage` | `line` | `window rolling_mean` / `ema` (`min` = full windows), over `source` then `join` to the rows on show | series | draw on; named in the plot's indicator key |
| Bollinger bands | `bollinger` | `area` between the edges + `line`s | `rolling_mean`, `rolling_std` (population σ), `derive` | series | as above |
| Indexed comparison (rebased to 100 / % change) | `indexed` | `line`s + baseline `rule` + end labels | `window first` per series, `derive`; `spread` for labels | series | adding a ticker draws its line on; others rescale |
| Drawdown | `drawdown` | `area` from zero / `line`s + end labels | `window cummax`, `derive` | series | — |
| Sparklines (tables, cards) | `sparkline` | `polyline` + soft `area` + last-value dot on its own scales | `group.first/last` (trend colour) | group | draw on |
| Log price axis | `plot({ yType: "log" })` | log scale, fitted to the prices (not whole decades), 1-2-5 labels | — | — | — |

Up and down colours are theme tokens (`up`, `down`, defaulting to `positive` / `negative`), so a
theme can swap them for markets that read red as up. Hovering a candle reads its date, OHLC and
change (the datum's label, which hosts show as the tooltip); a crosshair that follows the pointer
needs a pointer-position signal the engine doesn't expose yet.

## The coverage gate

During development, coverage was checked against the corpus rather than asserted. The corpus and
its importer live outside this repository and aren't part of its test suite:

1. **An importer** parsed each story with the prototype's own parser and emitted a datars document
   (TypeScript using `@datars/std`) — throwaway tooling that ported the corpus mechanically.
2. **The 115 stories** rendered through datars with the chart, map and motion features listed
   above, compared against the prototype's own frames (the porting oracle, [13](13-testing.md)).
3. **Zero chart vocabulary in engine crates** (P4) held while they did — the evidence that the
   primitives are the right ones.
4. **The components** ran as a user package with no engine changes; six of them are in this
   repository as `site/articles/recipes/components.js`.

The evidence you can run here: the documents in `examples/`, checked by `datars test` (visual and
motion goldens, motion invariants, and legibility checks on every state), the standard library's
tests (`node --test packages/std/test/*.test.js`), and `cargo test --workspace`. The P4 rule holds
for the engine crates in this repository.
