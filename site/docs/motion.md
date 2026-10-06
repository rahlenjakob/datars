---
title: Motion
description: How datars animates every change by key — the defaults, motion rules for timing, easing, matching, staggers, routes and enter/exit effects, splitting bars into units, clocks, reduced motion and checking motion with film.
lede: You don't write animations. Every change — a step, a filter, new data, a resize, a theme — is planned from one scene to the next by key and played. Motion rules shape how.
---

## What you get without asking

When the scene changes, the engine pairs every mark in the old scene with its counterpart in the new one, and plans the motion:

- **Pairs morph.** Positions, sizes, colours and shapes interpolate — a bar into a slice, a country's outline into a bar, a line into its new values. Different shapes blend through an area-matched disc, so nothing folds or flips on the way.
- **New marks enter, gone marks exit** — fading, by default. Recipes bring their own: bars grow from the baseline, lines draw on from their start.
- **Parents split into their parts.** A mark whose key is the parent of keys on the other side — a party's bar, and that party's seats `("S", 1…107)` — splits into them, or they merge back into it. No rule needed: the keys say the seats are the bar's parts.
- **Text crossfades, numbers count.** Value labels tick through the numbers between.

The defaults: 0.9 seconds, `cubic-in-out`, all marks together, straight paths.

<figure class="fig"><div class="frame"><div class="chart" data-chart="shapes" data-caption></div></div><figcaption>One dataset, four shapes. Every party keeps its key, so bars become tiles, tiles split into squares, and squares gather into dots — with one motion rule.</figcaption>{{alt:shapes}}</figure>

## Pairing: keys and matchers

By default marks pair by their **key path** — the keys from the scene's root down to the mark. That's right when a chart changes its data: the bar for `SE` inside `"chart"` stays the bar for `SE`.

When one recipe becomes another — bars into a pie — the same party lives at different paths. Pair data marks by their own key instead:

```ts
motion: motion({ select: { role: "datum" }, matcher: "by-key" }),
```

(Data marks left unpaired by path are also tried by their own key automatically, when that key identifies exactly one mark on each side.)

| Matcher | Pairs |
|---|---|
| `"by-path"` | the same key path (the default) |
| `"by-key"` | the same own key, wherever it sits — cross-recipe morphs, map regions into bars |
| `{ hierarchy: { partition } }` | by path, then a parent splits into its child keys; `partition: "slices"` or `"grid"` says how its shape divides |
| `"nearest"` | unkeyed marks, by position (optimal assignment) |
| `"none"` | nothing: everything exits and enters — a crossfade |

If most marks cross-fade where you expected them to move, the keys don't match — `datars lint` says so, per transition.

## Motion rules

`motion(...rules)` takes rules like CSS for data-joined elements. Each rule applies to the transitions and elements it selects; more specific rules win, and for each field the last matching rule that sets it wins.

```ts
import { motion, choreo, route } from "@datars/sdk";

motion: motion(
  { duration: 2.2, easing: "cubic-in-out", matcher: "by-path" },
  { when: { from: "globe", to: "orbits" }, select: { role: "datum" }, route: route.spiral(0.35), choreo: choreo.ripple(0.55) },
  { when: { from: "orbits", to: "spiral" }, select: { role: "datum" }, route: route.arc(0.3), choreo: choreo.stagger("value", 0.5) },
  { when: { from: "spiral", to: "blocks" }, select: { role: "datum" }, choreo: choreo.wave(0.5, 0) },
),
```

That's the whole choreography of the [worlds example](/features/extensibility/): countries spiral off a globe with a ripple from the centre, arc into a golden spiral in order of size, and pack into blocks in a wave.

