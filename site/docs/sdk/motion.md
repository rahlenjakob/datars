---
title: Motion
titleTag: Motion — SDK reference · datars docs
description: Motion rules in @datars/sdk — motion(), Rule, easings, matchers, choreographies and stagger orders, routes and enter/exit ghosts — each with a live figure you can step through.
lede: Every change animates on its own: marks pair by key and morph. `motion(...rules)` shapes how — like CSS for data-joined marks, each rule picks transitions and elements and says how long, with what easing, in what order, along what path. Step through the figures; each panel is one rule.
---

## Rules

{{sdk:motion}}

`motion(...rules)` returns the document's (or a recipe's) `motion`. A rule with no `when` applies to every transition, with no `select` to every element. More specific rules win, and for each field the last matching rule that sets it wins; a recipe's own rules sit under the document's.

```ts
motion: motion(
  // Every transition, every element:
  { duration: 1.2, easing: "cubic-in-out" },
  // Data marks pair by their own key and start one after another from the left:
  { select: { role: "datum" }, matcher: "by-key",
    choreo: choreo.stagger("left", 0.5) },
  // From the bars to the pie only, under root/chart, along arcs:
  { when: { from: "bars", to: "pie" }, select: { key: "root/chart" },
    route: route.arc(0.3) },
  // New lines draw on:
  { select: { kind: "polyline" }, enter: { trim: 0 } },
)
```

{{sdk:Rule}}

`select.kind` is an element kind — a geometry (`rect`, `ellipse` for circles too, `arc`, `segment`, `polyline`, `area`, `path`, `symbol`), `shape` for any of them, `text` or `instance`. `morph` is how two shapes of different kinds blend: through an area-matched `"disc"` (default), point-wise `"resample"`, or `"crossfade"`. Defaults without rules: 0.9 s, `cubic-in-out`, all together, straight paths, fading in and out.

## Easing

A rule's `easing` is a string: `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `step-start`, `step-end`; a family — `quad`, `cubic`, `quart`, `quint`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce` — with `-in`, `-out` or `-in-out`; `cubic-bezier(x1, y1, x2, y2)`; `spring(stiffness, damping)` (optionally mass and initial velocity); `steps(n)` or `steps(n, jump-start)`; `keyframes(0 0; 0.6 1.1 cubic-out; 1 1)`. All of them are seekable — a scroll can scrub a spring.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-easings" data-phone-ratio="1.78" data-caption></div></div><figcaption>Ten easings, progress against time. Step: each dot moves across with a <code>linear</code> rule while its own rule moves it up with the panel's easing — so the engine traces the curve, overshoots included. <a href="{{src}}/site/figures/sdk/motion-easings.ts">Figure source</a></figcaption>{{alt:sdk-motion-easings}}</figure>

## Matching

{{sdk:Matcher}}

Before anything moves, marks of the old scene are paired with marks of the new one. Pairs morph; the rest enter and exit.

| Matcher | Pairs |
|---|---|
| `"by-path"` | the same key path — the default. Data marks left unpaired are also tried by their own key when it's unique on both sides. |
| `"by-key"` | the same own key, wherever it sits — cross-recipe morphs, a map region into a bar |
| `{ hierarchy: { partition } }` | by path, then a parent splits into its child keys (`"slices"` or `"grid"` says how its shape divides) |
| `"nearest"` | by position, whatever the keys (unkeyed marks) |
| `"none"` | nothing: everything crossfades |

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-matchers" data-phone-ratio="1.78" data-caption></div></div><figcaption>Six keyed dots move to a shuffled column in a different parent group. By path nothing pairs (they crossfade); by key each dot travels to its new place; by nearest position they slide straight across and take on the colours they land on. <a href="{{src}}/site/figures/sdk/motion-matchers.ts">Figure source</a></figcaption>{{alt:sdk-motion-matchers}}</figure>

## Choreography

{{sdk:choreo}}

