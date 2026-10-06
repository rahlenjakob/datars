# 07 — Extensibility: recipes, the control ladder, and dogfooding

The engine does the heavy lifting; the developer or coding agent holds the controls. Every level of
control uses the same public API the standard library uses (P5).

## The control ladder

From least to most effort — and each step down keeps every engine guarantee (determinism,
seekability, accessibility, all platforms):

1. **Use a recipe with params.** `bar(votes, { x: "party", y: "share" })`.
2. **Override motion and style** with rules and tokens — no code changes to the recipe.
3. **Wrap or compose recipes.** A "ranked bars with a target line and a callout" recipe is ten lines.
4. **Eject a recipe** into your project and edit it (`datars eject std/bar`).
5. **Write a recipe from primitives.** Shapes, text, instances, scales, coordinates.
6. **Write an algorithm** — a transform, matcher, route or interpolator — as a kernel (JS or WASM).
7. **Write a native plugin** in Rust against the same traits, compiled into your build.

The steps in between are the point. A library that offers only step 1 and a weak step 5 fills the
space between with a wall of options (a scene declaration with 32 fields). Agents in particular are
far better at *editing a small readable recipe* than at finding the right combination of 40 flags.

## Four execution tiers

| Tier | What | Runs | Speed | Portable & deterministic |
|---|---|---|---|---|
| **IR** | the document: recipe instances, transforms, scales, motion rules | engine | native | yes |
| **Expressions** | per-element, per-frame logic | Rust bytecode VM, or GPU (WGSL) | native | yes |
| **Kernels** | arbitrary user code: recipes, transforms, matchers, routes, curve generators | the engine's sandbox, at resolve or plan time | JS (QuickJS): ~10–50× slower than native; WASM: near native | yes (same sandbox on every platform) |
| **Native plugins** | Rust crates implementing public traits | compiled in | native | per build |

Per-frame work never leaves the first two tiers (P7).

## Recipes — code builds the graph

A recipe is a function from typed params to a subgraph: scene nodes whose properties are values,
column references or expressions, plus the transforms, scales and motion rules they need. Recipe code
manipulates **lazy handles** (tables, columns, scales, signals, expressions), like a query builder or
a tracing compiler: it describes work, the engine does it. That keeps expansion O(1) in rows, so
running recipes in a sandboxed interpreter is cheap.

```ts
// A lollipop chart, written exactly the way @datars/std writes its own charts.
import { recipe, t, group, instances, text, expr } from "@datars/sdk";

export default recipe({
  name: "lollipop",
  doc: "A dot on a stem per row: a lighter bar chart for many categories.",
  params: {
    data: t.table({ keyed: true }),
    x: t.field({ of: "data" }),
    y: t.field({ of: "data", type: "number" }),
    r: t.length("4px"),
    fill: t.paint({ default: t.token("accent") }),
  },
  examples: ["examples/lollipop-basic.ts"],

  expand({ data, x, y, r, fill }, cx) {
    const sx = cx.scale("x"), sy = cx.scale("y");          // the enclosing plot's scales
    return group({ key: "lollipop" }, [
      instances.segment(data, {
        a: [sx(data.col(x)), sy.at(0)], b: [sx(data.col(x)), sy(data.col(y))],
        stroke: cx.token("ink-2"), width: "1.5px", semantics: "decoration",
      }),
      instances.circle(data, {
        pos: [sx(data.col(x)), sy(data.col(y))], r, fill,
        semantics: { role: "datum", label: expr`${x}: ${cx.format(y)}` },
        on: { inspect: cx.tooltip() },
      }),
    ]);
  },

  motion: {
    enter: { segment: { from: { b: "a" } }, circle: { from: { r: 0 } } },   // stems grow, dots pop
    choreo: { stagger: { order: "x", spread: 0.4 } },
  },
});
```

What the expansion context (`cx`) provides:

| Capability | Examples |
|---|---|
| Scales and coordinates of the enclosing plot | `cx.scale("x")`, `cx.coord()` |
| Text measurement (resolve time) | `cx.measure(label, style)` → width, height, lines |
| Tokens and theme | `cx.token("accent")`, palettes, typography roles |
| Formatting and locale | `cx.format(field)`, `cx.locale` |
| Graph building | `cx.transform("treemap", data, params)`, `cx.when(cond, a, b)`, `cx.signal("selected")` |
| Child recipes | `cx.use(axis.x, {...})` |
| Keys | `cx.key(...)`, scoped key paths |
| Semantics, anchors, tooltips | declared on nodes |
| Size class and viewport | `cx.size` (`phone` \| `tablet` \| `wide`), aspect |

**Conditional structure stays in the graph.** A recipe that shows direct labels when they fit and a
legend otherwise emits `cx.when(labels.fitRatio.lt(0.8), legend, labels)`: the engine evaluates the
condition, so a resize doesn't re-run the recipe. Recipes re-run only when their *params* change.

### Lambdas compile to expressions

Writing `expr` templates everywhere is clumsy. The toolchain compiles a checked subset of TypeScript
lambdas to the expression IR:

