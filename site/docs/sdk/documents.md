---
title: Documents
titleTag: Documents — SDK reference · datars docs
description: doc(), the document's fields, the format version, and the signal and control helpers of @datars/sdk — signatures from the source.
lede: `doc()` assembles a document: it fills in defaults, drops what you left out and turns lambdas into expressions. Signals are the document's named values — what steps, interactions and the host page set, and what the scene and tables read.
---

## The document

{{sdk:doc}}

`doc()` returns the document's JSON (the IR). What it does on the way:

- **Defaults.** `size` becomes `{ width, height }` (default 800 × 480); `locale` defaults to `"en"`.
- **Themes.** A string names a built-in or registered theme (`{ use: "datars/neutral" }`); a `ThemeDef` object (it has a `name`) is registered and used; `{ use, themes, tokens }` is passed as written.
- **Clean-up.** Fields left `undefined` are dropped and lambdas become expressions, recursively.
- **The format version.** It stamps `datars: FORMAT`.

```ts
import { data, doc, group } from "@datars/sdk";

export default doc({
  title: "Sales by region",
  description: "Five regions' sales as bars.",   // opens the text alternative
  size: [640, 300],
  data: { sales: data.values({ region: ["N", "S"], v: [4, 2] }, { key: "region" }) },
  scene: group({ key: "root", children: [ /* … */ ] }),
});
```

{{sdk:DocDef}}

The prose guide to these fields is [Documents](/docs/documents/); the JSON they become is the [document format](/docs/ir/).

{{sdk:FORMAT}}

The version of the document format the SDK writes. Documents written for an older format are upgraded by `datars migrate doc.json --write`, which also reports fields it doesn't know.

## Signals

{{sdk:signal}}

Each helper returns a signal declaration for the document's `signals`. The name you give it is how expressions read it (`selected.has(d.region)`, `income * 0.3`) and how steps (`set: { income: 45000 }`), interactions (`{ set: "income", value }`) and hosts (`view.setSignal("income", 45000)`) write it.

| Helper | Declares |
|---|---|
| {{sig:signal.num}} | a number (default 0) |
| {{sig:signal.str}} | a string (default `""`) — the usual way to say which view a step shows |
| {{sig:signal.bool}} | a boolean |
| {{sig:signal.keyset}} | a set of keys — a selection; expressions ask `sel.has(k)`, `sel.isEmpty()`, `sel.size()` |
| {{sig:signal.key}} | one key, or `null` |
| {{sig:signal.range}} | a brushed range: `r.lo`, `r.hi`, `r.active` (see [`brush`](/docs/sdk/options/#brush)) |
| {{sig:signal.clock}} | a number that counts up at `rate` per second while the chart is settled and on screen |

```ts
signals: {
  view:     signal.str("bars"),
  selected: signal.keyset(),
  range:    signal.range(),
  spin:     signal.clock(8),        // 8 a second: a projection centre that turns
},
```

The engine also provides signals you don't declare — `state`, `step`, `narration.title`, `narration.text`, `viewport.w`, `viewport.h`, `sizeClass`, `inspected`, and inside a node `box.w`, `box.h` — listed in [expressions](/docs/expressions/#signals). The [brush and scrub figure](/docs/sdk/options/#interaction) shows signals set by dragging and by a story step.

## Controls

{{sdk:control}}

A control is a hint attached to a signal (`signal.num(30000, control.slider(10000, 80000, 1000, "Income"))`) for hosts and editors that build their own inputs. No host draws one today: to put a slider on the canvas, use the [`slider`](/docs/std/slider/) recipe, which sets the signal with [`scrub`](/docs/sdk/options/#scrub).

| Helper | Hints at |
|---|---|
| {{sig:control.slider}} | a range input |
| {{sig:control.toggle}} | a checkbox |
| {{sig:control.select}} | a choice from `options` |
