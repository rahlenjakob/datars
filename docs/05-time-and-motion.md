# 05 — Time and motion

Motion is the product. A closed set of animation styles is quick to ship and soon outgrown; here
every stage of a transition is open, and all of it stays pure, seekable and fast (P2, P7).

## Four kinds of time — kept apart

| Time | Owner | Example |
|---|---|---|
| **Host clock** | the host (vsync, a video encoder's fixed step, a test's virtual clock) | `now = 12.016 s` |
| **Timeline time** | the program: where a story, film or loop is | "0.4 s into the move from step 3 to step 4" |
| **Local time** | a clip or element, derived by combinators (delay, stretch, ease, stagger) | "bar SE is 70 % through its own window" |
| **Data time** | a *signal*, e.g. `year = 1998.4` | a choropleth recolouring, a bar-chart race |

The last two are easy to conflate (a story `timeline` plus a per-step `time`). Here data time is
just a signal; a program may **bind** it to timeline time (`year = lerp(1967, 2023, timeline/40s)`)
or to a slider, to scroll, or to a live feed.

## Two ways things move

1. **Authored animation — clips.** A clip is a pure function of local time producing property
   values: intros, loops, draw-on reveals, a film's choreography. Clips compose with a small
   algebra; every composition has a known duration, so seeking is a lookup.
2. **Reactive transitions — plans.** When the resolved scene changes (a step, a filter, a data tick,
   a resize, a code edit), the engine builds a **plan** from the previous scene to the next and plays
   it. Plans are pure functions of `t`, so scroll can scrub them and video can render them.

Both produce the same thing — property values over keyed nodes — and both are evaluated in Rust at
frame time.

**Clock signals** (built; clips are not yet) cover the common ambient case: a signal declared with
a rate (`signal.clock(8)`, IR `"clock": 8`) counts up while a settled scene reads it — a globe whose
projection centre is `15 - spin` turns. Its time accumulates only between transitions, so a morph
never makes it jump; it pauses off screen and for reduced motion; renders, bakes and goldens see
its default. The engine probes once per state whether the scene reads it, so states that don't
sleep as usual (`crates/datars-engine/tests/clocks.rs`, the **worlds** example).

### Clip algebra

```
seq(a, b)        par(a, b)         delay(d, a)       stretch(k, a)     reverse(a)
loop(n | ∞, a)   ease(curve, a)    stagger(by, a)    hold(d)           keyframes([(t, v, curve)…])
trim(node, 0→1)  follow(node.prop, expr)             at(t0, a)         bind(signal, a)
```

`bind(signal, a)` drives a clip's local time from any signal (scroll position, a slider), which is
how scroll-scrubbed reveals work without special cases.

## Anatomy of a transition plan

```rust
pub struct Plan {
    pub corr: Correspondence,          // who becomes whom
    pub windows: Vec<Window>,          // per pair / enter / exit: [start, end] ⊂ [0,1] + easing
    pub interps: Vec<PropInterp>,      // per pair × property: which interpolator, precomputed data
    pub routes: Vec<Option<RouteRef>>, // optional path each element travels
}
impl Plan { pub fn at(&self, t: f64) -> FrameProps { /* pure */ } }
```

Built once per change, in four stages. Each stage has built-ins, an expression form (per-element,
compiled), and a kernel form (arbitrary user code at plan time).

### 1. Match — `Matcher`

Produces a `Correspondence`: pairs (1:1), enters, exits, **splits** (1:N) and **merges** (N:1).

| Built-in | Behaviour | Used for |
|---|---|---|
| `by_key` (default) | same key path → pair | every keyed morph |
| `by_hierarchy` | parent key ↔ its unit children | bar → waffle cells → hemicycle seats; a year's bar → one dot per event (`year#i` unit keys) |
| `by_parent(field)` | children ↔ their parent in another level | counties merge into their state on drill-up |
| `by_field(expr)` | pair on a computed value | re-keyed datasets |
| `nearest` | optimal assignment by position (deterministic Hungarian / auction) | unkeyed data, formation-style morphs |
| `none` | everything exits and enters | crossfades |
| kernel | any function returning a correspondence | domain-specific pairings |

