---
title: Expressions
description: The expression language that positions, colours and labels every mark in datars — how to write expressions, the syntax, and everything they can read, from rows and signals to scales and formats.
lede: Expressions are how a mark depends on its row, on signals and on the box it's drawn in. They run in the engine — per row, per frame — never as your JavaScript.
---

## Writing an expression

Any property of a mark can be a literal or an expression. There are three ways to write one; all compile to the same thing:

```ts
import { e, expr, lit, field } from "@datars/sdk";

fill: e('d.share > 30 ? "$accent" : "$muted"')     // a string of expression source
x:    expr`scale.x(d.${xField})`                     // a template: splice field names or other expressions
fill: (d) => (d.share > 30 ? "$accent" : "$muted")   // a lambda: its source is parsed by the engine
```

- **`e(source)`** is the plain form.
- **`` expr`…` ``** splices values in: strings splice raw (field names, identifiers), expressions and numbers splice as values, and `lit("text")` splices a quoted string literal.
- **`field("share")`** is the expression `d.share` (quoted when the name isn't an identifier).
- **Lambdas** are written as TypeScript arrow functions, but they are **never called**: their source text is parsed by the engine. They can't close over your variables — use `expr` to splice those in.

Anything written in JavaScript — `Math.random()`, `Date.now()`, a loop — would make charts differ between runs and platforms, so the language doesn't have it. What it has is deterministic everywhere: the same expression gives the same bits in the browser, on a phone and in the CLI.

## Syntax

A JavaScript subset: numbers, strings (`"…"`, `'…'`), template literals (`` `${d.name}: ${d.value}` ``), `true`, `false`, `null`, array literals, arithmetic (`+ - * / % **`), comparison (`== != < <= > >=`), `&& || !`, `??`, the ternary `a ? b : c`, member access and calls, and arrow functions `d => …` or `(d, i) => …`.

Values are numbers, strings, booleans and null. Everything is total: a missing column, signal or result is null; division by zero gives Infinity or NaN; nothing throws. `NaN` counts as missing for `== null` and `??`.

## What an expression can read

### The row

`d.<column>` is a column of the current row: `d.share`, `d["vote share"]`. Inside a `repeat` over ticks, `d.value` and `d.label`; over a legend, `d.value`, `d.ink` and `d.index`; over a count, `d.index`. Rows drawn from a point cloud carry `d.$row`, the row number in the source.

### Signals

A bare name is a signal: `shape == "bars"`, `selected.has(d.region)`, `income * 0.3`. Your document's own [signals](/docs/interaction/), plus these the engine provides:

| Signal | Value |
|---|---|
| `state`, `step` | the program's current state name and index |
| `narration.title`, `narration.text` | the current step's narration (for an on-canvas card) |
| `viewport.w`, `viewport.h`, `sizeClass` | the chart's size, and `"phone"`, `"tablet"` or `"wide"` |
| `box.w`, `box.h` | the size of the box the node is laid out in |
| `inspected` | the key of the mark under the pointer, while there is one |
| `<brush>.lo`, `<brush>.hi`, `<brush>.active` | a brushed range (see [brushing](/docs/interaction/#brushing)) |
| `<view>.x`, `<view>.y`, `<view>.zoom` | where the reader has explored an explorable view to |
| `tile.zoom` | inside a tile layer: the zoom the tile is drawn at |

Key-set signals (selections) have methods: `selected.has(k)` (or `.includes(k)`), `selected.isEmpty()`, `selected.size()`.

`hover()` is true while the pointer is over the element a node belongs to — the nearest node at or above it that the pointer can find (one with intents in `on`, or `pickable`). It's how a control shows where the pointer is: `opacity: e("hover() ? 0.06 : 0")` on a wash over a button. The change animates with the node's motion, only the elements some `hover()` asks about cost a new scene, and it's never true on a touch screen (touch has no hover).

### Scales

Inside a plot, or any group that declares `scales`:

| Call | Returns |
|---|---|
| `scale.x(v)` | the position (or colour, for a colour scale) of value `v` |
| `scale.x.bandwidth()`, `scale.x.step()` | a band's width, and the distance between bands |
| `scale.x.invert(px)` | the value at a position |
| `scale.x.ink(v)` | the colour for `v`, as an ink |
| `scale.x.label(v)` | `v` written as the axis writes it (dates in full) |
| `scale.x.min()`, `scale.x.max()` | the ends of the scale's range |

### Text and numbers

| Function | Does |
|---|---|
| `format(v, spec)` | a number, d3-format style, in the document's locale: `format(1234.5, ",.1f")` → `1,234.5`; default `",.2~f"` |
| `formatDate(v, spec)` | a date (days since 1970-01-01), strftime style: `formatDate(d.day, "%-d %b %Y")` |
| `key.name(k)`, `key.color(k)` | a key's display name and colour from the document's `keys` |
| `token(name)` | a theme number or text token (`token("size.label")`); for a colour token, its ink (`"$accent"`) |
| `measure(text, size?, weight?)` | the width of `text` in the chart's body font, measured by the engine's shaper |

Number formats follow [d3-format](https://d3js.org/d3-format): `[[fill]align][sign][symbol][0][width][,][.precision][~][type]`. Common ones:

| Spec | 1234.567 | Use |
|---|---|---|
| `,.0f` | 1,235 | whole numbers with separators |
| `,.1~f` | 1,234.6 | one decimal, trailing zeros dropped |
| `.0%` | 123457% | shares stored as fractions (0.25 → 25%) |
| `$,.2s` | $1.2k | SI prefixes |
| `+.1f` | +1234.6 | changes |

Separators, decimal mark, currency and minus sign come from `locale` — `sv` writes `1 234,6`.

Date directives: `%Y %y %m %d %e %j %b %h %B %a %A %q %u %w %U %W %V %G %H %M %S %%`, with `-` (no padding), `_` (space) and `0` modifiers: `%-d`. `%q` is the quarter.

### Tables and groups

| Function | Returns |
|---|---|
| `table.count("t")`, `table.sum("t", "col")`, `table.mean(…)`, `table.min(…)`, `table.max(…)` | an aggregate over a whole table |
| `group.count()`, `group.sum("col")`, `group.mean(…)`, `group.min(…)`, `group.max(…)` | the same over the current group (inside `repeat({ groups, by })`) |

```ts
text(e("'Mean ' + format(table.mean('detail', 'price'), '.0f') + ' öre/kWh'"), [0, 12])
```

### Maps

In a `geo` coordinate system (inside a map):

| Function | Returns |
|---|---|
| `geo.x(lon, lat)`, `geo.y(lon, lat)` | the projected position of a point |
| `geo.cx(source, id)`, `geo.cy(source, id)` | a region's visual centre (where a label or symbol belongs) |
| `geo.geodesic(lon1, lat1, lon2, lat2)` | a great-circle path between two points, as path data |

### Maths and strings

Built in: `abs floor ceil trunc round sqrt cbrt exp log log10 log2 sin cos tan asin acos atan atan2 sign pow hypot min max clamp lerp`, also as `Math.<name>`. `round(x, digits)` rounds to decimals. `String(x)`, `Number(x)`, `Boolean(x)`, `isNaN(x)`, `isFinite(x)` convert and test.

Strings have `.length`, `.toUpperCase()`, `.toLowerCase()`, `.trim()`, `.slice(a, b)`, `.startsWith(s)`, `.endsWith(s)`, `.includes(x)`, `.indexOf(x)` and `.toString()`.

### Randomness, seeded

`rand(key, stream)` is a uniform number in [0, 1), `randn(key, stream)` a standard normal draw — both a pure function of their arguments, so the "random" jitter or generated data is the same on every render and every platform. Use a row value as the key (`rand(d.i, 1)`) and different streams for independent draws.

## Where expressions run

- **Per row**, vectorised over whole columns, when a table is derived or a recipe's marks are laid out.
- **Per frame** only where something is animating; plans are computed once per transition, so frames stay cheap.
- **At build time**, `datars publish` evaluates everything that doesn't depend on a runtime signal and ships the result.

To see an expression's value for one element, ask the engine:

```sh
datars explain doc.ts --key '("SD",)'
```

{{run:explain examples/votes/doc.json --key '("SD",)'|16}}

Every expression that made the element is listed with its value — the fastest way to find out why a label sits where it does. More in [preview, check and debug](/docs/tools/).
