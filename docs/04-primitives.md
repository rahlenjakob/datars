# 04 — Primitives

The engine's vocabulary. Small on purpose: everything in the standard library — every chart type,
map style, guide and annotation in the old examples — expands into these ([15](15-chart-coverage.md)).
Rust sketches are illustrative, not final signatures.

## Keys

Identity is the heart of the model (P3). A key is a typed tuple, scoped by its parent:

```rust
pub enum KeyPart { Str(Sym), Int(i64), Num(OrdF64), Date(i64) }
pub struct Key(SmallVec<[KeyPart; 2]>);          // ("SE",)  ("Chile", 2021)  ("S", Unit(12))
pub struct KeyPath(Arc<[Key]>);                   // chart / marks / ("S",)
```

- **Composite keys are tuples**, not `"series@x"` strings. A grouped bar is `(series, x)`; a heatmap
  cell with the same data has the same key, so the two morph.
- **Units are hierarchical keys**: seat 12 of party S is `("S", Unit(12))`, a child of `("S",)`. The
  matcher can split one parent into its units (bar → waffle cells → hemicycle seats) and merge them
  back ([05](05-time-and-motion.md)).
- **Scope is the path.** Two recipes can both use `("SE",)` without colliding; a cross-recipe morph
  (a map region into a bar) happens when a transition rule relates the two scopes (or they share one).
- Keys are interned; comparisons are integer compares; order is total and deterministic.

## Values and interpolation

Every animatable property has a type that knows how to interpolate:

```rust
pub trait Interpolate: Clone {
    fn interpolate(a: &Self, b: &Self, t: f64, cx: &InterpCx) -> Self;
    fn distance(a: &Self, b: &Self) -> f64 { 1.0 }   // for springs, continuity checks, choreography
}
```

| Type | Default interpolation | Alternatives |
|---|---|---|
| `f64`, `Length` | linear | log, step, spring |
| `Vec2` | linear | route (arc, spiral, custom path), polar |
| `Color` | OKLab | OKLCH shortest hue, sRGB, step |
| `Angle` | shortest arc | long arc, winding-preserving |
| `Affine` | decomposed (translate · rotate · scale · skew) | matrix lerp |
| `Geom` | parametric when kinds match (rect → rect) | path morph strategies when they don't |
| `Paint` | per stop in OKLab | crossfade |
| `Text` | crossfade; numbers count (formatted each frame) | step at 0.5, typewriter |
| `Discrete<T>` | step at 0.5 | step at start/end/custom |

A transition may override any of these per property with a rule; users can register interpolators
for their own property types.

## Units

Lengths carry units so geometry and legibility scale differently under zoom:

```rust
pub enum Length { Px(f64), Units(f64), Em(f64), Percent(f64) }
```

- `Units` live in the node's coordinate system and move with the camera.
- `Px` stay screen-sized (stroke widths, label sizes, symbol radii) — the old `ctx.px(n)` trick,
  now a type.
- Hosts pass the device pixel ratio; the engine resolves `Px` per frame.

## Nodes

```rust
pub struct Node {
    pub key: Key,
    pub kind: NodeKind,
    pub common: Common,                 // transform, opacity, visible, z, blend, clip, trim, filter
    pub semantics: Option<Semantics>,
    pub hit: Hit,                       // None | Bounds | Geometry | Custom(Geom), + cursor
    pub on: Vec<Binding>,               // intent → signal update
    pub anchors: Vec<AnchorDecl>,
    pub provenance: ProvId,             // doc node · recipe · data row that produced it
}

pub enum NodeKind {
    Group     { children: Vec<Node>, coord: Option<CoordRef>, isolate: bool },
    View      { viewport: Rect<Length>, camera: Camera, content: Box<Node> },
    Shape     { geom: Geom, fill: Option<Paint>, stroke: Option<Stroke>, markers: Markers },
    Text      { content: RichText, layout: TextLayout },
    Image     { asset: AssetId, rect: Rect<Length>, fit: Fit, sampling: Sampling },
    Instances { proto: Proto, cols: InstanceColumns },
    Field     { grid: GridRef, colormap: ColorMap, sampling: Sampling, contours: Option<Contours> },
    Tiles     { source: TileSourceRef, style: StyleRef, lod: LodPolicy },
}
```

