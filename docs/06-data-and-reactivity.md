# 06 — Data and reactivity

## Tables

One data model for every chart, map and program: typed, columnar, keyed.

```rust
pub struct Table {
    pub schema: Schema,                 // names, types, nullability, semantic hints
    pub key: KeySpec,                   // column(s) forming the identity, or RowIndex (flagged unstable)
    pub columns: Vec<Column>,
    pub version: Version,               // bumps on every change (live data)
}
pub enum Column {
    F64(Vec<f64>, Nulls), I64(Vec<i64>, Nulls), Bool(BitVec, Nulls),
    Str(DictColumn),                    // dictionary-encoded strings
    Date(Vec<i32>, Nulls), Time(Vec<i64>, TimeUnit, Tz), Duration(Vec<i64>, TimeUnit),
    Geom(GeomColumn),                   // shared coordinate buffer + offsets (points, lines, polygons)
    List(Box<Column>, Offsets),
}
```

- **Arrow-compatible memory layout**, so Arrow IPC and Parquet readers can be zero-copy later without
  making `arrow-rs` a core dependency.
- **Real time types.** Dates are dates, not decimal years; time scales tick on calendar boundaries in
  the document's timezone and locale.
- **Keys are declared and validated.** Duplicate keys are an error; row-index keys work but are
  flagged by the linter, because transitions over them are meaningless (P3).
- **Semantic hints** on columns (`currency: SEK`, `unit: %`, `geo: iso3`, `geo: kommun-code`) drive
  default formats, joins to atlases, and accessibility labels.
- Sources give bytes to the engine (P10): inline, file, URL, host-provided, or a stream. Readers turn
  bytes into tables: CSV (with type inference and an explicit override), JSON (records or columns),
  Arrow IPC, later Parquet.
- **Generated sources** (`data.generate(rows, columns)`) are tables too big to write down —
  simulations, load tests, procedural datasets — as expressions over the row number, evaluated a
  chunk at a time, with `rand(key, stream)` / `randn(key, stream)` for seeded per-row randomness (a
  hash of the arguments: the same rows on every platform). The galaxy example generates 4,000,000
  stars from seventeen expressions; publishing ships them indexed (a point archive), not the
  generator, so nothing is generated on a reader's device.

There are no per-chart data shapes (`(label, value)` rows for bars, `(label, x, y)` points for a
scatter): both are ordinary tables, and queries over them are transforms.

## Transforms

Pure, deterministic graph nodes from tables to tables:

| Group | Transforms |
|---|---|
| Row-wise | `filter`, `derive` (expressions), `select`, `rename`, `cast`, `fold`/`unfold` |
| Grouping | `aggregate` (count, sum, mean, median, quantile, min, max, distinct, first/last) with `group_by`; `top_n` + `other` |
| Binning | numeric (nice bins), time (year, quarter, month, week, weekday, day, hour), text parts (before/after a separator, first word, first letter) |
| Ordering | `sort` (stable; total order for floats, NaN last), `rank`, `window` (cumsum, lag, lead, rolling mean/sum/σ/min/max, EMA, running max/min, first/last per partition, share of total, percent change; `min` rows before a rolling window counts) |
| Reshaping | `pivot`/`unpivot`, `join` (inner, left, by key), `union`, `nest`/`stratify` (hierarchies), `sample` (seeded) |
| Layouts (`datars-algo`) | `stack` (zero, center, normalize, wiggle), `dodge`, `pie`, `treemap`, `pack`, `partition`, `tree`, `sankey`, `chord`, `beeswarm`, `parliament`, `waffle`, `calendar`, `force` (seeded, fixed iterations), `voronoi`, `delaunay`, `contour`, `hexbin`, `density`, `scatter_in` (N points inside a shape), `polylabel` (visual centres), `geodesic`, `label_layout` |
| Time series | `interpolate_at(time)` (values at a data time, for races and recolours), `resample`, `align` |

Determinism details that matter: sums use a fixed order (pairwise with a fixed tree), never a
parallel reduction with scheduling-dependent order; sorting is stable with a total order; sampling
and force layouts are seeded; float formatting is locale-aware and exact.

Users add their own transforms as kernels (JS or WASM) or native plugins through the same public
`Transform` trait the built-ins use (P5):

```rust
pub trait Transform {
    fn schema(&self) -> &ParamSchema;
    fn output_schema(&self, inputs: &[&Schema], params: &Params) -> Result<Schema, Diag>;
    fn run(&self, inputs: &[&Table], params: &Params, cx: &mut RunCx) -> Result<Table, Diag>;
    fn cost_hint(&self, rows: usize) -> Cost { Cost::Linear }   // scheduling, budgets, devtools
}
```

## Reactivity: whole resolves, cached

A document's data side is sources, signals, derived tables, scales, layouts, recipe expansions and
derived values. When a signal or a source changes, the engine **re-resolves the scene** for the new
values, and the caching makes that cheap:

