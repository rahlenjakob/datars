> **Historical:** the crate API contracts written to build the first phases in parallel. The
> crates have moved on; their current APIs are in the code (`cargo doc --open`) and what's built is
> in [19-status](../19-status.md).

# Crate contracts

The public API each crate must expose, so crates can be built in parallel and integrated without
guesswork. A crate may add more; it must not rename or drop these without updating this file.
Types from lower crates are used as-is (`datars_scene::*`, `datars_math::*`, `datars_theme::Ink`).

## datars-math, datars-color, datars-theme, datars-scene, datars-render (done)

See their `src/lib.rs`. Key types: `Vec2`, `Affine`, `Rect`, `PathData`, `Rng`, `Hash64`;
`Color`; `Theme`, `ResolvedTheme`, `Ink`, `Mode`; `Scene`, `Node`, `NodeKind`, `Geom`, `Paint`,
`Stroke`, `TextNode`, `TextRun`, `TextStyle`, `Instances`, `Key`, `KeyPart`, `KeyPath`, `Semantics`,
`Role`, `Camera`; `DisplayList`, `Op`, `DPaint`, `InstancesOp`, `GlyphSource`, `flatten`, `trim_path`.

## datars-text

```rust
pub struct FontDb;                                   // font faces by family + weight + style
impl FontDb {
    pub fn empty() -> FontDb;                        // no faces: everything comes from the host
    pub fn builtin() -> FontDb;                      // + Inter 400/600/700 with the `bundled-fonts` feature
    pub fn add_font(&mut self, bytes: Vec<u8>) -> Result<Vec<String>, TextError>;   // returns face ids
    pub fn add_face(&mut self, bytes: Vec<u8>, family: &str, weight: u16, italic: bool) -> Result<Vec<String>, TextError>; // as a theme token names it
    pub fn add_face_part(&mut self, bytes: Vec<u8>, family: &str, weight: u16, italic: bool, part: &str) -> Result<Vec<String>, TextError>; // a lazily loaded script subset
    pub fn face_id(&self, family: &str, weight: u16, italic: bool) -> Option<String>; // best match, falls back to Inter, then the first face
    pub fn face_data(&self, id: &str) -> Option<(&[u8], u32)>;  // the font file (the publish compiler subsets it)
    pub fn missing_chars(&self) -> Vec<char>;        // laid out with no glyph (runtimes fetch script subsets)
}
pub fn bundled_file(name: &str) -> Option<&'static [u8]>;   // `datars:fonts/<name>` with `bundled-fonts`
impl datars_render::GlyphSource for FontDb;        // outlines (y down, baseline origin, px)
pub fn layout(db: &FontDb, node: &mut datars_scene::TextNode);    // fill runs + bounds from text + style
pub fn measure(db: &FontDb, text: &str, style: &datars_scene::text::TextStyle) -> datars_math::Rect;
pub mod format {
    pub fn number(v: f64, spec: &str, locale: &str) -> String;   // d3-format-like specs: ",.1f", ".0%", "$,.2s", "+.1f", "~s"
    pub fn date(days_since_epoch: i64, spec: &str, locale: &str) -> String; // "%Y", "%b %Y", "%-d %b", "%A"
}
pub mod locale { pub struct Locale; pub fn get(tag: &str) -> &'static Locale; } // at least en, sv, de, fr, es (+ decimal/group separators, month/day names)
```