| Kind | What it is for | Why it is one kind |
|---|---|---|
| `Group` | Nesting, transforms, a coordinate system, clipping, opacity, isolation for caching | layout lives in the tree, not as pixel offsets baked into every chart |
| `View` | A nested viewport with its own camera: plots, small multiples, insets, magnifiers, maps | a plot camera and a map camera are one mechanism, not two special cases |
| `Shape` | Any single keyed drawable (bar, slice, region, ribbon, rule, arrow) | glyphs, polylines, rules and map regions share keys, paints and morphs |
| `Text` | Engine-shaped text: labels, titles, tooltips, numbers that count | one shaper on every target, not Canvas2D text in page JS, a Rust rasterizer and SVG text side by side |
| `Image` | Pictures: raster logos and photos (PNG), fitted to a rect | a picture is data a renderer samples, not geometry (§Icons and pictures) |
| `Instances` | One prototype × N keyed instances with per-instance columns: 10³–10⁶ points, cells, seats | dot-density arrays and settled circles become instanced discs, not 10⁶ nodes |
| `Field` | Continuous data on a grid (density, heat, raster values), coloured on the GPU, optional contours | — |
| `Tiles` | Viewport-streamed content: vector/raster basemaps, LOD point clouds | a basemap is streamed content in the scene, not a separate map renderer |

`Common.trim: (f64, f64)` draws a portion of a path (line reveals, route draw-on, tracks up to the
clock). `Common.clip` accepts a geometry, so reveals by wipe, masks and plot clipping are the same
mechanism.

## Geometry

Parametric where possible (cheap, exact, morph by parameter); a canonical path for everything else:

```rust
pub enum Geom {
    Rect     { x: Length, y: Length, w: Length, h: Length, radii: [Length; 4] },
    Ellipse  { c: Vec2L, rx: Length, ry: Length },
    Arc      { c: Vec2L, r0: Length, r1: Length, a0: Angle, a1: Angle, pad: Angle, corner: Length },
    Segment  { a: Vec2L, b: Vec2L },
    Polyline { pts: Arc<[Vec2]>, curve: Curve },          // linear | monotone | natural | step | basis
    Area     { top: Arc<[Vec2]>, base: Baseline, curve: Curve },
    Ribbon   { a: (Vec2, Vec2), b: (Vec2, Vec2), bend: f64 },   // sankey links, chord ribbons
    Path     (PathData),                                   // béziers, subpaths, holes, fill rule
    Symbol   { kind: SymbolKind, at: Vec2L, size: Length },
}
```

- Every `Geom` converts to `PathData` (`to_path`) and to an arc-length-parameterized outline, so any
  two shapes can morph ([05](05-time-and-motion.md)).
- Geometry in a non-linear coordinate system (polar, geo) is **resampled adaptively** in screen space:
  a straight segment in data space becomes a curve; a `Rect` in polar coordinates becomes an annular
  sector. Great-circle routes are segments with `geodesic: true`.

## Paint and stroke

```rust
pub enum Paint {
    Solid(Color),
    Linear { from: Vec2L, to: Vec2L, stops: Stops },
    Radial { c: Vec2L, r: Length, stops: Stops },
    Pattern(PatternRef),                     // hatches, dots — uncertainty, print, colour-blind redundancy
    Lookup { lut: LutRef, index: IndexExpr }, // per-feature colour from a table the frame rewrites
}

pub struct Stroke {
    pub paint: Paint, pub width: Length, pub dash: Option<Dash>,
    pub cap: Cap, pub join: Join, pub align: Align,   // center | inside | outside
    pub non_scaling: bool,                            // stays px-wide under zoom (maps, zoomed charts)
}
```

