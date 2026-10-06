# 01 — Background: what shaped the design

datars is a second attempt. An earlier prototype of ours proved the model: a Rust core rendering
keyed, animated charts, maps and stories on several targets. It also showed which structural
choices make every later feature harder. This page keeps the lessons; the rest of the docs describe
the design they led to.

## Ideas that proved themselves

| Idea | Why it's right | Where it lives now |
|---|---|---|
| **One Rust core, many targets** | One implementation for web, apps, posters and video | The foundation ([03](03-architecture.md), [10](10-platforms.md)) |
| **Time is a parameter** — `frame(t)` is pure and seekable | Scrubbing, reversal, posters at any t, frame-exact video, tests by seeking | Principle P2; interactive sessions replay from event logs |
| **Keyed joins as a tweened reconciler** | Object constancy across any change is what readers follow | Matchers: 1:1, splits, merges, custom ([05](05-time-and-motion.md)) |
| **Cross-shape morphs** — outline resampling, an area-matched disc in flight, winding alignment | Bar → slice → dot → country without folding | Path-morph strategies in `datars-motion` |
| **Showy motion as detours that are zero at both ends** | Motion that never corrupts the settled states | Routes and choreographies, open for extension |
| **Retained GPU resources, tiny per-frame updates** | Hundreds of thousands of marks at display rate | The display list and slab-allocated meshes ([11](11-rendering.md)) |
| **Sandboxed JavaScript compiled into the engine, run at resolve time only** | Same output on every platform, no Node, `frame(t)` stays fast | The whole standard library runs this way ([07](07-extensibility.md)) |
| **Sans-IO readers** (range reads of tile archives) | IO lives at the host edge; the core stays portable | The whole engine is sans-IO (P10) |
| **One map from world to street** — world space, van Wijk flights, range reads, budgeted uploads | Maps as the same kind of graphic as charts | `Tiles` and ordinary primitives ([09](09-geo.md)) |
| **Visual tests that capture transitions** | Motion regressions are visible to people and coding agents | Exact goldens and dense sweeps ([13](13-testing.md)) |
| **Themes as data**, **an MCP server for agents** | Brands without forks; agents as first-class authors | [18](18-themes.md), [14](14-devtools-and-agents.md) |

## Mistakes the design avoids

| In the prototype | What it cost | Now |
|---|---|---|
| A closed enum of chart types and a scene struct with dozens of chart-specific fields | Every new chart was an engine change; charts didn't compose; power came from adding options | Primitives only; charts are recipes (P4, [04](04-primitives.md)) |
| Two fixed data shapes; dates as decimal years; composite identity encoded in strings | Tables and queries bolted on later compiled back down to them | Typed, columnar, keyed tables with composite keys ([06](06-data-and-reactivity.md)) |
| A separate frame type per chart family, and roles hidden in key prefixes | No single scene tree to test, pick, theme or animate | One scene tree with explicit roles |
| Signals that could only be numbers | No selections, ranges, pointer positions or keys | Typed signals, intents and bindings |
| Text measured and wrapped by each target's own machinery | Different line breaks on the web, in video and in posters | The engine shapes and lays out text with fonts that travel with the chart |
| Geometry only through the Web-Mercator map path | Floor plans and fictional maps drawn as tiny lon/lat degrees | Planar and custom coordinates |
| A story-shaped root object | Dashboards, explorables, live monitors and films had no home | Programs as statecharts; story is one preset ([08](08-programs.md)) |
| Product concerns (card styles, page CSS, localised chart names) in engine types | The engine couldn't be separated or opened | Engine types know nothing about products (P5) |
| Built-in charts as special-cased engine code; custom components a weaker second path | The extension API never became sufficient | The standard library uses only the public SDK (dogfooding) |
| Determinism claimed but not enforced (platform libm, the sandbox's `Math`) | The last bits differed between targets | Our own libm, lints, cross-target hash tests (P1) |
| Offline rendering only through one platform's GPU API; visual tests in a browser | Nothing ran in a Linux CI container or a cloud agent's sandbox | A bit-exact CPU reference renderer runs anywhere ([11](11-rendering.md)) |
| Pixel tests with loose thresholds and five frames per transition | Loose enough to hide a 2 px shift; one-frame flashes slipped between samples | Exact hashes, 64-sample sweeps, motion invariants, scene diffs as evidence |
| A hand-written text DSL as the storage format | Tools couldn't generate or validate against a stable schema | The JSON IR is the contract, with a generated schema and migrations |

The verdict that started the rebuild: the *model* — pure time, keyed identity, one core, many
targets, sandboxed recipes, retained GPU resources — was right; the *vocabulary* — chart types,
story parts, fixed data shapes, frames per chart family — was wrong. datars rebuilds the vocabulary
bottom-up and keeps the model.
