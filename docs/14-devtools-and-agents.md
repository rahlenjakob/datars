# 14 — Dev tools and the agent interface

The engine does the heavy lifting; the tools put control and visibility in the hands of the
developer or coding agent. Every capability has its tool (P11), and a visual editor built on
datars uses the same protocol.

## The `datars` CLI

| Command | What it does |
|---|---|
| `datars new <name> [--template]` | scaffold a document or a package (with tests and examples) |
| `datars dev <doc>` | preview server with hot reload; edits **morph** from the old version to the new ([05](05-time-and-motion.md)); inspector attached |
| `datars check <doc>` | load + type-check; diagnostics as text or `--json` (code, path, message, suggested fix) |
| `datars render <doc> --state S --t 0.5 --size phone` | one frame → PNG/SVG via the CPU reference (no GPU needed) |
| `datars film <doc> --from A --to B` | filmstrip, motion trails, onion skin and timing chart for a transition |
| `datars video <doc> --format 9:16` | frame-exact video |
| `datars inspect <doc> --state S [--t] [--select "role=datum"]` | the scene tree as text or JSON, with provenance |
| `datars explain <doc> --key '("SE")'` | why this element exists and looks the way it does: recipe, rule, data row, expression values |
| `datars diff <docA> <docB>` / `--states A B` | element-level scene diff |
| `datars lint <doc>` | design, accessibility and correctness checks (below) |
| `datars test [filter]` | the visual and motion suite ([13](13-testing.md)); `--explain`, `--accept` |
| `datars describe <package/recipe>` | param schema, defaults, examples, motion defaults — for humans and agents |
| `datars data profile <file>` | column types, cardinality, ranges, nulls, suggested keys and semantic hints |
| `datars eject <package/recipe>` | copy a recipe into the project and rewire the document to it |
| `datars build <doc> --floor 1.2 --profiles web,ios-noscript` | document → bundle: partial evaluation, variants per tier and runtime floor, `--explain` ([12](12-delivery.md)) |
| `datars bundle inspect <bundle>` | variants, chunks, sizes (cold and warm cache), what each runtime version would pick |
| `datars serve <dir>` | a local static bundle server with aliases, so a web page or a simulator app can load charts as it would in production |
| `datars publish <bundle> --to s3://… \| r2://… \| dir` | self-host: upload chunks, sign the manifest, move an alias |
| `datars replay <session.log>` | replay a recorded session frame-exactly |
| `datars bench <doc>` | resolve / plan / frame timings, bytes per tier |
| `datars migrate` | upgrade documents across IR schema versions |
| `datars mcp` | the agent interface over stdio |

Every command takes `--json` and writes machine-readable output; images go to files whose paths are
in the output.

## The inspector

A panel that attaches to any running engine (browser extension, embedded panel, desktop app, or
`datars dev`) through the devtools protocol:

- **Scene tree:** keys, roles, recipe of origin; hover to highlight on the canvas and vice versa;
  property values with their source (literal, expression, interpolated, rule).
- **Provenance:** "go to source" for any element — the document node, the recipe line, the data row.
- **Timeline:** scrub any transition; per-element windows and easings; which motion rule won and why;
  the correspondence view (pairs, enters, exits, splits, merges coloured).
- **Graph** (planned, with `datars-graph` wired into the engine): the reactive graph with live values,
  recompute counts and timings; what a signal change invalidated.
- **Program:** the statechart with the current state and available events; fire events by hand.
- **Signals:** read and set any signal (simulate a selection, a brush, a phone viewport, reduced
  motion, a locale).
- **Layout overlay:** box layout outlines, label placement and collisions, hit regions, anchors.
- **Accessibility:** the semantics tree as a screen reader would traverse it; contrast checks;
  colour-vision simulations.
- **Performance HUD:** resolve / plan / frame time, GPU time, instance counts, uploads, bytes.

## The devtools protocol

A typed, versioned protocol (over WebSocket, `postMessage` or stdio) that exposes the host contract
plus read-only queries: `scene.get`, `scene.subscribe`, `graph.get`, `plan.get`, `provenance.get`,
`signals.set`, `program.fire`, `doc.patch`, `seek`, `render`, `record`, `replay`. The CLI, the
inspector, the MCP server, the test harness and visual editors are all clients of it, so an editor
can never have capabilities the public tools lack (P5, [16](16-licensing.md)).

## Lint: visualization correctness, not just syntax

| Category | Examples |
|---|---|
| Identity | row-index keys; duplicate keys; a transition where most elements enter/exit (probably mismatched keys) |
| Encoding | bar axis not starting at zero; dual axes without labelling; too many categorical colours (> 8–10); pie with too many slices; log scale with zero or negative values |
| Legibility | label collisions above budget; text below minimum size at a size class; text overflow; truncated labels |
| Accessibility | datum nodes without semantics; contrast below WCAG AA; categorical palette not distinguishable under common colour-vision deficiencies; no text alternative for a step |
| Motion | transitions longer than a threshold; staggers that push the last element past a readable time; routes that leave the viewport |
| Performance | recipes that loop over rows in JS (suggest a transform or expression); kernels over budget; residual data above a threshold for the chosen tier |
| Portability | system fonts (per-platform determinism); host-only features used in a static profile |

Rules have ids, severities and fix suggestions; documents can configure them.

## The agent interface

Coding agents are primary users. Everything they need is structured, deterministic and runs without
a GPU or a browser.

**MCP tools** (thin wrappers over the CLI and the protocol):

| Tool | Returns |
|---|---|
| `check(doc)` | diagnostics with codes, paths, and fixes |
| `render(doc, state, t, size)` | image path + scene hash |
| `film(doc, from, to)` | filmstrip, trails, onion, timing image paths |
| `inspect(doc, state, select)` | scene subtree with provenance |
| `explain(doc, key)` | the chain from data row to pixels for one element |
| `lint(doc)` | findings with fixes |
| `run_tests(filter)` / `explain_failure(id)` / `accept_golden(id, reason)` | results, diffs, images |
| `describe(recipe)` / `search_recipes(query)` | schemas, examples, motion defaults |
| `profile_data(source)` | column profile, suggested keys and encodings |
| `patch(doc, ops)` | apply typed edits, return new diagnostics |
| `build(doc, target)` | the bundle: variants, bytes, decisions |
| `open_bundle(url, runtime_version)` | what a given runtime would show (variant, fallbacks) + a rendered frame |

**A typical agent loop:**

1. `profile_data` → pick keys and encodings.
2. Write the document with std recipes (types catch most mistakes at edit time).
3. `check` → fix diagnostics.
4. `render` the states at `phone` and `wide`; look at the images.
5. `film` each transition; look at the motion trails.
6. `lint` → apply fixes; `run_tests` → accept new goldens with a reason.
7. `build` → `open_bundle` on the oldest supported runtime; check the variant it picks and the bytes.

**Documentation built for agents:** reference docs are generated from the schema and package
sources (every recipe with params, defaults, examples and rendered thumbnails), plus a compact
`llms.txt`-style index. Examples are small and runnable, and double as tests, so they can't go stale.

**An agent-buildability benchmark:** a fixed set of prompts ("a ranked bar chart of X that morphs into
a map of Y", "a cross-filter dashboard of Z") run against the toolchain with an agent; we track the
success rate, iterations and time. It is a release metric: if agents struggle, the API or the docs are
wrong.

## Playground

A web playground (`@datars/sdk` + engine + inspector in the browser): editor, live view, inspector,
shareable links, and the example gallery as its starting points. It's also how people try datars
without installing anything.