Colours in paints are **inks** — a literal or a late-bound theme token (`"$accent"`, `"$categorical[3]"`), resolved at frame time ([18](18-themes.md)).

`Paint::Lookup` generalizes per-feature colour lookup tables: a choropleth's colours change per
frame by rewriting one small table, not by touching geometry.

## Text

```rust
pub struct RichText { pub spans: Vec<Span> }        // font, weight, style, size, colour, features, link-to-key
pub enum Content { Rich(RichText), Number { value: f64, format: NumberFormat, locale: LocaleRef } }
pub struct TextLayout {
    pub anchor: Vec2L, pub align: Align2, pub max_width: Option<Length>,
    pub wrap: Wrap, pub overflow: Overflow,          // clip | ellipsis | shrink(min) | hide
    pub line_height: f64, pub rotate: Angle, pub halo: Option<(Color, Length)>,
    pub priority: i32,                               // for collision resolution
}
```

- Shaped, bidi-resolved and line-broken by `datars-text` with the fonts the chart carries, identically on every
  target ([11](11-rendering.md)).
- **Measurement is available at resolve time**, so recipes can size margins to axis labels, choose
  between inside and outside pie labels, or fall back to a legend.
- `Content::Number` interpolates the value and formats each frame: labels that count, locale-correct
  everywhere.
- Spans can reference a key (`[the Social Democrats](S)`): the span takes that key's colour and
  follows it when it's recoloured.

### Text along a path

`text(…, { path: { arc: [cx, cy, r, a0, a1] } })` (or `path: { d }`, SVG path data) sets a label
along a curve: a gauge's band names, a radial chart's rings, a circular axis. The text is shaped and
measured as any text; then each glyph is placed on the path at the middle of its advance and turned
to the path's direction there (`TextNode.along`: `[x, y, turn, advance]` per glyph), so every
renderer draws the same glyphs at the same places. `offset` says where along the path the text's
`align` point goes (default by alignment); `upright` (default on) reverses a path that runs right to
left, so text on a circle's lower half still reads left to right.

A text longer than its path is **left out**, not squeezed or run past the end — and `datars labels`
reports it as cut. Recipes measure first (`measure()` against the arc length) and give way: the
gauge leaves out a band name that doesn't fit its band, and sizes names to what the band's
thickness holds, up to the reader's text size. Label audits judge curved labels by their glyphs'
boxes, not the box around the curve.

### Text that stays readable

`contain: true` on a text style nudges the label back inside the canvas after layout, only as far
as it overhangs (the last tick of an axis, a callout near the edge). Inside a map view it keeps the
label inside the view; a label set beside its point (a sideways `offset`) first tries the point's
other side, and one whose point is out of view is left out rather than cut at the edge.
`declutter: true` on a group keeps its texts off each other and off every other text: in order, a
text that would land on an earlier one tries the other side of its point (below instead of above,
left instead of right), else it's left out — place names that crowd together in a phone's narrow
box, the first (most important) winning. Overlap is judged on texts as drawn, a turned label by its
slanted outline. A text's `rotate` (degrees) is measured by the room it covers, so an `auto`-sized
axis under 45° labels is as tall as their slant. In expressions, `measure(text, size?, weight?)` is
a text's width on one line and `measure.word(…)` the width it can't wrap below (its widest
unbreakable run). `on(<ink>)` is an ink for text
on a coloured mark: the theme's ink or paper, whichever reads better on it, resolved late like any
ink (`"on($mark)"`, `"on(#1d4e89)"`). Two texts never show at once in one place during a
transition (docs/05). `datars test` and `lint` fail labels that overlap or are cut off at the
canvas edge.

### Backdrops, dodging and bands (cards)