| Helper | Spreads the rule's duration |
|---|---|
| {{sig:choreo.together}} | not at all: every element moves at once (default) |
| {{sig:choreo.stagger}} | one after another in `order`; `spread` is the share of the duration the start times spread over |
| {{sig:choreo.phased}} | exits first, then moves, then entries, as shares of the duration |
| {{sig:choreo.wave}} | as a wave across the scene; `angle` in radians, 0 = left to right |
| {{sig:choreo.ripple}} | outward from `origin` (the event's point, else the scene's centre) |

{{sdk:Choreo}}

{{sdk:Order}}

`"data"` is the elements' order in the scene (their rows' order), `"left"` and `"right"` their position across, `"center-out"` their distance from the middle of the elements the rule moves, `"value"` their `semantics.value`, largest first, and `{ random: seed }` a seeded shuffle.

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-orders" data-phone-ratio="0.89" data-caption></div></div><figcaption>The same twelve bars growing with <code>choreo.stagger(order, 0.75)</code> in six orders. The rows are listed shuffled, so <code>data</code> differs from <code>left</code>. <a href="{{src}}/site/figures/sdk/motion-orders.ts">Figure source</a></figcaption>{{alt:sdk-motion-orders}}</figure>

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-choreos" data-phone-ratio="0.83" data-caption></div></div><figcaption>Forty squares per grid growing and turning colour: all together, staggered from the left, in a diagonal wave, and rippling out from the centre of the scene. <a href="{{src}}/site/figures/sdk/motion-choreos.ts">Figure source</a></figcaption>{{alt:sdk-motion-choreos}}</figure>

## Routes

{{sdk:route}}

| Helper | The path |
|---|---|
| {{sig:route.straight}} | the direct line (default) |
| {{sig:route.arc}} | a curve bulging by `height` × the distance |
| {{sig:route.elbow}} | along x, then y — a clean re-sort |
| {{sig:route.spiral}} | swirling `turns` times around the scene's centre |
| {{sig:route.explode}} | out from the scene's centre, then in to land |
| {{sig:route.hop}} | a hop `height` px up on the way |
| {{sig:route.drift}} | each element's own seeded wander, up to `amount` px |
| {{sig:route.drop}} | lifted, then dropped into place; `bounce` 0 to 1 |

{{sdk:Route}}

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-routes" data-phone-ratio="1.56" data-caption></div></div><figcaption>Four dots swap sides in every panel, each along a different route. Spiral and explode work around the centre of the whole scene, so their dots leave their panel on the way. <a href="{{src}}/site/figures/sdk/motion-routes.ts">Figure source</a></figcaption>{{alt:sdk-motion-routes}}</figure>

## Enter and exit

{{sdk:ghost}}

| Helper | Is |
|---|---|
| {{sig:ghost.fade}} | `{ opacity: 0 }` — the default |
| {{sig:ghost.grow}} | `{ scale: 0, origin }`: from nothing, at `origin` |
| {{sig:ghost.fromParent}} | from the parent datum's place in the other scene (drill-downs) |
| {{sig:ghost.slide}} | fading in from an offset of `dx`, `dy` px |

{{sdk:Ghost}}

{{sdk:Origin}}

A rule's `enter` is the state entering elements come from; `exit` the state leaving ones go to. Fields combine (`{ opacity: 0, scale: 0.4, origin: "center" }`); `trim: 0` draws a line on (or off).

<figure class="fig"><div class="frame"><div class="chart" data-chart="sdk-motion-ghosts" data-phone-ratio="1.28" data-caption></div></div><figcaption>Six ghosts, each set as both <code>exit</code> and <code>enter</code>: step forward and the marks leave, back and they arrive. The last panel is a line with <code>{ trim: 0 }</code>, drawing off and on. <a href="{{src}}/site/figures/sdk/motion-ghosts.ts">Figure source</a></figcaption>{{alt:sdk-motion-ghosts}}</figure>

The guide, with recipes' own motion, clocks and reduced motion: [Motion](/docs/motion/). To see a transition as a filmstrip with motion trails: `datars film doc.ts --from 0 --to 1`.
