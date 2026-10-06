# 08 — Programs: beyond stories

Stories are very nice — and they are one way to drive a visualization among several. A **program**
describes what drives the signals and time of a document. The renderer, scene model and motion
system don't know which program is running (P4).

## One primitive: a small statechart

```rust
pub struct Program {
    pub states: Vec<State>,           // each sets signals (and may name a scene variant)
    pub transitions: Vec<Edge>,       // on event → target state (+ motion rule overrides)
    pub regions: Vec<Region>,         // parallel regions (e.g. a filter panel and a story at once)
    pub params: Vec<ParamDecl>,       // parameterized states: chapter(key)
    pub drivers: Vec<Driver>,         // what produces events: scroll, steps, timer, clock, gestures, data
}
pub struct State { pub name: Sym, pub sets: Vec<(Sym, ExprOrValue)>, pub hold: Option<f64>, pub children: Option<Program> }
```

- A **state** assigns signals (`step = 3`, `year = 2020`, `focus = "SE"`, `camera = fit("SE")`).
- An **event** (scroll threshold, click on a datum, timer, data arrival, a host button) moves between
  states; the scene re-resolves; the motion system plans the transition.
- **Nested states** are chapters and drill-downs; **parallel regions** let a dashboard's filters live
  beside a narrative.
- **Parameterized states** take a key at runtime (`chapter(country)` entered by clicking a country):
  one state, not a template copied per key by string substitution.

Statecharts are inspectable (the devtools draw them), deterministic, serializable, and editable by a
UI — and they reduce "story", "tabs", "drill-down", "autoplay" and "wizard" to presets.

Generation happens in code. A story DSL needs a looping construct of its own
(`for c in data top 5 { scene "zoom-{c}" … }`); here the SDK is TypeScript, so a loop is a loop:

```ts
const tour = top(eu_gdp, 5).map(c => state(`zoom-${c.key}`, { focus: c.key, camera: fit(c.key) }));
```

## The presets

| Program | Signals & events | Typical host affordances |
|---|---|---|
| **Static** | none | none — resolves once, renders one frame |
| **Interactive** (explorable, widget) | intents → selection, inspection, brushes; host controls → signals | tooltips, controls |
| **Dashboard** | parallel regions; many `View`s sharing signals; filters; drill; live sources | responsive grid, host filter UI |
| **Story** | linear states + drivers (`scroll` scrub/trigger, `step`, `autoplay`), nested chapters | narration UI anchored to the engine |
| **Live** | `<source>.version` events, retention, rate control | connection status |
| **Film** | a fixed clock over a timeline of clips and states | none — frame-exact export |
| **Loop** | infinite clips, seeded randomness, optional data refresh | kiosk, ambient screens |
| **Embedded** | a downloaded bundle inside an app or site; the host sets signals, fills data slots, supplies tokens and listens to events | the app's own UI |

Programs compose: a story step can contain a dashboard; a dashboard tile can play a loop; a film can
be rendered from a story's timeline (every step held for its `hold`, every transition at its duration).

## Stories, precisely

```ts
import { story, step, drivers } from "@datars/std/programs";

export default doc({
  scene: electionScene,                        // one scene template reading `step`, `focus`, `camera`
  program: story({
    steps: [
      step("every-state", { camera: fit("USA") }),
      step("blue-wall", { focus: ["PA", "MI", "WI"], camera: fit(["PA", "MI", "WI"]) }),
      step("sun-belt", { focus: ["AZ", "NV", "GA", "NC"], camera: fit(["AZ", "NV", "GA", "NC"]) }),
    ],
    drivers: [drivers.scroll({ mode: "scrub" }), drivers.steps(), drivers.keys()],
    chapters: { county: chapter(key => countyDetail(key)) },   // entered by activating a state
  }),
});
```

- **Narration is host chrome.** Each step can carry narration text and an anchor (`at: "PA"`); the
  host renders it however it likes (a DOM card, a SwiftUI overlay, a video caption), positioned by
  the engine's anchors. Card designs and article layouts are not engine concerns: they belong to
  the host's page.
- **Scroll-scrub** maps scroll progress between two steps to plan `t`; **trigger** starts a clocked
  transition; both use the same plans ([05](05-time-and-motion.md)).
- **Chapters** are nested parameterized states: activating a datum enters `chapter(key)`, the camera
  flies in (a transition like any other), Back/Esc leaves; the parent state is restored exactly.

## Dashboards, precisely

```ts
export default doc({
  layout: grid({ columns: { phone: 1, wide: 3 } }, [
    view("map",    usMap({ fill: "margin", on: { activate: select("state") } })),
    view("trend",  lines(polls, { series: "candidate", highlight: sel("state") })),
    view("table",  rankedBars(states, { filter: brushOf("trend") })),
  ]),
  program: interactive({ signals: { state: keySet(), trendBrush: range() } }),
});
```

- Every view is a `View` node; all read the same signals. Selecting a state on the map filters the
  bars and highlights a line; brushing the trend filters the table. Structural changes animate
  through plans; highlights are frame-time expressions (no re-resolve).
- Layout is engine box layout, responsive by size class; a rotation re-lays out and morphs.
- Filters and drill state can be serialized to a URL or app state and restored exactly.

## What the engine provides vs. what a host provides

| Engine (all platforms) | Host (per platform) |
|---|---|
| The statechart runtime; drivers as event sources from raw input | The scroll container, page layout, native navigation |
| Plans, clocks, holds, autoplay timers (virtual time) | The real clock (vsync), app lifecycle |
| Engine-drawn controls (sliders, toggles, legends as filters) with semantics | Native controls bound via `set_signal`, if preferred |
| Anchors, semantics, focus order | Narration UI, cards, menus, sharing |
| Serialization of program state | URLs, deep links, persistence |