A group's `backdrop: { fill, stroke, radius, padding, fit }` draws a box behind whatever the group
drew, sized once layout is done (`fit: "width"` spans the group's box across and hugs the content's
height) — a card grows with its text and morphs when the text changes. `dodge: [anchors…]` places a
group's content after the whole scene is laid out, at the first anchor in its box where it covers
the least data and text (marks by their exact outline, text weighted three times). When nowhere is
free, the next pass gives the group a band at the bottom of its parent's box and the other children
lay out in the rest: a card under a crowded chart instead of on it. An anchor may be an expression
(a card that moves between states), and a single anchor is a fixed place — the content goes there
whatever it covers and never takes a band (`std/card`'s `dodge: false`: a card over a map, where
land covers everything). `std/card` is built from these
(kicker, title, wrapped text; theme tokens `card`, `card-ink`, `card-ink-2`, `card-line`,
`radius.card`); a story's step narration is available to it as the signals `narration.title` and
`narration.text`, so one card narrates every state.

### Sizes and layouts for any box

A child's `size` along an axis may be an expression over its parent's box, in px
(`{ w: "=min(260, box.w - 24)" }`: a card as wide as asked, never wider than a phone leaves). A
`columns` layout with `wrap: 560` lays its children out as rows, equal shares, in a box narrower
than that — two charts side by side on a desktop, one above the other on a phone. A tick repeat's
`count` may be an expression (as many ticks as labels of a measured width fit), and each tick
knows how many there are (`d.count`) as well as its `d.index`.

## Instances

The path to 10⁶ marks:

```rust
pub struct InstanceColumns {
    pub keys: KeyColumn,                    // per-instance identity (joins, picking, semantics)
    pub pos: Col<Vec2>, pub size: Col<f32>, pub fill: Col<Color>,
    pub opacity: Col<f32>, pub rotation: Option<Col<f32>>, pub extra: Vec<(Sym, AnyCol)>,
}
pub enum Proto { Circle, Square, Rect, Symbol(SymbolKind), Geom(Geom), Glyph(GlyphRef) }
```

- Stored as structure-of-arrays; interpolation is column arithmetic on the CPU (in the vertex
  shader later: [05](05-time-and-motion.md) §GPU interpolation).
- Semantics and hit-testing work per instance (a 100,000-dot scatter is still navigable and
  pickable).
- Instances and `Shape`s interoperate: a transition may turn one bar (`Shape`) into 30 seat instances.
- **Beyond a frame's worth (`lod`):** an instances node with `lod` draws a table of any size — or a
  point archive read by range. Its rows are indexed once into a pyramid whose levels are seeded,
  density-preserving samples (`datars_algo::pyramid`); like `Tiles`, the node resolves to a
  placeholder that each frame fills from its camera, with the levels whose rows in view fit the
  frame's budget of points (every row once they fit). Tiles are Morton-ordered and templated once
  per canonical cell, so pans and zooms never re-evaluate a row. The **galaxy** example: 4,000,000
  rows, pan frames ~1 ms (`datars profile`), hover on every star drawn.

## Icons and pictures

**Icons** are vector drawings: `icon(name, at, { size, ink, inks, rotate, anchor, label })` (an
`icon` node in the IR). Where they come from:

