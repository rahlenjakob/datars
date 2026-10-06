---
title: Accessibility
description: What screen readers, keyboards and reduced-motion settings get from a datars chart on the web, iOS, Android and desktop — semantics, text alternatives, checks.
lede: Every mark can say what it means. The runtime turns that into what each platform's assistive technology understands — so a screen reader hears the chart's content, not "image".
---

## What a reader gets

Here is what a screen reader is told about a simple bar chart — the output of `datars semantics`, captured when this page was built:

{{run:semantics examples/votes/doc.json|24}}

That tree comes from the chart itself. On each platform the runtime hands it to the native accessibility system:

| Platform | What it becomes |
|---|---|
| Web (`<datars-view>`) | a hidden list of the chart's content beside the canvas (ARIA), a live region that announces each story step's narration, native range inputs for engine-drawn sliders, buttons for marks you can click, and real links over elements that link somewhere (a map's OpenStreetMap credit). The element itself is a focusable `figure` named after the chart. |
| iOS and iPadOS (DatarsKit) | VoiceOver elements with their frames, one per item; engine-drawn sliders are adjustable elements, clickable marks are activated with a double-tap |
| macOS (DatarsKit) | accessibility children with frames |
| Android | TalkBack nodes through an `AccessibilityNodeProvider`: virtual nodes with screen bounds, clickable marks, adjustable sliders |
| Desktop viewer | AccessKit (VoiceOver, Narrator/NVDA, Orca): the tree with frames, interactive marks as buttons, sliders adjustable |
| SVG poster, PDF | the poster is labelled; a PDF keeps its text as searchable text |

> **Status** The web mirror and iOS VoiceOver are built and tested. On Android the node tree has been checked on an emulator but not yet driven by TalkBack itself; on the desktop the AccessKit tree and its actions are unit-tested but not yet exercised with a real screen reader. The project's [status page]({{src}}/docs/19-status.md) tracks this.

## Semantics on marks

The standard library's recipes already label everything they draw: data marks as `datum` with `name: value`, axes and ticks, titles, legends, series. You mostly add semantics when you draw your own marks:

```ts
shape(geom.circle({ cx: e("scale.x(d.region)"), cy: e("scale.y(d.sales)"), r: 5 }), {
  key: e("d.region"),
  semantics: { role: "datum", label: e("`${key.name(d.region)}: ${format(d.sales, ',.0f')}`"), value: e("d.sales") },
})
```

| Field | Meaning |
|---|---|
| `role` | `group`, `datum`, `series`, `region`, `axis`, `tick`, `grid`, `legend`, `legend-item`, `annotation`, `title`, `label`, `tooltip`, `control`, `decoration` |
| `label` | what it's called — a string or an expression over the row |
| `value` | its value, when it has one |
| `link` | a URL it links to: the web runtime lays a real link over it, SVG and PDF keep it |

Use `role: "decoration"` for things that carry no data — gridlines, a background, orbits drawn for effect — so they stay out of the reader's way. Many recipes also take a `label` parameter to override their default accessible names:

```ts
bar({ labels: true, label: e("`${key.name(d.party)}: ${format(d.share, '.1f')} per cent`") })
```

For point clouds, `cloud({ name: "4,000,000 stars" })` gives the whole cloud a description instead of millions of items.

## Keyboard

On the web, a chart is focusable. With it focused:

| Key | Does |
|---|---|
| <kbd>→</kbd> or <kbd>Space</kbd> | next story step |
| <kbd>←</kbd> | previous step |
| <kbd>Esc</kbd> | back out of a drill-down chapter |
| <kbd>Tab</kbd> | into the chart's own controls: sliders as range inputs, clickable marks as buttons |

Engine-drawn sliders — like the one in the budget explorable — are offered as native `<input type="range">` elements, so the keyboard and screen readers operate them like any form control. Clickable marks (select, filter, drill down) become buttons in the mirror.

## Story steps are announced

A story step's narration — its `title` and `text` — goes to a polite live region on the web, so a screen reader announces each step as the reader moves through the story. Write narration as you'd write alt text: what the reader should notice, not what the chart looks like.

```ts
step("europe", { set: { focus: europe }, title: "Europe", text: "Across Europe the share ranges from 13% to 85%." })
```

## Reduced motion

Readers who ask their system for less motion get short crossfades instead of morphs, flights and staggers — on every platform, with nothing to configure:

| Platform | Follows |
|---|---|
| Web | `prefers-reduced-motion: reduce` (and autoplaying stories and clock signals pause) |
| iOS | Settings › Accessibility › Motion › Reduce Motion |
| macOS | System Settings › Accessibility › Display › Reduce motion |
| Android | Developer options / Accessibility › Remove animations (animator duration scale 0) |

Nothing in your document has to change to honour it. Autoplaying stories also pause when the chart scrolls out of view.

## Text alternatives in every bundle

Every published chart carries an accessible text chunk — its title, description, and each state's narration and labels — and an SVG poster, as its T0 variant. They are available before any script runs, to runtimes too old to play the chart, and to anything that can read HTML or SVG. This site shows that text under each chart (the *Text description* disclosure) — generated from the bundle, not written by hand.

Give every document a `title` and a `description`:

```ts
doc({
  title: "Renewable share of energy",
  description: "A world choropleth of the renewable share of energy use; the camera flies to Europe and the Nordic countries, which lead, then the countries become a ranked bar chart.",
  // …
})
```

## High contrast and colour

- **High-contrast mode** is one of every theme's three modes: pure ink on paper, stronger rules. On the web, `mode="high-contrast"`; in DatarsKit, `setMode(.highContrast)`.
- **Theme checks** run in `datars lint`: WCAG contrast of text tokens on their backgrounds, and categorical palettes that must stay distinct — also under simulated deuteranopia, protanopia and tritanopia. The built-in theme passes them in all three modes. See [Themes](/docs/theming/#checks).
- **Don't rely on colour alone.** Label data directly where you can (`bar({ labels: true })`, `line({ labels: true })`), and keep categorical colours to eight or fewer — `lint` warns past that.

## Checks in the toolchain

`datars lint` catches accessibility problems before readers do:

| Rule | Catches |
|---|---|
| `a11y/semantics` | interactive elements without an accessible label |
| `theme` | text tokens below their contrast minimum; palettes whose colours collapse under colour-vision deficiencies |
| `legibility/label-collisions` | labels that overlap |
| `legibility/offscreen` | labels cut off at the canvas edge |
| `encoding/too-many-colours` | more categorical colours than a reader can tell apart |

`datars semantics doc.ts --state 2` prints the tree for any state, so you (or a coding agent) can read a chart the way a screen reader will. See [Preview, check and debug](/docs/tools/).

## See also

- [Accessibility feature page](/features/accessibility/)
- [Web embedding](/docs/embed/web/) — the element's attributes and the mirror's parts.
- [Stories](/docs/stories/) — narration.
