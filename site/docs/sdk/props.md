---
title: Props and expressions
titleTag: Props and expressions — SDK reference · datars docs
description: How @datars/sdk writes property values — literals, e(), expr templates, field(), lit() and lambdas — and the helpers that normalize them, with a figure of the same property written five ways.
lede: Any property of a node is a `Prop`: a literal, or an expression the engine evaluates per row and per frame. Expressions are data, not code — your TypeScript builds them once; the engine runs them everywhere, identically. What they can say is the [expression language](/docs/expressions/).
---

## Values

{{sdk:Prop}}

{{sdk:Literal}}

Numbers, strings, booleans and `null` are used as they are. A string can also name a theme token where a value is expected — an ink (`"$accent"`), a size (`"$size.label"`), a font (`"font.title"`). In the JSON, a string starting with `=` is an expression too (`"=box.w - 10"`), which is handy inside plain data like a scale's `range`.

{{sdk:Expr}}

{{sdk:Lambda}}

## Writing expressions

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-props-forms" data-phone-ratio="1.44" data-caption></div></div><figcaption>The same dots' radius and fill written as a literal, with <code>e()</code>, with an <code>expr</code> template, as a lambda, and as an expression reading the signal <code>k</code>. Step: only the last row reads <code>k</code>, so only it changes. <a href="{{src}}/site/figures/sdk/props-forms.ts">Figure source</a></figcaption>{{alt:sdk-props-forms}}</figure>

{{sdk:e}}

The plain form: expression source as a string. `e("scale.y(d.v)")`, `e('view == "bars"')`, ``e("`${d.name}: ${format(d.v, ',.0f')}`")``.

{{sdk:expr}}

A template literal that splices: strings splice raw — field names, identifiers, operators — while expressions, lambdas and numbers splice as values (in parentheses). Use it to build an expression from your own variables:

```ts
const f = "share";
expr`scale.y(d.${f})`                          // scale.y(d.share)
expr`${e("d.v")} * ${2}`                       // (d.v) * 2
expr`d.party == ${lit(party)} ? 1 : 0.3`       // d.party == "S" ? 1 : 0.3
```

{{sdk:lit}}

A string spliced into `expr` as a quoted literal rather than raw text — for values, not names.

{{sdk:field}}

`field("share")` is `d.share`; a name that isn't an identifier is quoted (`field("vote share")` is `d["vote share"]`). Recipes use it through [`cx.field`](/docs/sdk/recipes/#Cx).

### Lambdas

```ts
fill: (d) => (d.share > 30 ? "$accent" : "$muted")
```

A lambda is written as TypeScript but never called: its source text becomes the expression, and the engine parses it (a JavaScript subset, `d => …` or `(d, i) => …`). So it can't close over your variables — `(d) => d.v > limit` sends the name `limit`, which the engine reads as a signal. Splice values with `expr` instead — and don't minify documents: a minifier would rename `d`.

## Helpers

These are what the builders use to turn props into JSON; recipes and tools that build templates by hand can use them too.

{{sdk:isExpr}}

{{sdk:src}}

`src(v)` is a prop as expression source, ready to splice: expressions and lambdas in parentheses, strings quoted, arrays as array literals, `undefined` and `null` as `null`.

{{sdk:norm}}

`norm(v)` is one prop in its JSON form: a lambda becomes `{ expr }`, arrays are normalized element by element, anything else is kept.

{{sdk:clean}}

`clean(o)` normalizes a whole object: `undefined` fields are dropped and lambdas become expressions, recursing into plain objects and arrays (expressions are left as they are). `doc()`, every builder and every recipe expansion run their output through it.