| Name | Source |
|---|---|
| `heart`, `lucide:heart` | the default set: [Lucide](https://lucide.dev) 1.49.0, ISC, 2,121 icons (`datars icons <word>` searches it) |
| `$icon.up` | a theme icon token, naming one of the others ([18](18-themes.md#icons)) |
| `icons/logo.svg`, `https://…/logo.svg` | a project file (relative to the document) or a URL |
| `<svg …>…</svg>`, `{ d, viewBox, stroke }` | inline markup or path data |

- **Read by `datars-icon`**, a small deterministic reader of the SVG subset icons and logos use:
  `path`, `rect` (with `rx`/`ry`), `circle`, `ellipse`, `line`, `polyline`, `polygon`, `g`,
  nested `svg`, `use` and `symbol` within the file; transforms; `viewBox` (or `width`/`height`);
  `fill`, `fill-rule`, `fill-opacity`, `stroke`, `stroke-width`, `stroke-opacity`, caps, joins,
  `opacity`, `display`, `visibility` — as attributes, inline `style`, or a `<style>` sheet of tag,
  class and id rules. Scripts, event attributes, embedded pictures, XML entities and any reference
  outside the file (`href`, `url(…)`, `@import`) are **refused**; text, clipping, masks, filters,
  dashes and animation are skipped with a note (convert text to outlines). Transforms are baked
  into the paths, stroke widths scaled with them.
- **Colours are inks, never the file's hex.** `currentColor` — and a file drawn in a single colour,
  like most icon sets' black — takes the node's `ink` (default `$ink`). A multi-colour file's other
  colours become **slots**, numbered in the order the file first uses them, for the `inks` the
  author passes (default `$categorical[i]`); `datars icons --slots file.svg` lists them. Gradients
  draw as one slot each.
- An `icon` node resolves to plain **shapes** (a group of one per layer), so hit tests, exports,
  bounds and transitions treat it as any shape; it turns about its point (`rotate`), and `anchor`
  says where `at` is on its box (the file's aspect, the longer side `size`, default the theme's
  `size.icon` or 16 px).
- **Icon symbols and markers:** `geom.symbol({ symbol: "lucide:star" })` draws the icon in the
  shape's fill, `size` its half box (the built-in names `circle` … `star` stay symbols); a stroke
  marker `{ type: "icon", icon, size }` sits centred on the line's end, turned along it, in the
  stroke's paint.
- **Instanced:** `instances({ icon, … })` (`proto: "icon"`) draws thousands of one icon as one node —
  pictograms, unit charts, map markers. The icon is normalized once (`IconGlyph`, the box's longer
  side −1…1, `r` its half size); CPU and PDF draw its layers per instance, SVG writes the drawing once
  in `<defs>` and a `<use>` per instance, wgpu tessellates each layer once and draws it per instance.
  An icon instance that flies to another shape (a pictogram's unit into a bar) travels as its own
  drawing, so it looks like itself at both ends.
- **Decorative unless labelled** (WCAG 1.1.1): without `label` (or `semantics`) an icon is left out
  of what assistive technology reads; with one, it reads as that word. `datars lint` checks a
  labelled icon's ink against what's behind it for 3:1 (WCAG 1.4.11, `a11y/icon-contrast`).

**Pictures** are `image(src, rect, { fit, alt })`: a PNG (lossless: every platform decodes the same
pixels) drawn into `rect` as `fit` says — `contain` (default), `cover` (cut to the rect), `fill`,
`none`. The CPU reference samples its premultiplied texels bilinearly with integer weights, wgpu
from a texture, SVG embeds the file as a `data:` URL, PDF as an image XObject with its alpha as a
soft mask. JPEG, WebP and GIF are refused with a message (a decoder would be a new dependency).
Without `alt` a picture is decorative.

**How they travel** — like fonts. The engine never fetches (P10): an icon or picture it needs is a
request (`icon:lucide:heart` → `datars:icons/lucide/heart.svg`, `image:photo.png`) the host answers;
until it arrives nothing is drawn (no stand-in that would jump), and the scene resolves again when
it does. Tools answer on the spot (`Engine::set_asset_fetch`); the build step fetches what every
state draws and ships **only that** — one `icons` chunk of compact drawings, an `image` chunk per
picture — with each set's licence in the manifest ([12](12-delivery.md#icons-and-pictures)). A
bundle's chart asks for nothing, on any runtime.

## Coordinates

A `Coord` maps a data space onto a group's local plane. It may be non-linear, invertible (for
picking and brushing) and animatable:

| Coord | Maps | Examples |
|---|---|---|
| `Cartesian { x: Scale, y: Scale }` | independent axes | most charts |
| `Polar { angle: Scale, radius: Scale, start, direction }` | (θ, r) | pies, radial bars, the climate spiral |
| `Geo { projection }` | lon/lat → projected plane | maps (Web Mercator, Equal Earth, Albers, orthographic, national grids such as SWEREF 99 TM) |
| `Planar { crs }` | metres or arbitrary units, affine | floor plans (the airport terminal), fictional maps (Westeros), CAD, sports pitches |
| `Custom(kernel)` | anything a kernel computes | cartograms, hex grids, ternary plots |

A coordinate change is a transition like any other: a bar chart can bend into a radial bar chart by
interpolating the coord (cartesian → polar) while geometry is resampled each frame.

## Scales

Named, typed, first-class graph values:

```rust
pub trait Scale {
    fn map(&self, v: &Value) -> Mapped;                  // number, colour, shape, …
    fn invert(&self, m: f64) -> Option<Value>;
    fn ticks(&self, hint: TickHint) -> Vec<Tick>;        // nice, calendar-aware for time
    fn format(&self, v: &Value, loc: &Locale) -> String;
    fn lerp(a: &Self, b: &Self, t: f64) -> Self where Self: Sized;   // animated rescale
}
```

Kinds: linear, log, symlog, pow/sqrt, time (calendar- and timezone-aware), band, point, ordinal,
quantize, quantile, threshold, piecewise, and colour scales (sequential, diverging, categorical,
pinned stops such as `#22c55e 2 · #f5a524 4 · #f97362 6.5`). Domain policies for live data (`fixed`,
`grow`, `window`, `nice-steps`) are scale properties. A band or point scale over **dates** is a
time axis without gaps — trading days side by side, no room for weekends and holidays: its slots
are in calendar order and its ticks fall on the first slot of each calendar interval (the first
session of a month), labelled like a time axis.

Because scales are values in the graph, a mark can be positioned **through** an interpolated scale
at frame time, so marks stay glued to their gridlines while an axis rescales ([05](05-time-and-motion.md)
§two-level interpolation).

## Semantics

```rust
pub struct Semantics {
    pub role: Role,          // Datum, Series, Axis, Tick, LegendItem, Annotation, Title, Control, Decoration, …
    pub label: Label,        // text or template over the datum ("{party}: {share:.1%}")
    pub datum: Option<RowRef>,
    pub order: Option<i64>,  // reading/keyboard order
    pub group: Option<KeyPath>,
    pub value: Option<AccessibleValue>,  // for controls: slider value, range, step
}
```

The engine assembles a semantics tree per frame (as deltas) that hosts map to ARIA, UIAccessibility,
Android accessibility nodes or AccessKit; it also derives data tables, summaries and keyboard
navigation (arrow keys move across data in reading order) ([10](10-platforms.md)). `Decoration`
nodes are hidden from assistive tech.

## Hit regions, events and intents

- Picking is exact at any t: a CPU spatial index over the frame's geometry (a GPU id buffer as an
  optimization for very large instance sets). Results carry the key path, the data row, local and data
  coordinates (through `Coord::invert`).
- Raw input becomes **intents** so documents don't hardcode a mouse: `inspect` (hover on desktop,
  tap on touch, focus on keyboard), `activate` (click, tap, Enter), `pan`, `zoom`, `brush`, `drag`,
  `dismiss`. A tap is its own input (`tap`: a touch released where it went down) — touch has no
  hover — and reaches further around thin and small marks than a mouse pointer.
- Instances are hit as their marks (with a few px of reach), or with `hit: "line"` as the line
  through them in row order: anywhere within `reach` px of it, the nearest instance to where the
  pointer meets it — a line chart's value wherever it's hovered, its points however far apart.
- `Binding`s map an intent on a node to a signal update (`selected.toggle(key)`,
  `focus = key`, `brush = range`), or to a program event (enter a chapter state). Everything
  downstream is dataflow.

## Anchors

A node can declare anchors — named points (`top-center`, the datum's value point, a lon/lat) —
emitted per frame in screen space with visibility. Hosts attach rich UI to them (a React card, a
SwiftUI popover, a native tooltip) and stay in sync with the camera and transitions, because anchors
come from the same frame as the pixels.

## Provenance

Every node records where it came from: the doc node id, the recipe (package, version, function), and
the data row(s). The inspector's "go to source", lint messages, and agents' "why is this bar here?"
all read it ([14](14-devtools-and-agents.md)). It costs one integer per node; the table lives beside
the scene.