Face ids are `"<Family>-<Weight>[-Italic]"` (e.g. `"Inter-400"`). `TextRun.font` holds a face id.
Layout: `style.max_width` wraps (UAX #14 line breaking), `align`/`baseline` position lines relative
to `origin`, bidi runs (UAX #9) are ordered visually. Deterministic: shaping and layout depend only
on inputs and the faces provided (bundles carry every face they draw with, docs/12-delivery.md).

## datars-render-cpu (and datars-render-svg)

```rust
pub struct Pixmap { pub width: u32, pub height: u32, pub data: Vec<u8> }   // RGBA8, straight alpha
impl Pixmap { pub fn to_png(&self) -> Vec<u8>; pub fn hash(&self) -> u64; pub fn pixel(&self, x: u32, y: u32) -> [u8; 4]; }
pub fn render(list: &DisplayList, glyphs: &dyn GlyphSource, dpr: f64) -> Pixmap;
// datars-render-svg
pub fn to_svg(list: &DisplayList, glyphs: &dyn GlyphSource) -> String;
```

The CPU rasterizer is the determinism reference: integer/fixed-point coverage accumulation, bit-exact
on every target. Supports fills (non-zero, even-odd), strokes (width, caps, joins, miter limit, dashes),
linear/radial gradients, clips (nested), layers (opacity), instances, glyphs (via `GlyphSource`).

## datars-expr

```rust
pub enum Expr { … }                                   // serde (JSON AST), Display = source form
pub fn parse(src: &str) -> Result<Expr, ParseError>;  // JS-like subset; accepts `d => …` / `(d) => …`
pub enum Value { Null, Num(f64), Str(Arc<str>), Bool(bool) }
pub enum Type { Num, Str, Bool, Any }
pub trait Env {                                        // what an expression can see
    fn column(&self, name: &str) -> Option<ColumnView<'_>>;
    fn signal(&self, name: &str) -> Option<Value>;
    fn call(&self, name: &str, args: &[Value]) -> Option<Value>;   // host functions: scales, format, signal methods
}
pub enum ColumnView<'a> { Num(&'a [f64]), Str(&'a [Option<Arc<str>>]), Bool(&'a [bool]) }
pub struct Compiled;
pub fn compile(e: &Expr) -> Result<Compiled, CompileError>;
impl Compiled {
    pub fn eval_rows(&self, n: usize, env: &dyn Env) -> Vec<Value>;   // vectorized over n rows
    pub fn eval_scalar(&self, env: &dyn Env) -> Value;
    pub fn fields(&self) -> Vec<String>;   // columns read (dependency tracking)
    pub fn signals(&self) -> Vec<String>;
}
pub fn to_wgsl(e: &Expr) -> Option<String>;   // numeric subset
```

Semantics: `d.x` or bare `x` reads column `x` of the current row; other identifiers read signals;
`a.b(…)` calls host function `"a.b"`; math functions (`abs, min, max, round, floor, ceil, sqrt, log,
exp, pow, clamp, lerp`) are built in and deterministic; total — division by zero is NaN, missing is
Null, no panics.

## datars-data

```rust
pub struct Table { pub name: String, pub columns: Vec<(String, Column)>, pub key: Vec<String>, pub version: u64 }
pub enum Column { Num(Vec<f64>), Str(Vec<Option<Arc<str>>>), Bool(Vec<bool>), Date(Vec<Option<i32>>) }  // NaN = null for Num; Date = days since 1970-01-01
impl Table { len, column(&str), key_of(row) -> datars_scene::Key, validate_keys() -> Result, … }
pub fn read_csv(bytes: &[u8], opts: &CsvOptions) -> Result<Table, DataError>;   // type inference + overrides
pub fn read_json(bytes: &[u8]) -> Result<Table, DataError>;                    // records or columns
pub mod transform { filter(&Table, &[bool]), derive(&Table, name, Column), aggregate(&Table, &[group cols], &[Agg]),
                    sort(&Table, &[(col, desc)]), top_n(&Table, by, n, other_label), bin_num(..), bin_time(..),
                    window(..), join(..), pivot(..), unpivot(..), union(..), sample(.., seed), interpolate_at(..) }
pub mod scale { Linear, Log, Sqrt, Pow, Symlog, Time, Band, Point, Ordinal, Quantize, Quantile, Threshold, ColorSeq, ColorDiv, ColorCat, Piecewise
                — map, invert, ticks (nice; calendar-aware for time), domain/range, lerp (animated rescale) }
```

## datars-algo

Pure functions over slices (no tables): `pie`, `stack`, `dodge`, `treemap` (squarified), `pack`,
`partition`, `tree`, `sankey`, `beeswarm`, `parliament`, `waffle`, `calendar`, `force` (seeded, fixed
iterations), `delaunay`/`voronoi`, `hexbin`, `contour` (marching squares), `density` (KDE grid),
`scatter_in` (n points inside a polygon, seeded, evenly spread), `polylabel`, `label_layout`
(greedy placement with priorities and collision), `nice_ticks`, `time_ticks`. Each with a doc
comment, deterministic, tested.

## datars-motion

```rust
pub enum Easing { … }   impl Easing { pub fn apply(&self, t: f64) -> f64; pub fn parse(s: &str) -> Option<Easing>; }
pub struct MotionRules { pub rules: Vec<Rule> }        // serde; selectors over role / kind / key prefix / state names
pub struct Plan;
pub fn plan(from: &Scene, to: &Scene, rules: &MotionRules, cx: &PlanCx) -> Plan;
impl Plan { pub fn at(&self, t: f64, shaper: &dyn TextShaper) -> Scene; pub fn duration(&self) -> f64; }
pub trait TextShaper { fn shape(&self, node: &mut TextNode); }   // implemented with datars-text (numbers re-format per frame)
```

Matchers (by key, hierarchy splits/merges, nearest, none), choreographies (together, stagger,
phased, wave, ripple, expression windows), interpolators per property (numbers, Vec2, OKLab colours,
parametric geometry, path morph strategies incl. the area-matched disc morph), routes (arc, elbow,
spiral, explode, hop, drift, drop), enter/exit ghost states, clip algebra, springs (closed form).
Invariants: `at(0) == from`, `at(1) == to` exactly; no NaN.

## datars-engine: tiles (Phase 6)

```rust
pub enum Request { …, Range { name: String, url: String, offset: u64, length: u64 } }   // a tile archive read
impl Engine {
    pub fn requests(&self) -> Vec<Request>;                        // load requests + tile ranges views need now
    pub fn provide_range(&mut self, source: &str, offset: u64, bytes: &[u8]) -> Result<(), String>;
    pub fn provide(&mut self, source: &str, bytes: &[u8]) -> Result<(), String>;  // a tiles source: the whole archive
    pub fn set_range_fetch(&mut self, f: Box<RangeFetch>);         // Fn(url, offset, length) -> Option<Vec<u8>>, host IO
    pub fn tile_stats(&self) -> TileStats;                         // requests, bytes, decoded, built, last frame's tiles
    pub fn planned_at(&self, plan: &Plan, t: f64) -> Scene;        // a frame without tile content (motion checks)
}
pub struct FrameOutput { …, pub pending_tiles: u32 }               // render again once ranges arrive
```

IR: `SourceKind::Tiles(url)`; `TKind::Tiles(TTiles { source, layers: Vec<TTileLayer>, tile_size })`;
`TTileLayer { layer, id, filter, minzoom, maxzoom, template, merge (default true), labels, priority }`;
`TGeom::Feature` without `source` = the current tile feature; camera `fit: {"geo": [w, s, e, n] | "=signal"}`;
`TText.offset` (screen px). `datars_geo::Projection::tile_transform(tile, extent) -> Option<Affine>`.
Frames (`frame`, `scene`, `scene_for_state`, `plan_at`, `plan_states`) come with tiles filled; plans
are made between unfilled scenes, so `plan_at(0)`/`plan_at(1)` equal the filled end scenes.