Splits and merges carry a **partition strategy**: how one shape divides into N pieces (slices along
its long axis, a grid, Voronoi cells, or pieces sized by the children's values), so a bar can shatter
into its seats and reassemble.

### 2. Choreograph — `Choreography`

Assigns each element a timing window and easing:

| Built-in | Parameters |
|---|---|
| `together` | — |
| `stagger` | `order` (data, left, right, center-out, value, random-seeded, or any expression), `spread` |
| `phased` | exit → update → enter shares, e.g. `(0.3, 0.5, 0.2)` |
| `wave` | direction, wavelength |
| `by_group` | group expression, inner choreography |
| `ripple` | an origin (e.g. the clicked point) — delay ∝ distance |
| expression | `window = { start: rank(d.value) / n * 0.6, end: start + 0.4 }` |
| kernel | arbitrary; output is a table of windows |

### 3. Interpolate — per property

Defaults come from the property type ([04](04-primitives.md)); rules override them. Geometry has
several **path-morph strategies**:

| Strategy | What it does | Good for |
|---|---|---|
| `parametric` | lerp parameters when kinds match | rect → rect, arc → arc, area → area |
| `resample` | equal-arc-length outlines, aligned winding and start point | convex-ish shapes |
| `disc` | travel + blend through an area-matched disc | bar → slice → dot → country, never folding |
| `triangulate` | piecewise morph of triangulations | complex polygons with holes |
| `split_parts` | multipolygons: largest part morphs, small parts fade | countries with islands |
| `crossfade` | opacity swap | anything else |

In-flight geometry uses a simplified outline (level of detail by on-screen size), so a 3,000-vertex
coastline doesn't cost 3,000 vertices per frame while it's the size of a bar.

**Lines keep their points.** Open lines (and areas' edges) of the same length lerp vertex by
vertex. Of different lengths they *merge* rather than resample by arc length: every vertex of both
is kept and partnered at the same relative place on the other line — by x when both run
monotonically in x (a series: points travel straight up and down), else by arc length — so a data
point is a vertex in every frame and the first and last frames are the lines as drawn (smooth
curves are sampled along their curves for this). **Markers ride their line**: instances whose every
instance sits on a vertex of a line in the same group (a line chart's dots — found geometrically,
no chart vocabulary) pair the line's vertices by their keys (a sliding window slides instead of
morphing in place), share its window, easing and route whatever their own rules say, enter from
their vertex's place on the old line and exit onto the new one, and appear (or go) as a draw-on
trim reaches them. An area whose top is such a line pairs its vertices the same way.

**A line that runs on grows along itself.** When every point of one line is a point of the other,
all at one end (a series with new data, or cut back to a period), the shared part moves with its
keys while the extra part is drawn on like a pen along its own shape — which moves with the point
where the two meet, so a line ending at the plot's edge keeps its end on that edge while the rest
rescales: new data scrolls in there, a part cut off slides out through it. The pen moves at a
steady x for a series (by length otherwise), its dots appear as it reaches them, and an area under
the line grows with it. **Callouts ride their point**: an annotation whose marker (a small dot)
sits on a line's vertex moves with that vertex — leaving and arriving ones too, so a note never
hangs where the line was — and carries its text and connector with it; one kept across the step
(the same key) travels along the line from its old point to its new one.

### 4. Route — the path an element travels

By default an element moves on a straight line in its coordinate space (so in polar or geo
coordinates the "straight" line is resampled correctly). Routes replace that path with a curve whose
detour is zero at both ends, so settled states stay exact:

`arc(height)`, `elbow(x-then-y)`, `spiral(turns)`, `explode(from center)`, `hop(height)` (for waves),
`drift(seeded)` (swarm), `drop(bounce)`, `along(path)` (follow a road, a great circle), or a
kernel that returns a polyline per element — computed once per plan, sampled per frame.

### Enter and exit

Elements without a partner interpolate from or to a **ghost state**: a partial set of property
overrides, relative values allowed:

```ts
enter: { from: { opacity: 0, scaleY: 0, origin: "baseline" } }   // bars grow from the axis
exit:  { to:   { opacity: 0, y: "+12px" } }
enter: { from: "parent" }                 // appear from the parent datum's position (drill-down)
enter: { from: (d, ev) => ({ pos: ev.point, r: 0 }) }  // burst out of the click
```

**Recipes ship their own motion defaults** — the bar recipe says bars grow from the baseline and
labels count; the arc recipe says new slices sweep in from their neighbours. They aren't hardcoded
per chart kind in the engine: they're part of the recipe and overridable per document.

## Motion rules — CSS for data-joined elements

Documents and recipes declare motion as rules with selectors over key paths, roles, node kinds and
states:

```ts
motion([
  { when: { from: "*", to: "*" }, duration: 1.2, easing: "cubic-in-out" },
  { when: { to: "ranked" }, select: "role=datum",
    choreo: stagger({ order: d => d.value, spread: 0.5 }), route: arc({ height: 40 }) },
  { select: "role=axis", interp: { scale: "data" } },          // axes rescale, not tween pixels
  { select: "kind=text role=datum", enter: { from: { opacity: 0 } }, delay: 0.7 },
]);
```

Rules cascade by specificity like CSS. The devtools timeline shows which rule won for any element.

## Invariants (property-tested)

- `plan.at(0)` equals the previous scene and `plan.at(1)` the next, bit for bit.
- No NaN or infinity at any t; windows within [0, 1]; routes vanish at both ends.
- Monotone properties stay monotone where declared (an entering element's opacity never dips — the
  "flash" bug class).
- Areas of morphing outlines never cross zero (the "inside-out" bug class).

## Retargeting and velocity

When the scene changes again mid-flight (a new data tick, a second click), the engine materializes
the current frame as the new "from" — including **velocities** — and plans onward from there. Springs
take the velocity as their initial condition, so motion stays smooth across interruptions. All of it
is deterministic given the event log (P2).

## Easing is open

```rust
pub enum Easing {
    Named(NamedEasing),                  // linear, cubic, sine, expo, back, elastic, bounce, …
    CubicBezier(f64, f64, f64, f64),     // CSS-compatible
    Spring { stiffness: f64, damping: f64, mass: f64, v0: f64 },  // closed form: seekable
    Steps(u32, StepPosition),
    Keyframed(Vec<(f64, f64, Curve)>),
    Expr(ExprRef),                       // compiled; runs per frame at native speed
    Lut(LutRef),                         // sampled from a kernel at plan time
}
```

Springs are the classic non-seekable animation; the damped harmonic oscillator has a closed form, so
here they are pure functions of `t` like everything else.

## Two-level interpolation

*Interpolate causes when you can, effects when you must.*

- **Data level:** when marks are positioned through a shared scale or coordinate system, interpolate
  the scale (domain, range, projection parameters, camera) and the data values, and evaluate the mark
  through them each frame. Marks stay glued to their gridlines during a rescale, log axes animate
  correctly, and a stacked area recomputes its stack from interpolated values instead of sliding
  vertices.
- **Visual level:** when the mapping changes structurally (bar → pie, map → chart), interpolate
  resolved geometry with a path-morph strategy.

The resolved scene keeps a property as a frame-time expression over interpolatable inputs when the
data level applies, and as a concrete value otherwise. The choice is per property and visible in the
inspector.

## GPU interpolation for instances

For `Instances`, the plan uploads *from* and *to* columns plus per-instance windows once; the vertex
shader computes each instance at uniform `t`. The CPU does no per-frame work, and seeking is free.
The CPU path uses the same formulas and remains the reference for determinism; the GPU path is
verified against it within tolerance ([13](13-testing.md)). This is what makes a 10⁶-point morph run
at 60 fps.

*Status: planned ([17](17-roadmap.md)).* Today the engine interpolates instance columns on the CPU
each frame and the renderer uploads the sets that moved. Sets that didn't change keep their
converted columns and their GPU buffer from frame to frame: a camera move or zoom over them, or a
fade (the groups' opacity is a per-draw alpha, not baked into the columns). `datars profile` and
the page's stats panel count the instances rebuilt per frame, so a transition that re-uploads a big
set every frame shows up on a fast machine too.

## Scroll, steps, and scrubbing

The story program ([08](08-programs.md)) supports two modes, both using the same plans:

- **scrub**: `t = scroll progress` between two steps — the reader drives the transition directly,
  forwards and backwards.
- **trigger**: crossing a threshold starts a clocked transition to the next step.

## Reduced motion and accessibility

`prefers-reduced-motion` (or the platform equivalent) is a signal. The standard library's rules
respond to it — routes flatten, staggers collapse, long moves become short crossfades — so every
document gets a reduced-motion variant without extra authoring. Screen readers get a semantic
description of what changed ("Norway moved from 5th to 2nd").

## Hot reload morphs

A code edit produces a new resolved scene; the engine plans from the old scene to the new one. A
developer (or agent) editing a recipe sees the chart **morph into its new version** instead of
flashing — the keyed join applied to development. Keys make it meaningful: the same datum glides to
its new place.

## Classic transition styles, as recipes

| Style | datars preset (in `@datars/std/motion`) |
|---|---|
| morph | defaults |
| stagger / cascade | `stagger({ order })` |
| wave | `wave` + `hop` route |
| arc | `arc` route |
| explode | `explode` route + `stagger(center-out)` |
| scatter | `drift` route (seeded) |
| spiral | `spiral` route |
| drop | enter ghost from above + `bounce` easing |
| fade | `none` matcher (crossfade) |
| phased | `phased(exit, update, enter)` |
| elbow | `elbow` route |
| lift / breathe / settle | quiet presets: short stagger + sine easing + small overshoot or none |
| order data/left/right/center/random/value | `order` parameter of `stagger` |
| enter/exit grow, fade, rise, drop, pop | ghost-state presets |

Each is a few lines of TypeScript in the standard library, and a user can copy one and change it
([07](07-extensibility.md)).

### Splits and merges by identity

A data element whose key is the parent of keys on the other side — a party's bar `("S",)` and its
seats `("S", 1…107)` — splits into them (or they merge into it) without a rule: the bar's
geometry is partitioned (`auto`: near-square cells when the pieces are compact marks like seats
and waffle squares, slices when they're strips) and each piece flies to its seat. Instanced marks
become per-instance elements for this only when the other scene has their parents, so 10⁵-point
scatters keep moving as columns. Texts take turns: a label leaving and a different one arriving
in the same place (a title that changes with the scene) never show at once — the old one is out
in the first half of their common span, the new one in during the second; screen-size text
fades where a ghost would scale it from nothing.