```ts
fill: d => d.share > 30 ? cx.token("accent") : "#c8c8c8",
opacity: d => selected.isEmpty() || selected.has(d.party) ? 1 : 0.3,
```

- At package build time, `@datars/compile` (a bundler plugin) extracts lambdas passed to expression
  slots, type-checks them against the schema, and emits `Expr`. Unsupported constructs (loops,
  closures over mutable state, calls to arbitrary functions) are **build errors** that say which
  construct and suggest a transform or kernel instead.
- Without the plugin, the engine parses the lambda's source at load time (same subset), with free
  variables allowed only if they are recipe params or declared constants.
- Lambdas **never run in JS per row.**

## The standard library is dogfood

`@datars/std` contains every chart type, guide, annotation, control, map style, transition preset and
program preset. It is:

- **Written in TypeScript with the public SDK only** (a lint forbids other imports), built from a
  separate directory as if it were a third-party package, and loaded by the engine like any user
  package. No private hooks.
- **Backed by Rust algorithms through public traits.** Treemap squarification, beeswarm packing,
  sankey layout, label placement and path morphing are `datars-algo` / `datars-motion` code behind the
  same `Transform`, `Matcher`, `Route` and `Interpolator` traits users implement.
- **Small and readable.** A recipe is typically 30–150 lines. A chart catalogue hand-written as one
  module of special cases (about 4,600 lines in an earlier prototype) should become a few thousand
  lines of recipes plus reusable algorithms.
- **Ejectable.** `datars eject std/bar --to recipes/bar.ts` copies the source (with its tests and
  examples) into your project and rewrites your document to use it. Same keys, same motion defaults,
  so it's a drop-in you can now change. (The shadcn/ui model, applied to charts.)

The rule that keeps the API honest: when a std recipe needs something the SDK can't express, we add
it to the SDK for everyone.

## Packages

A package bundles recipes, transforms, motion presets, themes, assets and fonts:

- **Content-hashed and versioned**, with a lockfile; documents reference packages by name + version +
  hash, so a published document renders the same forever.
- **Typed parameter schemas** generate TypeScript types, documentation, inspectors for visual
  editors and agent-facing descriptions (`datars describe std/bar`).
- **Examples and tests live beside the code** and join the test suite ([13](13-testing.md)).
- **Precompiled**: JS to QuickJS bytecode, lambdas to `Expr`, so load time is small.

## The sandbox

`datars-sandbox` runs kernels identically on every platform:

- **QuickJS** (via `rquickjs`), compiled into the engine natively and into the wasm for the web. Its
  `Math` object is replaced with `datars-math` functions, so `Math.sin` gives the same bits
  everywhere.
- **No ambient authority:** no `Date`, no `Math.random` (use the seeded `random(seed)`), no IO, no
  timers; inputs are deeply frozen; outputs are validated against declared schemas.
- **Budgets:** time (interrupt handler) and memory limits per call; a runaway kernel returns a
  diagnostic instead of freezing the app.
- **WASM kernels** for speed (Rust, Zig, AssemblyScript…): `wasmi` natively; in the browser the host
  instantiates the module and links it through a columnar shared-memory ABI. NaN canonicalization and
  no relaxed SIMD keep them deterministic.
- **Optional at runtime:** when a document's recipes and kernels don't depend on runtime signals, the
  publish compiler pre-expands them and the bundle variant needs no QuickJS ([12](12-delivery.md)).
  Apps can refuse downloaded script entirely (`allowScript: false`) and still get pre-expanded variants.

## SDKs

| SDK | Status | Use |
|---|---|---|
| TypeScript (`@datars/sdk`) | first | authoring docs and packages; runs in Node/Bun/Deno/browsers to *build* docs, and in the sandbox to *expand* recipes |
| Rust (`datars` crate) | first | native apps, plugins, the engine's own tests; same concepts, same IR |
| Python (`packages/python`, `import datars`) | available | notebooks and data science: build documents from dataframes with std's recipes, show them in Jupyter with the web runtime as a widget, export through the CLI ([guide](guides/python.md)) |
| IR directly (JSON) | always | agents, other languages, visual editors |

All generated from one schema, so they cannot drift.

## Why not a custom DSL?

A custom DSL (in an earlier prototype, a 4,700-line parser plus a 1,800-line lossless editor) is a
good *storage format for a visual editor*. As the engine's authoring surface it costs more than it
gives: no type checker, no package ecosystem, no language server, no training data for coding
agents, and it turns power into option lists. TypeScript gives us types, tooling and an ecosystem;
the IR gives visual editors a structured document they can edit losslessly with patch ops. A
compact text notation can come back later as an editor feature, compiling to the IR.

## Custom components, as proof

Six JavaScript chart types from the coverage corpus ([15](15-chart-coverage.md)) — `seasons`,
`spiral`, `bubbles`, `petals`, `cloud`, `formation` — are ported as an example package
(`site/articles/recipes/components.js`). Between them they exercise seeded randomness, polar
geometry, bitmap-to-points placement and cross-recipe morphs. `formation` and the dot-density maps
turn out to need the same algorithm — `scatter_in(shape, n, seed)` — which moves into `datars-algo` for everyone. That
is the dogfooding loop working as intended.
