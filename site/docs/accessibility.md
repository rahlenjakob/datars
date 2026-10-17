---
title: Accessibility
description: What screen readers, keyboards, text-size, contrast and reduced-motion settings get from a datars chart on the web, iOS, Android and desktop — semantics, text alternatives, checks.
lede: Every mark can say what it means. The runtime turns that into what each platform's assistive technology understands — so a screen reader hears the chart's content, not "image".
---

## What a reader gets

Here is what a screen reader is told about a simple bar chart — the output of `datars semantics`, captured when this page was built:

{{run:semantics examples/votes/doc.json|24}}

That tree comes from the chart itself. Every host first names the chart — its `title` — and says what it shows: its `description`, then how much data and the range, from the state on screen (*"Kilograms of each fruit a stall sold. 3 data points, from 18 to 42."*). Then the content, in one reading order: headings for the parts, an axis or a legend as one line (*"y axis: 0, 10, 20, 30, 40"*), each data mark with its name and its value apart, marks repeated in a row as one with a count (a parliament's *"S × 107"*), marks that have no value — a world map's countries without data — as one line (*"215 regions without data"*) rather than a stop each, and whatever a click acts on as a button. On each platform that becomes what its assistive technology understands:

| Platform | What it becomes |
|---|---|
| Web (`<datars-view>`) | The element is a focusable `figure`, named by the chart (`aria-label`) with the summary as its `aria-description`. Beside the canvas (hidden from the eye, the canvas hidden from screen readers): named groups for the parts, each run of data marks a `<table>` — name and value columns, so table commands read down a column — engine-drawn sliders and selects as native `<input type="range">` and `<select>`, buttons for marks you can click, real links over elements that link somewhere (a map's OpenStreetMap credit), and a live region that announces each story step once. |
| iOS and iPadOS (DatarsKit) | VoiceOver elements with their frames: the chart first (a summary element), then its controls (sliders adjustable, saying their value as drawn: *"32,000"*), headings in the rotor, data marks with their values; double-tap runs a clickable mark; a new step is announced. |
| macOS (DatarsKit) | the chart a group with its summary as help, the reading order as its children, buttons that press; steps announced |
| Android | TalkBack virtual nodes through an `AccessibilityNodeProvider`, in the same order, with screen bounds: headings, values, seek bars for sliders, spinners for selects, buttons; a new step is announced |
| Desktop viewer | AccessKit (VoiceOver, Narrator/NVDA, Orca): a figure named by the chart with its summary as description, sliders adjustable, buttons for what a click acts on |
| SVG poster, PDF | the poster is labelled; a PDF keeps its text as searchable text |

> **Status** The web runtime's accessibility tree is checked in Chrome by a browser test (name, summary, data tables, the keyboard path, the live region) — not yet with a screen reader driving it. The VoiceOver elements have been dumped in the iOS simulator and the TalkBack nodes on an Android emulator (`uiautomator`); neither has been driven by VoiceOver or TalkBack themselves, and the desktop's AccessKit tree is unit-tested only. The project's [status page]({{src}}/docs/19-status.md) tracks this.

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
| `kind`, `checked` | a control's native kind — `checkbox`, `switch` or `radio` (one of a set) — and its state: each host gives it the platform's own role (`role="switch"` with `aria-checked`, a toggle trait, Android's `Switch`, AccessKit's toggles), which says the state in the reader's words. The label stays the name. The standard library's toggle, checklist, segmented control and select do this. |

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
| <kbd>Tab</kbd> | into the chart's own controls (sliders as range inputs, selects, buttons), then into its data — one stop |
| <kbd>↑</kbd> <kbd>↓</kbd> (or <kbd>←</kbd> <kbd>→</kbd>), <kbd>Home</kbd>, <kbd>End</kbd> | in the data: the previous or next mark, the first or the last; the mark is outlined on the chart and named beside it, as a pointer's tooltip would |
| <kbd>Enter</kbd> | on a clickable mark: its click (select, filter, drill down) |

Engine-drawn sliders — like the one in the budget explorable — are offered as native `<input type="range">` elements, so the keyboard and screen readers operate them like any form control. Clickable marks become buttons.

## Story steps are announced

A story step's narration — its `title` and `text` — is announced once when the reader moves to it: through a polite live region on the web, `UIAccessibility` announcements on iOS and macOS, `announceForAccessibility` on Android. A step without narration says where the reader is (*"Step 2 of 4"*). Write narration as you'd write alt text: what the reader should notice, not what the chart looks like.

```ts
step("europe", { set: { focus: europe }, title: "Europe", text: "Across Europe the share ranges from 13% to 85%." })
```

## Text size

Readers who make text bigger — the browser's font size, Dynamic Type, Android's font size — get charts laid out for it, not scaled pictures: the theme's text sizes (`size.small`, `size.label`, `size.body`, `size.title`) grow by the reader's factor, so axes widen for their labels, numeric axes ask for fewer ticks, category names wrap or turn, legends and controls grow their rows, and labels that would still pile up give way. The chart moves to the new layout like any other change, live.

| Platform | Follows | Override |
|---|---|---|
| Web | the browser's default font size (20 px is 1.25 ×), not the page's own root size | `text-scale="1.5"`, `textScale` (React: `textScale`) |
| iOS, iPadOS | Dynamic Type (≈ 0.8 × at xSmall to 3.1 × at the largest accessibility size) | `view.textScale` |
| Android | Settings › Display › Font size (`Configuration.fontScale`) | `view.textScale` |
| Desktop viewer | Ctrl/⌘ +, − and 0 | `datars-view --text-scale 1.5` |

The engine keeps it between 0.75 and 3. Size classes are judged in the reader's text — an 800 px chart at 2 × lays out as a phone one — so a document that chooses less detail on phones chooses it for big text too. Text your document sizes with the theme's tokens (`"$size.label"`, `token("size.label")`) follows; a literal size stays as you wrote it. Check a chart the way such a reader gets it with `datars render doc.ts --text-scale 2`, or all of them with `datars labels`.

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

- **High-contrast mode** is one of every theme's three modes: pure ink on paper, stronger rules. On the web it's what a reader who asks for more contrast (`prefers-contrast: more`) gets, or set it with `mode="high-contrast"`; in DatarsKit, `setMode(.highContrast)`.
- **Forced colours** (Windows contrast themes): the browser can't recolour a canvas, so the web runtime paints the chart's text, rules and surfaces in the system's colours itself (CanvasText on Canvas, Highlight for the accent and the focus ring); the data keeps the colours that tell series apart.
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
| `legibility/clipped` | labels their box's clip cuts (a column too narrow for its names) |
| `encoding/too-many-colours` | more categorical colours than a reader can tell apart |

`datars lint doc.ts --size 390x600` lints the layout a phone gets. `datars labels doc.ts` (or a folder of documents) reads every state at a phone, a tablet and a wide column, each at 1, 1.5 and 2 × text, and lists the labels a reader couldn't read there — the repository runs it over its examples in `datars test`, where an example getting harder to read fails.

`datars semantics doc.ts --state 2` prints the tree for any state, so you (or a coding agent) can read a chart the way a screen reader will. See [Preview, check and debug](/docs/tools/).

## See also

- [Accessibility feature page](/features/accessibility/)
- [Web embedding](/docs/embed/web/) — the element's attributes and the mirror's parts.
- [Stories](/docs/stories/) — narration.