| Field | Meaning |
|---|---|
| `when` | `{ from, to }` — glob patterns over state names (`"*"`, `"map?"`); omit to match every transition |
| `select` | `{ role, kind, key }` — which elements: a semantic role (`datum`, `region`, `label`, `axis`, …), an element kind (`rect`, `arc`, `polyline`, `path`, `shape`, `text`, `instance`, …), or a key-path prefix (`"root/chart"`) |
| `duration`, `delay` | seconds |
| `easing` | see [easing](#easing) |
| `matcher` | see [pairing](#pairing-keys-and-matchers) |
| `choreo` | how elements are spread over the duration |
| `route` | the path each element travels |
| `morph` | for shapes of different kinds: `"disc"` (default), `"resample"` (point-wise outlines) or `"crossfade"` |
| `enter`, `exit` | what entering elements come from, and exiting ones go to |

### Choreography

| Helper | Elements move |
|---|---|
| `choreo.together()` | all at once (default) |
| `choreo.stagger(order, spread)` | one after another; `order` is `"data"`, `"left"`, `"right"`, `"center-out"`, `"value"` or `{ random: seed }`; `spread` is the share of the duration the start times spread over |
| `choreo.phased(exit, update, enter)` | exits first, then moves, then entries, as shares of the duration |
| `choreo.wave(spread, angle)` | in a wave rolling across the scene; `angle` in radians, 0 = left to right |
| `choreo.ripple(spread, origin?)` | outward from a point (the centre by default) |

### Routes

| Helper | Path |
|---|---|
| `route.straight()` | the direct line (default) |
| `route.arc(height)` | a curve bulging by `height` × the distance |
| `route.elbow()` | along x, then y — a clean re-sort |
| `route.spiral(turns)` | swirling around the scene centre |
| `route.explode()` | out from the centre, then in to land |
| `route.hop(height)` | a hop of `height` px on the way |
| `route.drift(seed, amount)` | each its own seeded wander, up to `amount` px |
| `route.drop(bounce)` | lifted, then dropped into place; `bounce` from 0 to 1 |

### Enter and exit

`enter` and `exit` describe a ghost: the state an element appears from or leaves to.

```ts
import { ghost } from "@datars/sdk";

{ select: { role: "datum" }, enter: ghost.grow("bottom") }        // bars grow up from their base
{ select: { kind: "polyline" }, enter: { trim: 0 } }              // lines draw on from their start
{ select: { role: "datum" }, enter: ghost.fromParent() }          // appear from the parent's place (drill-down)
{ select: { role: "label" }, exit: ghost.slide(0, 12) }           // fade out, dropping 12 px
```

| Ghost | Is |
|---|---|
| `ghost.fade()` | `{ opacity: 0 }` — the default |
| `ghost.grow(origin)` | `{ scale: 0, origin }` — from nothing; `origin` is `"center"`, `"bottom"`, `"top"`, `"left"`, `"right"`, `{ baseline: y }` or `{ point: [x, y] }` |
| `ghost.slide(dx, dy)` | fading in from an offset |
| `ghost.fromParent()` | from the parent datum's position in the other scene |
| `{ trim: 0 }` | none of the shape drawn: a line draws on (enter) or off (exit) |

Fields combine: `{ opacity: 0, scale: 0.6, origin: "center" }`.

### Easing

Easing strings, as in CSS and beyond:

- `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out`, `step-start`, `step-end`;
- families `quad`, `cubic`, `quart`, `quint`, `sine`, `expo`, `circ`, `back`, `elastic`, `bounce`, each with `-in`, `-out` or `-in-out` (`"cubic-in-out"`, `"back-out"`);
- `cubic-bezier(.2, .8, .2, 1)`;
- `spring(170, 26)` — stiffness, damping, and optionally mass and initial velocity;
- `steps(4)` or `steps(4, jump-start)`;
- `keyframes(0 0; 0.6 1.1 cubic-out; 1 1)` — time, value and an optional easing per segment.

## Recipes' own motion

Recipes declare motion defaults for the marks they make, scoped to where they're used: [`bar`](/docs/std/bar/) grows bars from the baseline (from the left when horizontal), [`line`](/docs/std/line/) draws lines on, charts built of units split out of their parents. Your document's rules always win over a recipe's defaults, and your own recipes can declare theirs the same way ([custom recipes](/docs/custom-recipes/)).

## Motion that never stops: clocks

A **clock signal** counts up while the chart sits still and is on screen — a globe that turns, a slow drift:

```ts
signals: { spin: signal.clock(8) },           // 8 units a second
// … a projection centred on 15 - spin turns eastward, a revolution every 45 s
```

A clock pauses during transitions (so a morph never jumps), off screen and for readers who prefer reduced motion. Renders, posters and tests see its default value.

## Reduced motion

Readers who ask their system for reduced motion get every transition as a short crossfade, on the web, iOS and Android alike, and autoplay stays off. Nothing to do in the document — but don't put meaning only in motion: the states themselves should tell the story.

## Checking motion

Motion is testable because a frame is a pure function of time.

```sh
datars film doc.ts --from bars --to pie --frames 8    # a filmstrip with motion trails
datars profile doc.ts                                 # every transition stepped at 60 fps: stall and per-frame cost
datars video doc.ts                                   # the whole program as an MP4
```

`film` draws evenly spaced frames of a transition side by side, with each mark's path traced — the quickest way to see a route, a stagger or a mark that pops instead of moving. In `datars dev`, every save morphs from what's on screen, so you watch your motion change as you edit it.