- **Scenes per signal signature.** A resolved scene is kept under its signature — the program's
  state path, the viewport, every signal's value (up to 256 scenes; resolution is deterministic, so
  dropping them only costs time). Returning to a state, a hovered mark or a camera position is a
  lookup; the steps next to the one on screen are resolved ahead in idle time.
- **Shared derived tables.** A derived table whose operations read no signal, layout box, scale or
  host state — all the way down its chain — is a pure function of its inputs and is kept across
  resolves: a scene per story step, layout's measuring passes and every re-resolve reuse it (a
  dot-density table of 400,000 dots is computed once per data it reads). Tables that read signals
  recompute per resolve.
- **Cached expansions.** Recipes the sandbox expands (T3) are cached by recipe, parameters and
  context.
- **Typed at load.** Expressions are parsed and type-checked when the document loads, and slots
  check the columns they receive, so mistakes surface as diagnostics before anything renders.
- **Arrivals are inputs.** Data, fonts and tile bytes arrive through `provide` calls, recorded in
  sessions like any input, so replays are exact. An arrival drops the cached scenes and re-plans a
  transition from what's on screen, so new data animates in; map tiles and point rows arriving
  mid-flight show a coarser level meanwhile and fade in.
- **Cost-aware at publish time.** The publish compiler decides what can be computed at build time
  from what reads what ([12](12-delivery.md)).

`datars-graph` — a finer-grained incremental graph (versioned inputs, demand-driven evaluation in a
deterministic order, recompute counts for the devtools) — is built and tested but not yet wired
into the engine. When it is, a brush that filters could re-run only the tables and layouts that
depend on it rather than re-resolving the scene.

## Signals

Typed reactive inputs:

```rust
pub enum SignalValue {
    Num(f64), Bool(bool), Str(Sym), Key(Option<KeyPath>), KeySet(KeySet),
    Range(Value, Value), Point(Vec2), Enum(Sym), Time(i64), Json(JsonValue),
}
```

| Built-in signal | Set by |
|---|---|
| `viewport` (size, pixel ratio, size class, orientation) | host |
| `pointer`, `inspected`, `focused` | engine, from intents |
| `step`, `state`, `timeline` | the program |
| `prefers_reduced_motion`, `contrast`, `color_scheme`, `locale` | host (platform settings) |
| `<source>.version` | data arrivals |

User signals: controls (a slider, a toggle, a segmented choice — engine-drawn `std` controls or host
widgets bound through `set_signal`), selections (`selected: KeySet`), brushes (`range`), anything
derived.

Because signals are global to a document, linked views, cross-filtering and "hover here, highlight
there" need no extra machinery.

## Expressions

The per-element, per-frame language: pure, total, typed, small.

- **Syntax:** a JavaScript-like subset (`d.share > 30 ? palette.accent : "#ccc"`, `max(d.a, d.b)`,
  `format(d.value, ".1%")`, `selected.has(d.party)`, `scale.y(d.value)`), so TypeScript lambdas can
  compile to it ([07](07-extensibility.md)).
- **Total:** division by zero is NaN, a missing field is null, every function is defined on every
  input. Expressions can't loop, allocate unboundedly or panic.
- **Typed:** checked against the table schema and signal types at load time.
- **Compiled:** to a register bytecode that runs vectorized over columns (one dispatch per column
  chunk, not per row), and optionally to WGSL so per-instance channels are computed on the GPU.
- **Where they're used:** channel encodings, filters and derives, conditional structure (`when`),
  choreography windows, custom easings, route parameters, label templates, accessibility labels.

Scales and signals are ordinary values in the language, not a hardwired evaluation context (a fixed
`y_domain`/`y_range` beside numbers, strings, colours and a few calls), so any expression can read
any scale or signal.

## Live data

A live source emits versions; each version is a signal change; the graph recomputes; the scene
changes; the motion system plans a transition from whatever is on screen (retargeting, with
velocity). One mechanism, the same one steps and filters use.

| Concern | Mechanism |
|---|---|
| Transport | the host: poll, SSE, WebSocket, or a native socket; the engine only sees versions (P10) |
| Update kinds | `snapshot` (keys absent now exit), `upsert`/`delete` by key, `append` |
| Retention | window by count or duration on the source; old rows exit with animation |
| Scale domains | per-scale policy: `fixed`, `grow` (animated rescale), `window`, `nice-steps` (rescale only at nice boundaries, so axes don't twitch) |
| Rate | coalesce to one version per frame; cap transition duration to the update interval; drop intermediate versions under load |
| Replay | a recorded stream is a file of versions; a static build can play an election night |

A production data plane (connectors, schedules, fan-out to many viewers) is a hosted service built
on top ([16](16-licensing.md)); the protocol it speaks is public.
