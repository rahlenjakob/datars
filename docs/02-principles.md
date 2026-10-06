# 02 — Principles

These are invariants, not aspirations. Each has a check (a test, lint or review rule) so it stays
true as the code grows. Other docs refer to them as P1…P16.

## P1 — The engine is a pure function

`frame = F(doc, packages, data, signals, t)`. Same inputs → the same **scene** and the same
**display list**, bit for bit, on every target: native (x86-64, arm64), wasm32, iOS, Android.
Pixels are bit-identical on the CPU reference rasterizer and perceptually equivalent on GPUs.
*Check:* the conformance suite hashes every sampled frame of every example on each target and
compares ([13](13-testing.md)).

## P2 — Time is a parameter, never state

Every animation is seekable: authored timelines are pure functions of their clock; reactive
transitions are pure functions of `(from, to, t)`. An interactive session is a pure function of its
**event log** (timestamped inputs + data arrivals), so any session replays exactly.
*Check:* property tests (`at(0) == from`, `at(1) == to` exactly); replay tests.

## P3 — Identity is data

Elements are identified by **keys** derived from data, not by render-node position. Object constancy
is the default across every kind of change: step, filter, data update, resize, rotation, theme
switch, even a code edit during hot reload. Keys are typed tuples, not encoded strings.
*Check:* lint for unstable keys (row-index keys), duplicate keys, transitions where most elements
enter/exit.

## P4 — Few primitives, many recipes

The engine knows keyed shapes, text, images, instances, fields, tiles, groups and views; scales,
coordinates, signals and transitions. It never knows "bar chart", "choropleth", "axis" or "story".
*Check:* the engine crates contain no chart, guide or program vocabulary (a grep in CI).

## P5 — Dogfood the public API

Every built-in — marks, guides, charts, map styles, annotations, transition styles, easings,
programs — is written against the same public API available to users and agents, and ships as an
ordinary package with no privileged hooks. If the standard library needs something the API can't
express, the API is wrong, and we fix the API.
*Check:* `@datars/std` builds from a separate directory as a third-party package; a lint forbids
imports outside the public SDK.

## P6 — Code builds the graph; the engine runs the graph

Authors use real languages (TypeScript first, Rust, later Python). Their code builds a lazy
description — tables, transforms, scales, expressions, scene nodes, motion rules — and the engine
evaluates it. Recipe code is O(1) in the number of rows; per-row work happens in Rust.

## P7 — Custom everything, fast by construction

User code runs either at **resolve time** (recipes, kernels: when params or structure change) or is
**compiled** to engine bytecode (expressions). Custom motion produces *data the engine samples*
(routes, timing windows, curves, lookup tables), so `frame(t)` never calls user-language code.
*Check:* the frame loop has no sandbox calls (enforced by crate boundaries).

## P8 — The engine owns pixels and geometry; the host owns chrome

Everything drawn — including text, tooltips and legends — is laid out by the engine, so it's the same
on every platform. Host UI (menus, pages, native controls, rich cards) attaches to the engine
through **anchors** (screen positions of keyed points per frame), **events** and **semantics**.

## P9 — The IR is the contract

A versioned, documented schema with stable node ids, patchable by typed operations, with generated
types for every SDK. SDKs, visual editors, agents and the publish compiler all produce and consume
it. Schema changes ship with migrations (`datars migrate`).

## P10 — The engine is sans-IO

The core never touches the network, the filesystem or the clock. It *requests* resources (a tile, a
font, a data chunk) and the host fulfils them. This is what makes each new platform cheap and every
run reproducible.

## P11 — No feature without its tool

Every capability ships with a way to inspect it, test it, and use it from an agent: a CLI command, a
devtools panel or protocol message, an MCP tool, and a test fixture.

## P12 — Accessibility is generated, not bolted on

Semantics (role, label, datum, reading order) are part of the node model. Accessibility trees, data
tables, text summaries, keyboard navigation and reduced-motion variants are generated from them for
every platform.
*Check:* lint fails when datum nodes lack semantics; a11y snapshots in the test suite.

## P13 — Performance is designed in

Resolve and frame are separate phases with separate budgets; storage is columnar; joins and
transition plans are computed once; instanced marks interpolate on the GPU; the graph recomputes
incrementally; big data is tiled. Budgets (bytes, time-to-poster, frame p95, memory) run in CI on
reference devices.

## P14 — Send the least that still does the job

Publishing partially evaluates a document: anything that doesn't depend on runtime signals *may* be
computed at build time, and a cost model decides whether baking it or shipping its source is smaller
and faster on the target. Readers get a poster and accessible text first, then the smallest variant
the interactivity needs ([12](12-delivery.md)).
*Check:* byte and time budgets per example and variant in CI.

## P15 — Curated dependencies, not zero dependencies

Dependencies must be pure Rust (or vetted C for QuickJS), deterministic, wasm-compatible,
permissively licensed, and pass `cargo deny`. Never hand-roll what is correctness-critical and
well-solved elsewhere (text shaping, line breaking, boolean polygon ops). Never pull in a dependency
that does IO, threads or time behind the engine's back.

## P16 — Content updates never require rebuilding the host

Apps and sites install the runtime once. Charts are **bundles** fetched over the network — data and
sandboxed bytecode, never native code — so publishing, updating or rolling back a chart needs no app
release and no site deploy. Bundles keep working on runtimes that are years old: the compiler targets
a runtime floor, emits variants, and always includes a static, accessible fallback.
*Check:* version-skew tests render every example bundle on the last N runtime releases; a web page and
the sample iOS and Android apps load newly published bundles without being rebuilt.
